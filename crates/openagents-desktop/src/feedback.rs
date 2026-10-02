//! **Give feedback** on selected transcript text, sent from this computer
//! (#10127; the shared parts are `playtest::feedback` and
//! `openagents_chat_app::feedback`).
//!
//! Right-clicking a selection in the chat offers **Give feedback** first in
//! the context menu. Its dialog ([`crate::chat`]) quotes the selection and
//! asks for a comment; **Send** files a playtest report sealed with NIP-17
//! to the triage key and signed by this computer's Verse world key, the
//! same report the phone's Report a problem sends, read with `openagents
//! playtest inbox`.
//!
//! Until a build knows the triage key ([`playtest::TRIAGE_KEY`], or
//! [`playtest::TRIAGE_KEY_ENV`] for an operator's proof) the report waits in
//! `~/.openagents/desktop/feedback/` (`0700`, each file `0600`) and is sent
//! with the next feedback a build that knows the key sends. It is never sent
//! to any other key.

use playtest::report::{Context, Platform, Report};
use playtest::session::{Route, Tab};
use std::path::{Path, PathBuf};

/// Sends one report, blocking; the line the dialog shows when it went.
pub type Sender = std::sync::Arc<dyn Fn(Report) -> Result<String, String> + Send + Sync>;

/// The live sender: this computer's world key, the triage key, the relay.
pub fn live() -> Sender {
    std::sync::Arc::new(|report| {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map_or_else(|| PathBuf::from("/"), PathBuf::from);
        let triage = playtest::triage_key(std::env::var(playtest::TRIAGE_KEY_ENV).ok().as_deref());
        deliver(&home.join(".openagents/desktop/feedback"), report, triage)
    })
}

#[cfg(not(windows))]
fn world_key() -> Result<secp256k1::SecretKey, String> {
    crate::grid::store::world_key()
}

#[cfg(windows)]
fn world_key() -> Result<secp256k1::SecretKey, String> {
    Err("Feedback can't be sent from this computer yet.".into())
}

/// Sends `report`, and any that waited, to `triage`; without a triage key
/// keeps it in `outbox`.
fn deliver(
    outbox: &Path,
    report: Report,
    triage: Option<secp256k1::XOnlyPublicKey>,
) -> Result<String, String> {
    let Some(triage) = triage else {
        keep(outbox, &report)?;
        return Ok(playtest::feedback::SAVED.into());
    };
    // The world key signs it, so an accepted comment can earn playtest XP.
    // When the keychain doesn't give it (a dev build's prompt was denied),
    // a one-time key signs instead: the comment still reaches the triage
    // key, without a public record or XP.
    let (world, public) = match world_key() {
        Ok(key) => (key, true),
        Err(_) => (
            secp256k1::SecretKey::new(&mut secp256k1::rand::rng()),
            false,
        ),
    };
    let sealed = openagents_chat_app::feedback::send(&report, &world, &triage, public)?;
    eprintln!("feedback: sent {} to the triage key", sealed.code);
    for (path, waiting) in waiting(outbox) {
        if openagents_chat_app::feedback::send(&waiting, &world, &triage, public).is_ok() {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(playtest::feedback::SENT.into())
}

fn keep(outbox: &Path, report: &Report) -> Result<(), String> {
    let failed = |_| "The feedback couldn't be saved on this computer.".to_string();
    std::fs::create_dir_all(outbox).map_err(failed)?;
    let content = report.content();
    let path = outbox.join(format!(
        "{}.json",
        &playtest::report::digest(&content)[..32]
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let _ = std::fs::set_permissions(outbox, std::fs::Permissions::from_mode(0o700));
        options.mode(0o600);
    }
    use std::io::Write as _;
    options
        .open(path)
        .and_then(|mut file| file.write_all(content.as_bytes()))
        .map_err(failed)
}

fn waiting(outbox: &Path) -> Vec<(PathBuf, Report)> {
    let Ok(entries) = std::fs::read_dir(outbox) else {
        return vec![];
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let report: Report =
                serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            Some((path, report))
        })
        .collect()
}

/// The build, the device, and where in the app: the chat.
pub fn context(at: u64) -> Context {
    Context {
        app_version: env!("CARGO_PKG_VERSION").into(),
        build: option_env!("OPENAGENTS_DESKTOP_BUILD")
            .filter(|b| !b.is_empty() && b.len() <= 8 && b.bytes().all(|c| c.is_ascii_digit()))
            .unwrap_or("0")
            .into(),
        platform: if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        },
        device: device(),
        os_version: os_version(),
        tab: Tab::Coder,
        route: Route::Chat,
        at,
    }
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// The model, such as `Mac15,3`, kept to what a report accepts.
fn device() -> String {
    let model = if cfg!(target_os = "macos") {
        command("sysctl", &["-n", "hw.model"])
    } else {
        None
    };
    let model: String = model
        .unwrap_or_else(|| "Desktop".into())
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || " ,._-()".contains(*ch))
        .take(40)
        .collect();
    if model.is_empty() {
        "Desktop".into()
    } else {
        model
    }
}

/// The OS version, such as `26.4`, kept to what a report accepts.
fn os_version() -> String {
    let version = if cfg!(target_os = "macos") {
        command("sw_vers", &["-productVersion"])
    } else {
        None
    };
    version
        .filter(|v| {
            v.len() <= 16
                && v.split('.').count() <= 4
                && v.split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        })
        .unwrap_or_else(|| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        playtest::feedback::report(
            context(1_790_000_000),
            openagents_chat_app::feedback::selection("Coder runs here.", Some("c1"), &[], 0, None),
            "It runs on my computer.",
        )
        .expect("a report")
    }

    #[test]
    fn this_computer_fills_in_a_context_a_report_accepts() {
        report().check().expect("valid");
    }

    #[test]
    fn without_a_triage_key_feedback_waits_on_this_computer() {
        let dir = std::env::temp_dir().join(format!("oa-feedback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let said = deliver(&dir, report(), None).expect("kept");
        assert_eq!(said, playtest::feedback::SAVED);
        let kept = waiting(&dir);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].1, report());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
