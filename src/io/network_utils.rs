/// Network utility functions for raw socket operations on Linux
#[cfg(target_os = "linux")]
use socket2::Socket;
#[cfg(target_os = "linux")]
use std::error::Error;
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

/// Minimum Ethernet frame size (header + minimal payload)
pub const MIN_ETHERNET_FRAME_SIZE: usize = 22;

/// Maximum Ethernet frame size for standard frames
pub const MAX_ETHERNET_FRAME_SIZE: usize = 2048;

/// Size of sockaddr_ll structure
pub const SOCKADDR_LL_SIZE: usize = 20;

/// Default source MAC address (should be replaced with actual interface MAC)
pub const DEFAULT_SRC_MAC: [u8; 6] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05];

#[cfg(target_os = "linux")]
pub fn get_interface_index(interface: &str) -> Result<u32, Box<dyn Error>> {
    use std::ffi::CString;
    
    let c_interface = CString::new(interface)?;
    let index = unsafe { libc::if_nametoindex(c_interface.as_ptr()) };
    
    if index == 0 {
        Err(format!("Interface '{}' not found", interface).into())
    } else {
        Ok(index)
    }
}

#[cfg(target_os = "linux")]
pub fn bind_to_interface(socket: &Socket, if_index: u32, addr_storage: &mut [u8]) -> Result<usize, Box<dyn Error>> {
    let mut offset = 0;
    
    // sll_family (AF_PACKET = 17)
    addr_storage[offset..offset+2].copy_from_slice(&17u16.to_ne_bytes());
    offset += 2;
    
    // sll_protocol (ETH_P_ALL = 0x0003 in network byte order)
    addr_storage[offset..offset+2].copy_from_slice(&0x0300u16.to_be_bytes());
    offset += 2;
    
    // sll_ifindex
    addr_storage[offset..offset+4].copy_from_slice(&(if_index as i32).to_ne_bytes());
    
    let addr_len = SOCKADDR_LL_SIZE;
    
    // Bind socket
    let ret = unsafe {
        libc::bind(
            socket.as_raw_fd(),
            addr_storage.as_ptr() as *const libc::sockaddr,
            addr_len as u32,
        )
    };
    
    if ret < 0 {
        Err("Failed to bind socket to interface".into())
    } else {
        Ok(addr_len)
    }
}

#[cfg(target_os = "linux")]
pub fn get_interface_mac(interface: &str) -> Result<[u8; 6], Box<dyn Error>> {
    use std::ffi::CString;
    use std::mem;
    
    let c_interface = CString::new(interface)?;
    
    // Create a socket for ioctl
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err("Failed to create socket for ioctl".into());
    }
    
    // Prepare ifreq structure
    let mut ifr: libc::ifreq = unsafe { mem::zeroed() };
    let name_bytes = c_interface.as_bytes_with_nul();
    let copy_len = std::cmp::min(name_bytes.len(), libc::IFNAMSIZ);
    unsafe {
        std::ptr::copy_nonoverlapping(
            name_bytes.as_ptr(),
            ifr.ifr_name.as_mut_ptr() as *mut u8,
            copy_len,
        );
    }
    
    // Get hardware address
    let ret = unsafe { libc::ioctl(fd, libc::SIOCGIFHWADDR, &mut ifr) };
    unsafe { libc::close(fd) };
    
    if ret < 0 {
        return Err(format!("Failed to get MAC address for interface '{}'", interface).into());
    }
    
    // Extract MAC address from ifr_hwaddr.sa_data
    let mut mac = [0u8; 6];
    unsafe {
        let sa_data = &ifr.ifr_ifru.ifru_hwaddr.sa_data;
        for i in 0..6 {
            mac[i] = sa_data[i] as u8;
        }
    }
    
    Ok(mac)
}
