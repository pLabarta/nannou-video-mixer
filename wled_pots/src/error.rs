use std::fmt;

#[derive(Debug)]
pub enum WledError {
    Http(String),
    Json(String),
}

impl fmt::Display for WledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WledError::Http(msg) => write!(f, "WLED HTTP error: {msg}"),
            WledError::Json(msg) => write!(f, "WLED JSON error: {msg}"),
        }
    }
}

impl std::error::Error for WledError {}

impl From<serde_json::Error> for WledError {
    fn from(err: serde_json::Error) -> Self {
        WledError::Json(err.to_string())
    }
}
