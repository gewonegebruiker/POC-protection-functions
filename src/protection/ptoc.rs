/// PTOC (Time Overcurrent Protection) implementation
use super::traits::{ProtectionFunction, ProtectionResult, TripState};
use crate::config::PtocConfig;
use std::time::Duration;

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

        match self.state {
            TripState::Idle => {
                if is_overcurrent {
                    // Current exceeded pickup, start timing
                    self.state = TripState::Pickup;
                    self.pickup_time = Some(timestamp);
                    ProtectionResult::TripPending(Duration::from_millis(self.config.tset))
                } else {
                    ProtectionResult::NoTrip
                }
            }
            TripState::Pickup => {
                if !is_overcurrent {
                    // Current dropped below pickup, reset
                    self.state = TripState::Idle;
                    self.pickup_time = None;
                    ProtectionResult::NoTrip
                } else {
                    // Check if time delay has expired
                    if let Some(elapsed) = self.time_since_pickup(timestamp) {
                        if elapsed >= self.config.tset {
                            // Time delay expired, issue trip
                            self.state = TripState::Trip;
                            ProtectionResult::Trip
                        } else {
                            // Still waiting for time delay
                            let remaining = self.config.tset - elapsed;
                            ProtectionResult::TripPending(Duration::from_millis(remaining))
                        }
                    } else {
                        // This shouldn't happen, but handle it gracefully
                        self.pickup_time = Some(timestamp);
                        ProtectionResult::TripPending(Duration::from_millis(self.config.tset))
                    }
                }
            }
            TripState::Trip => {
                // Once tripped, stay tripped until reset
                ProtectionResult::Trip
            }
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
