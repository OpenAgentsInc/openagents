use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityWindow {
    pub protocol_version: String,
    pub min_client_build_id: String,
    pub max_client_build_id: Option<String>,
    pub min_schema_version: u32,
    pub max_schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCompatibilityHandshake {
    pub client_build_id: String,
    pub protocol_version: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilitySurface {
    ControlApi,
    KhalaWebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub upgrade_required: bool,
    pub surface: CompatibilitySurface,
    pub min_client_build_id: String,
    pub max_client_build_id: Option<String>,
    pub min_schema_version: u32,
    pub max_schema_version: u32,
    pub protocol_version: String,
}

impl CompatibilityFailure {
    fn new(
        code: &str,
        message: String,
        retryable: bool,
        upgrade_required: bool,
        surface: CompatibilitySurface,
        window: &CompatibilityWindow,
    ) -> Self {
        Self {
            code: code.to_string(),
            message,
            retryable,
            upgrade_required,
            surface,
            min_client_build_id: window.min_client_build_id.clone(),
            max_client_build_id: window.max_client_build_id.clone(),
            min_schema_version: window.min_schema_version,
            max_schema_version: window.max_schema_version,
            protocol_version: window.protocol_version.clone(),
        }
    }
}

pub fn negotiate_compatibility(
    surface: CompatibilitySurface,
    handshake: &ClientCompatibilityHandshake,
    window: &CompatibilityWindow,
) -> Result<(), CompatibilityFailure> {
    let client_build_id = normalize_build_id(&handshake.client_build_id).ok_or_else(|| {
        CompatibilityFailure::new(
            "invalid_client_build",
            "client_build_id is required".to_string(),
            false,
            true,
            surface,
            window,
        )
    })?;

    if normalize_build_id(&handshake.protocol_version)
        != normalize_build_id(&window.protocol_version)
    {
        return Err(CompatibilityFailure::new(
            "unsupported_protocol_version",
            format!(
                "protocol version '{}' is unsupported; expected '{}'",
                handshake.protocol_version, window.protocol_version
            ),
            false,
            true,
            surface,
            window,
        ));
    }

    if handshake.schema_version < window.min_schema_version
        || handshake.schema_version > window.max_schema_version
    {
        return Err(CompatibilityFailure::new(
            "unsupported_schema_version",
            format!(
                "schema_version {} is outside supported range {}..={}",
                handshake.schema_version, window.min_schema_version, window.max_schema_version
            ),
            false,
            true,
            surface,
            window,
        ));
    }

    if compare_build_ids(&client_build_id, &window.min_client_build_id) < 0 {
        return Err(CompatibilityFailure::new(
            "upgrade_required",
            format!(
                "client build '{}' is older than minimum supported '{}'; upgrade is required",
                client_build_id, window.min_client_build_id
            ),
            false,
            true,
            surface,
            window,
        ));
    }

    if let Some(max_build_id) = window.max_client_build_id.as_deref()
        && compare_build_ids(&client_build_id, max_build_id) > 0
    {
        return Err(CompatibilityFailure::new(
            "unsupported_client_build",
            format!(
                "client build '{}' is newer than supported maximum '{}'",
                client_build_id, max_build_id
            ),
            false,
            true,
            surface,
            window,
        ));
    }

    Ok(())
}

fn normalize_build_id(raw: &str) -> Option<String> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.to_string())
}

fn compare_build_ids(left: &str, right: &str) -> i32 {
    match left.cmp(right) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> CompatibilityWindow {
        CompatibilityWindow {
            protocol_version: "khala.ws.v1".to_string(),
            min_client_build_id: "20260221T120000Z".to_string(),
            max_client_build_id: Some("20260221T180000Z".to_string()),
            min_schema_version: 1,
            max_schema_version: 2,
        }
    }

    #[test]
    fn negotiation_accepts_supported_client() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "20260221T130000Z".to_string(),
            protocol_version: "khala.ws.v1".to_string(),
            schema_version: 1,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::KhalaWebSocket, &handshake, &window());
        assert!(result.is_ok());
    }

    #[test]
    fn negotiation_rejects_missing_build_id() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "   ".to_string(),
            protocol_version: "khala.ws.v1".to_string(),
            schema_version: 1,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::ControlApi, &handshake, &window());
        let error = result.expect_err("missing build should fail");
        assert_eq!(error.code, "invalid_client_build");
    }

    #[test]
    fn negotiation_rejects_protocol_mismatch() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "20260221T130000Z".to_string(),
            protocol_version: "khala.ws.v2".to_string(),
            schema_version: 1,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::ControlApi, &handshake, &window());
        let error = result.expect_err("protocol mismatch should fail");
        assert_eq!(error.code, "unsupported_protocol_version");
        assert!(error.upgrade_required);
    }

    #[test]
    fn negotiation_rejects_schema_outside_window() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "20260221T130000Z".to_string(),
            protocol_version: "khala.ws.v1".to_string(),
            schema_version: 9,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::ControlApi, &handshake, &window());
        let error = result.expect_err("schema mismatch should fail");
        assert_eq!(error.code, "unsupported_schema_version");
    }

    #[test]
    fn negotiation_rejects_client_below_minimum() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "20260221T110000Z".to_string(),
            protocol_version: "khala.ws.v1".to_string(),
            schema_version: 1,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::KhalaWebSocket, &handshake, &window());
        let error = result.expect_err("old client should fail");
        assert_eq!(error.code, "upgrade_required");
        assert!(error.message.contains("minimum supported"));
    }

    #[test]
    fn negotiation_rejects_client_above_maximum() {
        let handshake = ClientCompatibilityHandshake {
            client_build_id: "20260221T190000Z".to_string(),
            protocol_version: "khala.ws.v1".to_string(),
            schema_version: 1,
        };

        let result =
            negotiate_compatibility(CompatibilitySurface::KhalaWebSocket, &handshake, &window());
        let error = result.expect_err("too new client should fail");
        assert_eq!(error.code, "unsupported_client_build");
    }
}
