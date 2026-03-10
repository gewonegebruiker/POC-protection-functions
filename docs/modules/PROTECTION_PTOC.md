# Module: PTOC — Time Overcurrent Protection

**Source files**:
- [`src/protection/ptoc.rs`](../../src/protection/ptoc.rs) — definite-time PTOC
- [`src/protection/ptoc_sliding.rs`](../../src/protection/ptoc_sliding.rs) — PTOC with sliding-window RMS

---

## Purpose

PTOC (Time OverCurrent Protection) trips the circuit breaker when the primary current exceeds the pickup setting `iset` for a continuous duration equal to `tset` (the intentional time delay). It implements **definite-time overcurrent** — the time delay is fixed regardless of the magnitude of overcurrent.

PTOC corresponds to IEC 61850 logical node class **PTOC** (IEC 61850-7-4).

---

## State Machine

```
         current > iset                    elapsed ≥ tset
  Idle ──────────────────▶  Pickup ──────────────────────▶  Trip
    ▲                          │
    │    current ≤ iset        │
    └──────────────────────────┘  (dropout — reset to Idle)
```

| State | `ProtectionResult` | Meaning |
|-------|--------------------|---------|
| `Idle` | `NoTrip` | Current below pickup; no action |
| `Pickup` | `TripPending(tset)` | Current above pickup; timer running |
| `Trip` | `Trip` | Time elapsed; breaker should open |

### Reset

`Ptoc::reset()` returns the state machine to `Idle` and clears the pickup timestamp. This is called:
- When the circuit breaker opens (external reset signal — TODO)
- When the function is disabled (`enabled = false`)

---

## `ptoc.rs` — Definite-Time Implementation

```rust
let mut ptoc = Ptoc::new(PtocConfig {
    iset: 100.0,   // 100 A pickup
    tset: 100,     // 100 ms time delay
    enabled: true,
});

// Called once per RMS update
let result = ptoc.process(primary_rms, timestamp_us);

match result {
    ProtectionResult::Trip => { /* send GOOSE */ },
    ProtectionResult::TripPending(d) => { /* timer running, d = remaining */ },
    ProtectionResult::NoTrip => { /* normal */ },
    ProtectionResult::Disabled => { /* function off */ },
}
```

### Timing Mechanism

The pickup timestamp is stored as `Option<u64>` in microseconds. The elapsed time is computed as:

```
elapsed_ms = (current_time_us − pickup_time_us) / 1000
if elapsed_ms ≥ tset → Trip
```

This relies on `CLOCK_REALTIME` being synchronised by PTP (`phc2sys`). No additional hardware timer is used.

---

## `ptoc_sliding.rs` — Sliding-Window RMS Variant

`PtocSliding` wraps the same PTOC state machine but maintains its own incremental RMS calculator. The RMS is updated on every call to `process_sample()` using an O(1) incremental update rather than recomputing from the full window each cycle.

```rust
let mut ptoc = PtocSliding::new(PtocConfig { iset: 100.0, tset: 100, enabled: true }, 80);

// Called once per SV sample (every 250 µs):
let result = ptoc.process_sample(primary_amps, timestamp_us);
```

**Preferred for production** because detection latency approaches one sample period (250 µs) rather than one cycle (20 ms). See [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) for the full explanation.

---

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | `f64` (primary Amperes RMS) | From `RmsCalculator` or `PtocSliding` internal RMS |
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
  "iset": 100.0,
  "tset": 100,
  "enabled": true
}
```

| Field | Type | Description |
|-------|------|-------------|
| `iset` | `f64` | Pickup current in primary Amperes |
| `tset` | `u64` | Time delay in milliseconds |
| `enabled` | `bool` | Enable or disable the function |

In Phase 2, `iset` and `tset` will be read from the SCD file's `PTOC` logical node (`StrVal`, `OpDlTmms` data attributes).

---

## Unit Tests

Located in `src/protection/ptoc.rs`:

| Test | Verifies |
|------|---------|
| `test_no_trip_below_pickup` | Current below `iset` → `NoTrip` |
| `test_trip_after_tset` | Current above `iset` for ≥ `tset` ms → `Trip` |
| `test_no_trip_dropout_before_tset` | Current drops before timer expires → `NoTrip` |
| `test_disabled` | `enabled = false` → `Disabled` regardless of current |

---

## TODO

- [ ] **Inverse-time curves** — IEC 255 (IEC Standard Inverse, Very Inverse, Extremely Inverse) and IEEE C37.112 curves. Would allow PTOC to trip faster at higher multiples of `iset`.
- [ ] **External reset input** — reset the state machine when a GOOSE message from XCBR (circuit breaker) confirms the breaker has opened.
- [ ] **Dropout ratio** — configurable hysteresis: current must drop below `iset × dropout_ratio` (e.g., 0.95) before the function resets to `Idle`.
- [ ] **Three-phase** — run three independent PTOC instances (A, B, C) and trip on any phase exceeding pickup.
- [ ] **SCD-driven settings** — read `iset` / `tset` from SCL PTOC data attributes.

---

## See Also

- [`docs/modules/MEASUREMENT_RMS.md`](MEASUREMENT_RMS.md) — produces the primary RMS input
- [`docs/modules/IO_GOOSE_OUTPUT.md`](IO_GOOSE_OUTPUT.md) — consumes the `Trip` result
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — P3 compliance and the role of `tset`
- [`docs/modules/PROTECTION_PIOC.md`](PROTECTION_PIOC.md) — instantaneous overcurrent (no time delay)
