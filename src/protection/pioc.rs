//! PIOC — Instantaneous Overcurrent Protection.
//!
//! Trips immediately (no intentional time delay) when the measured current
//! exceeds the pickup setting.  This meets IEC 61850-5 performance class P1
//! (TT6 ≤ 3 ms).

use super::traits::{ProtectionFunction, ProtectionResult, TripState};
use crate::config::PiocConfig;

/// Instantaneous Overcurrent Protection (PIOC).
///
/// State machine: `Idle → Trip` (no intermediate Pickup state).
pub struct Pioc {
    config: PiocConfig,
    state: TripState,
}

impl Pioc {
    /// Create a new PIOC function.
    pub fn new(config: PiocConfig) -> Self {
        Self {
            config,
            state: TripState::Idle,
        }
    }

    /// Current trip state.
    pub fn state(&self) -> TripState {
        self.state
    }

    /// Update configuration; resets the function if it is being disabled.
    pub fn set_config(&mut self, config: PiocConfig) {
        self.config = config;
        if !self.config.enabled {
            self.reset();
        }
    }

    /// Get the pickup current setting.
    pub fn iset(&self) -> f64 {
        self.config.iset
    }
}

impl ProtectionFunction for Pioc {
    /// Evaluate PIOC against the supplied RMS current.
    fn process(&mut self, current: f64, _timestamp: u64) -> ProtectionResult {
        if !self.config.enabled {
            return ProtectionResult::Disabled;
        }

        match self.state {
            TripState::Idle => {
                if current > self.config.iset {
                    self.state = TripState::Trip;
                    ProtectionResult::Trip
                } else {
                    ProtectionResult::NoTrip
                }
            }
            TripState::Trip => ProtectionResult::Trip,
            TripState::Pickup => {
                // PIOC should never enter Pickup, but handle defensively
                ProtectionResult::Trip
            }
        }
    }

    fn reset(&mut self) {
        self.state = TripState::Idle;
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
        "PIOC"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(iset: f64, enabled: bool) -> PiocConfig {
        PiocConfig { iset, enabled }
    }

    #[test]
    fn test_no_trip_below_pickup() {
        let mut pioc = Pioc::new(make_config(500.0, true));
        let result = pioc.process(300.0, 0);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(pioc.state(), TripState::Idle);
    }

    #[test]
    fn test_immediate_trip_above_pickup() {
        let mut pioc = Pioc::new(make_config(500.0, true));
        let result = pioc.process(600.0, 0);
        assert_eq!(result, ProtectionResult::Trip);
        assert_eq!(pioc.state(), TripState::Trip);
    }

    #[test]
    fn test_no_time_delay() {
        // A single call above pickup → trip immediately (t=0)
        let mut pioc = Pioc::new(make_config(100.0, true));
        assert_eq!(pioc.process(200.0, 0), ProtectionResult::Trip);
    }

    #[test]
    fn test_stays_tripped() {
        let mut pioc = Pioc::new(make_config(500.0, true));
        pioc.process(600.0, 0);
        assert_eq!(pioc.state(), TripState::Trip);

        // Even when current drops, stays tripped until reset
        let result = pioc.process(100.0, 1000);
        assert_eq!(result, ProtectionResult::Trip);
    }

    #[test]
    fn test_disabled() {
        let mut pioc = Pioc::new(make_config(500.0, false));
        let result = pioc.process(1000.0, 0);
        assert_eq!(result, ProtectionResult::Disabled);
        assert_eq!(pioc.state(), TripState::Idle);
    }

    #[test]
    fn test_reset() {
        let mut pioc = Pioc::new(make_config(500.0, true));
        pioc.process(600.0, 0);
        assert_eq!(pioc.state(), TripState::Trip);

        pioc.reset();
        assert_eq!(pioc.state(), TripState::Idle);
    }

    #[test]
    fn test_exact_pickup_no_trip() {
        // Strictly greater-than required; equality is no trip
        let mut pioc = Pioc::new(make_config(500.0, true));
        let result = pioc.process(500.0, 0);
        assert_eq!(result, ProtectionResult::NoTrip);
    }

    #[test]
    fn test_set_enabled() {
        let mut pioc = Pioc::new(make_config(500.0, true));
        pioc.process(600.0, 0);
        assert_eq!(pioc.state(), TripState::Trip);

        pioc.set_enabled(false);
        assert_eq!(pioc.state(), TripState::Idle);
        assert_eq!(pioc.process(1000.0, 1000), ProtectionResult::Disabled);
    }
}
