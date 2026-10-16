use serde::Deserialize;

// Mirrors wled00/const.h BTN_TYPE_* constants: the firmware reports a
// potentiometer wired to an analog pin as a "button" of one of these types.
pub const BTN_TYPE_TOUCH: u8 = 6;
pub const BTN_TYPE_ANALOG: u8 = 7;
pub const BTN_TYPE_ANALOG_INVERTED: u8 = 8;

/// One entry of WLED's `/json/pins` response (`serializePins()` in
/// wled00/json.cpp). Field names match the firmware's short JSON keys.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Pin {
    #[serde(rename = "p")]
    pub gpio: u8,
    #[serde(rename = "c")]
    pub capabilities: u16,
    #[serde(rename = "a")]
    pub allocated: bool,
    #[serde(rename = "o")]
    pub owner_id: Option<u8>,
    #[serde(rename = "n")]
    pub owner_name: Option<String>,
    #[serde(rename = "m")]
    pub mode: Option<u8>,
    #[serde(rename = "t")]
    pub button_type: Option<u8>,
    #[serde(rename = "s")]
    pub state: Option<i32>,
    #[serde(rename = "r")]
    pub raw: Option<u16>,
}

impl Pin {
    /// True for pins configured as an "Analog" or "Analog inverted" button,
    /// i.e. a potentiometer wired to this GPIO.
    pub fn is_potentiometer(&self) -> bool {
        matches!(
            self.button_type,
            Some(BTN_TYPE_ANALOG) | Some(BTN_TYPE_ANALOG_INVERTED)
        )
    }

    /// True for any pin that reports a live raw reading (touch or analog).
    pub fn is_readable(&self) -> bool {
        self.raw.is_some()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct PinsResponse {
    pub pins: Vec<Pin>,
}

impl PinsResponse {
    pub fn parse(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
    }

    /// Every pin reporting a live raw reading (touch + analog).
    pub fn readable_pins(&self) -> Vec<&Pin> {
        self.pins.iter().filter(|p| p.is_readable()).collect()
    }

    /// Pins specifically configured as potentiometers (analog / analog inverted).
    pub fn potentiometers(&self) -> Vec<&Pin> {
        self.pins.iter().filter(|p| p.is_potentiometer()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "pins": [
            {"p": 2, "c": 0, "a": false},
            {"p": 16, "c": 0, "a": true, "o": 10, "n": "LED", "m": 1, "s": 1},
            {"p": 0, "c": 0, "a": true, "o": 2, "n": "Button", "m": 0, "t": 2, "s": 0},
            {"p": 4, "c": 1, "a": true, "o": 2, "n": "Button", "m": 0, "t": 6, "s": 1, "r": 55},
            {"p": 32, "c": 1, "a": true, "o": 2, "n": "Button", "m": 0, "t": 7, "s": 0, "r": 812},
            {"p": 33, "c": 1, "a": true, "o": 2, "n": "Button", "m": 0, "t": 8, "s": 1, "r": 44}
        ]
    }"#;

    #[test]
    fn parses_all_pins() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        assert_eq!(resp.pins.len(), 6);
    }

    #[test]
    fn unallocated_pin_has_no_owner_or_reading() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let pin = &resp.pins[0];
        assert!(!pin.allocated);
        assert_eq!(pin.owner_name, None);
        assert_eq!(pin.raw, None);
        assert!(!pin.is_readable());
        assert!(!pin.is_potentiometer());
    }

    #[test]
    fn led_pin_is_not_readable_or_a_potentiometer() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let pin = &resp.pins[1];
        assert_eq!(pin.owner_name.as_deref(), Some("LED"));
        assert!(!pin.is_readable());
        assert!(!pin.is_potentiometer());
    }

    #[test]
    fn push_button_has_no_raw_reading() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let pin = &resp.pins[2];
        assert_eq!(pin.button_type, Some(2));
        assert!(!pin.is_readable());
        assert!(!pin.is_potentiometer());
    }

    #[test]
    fn touch_pin_is_readable_but_not_a_potentiometer() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let pin = &resp.pins[3];
        assert_eq!(pin.raw, Some(55));
        assert!(pin.is_readable());
        assert!(!pin.is_potentiometer());
    }

    #[test]
    fn analog_and_analog_inverted_are_potentiometers() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let analog = &resp.pins[4];
        let inverted = &resp.pins[5];
        assert!(analog.is_potentiometer());
        assert!(inverted.is_potentiometer());
        assert_eq!(analog.raw, Some(812));
        assert_eq!(inverted.raw, Some(44));
    }

    #[test]
    fn readable_pins_includes_touch_and_analog_only() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let readable = resp.readable_pins();
        assert_eq!(readable.len(), 3);
        assert!(readable.iter().all(|p| p.raw.is_some()));
    }

    #[test]
    fn potentiometers_excludes_touch() {
        let resp = PinsResponse::parse(FIXTURE).unwrap();
        let pots = resp.potentiometers();
        assert_eq!(pots.len(), 2);
        assert!(pots.iter().all(|p| p.is_potentiometer()));
    }

    #[test]
    fn rejects_malformed_json() {
        let result = PinsResponse::parse("{ not json");
        assert!(result.is_err());
    }
}
