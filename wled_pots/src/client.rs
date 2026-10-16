use std::net::SocketAddr;

use crate::error::WledError;
use crate::model::PinsResponse;

/// Blocking HTTP client for one WLED device's `/json/pins` endpoint.
pub struct WledClient {
    base_url: String,
}

impl WledClient {
    pub fn new(addr: SocketAddr) -> Self {
        WledClient {
            base_url: format!("http://{addr}"),
        }
    }

    /// For tests/mocks that aren't addressable as a `SocketAddr` (e.g. a
    /// `tiny_http` server bound to `127.0.0.1:0` reports its real port
    /// separately), callers can build a client directly from a base URL.
    pub fn from_base_url(base_url: impl Into<String>) -> Self {
        WledClient {
            base_url: base_url.into(),
        }
    }

    pub fn fetch_pins(&self) -> Result<PinsResponse, WledError> {
        let url = format!("{}/json/pins", self.base_url);
        let response = ureq::get(&url)
            .call()
            .map_err(|err| WledError::Http(err.to_string()))?;

        let body = response
            .into_string()
            .map_err(|err| WledError::Http(err.to_string()))?;

        Ok(PinsResponse::parse(&body)?)
    }
}
