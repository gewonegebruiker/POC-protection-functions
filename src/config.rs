/// Configuration structures for protection functions and I/O
use serde::{Deserialize, Serialize};

/// IEC/IEEE inverse-time overcurrent curve type.
///
/// When set to anything other than `DefiniteTime`, the effective trip delay is
/// `tset × k / ((I/Iset)^α − 1)` where k and α are curve-dependent constants.
/// `tset` acts as the **time multiplier setting (TMS)** in milliseconds.
///
/// Reference: IEC 60255-151, IEEE C37.112.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum PtocCurve {
    /// Fixed time delay equal to `tset` (default).
    #[default]
    DefiniteTime,
    /// IEC Standard Inverse  — k = 0.14, α = 0.02
    IecStandardInverse,
    /// IEC Very Inverse      — k = 13.5, α = 1.0
    IecVeryInverse,
    /// IEC Extremely Inverse — k = 80,   α = 2.0
    IecExtremelyInverse,
}

/// Configuration for PTOC (Time Overcurrent Protection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtocConfig {
    /// Pickup current in primary Amperes
    pub iset: f64,
    /// Definite time delay in milliseconds (also acts as TMS for inverse-time curves)
    pub tset: u64,
    /// Enable/disable the protection function
    pub enabled: bool,
    /// Dropout ratio — current must fall below `iset × dropout_ratio` to reset from
    /// Pickup back to Idle.  Values < 1.0 add hysteresis (e.g. 0.95).
    /// Default 0.95.
    #[serde(default = "PtocConfig::default_dropout_ratio")]
    pub dropout_ratio: f64,
    /// Inverse-time curve selection. Default: `DefiniteTime`.
    #[serde(default)]
    pub curve: PtocCurve,
}

impl PtocConfig {
    fn default_dropout_ratio() -> f64 {
        0.95
    }
}

impl Default for PtocConfig {
    fn default() -> Self {
        Self {
            iset: 100.0,
            tset: 100,
            enabled: true,
            dropout_ratio: 0.95,
            curve: PtocCurve::DefiniteTime,
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

/// Selects how the input current value is interpreted by PIOC.
///
/// - `Instantaneous`: the caller supplies the raw instantaneous sample (absolute value).
///   `iset` must be set as a **peak** threshold (i.e. `iset_rms × √2`).
/// - `ShortWindowRms(n)`: PIOC maintains an internal n-sample sliding-window RMS.
///   `iset` is then an RMS threshold, reducing noise sensitivity at the cost of a few
///   sample periods of additional detection latency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PiocInputMode {
    /// Use the instantaneous absolute sample value directly (default).
    Instantaneous,
    /// Compute an n-sample sliding RMS internally before comparing to `iset`.
    ShortWindowRms(usize),
}

impl Default for PiocInputMode {
    fn default() -> Self {
        PiocInputMode::Instantaneous
    }
}

/// Configuration for PIOC (Instantaneous Overcurrent Protection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiocConfig {
    /// Pickup current in primary Amperes.
    /// For `Instantaneous` mode this is a **peak** threshold; for `ShortWindowRms` it is RMS.
    pub iset: f64,
    /// Enable/disable the protection function
    pub enabled: bool,
    /// How the input value is interpreted. Default: `Instantaneous`.
    #[serde(default)]
    pub input_mode: PiocInputMode,
}

impl Default for PiocConfig {
    fn default() -> Self {
        Self {
            iset: 500.0,
            enabled: true,
            input_mode: PiocInputMode::Instantaneous,
        }
    }
}

/// Complete system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub ptoc: PtocConfig,
    pub pioc: PiocConfig,
    pub ct: CtConfig,
    pub adc: AdcConfig,
    pub goose: GooseConfig,
    pub sv: SvConfig,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            ptoc: PtocConfig::default(),
            pioc: PiocConfig::default(),
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
