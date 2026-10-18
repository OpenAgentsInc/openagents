//! What the background engine can't sign in to, checked before an app hands
//! its runs over. A login the service can't read is reported here, in plain
//! words, rather than found in the middle of a run.

use std::fmt;
use std::path::Path;

/// Where an agent keeps its login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessLogin {
    /// The agent's name as people know it.
    pub name: &'static str,
    /// Files under the home folder; any one of them is a login.
    pub files: &'static [&'static str],
    /// A Keychain item that is also a login (macOS).
    pub keychain: Option<&'static str>,
    /// What a person runs to sign in.
    pub login_command: &'static str,
}

/// The login of the agent with the harness id `id` (kebab case), when it
/// keeps one this check can see.
pub fn harness_login(id: &str) -> Option<HarnessLogin> {
    match id {
        "claude-code" => Some(HarnessLogin {
            name: "Claude Code",
            files: &[".claude/.credentials.json"],
            keychain: Some("Claude Code-credentials"),
            login_command: "claude",
        }),
        "codex" => Some(HarnessLogin {
            name: "Codex",
            files: &[".codex/auth.json"],
            keychain: None,
            login_command: "codex login",
        }),
        _ => None,
    }
}

/// One thing the background engine can't use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The app is signed in only for this window.
    SignInNotSaved,
    /// The agent's program isn't installed where the engine looks.
    NotInstalled { name: &'static str },
    /// The agent has no login the engine can read.
    SignedOut {
        name: &'static str,
        login_command: &'static str,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignInNotSaved => write!(
                f,
                "Couldn't save your account connection on this computer. Unlock your device, then try again."
            ),
            Self::NotInstalled { name } => write!(
                f,
                "{name} isn't installed where background agents can find it. Install it from Settings, then try again."
            ),
            Self::SignedOut {
                name,
                login_command,
            } => write!(
                f,
                "{name} isn't signed in for background agents. Run `{login_command}` in a terminal and sign in, then try again."
            ),
        }
    }
}

/// An agent in use, and whether its program is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessInUse {
    pub login: HarnessLogin,
    pub installed: bool,
}

/// The facts the check reads.
pub struct Checks<'a> {
    pub home: &'a Path,
    /// The app is signed in to OpenAgents.
    pub signed_in: bool,
    /// A process started now, like the service, finds that sign-in saved.
    pub sign_in_saved: bool,
    pub harnesses: &'a [HarnessInUse],
    /// Whether a Keychain item exists.
    pub keychain_has: &'a dyn Fn(&str) -> bool,
}

/// Everything the background engine can't use; empty means hand over.
pub fn preflight(checks: &Checks<'_>) -> Vec<Problem> {
    let mut problems = Vec::new();
    if checks.signed_in && !checks.sign_in_saved {
        problems.push(Problem::SignInNotSaved);
    }
    for harness in checks.harnesses {
        let login = harness.login;
        if !harness.installed {
            problems.push(Problem::NotInstalled { name: login.name });
            continue;
        }
        let file = login
            .files
            .iter()
            .any(|file| checks.home.join(file).is_file());
        let keychain = login
            .keychain
            .is_some_and(|item| (checks.keychain_has)(item));
        if !file && !keychain {
            problems.push(Problem::SignedOut {
                name: login.name,
                login_command: login.login_command,
            });
        }
    }
    problems
}

/// Whether the login Keychain has a generic password for `service`. It asks
/// only whether the item exists and never reads its secret.
pub fn keychain_has(service: &str) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    std::process::Command::new("security")
        .args(["find-generic-password", "-s", service])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
