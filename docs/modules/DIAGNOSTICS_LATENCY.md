# Module: Diagnostics — Latency Tracker

**Source files**:
- [`src/diagnostics/latency.rs`](../../src/diagnostics/latency.rs) — `LatencyTracker` and `LatencyStats`
- [`src/diagnostics/mod.rs`](../../src/diagnostics/mod.rs) — module root

---

## Purpose

The latency tracker measures and records the elapsed time between two points in the processing pipeline — typically from **SV frame received** to **GOOSE frame sent**. It stores measurements in a fixed-size ring buffer and computes statistics including the p99 (99th percentile) latency.

This is the primary tool for validating that the system meets IEC 61850-5 performance class requirements (P1 ≤ 3 ms, P3 ≤ 20 ms) at runtime.

---

## API

```rust
use poc_protection_functions::diagnostics::LatencyTracker;

let mut tracker = LatencyTracker::new(1000); // ring buffer of 1000 measurements

// Measurement:
let start = LatencyTracker::start();         // returns Instant::now()
// ... processing ...
let elapsed_us = tracker.stop(start);        // records elapsed µs, returns value

// Or record a pre-computed value:
tracker.record(latency_us);

// Print statistics:
if let Some(stats) = tracker.stats() {
    println!("{}", stats);
    // Output: n=1000 avg=850µs min=120µs max=2400µs p99=1980µs
}
```

---

## `LatencyStats` Fields

| Field | Type | Description |
|-------|------|-------------|
| `count` | `usize` | Number of measurements recorded |
| `avg_us` | `u64` | Mean latency in microseconds |
| `min_us` | `u64` | Minimum latency in microseconds |
| `max_us` | `u64` | Maximum latency in microseconds |
| `p99_us` | `u64` | 99th-percentile latency in microseconds |

---

## Ring Buffer Behaviour

- The tracker holds up to `capacity` measurements.
- When full, the **oldest** measurement is overwritten (circular buffer).
- Statistics are always computed over the most recent `capacity` samples.
- Ideal capacity: 4 000 (one second at 4 000 Sa/s) for a rolling 1-second p99.

---

## Real-Time Constraints

| Property | Value |
|----------|-------|
| `start()` cost | ~5 ns (`Instant::now()`) |
| `stop()` / `record()` cost | ~10 ns (one write + index wrap) |
| `stats()` cost | O(n log n) sort — **do not call in the RT hot path** |

`stats()` should be called from a low-priority background thread or printed periodically (e.g., every 10 seconds) outside the sample processing loop.

---

## Interpreting Results

| p99 result | Interpretation |
|-----------|---------------|
| ≤ 3 ms | P1 compliant (PIOC) |
| ≤ 10 ms | P2 compliant (distance, differential) |
| ≤ 20 ms | P3 compliant (PTOC) |
| > 20 ms | **Fail** — investigate CPU scheduling, core isolation, PTP sync |

Common causes of high latency spikes:
- **Non-isolated CPU core** — OS scheduler preempts the protection thread
- **Missing PREEMPT_RT kernel** — high interrupt latency
- **NIC software polling instead of interrupts** — add `irq_coalesce` tuning
- **PTP not synchronised** — timestamp arithmetic is wrong (not a latency issue, but corrupts measurements)
- **Memory not locked** (`mlockall` missing) — page faults cause spikes

---

## Unit Tests

Located in `src/diagnostics/latency.rs`:

| Test | Verifies |
|------|---------|
| `test_stats_empty` | No measurements → `stats()` returns `None` |
| `test_record_and_stats` | Five values: correct min/max/avg |
| `test_ring_buffer_wraps` | Capacity-3 buffer wraps correctly |
| `test_p99` | p99 of 1–100 = 99 |
| `test_display` | `Display` trait format string |
| `test_stop_records_measurement` | `stop()` records a value < 1 s |
| `test_reset` | `reset()` clears all state |

---

## Integration with the RT Loop

The typical integration in `main.rs`:

```rust
let mut latency_tracker = LatencyTracker::new(4000); // 1-second window

loop {
    let t_start = LatencyTracker::start();

    let sample = sv_subscriber.receive_sample()?;
    let primary = scaler.scale_to_primary(sample.current_adc);
    let rms = rms_calc.add_and_calculate(primary);
    let result = ptoc.process(rms, sample.timestamp);

    if result == ProtectionResult::Trip {
        goose_publisher.publish_trip(true, sample.timestamp)?;
        let _ = latency_tracker.stop(t_start); // record SV→GOOSE time
    }

    // Every ~10 000 samples, print stats (outside hot path)
    if sample_count % 10_000 == 0 {
        if let Some(stats) = latency_tracker.stats() {
            log::info!("Latency: {}", stats);
        }
    }
}
```

---

## TODO

- [ ] **Separate trip-path and non-trip-path measurements** — currently only trip events are measured; add a separate tracker for "no-trip" processing time to detect scheduling jitter independent of trips.
- [ ] **Histogram output** — export latency histograms in a format compatible with Prometheus / Grafana for continuous monitoring in a multi-bay deployment.
- [ ] **Per-phase trackers** — three-phase support will need per-phase latency tracking.

---

## See Also

- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — P1/P3 latency budgets
- [`docs/TESTING.md`](../TESTING.md) — how p99 targets are verified with Omicron
- [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) — CPU core allocation strategy
