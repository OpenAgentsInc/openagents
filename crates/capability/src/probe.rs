//! The probe: a bounded run of the manifest's own argvs, and the typed
//! answer it returns.
//!
//! The five states are the questions a survey asks, kept apart because
//! they mean different things to the host that asked: `present` is a
//! route, `absent` is a shorter list, `present_unavailable` is an
//! executor's own refusal, `unprobed` is a manifest nobody approved, and
//! `unknown` is an answer the host could not read. Collapsing any two of
//! them manufactures a route or hides a refusal.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{Map, json};

use crate::bounded::{self, Said, Stop};
use crate::registry::Source;
use crate::trust::Proof;
use crate::{Manifest, PROBE_CALL};

/// The most time a probe is allowed to take. A probe answers quickly or
/// it answers `unknown`, because a probe that hangs is not a probe that
/// found anything.
pub const PROBE_WALL: Duration = Duration::from_secs(10);

/// What a probe found.
///
/// The variants are deliberately not `Option<Something>`: absence and
/// refusal are different answers with different consequences, and a type
/// that collapses them makes the mistake this module exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// `detect` resolved and the version parsed, and a declared
    /// workspace probe accepted.
    Present {
        /// The version as it parsed — `3000.10.31`.
        version: String,
        /// The first line the version command printed, whole.
        report: String,
        /// The absolute path the host will run.
        path: PathBuf,
    },
    /// Nothing to run. Not an error.
    Absent {
        /// Why there is nothing to run, in a sentence.
        reason: String,
        /// Where the host looked, so an operator who expected it can see
        /// which directory to put it in.
        looked_in: Vec<PathBuf>,
    },
    /// Installed, detected, and refusing this context.
    Unavailable {
        /// The version the version argv reported.
        version: String,
        /// The line the version command printed.
        report: String,
        /// The resolved binary.
        path: PathBuf,
        /// The declared refusal's name.
        refusal: String,
        /// What the executor actually printed.
        detail: String,
    },
    /// No probe ran, because no host decision approves this manifest's
    /// argv. The capability is declared, not found.
    Unprobed {
        /// Why nothing ran, in words an operator can act on.
        reason: String,
    },
    /// A probe ran and could not be read: it timed out, exited wrong,
    /// or answered past the output bound or with no declared word. The
    /// capability may be there; the probe cannot say, and `unknown` is
    /// not a route.
    Unknown {
        /// Why the answer could not be read.
        reason: String,
        /// The version, when the version argv had already answered.
        version: Option<String>,
        /// The resolved binary, when it resolved.
        path: Option<PathBuf>,
    },
}

impl Presence {
    /// Whether a host may offer this capability as a route.
    ///
    /// Every other state answers `false`, and for the same practical
    /// reason: a route that cannot be taken is not a route.
    #[must_use]
    pub fn available(&self) -> bool {
        matches!(self, Presence::Present { .. })
    }

    /// Whether the executor is on the machine at all, refusing or not.
    /// `unprobed` answers false — the manifest is declared, not found.
    #[must_use]
    pub fn installed(&self) -> bool {
        matches!(
            self,
            Presence::Present { .. } | Presence::Unavailable { .. } | Presence::Unknown { .. }
        )
    }

    /// The state's name, as a trace records it.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match self {
            Presence::Present { .. } => "present",
            Presence::Absent { .. } => "absent",
            Presence::Unavailable { .. } => "present_unavailable",
            Presence::Unprobed { .. } => "unprobed",
            Presence::Unknown { .. } => "unknown",
        }
    }
}

/// One capability, probed on one machine in one workspace, with the
/// approval that let the probe run.
#[derive(Clone, Debug)]
pub struct Found {
    /// The manifest the probe read.
    pub manifest: Manifest,
    /// What it found.
    pub presence: Presence,
    /// The directory the probe asked about.
    pub workspace: PathBuf,
    /// Wall time the probe took.
    pub milliseconds: u64,
    /// The host decision the probe ran under — `none` means it did not
    /// run.
    pub proof: Proof,
    /// Which kind of directory the manifest came from.
    pub source: Source,
    /// The digest of the manifest file's exact bytes, when there is one.
    pub digest: String,
    /// The manifest file itself, when there is one.
    pub path: PathBuf,
}

impl Found {
    /// The capability slug.
    #[must_use]
    pub fn capability(&self) -> &str {
        &self.manifest.slug
    }

    /// Whether a host may offer this capability as a route.
    #[must_use]
    pub fn available(&self) -> bool {
        self.presence.available()
    }

    /// The sentence a surface shows beside the probe.
    #[must_use]
    pub fn message(&self) -> String {
        let slug = &self.manifest.slug;
        match &self.presence {
            Presence::Present { report, .. } => format!("{slug} is present: {report}."),
            Presence::Absent { reason, .. } => format!("{slug} is absent: {reason}."),
            Presence::Unavailable { refusal, .. } => {
                format!("{slug} is present and unavailable here: {refusal}.")
            }
            Presence::Unprobed { reason } => format!("{slug} was not probed: {reason}."),
            Presence::Unknown { reason, .. } => {
                format!("{slug} could not be established: {reason}.")
            }
        }
    }

