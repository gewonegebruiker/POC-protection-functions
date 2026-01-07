/// GOOSE output handling using iec_61850_lib
use crate::config::GooseConfig;
use std::error::Error;
use socket2::{Socket, Domain, Type, Protocol};
use iec_61850_lib::encode_goose::encode_goose;
use iec_61850_lib::types::{EthernetHeader, IECGoosePdu, IECData};

#[cfg(target_os = "linux")]
use super::network_utils::{get_interface_index, bind_to_interface, get_interface_mac, DEFAULT_SRC_MAC, SOCKADDR_LL_SIZE};

/// GOOSE trip message data
#[derive(Debug, Clone)]
pub struct GooseTripMessage {
    /// Trip signal (true = trip, false = normal)
    pub trip: bool,
    /// Sequence number for GOOSE messages
    pub sq_num: u32,
    /// State number (increments on data change)
    pub st_num: u32,
    /// Timestamp in microseconds
    pub timestamp: u64,
}

/// GOOSE publisher that sends trip messages over the network
pub struct GoosePublisher {
    config: GooseConfig,
    sq_num: u32,
    st_num: u32,
    last_trip_state: bool,
    socket: Option<Socket>,
    src_mac: [u8; 6],
}

impl GoosePublisher {
    /// Create a new GOOSE publisher with the given configuration
    pub fn new(config: GooseConfig) -> Self {
        Self {
            config,
            sq_num: 0,
            st_num: 0,
            last_trip_state: false,
            socket: None,
            src_mac: DEFAULT_SRC_MAC,
        }
    }

    /// Initialize the publisher with actual raw socket
    /// 
    /// This opens a raw Ethernet socket to send GOOSE packets
    /// Requires CAP_NET_RAW capability or root privileges on Linux
    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!(
            "Initializing GOOSE publisher on interface {} (MAC: {}, APPID: 0x{:04X})",
            self.config.interface,
            self.config.dst_mac,
            self.config.appid
        );
        
