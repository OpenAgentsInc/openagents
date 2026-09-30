//! The updater's place in the window on Linux and Windows.
//!
//! The Mac offers its update in the menu bar ([`crate::menubar`]). Linux and
//! Windows have no menu-bar item, so Settings offers it: an AppImage checks, downloads,
//! and verifies each release in the background and Settings shows
//! **Restart to update to VERSION**, which replaces the AppImage file,
//! restarts the host unit, and starts the new app. An app installed from
//! the `.deb` is updated by the package manager, so Settings shows
//! **Download VERSION**, which opens the new package in the browser. A build
//! directory (`cargo run`) checks nothing.
//!
//! On Windows the per-user MSI install downloads and verifies the new MSI
//! in the background, and **Restart to update to VERSION** hands it to
//! Windows Installer once the window has closed; the MSI stops the host,
//! upgrades the files, and opens the new app. A copy unpacked from the
//! `.zip` shows **Download VERSION**.
//!
//! `OPENAGENTS_UPDATE_MANIFEST_URL` points the check at another manifest,
//! for testing a release before it is published; it must still be signed by
//! a compiled-in key, so it cannot make the app install anything else.
//!
//! [`command`] is the same flow from a terminal: `--check-update` and
//! `--update`.

#[cfg(not(any(target_os = "linux", windows)))]
use openagents_desktop::chrome;

/// Where a test points the check instead of the published manifest.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
const MANIFEST_OVERRIDE: &str = "OPENAGENTS_UPDATE_MANIFEST_URL";

#[cfg(any(target_os = "linux", windows))]
pub use linux::{act, command, offer, ready, start};

/// Linux's and Windows' shared flow; [`detect`], [`for_install`], and
/// [`open_download`] are what differ.
#[cfg(any(target_os = "linux", windows))]
mod linux {
    use super::MANIFEST_OVERRIDE;
    use openagents_desktop::chrome;
    use openagents_desktop::update::{self, Check, Install, UpdateState, Updater};
    use rust_native_desktop::Waker;
    use std::sync::{Arc, Mutex, OnceLock};

    struct Shared {
        updater: Arc<Updater>,
        state: Arc<Mutex<UpdateState>>,
    }

    static SHARED: OnceLock<Shared> = OnceLock::new();

    #[cfg(target_os = "linux")]
    use update::linux::{detect, open_download};
    #[cfg(windows)]
    use update::windows::{detect, open_download};

    #[cfg(target_os = "linux")]
    fn for_install(install: Install) -> Result<Updater, update::UpdateError> {
        Updater::for_linux(install)
    }

    #[cfg(windows)]
    fn for_install(install: Install) -> Result<Updater, update::UpdateError> {
        Updater::for_windows(install)
    }

    /// Whether this install is only offered its download.
    fn offered_only(install: &Install) -> bool {
        !install.installs_itself()
    }

    /// The updater for this install, or `None` for a build directory
    /// (outside an AppImage or the `.deb` on Linux; not `OpenAgents.exe`
    /// on Windows).
    fn updater() -> Option<Result<Updater, update::UpdateError>> {
        let install = detect()?;
        Some(
            for_install(install).map(|updater| match std::env::var(MANIFEST_OVERRIDE) {
                Ok(url) if !url.is_empty() => updater.with_manifest_url(url),
                _ => updater,
            }),
        )
    }

    /// Starts the background check, once, when the window's event loop
    /// starts.
    pub fn start(waker: Waker) {
        let updater = match updater() {
            None => return,
            Some(Ok(updater)) => Arc::new(updater),
            Some(Err(error)) => {
                eprintln!("openagents-desktop: the updater did not start: {error}");
                return;
            }
        };
        let state = Arc::new(Mutex::new(UpdateState::Idle));
        let report = state.clone();
        let spawned = update::spawn_checker(updater.clone(), move |next| {
            if let Ok(mut current) = report.lock() {
                *current = next;
            }
            waker.wake();
        });
        if let Err(error) = spawned {
            eprintln!("openagents-desktop: the update checker did not start: {error}");
            return;
        }
        let _ = SHARED.set(Shared { updater, state });
    }

