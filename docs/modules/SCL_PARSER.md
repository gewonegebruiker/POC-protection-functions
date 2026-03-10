# Module: SCL / SCD Parser

**Source files**:
- [`src/scl/parser.rs`](../../src/scl/parser.rs) — `SclParser` with `parse_for_ied()` and `to_system_config()`
- [`src/scl/types.rs`](../../src/scl/types.rs) — SCL data model types

---

## Purpose

The SCL module parses an IEC 61850 **SCD (Substation Configuration Description)** file and converts the relevant IED settings into the runtime `SystemConfig` used by the protection functions and I/O modules.

In Phase 1, configuration comes from a hand-written JSON file. In **Phase 2**, the SCD file will be the single source of truth for all bay settings.

---

## What Must Be Parsed from an SCD File

An SCD file is an XML document structured as:

```xml
<SCL>
  <Header id="…" version="…" revision="…"/>
  <Communication>
    <SubNetwork name="ProcessBus" type="8-MMS">
      <ConnectedAP iedName="BAY1IED" apName="P1">
        <GSE ldInst="PROT" cbName="GCB_PTOC">
          <Address>
            <P type="MAC-Address">01-0C-CD-01-00-01</P>
            <P type="APPID">0x0001</P>
            <P type="VLAN-ID">…</P>
          </Address>
          <MinTime multiplier="m" unit="s">2</MinTime>
          <MaxTime multiplier="m" unit="s">1000</MaxTime>
        </GSE>
        <SMV ldInst="PROT" cbName="MSVCB01">
          <Address>
            <P type="MAC-Address">01-0C-CD-04-00-01</P>
            <P type="APPID">0x4001</P>
          </Address>
        </SMV>
      </ConnectedAP>
    </SubNetwork>
  </Communication>
  <IED name="BAY1IED" type="PROTECTION" manufacturer="…">
    <AccessPoint name="P1">
      <Server>
        <LDevice inst="PROT">
          <LN lnClass="PTOC" inst="1" …>
            <DOI name="StrVal">
              <DAI name="setMag">
                <Val>100.0</Val>      <!-- iset: pickup current -->
              </DAI>
            </DOI>
            <DOI name="OpDlTmms">
              <DAI name="setVal">
                <Val>100</Val>        <!-- tset: time delay in ms -->
              </DAI>
            </DOI>
          </LN>
          <LN lnClass="PIOC" inst="1" …>
            <DOI name="StrVal">
              <DAI name="setMag">
                <Val>1200.0</Val>     <!-- pioc iset -->
              </DAI>
            </DOI>
          </LN>
        </LDevice>
      </Server>
    </AccessPoint>
  </IED>
</SCL>
```

### Minimum Required Extractions

| SCD Section | Target field | Rust config field |
|-------------|-------------|------------------|
| `PTOC/DOI[@name='StrVal']/DAI[@name='setMag']/Val` | PTOC pickup | `PtocConfig.iset` |
| `PTOC/DOI[@name='OpDlTmms']/DAI[@name='setVal']/Val` | PTOC delay | `PtocConfig.tset` |
| `PIOC/DOI[@name='StrVal']/DAI[@name='setMag']/Val` | PIOC pickup | `PiocConfig.iset` |
| `Communication/GSE/Address[@type='MAC-Address']` | GOOSE MAC | `GooseConfig.dst_mac` |
| `Communication/GSE/Address[@type='APPID']` | GOOSE APPID | `GooseConfig.appid` |
| `Communication/SMV/Address[@type='MAC-Address']` | SV MAC | `SvConfig.multicast_mac` |

---

## Current Implementation Status

### `SclParser::to_system_config(ied_config: &IedConfig) -> SystemConfig`

✅ **Fully implemented.** Converts a pre-built `IedConfig` (from SCL or manually constructed) into `SystemConfig`. Already used in unit tests.

