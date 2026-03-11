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

## Retransmission Scheduler

IEC 61850-8-1 requires accelerated retransmissions after a state change so that subscribers recover quickly from a missed frame.

### Schedule (times from the state-change frame)

| Step | Cumulative offset | Next interval |
|------|-------------------|---------------|
| State change | 0 ms (immediate) | — |
| Retransmit 1 | +2 ms | 2 ms |
| Retransmit 2 | +6 ms | 4 ms |
| Retransmit 3 | +14 ms | 8 ms |
| Retransmit 4 | +30 ms | 16 ms |
| Heartbeat | every 1 000 ms | 1 000 ms |

Call `GoosePublisher::tick(now_us)` on every sample iteration:

```rust
loop {
    let now = get_timestamp_micros();
    goose.tick(now)?;                      // sends retransmit/heartbeat if due
    if trip != goose.last_trip_state() {
        goose.publish_trip(trip, now)?;    // state change — resets schedule
    }
}
```

## Implemented

- [x] Raw AF_PACKET socket creation and binding (Linux only)
- [x] Source MAC address detection from the network interface
- [x] GOOSE PDU encoding via `iec_61850_lib::encode_goose::encode_goose`
- [x] `stNum` increment on state change, `sqNum` increment on retransmission
- [x] IEC 61850-8-1 retransmission schedule (2/4/8/16 ms) + 1 s heartbeat via `tick()`
- [x] MAC address string parsing
- [x] Non-Linux build fix — `#[cfg]` conditional for `DEFAULT_SRC_MAC`

---

## TODO

- [ ] **VLAN tagging (802.1Q)** — add optional VLAN ID and priority to `GooseConfig`
- [ ] **`timeAllowedToLive`** — derive from retransmission profile (2 × T_max = 2 032 ms)
- [ ] **Multi-entry datasets** — trip + quality + timestamp
- [ ] **GOOSE receiver** — subscribe to XCBR/interlocking GOOSE from other IEDs

---

## See Also

- [`docs/modules/PROTECTION_PTOC.md`](PROTECTION_PTOC.md) — source of the trip signal
- [`docs/modules/PROTECTION_PIOC.md`](PROTECTION_PIOC.md) — source of the instantaneous trip signal
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — GOOSE transfer-time requirements
- [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md) — future SCD-driven configuration
