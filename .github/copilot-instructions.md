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

## Additional Resources

- [IEC 61850 Standard Overview](https://en.wikipedia.org/wiki/IEC_61850)
- [Rust Documentation](https://doc.rust-lang.org/)
- [The Rust Book](https://doc.rust-lang.org/book/)
# POC Protection Functions - Copilot Instructions

This repository implements IEC 61850 compliant protection functions in Rust.

## Context
- Uses `iec_61850_lib` from OpenEnergyTools/iec61850lib for GOOSE/SV encoding/decoding
- Target: Linux (VM or bare metal)
- Test equipment: Omicron
- Frequency: 50 Hz, 80 samples/cycle (4000 samples/sec)

## IEC 61850 Logical Nodes
- PTOC: Time Overcurrent Protection (implemented)
- PDIF: Differential Protection (future)
- PDIS: Distance Protection (future)
- XCBR: Circuit Breaker (future)

## Architecture
- SV input → Protection Function → GOOSE trip output
- All scaling factors (CT ratio, ADC) are configurable
- Protection settings (Iset, Tset) are configurable

## Key Files
- `src/protection/ptoc.rs` - PTOC logic with definite time characteristic
- `src/measurement/rms.rs` - RMS calculation from samples
- `src/io/sv_input.rs` - Sampled Values decoder
- `src/io/goose_output.rs` - GOOSE trip encoder

## Design Principles
- Modular architecture with clear separation of concerns
- Configuration-driven behavior
- Type-safe interfaces
- Efficient RMS calculation
- IEC 61850-8-1 compliant GOOSE/SV encoding
