/// Measurement module for RMS calculation and scaling
pub mod rms;
pub mod scaling;

pub use rms::{calculate_rms, calculate_rms_i32, RmsCalculator};
pub use scaling::{
    adc_to_primary, adc_to_secondary, secondary_to_primary, 
    adc_samples_to_primary, adc_samples_to_secondary, CurrentScaler
};
