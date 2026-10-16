use std::time::Duration;

use wled_pots::{DeviceDiscovery, MdnsDiscovery};

/// Real mDNS discovery can't be exercised in a sandboxed/CI environment
/// (no multicast, no actual WLED hardware). Run this manually on a LAN with
/// at least one real WLED device via `cargo test -- --ignored`.
#[test]
#[ignore]
fn discovers_real_wled_devices_on_the_lan() {
    let devices = MdnsDiscovery.discover(Duration::from_secs(3));

    assert!(!devices.is_empty(), "expected at least one WLED device on the LAN");
    for device in &devices {
        println!("found {} at {}", device.name, device.addr);
    }
}
