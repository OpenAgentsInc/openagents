//! Claude Code on this computer (#11234): why Coder can't use it yet, as
//! one plain card with the exact command and **Retry**.
//!
//! Coder runs the unmodified `claude` on this computer, as this user, on
//! the sign-in made there through Anthropic's own flow
//! (`docs/cloud/claude-code-byo.md`). Three things stop it, checked in
//! this order:
//!
//! - **Not found**: no `claude` on `$PATH`, in `~/.local/bin` (where the
//!   installer puts it), or in `~/bin`. The card gives the one-line
//!   install, [`INSTALL`].
//! - **Running as root**: Claude Code won't run tasks without asking for
//!   permission as root, so Coder must run as a regular user.
//! - **Not signed in**: no Claude Code login for this user. The card says
//!   to run `claude`, then type `/login`.
//!
//! Retry reads the state again; nothing here starts `claude`, reads its
//! login, or changes anything.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Claude Code's one-line install.
pub const INSTALL: &str = "curl -fsSL https://claude.ai/install.sh | bash";

/// Why Coder can't use Claude Code on this computer yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// No `claude` where Coder looks ([`find`]).
    NotFound,
    /// This app runs as root, and Claude Code won't run tasks as root.
    Root,
    /// `claude` is here but has no login for this user.
    NotSignedIn,
}

impl Problem {
    /// The card's title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::NotFound => "Claude Code not found",
            Self::Root => "Claude Code can't run as root",
            Self::NotSignedIn => "Claude Code isn't signed in",
        }
    }

    /// What happened and what to do, in one or two sentences.
    #[must_use]
    pub fn detail(self) -> &'static str {
        match self {
            Self::NotFound => {
                "Coder looked on your PATH, in ~/.local/bin, and in ~/bin. Install it in a terminal with this command, then press Retry."
            }
            Self::Root => {
                "Claude Code won't run tasks for an app running as root. Quit OpenAgents and open it again as your regular user, not with sudo, then press Retry."
            }
            Self::NotSignedIn => {
                "Run this in a terminal, then type /login and finish signing in on Anthropic's page. Then press Retry."
            }
        }
    }

    /// The exact command to run, when there is one.
    #[must_use]
    pub fn command(self) -> Option<&'static str> {
        match self {
            Self::NotFound => Some(INSTALL),
            Self::Root => None,
            Self::NotSignedIn => Some("claude"),
        }
    }

    /// Whether the card shows when Coder has another agent ready: a
    /// missing Claude Code matters only when nothing else can work, while
    /// one that is installed but can't run always says why.
    #[must_use]
    pub fn shows_beside_another_agent(self) -> bool {
        self != Self::NotFound
    }
}

/// The folders under the home folder Coder looks in after `$PATH`.
pub const HOME_DIRS: &[&str] = &[".local/bin", "bin"];

/// The `claude` Coder would run: the first on `path` (a `$PATH` value),
/// else in `~/.local/bin`, else in `~/bin` under `home`.
#[must_use]
pub fn find(path: Option<&OsStr>, home: Option<&Path>) -> Option<PathBuf> {
    let program = format!("claude{}", std::env::consts::EXE_SUFFIX);
    let on_path = path
        .into_iter()
        .flat_map(|path| std::env::split_paths(path))
        .filter(|dir| !dir.as_os_str().is_empty());
    let in_home = home
        .into_iter()
        .flat_map(|home| HOME_DIRS.iter().map(move |dir| home.join(dir)));
    on_path
        .chain(in_home)
        .map(|dir| dir.join(&program))
        .find(|candidate| candidate.is_file())
}

/// The first problem, in the order a person must fix them, or `None` when
/// Claude Code can run: `found` is whether [`find`] found it, `root`
/// whether this app runs as root, `signed_in` whether it has a login here.
#[must_use]
pub fn diagnose(found: bool, root: bool, signed_in: bool) -> Option<Problem> {
    if !found {
        Some(Problem::NotFound)
    } else if root {
        Some(Problem::Root)
    } else if !signed_in {
        Some(Problem::NotSignedIn)
    } else {
        None
    }
}

/// Whether this process runs as root.
#[must_use]
pub fn running_as_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// [`diagnose`] for this computer: this process's `PATH` and user, the
/// home folder `home`, and `signed_in` from the platform's login check.
#[must_use]
pub fn check(home: &Path, signed_in: bool) -> Option<Problem> {
    let path = std::env::var_os("PATH");
    diagnose(
        find(path.as_deref(), Some(home)).is_some(),
        running_as_root(),
        signed_in,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let program = dir.join(format!("claude{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&program, "#!/bin/sh\n").unwrap();
        program
    }

    #[test]
    fn claude_code_is_looked_for_on_path_then_local_bin_then_bin() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let elsewhere = temp.path().join("tools");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(find(None, Some(&home)), None);
        let bin = install(&home.join("bin"));
        assert_eq!(find(None, Some(&home)), Some(bin.clone()));
        let local = install(&home.join(".local/bin"));
        assert_eq!(find(None, Some(&home)), Some(local));
        let on_path = install(&elsewhere);
        let path = std::env::join_paths([elsewhere.as_path()]).unwrap();
        assert_eq!(find(Some(&path), Some(&home)), Some(on_path));
        // A folder named claude is not the program.
        let empty = temp.path().join("empty");
        std::fs::create_dir_all(empty.join(format!("claude{}", std::env::consts::EXE_SUFFIX)))
            .unwrap();
        assert_eq!(find(Some(empty.as_os_str()), None), None);
    }

    #[test]
    fn problems_come_in_the_order_they_must_be_fixed() {
        assert_eq!(diagnose(false, true, false), Some(Problem::NotFound));
        assert_eq!(diagnose(true, true, false), Some(Problem::Root));
        assert_eq!(diagnose(true, true, true), Some(Problem::Root));
        assert_eq!(diagnose(true, false, false), Some(Problem::NotSignedIn));
        assert_eq!(diagnose(true, false, true), None);
    }

    #[test]
    fn each_card_names_the_exact_command() {
        assert_eq!(
            Problem::NotFound.command(),
            Some("curl -fsSL https://claude.ai/install.sh | bash")
        );
        assert!(Problem::NotFound.detail().contains("~/.local/bin"));
        assert!(Problem::NotFound.detail().contains("~/bin"));
        assert_eq!(Problem::NotSignedIn.command(), Some("claude"));
        assert!(Problem::NotSignedIn.detail().contains("/login"));
        assert_eq!(Problem::Root.command(), None);
        assert!(Problem::Root.detail().contains("regular user"));
        for problem in [Problem::NotFound, Problem::Root, Problem::NotSignedIn] {
            assert!(problem.detail().contains("Retry"), "{problem:?}");
            let text = format!("{} {}", problem.title(), problem.detail());
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        }
        assert!(!Problem::NotFound.shows_beside_another_agent());
        assert!(Problem::NotSignedIn.shows_beside_another_agent());
        assert!(Problem::Root.shows_beside_another_agent());
    }
}
