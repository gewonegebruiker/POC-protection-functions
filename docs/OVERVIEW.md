# Project Overview

## Purpose

This repository implements **software-defined protection IEDs** (Intelligent Electronic Devices) as lightweight containers running on a [SEAPATH](https://lf-energy.org/projects/seapath/) hypervisor. Each container provides the full protection and control logic for one outgoing bay of an electrical substation — replacing dedicated proprietary hardware with a portable, configurable Rust application.

The long-term goal is to operate **70 bays** across multiple SEAPATH hypervisors, with each bay container receiving process-bus data and issuing protection trips entirely over standard IEC 61850 communication.

---

## System Inputs and Outputs

| Direction | Protocol | Standard | Notes |
|-----------|----------|----------|-------|
| **Input** | Sampled Values (SV) | IEC 61850-9-2LE | 80 samples/cycle at 50 Hz (4 000 Sa/s) |
| **Input** | Precision Time Protocol | IEEE 1588 / PTP | Synchronises sample timestamps via `ptp4l` + `phc2sys` |
| **Output** | GOOSE trip messages | IEC 61850-8-1 | Layer-2 multicast, sub-4 ms for PIOC |
| **Output** | MMS reporting *(planned)* | IEC 61850-8-1 | Phase 4 — SCADA integration |

---

## Scale Target

| Item | Value |
|------|-------|
| Bays per deployment | Up to 70 |
| Hypervisors | Multiple (SEAPATH cluster) |
| Containers per hypervisor | ~10–15 depending on CPU core count |
| Samples per second (per bay) | 4 000 |
| Max PIOC trip time | ≤ 3 ms (IEC 61850-5 P1) |
| Max PTOC trip time | `tset` + ≤ 20 ms detection (P3) |

---

## Test Equipment

- **Omicron CMC** — injects IEC 61850-9-2LE Sampled Values and subscribes to GOOSE trips
- **Grid-to-Great merging units** — real process-bus merging units generating 9-2LE SV streams

---

## Repository Layout

```
POC-protection-functions/
├── src/
│   ├── config.rs               # JSON-driven runtime configuration
│   ├── main.rs                 # RT event loop entry point
│   ├── lib.rs                  # Library root / public API
│   ├── io/
│   │   ├── sv_input.rs         # SV subscriber (EtherType 0x88BA)
│   │   └── goose_output.rs     # GOOSE publisher (EtherType 0x88B8)
│   ├── measurement/
│   │   ├── rms.rs              # RMS calculation (cycle-window + sliding)
│   │   └── scaling.rs          # ADC → secondary → primary scaling chain
│   ├── protection/
│   │   ├── traits.rs           # ProtectionFunction trait
│   │   ├── ptoc.rs             # PTOC — definite-time overcurrent
│   │   ├── ptoc_sliding.rs     # PTOC with sliding-window RMS
│   │   └── pioc.rs             # PIOC — instantaneous overcurrent
│   ├── diagnostics/
│   │   └── latency.rs          # Ring-buffer latency tracker (p99 budgets)
│   └── scl/
│       ├── parser.rs           # SCL/SCD parser skeleton
│       └── types.rs            # SCD data model types
├── config/
│   ├── bay1.json               # Example bay 1 runtime configuration
│   └── bay2.json               # Example bay 2 runtime configuration
├── deploy/
│   ├── docker-compose.yml      # Multi-bay container deployment
│   ├── setup-seapath-host.sh   # Host RT/SR-IOV preparation script
│   └── README.md               # Deployment guide
├── docs/
│   ├── OVERVIEW.md             # This file
│   ├── ARCHITECTURE.md         # SEAPATH host / VM / container topology
│   ├── IEC61850-PERFORMANCE.md # Performance class requirements
│   ├── ROADMAP.md              # Phased development plan
│   ├── TESTING.md              # Test strategy
│   └── modules/                # Per-module AI / engineer guides
│       ├── IO_SV_INPUT.md
│       ├── IO_GOOSE_OUTPUT.md
│       ├── MEASUREMENT_RMS.md
│       ├── MEASUREMENT_SCALING.md
│       ├── PROTECTION_PTOC.md
│       ├── PROTECTION_PIOC.md
│       ├── CONFIG.md
│       ├── DIAGNOSTICS_LATENCY.md
│       └── SCL_PARSER.md
└── examples/
    └── ptoc_test.rs            # Standalone PTOC demonstration
```

---

## Key Design Principles

1. **One container = one bay IED** — isolated resources, clear failure domain.
2. **Configuration-driven** — bay parameters come from a JSON file today; an SCD file will drive them in Phase 2.
3. **Real-time first** — the processing loop is pinned to an isolated CPU core; SCHED_FIFO is used inside the container.
4. **IEC 61850 native** — SV and GOOSE use the same layer-2 multicast wire format as a hardware IED.
5. **Incremental roadmap** — PTOC + PIOC now, SCL parser and MMS in later phases.

---

## Further Reading

| Document | Location |
|----------|----------|
| Architecture and deployment topology | [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) |
| IEC 61850-5 performance class compliance | [`docs/IEC61850-PERFORMANCE.md`](IEC61850-PERFORMANCE.md) |
| Development roadmap | [`docs/ROADMAP.md`](ROADMAP.md) |
| Test strategy | [`docs/TESTING.md`](TESTING.md) |
| Module guides | [`docs/modules/`](modules/) |
| Deployment instructions | [`deploy/README.md`](../deploy/README.md) |
