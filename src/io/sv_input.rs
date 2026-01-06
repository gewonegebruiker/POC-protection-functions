/// Sampled Values (SV) input handling using iec_61850_lib
use crate::config::SvConfig;
use std::error::Error;

/// Sample data structure representing one sample from SV stream
#[derive(Debug, Clone)]
pub struct SampleData {
    /// Raw ADC value for current
    pub current_adc: i32,
    /// Sample number within the cycle
    pub sample_number: u16,
    /// Timestamp in microseconds
    pub timestamp: u64,
}

/// SV subscriber that receives sampled values from the network
pub struct SvSubscriber {
    config: SvConfig,
}

impl SvSubscriber {
    /// Create a new SV subscriber with the given configuration
    pub fn new(config: SvConfig) -> Self {
        Self { config }
    }

    /// Initialize the subscriber (placeholder for actual implementation)
    /// 
    /// This would use iec_61850_lib to set up the network listener
    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!(
            "Initializing SV subscriber on interface {} (MAC: {})",
            self.config.interface,
            self.config.multicast_mac
        );
        
        // TODO: Implement actual SV subscription using iec_61850_lib
        // This would involve:
        // 1. Opening a raw socket on the specified interface
        // 2. Subscribing to the multicast MAC address
        // 3. Setting up the packet decoder
        
        Ok(())
    }

    /// Receive the next sample from the SV stream (placeholder)
    /// 
    /// This would use iec_61850_lib to decode incoming SV packets
    pub fn receive_sample(&mut self) -> Result<SampleData, Box<dyn Error>> {
        // TODO: Implement actual SV packet reception and decoding
        // This would involve:
        // 1. Receiving raw Ethernet frame
        // 2. Decoding IEC 61850-9-2 SV packet
        // 3. Extracting sample data (current, voltage, timestamp)
        
        // Placeholder implementation
        Err("SV reception not yet implemented".into())
    }

    /// Get the configuration
    pub fn config(&self) -> &SvConfig {
        &self.config
    }

    /// Get the expected samples per cycle
    pub fn samples_per_cycle(&self) -> usize {
        self.config.samples_per_cycle
    }
}

/// SV sample buffer that accumulates samples for one cycle
pub struct SvSampleBuffer {
    samples: Vec<i32>,
    capacity: usize,
    current_index: usize,
}

impl SvSampleBuffer {
    /// Create a new sample buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
            current_index: 0,
        }
    }

    /// Add a sample to the buffer
    /// 
    /// This buffer uses a circular/ring buffer approach:
    /// - Initially fills up to capacity by pushing samples
    /// - Once full, overwrites oldest samples in a circular manner
    /// - Samples are stored in insertion order, not sorted by time
    pub fn add_sample(&mut self, sample: i32) {
        if self.samples.len() < self.capacity {
            self.samples.push(sample);
        } else {
            self.samples[self.current_index] = sample;
        }
        self.current_index = (self.current_index + 1) % self.capacity;
    }

    /// Check if the buffer is full (has one complete cycle)
    pub fn is_full(&self) -> bool {
        self.samples.len() == self.capacity
    }

    /// Get all samples in the buffer
    pub fn samples(&self) -> &[i32] {
        &self.samples
    }

    /// Get the number of samples in the buffer
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.samples.clear();
        self.current_index = 0;
    }

    /// Get the capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_buffer_creation() {
        let buffer = SvSampleBuffer::new(80);
        assert_eq!(buffer.capacity(), 80);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }

    #[test]
    fn test_sample_buffer_fill() {
        let mut buffer = SvSampleBuffer::new(80);
        
        for i in 0..80 {
            buffer.add_sample(i);
        }
        
        assert_eq!(buffer.len(), 80);
        assert!(buffer.is_full());
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_sample_buffer_overflow() {
        let mut buffer = SvSampleBuffer::new(4);
        
        // Add more samples than capacity
        for i in 0..10 {
            buffer.add_sample(i);
        }
        
        // Should only keep 4 samples (circular buffer)
        assert_eq!(buffer.len(), 4);
        assert!(buffer.is_full());
        
        // The buffer is circular, so samples are ordered based on insertion
        // After 10 insertions with capacity 4, current_index will be at 2 (10 % 4)
        // The buffer will have: [8, 9, 6, 7] (indices 0, 1, 2, 3)
        let samples = buffer.samples();
        assert_eq!(samples[0], 8);
        assert_eq!(samples[1], 9);
        assert_eq!(samples[2], 6);
        assert_eq!(samples[3], 7);
    }
}
