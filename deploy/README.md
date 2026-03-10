# Deployment Guide

This directory contains everything needed to deploy bay IED containers on a SEAPATH host.

## Contents

| File | Purpose |
|------|---------|
| `setup-seapath-host.sh` | One-time host preparation (RT kernel check, SR-IOV, IRQ affinity, CPU governor) |
| `docker-compose.yml` | Multi-bay container deployment with RT resource constraints |
| `../config/bay1.json` | Example configuration for bay 1 |
| `../config/bay2.json` | Example configuration for bay 2 |

---

## Prerequisites

### Host Requirements

| Requirement | Check command | Expected |
|-------------|--------------|---------|
| PREEMPT_RT kernel | `uname -v \| grep PREEMPT_RT` | Contains `PREEMPT_RT` |
| CPU isolation active | `cat /sys/devices/system/cpu/isolated` | Lists isolated core numbers |
| SR-IOV capable NIC | `cat /sys/class/net/eth0/device/sriov_totalvfs` | > 0 |
| Hardware PTP support | `ethtool -T eth0 \| grep hardware-transmit` | Present |
| `ptp4l` / `phc2sys` | `which ptp4l && which phc2sys` | Both found |
| Docker / Podman | `docker info` or `podman info` | Running |
| `CAP_NET_RAW` granted | (see below) | — |

### Kernel Boot Parameters

Add to `GRUB_CMDLINE_LINUX` in `/etc/default/grub`, then run `update-grub` and reboot:

```
isolcpus=2,3,4
nohz_full=2,3,4
rcu_nocbs=2,3,4
rcu_nocb_poll
irqaffinity=0,1
nosoftlockup
processor.max_cstate=1
intel_idle.max_cstate=0
intel_pstate=disable
```

Adjust the core numbers to match your hardware topology.

### Network Capabilities

Each container needs raw Ethernet socket access. The `docker-compose.yml` grants this via:

```yaml
cap_add:
  - NET_RAW    # Raw socket (SV/GOOSE layer-2)
  - NET_ADMIN  # Interface configuration
  - SYS_NICE   # RT scheduling (SCHED_FIFO)
  - IPC_LOCK   # Memory locking (mlockall)
ulimits:
  rtprio:
    soft: 99
    hard: 99
  memlock:
    soft: -1
    hard: -1
```

Alternatively, grant `cap_net_raw` to the binary directly:

```bash
sudo setcap cap_net_raw+ep target/release/poc_ptoc
```

---

## Step 1 — Prepare the Host

```bash
# Run as root on the SEAPATH hypervisor
NIC=eth0 ISOLATED_CORES=2,3,4 sudo ./deploy/setup-seapath-host.sh
```

The script performs these steps idempotently:

1. Verifies the PREEMPT_RT kernel is running.
2. Prints the recommended GRUB boot parameters (you must apply and reboot manually).
3. Sets the CPU frequency governor to `performance` on isolated cores.
4. Creates SR-IOV VFs on the specified NIC (default: 4 VFs).
5. Verifies hardware PTP timestamping is available.
6. Configures `irqbalance` to avoid isolated cores.
7. Steers NIC interrupt affinity to core 0.

After running the script, apply the GRUB parameters and reboot.

---

## Step 2 — Build the Container Image

```bash
# From the repository root
docker build -t poc-ied:latest .
```

The `Dockerfile` produces a minimal image containing:
- The compiled `poc_ptoc` binary (release build)
- No shell or unnecessary tooling (attack surface reduction)

---

## Step 3 — Configure Bay JSON Files

Each bay needs its own JSON file in the `config/` directory. Copy an example and adjust:

```bash
cp config/bay1.json config/bay3.json
# Edit bay3.json — update iset, tset, interface, MAC addresses, APPID, etc.
```

Key fields to change per bay:

