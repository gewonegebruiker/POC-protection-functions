/// Traits for protection functions
use std::time::Duration;

/// Result of a protection function evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum ProtectionResult {
    /// No trip condition detected
    NoTrip,
    /// Trip condition detected with specified delay
    TripPending(Duration),
    /// Trip condition is active (delay expired)
    Trip,
    /// Function is disabled
    Disabled,
}

/// Generic trait for protection functions
pub trait ProtectionFunction {
    /// Process a new current measurement
    /// 
    /// # Arguments
    /// * `current` - RMS current value in primary amperes
    /// * `timestamp` - Timestamp of the measurement (microseconds)
    /// 
    /// # Returns
    /// Protection result indicating trip status
    fn process(&mut self, current: f64, timestamp: u64) -> ProtectionResult;

    /// Reset the protection function to initial state
    fn reset(&mut self);

    /// Check if the function is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable the function
    fn set_enabled(&mut self, enabled: bool);

    /// Get the name of the protection function
    fn name(&self) -> &str;
}

/// Trip state for protection functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TripState {
    /// Function is idle (no overcurrent)
    Idle,
    /// Overcurrent detected, waiting for time delay
    Pickup,
    /// Time delay expired, trip active
    Trip,
}

impl TripState {
    /// Check if the state is tripped
    pub fn is_tripped(&self) -> bool {
        matches!(self, TripState::Trip)
    }

    /// Check if the state is pickup (pending trip)
    pub fn is_pickup(&self) -> bool {
        matches!(self, TripState::Pickup)
    }

    /// Check if the state is idle
    pub fn is_idle(&self) -> bool {
        matches!(self, TripState::Idle)
    }
}
