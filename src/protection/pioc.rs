//! PIOC — Instantaneous Overcurrent Protection.
//!
//! Trips immediately (no intentional time delay) when the measured current
//! exceeds the pickup setting.  This meets IEC 61850-5 performance class P1
//! (TT6 ≤ 3 ms).
//!
//! Two input modes are supported via [`PiocInputMode`]:
//! - `Instantaneous`: compares `|sample|` against `iset` as a **peak** threshold.
//! - `ShortWindowRms(n)`: maintains an n-sample sliding-window RMS internally
//!   and compares the RMS against `iset` as an **RMS** threshold.

use super::traits::{ProtectionFunction, ProtectionResult, TripState};
use crate::config::{PiocConfig, PiocInputMode};

/// Instantaneous Overcurrent Protection (PIOC).
///
/// State machine: `Idle → Trip` (no intermediate Pickup state).
pub struct Pioc {
    config: PiocConfig,
    state: TripState,
    /// Circular buffer for `ShortWindowRms` mode. Empty when `Instantaneous`.
    buf: Vec<f64>,
    buf_head: usize,
    sum_sq: f64,
    buf_count: usize,
}

fn window_size(mode: &PiocInputMode) -> usize {
    match mode {
        PiocInputMode::ShortWindowRms(n) => *n,
        PiocInputMode::Instantaneous => 0,
    }
}

impl Pioc {
    /// Create a new PIOC function.
    pub fn new(config: PiocConfig) -> Self {
        let cap = window_size(&config.input_mode);
        Self {
            config,
            state: TripState::Idle,
            buf: vec![0.0; cap],
            buf_head: 0,
            sum_sq: 0.0,
            buf_count: 0,
        }
    }

    /// Current trip state.
    pub fn state(&self) -> TripState {
        self.state
    }

    /// Update configuration; resets the function.
    pub fn set_config(&mut self, config: PiocConfig) {
        let cap = window_size(&config.input_mode);
        self.config = config;
        self.buf = vec![0.0; cap];
        self.buf_head = 0;
        self.sum_sq = 0.0;
        self.buf_count = 0;
        if !self.config.enabled {
            self.state = TripState::Idle;
        }
    }

    /// Get the pickup current setting.
    pub fn iset(&self) -> f64 {
        self.config.iset
    }

    /// Push one sample into the ring buffer and return the current window RMS.
    fn push_rms(&mut self, sample: f64) -> f64 {
        let cap = self.buf.len();
        if self.buf_count == cap {
            self.sum_sq -= self.buf[self.buf_head] * self.buf[self.buf_head];
        } else {
            self.buf_count += 1;
        }
        self.buf[self.buf_head] = sample;
        self.sum_sq += sample * sample;
        self.buf_head = (self.buf_head + 1) % cap;
        (self.sum_sq / self.buf_count as f64).sqrt()
    }
}

impl ProtectionFunction for Pioc {
    /// Evaluate PIOC against the supplied current sample.
    ///
    /// In `Instantaneous` mode, `current` is treated as an instantaneous value
    /// and `|current|` is compared to `iset` as a **peak** threshold.
    /// In `ShortWindowRms(n)` mode the function accumulates samples internally
    /// and compares the window RMS against `iset` as an **RMS** threshold.
    fn process(&mut self, current: f64, _timestamp: u64) -> ProtectionResult {
        if !self.config.enabled {
            return ProtectionResult::Disabled;
        }

        let value = match &self.config.input_mode {
            PiocInputMode::Instantaneous => current.abs(),
            PiocInputMode::ShortWindowRms(_) => self.push_rms(current),
        };

        match self.state {
            TripState::Idle => {
                if value > self.config.iset {
                    self.state = TripState::Trip;
                    ProtectionResult::Trip
                } else {
                    ProtectionResult::NoTrip
                }
            }
            TripState::Trip | TripState::Pickup => ProtectionResult::Trip,
        }
    }

    fn reset(&mut self) {
        self.state = TripState::Idle;
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.buf_head = 0;
        self.sum_sq = 0.0;
        self.buf_count = 0;
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
    use crate::config::PiocInputMode;

    fn make_config(iset: f64, enabled: bool) -> PiocConfig {
        PiocConfig { iset, enabled, ..PiocConfig::default() }
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

    #[test]
    fn test_short_window_rms_no_trip_below_pickup() {
        let config = PiocConfig {
            iset: 100.0, enabled: true,
            input_mode: PiocInputMode::ShortWindowRms(4),
        };
        let mut pioc = Pioc::new(config);
        // DC 50 A → RMS = 50, below iset = 100
        for _ in 0..8 {
            assert_eq!(pioc.process(50.0, 0), ProtectionResult::NoTrip);
        }
    }

    #[test]
    fn test_short_window_rms_trips_above_pickup() {
        let config = PiocConfig {
            iset: 100.0, enabled: true,
            input_mode: PiocInputMode::ShortWindowRms(4),
        };
        let mut pioc = Pioc::new(config);
        // DC 150 A → RMS = 150, above iset = 100 → trip on first sample
        assert_eq!(pioc.process(150.0, 0), ProtectionResult::Trip);
    }

    #[test]
    fn test_short_window_rms_reset_clears_buffer() {
        let config = PiocConfig {
            iset: 100.0, enabled: true,
            input_mode: PiocInputMode::ShortWindowRms(4),
        };
        let mut pioc = Pioc::new(config);
        pioc.process(200.0, 0);
        assert_eq!(pioc.state(), TripState::Trip);
        pioc.reset();
        assert_eq!(pioc.state(), TripState::Idle);
        // After reset, low current should give NoTrip
        assert_eq!(pioc.process(50.0, 1), ProtectionResult::NoTrip);
    }
}
