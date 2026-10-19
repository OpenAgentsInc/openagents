//! Installing the pinned Psionic engine, and the rule that an update never
//! changes the served model without the owner saying so.
//!
//! `setup` and `update` download the `psionic-openai-server` build this
//! CLI pins for this platform ([`crate::release::PSIONIC_PINS`]) into
//! `~/.openagents/bin`, checking its size and sha256 before it takes its
//! name, the same way the model is fetched ([`crate::setup::fetch`]). A
//! platform with no published build keeps today's behavior: the engine is
//! whatever `psionic-openai-server` the user built and put on `PATH`.
//!
//! The served model id is remembered in the pylon home
//! ([`SERVED_MODEL_FILE`]). When a new CLI pins a different model id,
//! `update` refuses until the owner runs it with `--accept-model-change`,
//! which also clears the stored self-check so the new model is checked
//! before it serves. That flag is never offered over MCP or run by a
//! timer.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::setup::{Host, SERVER};

/// One published engine build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePin {
    pub platform: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// The engine platform for this computer: `macos-aarch64`,
/// `linux-x86_64-cuda` (an NVIDIA GPU), `linux-x86_64-cpu`, or `None` when
/// no build is made for it.
#[must_use]
pub fn platform(host: &Host) -> Option<String> {
    match (host.os.as_str(), host.arch.as_str()) {
        ("macos", "aarch64") => Some("macos-aarch64".into()),
        ("linux", "x86_64") if host.gpu == "nvidia" => Some("linux-x86_64-cuda".into()),
        ("linux", "x86_64") => Some("linux-x86_64-cpu".into()),
        _ => None,
    }
}

/// The pin for `platform` in `pins`, when its engine is published (a URL,
/// a sha256, and a size).
#[must_use]
pub fn pin_for(pins: &[crate::release::PsionicPin], platform: Option<&str>) -> Option<EnginePin> {
    let platform = platform?;
    let pin = pins.iter().find(|p| p.platform == platform)?;
    Some(EnginePin {
        platform: pin.platform.to_string(),
        url: pin.url?.to_string(),
        sha256: pin.sha256?.to_string(),
        size: pin.size?,
    })
}

/// This CLI build's pin for this computer.
#[must_use]
pub fn this_pin(host: &Host) -> Option<EnginePin> {
    pin_for(crate::release::PSIONIC_PINS, platform(host).as_deref())
}

/// Where the installed engine lives: `~/.openagents/bin`.
#[must_use]
pub fn bin_dir() -> PathBuf {
    user_home::home_os()
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents")
        .join("bin")
}

/// What installing the engine did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum EngineOutcome {
    /// The pinned build was downloaded, checked, and put in place
    /// (`replaced` when another file had that name).
    Installed { path: PathBuf, replaced: bool },
    /// The file in place already is the pinned build.
    AlreadyPinned { path: PathBuf },
    /// No build is published for this platform yet; `found` is the engine
    /// the user built, if one is on this computer.
    NotPublished {
        platform: Option<String>,
        found: Option<PathBuf>,
    },
}

impl EngineOutcome {
    /// The engine `start` will run, if there is one.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Installed { path, .. } | Self::AlreadyPinned { path } => Some(path),
            Self::NotPublished { found, .. } => found.as_deref(),
        }
    }

    /// One line for people.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Installed { path, replaced } => format!(
                "{} ({}, checked against this release)",
                path.display(),
                if *replaced { "updated" } else { "downloaded" }
            ),
            Self::AlreadyPinned { path } => {
                format!("{} (already this release's engine)", path.display())
            }
            Self::NotPublished { platform, found } => {
                let what = platform.as_deref().map_or_else(
                    || "this kind of computer".to_string(),
                    |p| format!("this platform ({p})"),
                );
                match found {
                    Some(path) => format!(
                        "{} (the engine is not published for {what} yet, so the one you built is used)",
                        path.display()
                    ),
                    None => format!("not installed; the engine is not published for {what} yet"),
                }
            }
        }
    }
}

