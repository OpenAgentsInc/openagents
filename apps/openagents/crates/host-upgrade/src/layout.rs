//! Where an upgrade reads and writes.

use std::path::{Path, PathBuf};

/// The bundle's name in `target/macos-dev`.
pub const BUNDLE_NAME: &str = "OpenAgents Dev.app";
/// The file in the bundle's `Contents/Resources` naming the commit it was
/// built from (written by `scripts/run-macos-dev.sh`).
pub const COMMIT_FILE: &str = "openagents-commit";
/// Overrides the worktree of `main` the app is built from.
pub const CHECKOUT_VAR: &str = "OPENAGENTS_HOST_UPGRADE_CHECKOUT";
/// Overrides the target folder the app is built in.
pub const TARGET_VAR: &str = "OPENAGENTS_HOST_UPGRADE_TARGET";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The clean worktree of `main` (`~/work/openagents-app`).
    pub checkout: PathBuf,
    /// The shared target folder (`~/work/openagents-target-app`).
    pub target_dir: PathBuf,
    /// The running app's bundle.
    pub bundle: PathBuf,
    /// The app's data folder, written into the bundle it builds.
    pub data_dir: PathBuf,
    /// The app's IPC port, written into the bundle it builds.
    pub ipc_port: u16,
}

impl Layout {
    /// The layout for the app whose binary is `exe`, with the given data
    /// folder and port: the checkout is the one the bundle sits in, unless
    /// [`CHECKOUT_VAR`] names another, and the target folder is
    /// `~/work/openagents-target-app` unless [`TARGET_VAR`] names another.
    ///
    /// # Errors
    /// `exe` isn't a development bundle's binary and no checkout is named.
    pub fn for_app(
        exe: &Path,
        data_dir: PathBuf,
        ipc_port: u16,
        env: &dyn Fn(&str) -> Option<String>,
        home: &Path,
    ) -> Result<Layout, String> {
        let bundle = bundle_of(exe);
        let checkout = match env(CHECKOUT_VAR).filter(|v| !v.trim().is_empty()) {
            Some(path) => PathBuf::from(path),
            None => bundle
                .as_deref()
                .and_then(checkout_of)
                .ok_or_else(|| {
                    format!(
                        "{} isn't a development bundle built from a checkout, so there is nothing to upgrade from main",
                        exe.display()
                    )
                })?,
        };
        let bundle = bundle.unwrap_or_else(|| default_bundle(&checkout));
        let target_dir = env(TARGET_VAR)
            .filter(|v| !v.trim().is_empty())
            .map_or_else(
                || home.join("work").join("openagents-target-app"),
                PathBuf::from,
            );
        Ok(Layout {
            checkout,
            target_dir,
            bundle,
            data_dir,
            ipc_port,
        })
    }

    /// `apps/openagents` in the checkout, where the build script runs.
    #[must_use]
    pub fn app_dir(&self) -> PathBuf {
        self.checkout.join("apps").join("openagents")
    }

    /// The upgrade's own files in the data folder.
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        state_dir(&self.data_dir)
    }

    /// The folder beside the bundle that holds staged and previous bundles.
    /// It's on the bundle's disk, so a swap is two renames.
    #[must_use]
    pub fn work_dir(&self) -> PathBuf {
        self.bundle
            .parent()
            .unwrap_or(Path::new("."))
            .join(".host-upgrade")
    }

    /// Where upgrade `id` builds its bundle.
    #[must_use]
    pub fn stage(&self, id: &str) -> PathBuf {
        self.work_dir().join(id).join(BUNDLE_NAME)
    }

    /// Where the bundle an upgrade replaced is kept, for a roll back.
    #[must_use]
    pub fn previous(&self) -> PathBuf {
        self.work_dir().join("previous").join(BUNDLE_NAME)
    }

    /// The binary in a bundle.
    #[must_use]
    pub fn binary(bundle: &Path) -> PathBuf {
        bundle.join("Contents").join("MacOS").join("openagents")
    }

    /// The commit `bundle` was built from, when it says.
    #[must_use]
    pub fn bundle_commit(bundle: &Path) -> Option<String> {
        let text =
            std::fs::read_to_string(bundle.join("Contents").join("Resources").join(COMMIT_FILE))
                .ok()?;
        let commit = text.trim();
        (commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| commit.to_owned())
    }
}

