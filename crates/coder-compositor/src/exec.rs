//! Starting a program from a chord or from an `open` request.
//!
//! A row runs through `/bin/sh -c`, the way Hyprland's `exec` does, so a
//! command line with arguments and quoting behaves the same on both
//! sessions. The child inherits the compositor's environment, which is the
//! session's, and gets three variables of its own: `WAYLAND_DISPLAY` for
//! the socket this compositor listens on, `CODER_DESK_SOCKET` for the desk
//! protocol, so a `coder` in a tile can read the screen it draws
//! on, and `DISPLAY` for the Xwayland server the compositor started, so an
//! X11 client, `xdotool`, and the `game` tool reach the same server.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use coder_desk::protocol::SOCKET_VAR;

use crate::extras::Extras;

/// What the two `exec` rows run.
///
/// The Hyprland session builds these lines in Nix from the store path of
/// `os/bin/coder-pane` and the Coder command. This compositor reads them
/// from the environment with the same defaults, until a session option
/// sets them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// The window a tile opens: `coder-pane` on a CoderOS host, which
    /// opens a Coder Desktop pane and falls back to a terminal emulator on
    /// a host whose build has no Coder Desktop.
    pub pane: String,
    /// The terminal a tile opened before the pane did. Nothing here runs
    /// it: `coder-pane` holds the fallback, so one rule covers this
    /// compositor and the Hyprland session.
    pub terminal: String,
    /// The command the pane runs for a Coder.
    pub coder: String,
    /// The Wayland socket the compositor listens on.
    pub display: String,
    /// The desk protocol socket the compositor answers.
    pub desk_socket: PathBuf,
    /// The X11 display the compositor's Xwayland answers on, such as `:1`,
    /// or `None` when no server is running.
    pub x11_display: Option<String>,
    /// The launchers the host granted, by the `coderos.desktop` option
    /// that gates each one, such as `camera`: the rows of
    /// `crates/coder-binds` the bind table answers. `None` when the
    /// session named no list, which grants every launcher.
    pub launchers: Option<Vec<String>>,
    /// The host's own launchers and window rules, from the grant's
    /// `extraBinds` and `extraRules`.
    pub extras: Extras,
}

/// The window a tile opens when nothing names one: `os/bin/coder-pane`,
/// which opens a Coder Desktop pane and falls back to a terminal emulator
/// on a host whose build has no Coder Desktop.
pub const DEFAULT_PANE: &str = "coder-pane";

/// The environment variable that names the window the rows open.
pub const PANE_VAR: &str = "CODER_COMPOSITOR_PANE";

/// The environment variable that names the terminal the two rows opened
/// before the pane did.
pub const TERMINAL_VAR: &str = "CODER_COMPOSITOR_TERMINAL";

/// The environment variable that names the command a Coder tile runs.
pub const COMMAND_VAR: &str = "CODER_COMPOSITOR_COMMAND";

/// The environment variable an X11 client reads its server from.
pub const DISPLAY_VAR: &str = "DISPLAY";

/// The environment variable that names the launchers the host granted,
/// as the options that gate them separated by spaces, such as
/// `camera browser`. `os/bin/coder-compositor-session` sets it from the
/// grant's `launchers` list; a run with it unset answers every launcher.
pub const LAUNCHERS_VAR: &str = "CODER_COMPOSITOR_LAUNCHERS";

impl Session {
    /// The session one display and one desk socket make, reading the
    /// terminal, the Coder command, and the launchers from the
    /// environment.
    pub fn read(display: impl Into<String>, desk_socket: &Path) -> Session {
        Session {
            pane: var_or(PANE_VAR, DEFAULT_PANE),
            terminal: var_or(TERMINAL_VAR, "foot"),
            coder: var_or(COMMAND_VAR, "coder"),
            display: display.into(),
            desk_socket: desk_socket.to_path_buf(),
            x11_display: None,
            launchers: launchers_from(std::env::var(LAUNCHERS_VAR).ok()),
            extras: Extras::read(),
        }
    }

    /// The line the Super+Return row runs: a pane running Coder.
    pub fn coder_line(&self) -> String {
        self.pane.clone()
    }

    /// The line the Super+T row runs: a pane running a bare shell.
    pub fn shell_line(&self) -> String {
        format!("{} --shell", self.pane)
    }

    /// The variables a child gets on top of the session's own.
    pub fn child_environment(&self) -> Vec<(String, String)> {
        let mut environment = vec![
            ("WAYLAND_DISPLAY".to_string(), self.display.clone()),
            (
                SOCKET_VAR.to_string(),
                self.desk_socket.display().to_string(),
            ),
        ];
        if let Some(display) = &self.x11_display {
            environment.push((DISPLAY_VAR.to_string(), display.clone()));
        }
        environment
    }

