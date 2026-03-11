# POC Protection Functions

IEC 61850 compliant protection functions implemented in Rust for power system protection applications.

## Overview

This project implements protection functions according to the IEC 61850 standard, with support for:
- **Sampled Values (SV)** input for current measurements (IEC 61850-9-2)
- **GOOSE** messaging for trip signal output (IEC 61850-8-1)
- Configurable scaling for CT ratios and ADC conversion
- Compatible with Omicron test equipment

### Currently Implemented

| Function | Description | Standard |
|----------|-------------|----------|
| **PTOC** — definite-time | Fixed delay, dropout hysteresis | IEC 61850-7-4 |
| **PTOC** — inverse-time | IEC Standard / Very / Extremely Inverse curves | IEC 60255-151 |
| **PTOC** — sliding-window | Per-sample O(1) RMS (production path) | — |
| **PIOC** — instantaneous | Single-sample peak detection | IEC 61850-7-4 |
| **PIOC** — short-window RMS | Internal n-sample ring-buffer RMS, noise-resistant | — |
| **GOOSE retransmission** | Accelerated schedule (2/4/8/16 ms) + 1 s heartbeat | IEC 61850-8-1 |
| **Live I/O** | SV → scaling → protection → GOOSE, end-to-end | — |

## Architecture

```
┌──────────────────┐     ┌───────────────────────────────┐    ┌─────────────────┐
│  Sampled Values  │───▶│     Protection Functions      │───▶│   GOOSE Output  │
│  (IEC 61850-9-2) │     │                               │    │  (IEC 61850-8-1)│
│  SvSubscriber    │     │  PtocSlidingWindow  (PTOC)    │    │  GoosePublisher │
│  adc_to_primary  │     │  Pioc               (PIOC)    │    │  tick() + RT    │
└──────────────────┘     └───────────────────────────────┘    └─────────────────┘
         │                           │                                │
    ADC + CT                  Sliding RMS                    Retransmit Schedule
    Scaling               Dropout Hysteresis              2/4/8/16 ms → 1 s HB
                          Inverse-Time Curves
```

### Data Flow

1. **SV Input** — receive 80 samples/cycle (4000 Sa/s at 50 Hz) via raw Ethernet socket
2. **Scaling** — `adc_to_primary()` converts ADC counts → primary Amperes
3. **PIOC** — compares `|sample|` (or short-window RMS) to `iset`; trips in < 1 µs
4. **PTOC** — per-sample sliding-window RMS; definite-time or inverse-time trip delay
5. **GOOSE** — `publish_trip()` on state change; `tick()` drives retransmission schedule

## Building and Running

### Prerequisites

- Rust 1.70 or later
- Linux operating system (for raw socket support)
- **CAP_NET_RAW capability or root privileges** (required for live SV/GOOSE network I/O)

### Build

```bash
cargo build --release
```

### Run — simulation mode (default, no hardware needed)

```bash
cargo run --release
```

Runs 15 synthetic cycles (10 overcurrent then 5 normal) demonstrating PTOC/PIOC detection.

### Run — live I/O mode

```bash
# Build first
cargo build --release

# Grant capability once
sudo setcap cap_net_raw+ep target/release/poc_ptoc

# Run with bay config
IED_CONFIG=config/bay1.json IED_LIVE=1 ./target/release/poc_ptoc
```

`IED_LIVE=1` switches from the synthetic loop to the real SV → GOOSE path.

### Run Tests

```bash
cargo test
```

## Live Network I/O

Set `IED_LIVE=1` to switch from the built-in simulation to the real network path.

### SV Subscriber

```rust
use poc_protection_functions::{SvSubscriber, adc_to_primary};

let mut sv = SvSubscriber::new(config.sv.clone());
sv.init()?;  // requires CAP_NET_RAW on Linux

match sv.receive_sample() {        // non-blocking
    Ok(sample) => {
        let primary = adc_to_primary(sample.current_adc, &config.adc, &config.ct);
        // feed into protection functions
    }
    Err(e) if e.to_string().contains("No data available") => {} // WouldBlock — spin
    Err(e) => return Err(e),
}
```

### GOOSE Publisher with Retransmission

