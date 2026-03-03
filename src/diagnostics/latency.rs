//! Latency measurement utilities for real-time performance monitoring.

use std::fmt;
use std::time::Instant;

/// Statistics derived from a `LatencyTracker` ring buffer.
#[derive(Debug, Clone, Copy)]
pub struct LatencyStats {
    /// Number of recorded measurements.
    pub count: usize,
    /// Average latency in microseconds.
    pub avg_us: u64,
    /// Minimum latency in microseconds.
    pub min_us: u64,
    /// Maximum latency in microseconds.
    pub max_us: u64,
    /// 99th-percentile latency in microseconds.
    pub p99_us: u64,
}

impl fmt::Display for LatencyStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "n={} avg={}µs min={}µs max={}µs p99={}µs",
            self.count, self.avg_us, self.min_us, self.max_us, self.p99_us
        )
    }
}

/// Ring-buffer latency tracker.
///
/// Records up to `capacity` measurements (oldest are overwritten when full).
/// All times are in microseconds.
pub struct LatencyTracker {
    buffer: Vec<u64>,
    capacity: usize,
    head: usize,
    count: usize,
}

impl LatencyTracker {
    /// Create a new tracker with the given ring-buffer capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self {
            buffer: vec![0u64; capacity],
            capacity,
            head: 0,
            count: 0,
        }
    }

    /// Start a latency measurement; returns an `Instant` to pass to `stop`.
    pub fn start() -> Instant {
        Instant::now()
    }

    /// Stop a measurement, record it, and return the elapsed microseconds.
    pub fn stop(&mut self, start: Instant) -> u64 {
        let us = start.elapsed().as_micros() as u64;
        self.record(us);
        us
    }

    /// Record a raw latency value (microseconds).
    pub fn record(&mut self, latency_us: u64) {
        self.buffer[self.head] = latency_us;
        self.head = (self.head + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    /// Compute statistics over all recorded measurements.
    ///
    /// Returns `None` if no measurements have been recorded yet.
    pub fn stats(&self) -> Option<LatencyStats> {
        if self.count == 0 {
            return None;
        }

        let samples = &self.buffer[..self.count];
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();

        let min_us = sorted[0];
        let max_us = *sorted.last().unwrap();
        let sum: u64 = sorted.iter().sum();
        let avg_us = sum / self.count as u64;
        let p99_index = ((self.count as f64 * 0.99).ceil() as usize).saturating_sub(1);
        let p99_us = sorted[p99_index];

        Some(LatencyStats {
            count: self.count,
            avg_us,
            min_us,
            max_us,
            p99_us,
        })
    }

    /// Number of measurements recorded so far (up to capacity).
    pub fn count(&self) -> usize {
        self.count
    }

    /// Reset all measurements.
    pub fn reset(&mut self) {
        self.buffer.fill(0);
        self.head = 0;
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_empty() {
        let tracker = LatencyTracker::new(100);
        assert!(tracker.stats().is_none());
    }

    #[test]
    fn test_record_and_stats() {
        let mut tracker = LatencyTracker::new(100);
        for us in [10, 20, 30, 40, 50] {
            tracker.record(us);
        }
        let stats = tracker.stats().unwrap();
        assert_eq!(stats.count, 5);
        assert_eq!(stats.min_us, 10);
        assert_eq!(stats.max_us, 50);
        assert_eq!(stats.avg_us, 30);
    }

    #[test]
    fn test_ring_buffer_wraps() {
        let mut tracker = LatencyTracker::new(3);
        tracker.record(1);
        tracker.record(2);
        tracker.record(3);
        // count should be capped at capacity
        assert_eq!(tracker.count(), 3);
        // Overwrite oldest
        tracker.record(4);
        assert_eq!(tracker.count(), 3);
    }

    #[test]
    fn test_p99() {
        let mut tracker = LatencyTracker::new(200);
        for i in 1u64..=100 {
            tracker.record(i);
        }
        let stats = tracker.stats().unwrap();
        // 99th percentile of 1..=100 should be 99
        assert_eq!(stats.p99_us, 99);
    }

    #[test]
    fn test_display() {
        let mut tracker = LatencyTracker::new(10);
        tracker.record(100);
        let stats = tracker.stats().unwrap();
        let s = format!("{}", stats);
        assert!(s.contains("n=1"));
        assert!(s.contains("avg=100µs"));
    }

    #[test]
    fn test_stop_records_measurement() {
        let mut tracker = LatencyTracker::new(10);
        let start = LatencyTracker::start();
        let us = tracker.stop(start);
        assert!(us < 1_000_000, "elapsed should be much less than 1 s");
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn test_reset() {
        let mut tracker = LatencyTracker::new(10);
        tracker.record(50);
        tracker.reset();
        assert_eq!(tracker.count(), 0);
        assert!(tracker.stats().is_none());
    }
}