    /// The variables a child does not inherit from the session.
    ///
    /// A compositor with no Xwayland takes `DISPLAY` away, because the
    /// value the session holds names another compositor's server, and an
    /// X11 client started here would open its window on that one.
    pub fn child_removed(&self) -> Vec<&'static str> {
        match self.x11_display {
            Some(_) => Vec::new(),
            None => vec![DISPLAY_VAR],
        }
    }
}

fn var_or(name: &str, fallback: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => fallback.to_string(),
    }
}

/// The launchers one value of `CODER_COMPOSITOR_LAUNCHERS` grants: the
/// names it holds, or `None` when the variable is unset. A variable set
/// to nothing grants none, which is a host with every launcher option
/// off.
pub fn launchers_from(value: Option<String>) -> Option<Vec<String>> {
    value.map(|names| names.split_whitespace().map(str::to_string).collect())
}

/// Starts one command line and returns the child's process id.
///
/// The compositor never waits for the child in its own loop, so a thread
/// reaps it. A child left unreaped is a zombie for as long as the session
/// runs.
pub fn spawn(line: &str, session: &Session) -> Result<u32, String> {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(line)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in session.child_removed() {
        command.env_remove(name);
    }
    for (name, value) in session.child_environment() {
        command.env(name, value);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("start `{line}`: {err}"))?;
    let pid = child.id();
    let reaper = std::thread::Builder::new().name("coder-exec-reaper".into());
    reaper
        .spawn(move || {
            let _ = child.wait();
        })
        .map_err(|err| format!("reaper thread for `{line}`: {err}"))?;
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            pane: "coder-pane".to_string(),
            terminal: "foot".to_string(),
            coder: "coder".to_string(),
            display: "wayland-2".to_string(),
            desk_socket: PathBuf::from("/run/user/1000/coder-desk/7.sock"),
            x11_display: None,
            launchers: None,
            extras: Extras::default(),
        }
    }

    #[test]
    fn the_launchers_the_session_names_are_read_by_option() {
        assert_eq!(launchers_from(None), None);
        assert_eq!(launchers_from(Some(String::new())), Some(Vec::new()));
        assert_eq!(
            launchers_from(Some("camera  deck\n".to_string())),
            Some(vec!["camera".to_string(), "deck".to_string()])
        );
    }

    /// What a child writes to one file, waited for.
    fn written(line_for: impl Fn(&Path) -> String, session: &Session, name: &str) -> String {
        let out = std::env::temp_dir().join(format!(
            "coder-compositor-exec-{name}-{}.txt",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&out);
        spawn(&line_for(&out), session).expect("the command starts");
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(&out)
                && text.ends_with('\n')
            {
                let _ = std::fs::remove_file(&out);
                return text.trim().to_string();
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the child never wrote {}", out.display());
    }

    #[test]
    fn the_coder_row_runs_a_pane_and_the_shell_row_runs_a_shell_pane() {
        assert_eq!(session().coder_line(), "coder-pane");
        assert_eq!(session().shell_line(), "coder-pane --shell");
    }

    #[test]
    fn a_child_is_told_the_display_and_the_desk_socket() {
        let environment = session().child_environment();
        assert!(environment.contains(&("WAYLAND_DISPLAY".to_string(), "wayland-2".to_string())));
        assert!(environment.contains(&(
            SOCKET_VAR.to_string(),
            "/run/user/1000/coder-desk/7.sock".to_string()
        )));
    }

    #[test]
    fn a_child_is_told_the_x11_display_once_xwayland_runs() {
        let mut session = session();
        assert!(
            !session
                .child_environment()
                .iter()
                .any(|(name, _)| name == DISPLAY_VAR)
        );
        assert_eq!(session.child_removed(), vec![DISPLAY_VAR]);
        session.x11_display = Some(":3".to_string());
        assert!(
            session
                .child_environment()
                .contains(&(DISPLAY_VAR.to_string(), ":3".to_string()))
        );
        assert!(session.child_removed().is_empty());
    }

    #[test]
    fn a_started_command_reads_display_from_its_environment() {
        let mut session = session();
        session.x11_display = Some(":3".to_string());
        let line = |out: &Path| format!("printf '%s\\n' \"$DISPLAY\" > {}", out.display());
        assert_eq!(written(line, &session, "display"), ":3");
        session.x11_display = None;
        let line =
            |out: &Path| format!("printf '%s\\n' \"${{DISPLAY-unset}}\" > {}", out.display());
        assert_eq!(written(line, &session, "no-display"), "unset");
    }

    #[test]
    fn a_started_command_reads_the_desk_socket_from_its_environment() {
        let out =
            std::env::temp_dir().join(format!("coder-compositor-exec-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&out);
        let line = format!("printenv CODER_DESK_SOCKET > {}", out.display());
        spawn(&line, &session()).expect("the command starts");
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(&out)
                && !text.trim().is_empty()
            {
                assert_eq!(text.trim(), "/run/user/1000/coder-desk/7.sock");
                let _ = std::fs::remove_file(&out);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the child never wrote {}", out.display());
    }
}