/// Put the pinned engine in `dir` unless the file there already is it.
/// Without a pin, look for one the user built ([`crate::setup::find_server`]).
///
/// # Errors
///
/// The download fails or is not the pinned build (nothing takes the
/// engine's name then).
pub async fn ensure_engine(
    pin: Option<&EnginePin>,
    platform: Option<&str>,
    dir: &Path,
    progress: impl FnMut(u64),
) -> Result<EngineOutcome, String> {
    let Some(pin) = pin else {
        return Ok(EngineOutcome::NotPublished {
            platform: platform.map(str::to_string),
            found: crate::setup::find_server(),
        });
    };
    let path = dir.join(SERVER);
    let present = path.is_file();
    if present && crate::setup::sha256_file(&path).ok().as_deref() == Some(pin.sha256.as_str()) {
        return Ok(EngineOutcome::AlreadyPinned { path });
    }
    crate::setup::fetch_file(
        &pin.url,
        &path,
        pin.size,
        &pin.sha256,
        Some(0o755),
        progress,
    )
    .await
    .map_err(|e| format!("the Psionic engine: {e}"))?;
    Ok(EngineOutcome::Installed {
        path,
        replaced: present,
    })
}

/// Whether `server` is the pinned engine: `None` without a pin or a
/// server, else whether its sha256 matches.
#[must_use]
pub fn engine_matches(pin: Option<&EnginePin>, server: Option<&Path>) -> Option<bool> {
    let (pin, server) = (pin?, server?);
    Some(crate::setup::sha256_file(server).ok().as_deref() == Some(pin.sha256.as_str()))
}

/// The file in the pylon home that names the model id this computer
/// serves.
pub const SERVED_MODEL_FILE: &str = "served-model";

/// The model id this computer was set up to serve, if recorded.
#[must_use]
pub fn served_model(home: &Path) -> Option<String> {
    std::fs::read_to_string(home.join(SERVED_MODEL_FILE))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|id| !id.is_empty())
}

/// Record the model id this computer serves.
///
/// # Errors
///
/// When the file cannot be written.
pub fn record_served_model(home: &Path, model_id: &str) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    std::fs::write(home.join(SERVED_MODEL_FILE), format!("{model_id}\n")).map_err(|e| e.to_string())
}

