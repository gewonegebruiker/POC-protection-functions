/// POC Protection Functions - IEC 61850 compliant protection functions in Rust
/// 
/// This library implements protection functions according to IEC 61850 standard,
/// including PTOC (Time Overcurrent Protection) with support for Sampled Values (SV)
/// input and GOOSE trip output.

pub mod config;
pub mod measurement;
pub mod protection;
pub mod io;

pub use config::{
    SystemConfig, PtocConfig, CtConfig, AdcConfig, GooseConfig, SvConfig,
};

pub use measurement::{
    calculate_rms, calculate_rms_i32, RmsCalculator,
    adc_to_primary, adc_to_secondary, secondary_to_primary, CurrentScaler,
};

pub use protection::{
    ProtectionFunction, ProtectionResult, TripState, Ptoc,
};

pub use io::{
    SampleData, SvSubscriber, SvSampleBuffer,
    GooseTripMessage, GoosePublisher,
};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
    }
}