| Field | Location | Notes |
|-------|----------|-------|
| `ptoc.iset` | `config/bayN.json` | Pickup current in primary A |
| `ptoc.tset` | `config/bayN.json` | Time delay in ms |
| `pioc.iset` | `config/bayN.json` | Instantaneous pickup in primary A |
| `ct.primary` | `config/bayN.json` | CT primary rating |
| `goose.dst_mac` | `config/bayN.json` | GOOSE multicast MAC (unique per bay) |
| `goose.appid` | `config/bayN.json` | GOOSE APPID (unique per bay) |
| `goose.interface` | `config/bayN.json` | SR-IOV VF interface (e.g. `eth0v0`) |
| `sv.multicast_mac` | `config/bayN.json` | SV multicast MAC from your MU |
| `sv.interface` | `config/bayN.json` | SR-IOV VF interface for SV reception |

---

## Step 4 — Update docker-compose.yml

Add a service block for each new bay, adjusting:

- `container_name` — unique per bay
- `environment.IED_CONFIG` — path to the bay's JSON file
- `cpuset` — the isolated core assigned to this bay

```yaml
bay3-ied:
  image: poc-ied:latest
  container_name: bay3-ied
  network_mode: host
  environment:
    - IED_CONFIG=/config/bay3.json
    - RUST_LOG=info
  volumes:
    - ./config:/config:ro
  cpuset: "5"          # Dedicated isolated core for bay 3
  mem_limit: 256m
  memswap_limit: 256m
  cap_add:
    - SYS_NICE
    - NET_RAW
    - NET_ADMIN
    - IPC_LOCK
  ulimits:
    rtprio: { soft: 99, hard: 99 }
    memlock: { soft: -1, hard: -1 }
  restart: unless-stopped
```

---

## Step 5 — Start Containers

```bash
# Start PTP sync and all bay IEDs
docker compose up -d

# Check logs
docker compose logs -f bay1-ied

# Check latency stats (printed periodically by the application)
docker compose logs bay1-ied | grep "latency"
```

---

## Scaling to Multiple Hypervisors (70 bays)

Distribute bays across SEAPATH servers as follows:

1. Replicate the `config/`, `deploy/` directories to each server.
2. Each server runs its own `docker-compose.yml` with its subset of bays.
3. All servers connect to the same process-bus Ethernet VLAN.
4. PTP Grandmaster is shared — all servers run `ptp4l` as slaves.
5. No inter-server coordination is needed at the protection layer.

Example distribution (16-core servers, 4 cores reserved for OS/PTP):

| Server | Bays | Cores used |
|--------|------|-----------|
| server-01 | Bay 1–12 | Core 3–14 |
| server-02 | Bay 13–24 | Core 3–14 |
| server-03 | Bay 25–36 | Core 3–14 |
| server-04 | Bay 37–48 | Core 3–14 |
| server-05 | Bay 49–60 | Core 3–14 |
| server-06 | Bay 61–70 | Core 3–12 |

---

## Troubleshooting

| Symptom | Likely cause | Resolution |
|---------|-------------|-----------|
| `Permission denied` on socket | Missing `CAP_NET_RAW` | Check `cap_add` in compose, or `setcap` the binary |
| No SV received | Wrong interface or multicast MAC | Verify `sv.interface` and `sv.multicast_mac` in JSON |
| GOOSE not reaching subscribers | Wrong APPID or dst_mac | Verify `goose.dst_mac` matches subscriber config |
| High trip latency (> 20 ms) | Core sharing / no RT kernel | Check `isolcpus`, `SCHED_FIFO`, `PREEMPT_RT` |
| PTP not synced | `ptp4l` not running or wrong interface | Check `ptp-sync` container logs |
| Container OOM-killed | Memory limit too low | Increase `mem_limit` (256 m is usually sufficient) |

---

## See Also

- [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) — full topology and design rationale
- [`docs/IEC61850-PERFORMANCE.md`](../docs/IEC61850-PERFORMANCE.md) — trip-time performance classes
- [`docs/TESTING.md`](../docs/TESTING.md) — HIL test procedure with Omicron