        #[cfg(target_os = "linux")]
        {
            
            
            // Create raw packet socket
            let socket = Socket::new(
                Domain::PACKET,
                Type::RAW,
                Some(Protocol::from(0x0003)), // ETH_P_ALL
            )?;
            
            // Bind to interface
            let if_index = get_interface_index(&self.config.interface)?;
            let mut addr_storage = [0u8; 128];
            bind_to_interface(&socket, if_index, &mut addr_storage)?;
            
            // Get MAC address of the interface
            self.src_mac = get_interface_mac(&self.config.interface)?;
            
            log::info!("GOOSE publisher initialized on interface {} (MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X})",
                      self.config.interface,
                      self.src_mac[0], self.src_mac[1], self.src_mac[2],
                      self.src_mac[3], self.src_mac[4], self.src_mac[5]);
            
            self.socket = Some(socket);
            Ok(())
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Err("Raw socket GOOSE transmission is only supported on Linux".into())
        }
    }

    /// Publish a trip message using iec_61850_lib encoding
    /// 
    /// This encodes and sends an actual GOOSE message over the network
    pub fn publish_trip(&mut self, trip: bool, timestamp: u64) -> Result<(), Box<dyn Error>> {
        // Increment sequence number
        self.sq_num = self.sq_num.wrapping_add(1);
        
        // Increment state number if trip state changed
        let state_changed = trip != self.last_trip_state;
        if state_changed {
            self.st_num += 1;
            self.last_trip_state = trip;
            
            log::info!(
                "GOOSE trip state changed: {} (stNum: {}, sqNum: {})",
                if trip { "TRIP" } else { "NORMAL" },
                self.st_num,
                self.sq_num
            );
        }

        // Parse destination MAC address
        let dst_mac = parse_mac_address(&self.config.dst_mac)?;
        
        // Create Ethernet header
        let eth_header = EthernetHeader {
            dst_addr: dst_mac,
            src_addr: self.src_mac,
            tpid: None,
            tci: None,
            ether_type: [0x88, 0xB8], // GOOSE EtherType
            appid: self.config.appid.to_be_bytes(),
            length: [0x00, 0x00], // Will be set by encode_goose
        };
        
        // Convert timestamp to IEC 61850 format
        let timestamp_bytes = timestamp_to_iec61850(timestamp);
        let t = iec_61850_lib::types::Timestamp::from_bytes(timestamp_bytes);
        
        // Create GOOSE PDU
        let pdu = IECGoosePdu {
            go_cb_ref: self.config.gocb_ref.clone(),
            time_allowed_to_live: if state_changed { 2000 } else { 5000 }, // Faster updates on state change
            dat_set: self.config.dat_set.clone(),
            go_id: self.config.goid.clone(),
            t,
            st_num: self.st_num,
            sq_num: self.sq_num,
            simulation: false,
            conf_rev: 1,
            nds_com: false,
            num_dat_set_entries: 1,
            all_data: vec![IECData::Boolean(trip)],
        };

        // Encode GOOSE message
        let frame = encode_goose(&eth_header, &pdu)
            .map_err(|e| format!("Failed to encode GOOSE: {:?}", e))?;

        // Send frame if socket is initialized
        if let Some(socket) = &self.socket {
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::io::AsRawFd;
                
                let if_index = get_interface_index(&self.config.interface)?;
                
                // Create sockaddr_ll for sending
                let mut addr_storage = [0u8; 128];
                let mut offset = 0;
                
                // sll_family (AF_PACKET = 17)
                addr_storage[offset..offset+2].copy_from_slice(&17u16.to_ne_bytes());
                offset += 2;
                
                // sll_protocol (ETH_P_ALL = 0x0003 in network byte order)
                addr_storage[offset..offset+2].copy_from_slice(&0x0300u16.to_be_bytes());
                offset += 2;
                
                // sll_ifindex
                addr_storage[offset..offset+4].copy_from_slice(&(if_index as i32).to_ne_bytes());
                offset += 4;
                
                // sll_hatype and sll_pkttype (not used for sending, set to 0)
                addr_storage[offset..offset+4].copy_from_slice(&[0u8; 4]);
                offset += 4;
                
                // sll_addr (destination MAC)
                addr_storage[offset..offset+6].copy_from_slice(&dst_mac);
                
                let addr_len = SOCKADDR_LL_SIZE;
                
                // Send packet
                let ret = unsafe {
                    libc::sendto(
                        socket.as_raw_fd(),
                        frame.as_ptr() as *const libc::c_void,
                        frame.len(),
                        0,
                        addr_storage.as_ptr() as *const libc::sockaddr,
                        addr_len as u32,
                    )
                };
                
                if ret < 0 {
                    return Err("Failed to send GOOSE frame".into());
                }
                
                log::debug!(
                    "Sent GOOSE frame: {} bytes, trip={}, sqNum={}, stNum={}",
                    frame.len(),
                    trip,
                    self.sq_num,
                    self.st_num
                );
            }
        } else {
            log::debug!(
                "GOOSE frame encoded but not sent (socket not initialized): trip={}, sqNum={}, stNum={}",
                trip,
                self.sq_num,
                self.st_num
            );
        }

        Ok(())
    }

    /// Get the configuration
    pub fn config(&self) -> &GooseConfig {
        &self.config
    }

    /// Get the current sequence number
    pub fn sq_num(&self) -> u32 {
        self.sq_num
    }

    /// Get the current state number
    pub fn st_num(&self) -> u32 {
        self.st_num
    }

    /// Get the last trip state
    pub fn last_trip_state(&self) -> bool {
        self.last_trip_state
    }

    /// Reset the publisher state
    pub fn reset(&mut self) {
        self.sq_num = 0;
        self.st_num = 0;
        self.last_trip_state = false;
    }
}

