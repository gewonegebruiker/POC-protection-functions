/// RMS (Root Mean Square) calculation from sampled values

/// Calculate RMS value from a slice of samples
/// 
/// # Arguments
/// * `samples` - Slice of sample values
/// 
/// # Returns
/// RMS value calculated as sqrt(sum(x^2) / n)
pub fn calculate_rms(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_of_squares: f64 = samples.iter().map(|&x| x * x).sum();
    let mean_square = sum_of_squares / samples.len() as f64;
    mean_square.sqrt()
}

/// Calculate RMS value from integer samples (raw ADC values)
/// 
/// # Arguments
/// * `samples` - Slice of integer sample values
/// 
/// # Returns
/// RMS value calculated from integer samples
pub fn calculate_rms_i32(samples: &[i32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_of_squares: f64 = samples.iter().map(|&x| (x as f64) * (x as f64)).sum();
    let mean_square = sum_of_squares / samples.len() as f64;
    mean_square.sqrt()
}

/// RMS calculator that accumulates samples over a window
pub struct RmsCalculator {
    samples: Vec<f64>,
    window_size: usize,
    current_index: usize,
}

impl RmsCalculator {
    /// Create a new RMS calculator with the specified window size
    /// 
    /// # Arguments
    /// * `window_size` - Number of samples to use for RMS calculation (e.g., 80 for one cycle)
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: vec![0.0; window_size],
            window_size,
            current_index: 0,
        }
    }

    /// Add a new sample to the calculator
    /// 
    /// # Arguments
    /// * `sample` - New sample value
    pub fn add_sample(&mut self, sample: f64) {
        self.samples[self.current_index] = sample;
        self.current_index = (self.current_index + 1) % self.window_size;
    }

    /// Calculate the current RMS value from accumulated samples
    pub fn calculate(&self) -> f64 {
        calculate_rms(&self.samples)
    }

    /// Check if the calculator has received a full window of samples
    pub fn is_full(&self) -> bool {
        self.current_index == 0 && self.samples.iter().any(|&x| x != 0.0)
    }

    /// Get the number of samples in the window
    pub fn window_size(&self) -> usize {
        self.window_size
    }

    /// Reset the calculator
    pub fn reset(&mut self) {
        self.samples.fill(0.0);
        self.current_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, SQRT_2};

    #[test]
    fn test_calculate_rms_dc() {
        let samples = vec![10.0; 100];
        let rms = calculate_rms(&samples);
        assert!((rms - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_rms_sine_wave() {
        // Generate one cycle of a sine wave with amplitude 1.0
        let samples: Vec<f64> = (0..80)
            .map(|i| (2.0 * PI * i as f64 / 80.0).sin())
            .collect();
        
        let rms = calculate_rms(&samples);
        // RMS of sine wave should be amplitude / sqrt(2) = 1.0 / sqrt(2) ≈ 0.707
        assert!((rms - 1.0 / SQRT_2).abs() < 0.01);
    }

    #[test]
    fn test_calculate_rms_empty() {
        let samples: Vec<f64> = vec![];
        let rms = calculate_rms(&samples);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn test_rms_calculator() {
        let mut calc = RmsCalculator::new(80);
        
        // Add 80 samples with value 10.0
        for _ in 0..80 {
            calc.add_sample(10.0);
        }
        
        let rms = calc.calculate();
        assert!((rms - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_rms_calculator_sine() {
        let mut calc = RmsCalculator::new(80);
        
        // Add one cycle of sine wave
        for i in 0..80 {
            let sample = (2.0 * PI * i as f64 / 80.0).sin();
            calc.add_sample(sample);
        }
        
        let rms = calc.calculate();
        assert!((rms - 1.0 / SQRT_2).abs() < 0.01);
    }
}
