# IEC 61850 Performance Classes

## Background

IEC 61850-5 defines **transfer-time performance classes** for protection messages. These classes specify how quickly a change in a published dataset (e.g., a trip signal going `true`) must propagate from the publishing IED to all subscribing IEDs on the process bus.

> **Important**: The transfer-time classes measure only the **network transmission time** — from the moment the IED issues the GOOSE frame to the moment it arrives at the subscriber. They do not include the protection function's intentional time delay (`tset`).

---

## Type 1A Trip Performance Classes

| Class | Max transfer time | Typical application |
|-------|-------------------|---------------------|
| **P1** | ≤ 3 ms | Instantaneous overcurrent (PIOC), busbar protection |
| **P2** | ≤ 10 ms | Distance protection Zone 1, differential protection |
| **P3** | ≤ 20 ms | Definite-time overcurrent (PTOC), slower trips |

The "total transmission time" (TTT) is measured from the moment the protection IED decides to trip until the GOOSE message is received and decoded by the subscriber.

It covers:
- GOOSE encoding time
- Network queuing delay
- Physical layer transmission
- Subscriber frame reception and decoding

It does **not** cover:
- Protection algorithm execution time (RMS, threshold comparison)
- The intentional time delay `tset`

---

## PTOC `tset` Is Not Part of the Transfer-Time Class

A common point of confusion: PTOC with `tset = 100 ms` does not violate P3.

The sequence is:

```
SV sample arrives
      │
      ▼
RMS calculated (< 1 ms)
      │
      ▼
Current > Iset ?  ──NO──▶ wait for next sample
      │ YES
      ▼
Start timing (pickup)
      │
      │  ← tset = 100 ms intentional delay (NOT part of TTT)
      │
      ▼
tset elapsed → decision to trip
      │
      ▼
GOOSE frame encoded and sent  ──────────────────┐
      │                                         │ TTT starts here
      ▼                                         │
Subscriber receives and decodes GOOSE           │
      │                                    TTT ends here
      ▼
Subscriber action (open breaker, etc.)
```

For P3 compliance, only the final step — GOOSE frame sent to subscriber received — must complete within 20 ms. In this implementation that step typically takes **< 1 ms** on a local process-bus Ethernet.

---

## RMS Evaluation and Its Effect on Trip Time

### Cycle-Based RMS (current implementation: `src/measurement/rms.rs`)

The `RmsCalculator` accumulates 80 samples (one full 50 Hz cycle) and then computes:

```
RMS = sqrt( (x₀² + x₁² + … + x₇₉²) / 80 )
```

This evaluation runs once per cycle (every 20 ms) after the window is full.

**Implication for P3**: The worst-case fault detection delay is up to one full cycle (20 ms) plus the time for the RMS buffer to align. This makes P3 marginal — it is achievable but leaves little headroom.

**Implication for P1/P2**: A cycle-based RMS updated every 20 ms cannot meet P1 (3 ms) or P2 (10 ms) detection latency on its own. PIOC (`src/protection/pioc.rs`) addresses this by operating on the **instantaneous sample** rather than the RMS value.

### Sliding-Window RMS (preferred for RT: `src/protection/ptoc_sliding.rs`)

The sliding-window variant updates the RMS estimate **on every new sample** using an incremental formula:

```
sum_sq_new = sum_sq_old - x_oldest² + x_newest²
RMS = sqrt( sum_sq_new / N )
```

Advantages:
- Detection latency approaches **1 sample period (250 µs)** for a large step fault
- Smooth RMS trace — no 20 ms update step
- Enables PTOC to approach P2 performance without the full RMS window delay

The trade-off is slightly higher per-sample CPU cost (one extra subtraction and addition vs. no work between cycle updates). This is negligible compared to the sample period budget.

### Recommendation

| Function | RMS method | Rationale |
|----------|-----------|-----------|
| PIOC | Instantaneous sample threshold | No averaging needed; sample-by-sample is sufficient for P1 |
| PTOC (default) | Sliding window (`PtocSliding`) | Best P3 margin; preferred for production |
| PTOC (fallback) | Cycle-window `RmsCalculator` | Simpler; acceptable if P3 headroom is verified by test |

---

## Latency Budget (end-to-end)

For a PTOC P3 trip (excluding `tset`):

| Step | Target budget |
|------|--------------|
| SV reception + decoding | < 0.5 ms |
| Scaling (ADC → primary) | < 0.1 ms |
| Sliding RMS update | < 0.1 ms |
| PTOC state machine | < 0.1 ms |
| GOOSE encoding + send | < 1.0 ms |
| **Total (TTT)** | **< 2 ms** (P3 target ≤ 20 ms) |

For a PIOC P1 trip:

| Step | Target budget |
|------|--------------|
| SV reception + decoding | < 0.5 ms |
| Scaling | < 0.1 ms |
| PIOC threshold check | < 0.05 ms |
| GOOSE encoding + send | < 1.0 ms |
| **Total (TTT)** | **< 2 ms** (P1 target ≤ 3 ms) |

These budgets are measured by `src/diagnostics/latency.rs` at runtime. See [`docs/modules/DIAGNOSTICS_LATENCY.md`](modules/DIAGNOSTICS_LATENCY.md) for how to read the p99 output.

---

## References

- IEC 61850-5: Communication networks and systems for power utility automation — Performance requirements (paraphrased; do not reproduce verbatim)
- IEC 61850-8-1: GOOSE encoding and transmission
- [`src/protection/pioc.rs`](../src/protection/pioc.rs) — P1 instantaneous protection
- [`src/protection/ptoc.rs`](../src/protection/ptoc.rs) — P3 definite-time protection
- [`src/protection/ptoc_sliding.rs`](../src/protection/ptoc_sliding.rs) — sliding-window RMS variant
- [`src/diagnostics/latency.rs`](../src/diagnostics/latency.rs) — latency measurement
