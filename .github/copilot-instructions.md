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
