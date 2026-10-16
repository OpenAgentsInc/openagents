//! The OpenAgents desktop app's product identity, in one place.
//!
//! The app (formerly Coder Desktop) is a fork of Zeron (MIT). Internal crate and module names stay
//! `zeron-*` so upstream merges stay easy; everything a user or the operating
//! system sees as the product's identity comes from here. See `FORK.md` for
//! the list of call sites that read these values.
//!
//! The `ZERON_*` environment variables (`ZERON_DATA_DIR`, `ZERON_IPC_PORT`,
//! ...) keep their upstream names: they are developer overrides, not product
//! identity. Only their defaults change.

/// The short name users see: app and bundle name, window titles, menus, dock,
/// notifications.
pub const NAME: &str = "OpenAgents";

/// The formal product name: the About panel's long name, docs, and the repo.
pub const LONG_NAME: &str = "OpenAgents";

/// Upstream attribution shown in About and the README (MIT notice is kept in
/// `LICENSE` and `THIRD_PARTY_NOTICES.md`).
pub const ATTRIBUTION: &str = "Based on Zeron by Wing, MIT";

/// macOS bundle identifier of packaged builds (owner, 2026-10-16): the
/// OpenAgents desktop app's own identifier, which this app takes over from
/// the earlier Rust Native desktop (`crates/openagents-desktop`).
pub const BUNDLE_ID: &str = "com.openagents.desktop";

/// Bundle identifier of the isolated development bundle
/// (`scripts/run-macos-dev.sh`).
pub const DEV_BUNDLE_ID: &str = "com.openagents.desktop.dev";

/// [`BUNDLE_ID`] as a C string, for AppKit notification-center lookups.
pub const BUNDLE_ID_C: &std::ffi::CStr = c"com.openagents.desktop";

/// launchd label of the optional background engine service (`zeron daemon`).
pub const LAUNCHD_LABEL: &str = BUNDLE_ID;

/// systemd user unit of the optional background engine service.
pub const SYSTEMD_UNIT: &str = "openagents-desktop.service";

/// URL scheme for conversation deep links and the sign-in callback
/// (`openagents://open/chat/...`). Never the bare `coder://`.
pub const URL_SCHEME: &str = "openagents";

/// Default loopback port of the engine IPC socket. Zeron uses 27654; a
/// different default lets this app and Zeron run side by side.
pub const DEFAULT_IPC_PORT: u16 = 27854;

/// Folder name of the data directory under the platform's application-data
/// root (`~/Library/Application Support`, `%LOCALAPPDATA%`). Not
/// `OpenAgents`, which the earlier Rust Native desktop uses in another format.
pub const DATA_DIR_NAME: &str = "OpenAgents Desktop";

/// Folder name of the data directory under `$XDG_DATA_HOME` on Linux.
pub const LINUX_DATA_DIR_NAME: &str = "openagents-desktop";

/// Home-relative folder for agent worktrees and projectless scratch folders
/// (Zeron uses `~/.zeron`). Kept out of the data directory, as upstream does,
/// because users open these folders in their editors. Kept from the Coder
/// Desktop name so existing worktrees and the chats that use them keep their
/// paths.
pub const HOME_WORK_DIR: &str = ".coder-desktop";

/// Identity before the 2026-10-16 rename, read once to carry data and the
/// OpenAgents sign-in over (`apps/zeron/src/paths.rs`,
/// `coder-account::desktop`).
pub const LEGACY_BUNDLE_ID: &str = "com.openagents.coder.desktop";
pub const LEGACY_DEV_BUNDLE_ID: &str = "com.openagents.coder.desktop.dev";
pub const LEGACY_DATA_DIR_NAME: &str = "Coder Desktop";
pub const LEGACY_LINUX_DATA_DIR_NAME: &str = "coder-desktop";

/// The update feed (`{feed}/manifest.json`). `None` turns the updater off:
/// there is no OpenAgents desktop feed yet, and the app must never offer a Zeron
/// release. Set it here when the OpenAgents desktop feed exists. The developer
/// override `ZERON_RELEASES_URL` still works.
pub const UPDATE_FEED: Option<&str> = None;

/// OpenAgents sync service. Authentication uses the shared OpenAgents account
/// client; upstream WorkOS support requires an explicit compatibility feature.
pub const HOSTED_EDGE_URL: Option<&str> = Some("https://sync.openagents.com");

/// Hosted WorkOS client id. `None` keeps sign-in off.
pub const WORKOS_CLIENT_ID: Option<&str> = None;

/// Placeholder edge URL used when [`HOSTED_EDGE_URL`] is `None`. The `.invalid`
/// top-level domain never resolves (RFC 2606), so a stray request fails
/// locally instead of reaching any hosted service.
pub const DISABLED_EDGE_URL: &str = "https://edge.invalid";

/// Where to download the app (also the "releases" page).
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
    fn identity_is_the_openagents_desktop_and_separate_from_zeron() {
        for id in [BUNDLE_ID, DEV_BUNDLE_ID, LAUNCHD_LABEL] {
            assert!(!id.starts_with("sh.zeron"));
            assert!(id.starts_with("com.openagents.desktop"));
            assert_ne!(id, "com.openagents.app");
        }
        assert_eq!(NAME, "OpenAgents");
        assert_ne!(DATA_DIR_NAME, "OpenAgents");
        assert_ne!(LEGACY_BUNDLE_ID, BUNDLE_ID);
        assert_ne!(LEGACY_DATA_DIR_NAME, DATA_DIR_NAME);
        assert_eq!(BUNDLE_ID_C.to_str().unwrap(), BUNDLE_ID);
        assert_ne!(URL_SCHEME, "zeron");
        assert_ne!(URL_SCHEME, "coder");
        assert_ne!(DEFAULT_IPC_PORT, 27654);
    }

    #[test]
    fn sync_uses_openagents_and_upstream_login_and_updates_are_off() {
        assert!(UPDATE_FEED.is_none());
        assert_eq!(default_edge_url(), "https://sync.openagents.com");
        assert!(WORKOS_CLIENT_ID.is_none());
        assert!(!default_edge_url().contains("zeron"));
    }
}
