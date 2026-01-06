# POC Protection Functions

IEC 61850 compliant protection functions implemented in Rust for power system protection applications.

## Overview

This project implements protection functions according to the IEC 61850 standard, with support for:
- **Sampled Values (SV)** input for current measurements (IEC 61850-9-2)
- **GOOSE** messaging for trip signal output (IEC 61850-8-1)
- Configurable scaling for CT ratios and ADC conversion
- Compatible with Omicron test equipment

### Currently Implemented

- **PTOC (Time Overcurrent Protection)** - Definite time characteristic
  - Configurable pickup current (Iset)
  - Configurable time delay (Tset)
  - RMS calculation from 80 samples per cycle (50 Hz)

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ Sampled     │────▶│  Protection  │────▶│   GOOSE     │
│ Values (SV) │     │  Function    │     │   Output    │
│             │     │   (PTOC)     │     │   (Trip)    │
└─────────────┘     └──────────────┘     └─────────────┘
      │                     │                    │
      │                     │                    │
   ADC + CT              RMS Calc            IEC 61850-8-1
   Scaling            Definite Time          Encoding
```

### Data Flow

1. **SV Input**: Receive 80 samples per cycle (4000 samples/sec at 50 Hz)
2. **Scaling**: Apply ADC scaling and CT ratio conversion
3. **RMS Calculation**: Calculate RMS current over one cycle
4. **Protection Logic**: Compare against pickup setting with time delay
5. **GOOSE Output**: Send trip signal when threshold exceeded

## Building and Running

### Prerequisites

- Rust 1.70 or later
- Linux operating system (for raw socket support)
- **CAP_NET_RAW capability or root privileges** (required for live SV/GOOSE network I/O)

### Build

```bash
cargo build --release
```

### Run Example Application

```bash
cargo run --release
```

This runs a simulation that demonstrates the PTOC function with overcurrent detection.

**Note**: The example runs in simulation mode. For live network operation with actual SV/GOOSE packets, you need root privileges or CAP_NET_RAW capability.

### Run Test Example

```bash
cargo run --example ptoc_test
```

This runs a simple test showing PTOC behavior with different current levels.

### Run Tests

```bash
cargo test
```

## Live Network I/O

The implementation now includes **full IEC 61850 network integration** using `iec_61850_lib` with raw socket support:

### SV Subscriber (Receiving Sampled Values)

```rust
use poc_protection_functions::{SvSubscriber, SvConfig};

let config = SvConfig {
    samples_per_cycle: 80,
    interface: "eth0".to_string(),
    multicast_mac: "01:0C:CD:04:00:00".to_string(),
};

let mut subscriber = SvSubscriber::new(config);
subscriber.init()?;  // Requires CAP_NET_RAW

// Receive samples (non-blocking)
match subscriber.receive_sample() {
    Ok(sample) => {
        println!("Current ADC: {}", sample.current_adc);
        // Process sample...
    }
    Err(e) => println!("No data: {}", e),
}
```

### GOOSE Publisher (Sending Trip Signals)

```rust
use poc_protection_functions::{GoosePublisher, GooseConfig};

let config = GooseConfig {
    dst_mac: "01:0C:CD:01:00:00".to_string(),
    appid: 0x0001,
    goid: "PTOC_TRIP".to_string(),
    gocb_ref: "IED1LD0/LLN0$GO$PTOC1".to_string(),
    dat_set: "IED1LD0/LLN0$PTOC1".to_string(),
    interface: "eth0".to_string(),
};

let mut publisher = GoosePublisher::new(config);
publisher.init()?;  // Requires CAP_NET_RAW

// Send trip message
let timestamp = get_timestamp_micros();
publisher.publish_trip(true, timestamp)?;  // Sends actual GOOSE frame
```

### Privileges Required

Raw socket operations require elevated privileges:

**Option 1: Run as root**
```bash
sudo cargo run --release
```

**Option 2: Grant CAP_NET_RAW capability**
```bash
# Build first
cargo build --release

