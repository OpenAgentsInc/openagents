//! The document that says what a decision model is, and what it may serve.
//!
//! Serving an adapter used to take four checks in three binaries, and none of
//! them was keyed off a single document. `lev-adapter-check` read the package
//! and confirmed the rank and the base signature. `lev-serve` pinned the
//! package at startup and exited on a mismatch. The per-family admission rule
//! read a calibration record. The estimator and the sample count were flags.
//! Nothing tied those together, and nothing recorded them as one artifact
//! identity.
//!
//! The cost of that absence is on disk: three adapters distinguished only by
//! the run directory they were written to, `creatorDefined` empty in all
//! three, and a calibration map fitted against the base model that survived
//! two adapter runs because no record could name which model it was fitted
//! against. A manifest is the name that was missing.
//!
//! # What it carries
//!
//! - [`Artifact`] — where the bytes are, what they hash to, how large they
//!   are, the format, and the rank. Absent for a door that serves no
//!   artifact, because a base model shipped by the operating system has no
//!   bytes to pin.
//! - [`Base`] — the base model signature the artifact is trained against, and
//!   the operating system build it has been checked under.
//! - [`Interface`] — the System One input and output shapes. Checked against
//!   the contract this crate implements rather than trusted, so a manifest
//!   that drifts from the code refuses.
//! - The estimator and its sample count, in the same shape a calibration
//!   record carries them, so the two can be compared field for field.
//! - [`EvalRef`] — the committed calibration records this model was measured
//!   against, each with the record's own `admitted` flag and verdict.
//! - [`crate::policy::SnapshotRef`] — the canonical service whose snapshot
//!   says whether this release may still serve, where the door keeps its
//!   copy, and the longest the release accepts running on one. Required: a
//!   release that could decline to name a policy source would escape
//!   revocation by leaving a field out.
//!
//! # The admission rule
//!
//! `docs/kev/mesh-plan.md` drafted this schema for decision-model artifacts
//! and set the rule that a row without a measured `evalRef` does not admit.
//! [`Manifest::admits`] is that rule: a family with no `evalRef` entry, or an
//! entry the gate refused, serves no probability. Nothing defaults to
//! admitted, and an empty `evalRef` list admits nothing at all.
//!
//! # Where the last issue attaches
//!
//! A behavioral admission floor (#9389) is a stricter reading of
//! [`Manifest::admits`]: the same document, with the isolation probe's result
//! joining the calibration record in `evalRef`. It does not need the schema
//! reshaped to land.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use gym::calibrate::{EstimatorConfig, Record};

use crate::adapter::{METADATA_FILE, Package, WEIGHTS_FILE};
use crate::api::{MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, MIN_CHOICE_OPTIONS, MIN_SCORE_LEVELS};

/// The schema tag every manifest carries.
///
/// Written into the file rather than inferred from its path, for the reason a
/// calibration record carries its own tag: a document that escapes its
/// directory still says what it is.
pub const MANIFEST_SCHEMA: &str = "openagents.lev.decision_model_manifest.v1";

/// The contract a manifest's interface describes.
pub const CONTRACT: &str = "openagents.systemone.v1";

/// The endpoint that answers it.
pub const ENDPOINT: &str = "POST /v1/systemone";

/// The only artifact format this crate attaches.
pub const FMADAPTER: &str = "fmadapter";

/// Why a manifest may not be used.
///
/// Each value names one field. "The manifest does not check out" is not an
/// answer anybody can act on, and a deployment error should say which claim
/// the artifact failed to keep.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Fault {
    /// The document is tagged as something other than a manifest.
    #[error("schema: the document is tagged {found}, not {MANIFEST_SCHEMA}")]
    Schema {
        /// The tag the file carried.
        found: String,
    },
    /// The file did not read, or did not parse.
    #[error("{path}: {reason}")]
    Unreadable {
        /// What was being read.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// A required field is blank.
    #[error("{field} is blank")]
    Blank {
        /// The field.
        field: &'static str,
    },
    /// The declared interface is not the contract this crate implements.
    #[error(
        "interface: the manifest declares {found} and this build of the contract is {expected}"
    )]
    Interface {
        /// What the manifest declared.
        found: String,
        /// What the code implements.
        expected: String,
    },
    /// The manifest names no artifact, or the package did not open.
    #[error("artifact: {reason}")]
    Artifact {
        /// What is missing.
        reason: String,
    },
    /// A field the package declares is not the field the manifest declares.
    #[error("artifact.{field}: the manifest says {declared} and the package says {found}")]
    Package {
        /// The field that disagrees.
        field: &'static str,
        /// What the manifest recorded.
        declared: String,
        /// What the package on disk holds.
        found: String,
    },
    /// The base model underneath is not the one the artifact is pinned to.
    #[error("base.signature: the manifest is pinned to {declared} and this device runs {found}")]
    Base {
        /// The signature the manifest declares.
        declared: String,
        /// The signature, or signature prefix, the device reports.
        found: String,
    },
    /// A calibration record the manifest names is not the record on disk.
    #[error("evalRef[{family}].{field}: the manifest says {declared} and the record says {found}")]
    EvalRef {
        /// The family the entry covers.
        family: String,
        /// The field that disagrees.
        field: &'static str,
        /// What the manifest recorded.
        declared: String,
        /// What the record holds now.
        found: String,
    },
}