    /// What Settings offers now.
    pub fn offer() -> Option<chrome::Update> {
        let shared = SHARED.get()?;
        match &*shared.state.lock().ok()? {
            UpdateState::Ready(staged) => Some(chrome::Update {
                version: staged.version.to_string(),
                ready: true,
            }),
            UpdateState::Available(release) => Some(chrome::Update {
                version: release.version.to_string(),
                ready: false,
            }),
            _ => None,
        }
    }

    /// The version of a downloaded, checked update waiting for a restart,
    /// for the window's update strip ([`crate::strip`]).
    pub fn ready() -> Option<String> {
        offer()
            .filter(|update| update.ready)
            .map(|update| update.version)
    }

    /// Settings' update button, and the update strip's.
    pub fn act() {
        let Some(shared) = SHARED.get() else {
            return;
        };
        let Ok(state) = shared.state.lock().map(|state| state.clone()) else {
            return;
        };
        let result = match state {
            UpdateState::Ready(staged) => shared
                .updater
                .install(&staged)
                .and_then(|app| update::relaunch_after_exit(&app))
                .map(|()| std::process::exit(0)),
            UpdateState::Available(release) => open_download(&release.artifact.url),
            _ => Ok(()),
        };
        if let Err(error) = result {
            eprintln!("openagents-desktop: the update did not install: {error}");
            if let Ok(mut current) = shared.state.lock() {
                *current = UpdateState::Failed(error.to_string());
            }
        }
    }

    /// `--check-update` (`install` false) and `--update` from a terminal:
    /// prints what it found or did and returns whether it succeeded. An
    /// AppImage replaces itself and restarts the host unit; the running
    /// window keeps the old version until it is opened again.
    pub fn command(install: bool) -> bool {
        let updater = match updater() {
            None => {
                eprintln!(
                    "This copy of OpenAgents is a development build, so it does not update itself."
                );
                return false;
            }
            Some(Err(error)) => {
                eprintln!("{error}");
                return false;
            }
            Some(Ok(updater)) => updater,
        };
        let current = updater.current().clone();
        let release = match updater.check() {
            Ok(Check::UpToDate) => {
                println!("OpenAgents {current} is up to date.");
                return true;
            }
            Ok(Check::Available(release)) => release,
            Err(error) => {
                eprintln!("{error}");
                return false;
            }
        };
        let url = &release.artifact.url;
        if !install || offered_only(updater.install_kind()) {
            println!("OpenAgents {} is available: {url}", release.version);
            if *updater.install_kind() == Install::Deb {
                println!("Install it with your package manager.");
            }
            return true;
        }
        println!("Downloading OpenAgents {}…", release.version);
        let installed = updater
            .fetch(&release)
            .and_then(|staged| updater.install(&staged));
        match installed {
            Ok(path) if cfg!(windows) => {
                println!(
                    "Windows Installer installs OpenAgents {} from {} once this exits.",
                    release.version,
                    path.display()
                );
                true
            }
            Ok(path) => {
                println!(
                    "Updated {} to OpenAgents {}.",
                    path.display(),
                    release.version
                );
                true
            }
            Err(error) => {
                eprintln!("{error}");
                false
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn start(_waker: rust_native_desktop::Waker) {}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn offer() -> Option<chrome::Update> {
    None
}

/// The Mac's waiting update, from the menu-bar item's updater.
#[cfg(not(any(target_os = "linux", windows)))]
pub fn ready() -> Option<String> {
    crate::menubar::update_ready()
}

/// The update strip's button on a Mac: the menu's **Restart to Update**.
#[cfg(not(any(target_os = "linux", windows)))]
pub fn act() {
    crate::menubar::install_update();
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn command(_install: bool) -> bool {
    eprintln!("On a Mac, OpenAgents offers updates in its menu-bar menu.");
    false
}
