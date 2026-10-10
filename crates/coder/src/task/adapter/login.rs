//! The owner's login-shell environment, for a run with full access.
//!
//! A host service, such as a launchd agent or a systemd user unit, starts
//! with a bare environment, so tools the owner installed (Homebrew, a
//! Rust toolchain in `~/.cargo/bin`, a Node version manager) are off its
//! `PATH`. A full-access run asks the owner's own login shell, once at
//! admission, for the environment an interactive terminal would have, and
//! gives every command that environment. Credential variables are left
//! out, and a shell that cannot answer leaves a fixed fallback `PATH` of
//! the usual tool directories. `XAI_API_KEY`, when the shell set it, is
//! kept aside for a Grok Build process and is not part of the command
//! environment or the admission record.

use std::ffi::OsString;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use supervise::{Job, Limits};

#[cfg(any(unix, test))]
/// Printed before the environment, so text a profile prints first is
/// skipped.
const MARKER: &str = "__OPENAGENTS_LOGIN_ENVIRONMENT__";

#[cfg(unix)]
/// The longest the login shell may take to answer.
const DEADLINE: Duration = Duration::from_secs(20);

/// Variables a login shell sets per terminal or per shell process, which a
/// command should get fresh from its own shell instead.
const PER_SHELL: &[&str] = &["_", "SHLVL", "PWD", "OLDPWD", "PS1", "TERM_SESSION_ID"];

/// Directories a fallback `PATH` names before the system path, relative to
/// `HOME` when relative.
const FALLBACK_DIRS: &[&str] = &[
    ".cargo/bin",
    ".local/bin",
    ".bun/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

/// The environment full-access commands run with.
#[derive(Clone, Debug)]
pub struct Environment {
    pub variables: Vec<(OsString, OsString)>,
    /// `login_shell` when the owner's shell answered, else `fallback`.
    pub source: &'static str,
    pub shell: PathBuf,
    /// Grok Build's `XAI_API_KEY`, when the login shell set a non-empty
    /// value. Command environments omit it. The admission record omits it.
    grok_key: Option<OsString>,
}

impl Environment {
    /// Grok Build's `XAI_API_KEY`, when the login shell set a non-empty
    /// value. The value is not part of [`Self::variables`].
    #[must_use]
    pub fn grok_key(&self) -> Option<&OsString> {
        self.grok_key.as_ref()
    }

    /// The value of `name`, when set.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&OsString> {
        self.variables
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    /// What the admission record keeps: the source, the shell, the `PATH`,
    /// and the variable names. Values other than `PATH` are never recorded.
    #[must_use]
    pub fn record(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source,
            "shell": self.shell,
            "path": self.get("PATH").map(|path| path.to_string_lossy().into_owned()),
            "names": self.variables.iter().map(|(key, _)| key.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        })
    }
}

/// Whether `name` holds a credential that commands never receive, by the
/// workspace's one policy.
#[must_use]
pub fn credential(name: &str) -> bool {
    acp_client::process::is_credential_name(name)
}

/// The owner's login-shell environment, or the fallback when the shell
/// cannot answer. `HOME`, `USER`, `LOGNAME`, and `SHELL` always hold this
/// account's own values.
/// On Windows there is no login shell to ask: the host's own environment,
/// which Windows built from the user's profile at sign-in, is the login
/// environment, less the same per-shell and credential variables.
#[cfg(windows)]
#[allow(clippy::unused_async)]
pub async fn capture() -> Environment {
    let account = account();
    finish(std::env::vars_os().collect(), "process", &account)
}

