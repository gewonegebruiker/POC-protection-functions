# Architecture

## Overview

The system runs one **bay IED container** per outgoing substation bay. Each container subscribes to an IEC 61850-9-2LE Sampled Values multicast stream, executes protection logic, and publishes GOOSE trip messages — all over a dedicated process-bus Ethernet network.

---

## Topology

```
┌───────────────────────────────────────────────────────────────────────┐
│                     SEAPATH Host (Yocto + PREEMPT_RT)                 │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │  CPU Core Layout                                                │  │
│  │                                                                 │  │
│  │  Core 0–1 : Host OS + cluster management (not isolated)         │  │
│  │  Core 2   : ptp4l / phc2sys  (dedicated, isolated)             │  │
│  │  Core 3   : Bay 1 IED container  (isolated, SCHED_FIFO)        │  │
│  │  Core 4   : Bay 2 IED container  (isolated, SCHED_FIFO)        │  │
│  │  ...                                                            │  │
│  │  Core N   : Shared — slow functions (MMS, logging, future)      │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐            │
│  │ Bay 1 IED     │  │ Bay 2 IED     │  │ Bay N IED     │            │
│  │ Container     │  │ Container     │  │ Container     │            │
│  │               │  │               │  │               │            │
│  │ SV rx → Scale │  │ SV rx → Scale │  │ SV rx → Scale │            │
│  │ → RMS → PTOC  │  │ → RMS → PTOC  │  │ → RMS → PTOC  │            │
│  │ → PIOC        │  │ → PIOC        │  │ → PIOC        │            │
│  │ → GOOSE tx    │  │ → GOOSE tx    │  │ → GOOSE tx    │            │
│  └──────┬────────┘  └──────┬────────┘  └──────┬────────┘            │
│         │                  │                  │                      │
│  ┌──────┴──────────────────┴──────────────────┴──────────────┐      │
│  │         SR-IOV Virtual Functions (one VF per bay)          │      │
│  └──────────────────────────┬───────────────────────────────-┘      │
│                             │                                        │
│  ┌──────────────────────────┴────────────────────────────────┐       │
│  │         Physical NIC (SR-IOV PF) — HW PTP support         │       │
│  └──────────────────────────┬────────────────────────────────┘       │
└────────────────────────────┬┴───────────────────────────────────────┘
                             │
             ┌───────────────┴───────────────────┐
             │         Process Bus Network         │
             │  • Merging Units (9-2LE SV)         │
             │  • Switchgear GOOSE subscribers     │
             │  • PTP Grandmaster (GPS-disciplined) │
             │  • Omicron CMC (HIL testing)         │
             │  • Grid-to-Great MUs                 │
             └────────────────────────────────────-┘
```

---

## SEAPATH Host

