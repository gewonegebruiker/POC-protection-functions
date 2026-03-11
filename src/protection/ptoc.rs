/// PTOC (Time Overcurrent Protection) implementation
use super::traits::{ProtectionFunction, ProtectionResult, TripState};
use crate::config::{PtocConfig, PtocCurve};
use std::time::Duration;

/// Compute the effective trip delay in milliseconds for a given curve and current ratio.
///
/// For `DefiniteTime`, returns `tset` unchanged.
/// For inverse-time curves, `tset` acts as the time multiplier setting (TMS):
/// `t_ms = tset × k / ((ratio)^α − 1)`, clamped to a minimum of 1 ms.
/// Returns `u64::MAX` if `ratio ≤ 1.0` (current at or below pickup — no trip).
pub(crate) fn effective_trip_delay_ms(curve: &PtocCurve, ratio: f64, tset: u64) -> u64 {
    match curve {
        PtocCurve::DefiniteTime => tset,
        _ => {
            if ratio <= 1.0 {
                return u64::MAX;
            }
            let (k, alpha): (f64, f64) = match curve {
                PtocCurve::IecStandardInverse  => (0.14,  0.02),
                PtocCurve::IecVeryInverse       => (13.5,  1.0),
                PtocCurve::IecExtremelyInverse  => (80.0,  2.0),
                PtocCurve::DefiniteTime         => unreachable!(),
            };
            let denom = ratio.powf(alpha) - 1.0;
            if denom <= 0.0 {
                return u64::MAX;
            }
            let t_ms = tset as f64 * k / denom;
            (t_ms.ceil() as u64).max(1)
        }
    }
}

/// PTOC protection function with definite time characteristic
pub struct Ptoc {
    config: PtocConfig,
    state: TripState,
    pickup_time: Option<u64>,
}

impl Ptoc {
    /// Create a new PTOC function with the given configuration
    pub fn new(config: PtocConfig) -> Self {
        Self {
            config,
            state: TripState::Idle,
            pickup_time: None,
        }
    }

    /// Get the current trip state
    pub fn state(&self) -> TripState {
        self.state
    }

    /// Get the configuration
    pub fn config(&self) -> &PtocConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: PtocConfig) {
        self.config = config;
        // Reset if disabled
        if !self.config.enabled {
            self.reset();
        }
    }

    /// Get the pickup current setting (Iset)
    pub fn iset(&self) -> f64 {
        self.config.iset
    }

    /// Get the time delay setting (Tset) in milliseconds
    pub fn tset(&self) -> u64 {
        self.config.tset
    }

    /// Check if current exceeds pickup setting
    fn is_overcurrent(&self, current: f64) -> bool {
        current > self.config.iset
    }

    /// Check if current is below the dropout threshold (for Pickup → Idle reset).
    fn is_below_dropout(&self, current: f64) -> bool {
        current < self.config.iset * self.config.dropout_ratio
    }

    /// Effective trip delay for the current value, considering the configured curve.
    fn trip_delay_ms(&self, current: f64) -> u64 {
        let ratio = if self.config.iset > 0.0 { current / self.config.iset } else { 0.0 };
        effective_trip_delay_ms(&self.config.curve, ratio, self.config.tset)
    }

    /// Calculate time elapsed since pickup in milliseconds
    fn time_since_pickup(&self, current_time: u64) -> Option<u64> {
        self.pickup_time.map(|pickup| {
            // Convert microseconds to milliseconds
            (current_time.saturating_sub(pickup)) / 1000
        })
    }
}

impl ProtectionFunction for Ptoc {
    fn process(&mut self, current: f64, timestamp: u64) -> ProtectionResult {
        if !self.config.enabled {
            return ProtectionResult::Disabled;
        }

        let is_overcurrent = self.is_overcurrent(current);

        let delay_ms = self.trip_delay_ms(current);

        match self.state {
            TripState::Idle => {
                if is_overcurrent {
                    self.state = TripState::Pickup;
                    self.pickup_time = Some(timestamp);
                    ProtectionResult::TripPending(Duration::from_millis(delay_ms))
                } else {
                    ProtectionResult::NoTrip
                }
            }
            TripState::Pickup => {
                if self.is_below_dropout(current) {
                    // Current dropped below dropout threshold — reset
                    self.state = TripState::Idle;
                    self.pickup_time = None;
                    ProtectionResult::NoTrip
                } else {
                    // Check if effective time delay has expired
                    if let Some(elapsed) = self.time_since_pickup(timestamp) {
                        if elapsed >= delay_ms {
                            self.state = TripState::Trip;
                            ProtectionResult::Trip
                        } else {
                            let remaining = delay_ms.saturating_sub(elapsed);
                            ProtectionResult::TripPending(Duration::from_millis(remaining))
                        }
                    } else {
                        self.pickup_time = Some(timestamp);
                        ProtectionResult::TripPending(Duration::from_millis(delay_ms))
                    }
                }
            }
            TripState::Trip => ProtectionResult::Trip,
        }
    }

