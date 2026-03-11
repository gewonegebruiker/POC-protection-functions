//! Sliding Window PTOC — evaluates protection on every sample using
//! an incremental O(1) RMS calculation for minimum detection latency.

use super::ptoc::effective_trip_delay_ms;
use super::traits::{ProtectionFunction, ProtectionResult, TripState};
use crate::config::PtocConfig;
use std::time::Duration;

/// PTOC with a sliding window RMS that updates on every sample.
///
/// Uses incremental sum-of-squares to avoid O(n) recalculation:
/// `sum_sq += new² - old²`
/// This gives O(1) RMS update per sample, and the protection decision
/// is evaluated every 250 µs (at 4 kSa/s), achieving P1/P3 compliance.
pub struct PtocSlidingWindow {
    config: PtocConfig,
    window_size: usize,
    buffer: Vec<f64>,
    head: usize,
    count: usize,
    sum_sq: f64,
    state: TripState,
    pickup_time: Option<u64>,
}

impl PtocSlidingWindow {
    /// Create a new sliding-window PTOC.
    ///
    /// # Arguments
    /// * `config`      – Protection settings (iset, tset, enabled).
    /// * `window_size` – Number of samples in the RMS window
    ///   (default 80 = one cycle at 50 Hz / 4 kSa/s).
    pub fn new(config: PtocConfig, window_size: usize) -> Self {
        Self {
            config,
            window_size,
            buffer: vec![0.0; window_size],
            head: 0,
            count: 0,
            sum_sq: 0.0,
            state: TripState::Idle,
            pickup_time: None,
        }
    }

    /// Create with the default window size of 80 samples.
    pub fn new_default(config: PtocConfig) -> Self {
        Self::new(config, 80)
    }

    /// Process a single sample (already scaled to primary amperes).
    ///
    /// Returns the protection result based on the current RMS computed from
    /// the sliding window.
    pub fn process_sample(&mut self, sample: f64, timestamp: u64) -> ProtectionResult {
        if !self.config.enabled {
            return ProtectionResult::Disabled;
        }

        // --- incremental RMS update ---
        let old = self.buffer[self.head];
        self.sum_sq += sample * sample - old * old;
        // Guard against floating-point drift below zero
        if self.sum_sq < 0.0 {
            self.sum_sq = 0.0;
        }
        self.buffer[self.head] = sample;
        self.head = (self.head + 1) % self.window_size;
        if self.count < self.window_size {
            self.count += 1;
        }

        // Only evaluate after the window is fully populated
        if self.count < self.window_size {
            return ProtectionResult::NoTrip;
        }

        let rms = (self.sum_sq / self.window_size as f64).sqrt();
        self.evaluate(rms, timestamp)
    }

    /// Current trip state.
    pub fn state(&self) -> TripState {
        self.state
    }

    /// Update configuration; resets the function if it is being disabled.
    pub fn set_config(&mut self, config: PtocConfig) {
        self.config = config;
        if !self.config.enabled {
            self.reset();
        }
    }

    /// Window size (number of samples).
    pub fn window_size(&self) -> usize {
        self.window_size
    }

    // --- private helpers ---

    fn evaluate(&mut self, rms: f64, timestamp: u64) -> ProtectionResult {
        let overcurrent = rms > self.config.iset;
        let below_dropout = rms < self.config.iset * self.config.dropout_ratio;
        let ratio = if self.config.iset > 0.0 { rms / self.config.iset } else { 0.0 };
        let delay_ms = effective_trip_delay_ms(&self.config.curve, ratio, self.config.tset);

        match self.state {
            TripState::Idle => {
                if overcurrent {
                    self.state = TripState::Pickup;
                    self.pickup_time = Some(timestamp);
                    ProtectionResult::TripPending(Duration::from_millis(delay_ms))
                } else {
                    ProtectionResult::NoTrip
                }
            }
            TripState::Pickup => {
                if below_dropout {
                    self.state = TripState::Idle;
                    self.pickup_time = None;
                    ProtectionResult::NoTrip
                } else if let Some(pickup) = self.pickup_time {
                    let elapsed_ms = timestamp.saturating_sub(pickup) / 1000;
                    if elapsed_ms >= delay_ms {
                        self.state = TripState::Trip;
                        ProtectionResult::Trip
                    } else {
                        let remaining = delay_ms.saturating_sub(elapsed_ms);
                        ProtectionResult::TripPending(Duration::from_millis(remaining))
                    }
                } else {
                    self.pickup_time = Some(timestamp);
                    ProtectionResult::TripPending(Duration::from_millis(delay_ms))
                }
            }
            TripState::Trip => ProtectionResult::Trip,
        }
    }
}

impl ProtectionFunction for PtocSlidingWindow {
    fn process(&mut self, current: f64, timestamp: u64) -> ProtectionResult {
        // When called via the trait the `current` value is already an RMS.
        // Treat it as a single-sample input so callers using the trait
        // interface continue to work.
        self.process_sample(current, timestamp)
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.head = 0;
        self.count = 0;
        self.sum_sq = 0.0;
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
        "PTOC_SLIDING"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn make_config(iset: f64, tset: u64) -> PtocConfig {
        PtocConfig {
            iset,
            tset,
            enabled: true,
        }
    }

    /// Fill the window with `n` identical samples and return the last result.
    fn fill(ptoc: &mut PtocSlidingWindow, value: f64, n: usize, t0: u64) -> ProtectionResult {
        let mut result = ProtectionResult::NoTrip;
        for i in 0..n {
            result = ptoc.process_sample(value, t0 + i as u64 * 250);
        }
        result
    }

    #[test]
    fn test_no_trip_below_pickup() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        // Fill window with sub-threshold RMS (DC value 50 A)
        let result = fill(&mut ptoc, 50.0, 80, 0);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_trip_pending_on_overcurrent() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        let result = fill(&mut ptoc, 150.0, 80, 0);
        assert!(matches!(result, ProtectionResult::TripPending(_)));
        assert_eq!(ptoc.state(), TripState::Pickup);
    }

