# Module: SV Input

**Source file**: [`src/io/sv_input.rs`](../../src/io/sv_input.rs)

---

## Purpose

The SV Input module receives **IEC 61850-9-2LE Sampled Values** frames from the process-bus Ethernet network and extracts the raw ADC current sample for each arriving frame.

It is the entry point of the protection data path. Every arriving SV frame triggers one iteration of the processing loop.

---

## Inputs / Outputs

| Direction | Type | Description |
|-----------|------|-------------|
| Input | Raw Ethernet frame (AF_PACKET socket) | Layer-2 frame received from NIC VF |
| Input | `SvConfig` | Interface name, multicast MAC, samples per cycle |
| Output | `SampleData` | `current_adc`, `sample_number`, `timestamp_us` |

### `SampleData` fields

| Field | Type | Description |
|-------|------|-------------|
| `current_adc` | `i32` | Raw ADC value from the SV ASDU (first current channel) |
| `sample_number` | `u16` | ASDU sample counter (0–79 per cycle at 50 Hz) |
| `timestamp` | `u64` | Microseconds since UNIX epoch (from `clock_gettime(CLOCK_REALTIME)`) |

---

## IEC 61850-9-2LE Assumptions

- **EtherType**: `0x88BA` (Sampled Values)
- **Sample rate**: 80 samples/cycle at 50 Hz = 4 000 Sa/s
- **ASDU count per frame**: 1 (multi-ASDU support is TODO — see below)
- **Current channel**: First `i32` value in the ASDU dataset entry (index 0)
- **Multicast MAC**: Configurable; typically `01:0C:CD:04:XX:XX`
- **No VLAN tag**: Assumes untagged frames on the process-bus VLAN (VLAN filtering is TODO)

Decoding is performed by `iec_61850_lib::decode_smv::decode_smv` from the [`OpenEnergyTools/iec61850lib`](https://github.com/OpenEnergyTools) library.

---

## Real-Time Constraints

| Constraint | Value | Notes |
|------------|-------|-------|
| Sample interval | 250 µs | At 4 000 Sa/s |
| Max receive latency | < 100 µs | Budget for socket read + decode |
| Socket mode | Non-blocking | `receive_sample()` returns immediately if no data |

The socket is set to **non-blocking** so the calling RT loop can poll and continue if no frame has arrived yet. Dropped frames are detected via `sample_number` gaps.

---

## Configuration

Configuration is provided through `SvConfig` in the JSON file:

```json
"sv": {
  "samples_per_cycle": 80,
  "interface": "eth0v0",
  "multicast_mac": "01:0C:CD:04:00:01"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `samples_per_cycle` | 80 | Must match the MU configuration |
| `interface` | `"eth0"` | Linux network interface — use the SR-IOV VF, not the PF |
| `multicast_mac` | `"01:0C:CD:04:00:00"` | SV multicast MAC from the merging unit |

In the future, `interface` and `multicast_mac` will be derived from the SCD file's SMV control block — see [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md).

---

## Implemented

- [x] Raw AF_PACKET socket creation and binding (Linux only)
- [x] Non-blocking receive with frame size validation
- [x] Ethernet header parsing via `iec_61850_lib::decode_basics::decode_ethernet_header`
- [x] SV PDU decoding via `iec_61850_lib::decode_smv::decode_smv`
- [x] `SampleData` extraction (ADC value, sample number, system timestamp)
- [x] Fallback simulation mode on non-Linux targets

---

## TODO

- [ ] **Multi-ASDU support** — 9-2LE frames may carry up to 8 ASDUs per frame (common for IEC 61869-9 MUs). Currently only the first ASDU is processed.
- [ ] **Multicast group join** — the socket should join the SV multicast group (`setsockopt SO_ADD_MEMBERSHIP`) to work correctly on switches with IGMP snooping.
- [ ] **VLAN tag stripping** — process-bus networks often use 802.1Q VLAN tags. The decoder should strip the 4-byte VLAN header before passing the frame to `decode_smv`.
- [ ] **PTP-based timestamp** — currently `timestamp` is taken from `clock_gettime(CLOCK_REALTIME)` at receive time. Future improvement: use the hardware RX timestamp from the socket `SO_TIMESTAMPING` ancillary data for sub-microsecond accuracy.
- [ ] **Sample drop detection** — compare `sample_number` between consecutive frames and log/alarm on gaps.
- [ ] **Stream ID filtering** — when multiple SV streams arrive on the same interface, filter by SVID from the SV header.

---

## See Also

- [`docs/modules/MEASUREMENT_SCALING.md`](MEASUREMENT_SCALING.md) — converts `current_adc` to primary Amperes
- [`docs/modules/MEASUREMENT_RMS.md`](MEASUREMENT_RMS.md) — RMS calculation from the scaled samples
- [`docs/IEC61850-PERFORMANCE.md`](../IEC61850-PERFORMANCE.md) — timing budget for SV reception