    fn reset(&mut self) {
        self.state = TripState::Idle;
        self.pickup_time = None;
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    fn name(&self) -> &str {
        "PTOC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptoc_no_trip_below_pickup() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Current below pickup
        let result = ptoc.process(50.0, 0);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_ptoc_trip_pending() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Current exceeds pickup
        let result = ptoc.process(150.0, 0);
        assert!(matches!(result, ProtectionResult::TripPending(_)));
        assert_eq!(ptoc.state(), TripState::Pickup);
    }

    #[test]
    fn test_ptoc_trip_after_delay() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Current exceeds pickup at t=0
        ptoc.process(150.0, 0);

        // Still overcurrent at t=50ms (50000 microseconds)
        let result = ptoc.process(150.0, 50_000);
        assert!(matches!(result, ProtectionResult::TripPending(_)));

        // Still overcurrent at t=100ms (100000 microseconds) - should trip
        let result = ptoc.process(150.0, 100_000);
        assert_eq!(result, ProtectionResult::Trip);
        assert_eq!(ptoc.state(), TripState::Trip);
    }

    #[test]
    fn test_ptoc_reset_on_current_drop() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Current exceeds pickup
        ptoc.process(150.0, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);

        // Current drops before delay expires
        let result = ptoc.process(50.0, 50_000);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_ptoc_disabled() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: false,
        };
        let mut ptoc = Ptoc::new(config);

        // Current exceeds pickup but function is disabled
        let result = ptoc.process(150.0, 0);
        assert_eq!(result, ProtectionResult::Disabled);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_ptoc_dropout_hysteresis_no_reset_above_threshold() {
        // dropout_ratio = 0.95 → dropout threshold = 95 A
        // Current drops to 96 A — above dropout threshold, must stay in Pickup
        let config = PtocConfig { iset: 100.0, tset: 100, enabled: true, dropout_ratio: 0.95, ..PtocConfig::default() };
        let mut ptoc = Ptoc::new(config);
        ptoc.process(150.0, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);
        let result = ptoc.process(96.0, 50_000);
        assert!(matches!(result, ProtectionResult::TripPending(_)));
        assert_eq!(ptoc.state(), TripState::Pickup);
    }

    #[test]
    fn test_ptoc_dropout_hysteresis_resets_below_threshold() {
        // Current drops to 94 A — below dropout threshold (95 A), must reset to Idle
        let config = PtocConfig { iset: 100.0, tset: 100, enabled: true, dropout_ratio: 0.95, ..PtocConfig::default() };
        let mut ptoc = Ptoc::new(config);
        ptoc.process(150.0, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);
        let result = ptoc.process(94.0, 50_000);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_ptoc_inverse_time_trips_faster_at_higher_current() {
        use crate::config::PtocCurve;
        // IEC Very Inverse: t = tset × 13.5 / (ratio − 1)
        // At ratio=2 (200A): t = 100 × 13.5 / 1 = 1350 ms
        // At ratio=5 (500A): t = 100 × 13.5 / 4 = 337 ms
        let config = PtocConfig {
            iset: 100.0, tset: 100, enabled: true,
            dropout_ratio: 0.95, curve: PtocCurve::IecVeryInverse,
        };
        let delay_2x = effective_trip_delay_ms(&config.curve, 2.0, config.tset);
        let delay_5x = effective_trip_delay_ms(&config.curve, 5.0, config.tset);
        assert!(delay_5x < delay_2x, "Higher overcurrent should trip faster");
        assert_eq!(delay_2x, 1350);
        assert_eq!(delay_5x, 338); // ceil(337.5)
    }

    #[test]
    fn test_ptoc_reset() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Trip the function
        ptoc.process(150.0, 0);
        ptoc.process(150.0, 100_000);
        assert_eq!(ptoc.state(), TripState::Trip);

        // Reset
        ptoc.reset();
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_ptoc_stays_tripped() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: true,
        };
        let mut ptoc = Ptoc::new(config);

        // Trip the function
        ptoc.process(150.0, 0);
        ptoc.process(150.0, 100_000);
        assert_eq!(ptoc.state(), TripState::Trip);

        // Even if current drops, should stay tripped
        let result = ptoc.process(50.0, 200_000);
        assert_eq!(result, ProtectionResult::Trip);
        assert_eq!(ptoc.state(), TripState::Trip);
    }
}