#[cfg(unix)]
pub async fn capture() -> Environment {
    let account = account();
    let shell = account.shell.clone();
    let mut command = std::process::Command::new(&shell);
    command
        .env_clear()
        .env("PATH", crate::task::owner::SYSTEM_PATH)
        .env("HOME", &account.home)
        .env("USER", &account.name)
        .env("LOGNAME", &account.name)
        .env("SHELL", &shell)
        .env("TERM", "dumb")
        .args(["-l", "-i", "-c"])
        .arg(format!("printf '%s' {MARKER}; env -0"))
        .current_dir(&account.home);
    let ended = Job::from_command(command)
        .bounded(Limits::within(DEADLINE).keeping(1024 * 1024))
        .run()
        .await;
    let parsed = (ended.ending.success() && !ended.truncated())
        .then(|| parse(&ended.stdout.text))
        .flatten();
    let (variables, source) = match parsed {
        Some(variables) => (variables, "login_shell"),
        None => (Vec::new(), "fallback"),
    };
    finish(variables, source, &account)
}

/// The account a full-access run acts as.
#[derive(Clone, Debug)]
pub struct Account {
    pub name: OsString,
    pub home: PathBuf,
    pub shell: PathBuf,
}

/// This process's account: its name, home, and login shell from the
/// account database, with `USER`, `HOME`, and `SHELL` preferred when set.
#[cfg(windows)]
#[must_use]
pub fn account() -> Account {
    let comspec = std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file());
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    Account {
        name: std::env::var_os("USERNAME").unwrap_or_default(),
        home: std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| PathBuf::from(r"C:\")),
        shell: comspec.unwrap_or_else(|| root.join(r"System32\cmd.exe")),
    }
}

/// This process's account: its name, home, and login shell from the
/// account database, with `USER`, `HOME`, and `SHELL` preferred when set.
#[cfg(unix)]
#[must_use]
pub fn account() -> Account {
    use std::os::unix::ffi::OsStrExt;
    let (mut name, mut home, mut shell) = (None, None, None);
    // SAFETY: getpwuid returns a pointer into static storage or null; the
    // fields are copied before any other call that could reuse it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if !entry.is_null() {
            let copy = |field: *const libc::c_char| {
                (!field.is_null()).then(|| {
                    std::ffi::OsStr::from_bytes(std::ffi::CStr::from_ptr(field).to_bytes())
                        .to_os_string()
                })
            };
            name = copy((*entry).pw_name);
            home = copy((*entry).pw_dir).map(PathBuf::from);
            shell = copy((*entry).pw_shell).map(PathBuf::from);
        }
    }
    let usable = |path: &Path| path.is_absolute() && path.is_file();
    Account {
        name: std::env::var_os("USER")
            .filter(|v| !v.is_empty())
            .or(name)
            .unwrap_or_default(),
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or(home)
            .unwrap_or_else(|| PathBuf::from("/")),
        shell: std::env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|p| usable(p))
            .or(shell.filter(|p| usable(p)))
            .unwrap_or_else(|| PathBuf::from("/bin/sh")),
    }
}

#[cfg(any(unix, test))]
/// The variables in `env -0` output after [`MARKER`], or `None` without
/// the marker or a `PATH`.
fn parse(stdout: &str) -> Option<Vec<(OsString, OsString)>> {
    let (_, listing) = stdout.split_once(MARKER)?;
    let variables: Vec<(OsString, OsString)> = listing
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(name, _)| {
            !name.is_empty()
                && !name.contains(char::is_whitespace)
                && !name.starts_with("BASH_FUNC_")
        })
        .map(|(name, value)| (name.into(), value.into()))
        .collect();
    variables
        .iter()
        .any(|(name, _)| name == "PATH")
        .then_some(variables)
}