```rust
let mut goose = GoosePublisher::new(config.goose.clone());
goose.init()?;

loop {
    let now = get_timestamp_micros();
    goose.tick(now)?;                        // retransmit / heartbeat if due

    if trip != goose.last_trip_state() {
        goose.publish_trip(trip, now)?;      // state change → resets retransmit schedule
    }
}
```

`tick()` retransmits at +2 ms, +4 ms, +8 ms, +16 ms after a state change, then every 1 s.

### Privileges

```bash
sudo setcap cap_net_raw+ep target/release/poc_ptoc
IED_CONFIG=config/bay1.json IED_LIVE=1 ./target/release/poc_ptoc
```

### Network Setup

1. Connect IED/test equipment to the same Ethernet network
2. Set correct `interface` in the config file
3. Match multicast MACs: SV `01:0C:CD:04:XX:XX`, GOOSE `01:0C:CD:01:XX:XX`
4. Ensure no firewall blocks raw Ethernet frames

## Configuration

Configuration is loaded from a JSON file. Path is set via the `IED_CONFIG` environment variable (defaults to `config/ied.json`). Bay-specific examples: `config/bay1.json`, `config/bay2.json`.

### PTOC

```json
"ptoc": {
  "iset": 300.0,
  "tset": 100,
  "enabled": true,
  "dropout_ratio": 0.95,
  "curve": "DefiniteTime"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `iset` | `f64` | Pickup current in primary Amperes |
| `tset` | `u64` | Time delay in ms (acts as TMS for inverse-time curves) |
| `enabled` | `bool` | Enable / disable |
| `dropout_ratio` | `f64` | Current must fall below `iset × ratio` to reset (default 0.95) |
| `curve` | string | `"DefiniteTime"` \| `"IecStandardInverse"` \| `"IecVeryInverse"` \| `"IecExtremelyInverse"` |

Inverse-time formula (IEC 60255-151): `t = tset × k / ((I/Iset)^α − 1)`

| Curve | k | α |
|-------|---|---|
| IEC Standard Inverse | 0.14 | 0.02 |
| IEC Very Inverse | 13.5 | 1.0 |
| IEC Extremely Inverse | 80.0 | 2.0 |

### PIOC

```json
"pioc": {
  "iset": 1200.0,
  "enabled": true,
  "input_mode": "Instantaneous"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `iset` | `f64` | Pickup — **peak** A for `Instantaneous`, **RMS** A for `ShortWindowRms` |
| `enabled` | `bool` | Enable / disable |
| `input_mode` | string/object | `"Instantaneous"` or `{"ShortWindowRms": 8}` |

### CT, ADC, GOOSE, SV

```json
"ct":  { "primary": 400.0, "secondary": 1.0 },
"adc": { "scale_factor": 0.001, "offset": 0.0 },
"goose": {
  "dst_mac": "01:0C:CD:01:00:01",
  "appid": 1,
  "goid": "BAY1_PTOC_TRIP",
  "gocb_ref": "BAY1IED/PROT/LLN0$GO$GCB_PTOC",
  "dat_set": "BAY1IED/PROT/LLN0$PTOC1",
  "interface": "eth0"
},
"sv": {
  "samples_per_cycle": 80,
  "interface": "eth0",
  "multicast_mac": "01:0C:CD:04:00:01"
}
```

## Usage with Omicron Test Equipment

1. **Configure Omicron** SV output: 50 Hz, 4000 Sa/s, desired amplitude
2. **Configure GOOSE subscriber** in Omicron: subscribe to configured multicast MAC / APPID
3. **Network**: connect Omicron and IED to the same switch; use a dedicated NIC

### Example Test Scenarios

| Test | Setup | Expected |
|------|-------|----------|
| Below pickup | I = 0.8× Iset | No trip |
| Definite-time trip | I = 1.5× Iset held > Tset | Trip after Tset ms |
| Dropout (no trip) | I = 1.5× Iset, remove < Tset | No trip; resets to Idle |
| Instantaneous (PIOC) | I = 10× Iset | Trip within 1–2 samples (< 1 ms) |
| Inverse-time faster | I = 5× vs 2× Iset | Higher fault trips sooner |

## Project Structure

```
POC-protection-functions/
├── Cargo.toml
├── config/
│   ├── bay1.json               # Bay 1 settings
│   └── bay2.json               # Bay 2 settings
├── src/
│   ├── lib.rs                  # Public API re-exports
│   ├── main.rs                 # RT event loop (sim + live modes)
│   ├── config.rs               # All config structs + serde
│   ├── protection/
│   │   ├── traits.rs           # ProtectionFunction trait, ProtectionResult, TripState
│   │   ├── ptoc.rs             # PTOC — definite + inverse-time, dropout hysteresis
│   │   ├── ptoc_sliding.rs     # PTOC with per-sample sliding-window RMS
│   │   ├── pioc.rs             # PIOC — instantaneous or short-window RMS
│   │   └── three_phase.rs      # ThreePhasePtoc + ThreePhasePioc
│   ├── measurement/
│   │   ├── rms.rs              # RMS calculation (cycle + incremental)
│   │   └── scaling.rs          # adc_to_primary, CurrentScaler
│   ├── io/
│   │   ├── sv_input.rs         # SvSubscriber — raw socket, IEC 61850-9-2 decode
│   │   └── goose_output.rs     # GoosePublisher — raw socket, retransmission scheduler
│   ├── diagnostics/
│   │   └── latency.rs          # LatencyTracker — p50/p99/max statistics
│   └── scl/                    # SCL/SCD parser skeleton (Phase 2)
└── docs/
    ├── ARCHITECTURE.md
    ├── ROADMAP.md
    └── modules/                # Per-module deep-dives
```

## IEC 61850 Compliance

### Logical Nodes

| Node | Status |
|------|--------|
| **PTOC** | Implemented — definite-time + IEC inverse-time curves |
| **PIOC** | Implemented — instantaneous + short-window RMS mode |
| **XCBR** | Future |
| **PDIF** | Future |
| **PDIS** | Future |

### Communication

- **IEC 61850-9-2 (SV)** — raw Ethernet, 80 Sa/cycle at 50 Hz, multicast
- **IEC 61850-8-1 (GOOSE)** — raw Ethernet, accelerated retransmit schedule, `stNum`/`sqNum` compliant

## Roadmap

### Completed
- [x] PTOC definite-time + sliding-window RMS
- [x] PTOC inverse-time curves (Standard / Very / Extremely Inverse)
- [x] PTOC dropout hysteresis
- [x] PIOC instantaneous + short-window RMS input mode
- [x] GOOSE IEC 61850-8-1 retransmission scheduler + heartbeat
- [x] Live I/O — SV → protection → GOOSE (`IED_LIVE=1`)
- [x] JSON-driven per-bay configuration
- [x] Latency tracker (p50 / p99 / max)
- [x] Container image + docker-compose deployment

### Near Term
- [x] Three-phase PTOC/PIOC — trip on any of phases A, B, C
- [ ] External reset from XCBR GOOSE message
- [ ] End-to-end HIL test with Omicron (PTOC ≤ P3, PIOC ≤ P1)

### Phase 2 — SCD-Driven Configuration
- [ ] XML SCD parser (IED, LN, DataSet, SV, GOOSE sections)
- [ ] Map PTOC/PIOC data attributes from SCL
- [ ] Deploy bay from SCD with no manual JSON editing

### Long Term
- [ ] MMS server (LLN0, LPHD, PTOC1, PIOC1 data model)
- [ ] PDIF differential protection
- [ ] RBRF breaker failure protection
- [ ] PRP/HSR network redundancy

## Dependencies

- **iec_61850_lib**: IEC 61850 protocol implementation for GOOSE and SV
- **serde/serde_json**: Configuration serialization
- **log/env_logger**: Logging infrastructure

## License

Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## References

- IEC 61850-8-1: Communication networks and systems for power utility automation - Part 8-1: Specific communication service mapping (SCSM) - Mappings to MMS and to ISO/IEC 8802-3
- IEC 61850-9-2: Communication networks and systems for power utility automation - Part 9-2: Specific communication service mapping (SCSM) - Sampled values over ISO/IEC 8802-3
- IEEE C37.112: IEEE Standard for Inverse-Time Characteristic Equations for Overcurrent Relays
