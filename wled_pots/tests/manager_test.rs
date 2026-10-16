mod support;

use std::time::Duration;

use support::fixtures::PINS_WITH_ONE_POT;
use support::mock_wled::MockWled;
use wled_pots::{DeviceDiscovery, DiscoveredDevice, WledManager};

/// Fixed device list, standing in for real mDNS discovery (which can't be
/// exercised reliably in a sandboxed/CI environment).
struct FakeDiscovery {
    devices: Vec<DiscoveredDevice>,
}

impl DeviceDiscovery for FakeDiscovery {
    fn discover(&self, _timeout: Duration) -> Vec<DiscoveredDevice> {
        self.devices.clone()
    }
}

#[test]
fn discover_and_track_starts_polling_discovered_devices() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let discovery = FakeDiscovery {
        devices: vec![DiscoveredDevice {
            name: "wled-test".to_string(),
            addr: mock.addr,
            mac: Some("aabbccddeeff".to_string()),
        }],
    };

    let manager = WledManager::new(Box::new(discovery), Duration::from_millis(15));
    manager.discover_and_track(Duration::from_millis(10));

    assert_eq!(manager.devices(), vec![mock.addr]);
    assert_eq!(manager.device_name(mock.addr), Some("wled-test".to_string()));
    assert_eq!(manager.device_mac(mock.addr), Some("aabbccddeeff".to_string()));

    std::thread::sleep(Duration::from_millis(60));
    let reading = manager.reading(mock.addr).expect("device should have a reading");
    assert_eq!(reading.pins[0].raw, Some(100));
}

#[test]
fn rediscovering_the_same_device_does_not_duplicate_it() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let discovery = FakeDiscovery {
        devices: vec![DiscoveredDevice {
            name: "wled-test".to_string(),
            addr: mock.addr,
            mac: None,
        }],
    };

    let manager = WledManager::new(Box::new(discovery), Duration::from_millis(50));
    manager.discover_and_track(Duration::from_millis(10));
    manager.discover_and_track(Duration::from_millis(10));

    assert_eq!(manager.devices().len(), 1);
}

#[test]
fn add_manual_tracks_a_device_without_discovery() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let discovery = FakeDiscovery { devices: vec![] };
    let manager = WledManager::new(Box::new(discovery), Duration::from_millis(15));

    manager.add_manual(mock.addr);

    assert_eq!(manager.devices(), vec![mock.addr]);
    std::thread::sleep(Duration::from_millis(60));
    assert!(manager.reading(mock.addr).unwrap().last_success.is_some());
}

#[test]
fn remove_stops_tracking_a_device() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let discovery = FakeDiscovery { devices: vec![] };
    let manager = WledManager::new(Box::new(discovery), Duration::from_millis(15));
    manager.add_manual(mock.addr);

    manager.remove(mock.addr);

    assert!(manager.devices().is_empty());
    assert!(manager.reading(mock.addr).is_none());
}

#[test]
fn set_poll_interval_applies_to_tracked_devices() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let discovery = FakeDiscovery { devices: vec![] };
    let manager = WledManager::new(Box::new(discovery), Duration::from_millis(200));
    manager.add_manual(mock.addr);

    std::thread::sleep(Duration::from_millis(50));
    let slow_count = mock.request_count();

    manager.set_poll_interval(Duration::from_millis(10));
    std::thread::sleep(Duration::from_millis(150));
    let fast_count = mock.request_count();

    assert!(
        fast_count - slow_count >= 5,
        "expected cadence to speed up: slow={slow_count} fast={fast_count}"
    );
}
