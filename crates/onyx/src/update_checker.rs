//! Update checker for Onyx
//!
//! Checks GitHub Releases for new versions of Onyx using the tag prefix pattern `onyx-v{version}`.

use semver::Version;
use serde::Deserialize;

/// Current version from Cargo.toml
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub API URL for releases
const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/OpenAgentsInc/openagents/releases";

/// Tag prefix for Onyx releases
const TAG_PREFIX: &str = "onyx-v";

/// A GitHub release
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    pub prerelease: bool,
    pub name: Option<String>,
}

/// Result of checking for updates
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// Current version is up to date
    UpToDate,
    /// A newer version is available
    UpdateAvailable {
        version: String,
        url: String,
        release_name: Option<String>,
    },
    /// Error occurred during check
    Error(String),
}

/// Check for updates against GitHub Releases
///
/// Fetches all releases and finds the latest one with the `onyx-v` prefix.
/// Compares against the current version using semver.
pub async fn check_for_updates() -> UpdateCheckResult {
    let client = match reqwest::Client::builder().user_agent("Onyx").build() {
        Ok(c) => c,
        Err(e) => return UpdateCheckResult::Error(format!("Failed to create HTTP client: {}", e)),
    };

    let response = match client.get(GITHUB_RELEASES_URL).send().await {
        Ok(r) => r,
        Err(e) => return UpdateCheckResult::Error(format!("Failed to fetch releases: {}", e)),
    };

    if !response.status().is_success() {
        return UpdateCheckResult::Error(format!(
            "GitHub API returned status {}",
            response.status()
        ));
    }

    let releases: Vec<Release> = match response.json().await {
        Ok(r) => r,
        Err(e) => return UpdateCheckResult::Error(format!("Failed to parse releases: {}", e)),
    };

    // Find latest non-prerelease onyx release
    let latest = releases
        .iter()
        .find(|r| r.tag_name.starts_with(TAG_PREFIX) && !r.prerelease);

    let Some(release) = latest else {
        // No releases found - we're up to date (or first release)
        return UpdateCheckResult::UpToDate;
    };

    // Extract version from tag
    let latest_version_str = release.tag_name.strip_prefix(TAG_PREFIX).unwrap_or("0.0.0");

    // Parse versions
    let current = match Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => Version::new(0, 0, 0),
    };

    let latest = match Version::parse(latest_version_str) {
        Ok(v) => v,
        Err(_) => return UpdateCheckResult::UpToDate,
    };

    if latest > current {
        UpdateCheckResult::UpdateAvailable {
            version: latest_version_str.to_string(),
            url: release.html_url.clone(),
            release_name: release.name.clone(),
        }
    } else {
        UpdateCheckResult::UpToDate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version_parses() {
        let version = Version::parse(CURRENT_VERSION);
        assert!(version.is_ok(), "CURRENT_VERSION should be valid semver");
    }
}
