# Module: Configuration

**Source file**: [`src/config.rs`](../../src/config.rs)

---

## Purpose

The configuration module defines all runtime settings for a bay IED container. Settings are loaded from a JSON file at startup and mapped to typed Rust structs. The same structs are used throughout the codebase — no "stringly typed" settings at runtime.

---

## JSON Layout

A complete bay configuration file:

```json
{
  "ptoc": {
    "iset": 300.0,
    "tset": 100,
    "enabled": true
  },
  "pioc": {
    "iset": 1200.0,
    "enabled": true
  },
  "ct": {
    "primary": 400.0,
    "secondary": 1.0
  },
  "adc": {
    "scale_factor": 0.001,
    "offset": 0.0
  },
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
}
```

---

## Configuration Structs

### `SystemConfig`

Top-level container. Serialised/deserialised as the root JSON object.

```rust
pub struct SystemConfig {
    pub ptoc:  PtocConfig,
    pub pioc:  PiocConfig,
    pub ct:    CtConfig,
    pub adc:   AdcConfig,
    pub goose: GooseConfig,
    pub sv:    SvConfig,
}
```

Load from file:

```rust
let config = SystemConfig::from_json_file("/config/bay1.json")?;
```

### `PtocConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `iset` | `f64` | 100.0 | Pickup current (primary A) |
| `tset` | `u64` | 100 | Time delay (ms) |
| `enabled` | `bool` | true | Enable/disable |

### `PiocConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `iset` | `f64` | 500.0 | Instantaneous pickup (primary A) |
| `enabled` | `bool` | true | Enable/disable |

### `CtConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `primary` | `f64` | 400.0 | CT primary current (A) |
| `secondary` | `f64` | 1.0 | CT secondary current (A) |

Helper: `CtConfig::ratio()` returns `primary / secondary`.

### `AdcConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scale_factor` | `f64` | 0.001 | ADC count → secondary A factor |
| `offset` | `f64` | 0.0 | ADC zero-offset (counts) |

### `GooseConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `dst_mac` | `String` | `"01:0C:CD:01:00:00"` | GOOSE multicast destination MAC |
| `appid` | `u16` | 0x0001 | GOOSE APPID |
| `goid` | `String` | `"PTOC_TRIP"` | GOOSE identifier |
| `gocb_ref` | `String` | `"IED1LD0/LLN0$GO$PTOC1"` | Control block reference |
| `dat_set` | `String` | `"IED1LD0/LLN0$PTOC1"` | Dataset reference |
| `interface` | `String` | `"eth0"` | Network interface name |

### `SvConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `samples_per_cycle` | `usize` | 80 | Must match MU configuration |
| `interface` | `String` | `"eth0"` | Network interface name |
| `multicast_mac` | `String` | `"01:0C:CD:04:00:00"` | SV multicast MAC from the MU |

---

## Loading and Saving

```rust
// Load from JSON file
let config = SystemConfig::from_json_file("/config/bay1.json")?;

// Save to JSON file
config.to_json_file("/config/bay1_backup.json")?;

// Use defaults
let config = SystemConfig::default();
```

The environment variable `IED_CONFIG` is read by `main.rs` to determine the config file path (fallback: `config/bay1.json`).

---

## Future: SCD-Driven Configuration

In Phase 2, `SystemConfig` will be populated by the SCD parser rather than a hand-written JSON file:

```
SCD file ──▶ SclParser::parse_for_ied() ──▶ IedConfig ──▶ SclParser::to_system_config() ──▶ SystemConfig
```

The bridge function `SclParser::to_system_config()` is already implemented in `src/scl/parser.rs`. What remains is the XML parsing step — see [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md).

The JSON files in `config/` will remain as fallback/override mechanism and for testing without a full SCD.

---

## TODO

- [ ] **Validation** — add range checks on `iset`, `tset`, CT ratio, APPID (must be ≤ 0x3FFF for GOOSE)
- [ ] **Hot reload** — allow settings to be updated at runtime via a signal or file watch without restarting the container
- [ ] **Per-phase settings** — extend `PtocConfig` / `PiocConfig` with per-phase enable flags for three-phase support
- [ ] **SCD source** — populate from SCD in Phase 2

---

## See Also

- Example config files: [`config/bay1.json`](../../config/bay1.json), [`config/bay2.json`](../../config/bay2.json)
- [`docs/modules/SCL_PARSER.md`](SCL_PARSER.md) — future SCD-driven configuration
- [`deploy/README.md`](../../deploy/README.md) — per-bay JSON configuration guide
