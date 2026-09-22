//! The manifest body — `v: 1` — and the typed probe answer it produces.
//!
//! The fields are the checked-in schema, extracted unchanged: `summary`
//! and `cost` are v1 fields, a refusal's text is the JSON `"match"`
//! string, and `concurrent_max` is optional. The additions are additive:
//! [`WorkspaceProbe::accepts`] defaults empty, and validation checks what
//! the old loader assumed — that every argv's head is the binary the
//! manifest names, and that no bound is both kept and admitted-ignored.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::bounded::Stop;
use crate::probe::{Presence, argv};
use crate::registry::{resolve, search_dirs};
use crate::{MANIFEST_VERSION, first_line, is_slug, same_binary, version_in};

/// What a capability can be asked to drive.
///
/// `subprocess` is the only transport this host can run: the manifest's
/// argvs are executed as processes under the supervisor. Any other
/// transport is a valid manifest but not a probeable one here — the probe
/// records `unprobed` rather than running a command line it does not
/// understand.
pub const SUBPROCESS: &str = "subprocess";

/// A capability whose adapter is a worker on the other side of a Nostr
/// relay. Nothing runs on this machine: the host publishes one NIP-CJ job
/// per task and the worker runs it under its own approval. A `relay`
/// manifest names no binary, and the probe that finds it is the host's
/// relay door rather than an argv, so this crate records it `unprobed`
/// and leaves the question to the host.
pub const RELAY: &str = "nostr-cj";

/// One capability manifest: how to drive one executor.
///
/// `slug` and `name` are the `d` and `name` tags the published
/// `kind:30180` carries; everything else is the body. The file on disk
/// holds them together so one document is one manifest.
#[derive(Clone, Debug, Serialize)]
pub struct Manifest {
    /// The body schema version.
    pub v: u32,
    /// The capability slug — the component of the qualified id, and the file stem.
    pub slug: String,
    /// The qualified definition id.
    pub qualified_id: String,
    /// `native`, `executor`, `plugin`, or `adapter`.
    pub profile: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// How the host speaks to it: `subprocess` or `relay`.
    pub transport: String,
    /// What resolves the executor on this machine. Empty for a `relay`
    /// manifest, which has nothing on this machine to detect.
    #[serde(default)]
    pub detect: Detect,
    /// Bounds the executor will hold to if given. A claim, not a proof:
    /// an approved manifest says this, and admission records it as a
    /// claim — the host's own enforcement is what a bound is proven by.
    #[serde(default)]
    pub enforces: Vec<String>,
    /// Bounds it will silently ignore. Stated positively because an
    /// omission is ambiguous and a refusal must not rest on one.
    #[serde(default)]
    pub cannot_enforce: Vec<String>,
    /// Whether the executor can read the caller's working directory.
    #[serde(default)]
    pub sees_repository: bool,
    /// The most simultaneous instances the manifest claims are safe.
    #[serde(default)]
    pub concurrent_max: Option<u32>,
    /// Who pays: `operator_account`, `metered`, `local`.
    #[serde(default)]
    pub cost: String,
    /// Which checkout shapes it accepts.
    #[serde(default)]
    pub isolation: Vec<String>,
    /// The argv that hands this executor a task, with the prompt appended
    /// as the final argument. Empty means a host can detect the executor
    /// and cannot drive it, which is a manifest that describes something
    /// without saying how to use it.
    #[serde(default)]
    pub invoke: Vec<String>,
    /// The argv that hands this executor a task that writes, when it
    /// differs from `invoke`. An executor that asks for confirmation
    /// before it edits or runs a command cannot be confirmed from a
    /// fan-out, so the manifest states the argv that lets it work
    /// unattended, and the host's filesystem boundary is the wall. Empty
    /// means a writing task runs `invoke` unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invoke_writing: Vec<String>,
    /// What to run in a candidate working directory to find out whether
    /// the executor will accept it.
    #[serde(default)]
    pub workspace_probe: Option<WorkspaceProbe>,
    /// What the executor declines, beyond being absent.
    #[serde(default)]
    pub refuses: Vec<Refusal>,
}