/// The upgrade's files in a data folder.
#[must_use]
pub fn state_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("host-upgrade")
}

/// The `.app` folder holding `exe`, if it's `<X>.app/Contents/MacOS/<exe>`.
#[must_use]
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_path_buf())
}

/// The checkout a development bundle was built in:
/// `<checkout>/apps/openagents/target/macos-dev/<bundle>`.
fn checkout_of(bundle: &Path) -> Option<PathBuf> {
    let macos_dev = bundle.parent()?;
    let target = macos_dev.parent()?;
    let app = target.parent()?;
    let apps = app.parent()?;
    (macos_dev.file_name()? == "macos-dev"
        && target.file_name()? == "target"
        && app.file_name()? == "openagents"
        && apps.file_name()? == "apps")
        .then(|| apps.parent().map(Path::to_path_buf))
        .flatten()
}

fn default_bundle(checkout: &Path) -> PathBuf {
    checkout
        .join("apps")
        .join("openagents")
        .join("target")
        .join("macos-dev")
        .join(BUNDLE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_bundle_names_its_checkout() {
        let exe = Path::new(
            "/Users/me/work/openagents-app/apps/openagents/target/macos-dev/OpenAgents Dev.app/Contents/MacOS/openagents",
        );
        let none = |_: &str| None;
        let layout =
            Layout::for_app(exe, "/data".into(), 49777, &none, Path::new("/Users/me")).unwrap();
        assert_eq!(layout.checkout, Path::new("/Users/me/work/openagents-app"));
        assert_eq!(
            layout.bundle,
            Path::new(
                "/Users/me/work/openagents-app/apps/openagents/target/macos-dev/OpenAgents Dev.app"
            )
        );
        assert_eq!(
            layout.target_dir,
            Path::new("/Users/me/work/openagents-target-app")
        );
        assert_eq!(
            layout.stage("u1"),
            Path::new(
                "/Users/me/work/openagents-app/apps/openagents/target/macos-dev/.host-upgrade/u1/OpenAgents Dev.app"
            )
        );
        assert_eq!(layout.state_dir(), Path::new("/data/host-upgrade"));

        // A packaged app has no checkout unless one is named.
        let packaged = Path::new("/Applications/OpenAgents.app/Contents/MacOS/openagents");
        assert!(Layout::for_app(packaged, "/d".into(), 1, &none, Path::new("/h")).is_err());
        let named = |name: &str| (name == CHECKOUT_VAR).then(|| "/src/oa".to_owned());
        let layout = Layout::for_app(packaged, "/d".into(), 1, &named, Path::new("/h")).unwrap();
        assert_eq!(layout.checkout, Path::new("/src/oa"));
        assert_eq!(layout.bundle, Path::new("/Applications/OpenAgents.app"));
    }

    #[test]
    fn the_bundle_commit_must_look_like_one() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join(BUNDLE_NAME);
        let resources = bundle.join("Contents").join("Resources");
        std::fs::create_dir_all(&resources).unwrap();
        assert_eq!(Layout::bundle_commit(&bundle), None);
        std::fs::write(resources.join(COMMIT_FILE), "not a commit\n").unwrap();
        assert_eq!(Layout::bundle_commit(&bundle), None);
        std::fs::write(resources.join(COMMIT_FILE), "fb15448d6a0c\n").unwrap();
        assert_eq!(
            Layout::bundle_commit(&bundle).as_deref(),
            Some("fb15448d6a0c")
        );
    }
}