    /// What the probe answered, in one line.
    #[must_use]
    pub fn output(&self) -> String {
        match &self.presence {
            Presence::Present { report, path, .. } => format!("{report} at {}", path.display()),
            Presence::Absent { reason, .. } => reason.clone(),
            Presence::Unavailable {
                report,
                path,
                refusal,
                ..
            } => format!(
                "{report} at {}, refusing this workspace: {refusal}",
                path.display()
            ),
            Presence::Unprobed { reason } => format!("unprobed: {reason}"),
            Presence::Unknown { reason, .. } => format!("unknown: {reason}"),
        }
    }

    /// The probe as a trace records it.
    ///
    /// The outcome is `Completed` in every state, including absence and
    /// refusal: the host asked and got an answer, and a capability that
    /// is not installed is not a fault in the trace. `trusted_by` names
    /// the proof source — the approval that let the probe run — so a
    /// reader can tell a probed capability from a declared one.
    #[must_use]
    pub fn call(&self) -> atif::Call {
        let manifest = &self.manifest;
        let mut extra = Map::new();
        extra.insert("capability".to_string(), json!(manifest.slug));
        extra.insert("state".to_string(), json!(self.presence.state()));
        extra.insert("present".to_string(), json!(self.presence.installed()));
        extra.insert("available".to_string(), json!(self.presence.available()));
        extra.insert("transport".to_string(), json!(manifest.transport));
        extra.insert("enforces".to_string(), json!(manifest.enforces));
        extra.insert("cannot_enforce".to_string(), json!(manifest.cannot_enforce));
        extra.insert(
            "sees_repository".to_string(),
            json!(manifest.sees_repository),
        );
        extra.insert(
            "refuses".to_string(),
            json!(
                manifest
                    .refuses
                    .iter()
                    .map(|refusal| refusal.name.clone())
                    .collect::<Vec<_>>()
            ),
        );
        extra.insert(
            "workspace".to_string(),
            json!(self.workspace.display().to_string()),
        );
        extra.insert("trusted_by".to_string(), json!(self.proof.word()));
        extra.insert("manifest_source".to_string(), json!(self.source.word()));
        if !self.digest.is_empty() {
            extra.insert("manifest_digest".to_string(), json!(self.digest));
        }
        if !self.path.as_os_str().is_empty() {
            extra.insert(
                "manifest".to_string(),
                json!(self.path.display().to_string()),
            );
        }
        match &self.presence {
            Presence::Present { version, path, .. } => {
                extra.insert("version".to_string(), json!(version));
                extra.insert(
                    "resolved_path".to_string(),
                    json!(path.display().to_string()),
                );
            }
            Presence::Absent { reason, looked_in } => {
                extra.insert("reason".to_string(), json!(reason));
                extra.insert(
                    "looked_in".to_string(),
                    json!(
                        looked_in
                            .iter()
                            .map(|dir| dir.display().to_string())
                            .collect::<Vec<_>>()
                    ),
                );
            }
            Presence::Unavailable {
                version,
                path,
                refusal,
                detail,
                ..
            } => {
                extra.insert("version".to_string(), json!(version));
                extra.insert(
                    "resolved_path".to_string(),
                    json!(path.display().to_string()),
                );
                let explanation = manifest
                    .refuses
                    .iter()
                    .find(|declared| &declared.name == refusal)
                    .map(|declared| declared.explanation.clone())
                    .unwrap_or_default();
                extra.insert(
                    "refused".to_string(),
                    json!({
                        "name": refusal,
                        "detail": detail,
                        "explanation": explanation,
                        "workspace": self.workspace.display().to_string(),
                    }),
                );
            }
            Presence::Unprobed { reason } => {
                extra.insert("reason".to_string(), json!(reason));
            }
            Presence::Unknown {
                reason,
                version,
                path,
            } => {
                extra.insert("reason".to_string(), json!(reason));
                if let Some(version) = version {
                    extra.insert("version".to_string(), json!(version));
                }
                if let Some(path) = path {
                    extra.insert(
                        "resolved_path".to_string(),
                        json!(path.display().to_string()),
                    );
                }
            }
        }
        atif::Call {
            id: String::new(),
            name: PROBE_CALL.to_string(),
            arguments: json!({ "detect": manifest.detect }),
            output: self.output(),
            outcome: atif::Outcome::Completed,
            milliseconds: self.milliseconds,
            purpose: Some(format!(
                "Resolve {} before offering it as a route.",
                manifest.slug
            )),
            extra,
        }
    }
}

/// Runs `argv` — whose first word names the binary again — with `path`
/// as the resolved binary, inside `workspace`, under the bounded run.
///
/// The head is dropped because `path` is already the resolution of it:
/// `detect.binary` is what resolved, and `argv`'s tail is what runs.
pub(crate) fn argv(
    path: &Path,
    argv: &[String],
    workspace: &Path,
    wall: Duration,
) -> Result<Said, Stop> {
    let mut command = Command::new(path);
    command.args(&argv[1..]).current_dir(workspace);
    bounded::run(command, wall)
}