/// Where the bytes are and what they have to be.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Where the package lives. A leading `~` is the running user's home.
    ///
    /// The path is machine-local, and the manifest says so by carrying a
    /// digest beside it: the path finds the bytes, and the digest decides
    /// whether they are the right ones.
    pub path: String,
    /// The package layout. `fmadapter` is the only one this crate attaches.
    pub format: String,
    /// The digest of `adapter_weights.bin`.
    ///
    /// The weights are what the runtime loads, so they are what "the bytes"
    /// means. Hashing the directory would need a canonical ordering this
    /// repository does not otherwise define, and would hide which file
    /// changed.
    pub sha256: String,
    /// The size of `adapter_weights.bin`.
    pub size_bytes: u64,
    /// The digest of `metadata.json`, which the runtime also reads.
    pub metadata_sha256: String,
    /// The LoRA rank the package declares.
    pub lora_rank: u32,
    /// The stable label the package declares.
    pub adapter_identifier: String,
}

impl Artifact {
    /// Reads a package and describes it.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Unreadable`] when a file in the package does not
    /// read.
    pub fn of_package(package: &Package) -> Result<Self, Fault> {
        let weights = package.path.join(WEIGHTS_FILE);
        let metadata = package.path.join(METADATA_FILE);
        Ok(Self {
            path: package.path.display().to_string(),
            format: FMADAPTER.to_string(),
            sha256: digest_of(&weights)?,
            size_bytes: size_of(&weights)?,
            metadata_sha256: digest_of(&metadata)?,
            lora_rank: package.metadata.lora_rank,
            adapter_identifier: package.metadata.adapter_identifier.clone(),
        })
    }

    /// The package path with a leading `~` expanded.
    #[must_use]
    pub fn resolved_path(&self) -> PathBuf {
        expand(&self.path)
    }

    /// Whether the package on disk is the one this manifest describes.
    ///
    /// # Errors
    ///
    /// Returns the first [`Fault::Package`] found, which names the field.
    pub fn check(&self, package: &Package) -> Result<(), Fault> {
        let found = Self::of_package(package)?;
        let compare = |field: &'static str, declared: String, found: String| {
            if declared == found {
                Ok(())
            } else {
                Err(Fault::Package {
                    field,
                    declared,
                    found,
                })
            }
        };
        compare("format", self.format.clone(), found.format)?;
        compare(
            "adapterIdentifier",
            self.adapter_identifier.clone(),
            found.adapter_identifier,
        )?;
        compare(
            "loraRank",
            self.lora_rank.to_string(),
            found.lora_rank.to_string(),
        )?;
        compare(
            "sizeBytes",
            self.size_bytes.to_string(),
            found.size_bytes.to_string(),
        )?;
        compare(
            "metadataSha256",
            self.metadata_sha256.clone(),
            found.metadata_sha256,
        )?;
        compare("sha256", self.sha256.clone(), found.sha256)
    }
}

/// What the artifact sits on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Base {
    /// The base model signature, 40 lowercase hex characters.
    ///
    /// The base ships with the operating system, so an update replaces it and
    /// invalidates every adapter and every calibration map fitted against the
    /// old one. That is a revocation event, and this field is what a
    /// revocation would name.
    pub signature: String,
    /// The operating system build the artifact has been checked under.
    ///
    /// The earliest build this model has been attached and measured on, not a
    /// supported range: one build has been observed, and the field says only
    /// that.
    pub min_os_build: String,
    /// The runtime that holds the base.
    pub runtime: String,
}

