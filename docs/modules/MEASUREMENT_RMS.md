# Module: RMS Measurement

**Source files**:
- [`src/measurement/rms.rs`](../../src/measurement/rms.rs) — cycle-window RMS functions and `RmsCalculator`
- [`src/protection/ptoc_sliding.rs`](../../src/protection/ptoc_sliding.rs) — sliding-window PTOC using incremental RMS

---

## Purpose

This module computes the **Root Mean Square (RMS)** value of the primary current from a stream of instantaneous samples. The RMS value is what protection functions (PTOC, future PDIF) compare against their pickup settings.

---

## Current Implementation: Cycle-Window RMS

### `calculate_rms(samples: &[f64]) -> f64`

One-shot RMS over a slice of samples:

```
RMS = sqrt( Σ(xᵢ²) / n )
```

Used when a full window of samples is already available (e.g., batch processing).

### `RmsCalculator`

A ring-buffer accumulator that holds the last `window_size` samples (default: 80 for one 50 Hz cycle):

```rust
let mut calc = RmsCalculator::new(80);
calc.add_sample(primary_current);
if calc.is_full() {
    let rms = calc.calculate(); // available after 80 samples
}
```

The RMS value is recomputed from the full window on every call to `calculate()`. Between window updates, the RMS value is **stale** — it reflects the previous cycle, not the current one. This introduces up to one full cycle (20 ms) of detection latency.

---

## Preferred Alternative: Sliding-Window / Incremental RMS

`src/protection/ptoc_sliding.rs` implements PTOC with an **incremental RMS** update:

```
sum_sq = sum_sq - x_oldest² + x_newest²
RMS    = sqrt( sum_sq / N )
```

This updates the RMS estimate on **every new sample** rather than once per cycle. For a fault starting mid-cycle, the sliding-window detects it within 1–2 sample periods (250–500 µs) rather than up to 20 ms.

### When to use which

| Scenario | Recommended RMS | Reason |
|----------|----------------|--------|
| Production PTOC | `PtocSliding` (sliding window) | Lowest detection latency, P3 margin |
| Simple testing / simulation | `RmsCalculator` (cycle window) | Easy to reason about |
| PIOC (instantaneous OC) | No RMS — use instantaneous sample | RMS averaging defeats the purpose |

---

## Real-Time Constraints

| Constraint | Value |
|------------|-------|
| Max time per sample update | < 50 µs |
| Memory allocation | None (pre-allocated ring buffer) |
| Branches per sample | O(1) (sliding) or O(N) (cycle, only on `calculate()`) |

Both implementations avoid heap allocation in the hot path. The ring buffer is allocated once at construction.

---

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | `f64` (primary Amperes) | Output of `CurrentScaler::scale_to_primary()` |
| Output | `f64` (primary Amperes RMS) | Fed into `Ptoc::process()` |

---

## Unit Tests

Located in `src/measurement/rms.rs`:

| Test | Description |
|------|-------------|
| `test_calculate_rms_dc` | DC signal RMS = signal value |
| `test_calculate_rms_sine_wave` | RMS of unit sine = 1/√2 ≈ 0.707 |
| `test_calculate_rms_empty` | Empty slice returns 0.0 |
| `test_rms_calculator` | DC via `RmsCalculator` |
| `test_rms_calculator_sine` | Sine via `RmsCalculator` |

---

## TODO

- [ ] **Sliding RMS exposed as a standalone calculator** — currently the sliding-window logic is embedded in `PtocSliding`; consider extracting a reusable `SlidingRmsCalculator` struct for use by future PDIF/PDIS functions.
- [ ] **Three-phase support** — current implementation is single-phase. Three-phase protection needs three independent `RmsCalculator` instances (A, B, C phases).
- [ ] **Integer RMS path** — `calculate_rms_i32` exists but is not used in the RT loop; evaluate whether operating on raw ADC integers (avoiding the ADC→secondary→primary conversion in the hot path) improves latency.

---

## See Also

- [`docs/modules/MEASUREMENT_SCALING.md`](MEASUREMENT_SCALING.md) — produces the `f64` primary Amperes input to this module
- [`docs/modules/PROTECTION_PTOC.md`](PROTECTION_PTOC.md) — consumes the RMS output
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — explains why sliding-window RMS is preferred for P3 compliance
