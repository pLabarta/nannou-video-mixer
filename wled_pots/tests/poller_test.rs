mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::fixtures::{pins_with_pot_value, PINS_WITH_ONE_POT};
use support::mock_wled::MockWled;
use wled_pots::{DevicePoller, WledClient};

#[test]
fn polls_repeatedly_at_the_configured_interval() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let client = WledClient::new(mock.addr);
    let interval = Arc::new(Mutex::new(Duration::from_millis(20)));

    let poller = DevicePoller::spawn(client, interval);

    std::thread::sleep(Duration::from_millis(220));

    // ~11 polls expected at 20ms; generous bounds to avoid flakiness.
    let count = mock.request_count();
    assert!(count >= 4, "expected several polls, got {count}");

    let reading = poller.reading();
    assert_eq!(reading.pins[0].raw, Some(100));
    assert!(reading.last_success.is_some());
    assert!(reading.last_error.is_none());
}

#[test]
fn reading_reflects_the_latest_mock_response() {
    let mock = MockWled::start(&pins_with_pot_value(10));
    let client = WledClient::new(mock.addr);
    let interval = Arc::new(Mutex::new(Duration::from_millis(15)));
    let poller = DevicePoller::spawn(client, interval);

    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(poller.reading().pins[0].raw, Some(10));

    mock.set_body(&pins_with_pot_value(999));
    std::thread::sleep(Duration::from_millis(80));

    assert_eq!(poller.reading().pins[0].raw, Some(999));
}

#[test]
fn changing_the_shared_interval_changes_poll_cadence() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let client = WledClient::new(mock.addr);
    let interval = Arc::new(Mutex::new(Duration::from_millis(200)));

    let _poller = DevicePoller::spawn(client, interval.clone());

    std::thread::sleep(Duration::from_millis(50));
    let slow_count = mock.request_count();

    *interval.lock().unwrap() = Duration::from_millis(10);
    std::thread::sleep(Duration::from_millis(150));
    let fast_count = mock.request_count();

    assert!(
        fast_count - slow_count >= 5,
        "expected cadence to speed up after lowering the interval: slow={slow_count} fast={fast_count}"
    );
}

#[test]
fn network_errors_keep_the_last_good_reading_without_crashing() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let client = WledClient::new(mock.addr);
    let interval = Arc::new(Mutex::new(Duration::from_millis(15)));
    let poller = DevicePoller::spawn(client, interval);

    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(poller.reading().pins[0].raw, Some(100));

    mock.set_status_500(true);
    std::thread::sleep(Duration::from_millis(60));

    let reading = poller.reading();
    assert_eq!(reading.pins[0].raw, Some(100), "last good reading should be retained");
    assert!(reading.last_error.is_some());
}

#[test]
fn dropping_the_poller_stops_polling() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let client = WledClient::new(mock.addr);
    let interval = Arc::new(Mutex::new(Duration::from_millis(15)));

    let poller = DevicePoller::spawn(client, interval);
    std::thread::sleep(Duration::from_millis(60));
    drop(poller);

    let count_after_drop = mock.request_count();
    std::thread::sleep(Duration::from_millis(100));
    let count_later = mock.request_count();

    assert_eq!(
        count_after_drop, count_later,
        "no further requests should arrive after the poller is dropped"
    );
}