impl Base {
    /// Whether a running device may attach this artifact.
    ///
    /// The runtime publishes a signature prefix rather than the whole
    /// signature, so a prefix match counts. A mismatch is a deployment error:
    /// serving an adapter trained against a base the device no longer runs
    /// produces confident nonsense.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Base`], naming both signatures.
    pub fn check(&self, running: &str) -> Result<(), Fault> {
        if !running.is_empty() && self.signature.starts_with(running) {
            return Ok(());
        }
        Err(Fault::Base {
            declared: self.signature.clone(),
            found: name_or_none(running),
        })
    }
}

/// One question type the contract admits, and its bounds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionShape {
    /// The fewest criteria the type admits, when it bounds them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_criteria: Option<usize>,
    /// The most criteria the type admits, when it bounds them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_criteria: Option<usize>,
    /// The fields the answer carries, in the order the wire writes them.
    pub answer: Vec<String>,
}

/// The System One input and output shapes this model answers.
///
/// Declared in the manifest and checked against [`Interface::of_contract`]
/// rather than trusted. A manifest is a grant, and a grant that describes an
/// interface the code no longer implements grants nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    /// The contract's name.
    pub contract: String,
    /// The endpoint that answers it.
    pub endpoint: String,
    /// One entry per question type, keyed by its wire label.
    pub questions: BTreeMap<String, QuestionShape>,
    /// The response extensions a caller may ask for.
    pub extensions: Vec<String>,
    /// The question families this model was trained and measured on.
    ///
    /// The only part of the interface that is a property of this model rather
    /// than of the contract, and so the only part [`Interface::check`] does
    /// not compare.
    pub families: Vec<String>,
}

impl Interface {
    /// The contract this build implements, over the given families.
    #[must_use]
    pub fn of_contract(families: Vec<String>) -> Self {
        let shape = |min: Option<usize>, max: Option<usize>, answer: &[&str]| QuestionShape {
            min_criteria: min,
            max_criteria: max,
            answer: answer.iter().map(|field| (*field).to_string()).collect(),
        };
        let mut questions = BTreeMap::new();
        questions.insert("noul".to_string(), shape(None, None, &["noul"]));
        questions.insert(
            "choice".to_string(),
            shape(
                Some(MIN_CHOICE_OPTIONS),
                Some(MAX_CHOICE_OPTIONS),
                &["choice", "confidence", "probabilities"],
            ),
        );
        questions.insert(
            "score".to_string(),
            shape(
                Some(MIN_SCORE_LEVELS),
                Some(MAX_SCORE_LEVELS),
                &["score", "confidence", "legend", "probabilities"],
            ),
        );
        Self {
            contract: CONTRACT.to_string(),
            endpoint: ENDPOINT.to_string(),
            questions,
            extensions: ["estimator", "require_calibration", "family"]
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            families,
        }
    }

    /// Whether the declared interface is the one this build implements.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Interface`] carrying both shapes, because the useful
    /// thing to show a reader is the difference.
    pub fn check(&self) -> Result<(), Fault> {
        let expected = Self::of_contract(self.families.clone());
        if *self == expected {
            return Ok(());
        }
        Err(Fault::Interface {
            found: brief(self),
            expected: brief(&expected),
        })
    }
}

fn brief(interface: &Interface) -> String {
    serde_json::to_string(interface).unwrap_or_else(|_| "an interface that does not encode".into())
}

/// The measurement a family's admission rests on.
///
/// The record is committed, so the claim is checkable rather than quoted: the
/// entry carries the record's digest and repeats its `admitted` flag and
/// verdict, and [`EvalRef::check`] reads the file and compares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRef {
    /// The question family the record covers.
    pub family: String,
    /// Where the record lives, relative to the manifest's own directory.
    pub record: String,
    /// The record file's digest.
    pub sha256: String,
    /// The suite it was fitted and scored on.
    pub suite: String,
    /// That suite's content digest.
    pub suite_digest: String,
    /// The partition the map was fitted on.
    pub partition_id: String,
    /// The acceptance rule that judged it.
    pub gate_id: String,
    /// Whether the gate admitted it.
    pub admitted: bool,
    /// Why, in the gate's own words.
    pub verdict: String,
}

