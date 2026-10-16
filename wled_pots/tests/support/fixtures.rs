pub const PINS_WITH_ONE_POT: &str = r#"{
    "pins": [
        {"p": 32, "c": 1, "a": true, "o": 2, "n": "Button", "m": 0, "t": 7, "s": 0, "r": 100}
    ]
}"#;

pub fn pins_with_pot_value(value: u16) -> String {
    format!(
        r#"{{"pins": [{{"p": 32, "c": 1, "a": true, "o": 2, "n": "Button", "m": 0, "t": 7, "s": 0, "r": {value}}}]}}"#
    )
}
