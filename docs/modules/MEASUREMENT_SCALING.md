# Module: Measurement Scaling

**Source file**: [`src/measurement/scaling.rs`](../../src/measurement/scaling.rs)

---

## Purpose

Converts raw ADC integer counts from the SV frame into **primary Amperes** by applying a two-stage scaling chain:

```
ADC counts → secondary Amperes → primary Amperes
```

This is necessary because:

1. The merging unit (MU) digitises the secondary current of a current transformer (CT).
2. The ADC output is an integer count proportional to the secondary current.
3. Protection functions work in **primary Amperes** to match nameplate settings (e.g., "pick up at 100 A primary").

---

## Scaling Chain

```
                  ADC scale_factor                  CT ratio (primary/secondary)
raw ADC (i32)  ─────────────────────▶  secondary A  ───────────────────────────▶  primary A
                  − offset
```

### Stage 1: ADC → Secondary Amperes

```
secondary_A = (adc_value − offset) × scale_factor
```

- `offset`: zero-point correction (ADC counts at zero current). Typically 0.
- `scale_factor`: converts one ADC count to secondary Amperes (e.g., `0.001` → 1 mA/count).

### Stage 2: Secondary → Primary Amperes

```
primary_A = secondary_A × (ct_primary / ct_secondary)
```

- `ct_primary`: CT primary rating (e.g., 400 for a 400/1 CT).
- `ct_secondary`: CT secondary rating (1 or 5 A).
- CT ratio = `ct_primary / ct_secondary` (e.g., 400).

### Combined

```
primary_A = (adc_value − offset) × scale_factor × (ct_primary / ct_secondary)
```

---

## Key Types

### `CurrentScaler`

The main struct for the RT hot path. Constructed once, then called per sample:

```rust
let scaler = CurrentScaler::new(adc_config, ct_config);

// Per sample (hot path):
let primary_a = scaler.scale_to_primary(sample.current_adc);
```

### Standalone Functions

Available for testing or batch use:

| Function | Description |
|----------|-------------|
| `adc_to_secondary(i32, &AdcConfig) -> f64` | Stage 1 only |
| `secondary_to_primary(f64, &CtConfig) -> f64` | Stage 2 only |
| `adc_to_primary(i32, &AdcConfig, &CtConfig) -> f64` | Both stages combined |
| `adc_samples_to_secondary(&[i32], &AdcConfig) -> Vec<f64>` | Batch stage 1 |
| `adc_samples_to_primary(&[i32], &AdcConfig, &CtConfig) -> Vec<f64>` | Batch both stages |

---

## Configuration

Configuration is provided in the JSON file:

```json
"ct": {
  "primary": 400.0,
  "secondary": 1.0
},
"adc": {
  "scale_factor": 0.001,
  "offset": 0.0
}
```

| Field | Example | Description |
|-------|---------|-------------|
| `ct.primary` | 400.0 | CT primary current rating (A) |
| `ct.secondary` | 1.0 | CT secondary current rating (A); use 1.0 or 5.0 |
| `adc.scale_factor` | 0.001 | ADC count → secondary A conversion factor |
| `adc.offset` | 0.0 | ADC zero-offset correction (counts) |

**Example**: Omicron CMC with a 400/1 CT and an ADC that outputs 1 000 counts per 1 A secondary:

```
scale_factor = 1.0 / 1000.0 = 0.001
ct.primary   = 400.0
ct.secondary = 1.0

→ 1000 ADC counts = 1.0 A secondary = 400.0 A primary
```

---

## Real-Time Constraints

| Constraint | Value |
|------------|-------|
| Allocations in hot path | None |
| Operations per sample | 3 multiplications + 1 subtraction |
| Max time per sample | < 1 µs |

---

## Unit Tests

Located in `src/measurement/scaling.rs`:

| Test | Verifies |
|------|---------|
| `test_adc_to_secondary` | 1000 counts × 0.001 = 1.0 A secondary |
| `test_adc_to_secondary_with_offset` | Offset correction applied |
| `test_secondary_to_primary` | 1 A × 400 ratio = 400 A primary |
| `test_adc_to_primary` | Combined chain |
| `test_current_scaler` | `CurrentScaler` struct end-to-end |
| `test_scale_samples` | Batch conversion |

---

## TODO

- [ ] **Three-phase scaling** — a single `CurrentScaler` handles one phase channel. Three-phase support needs three scalers (or a `ThreePhaseScaler` wrapper).
- [ ] **Voltage scaling** — future distance protection (PDIS) requires voltage measurements using a similar VT ratio + ADC chain.
- [ ] **SCD-driven calibration** — in Phase 2, CT/ADC parameters will be read from the SCD file's `DO` attributes under `TCTR` and `MMXU` logical nodes.

---

## See Also

- [`docs/modules/IO_SV_INPUT.md`](IO_SV_INPUT.md) — source of the raw `current_adc` value
- [`docs/modules/MEASUREMENT_RMS.md`](MEASUREMENT_RMS.md) — consumes the scaled primary Amperes
- [`docs/modules/CONFIG.md`](CONFIG.md) — `AdcConfig` and `CtConfig` JSON layout