impl EvalRef {
    /// Describes a committed record.
    #[must_use]
    pub fn of_record(record: &Record, path: impl Into<String>, sha256: impl Into<String>) -> Self {
        Self {
            family: record.family.clone(),
            record: path.into(),
            sha256: sha256.into(),
            suite: record.suite.clone(),
            suite_digest: record.suite_digest.clone(),
            partition_id: record.partition_id.clone(),
            gate_id: record.gate_id.clone().unwrap_or_default(),
            admitted: record.admitted,
            verdict: record.verdict.clone(),
        }
    }

    /// Whether the record on disk is the one this entry describes.
    ///
    /// `beside` is the manifest's own directory, which the `record` path is
    /// relative to.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Unreadable`] when the record does not read, and the
    /// first [`Fault::EvalRef`] otherwise, which names the field.
    pub fn check(&self, beside: &Path) -> Result<(), Fault> {
        self.check_fitted_against(beside, None)
    }

    /// Whether the record is the one this entry describes, and whether it was
    /// fitted against the door `adapter` names.
    ///
    /// This is the check that made the emptiness rule unnecessary. Until
    /// 2026-09-19 no calibration map had been fitted against an adapter, so
    /// "an adapted release names no measurement" stood in for "an adapted
    /// release names no measurement fitted against something else" — which is
    /// the rule actually worth keeping, and the one a first adapted record
    /// turned from a tautology into a test.
    ///
    /// `adapter` is the release id an adapted door publishes, or `None` for a
    /// base release, whose records carry no adapter at all.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Unreadable`] when the record does not read, and the
    /// first [`Fault::EvalRef`] otherwise, which names the field.
    pub fn check_fitted_against(&self, beside: &Path, adapter: Option<&str>) -> Result<(), Fault> {
        let path = beside.join(&self.record);
        let digest = digest_of(&path)?;
        if digest != self.sha256 {
            return Err(Fault::EvalRef {
                family: self.family.clone(),
                field: "sha256",
                declared: self.sha256.clone(),
                found: digest,
            });
        }
        let text = std::fs::read_to_string(&path).map_err(|error| Fault::Unreadable {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let record = Record::from_json(&text).map_err(|error| Fault::Unreadable {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let declared = adapter.unwrap_or_default();
        if record.door_identity.adapter != declared {
            return Err(Fault::EvalRef {
                family: self.family.clone(),
                field: "door_identity.adapter",
                declared: declared.to_string(),
                found: record.door_identity.adapter.clone(),
            });
        }
        self.matches(&record)
    }

    /// Whether a loaded record is the one this entry describes.
    ///
    /// # Errors
    ///
    /// Returns the first [`Fault::EvalRef`] found, which names the field.
    pub fn matches(&self, record: &Record) -> Result<(), Fault> {
        let compare = |field: &'static str, declared: String, found: String| {
            if declared == found {
                Ok(())
            } else {
                Err(Fault::EvalRef {
                    family: self.family.clone(),
                    field,
                    declared,
                    found,
                })
            }
        };
        compare("family", self.family.clone(), record.family.clone())?;
        compare("suite", self.suite.clone(), record.suite.clone())?;
        compare(
            "suiteDigest",
            self.suite_digest.clone(),
            record.suite_digest.clone(),
        )?;
        compare(
            "partitionId",
            self.partition_id.clone(),
            record.partition_id.clone(),
        )?;
        compare(
            "gateId",
            self.gate_id.clone(),
            record.gate_id.clone().unwrap_or_default(),
        )?;
        compare(
            "admitted",
            self.admitted.to_string(),
            record.admitted.to_string(),
        )?;
        compare("verdict", self.verdict.clone(), record.verdict.clone())
    }
}

/// What a decision model is, in one document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// What this document is. Always [`MANIFEST_SCHEMA`].
    #[serde(default = "manifest_schema")]
    pub schema: String,
    /// The decision model's name, which is the model id its door reports.
    pub name: String,
    /// Which release of that model this is.
    ///
    /// Not bookkeeping. Three adapters exist and are distinguished only by
    /// the run directory they were written to, so a calibration record fitted
    /// against one of them cannot name which one. `name@version` is the name
    /// it could not say, and it is what a door publishes as its adapter
    /// identity.
    pub version: u32,
    /// What the model is for, in one line.
    pub description: String,
    /// The day this release was produced.
    pub created: String,
    /// The bytes, when the model has any.
    ///
    /// Absent for a base model the operating system ships: there is no
    /// artifact to pin, and `docs/kev/mesh-plan.md` already concluded that
    /// the verification floor for such a model has to move from digests to
    /// behavior. For those, `evalRef` is the whole of the grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    /// What it sits on.
    pub base: Base,
    /// What keeps it serving, and what can stop it.
    ///
    /// Load time pins [`Base::signature`] and refuses to start on a
    /// mismatch, which protects a door that restarts. This field is the other
    /// half: the service a running door asks, and the window after which a
    /// door that cannot ask stops on its own. See [`crate::policy`].
    #[serde(default)]
    pub policy_snapshot: crate::policy::SnapshotRef,
    /// The contract it answers.
    pub interface: Interface,
    /// Which estimator draws the raw signal, with how many draws, from which
    /// seed block.
    ///
    /// Shaped exactly as a calibration record carries it, so a door can
    /// compare the two field for field rather than by eye. This is what used
    /// to be `--samples` and `--seed-base` on the command line, where two
    /// runs of the same model could differ and nothing recorded it.
    pub estimator: EstimatorConfig,
    /// The measurements admission rests on, at most one per family.
    #[serde(default)]
    pub eval_ref: Vec<EvalRef>,
    /// Raw evaluation evidence. These references never grant admission.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observation_ref: Vec<crate::observation::ObservationRef>,
    /// The directory the `evalRef` paths resolve against.
    ///
    /// Set from the file's own location by [`Manifest::load`], and not part
    /// of the document: a manifest that named its own directory would be
    /// wrong the moment it moved.
    #[serde(skip)]
    pub source: PathBuf,
}

fn manifest_schema() -> String {
    MANIFEST_SCHEMA.to_string()
}

impl Manifest {
    /// Reads a manifest and checks the claims that need no device.
    ///
    /// The schema tag, the required fields, and the declared interface are
    /// checked here. The artifact, the base, and the calibration records are
    /// checked by [`Manifest::check_artifact`], [`Base::check`], and
    /// [`Manifest::check_eval_refs`], because each needs something this
    /// function does not have.
    ///
    /// # Errors
    ///
    /// Returns the [`Fault`] that names the field.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Fault> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|error| Fault::Unreadable {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let manifest = Self::from_json(&text)
            .map_err(|error| Fault::Unreadable {
                path: path.display().to_string(),
                reason: error.to_string(),
            })?
            .with_source(path.parent().unwrap_or_else(|| Path::new(".")));
        manifest.check()?;
        Ok(manifest)
    }

    /// Reads a manifest from JSON, without checking it.
    ///
    /// # Errors
    ///
    /// Returns what `serde_json` reports when the document is not a manifest.
    pub fn from_json(source: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(source)
    }

    /// The directory the `evalRef` paths are relative to.
    #[must_use]
    pub fn beside(&self) -> &Path {
        &self.source
    }

    /// Resolves `evalRef` paths against `beside`.
    ///
    /// [`Manifest::load`] does this from the file's own location; a caller
    /// that built a manifest in memory says where it would have lived.
    #[must_use]
    pub fn with_source(mut self, beside: impl AsRef<Path>) -> Self {
        self.source = beside.as_ref().to_path_buf();
        self
    }

    /// The release this document describes: `name@version`.
    ///
    /// This is the string a door publishes as its adapter identity and a
    /// calibration record stores, so a later reader can say which of three
    /// adapters a map was fitted against.
    #[must_use]
    pub fn release(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }

    /// The policy governing this release, reading the machine's clock.
    ///
    /// A door holds one of these and asks it on every question, which is how
    /// a revocation reaches a door that is already running.
    #[must_use]
    pub fn policy(&self) -> crate::policy::Policy {
        crate::policy::Policy::for_release(
            &self.policy_snapshot,
            self.beside(),
            self.release(),
            self.base.signature.clone(),
        )
    }

    /// Whether this model may serve a probability for `family`.
    ///
    /// The rule `docs/kev/mesh-plan.md` set for decision-model artifacts: a
    /// row without a measured `evalRef` does not admit. A family with no
    /// entry is refused, and so is a family whose entry the gate refused.
    /// Nothing is admitted by default.
    #[must_use]
    pub fn admits(&self, family: &str) -> bool {
        self.eval_ref(family)
            .is_some_and(|reference| reference.admitted)
    }

    /// The entry covering `family`, admitted or not.
    #[must_use]
    pub fn eval_ref(&self, family: &str) -> Option<&EvalRef> {
        self.eval_ref
            .iter()
            .find(|reference| reference.family == family)
    }

    /// The families this model may serve a probability for.
    #[must_use]
    pub fn admitted_families(&self) -> Vec<&str> {
        self.eval_ref
            .iter()
            .filter(|reference| reference.admitted)
            .map(|reference| reference.family.as_str())
            .collect()
    }

    /// Checks the schema tag, the required fields, and the interface.
    ///
    /// # Errors
    ///
    /// Returns the [`Fault`] that names the field.
    pub fn check(&self) -> Result<(), Fault> {
        if self.schema != MANIFEST_SCHEMA {
            return Err(Fault::Schema {
                found: self.schema.clone(),
            });
        }
        if self.name.trim().is_empty() {
            return Err(Fault::Blank { field: "name" });
        }
        if self.base.signature.trim().is_empty() {
            return Err(Fault::Blank {
                field: "base.signature",
            });
        }
        if self.estimator.estimator.trim().is_empty() {
            return Err(Fault::Blank {
                field: "estimator.estimator",
            });
        }
        // A release with no policy source is a release nothing can revoke,
        // which is the deleted cache again under a tidier name. The window is
        // checked here too, because a release that accepts an unbounded one
        // has a freshness rule only on paper.
        if self.policy_snapshot.source.trim().is_empty() {
            return Err(Fault::Blank {
                field: "policySnapshot.source",
            });
        }
        if self.policy_snapshot.cache.trim().is_empty() {
            return Err(Fault::Blank {
                field: "policySnapshot.cache",
            });
        }
        if self.policy_snapshot.freshness_window_seconds == 0 {
            return Err(Fault::Blank {
                field: "policySnapshot.freshnessWindowSeconds",
            });
        }
        self.interface.check()
    }

    /// Opens the package this manifest names and checks every claim it makes
    /// about it.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Artifact`] when the manifest names no artifact or the
    /// package does not open, and [`Fault::Package`] when a field disagrees.
    pub fn check_artifact(&self) -> Result<Package, Fault> {
        let Some(artifact) = &self.artifact else {
            return Err(Fault::Artifact {
                reason: format!("{} names no artifact", self.release()),
            });
        };
        let path = artifact.resolved_path();
        let package = Package::open(&path).map_err(|refusal| Fault::Artifact {
            reason: format!("{}: {}", path.display(), refusal.message),
        })?;
        artifact.check(&package)?;
        // The package's own pin and the manifest's have to agree, or the
        // document describes a different pairing than the bytes do.
        if package.metadata.base_model_signature != self.base.signature {
            return Err(Fault::Package {
                field: "baseModelSignature",
                declared: self.base.signature.clone(),
                found: package.metadata.base_model_signature.clone(),
            });
        }
        Ok(package)
    }

    /// Checks every calibration record this manifest names.
    ///
    /// # Errors
    ///
    /// Returns the first [`Fault`] found, which names the family and field.
    pub fn check_eval_refs(&self) -> Result<(), Fault> {
        // A record fitted against a different door is the fault this whole
        // directory exists to stop, so the release id travels into the check
        // rather than being compared afterwards by a caller who might not.
        let release = self.release();
        let adapter = self.artifact.as_ref().map(|_| release.as_str());
        for reference in &self.eval_ref {
            reference.check_fitted_against(&self.source, adapter)?;
        }
        Ok(())
    }

    /// Checks observational evidence without granting probability admission.
    ///
    /// # Errors
    ///
    /// Returns an error when a store or its declared provenance differs.
    pub fn check_observation_refs(&self) -> Result<(), crate::observation::Fault> {
        for reference in &self.observation_ref {
            reference.check(&self.source)?;
        }
        Ok(())
    }

    /// Writes the manifest as the committed files are written.
    ///
    /// # Errors
    ///
    /// Returns what `serde_json` reports.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// A leading `~` is the running user's home. Any other path is untouched.
fn expand(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix("~/") else {
        return PathBuf::from(path);
    };
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// The sha256 of a file, lowercase hex, as `shasum -a 256` writes it.
///
/// # Errors
///
/// Returns [`Fault::Unreadable`] when the file does not read.
pub fn digest_of(path: &Path) -> Result<String, Fault> {
    let bytes = std::fs::read(path).map_err(|error| Fault::Unreadable {
        path: path.display().to_string(),
        reason: error.to_string(),
    })?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn size_of(path: &Path) -> Result<u64, Fault> {
    std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| Fault::Unreadable {
            path: path.display().to_string(),
            reason: error.to_string(),
        })
}

fn name_or_none(value: &str) -> String {
    if value.is_empty() {
        "none".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::adapter::{Metadata, write_package, write_records};

    const SIGNATURE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lev-manifest-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    fn package(dir: &Path) -> Package {
        let path = dir.join("lev.fmadapter");
        let metadata = Metadata {
            adapter_identifier: "fmadapter-lev-9799725".to_string(),
            base_model_signature: SIGNATURE.to_string(),
            lora_rank: 32,
            author: None,
            description: None,
            license: None,
            draft_token_count: None,
            creator_defined: BTreeMap::new(),
        };
        write_package(&path, &metadata, &[(1, vec![7; 64])]).expect("the package writes");
        Package::open(&path).expect("the package opens")
    }

    fn manifest(dir: &Path) -> Manifest {
        let package = package(dir);
        Manifest {
            schema: MANIFEST_SCHEMA.to_string(),
            name: "lev-adapted".to_string(),
            version: 1,
            description: "a test model".to_string(),
            created: "2026-09-19".to_string(),
            artifact: Some(Artifact::of_package(&package).expect("the package describes")),
            base: Base {
                signature: SIGNATURE.to_string(),
                min_os_build: "25E246".to_string(),
                runtime: "Apple FoundationModels".to_string(),
            },
            policy_snapshot: crate::policy::SnapshotRef::published(),
            interface: Interface::of_contract(vec!["routing".to_string()]),
            estimator: EstimatorConfig::new("l2", 8, 0),
            eval_ref: Vec::new(),
            observation_ref: Vec::new(),
            source: dir.to_path_buf(),
        }
    }

    /// A record shaped as the committed ones are, carrying the parts an
    /// `evalRef` repeats.
    fn record() -> Record {
        let text = serde_json::json!({
            "schema": "openagents.gym.calibration_record.v1",
            "family": "routing",
            "estimator_config": {"estimator": "l2", "samples": 8, "seed_base": 0},
            "language": "en",
            "suite": "support-v2-three-way",
            "suite_digest": "54fbf413",
            "partition_id": "calibration",
            "os_build": "25E246",
            // Fitted against the release the test fixture describes, which is
            // what `check_eval_refs` compares. A record naming another door is
            // the fault the check exists for, and one test below is exactly
            // that case.
            "door": "lev-adapted@1",
            "door_identity": {
                "model": "lev-adapted",
                "base_model_signature": "9799725",
                "adapter": "lev-adapted@1",
                "verified": true
            },
            "gate_id": "probability-v1",
            "gate_digest": "gate:368cefd1",
            "locked_reads": [],
            "fitted": "2026-09-19",
            "map": {"bins": [], "base_rate": 0.775, "fitted_on": 40},
            "raw_metrics": {
                "accuracy": 0.875, "ece": 0.156, "brier": 0.136, "nll": 3.493,
                "confident_errors": 5, "items": 40
            },
            "calibrated_metrics": {
                "accuracy": 0.875, "ece": 0.106, "brier": 0.120, "nll": 0.413,
                "confident_errors": 0, "items": 40
            },
            "admitted": true,
            "verdict": "passed: log loss 3.493 to 0.413"
        })
        .to_string();
        Record::from_json(&text).expect("the record parses")
    }

    #[test]
    fn a_manifest_round_trips_through_its_own_writer() {
        let dir = scratch("round-trip");
        let written = manifest(&dir);
        let read = Manifest::from_json(&written.to_json().expect("it encodes"))
            .expect("it decodes")
            .with_source(&dir);
        assert_eq!(read, written);
        assert_eq!(read.release(), "lev-adapted@1");
    }

    #[test]
    fn a_family_with_no_eval_ref_does_not_admit() {
        // The rule `docs/kev/mesh-plan.md` set, and the only one in this file
        // that decides whether a number reaches a caller.
        let dir = scratch("no-eval-ref");
        let manifest = manifest(&dir);
        assert!(!manifest.admits("routing"), "an unmeasured family admitted");
        assert!(manifest.admitted_families().is_empty());
    }

    #[test]
    fn a_refused_eval_ref_does_not_admit_either() {
        let dir = scratch("refused");
        let mut manifest = manifest(&dir);
        manifest.eval_ref.push(EvalRef {
            family: "severity".to_string(),
            record: "severity.json".to_string(),
            sha256: "0".repeat(64),
            suite: "support-v2-three-way".to_string(),
            suite_digest: "54fbf413".to_string(),
            partition_id: "calibration".to_string(),
            gate_id: "probability-v1".to_string(),
            admitted: false,
            verdict: "unverifiable: fitted_on>=30".to_string(),
        });
        assert!(!manifest.admits("severity"));
        assert!(
            manifest.eval_ref("severity").is_some(),
            "the refusal is still recorded"
        );
    }

    #[test]
    fn the_package_on_disk_has_to_be_the_one_the_manifest_describes() {
        let dir = scratch("digest");
        let manifest = manifest(&dir);
        manifest.check_artifact().expect("the package matches");

        // The same package with one tensor value changed: the identifier, the
        // rank, and the size all still agree, and the digest does not. This is
        // the check that catches a stale artifact beside a manifest written
        // for another one.
        let package = manifest
            .artifact
            .as_ref()
            .expect("an artifact")
            .resolved_path();
        std::fs::write(
            package.join(WEIGHTS_FILE),
            write_records(&[(1, vec![8; 64])]),
        )
        .expect("the weights rewrite");
        let fault = manifest
            .check_artifact()
            .expect_err("a changed package is refused");
        assert!(
            matches!(
                fault,
                Fault::Package {
                    field: "sha256",
                    ..
                }
            ),
            "{fault}"
        );
    }

    #[test]
    fn a_manifest_pinned_to_another_base_is_refused() {
        let dir = scratch("base");
        let manifest = manifest(&dir);
        manifest
            .base
            .check("9799725")
            .expect("the device prefix matches");
        let fault = manifest
            .base
            .check("0000000")
            .expect_err("another base is refused");
        assert!(matches!(fault, Fault::Base { .. }), "{fault}");
        let silent = manifest
            .base
            .check("")
            .expect_err("an unidentifiable device is refused");
        assert!(silent.to_string().contains("none"), "{silent}");
    }

    #[test]
    fn a_declared_interface_that_is_not_the_contract_is_refused() {
        let dir = scratch("interface");
        let mut manifest = manifest(&dir);
        manifest
            .interface
            .questions
            .get_mut("choice")
            .expect("the choice shape")
            .max_criteria = Some(1_000);
        let fault = manifest
            .check()
            .expect_err("a drifted interface is refused");
        assert!(matches!(fault, Fault::Interface { .. }), "{fault}");
    }

    #[test]
    fn an_eval_ref_that_no_longer_matches_its_record_is_refused() {
        let dir = scratch("eval-ref");
        let mut manifest = manifest(&dir);
        let record = record();
        let text = serde_json::to_string_pretty(&record).expect("the record encodes");
        std::fs::write(dir.join("routing.json"), &text).expect("the record writes");
        let digest = digest_of(&dir.join("routing.json")).expect("the digest");
        manifest
            .eval_ref
            .push(EvalRef::of_record(&record, "routing.json", digest));
        manifest.check_eval_refs().expect("the record matches");

        // The verdict is edited in place, which is how a stale claim gets
        // made: the file still parses and still says `admitted`.
        let edited = text.replace("passed:", "passed after retuning:");
        std::fs::write(dir.join("routing.json"), edited).expect("the record rewrites");
        let fault = manifest
            .check_eval_refs()
            .expect_err("an edited record is refused");
        assert!(
            matches!(
                fault,
                Fault::EvalRef {
                    field: "sha256",
                    ..
                }
            ),
            "{fault}"
        );
    }

    #[test]
    fn a_map_fitted_against_another_door_may_not_be_named() {
        // The stale-map fault in its own words. Three base-fitted records sat
        // on disk through two adapter runs, and nothing could tell that they
        // described a different door, because a record could not name one.
        // Now it can, so a release that points at one is refused by field.
        let dir = scratch("eval-ref-door");
        let mut manifest = manifest(&dir);
        let mut record = record();
        record.door = "lev-base".to_string();
        record.door_identity.model = "lev-base".to_string();
        record.door_identity.adapter = String::new();
        let text = serde_json::to_string_pretty(&record).expect("the record encodes");
        std::fs::write(dir.join("routing.json"), &text).expect("the record writes");
        let digest = digest_of(&dir.join("routing.json")).expect("the digest");
        manifest
            .eval_ref
            .push(EvalRef::of_record(&record, "routing.json", digest));

        let fault = manifest
            .check_eval_refs()
            .expect_err("a base-fitted map is refused");
        assert!(
            matches!(
                fault,
                Fault::EvalRef {
                    field: "door_identity.adapter",
                    ..
                }
            ),
            "{fault}"
        );
    }
}
