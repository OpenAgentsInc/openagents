//! Claude Code credential extraction and injection for containers.
//!
//! Extracts OAuth credentials from Mac Keychain at runtime.
//! Creates temporary credential files for container mounting.

use crate::config::CredentialMount;
use crate::error::{CredentialError, CredentialResult};
use std::path::PathBuf;
use tokio::fs;
use tokio::process::Command;
use uuid::Uuid;

/// Keychain service name for Claude Code credentials
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Credential filename inside the mount directory
const CREDENTIAL_FILENAME: &str = ".credentials.json";

/// Container directory where credentials are mounted
const CONTAINER_DIR: &str = "/root/.claude";

/// Extract raw credentials JSON from Mac Keychain.
///
/// Uses `security find-generic-password` to extract Claude Code OAuth credentials.
/// Returns the raw JSON string containing the credentials.
pub async fn extract_credentials_from_keychain() -> CredentialResult<String> {
    // Only available on macOS
    if std::env::consts::OS != "macos" {
        return Err(CredentialError::not_found(
            "Mac Keychain only available on macOS",
        ));
    }

    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-g"])
        .output()
        .await
        .map_err(|e| {
            CredentialError::extraction_failed(format!("Failed to run security command: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("could not be found") {
            return Err(CredentialError::not_found(
                "Claude Code credentials not found in Keychain. Please authenticate with Claude Code first.",
            ));
        }
        return Err(CredentialError::access_denied(format!(
            "Keychain access failed: {}",
            stderr
        )));
    }

    // Parse: password: "{\"claudeAiOauth\":{...}}"
    let stderr = String::from_utf8_lossy(&output.stderr);
    let password_regex = regex_lite::Regex::new(r#"^password:\s*"(.+)"$"#).ok();

    let json_str = if let Some(regex) = password_regex {
        if let Some(caps) = regex.captures(&stderr) {
            // Unescape the JSON (Keychain escapes quotes and backslashes)
            caps.get(1)
                .map(|m| m.as_str().replace("\\\"", "\"").replace("\\\\", "\\"))
        } else {
            None
        }
    } else {
        // Fallback: try to find the password line manually
        stderr
            .lines()
            .find(|line| line.starts_with("password: "))
            .and_then(|line| {
                let content = line.strip_prefix("password: ")?;
                // Remove surrounding quotes
                let content = content.trim_matches('"');
                Some(content.replace("\\\"", "\"").replace("\\\\", "\\"))
            })
    };

    let json_str = json_str.ok_or_else(|| {
        CredentialError::invalid_format("Could not parse password from Keychain output")
    })?;

    // Validate it's valid JSON
    serde_json::from_str::<serde_json::Value>(&json_str).map_err(|_| {
        CredentialError::invalid_format("Keychain password is not valid JSON")
    })?;

    Ok(json_str)
}

/// Create a temporary credential mount for container use.
///
/// Extracts credentials from Keychain and writes them to a temp directory
/// that can be mounted into the container at /root/.claude.
///
/// Returns paths and the volume mount string for use with container run.
pub async fn create_credential_mount() -> CredentialResult<CredentialMount> {
    // Extract credentials from Keychain
    let credentials_json = extract_credentials_from_keychain().await?;

    // Create temp directory with unique name
    let uuid = Uuid::new_v4().to_string();
    let short_uuid = &uuid[..8];
    let host_dir = PathBuf::from(format!("/tmp/mechacoder-creds-{}", short_uuid));
    let host_file_path = host_dir.join(CREDENTIAL_FILENAME);

    // Create directory
    fs::create_dir_all(&host_dir)
        .await
        .map_err(|e| CredentialError::extraction_failed(format!("Failed to create temp dir: {}", e)))?;

    // Write credentials file
    fs::write(&host_file_path, &credentials_json)
        .await
        .map_err(|e| {
            CredentialError::extraction_failed(format!("Failed to write credentials: {}", e))
        })?;

    // Set restrictive permissions (owner read-only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(&host_file_path, perms)
            .await
            .map_err(|e| {
                CredentialError::extraction_failed(format!("Failed to set permissions: {}", e))
            })?;
    }

    Ok(CredentialMount::new(
        host_dir,
        host_file_path,
        CONTAINER_DIR.to_string(),
    ))
}

/// Remove credential mount directory.
///
/// Should be called after container execution completes to clean up
/// temporary credential files.
pub async fn cleanup_credential_mount(mount: &CredentialMount) -> Result<(), std::io::Error> {
    fs::remove_dir_all(&mount.host_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_mount_new() {
        let host_dir = PathBuf::from("/tmp/test-creds");
        let host_file = PathBuf::from("/tmp/test-creds/.credentials.json");
        let mount = CredentialMount::new(host_dir.clone(), host_file.clone(), "/root/.claude".to_string());

        assert_eq!(mount.host_dir, host_dir);
        assert_eq!(mount.host_file_path, host_file);
        assert_eq!(mount.container_dir, "/root/.claude");
        assert_eq!(mount.volume_mount, "/tmp/test-creds:/root/.claude:ro");
    }
}
