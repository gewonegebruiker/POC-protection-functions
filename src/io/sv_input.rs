/// Sampled Values (SV) input handling using iec_61850_lib
use crate::config::SvConfig;
use std::error::Error;
use socket2::{Socket, Domain, Type, Protocol};
use iec_61850_lib::decode_basics::decode_ethernet_header;
use iec_61850_lib::decode_smv::decode_smv;
use iec_61850_lib::types::EthernetHeader;

#[cfg(target_os = "linux")]
use super::network_utils::{get_interface_index, bind_to_interface, MAX_ETHERNET_FRAME_SIZE, MIN_ETHERNET_FRAME_SIZE};

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
    socket: Option<Socket>,
}

impl SvSubscriber {
    /// Create a new SV subscriber with the given configuration
    pub fn new(config: SvConfig) -> Self {
        Self { 
            config,
            socket: None,
        }
    }

    /// Initialize the subscriber with actual raw socket
    /// 
    /// This opens a raw Ethernet socket to receive SV packets
    /// Requires CAP_NET_RAW capability or root privileges on Linux
    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!(
            "Initializing SV subscriber on interface {} (MAC: {})",
            self.config.interface,
            self.config.multicast_mac
        );
        
        // Create raw socket for Ethernet (AF_PACKET on Linux)
        #[cfg(target_os = "linux")]
        {
            
            
            
            // Create raw packet socket (ETH_P_ALL = 0x0003 to receive all protocols)
            let socket = Socket::new(
                Domain::PACKET,
                Type::RAW,
                Some(Protocol::from(0x0003)), // ETH_P_ALL
            )?;
            
            // Set socket to non-blocking mode
            socket.set_nonblocking(true)?;
            
            // Bind to specific interface
            let if_index = get_interface_index(&self.config.interface)?;
            
            // Create sockaddr_ll structure for binding
            let mut addr_storage = [0u8; 128];
            let _addr_len = bind_to_interface(&socket, if_index, &mut addr_storage)?;
            
            log::info!("Raw socket created and bound to interface {} (index: {})", 
                       self.config.interface, if_index);
            
            self.socket = Some(socket);
            Ok(())
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Err("Raw socket SV reception is only supported on Linux".into())
        }
    }

    /// Receive the next sample from the SV stream
    /// 
    /// This receives and decodes actual IEC 61850-9-2 SV packets from the network
    /// Returns the first current sample from the first ASDU
    pub fn receive_sample(&mut self) -> Result<SampleData, Box<dyn Error>> {
        let socket = self.socket.as_ref()
            .ok_or("Socket not initialized. Call init() first.")?;
        
        // Buffer for receiving Ethernet frame
        let mut buffer = vec![0u8; MAX_ETHERNET_FRAME_SIZE];
        let mut recv_buf: Vec<std::mem::MaybeUninit<u8>> = vec![std::mem::MaybeUninit::uninit(); MAX_ETHERNET_FRAME_SIZE];
        
        loop {
            // Receive packet (non-blocking)
            let (len, _) = match socket.recv_from(&mut recv_buf) {
                Ok((n, addr)) => (n, addr),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err("No data available (non-blocking mode)".into());
                }
                Err(e) => return Err(Box::new(e)),
            };
            
            // Copy to initialized buffer
            for i in 0..len {
                buffer[i] = unsafe { recv_buf[i].assume_init() };
            }
            
            if len < MIN_ETHERNET_FRAME_SIZE {
                // Too small to be a valid Ethernet frame
                continue;
            }
            
            // Decode Ethernet header
            let mut eth_header = EthernetHeader::default();
            let pos = decode_ethernet_header(&mut eth_header, &buffer[0..len]);
            
            // Check if this is an SV packet (EtherType 0x88BA)
            if eth_header.ether_type != [0x88, 0xBA] {
                continue;
            }
            
            // Decode SMV PDU
            let pdu = match decode_smv(&buffer[0..len], pos) {
                Ok(p) => p,
                Err(e) => {
                    log::debug!("Failed to decode SMV PDU: {:?}", e);
                    continue;
                }
            };
            
            // Extract sample data from first ASDU
            if let Some(asdu) = pdu.sav_asdu.first() {
                if let Some(sample) = asdu.all_data.first() {
                    // Get current timestamp in microseconds
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;
                    
                    return Ok(SampleData {
                        current_adc: sample.value,
                        sample_number: asdu.smp_cnt,
                        timestamp,
                    });
                }
            }
            
            log::debug!("Received SV packet but no samples found");
        }
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
