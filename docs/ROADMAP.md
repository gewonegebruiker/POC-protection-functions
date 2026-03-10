# Roadmap

## Phase 1 — Single Bay SV → PTOC/PIOC → GOOSE on SEAPATH

**Goal**: Validate the end-to-end protection chain for one bay on a SEAPATH host with real test equipment.

### Milestones

- [x] PTOC protection logic with definite-time characteristic (`src/protection/ptoc.rs`)
- [x] PIOC instantaneous protection (`src/protection/pioc.rs`)
- [x] RMS calculation — cycle-window (`src/measurement/rms.rs`)
- [x] Sliding-window PTOC (`src/protection/ptoc_sliding.rs`)
- [x] ADC / CT scaling chain (`src/measurement/scaling.rs`)
- [x] SV subscriber — raw Ethernet socket, 9-2LE decode (`src/io/sv_input.rs`)
- [x] GOOSE publisher — raw Ethernet socket, IEC 61850-8-1 encode (`src/io/goose_output.rs`)
- [x] JSON-driven configuration (`src/config.rs`, `config/bay1.json`)
- [x] Latency tracker with p99 statistics (`src/diagnostics/latency.rs`)
- [x] Container image + docker-compose deployment (`Dockerfile`, `deploy/docker-compose.yml`)
- [x] SEAPATH host setup script (`deploy/setup-seapath-host.sh`)
- [ ] End-to-end HIL test with Omicron CMC — verify PTOC trip time ≤ P3
- [ ] End-to-end HIL test — verify PIOC trip time ≤ P1 (3 ms)
- [ ] Confirm PTP synchronisation < 1 µs using hardware timestamping

---

## Phase 2 — SCD-Driven Configuration

**Goal**: Replace JSON configuration with protection function settings derived from an IEC 61850 SCD (Substation Configuration Description) file.

### Milestones

- [x] SCL type definitions (`src/scl/types.rs`)
- [x] SCL-to-runtime-config bridge (`src/scl/parser.rs` — `to_system_config`)
- [ ] XML SCD parser — extract IED, LN, DataSet, SV, GOOSE sections (`src/scl/parser.rs` — `parse_for_ied`)
- [ ] Map PTOC/PIOC data attributes from SCL to `PtocConfig` / `PiocConfig`
- [ ] Map SV control block → `SvConfig`
- [ ] Map GOOSE control block → `GooseConfig`
- [ ] Validate parsed SCD against IEC 61850-6 schema rules
- [ ] Acceptance test: deploy bay from SCD file with no manual JSON editing

---

## Phase 3 — Multi-Container Core-Sharing Experiment

**Goal**: Quantify the latency impact of sharing a single isolated core between two PTOC-class bay containers.

### Milestones

- [ ] Deploy 2 bay containers on the same isolated core
- [ ] Measure worst-case SV-to-GOOSE latency with shared core (cyclictest + application latency tracker)
- [ ] Establish pass/fail criterion: p99 latency < 5 ms with shared core
- [ ] Document results — update core allocation guidance in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)
- [ ] Scale to 10+ bays across multiple hypervisors
- [ ] Validate PTP accuracy stays < 1 µs under load

---

## Phase 4 — MMS Server

**Goal**: Add IEC 61850-8-1 MMS reporting so SCADA systems can read protection status and settings.

### Milestones

- [ ] Evaluate MMS library options (open62541, libiec61850, pure Rust)
- [ ] Implement LLN0, LPHD, PTOC1, PIOC1 data model in MMS server
- [ ] Map protection function state (Idle / Pickup / Trip) to MMS data attributes
- [ ] Implement BRCB (Buffered Report Control Block) for event reporting
- [ ] Test MMS connection from SCADA simulator
- [ ] Document MMS configuration in SCD

---

## Future Considerations

- **PDIF** — Transformer/line differential protection (requires multi-terminal SV comparison)
- **PDIS** — Distance protection (impedance measurement)
- **RBRF** — Breaker failure protection
- **PRP/HSR** — Parallel Redundancy Protocol for network redundancy
- **XCBR** — Circuit breaker logical node with GOOSE-based control
- **Watchdog** — Container health monitoring, automatic restart on hang
- **Web UI** — Real-time monitoring of protection function state and latency metrics
