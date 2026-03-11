# POC-protection-functions

IEC 61850 compliant protection functions implemented in Rust.

## Tech Stack

- **Language**: Rust
- **Domain**: IEC 61850 electrical power system protection
- **Focus**: Protection functions for electrical grids

## Coding Guidelines

- Follow Rust naming conventions (snake_case for functions/variables, PascalCase for types)
- Use `cargo fmt` for code formatting
- Run `cargo clippy` for linting before committing
- Write documentation comments (`///`) for all public APIs
- Add unit tests for all functionality
- Use `Result<T, E>` for error handling instead of panics
- Prefer `&str` over `String` for function parameters when ownership isn't needed
- Use explicit type annotations when it improves code clarity

## Project Structure

- Place source code in `src/`
- Keep tests next to the code they test or in `tests/` for integration tests
- Document modules with module-level comments (`//!`)
- Organize code by protection function type (e.g., overcurrent, distance, differential)

## IEC 61850 Specific Guidelines

- Follow IEC 61850 naming conventions for data objects and attributes
- Ensure compliance with IEC 61850-7-4 for common data classes
- Document any deviations from the standard
- Use appropriate data types that map to IEC 61850 data types

## Build & Test

- Build: `cargo build`
- Test: `cargo test`
- Format: `cargo fmt`
- Lint: `cargo clippy`
- Documentation: `cargo doc --open`

---

# POC Protection Functions — Copilot Context

This repository implements IEC 61850 compliant protection functions in Rust for real-time bay protection on a SEAPATH host.

## Context

- Uses `iec_61850_lib` (OpenEnergyTools/iec61850lib) for GOOSE/SV encoding/decoding
- Target: Linux (VM or bare metal, SEAPATH)
- Test equipment: Omicron CMC
- Frequency: 50 Hz, 80 samples/cycle (4000 Sa/s)
- Main loop: `IED_LIVE=1` → live SV → protection → GOOSE; default → simulation

## IEC 61850 Logical Nodes

| Node | Status |
|------|--------|
| PTOC | Implemented — definite-time + IEC inverse-time curves + dropout hysteresis |
| PIOC | Implemented — instantaneous peak or short-window RMS mode |
| XCBR | Future — circuit breaker control |
| PDIF | Future — differential protection |
| PDIS | Future — distance protection |

## Architecture

```
SvSubscriber → adc_to_primary → PtocSlidingWindow ┐
                                                   ├─ GoosePublisher.publish_trip()
                             →        Pioc         ┘             + tick()
```

- `PtocSlidingWindow` evaluates PTOC on every sample with an O(1) incremental RMS
- `Pioc` in `Instantaneous` mode compares `|sample|` directly; in `ShortWindowRms(n)` mode it maintains an internal ring buffer
- `GoosePublisher.tick(now_us)` drives the IEC 61850-8-1 retransmission schedule (2/4/8/16 ms then 1 s heartbeat)

## Key Files

| File | Role |
|------|------|
| `src/protection/ptoc.rs` | PTOC — `effective_trip_delay_ms()`, dropout, inverse-time |
| `src/protection/ptoc_sliding.rs` | Per-sample sliding-window PTOC (production path) |
| `src/protection/pioc.rs` | PIOC — instantaneous and ShortWindowRms modes |
| `src/protection/three_phase.rs` | ThreePhasePtoc + ThreePhasePioc — per-phase instances, worst-case consolidation |
| `src/measurement/rms.rs` | RMS calculation (cycle + incremental) |
| `src/measurement/scaling.rs` | `adc_to_primary()`, `CurrentScaler` |
| `src/io/sv_input.rs` | SvSubscriber — raw AF_PACKET socket, 9-2LE decode |
| `src/io/goose_output.rs` | GoosePublisher — raw socket, retransmit scheduler |
| `src/config.rs` | All config structs: `PtocConfig`, `PiocConfig`, `PiocInputMode`, `PtocCurve` |
| `src/main.rs` | RT event loop — dual-mode (live via `IED_LIVE=1` / simulation) |
| `config/bay1.json` | Bay 1 production settings |
| `config/bay2.json` | Bay 2 production settings |

## Config Fields to Know

**PtocConfig** — `iset`, `tset`, `enabled`, `dropout_ratio` (default 0.95), `curve` (DefiniteTime | IecStandardInverse | IecVeryInverse | IecExtremelyInverse)

**PiocConfig** — `iset`, `enabled`, `input_mode` (`"Instantaneous"` or `{"ShortWindowRms": n}`)

## Design Principles

- Zero allocations in the hot path (ring buffers pre-allocated at startup)
- Single source of truth for inverse-time math: `effective_trip_delay_ms()` in `ptoc.rs`, reused by `ptoc_sliding.rs`
- Dropout hysteresis is applied at the `is_below_dropout()` check, not at `is_overcurrent()`
- GOOSE retransmission is driven by `tick()` — callers must call it every sample iteration
- `SvSubscriber` is non-blocking; the live loop spins on `WouldBlock` (low latency preferred over CPU saving)
- `src/io/goose_output.rs` compiles on all platforms; network sends are Linux-only (`#[cfg(target_os = "linux")]`)
