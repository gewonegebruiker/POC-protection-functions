# Test Strategy

## Overview

Testing is organised into three layers:

| Layer | Location | Status |
|-------|----------|--------|
| Unit tests | `src/**/*.rs` (inline `#[test]`) | ✅ Implemented |
| Integration tests (SV PCAP replay) | `tests/` | TODO |
| HIL tests with Omicron CMC | Physical lab | TODO |

---

## Unit Tests

Unit tests live next to the code they test and are run with `cargo test`.

### Current Coverage

| Module | Test file | What is tested |
|--------|-----------|---------------|
| `measurement::rms` | `src/measurement/rms.rs` | DC RMS, sine-wave RMS, empty input, `RmsCalculator` window, sine via calculator |
| `measurement::scaling` | `src/measurement/scaling.rs` | ADC→secondary, offset, secondary→primary, ADC→primary, `CurrentScaler`, batch scaling |
| `protection::ptoc` | `src/protection/ptoc.rs` | No-trip below pickup, trip after `tset`, dropout before `tset`, disabled |
| `protection::pioc` | `src/protection/pioc.rs` | No-trip below pickup, immediate trip, no time delay, stays tripped, disabled, reset, `set_enabled` |
| `diagnostics::latency` | `src/diagnostics/latency.rs` | Empty stats, record+stats, ring-buffer wrap, p99, display format, `stop()`, reset |
| `scl::parser` | `src/scl/parser.rs` | Default config, PTOC mapping, PIOC mapping, SV mapping, GOOSE mapping |

### Running Unit Tests

```bash
cargo test
```

### Latency p99 Budget

The `LatencyTracker` is used to measure SV-to-GOOSE elapsed time at runtime. The target budgets (see [`docs/IEC61850-PERFORMANCE.md`](IEC61850-PERFORMANCE.md)) are:

| Function | p99 target |
|----------|-----------|
| PIOC (P1) | ≤ 3 ms |
| PTOC (P3) | ≤ 20 ms (excluding `tset`) |

---

## Integration Tests with Recorded SV PCAPs

> **Status: TODO**

A PCAP-based replay test will allow the protection functions to be tested without hardware:

1. Capture a real SV stream from the Omicron CMC as a `.pcap` file.
2. Replay the PCAP file into the SV subscriber using `tcpreplay` or a purpose-built Rust test harness.
3. Assert that GOOSE trip messages are produced at the correct times.

Planned test cases:

- Normal load current — no trip expected
- Step overcurrent (> `iset`) — PTOC trip after `tset` ± 5 ms
- High overcurrent (> `pioc.iset`) — PIOC trip within 1 sample period (250 µs)
- Dropout before `tset` — no trip (current drops below `iset` before timer expires)
- Sample gap / lost ASDU — verify graceful handling (no spurious trip)

Files will be placed in `tests/pcap/` and the test harness in `tests/integration_sv.rs`.

---

## HIL Tests with Omicron CMC

Hardware-in-the-loop tests use the **Omicron CMC** to inject precise SV streams and verify GOOSE outputs. These tests validate the full path including the operating system, network stack, and hardware.

### Test Setup

```
Omicron CMC ──[IEC 61850-9-2LE SV]──▶ Process Bus Ethernet
                                              │
                                    ┌─────────┴──────────┐
                                    │  SEAPATH Container  │
                                    │  (bay IED binary)   │
                                    └─────────┬──────────-┘
                                              │
Omicron CMC ◀──[IEC 61850-8-1 GOOSE]────────┘
```

### Omicron Configuration

1. Configure the CMC to output SV at:
   - 50 Hz, 80 samples/cycle (4 000 Sa/s)
   - SVID matching the bay's `sv.multicast_mac`
2. Configure GOOSE subscriber to:
   - Subscribe to the bay's `goose.dst_mac` / `goose.appid`
   - Trigger timing on `Trip` boolean going `true`

### Test Scenarios

| Test | Input | Expected output | Pass criterion |
|------|-------|----------------|----------------|
| PTOC no-trip | I = 0.9 × `iset` sustained | No GOOSE trip | No `Trip = true` within 5 s |
| PTOC trip | I = 1.1 × `iset` sustained for `tset` + 50 ms | GOOSE trip | `Trip = true` within `tset` + 20 ms |
| PTOC dropout | I = 1.1 × `iset` for `tset` − 10 ms, then I drops | No GOOSE trip | No `Trip = true` |
| PIOC trip | I = 1.1 × `pioc.iset` (single sample) | Immediate GOOSE trip | `Trip = true` within 3 ms |
| PIOC no-trip | I = 0.9 × `pioc.iset` | No GOOSE trip | No `Trip = true` |
| PTP loss | Disconnect PTP Grandmaster | Continued operation, degraded timestamps | No spurious trip; latency alarm logged |

### Latency Measurement

Record p99 latency from Omicron timing (SV injection → GOOSE detection):

- Run each test scenario 1 000 times (Omicron loop mode)
- Capture Omicron timing histogram
- Compare against IEC 61850-5 P1/P3 budgets

---

## Continuous Integration

The following checks run automatically on every pull request:

```bash
cargo fmt --check         # Code formatting
cargo clippy -- -D warnings  # Linting
cargo test                # Unit tests
```

Integration and HIL tests are run manually in the lab before releases.

---

## See Also

- [`docs/IEC61850-PERFORMANCE.md`](IEC61850-PERFORMANCE.md) — trip-time performance classes and latency budgets
- [`docs/modules/DIAGNOSTICS_LATENCY.md`](modules/DIAGNOSTICS_LATENCY.md) — runtime latency tracker
- [`docs/ROADMAP.md`](ROADMAP.md) — planned test milestones