/// What a host runs to decide the executor is present and to read its
/// version. Each is a fixed argv, never a shell string.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Detect {
    pub binary: String,
    pub version: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<Vec<String>>,
}

/// The argv that asks the executor whether it will work in a directory.
///
/// It runs in the candidate workspace and is read for a declared
/// [`Refusal`]. Whether it exits zero is not the question: an argv that
/// stops short of doing any work is the right one, because a probe that
/// started a session would cost the operator something every time a host
/// looked. `accepts` is the typed version of the same idea: a probe that
/// exits non-zero by design declares the output word that proves the
/// workspace was accepted, so a silent non-zero exit reads as `unknown`
/// instead of being mistaken for either answer.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkspaceProbe {
    pub argv: Vec<String>,
    /// Output that proves the workspace was accepted — for a probe that
    /// exits non-zero either way, the word that only an accepted
    /// workspace reaches.
    #[serde(default)]
    pub accepts: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// One refusal the executor is known to answer with, and the text that
/// identifies it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Refusal {
    pub name: String,
    /// The text the executor prints when it refuses.
    #[serde(rename = "match")]
    pub matches: String,
    #[serde(default)]
    pub explanation: String,
}

/// Who holds a bound, read off the manifest rather than by convention.
///
/// The claim is the manifest's word for itself — the distinction the
/// runtime's admission check records. `Executor` means "the adapter says
/// it keeps this", and because the manifest is approved before it is
/// probed, the word is a statement by something the host chose to trust.
/// It is still a claim: approval lets the probe run; it does not verify
/// that the adapter holds the bound. `Ignored` is the honest refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    /// The adapter claims to keep this bound — the manifest's own list.
    Executor,
    /// The manifest admits it cannot keep this bound.
    Ignored,
    /// The manifest says nothing about this bound, so nothing verifies
    /// it. A required bound that lands here refuses, never passes
    /// silently.
    Unknown,
}

fn enforcement_plan(
    value: Option<&serde_json::Value>,
) -> Result<Vec<nostr::contracts::BoundAssignment>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| "binding.enforcement is not an array".to_string())?;
    items
        .iter()
        .map(|item| nostr::contracts::parse_bound(item).map_err(|error| error.to_string()))
        .collect()
}

fn manifest_from(
    definition: nostr::cap::Definition,
    binding: &serde_json::Map<String, serde_json::Value>,
) -> Result<Manifest, String> {
    for key in binding.keys() {
        if ![
            "name",
            "detect",
            "invoke",
            "invoke_writing",
            "workspace_probe",
            "refuses",
            "sees_repository",
            "concurrent_max",
            "cost",
            "claims_enforced",
            "claims_not_enforced",
            "enforcement",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unsupported binding field {key}"));
        }
    }
    let detect = match binding.get("detect") {
        None => Detect::default(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|error| error.to_string())?,
    };
    let invoke = strings(binding.get("invoke"))?;
    let invoke_writing = strings(binding.get("invoke_writing"))?;
    let workspace_probe = match binding.get("workspace_probe") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => {
            Some(serde_json::from_value(value.clone()).map_err(|error| error.to_string())?)
        }
    };
    let refuses = match binding.get("refuses") {
        None => Vec::new(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|error| error.to_string())?,
    };
    Ok(Manifest {
        v: crate::MANIFEST_VERSION,
        slug: definition.component,
        qualified_id: definition.id,
        profile: match definition.profile {
            nostr::cap::Profile::Native => "native",
            nostr::cap::Profile::Executor => "executor",
            nostr::cap::Profile::Plugin => "plugin",
            nostr::cap::Profile::Adapter => "adapter",
            nostr::cap::Profile::Service => {
                return Err("a service manifest is not an executor binding".to_string());
            }
        }
        .to_string(),
        name: binding
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(definition.summary.as_str())
            .to_string(),
        summary: definition.summary,
        transport: definition.transport,
        detect,
        enforces: strings(binding.get("claims_enforced"))?,
        cannot_enforce: strings(binding.get("claims_not_enforced"))?,
        sees_repository: binding
            .get("sees_repository")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        concurrent_max: binding
            .get("concurrent_max")
            .and_then(serde_json::Value::as_u64)
            .map(|value| u32::try_from(value).map_err(|_| "concurrent_max".to_string()))
            .transpose()?,
        cost: binding
            .get("cost")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        isolation: definition.isolation,
        invoke,
        invoke_writing,
        workspace_probe,
        refuses,
    })
}

