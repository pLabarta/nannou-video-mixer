use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};

/// A WLED device found on the network (or added manually).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub name: String,
    pub addr: SocketAddr,
    /// From the "wled" service's "mac" TXT record, when available. Stable
    /// across DHCP lease changes, unlike the IP address.
    pub mac: Option<String>,
}

/// Abstracts over how WLED devices are found, so the manager/poller can be
/// tested against a fixed device list without touching the real network.
pub trait DeviceDiscovery {
    fn discover(&self, timeout: Duration) -> Vec<DiscoveredDevice>;
}

/// Finds WLED devices by browsing mDNS for the "_wled._tcp.local." service
/// that `WLED::initInterfaces()` registers (wled00/wled.cpp) on port 80.
pub struct MdnsDiscovery;

const WLED_SERVICE_TYPE: &str = "_wled._tcp.local.";

impl DeviceDiscovery for MdnsDiscovery {
    fn discover(&self, timeout: Duration) -> Vec<DiscoveredDevice> {
        let daemon = match ServiceDaemon::new() {
            Ok(daemon) => daemon,
            Err(_) => return Vec::new(),
        };

        let receiver = match daemon.browse(WLED_SERVICE_TYPE) {
            Ok(receiver) => receiver,
            Err(_) => return Vec::new(),
        };

        let mut devices = Vec::new();
        let deadline = std::time::Instant::now() + timeout;

        while let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let port = info.get_port();
                    let mac = info.get_properties().get("mac").map(|v| v.val_str().to_string());
                    for ip in info.get_addresses() {
                        let addr: IpAddr = *ip;
                        devices.push(DiscoveredDevice {
                            name: info.get_fullname().to_string(),
                            addr: SocketAddr::new(addr, port),
                            mac: mac.clone(),
                        });
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        let _ = daemon.shutdown();
        devices
    }
}
