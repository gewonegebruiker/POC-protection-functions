/// Scaling functions for CT ratio and ADC conversion
use crate::config::{AdcConfig, CtConfig};

/// Convert raw ADC value to secondary current (Amperes)
/// 
/// # Arguments
/// * `adc_value` - Raw ADC reading
/// * `config` - ADC configuration with scale factor and offset
/// 
/// # Returns
/// Current in secondary amperes
pub fn adc_to_secondary(adc_value: i32, config: &AdcConfig) -> f64 {
    (adc_value as f64 - config.offset) * config.scale_factor
}

/// Convert secondary current to primary current using CT ratio
/// 
/// # Arguments
/// * `secondary_current` - Current in secondary amperes
/// * `config` - CT configuration
/// 
/// # Returns
/// Current in primary amperes
pub fn secondary_to_primary(secondary_current: f64, config: &CtConfig) -> f64 {
    secondary_current * config.ratio()
}

/// Convert raw ADC value directly to primary current
/// 
/// # Arguments
/// * `adc_value` - Raw ADC reading
/// * `adc_config` - ADC configuration
/// * `ct_config` - CT configuration
/// 
/// # Returns
/// Current in primary amperes
pub fn adc_to_primary(adc_value: i32, adc_config: &AdcConfig, ct_config: &CtConfig) -> f64 {
    let secondary = adc_to_secondary(adc_value, adc_config);
    secondary_to_primary(secondary, ct_config)
}

/// Convert a slice of raw ADC values to secondary currents
/// 
/// # Arguments
/// * `adc_values` - Slice of raw ADC readings
/// * `config` - ADC configuration
/// 
/// # Returns
/// Vector of currents in secondary amperes
pub fn adc_samples_to_secondary(adc_values: &[i32], config: &AdcConfig) -> Vec<f64> {
    adc_values
        .iter()
        .map(|&val| adc_to_secondary(val, config))
        .collect()
}

/// Convert a slice of raw ADC values to primary currents
/// 
/// # Arguments
/// * `adc_values` - Slice of raw ADC readings
/// * `adc_config` - ADC configuration
/// * `ct_config` - CT configuration
/// 
/// # Returns
/// Vector of currents in primary amperes
pub fn adc_samples_to_primary(
    adc_values: &[i32],
    adc_config: &AdcConfig,
    ct_config: &CtConfig,
) -> Vec<f64> {
    adc_values
        .iter()
        .map(|&val| adc_to_primary(val, adc_config, ct_config))
        .collect()
}

/// Scaler struct that holds configuration for complete scaling chain
pub struct CurrentScaler {
    adc_config: AdcConfig,
    ct_config: CtConfig,
}

impl CurrentScaler {
    /// Create a new current scaler
    pub fn new(adc_config: AdcConfig, ct_config: CtConfig) -> Self {
        Self {
            adc_config,
            ct_config,
        }
    }

    /// Scale raw ADC value to primary current
    pub fn scale_to_primary(&self, adc_value: i32) -> f64 {
        adc_to_primary(adc_value, &self.adc_config, &self.ct_config)
    }

    /// Scale raw ADC samples to primary currents
    pub fn scale_samples_to_primary(&self, adc_values: &[i32]) -> Vec<f64> {
        adc_samples_to_primary(adc_values, &self.adc_config, &self.ct_config)
    }

    /// Get the ADC configuration
    pub fn adc_config(&self) -> &AdcConfig {
        &self.adc_config
    }

    /// Get the CT configuration
    pub fn ct_config(&self) -> &CtConfig {
        &self.ct_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adc_to_secondary() {
        let config = AdcConfig {
            scale_factor: 0.001,
            offset: 0.0,
        };
        
        let secondary = adc_to_secondary(1000, &config);
        assert_eq!(secondary, 1.0);
    }

    #[test]
    fn test_adc_to_secondary_with_offset() {
        let config = AdcConfig {
            scale_factor: 0.001,
            offset: 100.0,
        };
        
        let secondary = adc_to_secondary(1100, &config);
        assert_eq!(secondary, 1.0);
    }

    #[test]
    fn test_secondary_to_primary() {
        let config = CtConfig {
            primary: 400.0,
            secondary: 1.0,
        };
        
        let primary = secondary_to_primary(1.0, &config);
        assert_eq!(primary, 400.0);
    }

    #[test]
    fn test_adc_to_primary() {
        let adc_config = AdcConfig {
            scale_factor: 0.001,
            offset: 0.0,
        };
        let ct_config = CtConfig {
            primary: 400.0,
            secondary: 1.0,
        };
        
        // 1000 ADC counts = 1A secondary = 400A primary
        let primary = adc_to_primary(1000, &adc_config, &ct_config);
        assert_eq!(primary, 400.0);
    }

    #[test]
    fn test_current_scaler() {
        let adc_config = AdcConfig {
            scale_factor: 0.001,
            offset: 0.0,
        };
        let ct_config = CtConfig {
            primary: 400.0,
            secondary: 1.0,
        };
        
        let scaler = CurrentScaler::new(adc_config, ct_config);
        let primary = scaler.scale_to_primary(1000);
        assert_eq!(primary, 400.0);
    }

    #[test]
    fn test_scale_samples() {
        let adc_config = AdcConfig {
            scale_factor: 0.001,
            offset: 0.0,
        };
        let ct_config = CtConfig {
            primary: 400.0,
            secondary: 1.0,
        };
        
        let scaler = CurrentScaler::new(adc_config, ct_config);
        let samples = vec![1000, 2000, 3000];
        let primaries = scaler.scale_samples_to_primary(&samples);
        
        assert_eq!(primaries, vec![400.0, 800.0, 1200.0]);
    }
}
