/// Configuration structures for protection functions and I/O
use serde::{Deserialize, Serialize};

/// Configuration for PTOC (Time Overcurrent Protection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtocConfig {
    /// Pickup current in primary Amperes
    pub iset: f64,
    /// Definite time delay in milliseconds
    pub tset: u64,
    /// Enable/disable the protection function
    pub enabled: bool,
}

impl Default for PtocConfig {
    fn default() -> Self {
        Self {
            iset: 100.0,  // 100A default pickup
            tset: 100,    // 100ms default delay
            enabled: true,
        }
    }
}

/// Configuration for CT (Current Transformer) scaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtConfig {
    /// Primary current rating (e.g., 400 for 400/1 CT)
    pub primary: f64,
    /// Secondary current rating (typically 1 or 5)
    pub secondary: f64,
}

impl Default for CtConfig {
    fn default() -> Self {
        Self {
            primary: 400.0,
            secondary: 1.0,
        }
    }
}

impl CtConfig {
    /// Get the CT ratio (primary/secondary)
    pub fn ratio(&self) -> f64 {
        self.primary / self.secondary
    }
}

/// Configuration for ADC (Analog-to-Digital Converter) scaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcConfig {
    /// ADC scaling factor (converts ADC counts to secondary amperes)
    pub scale_factor: f64,
    /// ADC offset (zero point correction)
    pub offset: f64,
}

impl Default for AdcConfig {
    fn default() -> Self {
        Self {
            scale_factor: 0.001,  // 1 mA per count as default
            offset: 0.0,
        }
    }
}

/// Configuration for GOOSE output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooseConfig {
    /// Destination MAC address (format: "01:0C:CD:01:00:00")
    pub dst_mac: String,
    /// Application ID
    pub appid: u16,
    /// GOOSE ID (identifies the GOOSE message)
    pub goid: String,
    /// GOOSE Control Block Reference
    pub gocb_ref: String,
    /// Dataset reference
    pub dat_set: String,
    /// Network interface name (e.g., "eth0")
    pub interface: String,
}

impl Default for GooseConfig {
    fn default() -> Self {
        Self {
            dst_mac: "01:0C:CD:01:00:00".to_string(),
            appid: 0x0001,
            goid: "PTOC_TRIP".to_string(),
            gocb_ref: "IED1LD0/LLN0$GO$PTOC1".to_string(),
            dat_set: "IED1LD0/LLN0$PTOC1".to_string(),
            interface: "eth0".to_string(),
        }
    }
}

/// Configuration for Sampled Values input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvConfig {
    /// Number of samples per cycle (80 for 50Hz @ 4000 samples/sec)
    pub samples_per_cycle: usize,
    /// Network interface name (e.g., "eth0")
    pub interface: String,
    /// Multicast MAC address to subscribe to
    pub multicast_mac: String,
}

impl Default for SvConfig {
    fn default() -> Self {
        Self {
            samples_per_cycle: 80,
            interface: "eth0".to_string(),
            multicast_mac: "01:0C:CD:04:00:00".to_string(),
        }
    }
}

/// Complete system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub ptoc: PtocConfig,
    pub ct: CtConfig,
    pub adc: AdcConfig,
    pub goose: GooseConfig,
    pub sv: SvConfig,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            ptoc: PtocConfig::default(),
            ct: CtConfig::default(),
            adc: AdcConfig::default(),
            goose: GooseConfig::default(),
            sv: SvConfig::default(),
        }
    }
}

impl SystemConfig {
    /// Load configuration from JSON file
    pub fn from_json_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to JSON file
    pub fn to_json_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