    #[test]
    fn test_trip_after_delay() {
        let config = make_config(100.0, 100);
        let mut ptoc = PtocSlidingWindow::new(config, 80);

        // Fill window so RMS = 150 A (> 100 A iset) — pickup starts here.
        // Window fills at sample index 79 → pickup_time ≈ 79 * 250 = 19_750 µs.
        fill(&mut ptoc, 150.0, 80, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);

        // Send a sample 100 ms after pickup_time (19_750 + 100_000 = 119_750 µs) — should trip.
        let result = ptoc.process_sample(150.0, 120_000);
        assert_eq!(result, ProtectionResult::Trip);
        assert_eq!(ptoc.state(), TripState::Trip);
    }

    #[test]
    fn test_reset_on_current_drop() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);

        // Enter pickup
        fill(&mut ptoc, 150.0, 80, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);

        // Slide in 80 sub-threshold samples — window RMS drops below iset
        fill(&mut ptoc, 50.0, 80, 20_000);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_disabled() {
        let config = PtocConfig {
            iset: 100.0,
            tset: 100,
            enabled: false,
        };
        let mut ptoc = PtocSlidingWindow::new(config, 80);
        let result = ptoc.process_sample(200.0, 0);
        assert_eq!(result, ProtectionResult::Disabled);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_reset_method() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        fill(&mut ptoc, 150.0, 80, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);

        ptoc.reset();
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_stays_tripped() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        fill(&mut ptoc, 150.0, 80, 0);
        // Trip after tset expires (window fills ≈ 19_750 µs, tset = 100 ms)
        ptoc.process_sample(150.0, 120_000);
        assert_eq!(ptoc.state(), TripState::Trip);

        let result = ptoc.process_sample(50.0, 200_000);
        assert_eq!(result, ProtectionResult::Trip);
        assert_eq!(ptoc.state(), TripState::Trip);
    }

    #[test]
    fn test_window_not_full_returns_no_trip() {
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        // Only 40 samples — window not yet full
        let result = fill(&mut ptoc, 200.0, 40, 0);
        assert_eq!(result, ProtectionResult::NoTrip);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_sine_wave_rms() {
        // One cycle of 150 A peak sine wave → RMS ≈ 106 A (> 100 A iset)
        let peak = 150.0_f64;
        let mut ptoc = PtocSlidingWindow::new(make_config(100.0, 100), 80);
        let mut result = ProtectionResult::NoTrip;
        for i in 0..80 {
            let sample = peak * (2.0 * PI * i as f64 / 80.0).sin();
            result = ptoc.process_sample(sample, i as u64 * 250);
        }
        // RMS of sine = peak/√2 ≈ 106 A → should be in Pickup or Trip
        assert!(
            matches!(result, ProtectionResult::TripPending(_)),
            "Expected TripPending, got {:?}",
            result
        );
    }

    #[test]
    fn test_dropout_hysteresis_no_reset_above_threshold() {
        use crate::config::PtocConfig;
        let config = PtocConfig { iset: 100.0, tset: 100, enabled: true, dropout_ratio: 0.95, ..PtocConfig::default() };
        let mut ptoc = PtocSlidingWindow::new(config, 80);
        fill(&mut ptoc, 150.0, 80, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);
        // 96 A > 95 A (dropout threshold) → must stay in Pickup
        let result = fill(&mut ptoc, 96.0, 80, 20_000);
        assert!(matches!(result, ProtectionResult::TripPending(_)));
        assert_eq!(ptoc.state(), TripState::Pickup);
    }

    #[test]
    fn test_dropout_hysteresis_resets_below_threshold() {
        use crate::config::PtocConfig;
        let config = PtocConfig { iset: 100.0, tset: 100, enabled: true, dropout_ratio: 0.95, ..PtocConfig::default() };
        let mut ptoc = PtocSlidingWindow::new(config, 80);
        fill(&mut ptoc, 150.0, 80, 0);
        assert_eq!(ptoc.state(), TripState::Pickup);
        // 50 A < 95 A → must reset to Idle
        fill(&mut ptoc, 50.0, 80, 20_000);
        assert_eq!(ptoc.state(), TripState::Idle);
    }

    #[test]
    fn test_incremental_rms_accuracy() {
        // Verify that incremental sum-of-squares matches batch calculation
        let window_size = 80;
        let mut ptoc = PtocSlidingWindow::new(make_config(1000.0, 100), window_size);
        let samples: Vec<f64> = (0..window_size)
            .map(|i| (2.0 * PI * i as f64 / window_size as f64).sin() * 100.0)
            .collect();

        for (i, &s) in samples.iter().enumerate() {
            ptoc.process_sample(s, i as u64 * 250);
        }

        let expected: f64 = {
            let sum_sq: f64 = samples.iter().map(|&x| x * x).sum();
            (sum_sq / window_size as f64).sqrt()
        };
        let actual = (ptoc.sum_sq / window_size as f64).sqrt();
        assert!(
            (actual - expected).abs() < 1e-6,
            "RMS mismatch: {} vs {}",
            actual,
            expected
        );
    }
}
