# Module: PTOC — Time Overcurrent Protection

**Source files**:
- [`src/protection/ptoc.rs`](../../src/protection/ptoc.rs) — definite-time PTOC
- [`src/protection/ptoc_sliding.rs`](../../src/protection/ptoc_sliding.rs) — PTOC with sliding-window RMS

---

## Purpose

PTOC (Time OverCurrent Protection) trips the circuit breaker when the primary current exceeds the pickup setting `iset` for a continuous duration equal to the effective trip delay. Two delay modes are supported:

- **Definite-time** — fixed delay equal to `tset` regardless of overcurrent magnitude
- **Inverse-time** — delay decreases as current increases, following IEC 60255-151 curves

PTOC corresponds to IEC 61850 logical node class **PTOC** (IEC 61850-7-4).

---

## State Machine

```
         current > iset                    elapsed ≥ delay(I)
  Idle ──────────────────▶  Pickup ──────────────────────────▶  Trip
    ▲                          │
    │  current < iset × dropout_ratio      (hysteresis band)
    └──────────────────────────┘
```

| State | `ProtectionResult` | Meaning |
|-------|--------------------|---------|
| `Idle` | `NoTrip` | Current below pickup; no action |
| `Pickup` | `TripPending(remaining)` | Current above pickup; timer running |
| `Trip` | `Trip` | Time elapsed; breaker should open |

### Dropout Hysteresis

The reset condition uses `dropout_ratio` (default 0.95) to add hysteresis: current must fall below `iset × dropout_ratio` to transition Pickup → Idle. This prevents chattering near the pickup threshold.

### Reset

`Ptoc::reset()` returns the state machine to `Idle` and clears the pickup timestamp. This is called when the function is disabled (`enabled = false`). External reset from an XCBR GOOSE message is a future item.

---

## `ptoc.rs` — Definite-Time and Inverse-Time Implementation

```rust
use poc_protection_functions::{Ptoc, PtocConfig, PtocCurve};

// Definite-time
let mut ptoc = Ptoc::new(PtocConfig {
    iset: 100.0, tset: 100, enabled: true,
    dropout_ratio: 0.95, curve: PtocCurve::DefiniteTime,
});

// IEC Very Inverse — tset acts as TMS
let mut ptoc_inv = Ptoc::new(PtocConfig {
    iset: 100.0, tset: 100, enabled: true,
    dropout_ratio: 0.95, curve: PtocCurve::IecVeryInverse,
});

let result = ptoc.process(primary_rms, timestamp_us);
match result {
    ProtectionResult::Trip => { /* send GOOSE */ },
    ProtectionResult::TripPending(d) => { /* d = remaining ms */ },
    ProtectionResult::NoTrip => {},
    ProtectionResult::Disabled => {},
}
```

### Inverse-Time Formula (IEC 60255-151)

`t_ms = tset × k / ((I / Iset)^α − 1)`

| Curve | k | α |
|-------|---|---|
| IEC Standard Inverse | 0.14 | 0.02 |
| IEC Very Inverse | 13.5 | 1.0 |
| IEC Extremely Inverse | 80.0 | 2.0 |

Computed by `effective_trip_delay_ms(curve, ratio, tset)` — also used by `ptoc_sliding.rs`.

### Timing Mechanism

Pickup timestamp stored as `Option<u64>` in microseconds:

```
elapsed_ms = (current_time_us − pickup_time_us) / 1000
if elapsed_ms ≥ effective_trip_delay_ms(curve, I/Iset, tset) → Trip
```

Relies on `CLOCK_REALTIME` synchronised by PTP (`phc2sys`). No additional hardware timer.

---

## `ptoc_sliding.rs` — Sliding-Window RMS Variant

`PtocSlidingWindow` wraps the same PTOC state machine but maintains its own incremental RMS calculator. The RMS is updated on every call to `process_sample()` using an O(1) incremental update rather than recomputing from the full window each cycle.