```rust
let mut ied_config = IedConfig::default();
ied_config.protection_functions.push(
    ProtectionFunctionConfig::Ptoc(PtocConfig { iset: 200.0, tset: 50, enabled: true })
);
let sys_config = SclParser::to_system_config(&ied_config);
```

### `SclParser::parse_for_ied(scd_path: &str, ied_name: &str) -> ParseResult<IedConfig>`

❌ **Not yet implemented** — returns `todo!()`. The function signature and return type are defined; the XML parsing body is deferred until an SCD file is available.

---

## Type Definitions (`src/scl/types.rs`)

The full SCD data model is defined in `types.rs`:

| Type | Description |
|------|-------------|
| `ScdFile` | Top-level SCD document |
| `SclHeader` | `<Header>` metadata |
| `Communication` | `<Communication>` section with sub-networks |
| `SubNetwork` | A process bus or station bus sub-network |
| `ConnectedAP` | IED access point on a sub-network (holds GSE and SMV addresses) |
| `GseAddress` | MAC, APPID, VLAN, timing for a GOOSE control block |
| `SmvAddress` | MAC, APPID, VLAN for an SV control block |
| `IedDefinition` | `<IED>` element with access points |
| `LogicalDevice` | `<LDevice>` with logical nodes |
| `LogicalNode` | `<LN>` with data objects |
| `DataObject` | `<DOI>` with data attributes |
| `DataAttribute` | `<DAI>` name/value pair |
| `IedConfig` | Application-level derived config (protection functions + SV/GOOSE) |
| `ProtectionFunctionConfig` | Enum: `Ptoc(PtocConfig)` or `Pioc(PiocConfig)` |

---

## Mapping: SCL → `SystemConfig`

The `to_system_config` bridge applies these rules:

1. The first `ProtectionFunctionConfig::Ptoc(…)` found in `ied_config.protection_functions` is used; defaults apply if none found.
2. The first `ProtectionFunctionConfig::Pioc(…)` found is used; defaults apply if none found.
3. The first `sv_subscriptions` entry is mapped to `SvConfig` (interface, multicast MAC, samples/cycle).
4. The first `goose_publications` entry is mapped to `GooseConfig` (dst_mac, appid, interface).
5. CT and ADC config is taken directly from `ied_config.ct` and `ied_config.adc`.

---

## Unit Tests

Located in `src/scl/parser.rs`:

| Test | Verifies |
|------|---------|
| `test_to_system_config_defaults` | Empty `IedConfig` → defaults for PTOC and PIOC |
| `test_to_system_config_with_ptoc` | PTOC settings correctly mapped |
| `test_to_system_config_with_pioc` | PIOC settings correctly mapped |
| `test_to_system_config_sv_mapping` | SV subscription → `SvConfig` |
| `test_to_system_config_goose_mapping` | GOOSE publication → `GooseConfig` |

---

## TODO

- [ ] **XML parsing** (`parse_for_ied`) — implement using an XML parser (e.g., `quick-xml` crate). Extract IED section, LN data attributes, and `Communication` section GSE/SMV addresses.
- [ ] **IEC 61850-6 validation** — verify that referenced LN classes, data objects, and attributes exist in the IEC 61850-7-4 data model.
- [ ] **Multiple PTOC/PIOC instances** — handle `PTOC1`, `PTOC2`, etc. per IED; select by instance number or zone.
- [ ] **DataSet parsing** — extract dataset entries to know which signals are published in each GOOSE frame.
- [ ] **VLAN parameters** — map `VLAN-ID` and `VLAN-PRIORITY` from `GseAddress` / `SmvAddress` to config.
- [ ] **SCD schema validation** — validate against the IEC 61850-6 XSD before parsing.

---

## See Also

- [`docs/modules/CONFIG.md`](CONFIG.md) — the `SystemConfig` that `to_system_config` produces
- [`docs/ROADMAP.md`](../ROADMAP.md) — Phase 2 SCD-driven configuration milestone
- IEC 61850-6: Configuration language for communication in electrical substations (SCL schema definition)