/// A subprocess executor file for tests and fixtures.
///
/// `extra` may set `summary`, `transport`, `isolation`, `invoke`,
/// `invoke_writing`, `detect`, `workspace_probe`, `refuses`,
/// `sees_repository`, `cost`, `concurrent_max`, `enforces`, and
/// `cannot_enforce`. Those last two are claims on the host binding, not
/// proven enforcement.
#[must_use]
pub fn executor_document(
    slug: &str,
    binary: &str,
    version: Vec<String>,
    extra: serde_json::Value,
) -> serde_json::Value {
    let publisher = "a".repeat(64);
    let schema = serde_json::json!({
        "digest": "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0",
        "size": 17,
        "media_type": "application/schema+json"
    });
    let extra = extra.as_object().cloned().unwrap_or_default();
    let transport = extra
        .get("transport")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUBPROCESS);
    let mut contract = serde_json::json!({
        "interface": "task.v1",
        "transport": transport,
        "task": schema,
        "context": schema,
        "isolation": extra.get("isolation").cloned().unwrap_or_else(|| serde_json::json!([]))
    });
    if transport == "http" {
        contract["remote"] = serde_json::json!({"endpoint": "http://127.0.0.1/"});
    }
    if transport == RELAY {
        contract["remote"] = serde_json::json!({
            "worker": publisher,
            "relays": ["ws://127.0.0.1/"]
        });
    }
    let detect = if transport == RELAY {
        serde_json::json!({"binary": "", "version": []})
    } else {
        extra
            .get("detect")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"binary": binary, "version": version}))
    };
    let invoke = if transport == RELAY {
        serde_json::json!([])
    } else {
        extra
            .get("invoke")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([binary, "--"]))
    };
    serde_json::json!({
        "definition": {
            "v": 1,
            "requires": [],
            "id": format!("{publisher}:openagents/{slug}"),
            "profile": "executor",
            "summary": extra.get("summary").and_then(serde_json::Value::as_str).unwrap_or("a contract-test capability"),
            "input": schema,
            "output": schema,
            "effects": {"reads": ["workspace"], "writes": [], "network": [], "process": true, "delegates": false, "spend": false},
            "minimum": {},
            "support": {
                "bounds": {},
                "cancellation": "host_terminated",
                "idempotency": "none",
                "evidence": []
            },
            "binding_contract": contract
        },
        "binding": {
            "name": extra.get("name").cloned().unwrap_or_else(|| serde_json::json!(slug)),
            "detect": detect,
            "invoke": invoke,
            "invoke_writing": extra.get("invoke_writing").cloned().unwrap_or_else(|| serde_json::json!([])),
            "workspace_probe": extra.get("workspace_probe").cloned().unwrap_or(serde_json::Value::Null),
            "refuses": extra.get("refuses").cloned().unwrap_or_else(|| serde_json::json!([])),
            "sees_repository": extra.get("sees_repository").cloned().unwrap_or(serde_json::json!(true)),
            "concurrent_max": extra.get("concurrent_max").cloned().unwrap_or(serde_json::Value::Null),
            "cost": extra.get("cost").cloned().unwrap_or(serde_json::json!("local")),
            "claims_enforced": extra.get("enforces").cloned().unwrap_or_else(|| serde_json::json!([])),
            "claims_not_enforced": extra.get("cannot_enforce").cloned().unwrap_or_else(|| serde_json::json!([]))
        }
    })
}

