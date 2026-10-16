mod support;

use support::fixtures::PINS_WITH_ONE_POT;
use support::mock_wled::MockWled;
use wled_pots::WledClient;

#[test]
fn fetches_and_parses_pins_from_mock_server() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    let client = WledClient::new(mock.addr);

    let pins = client.fetch_pins().expect("fetch_pins should succeed");

    assert_eq!(pins.pins.len(), 1);
    assert_eq!(pins.pins[0].raw, Some(100));
    assert!(pins.pins[0].is_potentiometer());
}

#[test]
fn surfaces_http_error_on_500_response() {
    let mock = MockWled::start(PINS_WITH_ONE_POT);
    mock.set_status_500(true);
    let client = WledClient::new(mock.addr);

    let result = client.fetch_pins();

    assert!(result.is_err());
}

#[test]
fn surfaces_json_error_on_malformed_body() {
    let mock = MockWled::start("not json at all");
    let client = WledClient::new(mock.addr);

    let result = client.fetch_pins();

    assert!(result.is_err());
}

#[test]
fn surfaces_error_when_server_is_unreachable() {
    // Nothing is listening on this address.
    let client = WledClient::new("127.0.0.1:1".parse().unwrap());

    let result = client.fetch_pins();

    assert!(result.is_err());
}