/// The variables commands get: the captured ones without credentials or
/// per-shell state, the account's own identity, and a `PATH` that falls
/// back to the usual tool directories.
fn finish(
    captured: Vec<(OsString, OsString)>,
    source: &'static str,
    account: &Account,
) -> Environment {
    let grok_key = captured
        .iter()
        .find(|(name, value)| name == "XAI_API_KEY" && !value.is_empty())
        .map(|(_, value)| value.clone());
    let mut variables: Vec<(OsString, OsString)> = captured
        .into_iter()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            !credential(&name)
                && !PER_SHELL.contains(&name.as_ref())
                && !matches!(name.as_ref(), "HOME" | "USER" | "LOGNAME" | "SHELL")
        })
        .collect();
    if !variables.iter().any(|(name, _)| name == "PATH") {
        let path = FALLBACK_DIRS
            .iter()
            .map(|dir| {
                if dir.starts_with('/') {
                    PathBuf::from(dir)
                } else {
                    account.home.join(dir)
                }
            })
            .filter(|dir| dir.is_dir())
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(":");
        variables.push(("PATH".into(), path.into()));
    }
    variables.push(("HOME".into(), account.home.clone().into()));
    variables.push(("USER".into(), account.name.clone()));
    variables.push(("LOGNAME".into(), account.name.clone()));
    variables.push(("SHELL".into(), account.shell.clone().into()));
    variables.sort();
    Environment {
        variables,
        source,
        shell: account.shell.clone(),
        grok_key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> Account {
        Account {
            name: "owner".into(),
            home: std::env::temp_dir(),
            shell: "/bin/sh".into(),
        }
    }

    #[test]
    fn a_login_listing_keeps_the_path_and_leaves_out_credentials() {
        let stdout = format!(
            "profile noise\n{MARKER}PATH=/opt/homebrew/bin:/usr/bin\0OPENAI_API_KEY=sk-x\0\
             XAI_API_KEY=present\0GH_TOKEN=t\0APP_SECRET=s\0SHLVL=2\0HOME=/elsewhere\0NVM_DIR=/n\0\
             BASH_FUNC_x%%=() {{ :; }}\0"
        );
        let environment = finish(parse(&stdout).unwrap(), "login_shell", &account());
        let names: Vec<String> = environment
            .variables
            .iter()
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            environment.get("PATH").unwrap(),
            "/opt/homebrew/bin:/usr/bin"
        );
        assert_eq!(environment.get("NVM_DIR").unwrap(), "/n");
        assert_eq!(environment.get("USER").unwrap(), "owner");
        assert_eq!(environment.get("LOGNAME").unwrap(), "owner");
        assert_eq!(
            environment.get("HOME").unwrap(),
            std::env::temp_dir().as_os_str()
        );
        for gone in [
            "OPENAI_API_KEY",
            "XAI_API_KEY",
            "GH_TOKEN",
            "APP_SECRET",
            "SHLVL",
        ] {
            assert!(!names.iter().any(|name| name == gone), "{gone}");
        }
        assert_eq!(
            environment
                .grok_key()
                .map(|value| value.to_string_lossy().into_owned()),
            Some("present".to_owned())
        );
        assert!(!names.iter().any(|name| name.starts_with("BASH_FUNC_")));
        let record = environment.record().to_string();
        assert!(
            !record.contains("sk-x") && !record.contains("present") && !record.contains("/n\"")
        );
    }

    #[test]
    fn a_shell_that_cannot_answer_leaves_the_fallback_path() {
        assert!(parse("no marker here").is_none());
        assert!(parse(&format!("{MARKER}HOME=/h\0")).is_none());
        let environment = finish(Vec::new(), "fallback", &account());
        let path = environment
            .get("PATH")
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(path.split(':').any(|dir| dir == "/usr/bin"), "{path}");
        assert_eq!(environment.source, "fallback");
    }

    #[tokio::test]
    async fn the_owners_shell_answers_with_a_usable_path() {
        let environment = capture().await;
        let path = environment
            .get("PATH")
            .unwrap()
            .to_string_lossy()
            .into_owned();
        // Windows has no login shell; the host's own environment is the
        // login environment, whose `PATH` holds the system directory.
        #[cfg(windows)]
        assert!(
            path.split(';')
                .any(|dir| dir.to_ascii_lowercase().ends_with(r"\system32")),
            "{path}"
        );
        #[cfg(not(windows))]
        assert!(path.split(':').any(|dir| dir == "/usr/bin"), "{path}");
        assert!(environment.get("HOME").is_some());
    }
}