/// Clear the stored self-check, so the next `start` checks again.
///
/// # Errors
///
/// When it exists and cannot be removed.
pub fn reset_selfcheck(home: &Path) -> Result<(), String> {
    match std::fs::remove_file(home.join(crate::calibrate::SELFCHECK_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// What an update would do to the served model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "model_change", rename_all = "snake_case")]
pub enum ModelChange {
    /// Nothing was served before.
    First,
    /// The same model id.
    Same,
    /// A different model id.
    Changed { from: String, to: String },
}

/// Compare what was served (the record, else the model on disk) with the
/// id this build pins.
#[must_use]
pub fn model_change(recorded: Option<&str>, on_disk: Option<&str>, pinned: &str) -> ModelChange {
    match recorded.or(on_disk) {
        None => ModelChange::First,
        Some(id) if id == pinned => ModelChange::Same,
        Some(id) => ModelChange::Changed {
            from: id.to_string(),
            to: pinned.to_string(),
        },
    }
}

/// The flag that accepts a change of served model.
pub const ACCEPT_MODEL_CHANGE: &str = "--accept-model-change";

/// Apply the model-change rule before `setup` or `update` touches the
/// model: a change needs `accept`, and an accepted change clears the
/// stored self-check. Returns the change.
///
/// # Errors
///
/// The model id would change and `accept` is false; the message names
/// the step only the owner takes.
pub fn guard_model_change(
    home: &Path,
    on_disk: Option<&str>,
    pinned: &str,
    accept: bool,
) -> Result<ModelChange, String> {
    let change = model_change(served_model(home).as_deref(), on_disk, pinned);
    if let ModelChange::Changed { from, to } = &change {
        if !accept {
            return Err(format!(
                "needs the owner: this release serves a different model ({to}) than this computer does now ({from}). \
                 Nothing was changed. To switch, the owner runs `openagents pylon update {ACCEPT_MODEL_CHANGE}`; \
                 the self-check then runs again for the new model before it serves."
            ));
        }
        reset_selfcheck(home)?;
    }
    Ok(change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn host(os: &str, arch: &str, gpu: &str) -> Host {
        Host {
            os: os.into(),
            arch: arch.into(),
            gpu: gpu.into(),
            ..Host::default()
        }
    }

    #[test]
    fn platforms_follow_the_computer() {
        assert_eq!(
            platform(&host("macos", "aarch64", "apple")).as_deref(),
            Some("macos-aarch64")
        );
        assert_eq!(
            platform(&host("linux", "x86_64", "nvidia")).as_deref(),
            Some("linux-x86_64-cuda")
        );
        assert_eq!(
            platform(&host("linux", "x86_64", "none")).as_deref(),
            Some("linux-x86_64-cpu")
        );
        assert_eq!(platform(&host("windows", "x86_64", "nvidia")), None);
    }

    #[test]
    fn a_pin_is_found_only_for_its_platform() {
        let pins = [
            crate::release::PsionicPin {
                platform: "macos-aarch64",
                url: Some("https://x/psionic"),
                sha256: Some("ab"),
                size: Some(7),
                build_commit: Some("c0ffee"),
            },
            crate::release::PsionicPin {
                platform: "linux-x86_64-cpu",
                url: None,
                sha256: None,
                size: None,
                build_commit: None,
            },
        ];
        let pin = pin_for(&pins, Some("macos-aarch64")).unwrap();
        assert_eq!((pin.size, pin.sha256.as_str()), (7, "ab"));
        // Named but not published: no pin.
        assert_eq!(pin_for(&pins, Some("linux-x86_64-cpu")), None);
        assert_eq!(pin_for(&pins, Some("linux-x86_64-cuda")), None);
        assert_eq!(pin_for(&pins, None), None);
        // Every published row in this build is complete and well formed.
        for pin in crate::release::PSIONIC_PINS {
            let Some(sha) = pin.sha256 else { continue };
            let url = pin.url.expect("a published pin names its URL");
            assert!(
                url.starts_with("https://") && url.contains(pin.platform),
                "{url}"
            );
            assert_eq!(sha.len(), 64);
            assert!(pin.size.is_some_and(|size| size > 0));
            assert!(pin.build_commit.is_some());
        }
    }

    #[tokio::test]
    async fn without_a_pin_setup_keeps_looking_for_a_built_engine() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = ensure_engine(None, Some("linux-x86_64-cpu"), dir.path(), |_| {})
            .await
            .unwrap();
        let EngineOutcome::NotPublished { platform, .. } = &outcome else {
            panic!("{outcome:?}");
        };
        assert_eq!(platform.as_deref(), Some("linux-x86_64-cpu"));
        assert!(outcome.describe().contains("not published"));
        assert!(!dir.path().join(SERVER).exists());
    }

    /// A one-file HTTP server that answers every request with `body`.
    async fn serve(body: &'static [u8]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body).await;
            }
        });
        format!("http://127.0.0.1:{port}/psionic/0.1.0/linux-x86_64-cpu/{SERVER}")
    }

    fn sha(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[tokio::test]
    async fn the_engine_takes_its_name_only_when_size_and_digest_match() {
        const BODY: &[u8] = b"#!/bin/sh\necho engine\n";
        let url = serve(BODY).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SERVER);
        let part = dir.path().join(format!("{SERVER}.part"));
        let pin = |sha256: String, size: u64| EnginePin {
            platform: "linux-x86_64-cpu".into(),
            url: url.clone(),
            sha256,
            size,
        };
        let size = BODY.len() as u64;

        // A wrong digest: refused, nothing left behind.
        let bad = pin("00".repeat(32), size);
        let err = ensure_engine(Some(&bad), None, dir.path(), |_| {})
            .await
            .unwrap_err();
        assert!(err.contains("not the pinned file"), "{err}");
        assert!(!path.exists() && !part.exists());

        // A wrong size with the right digest: refused too.
        let short = pin(sha(BODY), size + 1);
        assert!(
            ensure_engine(Some(&short), None, dir.path(), |_| {})
                .await
                .is_err()
        );
        assert!(!path.exists() && !part.exists());

        // The pinned build: in place, executable.
        let good = pin(sha(BODY), size);
        let outcome = ensure_engine(Some(&good), None, dir.path(), |_| {})
            .await
            .unwrap();
        assert_eq!(
            outcome,
            EngineOutcome::Installed {
                path: path.clone(),
                replaced: false
            }
        );
        assert_eq!(std::fs::read(&path).unwrap(), BODY);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{mode:o}");
        }
        assert_eq!(engine_matches(Some(&good), Some(&path)), Some(true));
        assert_eq!(engine_matches(Some(&bad), Some(&path)), Some(false));
        assert_eq!(engine_matches(None, Some(&path)), None);

        // Run again: nothing to do.
        assert_eq!(
            ensure_engine(Some(&good), None, dir.path(), |_| {})
                .await
                .unwrap(),
            EngineOutcome::AlreadyPinned { path: path.clone() }
        );

        // A different file under the name is replaced by the pinned one.
        std::fs::write(&path, b"an older engine").unwrap();
        assert_eq!(
            ensure_engine(Some(&good), None, dir.path(), |_| {})
                .await
                .unwrap(),
            EngineOutcome::Installed {
                path: path.clone(),
                replaced: true
            }
        );
        assert_eq!(std::fs::read(&path).unwrap(), BODY);
    }

    #[test]
    fn an_update_never_changes_the_served_model_without_the_owner() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let selfcheck = home.join(crate::calibrate::SELFCHECK_FILE);

        // First setup: nothing served before.
        assert_eq!(
            guard_model_change(home, None, "model-a", false).unwrap(),
            ModelChange::First
        );
        record_served_model(home, "model-a").unwrap();
        assert_eq!(served_model(home).as_deref(), Some("model-a"));

        // Same model (an engine-only update): goes ahead.
        assert_eq!(
            guard_model_change(home, Some("model-a"), "model-a", false).unwrap(),
            ModelChange::Same
        );

        // A new pinned model: refused, the self-check kept.
        std::fs::write(&selfcheck, "{}").unwrap();
        let err = guard_model_change(home, Some("model-a"), "model-b", false).unwrap_err();
        assert!(err.starts_with("needs the owner"), "{err}");
        assert!(err.contains(ACCEPT_MODEL_CHANGE), "{err}");
        assert!(selfcheck.exists());
        assert_eq!(served_model(home).as_deref(), Some("model-a"));

        // Accepted: the self-check is cleared for the new model.
        assert_eq!(
            guard_model_change(home, Some("model-a"), "model-b", true).unwrap(),
            ModelChange::Changed {
                from: "model-a".into(),
                to: "model-b".into()
            }
        );
        assert!(!selfcheck.exists());
    }

    #[test]
    fn without_a_record_the_model_on_disk_is_what_was_served() {
        assert_eq!(model_change(None, Some("a"), "a"), ModelChange::Same);
        assert_eq!(
            model_change(None, Some("a"), "b"),
            ModelChange::Changed {
                from: "a".into(),
                to: "b".into()
            }
        );
        // The record wins over the file.
        assert_eq!(model_change(Some("b"), Some("a"), "b"), ModelChange::Same);
        assert_eq!(model_change(None, None, "b"), ModelChange::First);
    }
}