```rust
let mut ptoc = PtocSlidingWindow::new(PtocConfig { iset: 100.0, tset: 100, enabled: true, dropout_ratio: 0.95, curve: PtocCurve::DefiniteTime }, 80);

// Called once per SV sample (every 250 µs):
let result = ptoc.process_sample(primary_amps, timestamp_us);
```

**Preferred for production** because detection latency approaches one sample period (250 µs) rather than one cycle (20 ms). See [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) for the full explanation.

---

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | `f64` (primary Amperes RMS) | From `RmsCalculator` or `PtocSlidingWindow` internal RMS |
| Input | `u64` (microseconds) | Timestamp from `clock_gettime(CLOCK_REALTIME)` |
| Output | `ProtectionResult` | `NoTrip`, `TripPending`, `Trip`, or `Disabled` |

---

## Real-Time Constraints

| Constraint | Value |
|------------|-------|
| Max time per `process()` call | < 5 µs |
| Allocations in hot path | None |
| Timer resolution | Microseconds (via `u64` timestamp) |

---

## Configuration

```json
"ptoc": {
  "iset": 300.0,
  "tset": 100,
  "enabled": true,
  "dropout_ratio": 0.95,
  "curve": "DefiniteTime"
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `iset` | `f64` | — | Pickup current in primary Amperes |
| `tset` | `u64` | — | Time delay in ms; acts as TMS for inverse-time curves |
| `enabled` | `bool` | — | Enable or disable |
| `dropout_ratio` | `f64` | 0.95 | Current must fall below `iset × ratio` to reset to Idle |
| `curve` | string | `"DefiniteTime"` | `"DefiniteTime"` \| `"IecStandardInverse"` \| `"IecVeryInverse"` \| `"IecExtremelyInverse"` |

In Phase 2, `iset` and `tset` will be read from the SCD file's `PTOC` logical node (`StrVal`, `OpDlTmms` data attributes).

---

## Unit Tests

Located in `src/protection/ptoc.rs` and `src/protection/ptoc_sliding.rs`:

| Test | Verifies |
|------|---------|
| `test_no_trip_below_pickup` | Current below `iset` → `NoTrip` |
| `test_trip_after_tset` | Current above `iset` for ≥ `tset` ms → `Trip` |
| `test_no_trip_dropout_before_tset` | Current drops before timer expires → `NoTrip` |
| `test_disabled` | `enabled = false` → `Disabled` |
| `test_ptoc_dropout_hysteresis_no_reset_above_threshold` | 96 A (> 95 A dropout) stays in Pickup |
| `test_ptoc_dropout_hysteresis_resets_below_threshold` | 94 A (< 95 A dropout) resets to Idle |
| `test_ptoc_inverse_time_trips_faster_at_higher_current` | IEC Very Inverse: 5× trips before 2× |

---

## Implemented

- [x] Definite-time characteristic
- [x] IEC 60255-151 inverse-time curves (Standard, Very, Extremely Inverse)
- [x] Dropout hysteresis (`dropout_ratio`)
- [x] Sliding-window per-sample RMS (`ptoc_sliding.rs`)

## TODO

- [ ] **External reset** — reset state machine via XCBR GOOSE confirmation
- [x] **Three-phase** — `ThreePhasePtoc` in `src/protection/three_phase.rs`
- [ ] **SCD-driven settings** — read `iset` / `tset` from SCL PTOC data attributes

---

## See Also

- [`docs/modules/MEASUREMENT_RMS.md`](MEASUREMENT_RMS.md) — produces the primary RMS input
- [`docs/modules/IO_GOOSE_OUTPUT.md`](IO_GOOSE_OUTPUT.md) — consumes the `Trip` result
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — P3 compliance and the role of `tset`
- [`docs/modules/PROTECTION_PIOC.md`](PROTECTION_PIOC.md) — instantaneous overcurrent (no time delay)