[SEAPATH](https://lf-energy.org/projects/seapath/) is a Linux Foundation Energy project that provides a purpose-built hypervisor for substation automation. It supplies:

- **PREEMPT_RT kernel** — deterministic interrupt latency
- **CPU isolation** (`isolcpus`, `nohz_full`, `rcu_nocbs`) — prevents OS jitter on dedicated cores
- **SR-IOV passthrough** — each container receives its own NIC VF with hardware-level isolation
- **PTP infrastructure** — `ptp4l` + `phc2sys` disciplined from a GPS-backed Grandmaster
- **Cluster management** — Ceph + live migration across multiple physical servers

The script [`deploy/setup-seapath-host.sh`](../deploy/setup-seapath-host.sh) automates the host-level preparation steps that SEAPATH does not handle automatically (SR-IOV VF count, IRQ affinity, CPU governor).

---

## SR-IOV Network Isolation

Each bay container receives a dedicated **SR-IOV Virtual Function (VF)** mapped directly into the container. This provides:

- **Hardware-level VLAN/MAC isolation** — each bay only sees its own SV and GOOSE multicast addresses
- **Low-latency DMA** — packets bypass the kernel network stack in the host
- **Independent interrupt steering** — NIC interrupts for each VF can be pinned to a specific CPU core

Configure the number of VFs using the script or manually:

```bash
echo 16 > /sys/class/net/eth0/device/sriov_numvfs
```

Then pass the VF interface (e.g., `eth0v0`) as the `interface` value in the bay's JSON configuration.

---

## PTP Time Synchronisation

Accurate timestamps are critical for:

1. **SV processing** — detecting sample drops and calculating inter-sample jitter
2. **GOOSE timestamps** — `T` field in GOOSE PDU (used by subscribers to detect replays)
3. **Latency measurement** — comparing SV arrival time to GOOSE publish time

### Setup

```
GPS Grandmaster ──[PTP over process bus]──▶ ptp4l (SEAPATH host)
                                                │
                                           phc2sys
                                                │
                                      CLOCK_REALTIME (system clock)
                                                │
                                    Available to all containers
                                    via clock_gettime(CLOCK_REALTIME)
```

- `ptp4l` runs on **Core 2** (dedicated, isolated) with the process-bus NIC.
- `phc2sys` synchronises the PHC (PTP Hardware Clock) to `CLOCK_REALTIME`.
- Containers call `clock_gettime(CLOCK_REALTIME)` — no additional PTP setup needed inside the container.
- Target synchronisation accuracy: **< 1 µs** with hardware PTP timestamping.

Reference configuration: [`deploy/docker-compose.yml`](../deploy/docker-compose.yml) — `ptp-sync` service.

---

## CPU Core Allocation Strategy

| Core | Role | Scheduling | Notes |
|------|------|-----------|-------|
| 0–1 | Host OS, libvirt, cluster mgmt | CFS (normal) | Not isolated |
| 2 | PTP (`ptp4l` + `phc2sys`) | SCHED_FIFO 90 | Isolated, dedicated |
| 3 | Bay 1 IED | SCHED_FIFO 80 | Isolated |
| 4 | Bay 2 IED | SCHED_FIFO 80 | Isolated |
| … | Bay N IED | SCHED_FIFO 80 | Isolated |
| Last 1–2 | MMS / logging / future | CFS | Shared |

### Rules

- **Fast functions (PIOC, target ≤ 3 ms)** — must have a **dedicated isolated core**. No other workload may share it.
- **Medium functions (PTOC, target ≤ 20 ms detection + `tset`)** — may share a core with one other PTOC-class bay provided measured jitter stays below 100 µs.
- **Slow functions (MMS, logging)** — may share cores freely; no hard real-time requirement.

The core-sharing experiment is planned for **Phase 3** (see [`docs/ROADMAP.md`](ROADMAP.md)).

---

## Container Internal Architecture

Each bay IED container runs the same binary, driven by a bay-specific JSON configuration file:

```
┌──────────────────────────────────────────────────────────────┐
│                   Bay N IED Container                         │
│                                                              │
│  ┌───────────────┐                                           │
│  │ bay_N.json    │ ← mounted read-only at /config/           │
│  └───────┬───────┘                                           │
│          │ SystemConfig                                       │
│          ▼                                                    │
│  ┌───────────────────────────────────────────────────┐       │
│  │                RT Event Loop (main.rs)             │       │
│  │                                                   │       │
│  │  SvSubscriber ──▶ CurrentScaler ──▶ RmsCalculator │       │
│  │       │                                    │      │       │
│  │       │                             primary_rms   │       │
│  │       │                                    │      │       │
│  │       │                            ┌───────┴────┐ │       │
│  │       │                            │ Ptoc/Pioc  │ │       │
│  │       │                            └───────┬────┘ │       │
│  │       │                                    │trip  │       │
│  │       │                                    ▼      │       │
│  │       │                          GoosePublisher   │       │
│  │       │                                           │       │
│  │       └──── LatencyTracker (diagnostics) ─────────┘       │
│  └───────────────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────────────┘
```

### Data Flow (per sample, every 250 µs at 4 000 Sa/s)

1. `SvSubscriber::receive_sample()` — raw Ethernet frame decoded from VF socket
2. `CurrentScaler::scale_to_primary()` — ADC counts → secondary A → primary A
3. `RmsCalculator::add_sample()` / `calculate()` — sliding or cycle-window RMS
4. `Ptoc::process()` / `Pioc::process()` — protection logic, produces `ProtectionResult`
5. On `Trip` / `TripPending` resolution → `GoosePublisher::publish_trip()` — GOOSE frame sent
6. `LatencyTracker::stop()` — records SV-to-GOOSE elapsed time for p99 reporting

---

## Multi-Hypervisor Deployment

For 70 bays distributed across multiple SEAPATH servers, the deployment scales horizontally:

- Each server runs its own `docker-compose.yml` with its subset of bay containers.
- All servers connect to the same process-bus Ethernet (or a dedicated VLAN per server).
- PTP Grandmaster is shared across all servers via the process bus.
- There is no cross-server coordination required at the protection layer — each bay is independent.

See [`deploy/README.md`](../deploy/README.md) for per-server prerequisites and scaling guidance.
