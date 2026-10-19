//! Workspace lifecycle types shared by the engine and its clients.

use serde::{Deserialize, Serialize};

/// Protocol features are advertised explicitly because personal/integration
/// builds may share a semver with upstream while exposing a different RPC and
/// document surface.
pub mod capabilities {
    /// The host decodes durable composer references at the harness boundary.
    pub const COMPOSER_REFERENCES_V1: &str = "composer-references-v1";
    pub const MESSAGE_QUEUE_V1: &str = "message-queue-v1";
    pub const MESSAGE_QUEUE_ACTIONS_V1: &str = "message-queue-actions-v1";
    pub const MESSAGE_QUEUE_ATTACHMENTS_V1: &str = "message-queue-attachments-v1";
    pub const MESSAGE_QUEUE_CLEAN_ATTACHMENT_TEXT_V1: &str =
        "message-queue-clean-attachment-text-v1";
    pub const MESSAGE_QUEUE_EDIT_LEASE_V1: &str = "message-queue-edit-lease-v1";
    pub const HARNESS_UPDATES_V1: &str = "harness-updates-v1";
    /// The host creates a projectless chat's scratch folder
    /// (`crate::projectless`) when it runs; older hosts only know `~`.
    pub const PROJECTLESS_SCRATCH_V1: &str = "projectless-scratch-v1";
    pub const OPENAGENTS_CONTINUATIONS_V1: &str = "openagents-continuations-v1";
    pub const CHAT_HOME_TRANSFER_V1: &str = "chat-home-transfer-v1";

    pub const CURRENT: &[&str] = &[
        COMPOSER_REFERENCES_V1,
        MESSAGE_QUEUE_V1,
        MESSAGE_QUEUE_ACTIONS_V1,
        MESSAGE_QUEUE_ATTACHMENTS_V1,
        MESSAGE_QUEUE_CLEAN_ATTACHMENT_TEXT_V1,
        MESSAGE_QUEUE_EDIT_LEASE_V1,
        HARNESS_UPDATES_V1,
        PROJECTLESS_SCRATCH_V1,
        OPENAGENTS_CONTINUATIONS_V1,
        CHAT_HOME_TRANSFER_V1,
        crate::voice::remote::CAPABILITY,
    ];

    pub fn current() -> Vec<String> {
        CURRENT.iter().map(|value| (*value).to_string()).collect()
    }
}

/// The fixed data boundary selected when an engine runtime is assembled.
///
/// Authentication can change while a runtime is alive, but its workspace scope
/// cannot. Switching scopes requires assembling a new runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceScope {
    Local,
    Synced,
    Development,
}

/// Stable information about the engine runtime reached by a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub device_id: String,
    pub workspace_scope: WorkspaceScope,
    /// SDK selected by the owning engine, absent on older versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_sdk_version: Option<String>,
    /// Supported protocol/document features. Missing on older engines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// The program the engine runs. Missing on older engines (#12179).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<EngineBuild>,
}

/// Which program file an engine or app runs, as it was when the process
/// started. Two processes run the same build when these match; an app that
/// finds its background engine on another build replaces it (#12179).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineBuild {
    /// The package version.
    pub version: String,
    /// The program's path.
    pub exe: String,
    /// The program file's size in bytes.
    pub size: u64,
    /// The program file's modification time, in milliseconds since the Unix
    /// epoch.
    pub modified_ms: u64,
}

impl EngineBuild {
    /// The build of the program file at `exe`, or `None` when it can't be read.
    pub fn of_file(version: &str, exe: &std::path::Path) -> Option<Self> {
        let metadata = std::fs::metadata(exe).ok()?;
        let modified_ms = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64;
        Some(Self {
            version: version.to_string(),
            exe: exe.to_string_lossy().into_owned(),
            size: metadata.len(),
            modified_ms,
        })
    }

    /// This process's build, read once: a program file replaced while the
    /// process runs doesn't change what it reports.
    pub fn current() -> Option<Self> {
        static CURRENT: std::sync::OnceLock<Option<EngineBuild>> = std::sync::OnceLock::new();
        CURRENT
            .get_or_init(|| {
                let exe = std::env::current_exe().ok()?;
                Self::of_file(env!("CARGO_PKG_VERSION"), &exe)
            })
            .clone()
    }
}

impl EngineInfo {
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|value| value == capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_scope_uses_wire_safe_names() {
        for (scope, encoded) in [
            (WorkspaceScope::Local, "\"local\""),
            (WorkspaceScope::Synced, "\"synced\""),
            (WorkspaceScope::Development, "\"development\""),
        ] {
            assert_eq!(serde_json::to_string(&scope).unwrap(), encoded);
            assert_eq!(
                serde_json::from_str::<WorkspaceScope>(encoded).unwrap(),
                scope
            );
        }
    }

    #[test]
    fn engine_info_uses_camel_case_fields() {
        let info = EngineInfo {
            device_id: "device-1".into(),
            workspace_scope: WorkspaceScope::Local,
            cursor_sdk_version: Some("1.0.31".into()),
            capabilities: capabilities::current(),
            build: None,
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            serde_json::json!({
                "deviceId": "device-1",
                "workspaceScope": "local",
                "cursorSdkVersion": "1.0.31",
                "capabilities": [
                    "composer-references-v1",
                    "message-queue-v1",
                    "message-queue-actions-v1",
                    "message-queue-attachments-v1",
                    "message-queue-clean-attachment-text-v1",
                    "message-queue-edit-lease-v1",
                    "harness-updates-v1",
                    "projectless-scratch-v1",
                    "openagents-continuations-v1",
                    "chat-home-transfer-v1",
                    "voice-client-media-v1"
                ],
            })
        );
    }

    #[test]
    fn old_engine_info_defaults_to_no_capabilities() {
        let info: EngineInfo = serde_json::from_value(serde_json::json!({
            "deviceId": "old",
            "workspaceScope": "synced"
        }))
        .unwrap();
        assert!(info.capabilities.is_empty());
        assert!(info.cursor_sdk_version.is_none());
        assert!(!info.supports(capabilities::MESSAGE_QUEUE_V1));
        assert!(!info.supports(capabilities::COMPOSER_REFERENCES_V1));
    }
}