fn strings(value: Option<&serde_json::Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| "expected an array of strings".to_string())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| "expected an array of strings".to_string())
        })
        .collect()
}

impl Claim {
    /// The word a trace or a check records.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Claim::Executor => "executor",
            Claim::Ignored => "ignored",
            Claim::Unknown => "unknown",
        }
    }
}

impl Manifest {
    /// Reads a manifest from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a manifest this
    /// version runs: unreadable, unparseable, a `v` it does not know, a
    /// slug outside the NIP-CAP grammar, a `detect` with no argv, or an
    /// argv whose head is not the binary `detect` names. A manifest a
    /// host half-understands drives an executor by a rule nobody stated.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&bytes).map_err(|reason| format!("{}: {reason}", path.display()))
    }

    /// Parses a NIP-CAP file: a `definition` plus a host `binding`.
    ///
    /// Root fields from the earlier executor draft (`slug`, `enforces`,
    /// `invoke`, and the rest) are refused. They are not read as authority.
    ///
    /// # Errors
    ///
    /// Returns the first reason the document is not a v1 capability file.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value = nostr::cap::parse_json(bytes).map_err(|error| error.to_string())?;
        let root = value
            .as_object()
            .ok_or_else(|| "capability file is not an object".to_string())?;
        for key in root.keys() {
            if key != "definition" && key != "binding" {
                return Err(format!(
                    "unsupported field {key}: earlier executor fields are not a capability definition"
                ));
            }
        }
        let definition = nostr::cap::parse_definition(
            root.get("definition")
                .ok_or_else(|| "capability file has no definition".to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let binding = root
            .get("binding")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "capability file has no host binding".to_string())?;
        let plan = enforcement_plan(binding.get("enforcement"))?;
        nostr::cap::cover(&definition, &plan).map_err(|error| error.to_string())?;
        let manifest = manifest_from(definition, binding)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The manifest body as a JSON string.
    ///
    /// # Errors
    ///
    /// Fails only if the manifest cannot be serialized, which a loaded
    /// manifest always can.
    pub fn json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Reads the event form of this manifest.
    ///
    /// # Errors
    ///
    /// Returns the parse error if the content is not a manifest.
    pub fn event(content: &str) -> Result<Self, String> {
        Self::parse(content.as_bytes())
    }

    /// Whether this manifest is one this version runs.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != MANIFEST_VERSION {
            return Err(format!(
                "body version is {}, this version reads {MANIFEST_VERSION}",
                self.v
            ));
        }
        if !is_slug(&self.slug) {
            return Err(format!("slug {:?} is not a capability slug", self.slug));
        }
        if self.transport.is_empty() {
            return Err("no transport".to_string());
        }
        if self.transport == RELAY || self.profile == "native" || self.profile == "plugin" {
            if !self.detect.binary.is_empty() || !self.detect.version.is_empty() {
                return Err("a relay manifest detects nothing on this machine".to_string());
            }
            if !self.invoke.is_empty() || !self.invoke_writing.is_empty() {
                return Err("a relay manifest runs no argv here".to_string());
            }
            if self.workspace_probe.is_some() {
                return Err("a relay manifest probes no workspace here".to_string());
            }
            return self.check_claims();
        }
        if self.detect.binary.is_empty() || self.detect.binary.contains('\0') {
            return Err("detect names no binary".to_string());
        }
        if self.detect.version.is_empty() {
            return Err("detect carries no version argv".to_string());
        }
        self.check_argv(&self.detect.version, "detect.version")?;
        if let Some(probe) = &self.detect.probe {
            self.check_argv(probe, "detect.probe")?;
        }
        if let Some(probe) = &self.workspace_probe {
            self.check_argv(&probe.argv, "workspace_probe.argv")?;
        }
        if !self.invoke.is_empty() {
            self.check_argv(&self.invoke, "invoke")?;
        }
        if !self.invoke_writing.is_empty() {
            if self.invoke.is_empty() {
                return Err("invoke_writing without invoke drives nothing".to_string());
            }
            self.check_argv(&self.invoke_writing, "invoke_writing")?;
        }
        self.check_claims()
    }

    /// A bound is kept or admitted-ignored, never both.
    fn check_claims(&self) -> Result<(), String> {
        let claims: BTreeSet<&String> = self.enforces.iter().collect();
        let overlap: Vec<&str> = self
            .cannot_enforce
            .iter()
            .filter(|bound| claims.contains(*bound))
            .map(String::as_str)
            .collect();
        if !overlap.is_empty() {
            return Err(format!(
                "{}: a bound cannot be both kept and admitted-ignored",
                overlap.join(", ")
            ));
        }
        Ok(())
    }

    /// An argv's head must be the binary the manifest names — the probe
    /// resolves the binary and runs it with the argv's tail, so a head
    /// that disagrees names a different program.
    fn check_argv(&self, argv: &[String], field: &str) -> Result<(), String> {
        match argv.first() {
            Some(head) if same_binary(&self.detect.binary, head) => Ok(()),
            Some(head) => Err(format!(
                "{field} starts with {head:?}, not the binary {:?} detect resolves",
                self.detect.binary
            )),
            None => Err(format!("{field} carries no argv")),
        }
    }

    /// The bounds this executor would silently ignore, out of the ones a
    /// delegation needs. An empty answer admits the delegation.
    ///
    /// The intersection with `cannot_enforce` is the whole test. An
    /// executor that ignores a bound is more dangerous than one that
    /// refuses it, so a host refuses the pairing rather than issuing it
    /// and hoping.
    #[must_use]
    pub fn ignored_bounds(&self, required: &[String]) -> Vec<String> {
        required
            .iter()
            .filter(|bound| self.cannot_enforce.contains(bound))
            .cloned()
            .collect()
    }

    /// Who the manifest says holds `bound`: the adapter claims it,
    /// nobody, or — the honest case — it does not say. A claim is what
    /// an approval records; it is not proof the bound is held.
    #[must_use]
    pub fn claim(&self, bound: &str) -> Claim {
        if self.cannot_enforce.iter().any(|held| held == bound) {
            Claim::Ignored
        } else if self.enforces.iter().any(|held| held == bound) {
            Claim::Executor
        } else {
            Claim::Unknown
        }
    }

    /// Probes this machine for the executor, asking about one workspace.
    ///
    /// Never returns an error. Every outcome a probe can have is one of
    /// the states, and a caller that has to handle one more would start
    /// treating absence as one.
    ///
    /// This is the caller building the manifest and running it, so the
    /// [`Found`](crate::Found) it returns carries
    /// [`Proof::Unconditional`](crate::Proof::Unconditional) — a caller's
    /// own manifest needs no registry decision. Registry probing under a
    /// trust policy is [`Entry::probe`](crate::Entry::probe).
    #[must_use]
    pub fn probe(&self, workspace: &Path) -> crate::Found {
        self.probe_within(workspace, crate::probe::PROBE_WALL)
    }

    /// Probes with a shorter wall clock, for a test that wants a timeout
    /// without the wait.
    #[must_use]
    pub fn probe_within(&self, workspace: &Path, wall: Duration) -> crate::Found {
        let started = std::time::Instant::now();
        crate::Found {
            manifest: self.clone(),
            presence: self.presence(workspace, wall, true),
            workspace: workspace.to_path_buf(),
            milliseconds: started.elapsed().as_millis() as u64,
            proof: crate::Proof::Unconditional,
            source: crate::Source::Operator,
            digest: self.digest(),
            path: PathBuf::new(),
        }
    }

    /// The digest of the manifest as a JSON string — a stable identity
    /// for a manifest that did not come from a file.
    #[must_use]
    pub fn digest(&self) -> String {
        crate::trust::digest_bytes(&serde_json::to_vec(self).unwrap_or_default())
    }

    /// Runs the probe and classifies the answer.
    ///
    /// The typed contract, in order:
    ///
    /// - The binary does not resolve — absent.
    /// - The binary resolves but will not run — absent.
    /// - The version argv does not answer inside the wall clock, exits
    ///   wrong, or answers past the output cap — unknown: the executor
    ///   is there and did not prove itself.
    /// - The version answer carries no version — absent: what answered
    ///   is not the thing the manifest describes.
    /// - No workspace probe, or a version-only [`detect`](crate::Entry::detect)
    ///   — present. A manifest without one means the executor accepts
    ///   every workspace, so the question need not be asked.
    /// - The workspace probe names a declared refusal — unavailable:
    ///   present, and this workspace is one it will not work.
    /// - The workspace probe names a declared acceptance, or exits
    ///   clean — present.
    /// - Anything else — unknown: an exit the manifest did not declare
    ///   cannot be read as either answer.
    pub(crate) fn presence(
        &self,
        workspace: &Path,
        wall: Duration,
        ask_workspace: bool,
    ) -> Presence {
        let dirs = search_dirs();
        let Some(path) = resolve(&self.detect.binary, &dirs) else {
            return Presence::Absent {
                reason: format!(
                    "no {} in {} directories on this machine",
                    self.detect.binary,
                    dirs.len()
                ),
                looked_in: dirs,
            };
        };
        let said = match argv(&path, &self.detect.version, workspace, wall) {
            Ok(said) => said,
            Err(Stop::TimedOut) => {
                return Presence::Unknown {
                    reason: format!("{} gave no answer in {wall:?}", path.display()),
                    version: None,
                    path: Some(path),
                };
            }
            Err(Stop::Failed(why)) => {
                return Presence::Absent {
                    reason: format!("{} will not run: {why}", path.display()),
                    looked_in: dirs,
                };
            }
        };
        if said.truncated {
            return Presence::Unknown {
                reason: format!(
                    "{}'s version answer overran the output bound",
                    path.display()
                ),
                version: None,
                path: Some(path),
            };
        }
        if said.code != Some(0) {
            return Presence::Unknown {
                reason: format!("{}'s version argv {}", path.display(), exit_word(said.code)),
                version: None,
                path: Some(path),
            };
        }
        let report = first_line(&format!("{}{}", said.out, said.err)).to_string();
        let Some(version) = version_in(&report) else {
            return Presence::Absent {
                reason: format!(
                    "{} printed {report:?}, which carries no version",
                    path.display()
                ),
                looked_in: dirs,
            };
        };
        let Some(probe) = self.workspace_probe.as_ref().filter(|_| ask_workspace) else {
            return Presence::Present {
                version,
                report,
                path,
            };
        };
        match argv(&path, &probe.argv, workspace, wall) {
            Err(Stop::TimedOut) => Presence::Unknown {
                reason: format!("the workspace probe gave no answer in {wall:?}"),
                version: Some(version),
                path: Some(path),
            },
            Err(Stop::Failed(why)) => Presence::Unknown {
                reason: format!("the workspace probe would not run: {why}"),
                version: Some(version),
                path: Some(path),
            },
            Ok(said) => {
                if said.truncated {
                    return Presence::Unknown {
                        reason: "the workspace probe's answer overran the output bound".to_string(),
                        version: Some(version),
                        path: Some(path),
                    };
                }
                let answer = format!("{}{}", said.out, said.err);
                for refusal in &self.refuses {
                    if answer.contains(&refusal.matches) {
                        return Presence::Unavailable {
                            version,
                            report,
                            path,
                            refusal: refusal.name.clone(),
                            detail: first_line(&answer).to_string(),
                        };
                    }
                }
                if probe.accepts.iter().any(|word| answer.contains(word)) || said.code == Some(0) {
                    return Presence::Present {
                        version,
                        report,
                        path,
                    };
                }
                Presence::Unknown {
                    reason: format!(
                        "the workspace probe {} without a declared word",
                        exit_word(said.code)
                    ),
                    version: Some(version),
                    path: Some(path),
                }
            }
        }
    }
}

/// How an exited argv ended, in words a reason can carry.
fn exit_word(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("exited {code}"),
        None => "died on a signal".to_string(),
    }
}
