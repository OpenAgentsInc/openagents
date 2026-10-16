//! Coder Desktop's product identity, in one place.
//!
//! Coder Desktop is a fork of Zeron (MIT). Internal crate and module names stay
//! `zeron-*` so upstream merges stay easy; everything a user or the operating
//! system sees as the product's identity comes from here. See `FORK.md` for
//! the list of call sites that read these values.
//!
//! The `ZERON_*` environment variables (`ZERON_DATA_DIR`, `ZERON_IPC_PORT`,
//! ...) keep their upstream names: they are developer overrides, not product
//! identity. Only their defaults change.

/// The short name users see: app and bundle name, window titles, menus, dock,
/// notifications.
pub const NAME: &str = "Coder";

/// The formal product name: the About panel's long name, docs, and the repo.
pub const LONG_NAME: &str = "Coder Desktop";

/// Upstream attribution shown in About and the README (MIT notice is kept in
/// `LICENSE` and `THIRD_PARTY_NOTICES.md`).
pub const ATTRIBUTION: &str = "Based on Zeron by Wing, MIT";

/// macOS bundle identifier of packaged builds (owner, 2026-10-15). Not
/// `com.openagents.coder`, which is the iPhone app's App Store record and
/// the Android package.
pub const BUNDLE_ID: &str = "com.openagents.coder.desktop";

/// Bundle identifier of the isolated development bundle
/// (`scripts/run-macos-dev.sh`).
pub const DEV_BUNDLE_ID: &str = "com.openagents.coder.desktop.dev";

/// [`BUNDLE_ID`] as a C string, for AppKit notification-center lookups.
pub const BUNDLE_ID_C: &std::ffi::CStr = c"com.openagents.coder.desktop";

/// launchd label of the optional background engine service (`zeron daemon`).
pub const LAUNCHD_LABEL: &str = BUNDLE_ID;

/// systemd user unit of the optional background engine service.
pub const SYSTEMD_UNIT: &str = "coder-desktop.service";

/// URL scheme for conversation deep links and the sign-in callback
/// (`coder-desktop://open/chat/...`). Never the bare `coder://`.
pub const URL_SCHEME: &str = "coder-desktop";

/// Default loopback port of the engine IPC socket. Zeron uses 27654; a
/// different default lets Coder and Zeron run side by side.
pub const DEFAULT_IPC_PORT: u16 = 27854;

/// Folder name of the data directory under the platform's application-data
/// root (`~/Library/Application Support`, `%LOCALAPPDATA%`).
pub const DATA_DIR_NAME: &str = "Coder Desktop";

/// Folder name of the data directory under `$XDG_DATA_HOME` on Linux.
pub const LINUX_DATA_DIR_NAME: &str = "coder-desktop";

/// Home-relative folder for agent worktrees and projectless scratch folders
/// (Zeron uses `~/.zeron`). Kept out of the data directory, as upstream does,
/// because users open these folders in their editors.
pub const HOME_WORK_DIR: &str = ".coder-desktop";

/// The update feed (`{feed}/manifest.json`). `None` turns the updater off:
/// there is no Coder Desktop feed yet, and the app must never offer a Zeron
/// release. Set it here when the Coder Desktop feed exists. The developer
/// override `ZERON_RELEASES_URL` still works.
pub const UPDATE_FEED: Option<&str> = None;

/// Hosted sync and sign-in service. `None` turns off Zeron's hosted edge
/// (WorkOS sign-in, sync, relays, preview signaling): the app runs local and
/// account-free. OpenAgents sign-in is separate work (openagents#11824).
pub const HOSTED_EDGE_URL: Option<&str> = None;

/// Hosted WorkOS client id. `None` keeps sign-in off.
pub const WORKOS_CLIENT_ID: Option<&str> = None;

/// Placeholder edge URL used when [`HOSTED_EDGE_URL`] is `None`. The `.invalid`
/// top-level domain never resolves (RFC 2606), so a stray request fails
/// locally instead of reaching any hosted service.
pub const DISABLED_EDGE_URL: &str = "https://edge.invalid";

/// Where to download Coder Desktop (also the "releases" page).
pub const DOWNLOAD_URL: &str = "https://openagents.com/download";

/// Product documentation.
pub const DOCS_URL: &str = "https://openagents.com/docs";

/// The edge URL the app should use: the hosted edge when one is configured,
/// otherwise the never-resolving placeholder.
pub fn default_edge_url() -> &'static str {
    HOSTED_EDGE_URL.unwrap_or(DISABLED_EDGE_URL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_separate_from_zeron_and_openagents_apps() {
        for id in [BUNDLE_ID, DEV_BUNDLE_ID, LAUNCHD_LABEL] {
            assert!(!id.starts_with("sh.zeron"));
            assert_ne!(id, "com.openagents.coder");
            assert_ne!(id, "com.openagents.desktop");
        }
        assert_eq!(BUNDLE_ID_C.to_str().unwrap(), BUNDLE_ID);
        assert_ne!(URL_SCHEME, "zeron");
        assert_ne!(URL_SCHEME, "coder");
        assert_ne!(DEFAULT_IPC_PORT, 27654);
    }

    #[test]
    fn hosted_services_are_off() {
        assert!(UPDATE_FEED.is_none());
        assert!(HOSTED_EDGE_URL.is_none());
        assert!(WORKOS_CLIENT_ID.is_none());
        assert!(!default_edge_url().contains("zeron"));
    }
}