/// Parse MAC address from string format "XX:XX:XX:XX:XX:XX"
fn parse_mac_address(mac_str: &str) -> Result<[u8; 6], Box<dyn Error>> {
    let parts: Vec<&str> = mac_str.split(':').collect();
    if parts.len() != 6 {
        return Err(format!("Invalid MAC address format: {}", mac_str).into());
    }
    
    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16)
            .map_err(|_| format!("Invalid MAC address byte: {}", part))?;
    }
    
    Ok(mac)
}

/// Convert microseconds timestamp to IEC 61850 8-byte timestamp
fn timestamp_to_iec61850(timestamp_us: u64) -> [u8; 8] {
    // Convert microseconds to seconds and fraction
    let seconds = (timestamp_us / 1_000_000) as u32;
    let remaining_us = (timestamp_us % 1_000_000) as u32;
    
    // Convert microseconds to 24-bit fraction
    // fraction = (microseconds * 2^24) / 1_000_000
    let fraction = ((remaining_us as u64 * 16_777_216) / 1_000_000) as u32;
    
    let mut bytes = [0u8; 8];
    bytes[0..4].copy_from_slice(&seconds.to_be_bytes());
    let fraction_bytes = fraction.to_be_bytes();
    bytes[4] = fraction_bytes[1];
    bytes[5] = fraction_bytes[2];
    bytes[6] = fraction_bytes[3];
    bytes[7] = 0x00; // Quality: good, synchronized
    
    bytes
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goose_publisher_creation() {
        let config = GooseConfig::default();
        let publisher = GoosePublisher::new(config);
        
        assert_eq!(publisher.sq_num(), 0);
        assert_eq!(publisher.st_num(), 0);
        assert_eq!(publisher.last_trip_state(), false);
    }

    #[test]
    fn test_goose_state_change() {
        let config = GooseConfig::default();
        let mut publisher = GoosePublisher::new(config);
        
        // First trip message (without socket, just tests state management)
        publisher.publish_trip(true, 1000).unwrap();
        assert_eq!(publisher.st_num(), 1);
        assert_eq!(publisher.sq_num(), 1);
        
        // Same state - st_num should not increment
        publisher.publish_trip(true, 2000).unwrap();
        assert_eq!(publisher.st_num(), 1);
        assert_eq!(publisher.sq_num(), 2);
        
        // State change - st_num should increment
        publisher.publish_trip(false, 3000).unwrap();
        assert_eq!(publisher.st_num(), 2);
        assert_eq!(publisher.sq_num(), 3);
    }

    #[test]
    fn test_goose_reset() {
        let config = GooseConfig::default();
        let mut publisher = GoosePublisher::new(config);
        
        publisher.publish_trip(true, 1000).unwrap();
        publisher.reset();
        
        assert_eq!(publisher.sq_num(), 0);
        assert_eq!(publisher.st_num(), 0);
        assert_eq!(publisher.last_trip_state(), false);
    }
    
    #[test]
    fn test_mac_address_parsing() {
        let mac = parse_mac_address("01:0C:CD:01:00:00").unwrap();
        assert_eq!(mac, [0x01, 0x0C, 0xCD, 0x01, 0x00, 0x00]);
        
        let mac2 = parse_mac_address("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(mac2, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        
        // Test invalid format
        assert!(parse_mac_address("invalid").is_err());
        assert!(parse_mac_address("AA:BB:CC:DD:EE").is_err());
    }
    
    #[test]
    fn test_timestamp_conversion() {
        // Test converting 1 second in microseconds
        let bytes = timestamp_to_iec61850(1_000_000);
        let seconds = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(seconds, 1);
        
        // Test converting 1.5 seconds
        let bytes = timestamp_to_iec61850(1_500_000);
        let seconds = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(seconds, 1);
        
        // Fraction should be approximately half of 2^24
        let fraction = u32::from_be_bytes([0, bytes[4], bytes[5], bytes[6]]);
        // 0.5 seconds = (0.5 * 2^24) / 1 = 8388608
        assert!((fraction as i32 - 8_388_608).abs() < 100); // Allow small tolerance
    }
}