# Grant capability to binary
sudo setcap cap_net_raw+ep target/release/poc_ptoc

# Now can run without sudo
./target/release/poc_ptoc
```

### Network Setup

For live operation:
1. Connect IED/test equipment to same Ethernet network
2. Configure correct network interface (`eth0`, `enp0s3`, etc.)
3. Set multicast MAC addresses to match your equipment:
   - SV typically uses `01:0C:CD:04:XX:XX`
   - GOOSE typically uses `01:0C:CD:01:XX:XX`
4. Ensure no firewall blocks raw Ethernet frames

## Configuration

The system is configured through `SystemConfig` which includes:

### PTOC Configuration

```rust
PtocConfig {
    iset: 100.0,      // Pickup current in primary Amperes
    tset: 100,        // Definite time delay in milliseconds
    enabled: true,    // Enable/disable the function
}
```

### CT (Current Transformer) Configuration

```rust
CtConfig {
    primary: 400.0,   // Primary current rating (e.g., 400A)
    secondary: 1.0,   // Secondary current rating (typically 1A or 5A)
}
```

The CT ratio is automatically calculated as `primary / secondary` (e.g., 400/1 = 400).

### ADC (Analog-to-Digital Converter) Configuration

```rust
AdcConfig {
    scale_factor: 0.001,  // Converts ADC counts to secondary amperes
    offset: 0.0,          // Zero point correction
}
```

### GOOSE Output Configuration

```rust
GooseConfig {
    dst_mac: "01:0C:CD:01:00:00".to_string(),  // Multicast MAC address
    appid: 0x0001,                              // Application ID
    goid: "PTOC_TRIP".to_string(),              // GOOSE ID
    gocb_ref: "IED1LD0/LLN0$GO$PTOC1".to_string(),  // Control block reference
    dat_set: "IED1LD0/LLN0$PTOC1".to_string(),  // Dataset reference
    interface: "eth0".to_string(),              // Network interface
}
```

### Sampled Values Input Configuration

```rust
SvConfig {
    samples_per_cycle: 80,                          // 80 samples @ 50Hz = 4000 samples/sec
    interface: "eth0".to_string(),                  // Network interface
    multicast_mac: "01:0C:CD:04:00:00".to_string(), // SV multicast address
}
```

### Configuration File

You can save and load configuration from JSON:

```rust
// Save configuration
let config = SystemConfig::default();
config.to_json_file("config.json")?;

// Load configuration
let config = SystemConfig::from_json_file("config.json")?;
```

Example `config.json`:

```json
{
  "ptoc": {
    "iset": 100.0,
    "tset": 100,
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
    "dst_mac": "01:0C:CD:01:00:00",
    "appid": 1,
    "goid": "PTOC_TRIP",
    "gocb_ref": "IED1LD0/LLN0$GO$PTOC1",
    "dat_set": "IED1LD0/LLN0$PTOC1",
    "interface": "eth0"
  },
  "sv": {
    "samples_per_cycle": 80,
    "interface": "eth0",
    "multicast_mac": "01:0C:CD:04:00:00"
  }
}
```

## Usage with Omicron Test Equipment

### Test Setup

1. **Configure Omicron** to output Sampled Values (SV):
   - Frequency: 50 Hz
   - Sample rate: 4000 samples/second (80 samples/cycle)
   - Current amplitude: Configure based on test requirements

2. **Configure GOOSE Subscriber** in Omicron:
   - Subscribe to the configured GOOSE multicast address
   - Monitor for trip signal from the PTOC function

3. **Network Setup**:
   - Connect Omicron and protection device to same Ethernet network
   - Use dedicated network interface for IEC 61850 traffic
   - Configure MAC addresses and APPID to match Omicron settings

### Example Test Scenario

**Test Definite Time Overcurrent:**

1. Set PTOC parameters: Iset = 100A, Tset = 100ms
2. Apply normal load current (< 100A) - verify no trip
3. Apply overcurrent (> 100A) - verify trip after 100ms
4. Reduce current below 100A before 100ms - verify no trip
5. Apply sustained overcurrent - verify trip persists

## Project Structure

```
POC-protection-functions/
├── Cargo.toml                  # Project dependencies
├── README.md                   # This file
├── .github/
│   └── copilot-instructions.md # Copilot context
├── src/
│   ├── lib.rs                  # Library root
│   ├── main.rs                 # Example application
│   ├── config.rs               # Configuration structures
│   ├── protection/
│   │   ├── mod.rs
│   │   ├── traits.rs           # ProtectionFunction trait
│   │   └── ptoc.rs             # PTOC implementation
│   ├── measurement/
│   │   ├── mod.rs
│   │   ├── rms.rs              # RMS calculation
│   │   └── scaling.rs          # CT ratio, ADC scaling
│   └── io/
│       ├── mod.rs
│       ├── sv_input.rs         # SV subscriber
│       └── goose_output.rs     # GOOSE publisher
└── examples/
    └── ptoc_test.rs            # Simple test setup
