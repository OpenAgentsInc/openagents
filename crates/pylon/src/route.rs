//! Routing the owner's own low-risk text jobs to the pool: `openagents pylon
//! route on|off|status` records the choice in `route.json` in the pylon
//! home, and Coder's agents (Alice and the crew's day plans) ask the pool
//! through [`ask_blocking`] while it is on, falling back to their own model
//! when the pool doesn't answer.
//!
//! Off by default. A job's text goes, NIP-44 encrypted, to the pylon that
//! runs it, so the owner turns this on and can name the one pylon they
//! trust (`--pylon NPUB`, such as their own other computer).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::client::{self, Answer, Ask};
use crate::identity::{Identity, hex_pubkey, npub};
use crate::{DEFAULT_RELAY, now};

/// The settings file's schema.
pub const SCHEMA: &str = "openagents.pylon.route.v1";

/// Where the owner's own jobs go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub schema: String,
    pub on: bool,
    pub changed_at: u64,
    pub relay: String,
    /// The one pylon (hex key) jobs go to; `None` takes the best fresh one.
    pub pylon: Option<String>,
    /// How long a job waits for its answer, s.
    pub wait_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            on: false,
            changed_at: 0,
            relay: DEFAULT_RELAY.into(),
            pylon: None,
            wait_secs: 90,
        }
    }
}

/// The settings file in `home`.
#[must_use]
pub fn path(home: &Path) -> PathBuf {
    home.join("route.json")
}

/// The stored settings; off when there are none or they don't parse.
#[must_use]
pub fn load(home: &Path) -> Settings {
    std::fs::read(path(home))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Store `settings`.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save(home: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let tmp = home.join("route.json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path(home)).map_err(|e| e.to_string())
}

/// The prompt a pool job carries for `system` and `prompt`: NIP-CJ's
/// conversation lane takes user turns, so the instructions lead.
#[must_use]
pub fn fold(system: &str, prompt: &str) -> String {
    if system.trim().is_empty() {
        prompt.to_owned()
    } else {
        format!("{}\n\n{}", system.trim(), prompt)
    }
}

/// Ask the pool for one answer under `settings`, as this computer's buyer
/// key, publishing a receipt. Blocks on a runtime of its own, so call it
/// from a thread that isn't driving one.
///
/// # Errors
///
/// When routing is off, no pylon answers, or the job fails.
pub fn ask_blocking(
    home: &Path,
    settings: &Settings,
    system: &str,
    prompt: &str,
) -> Result<Answer, String> {
    if !settings.on {
        return Err("routing to the pool is off".into());
    }
    let buyer = Identity::load_or_create(&home.join("buyer.key"))?;
    let ask = Ask {
        relay: settings.relay.clone(),
        pylon: settings.pylon.clone(),
        prompt: fold(system, prompt),
        wait: Duration::from_secs(settings.wait_secs.clamp(1, 600)),
        publish_receipt: true,
        home: home.to_path_buf(),
        checkers: crate::check::trusted(home),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start a runtime: {e}"))?;
    let answer = runtime.block_on(client::ask(&buyer, &ask))?;
    if answer.text.is_none() {
        return Err(answer
            .error
            .clone()
            .unwrap_or_else(|| "the pylon gave no answer".into()));
    }
    Ok(answer)
}

/// Run `openagents pylon route WORDS` over `home`. Returns the printed
/// document and text.
///
/// # Errors
///
/// On an unknown command or bad option.
pub fn command(words: &[String], home: &Path) -> Result<(serde_json::Value, String), String> {
    let mut words = words.to_vec();
    let mut settings = load(home);
    match words.first().map(String::as_str) {
        Some("on") => {
            words.remove(0);
            while let Some(flag) = words.first().cloned() {
                let value = words
                    .get(1)
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value"))?;
                match flag.as_str() {
                    "--pylon" => {
                        settings.pylon = Some(hex_pubkey(&value).ok_or("--pylon is not a key")?);
                    }
                    "--any" => return Err("--any takes no value".into()),
                    "--relay" => settings.relay = value,
                    "--wait" => {
                        settings.wait_secs = value.parse().map_err(|_| "--wait takes seconds")?;
                    }
                    other => return Err(format!("unexpected argument `{other}`")),
                }
                words.drain(..2);
            }
            settings.on = true;
            settings.changed_at = now();
            save(home, &settings)?;
        }
        Some("off") => {
            settings.on = false;
            settings.changed_at = now();
            save(home, &settings)?;
        }
        Some("status") | None => {}
        Some(other) => return Err(format!("unknown route command `{other}`")),
    }
    let pylon = settings.pylon.as_deref().map(npub);
    Ok((
        json!({"on": settings.on, "relay": settings.relay, "pylon": pylon, "wait_secs": settings.wait_secs}),
        format!(
            "routing the owner's low-risk text jobs to the pool: {}\npylon  {}\nrelay  {}",
            if settings.on { "on" } else { "off" },
            pylon.as_deref().unwrap_or("the best fresh one"),
            settings.relay
        ),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn routing_is_off_until_turned_on() {
        let home = tempfile::tempdir().unwrap();
        assert!(!load(home.path()).on);
        let err = ask_blocking(home.path(), &load(home.path()), "s", "p").unwrap_err();
        assert!(err.contains("off"));
        let pylon = Identity::generate();
        let words = ["on", "--pylon", &pylon.npub(), "--wait", "30"].map(String::from);
        command(&words, home.path()).unwrap();
        let settings = load(home.path());
        assert!(settings.on);
        assert_eq!(settings.pylon.as_deref(), Some(pylon.pubkey()));
        assert_eq!(settings.wait_secs, 30);
        command(&["off".to_string()], home.path()).unwrap();
        assert!(!load(home.path()).on);
        assert!(command(&["sideways".to_string()], home.path()).is_err());
    }

    #[test]
    fn instructions_lead_the_prompt() {
        assert_eq!(fold("", "hi"), "hi");
        assert_eq!(fold("Be brief.", "hi"), "Be brief.\n\nhi");
    }
}
