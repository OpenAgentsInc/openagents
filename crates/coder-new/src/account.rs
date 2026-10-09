//! Signing Coder in to an openagents.com account (#11045): `coder-new
//! login` / `logout` and `/login` / `/logout`, over the website's device
//! code ([`openagents_login`]). The token lives in
//! `~/.openagents/coder-new/account.json` (0600), never in session files,
//! and is never shown or logged.

use std::io::Write;
use std::path::Path;
use std::sync::mpsc;

use openagents_login::{Error, Saved};

/// The app name the approval page shows: "Sign in to Coder on …?".
pub const APP: &str = "Coder";

/// The website to sign in to.
#[must_use]
pub fn origin() -> String {
    openagents_login::origin_from(|name| std::env::var(name).ok())
}

/// The signed-in account's name, when this folder holds a live token.
#[must_use]
pub fn signed_in(dir: &Path) -> Option<String> {
    Saved::load(dir)
        .filter(|saved| !saved.expired(now()))
        .map(|saved| saved.label)
}

/// What the background sign-in reports to the terminal.
#[derive(Debug)]
pub enum Event {
    /// Show the code and where to enter it.
    Code {
        user_code: String,
        url: String,
        opened: bool,
    },
    Signed(Saved),
    Failed(String),
}

/// A sign-in running on its own thread.
pub struct Login {
    events: mpsc::Receiver<Event>,
}

impl Login {
    /// Start signing in to `origin` in the background.
    #[must_use]
    pub fn start(origin: String) -> Self {
        let (send, events) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = send.send(Event::Failed("Couldn't start sign-in.".into()));
                return;
            };
            let outcome = runtime.block_on(run(&origin, |user_code, url, opened| {
                let _ = send.send(Event::Code {
                    user_code: user_code.into(),
                    url: url.into(),
                    opened,
                });
            }));
            let _ = send.send(match outcome {
                Ok(saved) => Event::Signed(saved),
                Err(error) => Event::Failed(error.to_string()),
            });
        });
        Self { events }
    }

    /// Events since the last call.
    pub fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

/// The whole flow: start, show the code (and open the browser when there
/// is one), wait for approval.
async fn run(origin: &str, show: impl FnOnce(&str, &str, bool)) -> Result<Saved, Error> {
    let started = openagents_login::start(origin, APP, &openagents_login::computer_name()).await?;
    let complete = started
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| started.verification_uri.clone());
    let opened = openagents_login::open_browser(&complete);
    show(&started.user_code, &started.verification_uri, opened);
    openagents_login::wait(origin, &started).await
}

/// `coder-new login`: prints the code and the page, waits, and saves the
/// account in `dir`.
pub fn login_command(dir: &Path, out: &mut impl Write) -> Result<(), String> {
    let origin = origin();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Couldn't start sign-in.".to_string())?;
    let saved = runtime
        .block_on(run(&origin, |user_code, url, opened| {
            let _ = writeln!(out, "{}", code_text(user_code, url, opened));
            let _ = writeln!(out, "Waiting for you to approve…");
            let _ = out.flush();
        }))
        .map_err(|error| error.to_string())?;
    let label = saved.label.clone();
    saved.store(dir)?;
    writeln!(out, "Signed in to OpenAgents as {label}.").map_err(|e| e.to_string())
}

/// `coder-new logout`: ends the token on the website and removes it here.
pub fn logout_command(dir: &Path, out: &mut impl Write) -> Result<(), String> {
    let Some(saved) = Saved::load(dir) else {
        return writeln!(out, "Not signed in.").map_err(|e| e.to_string());
    };
    Saved::forget(dir)?;
    let ended = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ())
        .and_then(|runtime| {
            runtime
                .block_on(openagents_login::sign_out(&saved))
                .map_err(|_| ())
        });
    match ended {
        Ok(()) => writeln!(out, "Signed out of OpenAgents."),
        Err(()) => writeln!(
            out,
            "Signed out here. The website couldn't be reached; remove this computer in Settings on openagents.com."
        ),
    }
    .map_err(|e| e.to_string())
}

/// What to tell the person once the code exists.
#[must_use]
pub fn code_text(user_code: &str, url: &str, opened: bool) -> String {
    if opened {
        format!("Your browser opened {url}. Check that it shows {user_code}, then approve.")
    } else {
        format!("Open {url} and enter {user_code}.")
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl crate::App {
    /// `/login`.
    pub fn login(&mut self) {
        if self.account_dir.is_none() {
            self.notice = Some(
                "Coder has no folder to keep your sign-in in. Set HOME or use --state.".into(),
            );
            return;
        }
        if self.login.is_some() {
            self.notice = Some("Sign-in is already waiting for you on the website.".into());
            return;
        }
        self.login = Some(Login::start(origin()));
        self.notice = Some("Starting sign-in…".into());
    }

    /// `/logout`.
    pub fn logout(&mut self) {
        self.login = None;
        let Some(dir) = self.account_dir.clone() else {
            self.notice = Some("Not signed in.".into());
            return;
        };
        let Some(saved) = Saved::load(&dir) else {
            self.account = None;
            self.notice = Some("Not signed in.".into());
            return;
        };
        if let Err(error) = Saved::forget(&dir) {
            self.notice = Some(error);
            return;
        }
        self.account = None;
        self.stop_sync();
        self.notice = Some("Signed out of OpenAgents.".into());
        // End the token on the website without holding the terminal.
        std::thread::spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                let _ = runtime.block_on(openagents_login::sign_out(&saved));
            }
        });
    }

    /// Apply what the background sign-in reported (each tick).
    pub(crate) fn poll_login(&mut self) {
        let Some(login) = &self.login else {
            return;
        };
        for event in login.drain() {
            match event {
                Event::Code {
                    user_code,
                    url,
                    opened,
                } => {
                    self.notice = Some(format!(
                        "{} Waiting for you to approve…",
                        code_text(&user_code, &url, opened)
                    ));
                }
                Event::Signed(saved) => {
                    self.login = None;
                    let label = saved.label.clone();
                    let stored = self
                        .account_dir
                        .as_deref()
                        .map_or(Err("No folder for the sign-in.".to_string()), |dir| {
                            saved.store(dir)
                        });
                    match stored {
                        Ok(_) => {
                            self.notice = Some(format!("Signed in to OpenAgents as {label}."));
                            self.account = Some(label);
                            self.start_sync();
                        }
                        Err(error) => self.notice = Some(error),
                    }
                    return;
                }
                Event::Failed(error) => {
                    self.login = None;
                    self.notice = Some(error);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_text_says_where_to_go_and_what_to_check() {
        assert_eq!(
            code_text("BCDF-GHJK", "https://openagents.com/device", false),
            "Open https://openagents.com/device and enter BCDF-GHJK."
        );
        assert!(
            code_text("BCDF-GHJK", "https://openagents.com/device", true)
                .contains("shows BCDF-GHJK")
        );
    }

    #[test]
    fn logout_without_a_sign_in_says_so_and_slash_logout_clears_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        logout_command(dir.path(), &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "Not signed in.\n");

        let mut app = crate::App {
            account_dir: Some(dir.path().to_path_buf()),
            account: Some("Octo".into()),
            ..crate::App::default()
        };
        app.logout();
        assert_eq!(app.account, None);
        assert_eq!(app.notice.as_deref(), Some("Not signed in."));
        assert_eq!(signed_in(dir.path()), None);
    }
}