```

## Key Modules

### Protection Functions (`src/protection/`)

- **traits.rs**: Defines the `ProtectionFunction` trait that all protection functions implement
- **ptoc.rs**: Time Overcurrent Protection with definite time characteristic

### Measurement (`src/measurement/`)

- **rms.rs**: RMS calculation from sampled values (supports 80 samples per cycle)
- **scaling.rs**: Current scaling (ADC → secondary → primary conversion)

### I/O (`src/io/`)

- **sv_input.rs**: Sampled Values subscriber (uses `iec_61850_lib`)
- **goose_output.rs**: GOOSE publisher for trip signals (uses `iec_61850_lib`)

## IEC 61850 Compliance

### Logical Nodes

- **PTOC**: Time overcurrent (implemented)
- **XCBR**: Circuit breaker (future)
- **PDIF**: Differential protection (future)
- **PDIS**: Distance protection (future)

### Communication

- **IEC 61850-9-2 (Sampled Values)**:
  - Receives current measurements over Ethernet
  - 80 samples per cycle at 50 Hz (4000 samples/second)
  - Multicast communication

- **IEC 61850-8-1 (GOOSE)**:
  - Sends trip signals over Ethernet
  - Fast transmission (< 4ms)
  - State-based messaging with sequence numbers
  - Compatible with standard IED test equipment

## Future Roadmap

### Near Term
- [ ] Complete integration with `iec_61850_lib` for actual SV/GOOSE communication
- [ ] Add PTOC inverse time curves (IEC 255, IEEE C37.112)
- [ ] Add configuration validation and error handling
- [ ] Add detailed logging and diagnostics

### Medium Term
- [ ] **PDIF**: Differential protection function
- [ ] **PDIS**: Distance protection function
- [ ] **XCBR**: Circuit breaker logical node
- [ ] Web-based configuration interface
- [ ] Real-time monitoring and visualization

### Long Term
- [ ] Multiple protection zones
- [ ] Breaker failure protection
- [ ] Fault recording
- [ ] Integration with SCADA systems
- [ ] IEC 61850 MMS server for configuration

## Dependencies

- **iec_61850_lib**: IEC 61850 protocol implementation for GOOSE and SV
- **serde/serde_json**: Configuration serialization
- **log/env_logger**: Logging infrastructure

## License

Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## References

- IEC 61850-8-1: Communication networks and systems for power utility automation - Part 8-1: Specific communication service mapping (SCSM) - Mappings to MMS and to ISO/IEC 8802-3
- IEC 61850-9-2: Communication networks and systems for power utility automation - Part 9-2: Specific communication service mapping (SCSM) - Sampled values over ISO/IEC 8802-3
- IEEE C37.112: IEEE Standard for Inverse-Time Characteristic Equations for Overcurrent Relays
