# Module: PIOC — Instantaneous Overcurrent Protection

**Source file**: [`src/protection/pioc.rs`](../../src/protection/pioc.rs)

---

## Purpose

PIOC (Instantaneous OverCurrent Protection) trips **immediately** when the measured current exceeds the pickup setting. There is no intentional time delay. This is the fastest protection function in the system and targets **IEC 61850-5 performance class P1** (transfer time ≤ 3 ms).

PIOC corresponds to IEC 61850 logical node class **PIOC** (IEC 61850-7-4).

### Relationship to PTOC

| Function | Time delay | Use case |
|----------|-----------|---------|
| PIOC | None (instantaneous) | High-current faults (e.g., short circuit close to the busbar) |
| PTOC | `tset` (configurable) | Sustained overcurrents with coordination delay |

In a typical bay protection scheme, both PIOC and PTOC run in parallel:
- PIOC trips on heavy faults (e.g., I > 5× rated current) within 1–2 sample periods
- PTOC trips on moderate overcurrents (e.g., I > 1.2× rated) after the coordination delay

---

## State Machine

```
         current > iset
  Idle ─────────────────▶  Trip
```

There is no `Pickup` intermediate state — the transition from `Idle` to `Trip` is direct and happens on the same function call that detects the overcurrent.

| State | `ProtectionResult` | Meaning |
|-------|--------------------|---------|
| `Idle` | `NoTrip` | Current at or below pickup |
| `Trip` | `Trip` | Current exceeded pickup; trip latched |

Once in `Trip`, the function stays tripped until explicitly reset via `reset()`. The current value is not re-evaluated while tripped.

---

## Input Modes

Two input modes are selected via `PiocInputMode` in `PiocConfig`:

### `Instantaneous` (default)
The caller passes the raw sample value. PIOC compares `|current|` against `iset`.
`iset` must be set as a **peak** threshold (≥ `iset_rms × √2`).

### `ShortWindowRms(n)`
PIOC maintains an internal n-sample sliding-window RMS ring buffer.
`iset` is then an **RMS** threshold, reducing noise sensitivity at the cost of n sample periods of additional detection latency (n × 250 µs at 4 kSa/s).

```json
"pioc": { "iset": 700.0, "enabled": true, "input_mode": { "ShortWindowRms": 8 } }
```

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | `f64` (primary Amperes) | Instantaneous sample or ring-buffer output depending on mode |
| Input | `u64` (timestamp, microseconds) | Unused; present for `ProtectionFunction` trait compatibility |
| Output | `ProtectionResult` | `NoTrip`, `Trip`, or `Disabled` |

---

## Real-Time Constraints

| Constraint | Value |
|------------|-------|
| Max time per `process()` call | < 1 µs (threshold comparison only) |
| Allocations in hot path | None |
| Required CPU core type | **Dedicated isolated core** — PIOC must not share a core; see [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) |

For PIOC to meet P1 (≤ 3 ms end-to-end), the processing path from SV receive to GOOSE transmit must complete within 3 ms. The PIOC logic itself takes < 1 µs; the budget is spent primarily on socket I/O.

---

## Configuration

```json
"pioc": {
  "iset": 1200.0,
  "enabled": true,
  "input_mode": "Instantaneous"
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `iset` | `f64` | — | Peak A for `Instantaneous`; RMS A for `ShortWindowRms` |
| `enabled` | `bool` | — | Enable or disable |
| `input_mode` | string/object | `"Instantaneous"` | `"Instantaneous"` or `{"ShortWindowRms": n}` |

Typical settings:
- `iset` = 5–10× rated primary current (busbar faults produce the highest peak)
- For `Instantaneous` mode: `iset` should exceed the maximum load current peak (`load_rms × √2`)

In Phase 2, `iset` will be read from the SCD file's `PIOC` logical node `StrVal` data attribute.

---

## Unit Tests

Located in `src/protection/pioc.rs`:

| Test | Verifies |
|------|---------|
| `test_no_trip_below_pickup` | Current ≤ `iset` → `NoTrip` |
| `test_immediate_trip_above_pickup` | Current > `iset` → `Trip` on first call |
| `test_no_time_delay` | Trip at t=0 (no delay) |
| `test_stays_tripped` | Trip state persists even if current drops |
| `test_disabled` | `enabled = false` → `Disabled` |
| `test_reset` | `reset()` returns state to `Idle` |
| `test_exact_pickup_no_trip` | Strictly greater-than: `current == iset` → `NoTrip` |
| `test_set_enabled` | Disabling an active function resets state |
| `test_short_window_rms_no_trip_below_pickup` | 50 A DC < 100 A RMS threshold → `NoTrip` |
| `test_short_window_rms_trips_above_pickup` | 150 A DC > 100 A RMS threshold → `Trip` |
| `test_short_window_rms_reset_clears_buffer` | `reset()` clears ring buffer; subsequent low current → `NoTrip` |

---

## Implemented

- [x] Instantaneous mode (`|sample| > iset` as peak threshold)
- [x] Short-window RMS mode (internal n-sample ring buffer, RMS threshold)
- [x] `reset()` clears ring buffer state

## TODO

- [x] **Three-phase** — `ThreePhasePioc` in `src/protection/three_phase.rs`
- [ ] **Dedicated core enforcement** — runtime assertion / documentation that PIOC is on an isolated CPU core
- [ ] **SCD-driven settings** — read `iset` from SCL `PIOC` logical node

---

## See Also

- [`docs/modules/PROTECTION_PTOC.md`](PROTECTION_PTOC.md) — time-delayed overcurrent companion function
- [`docs/modules/IO_GOOSE_OUTPUT.md`](IO_GOOSE_OUTPUT.md) — consumes the `Trip` result
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — P1 performance class requirements
- [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md) — CPU core allocation rules for PIOC
