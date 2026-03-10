# Module: GOOSE Output

**Source file**: [`src/io/goose_output.rs`](../../src/io/goose_output.rs)

---

## Purpose

The GOOSE Output module encodes and transmits **IEC 61850-8-1 GOOSE** (Generic Object Oriented Substation Event) messages over a raw Ethernet socket. It is the final step in the protection data path — converting a protection function's `Trip` decision into a layer-2 multicast frame on the process bus.

---

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | `GooseConfig` | Destination MAC, APPID, GOID, GoCBRef, dataset, interface |
| Input | `trip: bool` | Trip signal from protection function |
| Input | `timestamp: u64` | Microseconds since UNIX epoch (GOOSE `T` field) |
| Output | Raw Ethernet frame | Sent via AF_PACKET socket to the process bus |

---

## IEC 61850-8-1 GOOSE Frame

- **EtherType**: `0x88B8`
- **Destination MAC**: Configurable multicast (typically `01:0C:CD:01:XX:XX`)
- **APPID**: 16-bit identifier, unique per GOOSE control block
- **`stNum`** (State Number): Increments on every **data change** (trip → clear or clear → trip)
- **`sqNum`** (Sequence Number): Increments on every retransmission within the same state
- **Dataset**: Single `BOOLEAN` entry representing the trip signal
- **`timeAllowedToLive`**: Set to the maximum expected retransmission interval (currently hardcoded)

Encoding is performed by `iec_61850_lib::encode_goose::encode_goose`.

---

## `stNum` / `sqNum` Handling

IEC 61850-8-1 requires specific counter behaviour:

| Event | `stNum` | `sqNum` |
|-------|---------|---------|
| Data change (e.g., `false → true`) | Increment | Reset to 0 |
| Retransmission (same data, no change) | Unchanged | Increment |
| Startup / init | 1 | 0 |

The current implementation increments `stNum` on every call to `publish_trip()` when the trip state changes from the previous call. `sqNum` is incremented on subsequent calls with the same state.

---

## Real-Time Constraints

| Constraint | Value | Notes |
|------------|-------|-------|
| Max encoding + send time | < 1 ms | Budget for PIOC P1 compliance |
| Socket mode | Blocking (send) | `sendto` is synchronous; frame is queued in kernel |
| First GOOSE after trip | Immediate | No buffering or batching |

---

## Configuration

Configuration is provided through `GooseConfig` in the JSON file:

```json
"goose": {
  "dst_mac": "01:0C:CD:01:00:01",
  "appid": 1,
  "goid": "BAY1_PTOC_TRIP",
  "gocb_ref": "BAY1IED/PROT/LLN0$GO$GCB_PTOC",
  "dat_set": "BAY1IED/PROT/LLN0$PTOC1",
  "interface": "eth0v0"
}
```

| Field | Description |
|-------|-------------|
| `dst_mac` | Multicast MAC for GOOSE subscribers (must be unique per GoCB) |
| `appid` | APPID identifies the GOOSE control block (0x0001–0x3FFF) |
| `goid` | Informational string identifying the GOOSE message |
| `gocb_ref` | GOOSE control block reference in IEC 61850 naming format |
| `dat_set` | Dataset reference (logical device / LN / dataset) |
| `interface` | Linux interface — use the SR-IOV VF |

In the future, these values will be read from the SCD file's `GSE` address section — see [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md).

---

## Implemented

- [x] Raw AF_PACKET socket creation and binding (Linux only)
- [x] Source MAC address detection from the network interface
- [x] GOOSE PDU encoding via `iec_61850_lib::encode_goose::encode_goose`
- [x] `stNum` increment on state change, `sqNum` increment on retransmission
- [x] MAC address string parsing
- [x] Fallback simulation mode on non-Linux targets

---

## TODO

- [ ] **Retransmission profile** — IEC 61850-8-1 requires GOOSE to be retransmitted after a state change using an accelerated schedule (e.g., T1 = 2 ms, T2 = 4 ms, T3 = 8 ms … up to a steady-state interval T_max). Currently only a single frame is sent per call; the RT loop must call `publish_trip()` repeatedly to retransmit.
- [ ] **VLAN tagging (802.1Q)** — process-bus switches may require VLAN-tagged frames. Add optional VLAN ID and priority fields to `GooseConfig`.
- [ ] **`timeAllowedToLive` calculation** — should be derived from the retransmission profile (2 × T_max).
- [ ] **Dataset size / additional entries** — the current implementation sends a single `BOOLEAN` trip value. Future support for multi-entry datasets (e.g., trip + quality + timestamp).
- [ ] **GOOSE receiver** — for RBRF (breaker failure) and interlocking, the container needs to subscribe to GOOSE messages from other IEDs.

---

## See Also

- [`docs/modules/PROTECTION_PTOC.md`](PROTECTION_PTOC.md) — source of the trip signal
- [`docs/modules/PROTECTION_PIOC.md`](PROTECTION_PIOC.md) — source of the instantaneous trip signal
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — GOOSE transfer-time requirements
- [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md) — future SCD-driven configuration
