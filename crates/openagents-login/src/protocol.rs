//! Device-grant results shared by the CLI and external SDK.

use serde_json::Value;

/// A device endpoint's error, apart from transport and presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceError {
    Pending,
    SlowDown(u64),
    Denied,
    Expired,
    InvalidGrant,
    Other,
}

/// Interpret the existing OAuth error shape without retaining server text.
#[must_use]
pub fn device_error(body: &Value, previous_interval: u64) -> DeviceError {
    match body["error"].as_str() {
        Some("authorization_pending") => DeviceError::Pending,
        Some("slow_down") => DeviceError::SlowDown(
            body["interval"]
                .as_u64()
                .unwrap_or(previous_interval.saturating_add(5)),
        ),
        Some("access_denied") => DeviceError::Denied,
        Some("expired_token") => DeviceError::Expired,
        Some("invalid_grant") => DeviceError::InvalidGrant,
        _ => DeviceError::Other,
    }
}
