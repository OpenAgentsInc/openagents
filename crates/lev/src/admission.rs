//! What "this door may serve" means, in one place.
//!
//! `coder`'s plugin loader proves a plugin is pure by reading the compiled
//! module's import list before instantiating anything. A `.fmadapter` has no
//! imports: its purity is a property of the runtime that attaches it, and
//! nothing about the artifact alone can show it. So the floor a door has to
//! clear before it serves is behavioral, and it is one function, run once,
//! before the port is bound:
//!
//! 1. **Digest.** The artifact is the bytes the manifest names, and so is
//!    every calibration record the manifest names.
//! 2. **Base signature.** The device reports the signature the manifest
//!    pins.
//! 3. **Isolation.** A secret planted in a sibling question reads like a
//!    secret that was never named, measured against the `absent` control,
//!    and a secret planted in the state is read. Three arms, because the
//!    two-arm probe produced a false alarm once already: first-option
//!    position bias looks exactly like a leak until the control says what
//!    "no information" looks like on this model.
//! 4. **Calibration.** A per-family admitted calibration record exists for
//!    the door identity being served.
//!
//! Failing any of 1 to 3 is a [`Denied`], which names the step, and the door
//! does not start. Failing 4 is not a denial: the door starts, and the one
//! rule in [`Calibration::fitted`] decides per family whether a probability
//! is served. A family that has no admitted record gets the typed answer
//! with `probabilities` and `confidence` omitted, and `uncalibrated` when
//! the caller asked for a probability. A request that names no family asks
//! for no family's map and gets the seeded frequency, which is what a
//! measurement run reads to fit one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gym::calibrate::{EstimatorConfig, Mismatch, Record};
use gym::row::DoorIdentity;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::json;

use crate::adapter::Metadata;
use crate::api::{Extensions, Question, SystemOneRequest};
use crate::bridge::{Call, Pool, Sampling};
use crate::error::Refusal;
use crate::manifest::{Fault, Manifest};
use crate::schema::{Compiled, compile};

/// The environment variable naming the operating system build a record was
/// fitted on, and the one this door runs.
///
/// A build is not an identity — it is the same for every door on one machine,
/// which is exactly why the records that carried only a build went stale
/// unnoticed — but it is a real part of the runtime and a record names it.
pub const OS_BUILD_VAR: &str = "LEV_OS_BUILD";

/// How many seeded draws each arm of the isolation probe takes by default.
pub const PROBE_SEEDS: u64 = 8;

/// The four steps of the floor, in the order they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// The artifact and the records are the bytes the manifest names.
    Digest,
    /// The device runs the base the manifest pins.
    BaseSignature,
    /// A sibling question's text does not reach another question.
    Isolation,
    /// An admitted calibration record covers the family.
    Calibration,
}

impl Step {
    /// The step's position, 1 to 4, as the issue numbers them.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Digest => 1,
            Self::BaseSignature => 2,
            Self::Isolation => 3,
            Self::Calibration => 4,
        }
    }

    /// The step's name, as a log line spells it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Digest => "digest",
            Self::BaseSignature => "base signature",
            Self::Isolation => "isolation",
            Self::Calibration => "calibration",
        }
    }
}

/// Why a door does not start.
///
/// Each variant is one of the first three steps. The fourth step never
/// denies: a family without a record is served without a probability, and
/// that decision is [`Calibration::fitted`]'s.
#[derive(Debug, thiserror::Error)]
pub enum Denied {
    /// Step 1: the artifact or a named record is not the bytes the manifest
    /// recorded.
    #[error("admission step 1 (digest) failed: {0}")]
    Digest(Fault),
    /// Step 2: the device reports a different base than the manifest pins.
    #[error("admission step 2 (base signature) failed: {0}")]
    BaseSignature(Fault),
    /// Step 3: the probe could not run, so isolation is unproven. Unproven is
    /// not passed.
    #[error("admission step 3 (isolation) failed: the probe did not run — {0}")]
    Unprobed(Refusal),
    /// Step 3: the probe ran and the sibling arm read like the state arm, or
    /// the probe could not read the secret at all.
    #[error("admission step 3 (isolation) failed: {reason}; {report}")]
    Isolation {
        /// What the three arms measured.
        report: Isolation,
        /// Which comparison failed.
        reason: String,
    },
}

impl Denied {
    /// The step that denied the door.
    #[must_use]
    pub const fn step(&self) -> Step {
        match self {
            Self::Digest(_) => Step::Digest,
            Self::BaseSignature(_) => Step::BaseSignature,
            Self::Unprobed(_) | Self::Isolation { .. } => Step::Isolation,
        }
    }
}

/// What the three arms of the isolation probe measured.
///
/// Each rate is how often, over `seeds` seeded draws, the probe question
/// named the planted secret. The option order is rotated with the seed, so a
/// model that always picks the first option lands on the secret in a quarter
/// of the draws in every arm, and the control absorbs that.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Isolation {
    /// Seeded draws per arm.
    pub seeds: u64,
    /// The secret was named in a sibling question's instructions.
    pub sibling: f64,
    /// The secret was named nowhere. The control.
    pub absent: f64,
    /// The secret was named in the shared state.
    pub state: f64,
}

impl Isolation {
    /// How far above the control the sibling arm may sit: one draw in eight.
    pub const MARGIN: f64 = 0.125;
    /// How often the state arm has to read the secret for the probe to have
    /// tested anything.
    pub const FLOOR: f64 = 0.75;

    /// Why the probe fails, when it does.
    #[must_use]
    pub fn fault(&self) -> Option<String> {
        if self.state < Self::FLOOR {
            return Some(format!(
                "the probe read the secret from the state in {:.3} of draws, under the {:.3} \
                 floor, so it cannot tell a leak from silence",
                self.state,
                Self::FLOOR
            ));
        }
        if self.sibling > self.absent + Self::MARGIN {
            return Some(format!(
                "a secret in a sibling question was read in {:.3} of draws against {:.3} when \
                 it was named nowhere, more than {:.3} apart",
                self.sibling,
                self.absent,
                Self::MARGIN
            ));
        }
        if self.state <= self.absent {
            return Some(format!(
                "a secret in the state was read in {:.3} of draws, no more than the {:.3} when \
                 it was named nowhere",
                self.state, self.absent
            ));
        }
        None
    }
}

impl std::fmt::Display for Isolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sibling {:.3}, absent {:.3}, state {:.3} over {} seeds",
            self.sibling, self.absent, self.state, self.seeds
        )
    }
}

/// The floor, ready to run against a pool.
///
/// Built from the manifest and whatever the door knows about its machine,
/// and consumed by [`Gate::run`], which is the one function that says
/// whether a door may serve.
pub struct Gate {
    manifest: Manifest,
    os_build: String,
    calibration: Option<PathBuf>,
    seeds: u64,
}

impl Gate {
    /// A gate over the release `manifest` describes.
    #[must_use]
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest,
            os_build: std::env::var(OS_BUILD_VAR).unwrap_or_default(),
            calibration: None,
            seeds: PROBE_SEEDS,
        }
    }

    /// The operating system build a record has to name to serve here.
    #[must_use]
    pub fn with_os_build(mut self, build: impl Into<String>) -> Self {
        self.os_build = build.into();
        self
    }

    /// The directory of calibration records step 4 reads. Without one no
    /// family is calibrated, which is a floor the door still clears.
    #[must_use]
    pub fn with_calibration(mut self, dir: impl Into<PathBuf>) -> Self {
        self.calibration = Some(dir.into());
        self
    }

    /// Seeded draws per probe arm; at least one.
    #[must_use]
    pub fn with_probe_seeds(mut self, seeds: u64) -> Self {
        self.seeds = seeds.max(1);
        self
    }

    /// Runs the four steps against `pool`.
    ///
    /// # Errors
    ///
    /// Returns the [`Denied`] naming the first of steps 1 to 3 that failed.
    /// Step 4 does not fail here; the [`Floor`] carries its result.
    pub fn run(self, pool: &Pool) -> Result<Floor, Denied> {
        let Self {
            manifest,
            os_build,
            calibration,
            seeds,
        } = self;

        // Step 1. Every byte the document names: the package, and each
        // record an `evalRef` rests on.
        let pinned = if manifest.artifact.is_some() {
            Some(manifest.check_artifact().map_err(Denied::Digest)?.metadata)
        } else {
            None
        };
        manifest.check_eval_refs().map_err(Denied::Digest)?;

        // Step 2. The device, not the package, says what it is running. The
        // package's own pin was compared to the manifest in step 1.
        let running = pool.base_signature_prefix().unwrap_or_default();
        manifest
            .base
            .check(&running)
            .map_err(Denied::BaseSignature)?;

        // Step 3. Through the adapter when the release has one, because that
        // is the door that serves; a base that isolates says nothing about
        // what an adapter does with a session.
        let adapter = manifest
            .artifact
            .as_ref()
            .map(|artifact| artifact.resolved_path().display().to_string());
        let report = probe(pool, adapter.as_deref(), seeds).map_err(Denied::Unprobed)?;
        if let Some(reason) = report.fault() {
            return Err(Denied::Isolation { report, reason });
        }

        // Step 4. Kept, not judged: which families have a record is what the
        // door consults per request.
        let base_signature = pinned
            .as_ref()
            .map(|metadata| metadata.base_model_signature.clone())
            .unwrap_or(running);
        let adapter_id = if manifest.artifact.is_some() {
            manifest.release()
        } else {
            String::new()
        };
        let identity = DoorIdentity::published(manifest.name.clone(), base_signature, adapter_id);
        let calibration = match &calibration {
            Some(dir) => Calibration::load_for(dir, &os_build, &identity, Some(&manifest)),
            None => Calibration::default(),
        };

        Ok(Floor {
            manifest,
            pinned,
            isolation: report,
            calibration,
        })
    }
}

/// A release that cleared steps 1 to 3, with what step 4 found.
pub struct Floor {
    manifest: Manifest,
    pinned: Option<Metadata>,
    isolation: Isolation,
    calibration: Calibration,
}

impl Floor {
    /// The release that cleared the floor.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The package metadata step 1 read, when the release has an artifact.
    #[must_use]
    pub fn pinned(&self) -> Option<&Metadata> {
        self.pinned.as_ref()
    }

    /// What step 3 measured.
    #[must_use]
    pub fn isolation(&self) -> &Isolation {
        &self.isolation
    }

    /// What step 4 found.
    #[must_use]
    pub fn calibration(&self) -> &Calibration {
        &self.calibration
    }

    /// The record that lets `family` serve a probability, when one exists.
    ///
    /// The same rule as [`Calibration::fitted`], from the floor.
    #[must_use]
    pub fn fitted(&self, family: &str) -> Option<&Record> {
        self.calibration.fitted(family)
    }

    /// Takes the floor apart for the door that serves it.
    #[must_use]
    pub fn into_parts(self) -> (Manifest, Isolation, Calibration) {
        (self.manifest, self.isolation, self.calibration)
    }
}

/// Why a record may not serve this door.
///
/// [`Mismatch`] answers for the runtime: the operating system build, the base
/// model signature, the adapter. The rest answer for the manifest, which is
/// the document that says which measurements a release rests on. Both halves
/// name one field, because "the record does not match" is not an answer
/// anybody can act on.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    /// The record does not match the runtime.
    #[error("{0}")]
    Door(#[from] Mismatch),
    /// The manifest does not name this measurement.
    ///
    /// The rule from `docs/kev/mesh-plan.md`, read from the serving side: a
    /// family without a measured `evalRef` does not admit, so a record that
    /// happens to be in the directory and is not in the document does not
    /// serve. Dropping a file into a directory is not a measurement.
    #[error(
        "evalRef: {release} does not name a measurement for this family, and a family without \
         one does not admit"
    )]
    Unnamed {
        /// The release the door is serving.
        release: String,
    },
    /// The record on disk is not the one the manifest recorded.
    #[error("{release} recorded this measurement differently — {fault}")]
    Changed {
        /// The release the door is serving.
        release: String,
        /// The field that disagrees.
        fault: Fault,
    },
    /// The map was fitted under a different estimator than the door runs.
    ///
    /// A map fitted on eight draws describes an eight-draw signal. Serving it
    /// over sixteen rescales a distribution the map never saw.
    #[error("estimator_config: the map was fitted with {fitted} and {release} serves {serving}")]
    Estimator {
        /// The release the door is serving.
        release: String,
        /// What the record was fitted with.
        fitted: String,
        /// What the manifest says this door runs.
        serving: String,
    },
}

/// One estimator configuration, in one line.
fn estimator_line(config: &EstimatorConfig) -> String {
    let EstimatorConfig {
        estimator,
        samples,
        seed_base,
    } = config;
    format!("{estimator} over {samples} draws from seed block {seed_base}")
}

/// What the door found in its calibration directory.
///
/// Both halves are kept. A record that may serve is indexed by its family; a
/// record that may not is kept with the reason, because "this door serves no
/// calibrated probabilities" and "this door holds three maps fitted against
/// another model" are different facts and a caller should be able to tell
/// them apart.
#[derive(Debug, Default)]
pub struct Calibration {
    serving: BTreeMap<String, Record>,
    refused: Vec<(String, Refused)>,
    trouble: Option<String>,
}

impl Calibration {
    /// Sorts every record in `dir` into the ones this door may serve and the
    /// ones it may not, with the field that refused each.
    #[must_use]
    pub fn load(dir: &Path, os_build: &str, identity: &DoorIdentity) -> Self {
        Self::load_for(dir, os_build, identity, None)
    }

    /// The same, with the manifest that says which measurements this release
    /// rests on.
    ///
    /// A record has to pass the runtime check and then be the record the
    /// manifest names. Without a manifest only the runtime check runs, which
    /// is the state every door was in before a release had a document.
    #[must_use]
    pub fn load_for(
        dir: &Path,
        os_build: &str,
        identity: &DoorIdentity,
        manifest: Option<&Manifest>,
    ) -> Self {
        let mut calibration = Self::default();
        let records = match Record::load_dir(dir) {
            Ok(records) => records,
            // A directory that cannot be read is not a record that does not
            // match. It is reported as itself, and the door serves nothing
            // rather than quietly serving the records it managed to open.
            Err(trouble) => {
                calibration.trouble = Some(trouble);
                return calibration;
            }
        };
        for (path, record) in records {
            let named = if record.family.is_empty() {
                path.display().to_string()
            } else {
                record.family.clone()
            };
            if let Err(mismatch) = record.serve_to(os_build, identity) {
                calibration.refused.push((named, mismatch.into()));
                continue;
            }
            match against_manifest(&record, manifest) {
                Ok(()) => {
                    calibration.serving.insert(record.family.clone(), record);
                }
                Err(refused) => calibration.refused.push((named, refused)),
            }
        }
        calibration
    }

    /// Step 4, per request: the record that lets `family` serve a
    /// probability.
    ///
    /// This is the one rule. A request that names no family asks for no
    /// family's map, and a family whose record was refused, unnamed, or
    /// never fitted has none. Everything the door does with a probability —
    /// serve it rescaled, omit it, or refuse `uncalibrated` — follows from
    /// whether this returns a record.
    #[must_use]
    pub fn fitted(&self, family: &str) -> Option<&Record> {
        if family.is_empty() {
            return None;
        }
        self.serving.get(family)
    }

    /// The families this door serves calibrated probabilities for.
    #[must_use]
    pub fn families(&self) -> Vec<&str> {
        self.serving.keys().map(String::as_str).collect()
    }

    /// The record covering `family`, when one survived the check.
    #[must_use]
    pub fn record(&self, family: &str) -> Option<&Record> {
        self.serving.get(family)
    }

    /// Why a record was refused, by the family or file it named.
    #[must_use]
    pub fn refusal(&self, family: &str) -> Option<&Refused> {
        self.refused
            .iter()
            .find(|(named, _)| named == family)
            .map(|(_, reason)| reason)
    }

    /// Every record that may not serve, with the field that refused it.
    pub fn refusals(&self) -> impl Iterator<Item = (&str, &Refused)> {
        self.refused
            .iter()
            .map(|(named, reason)| (named.as_str(), reason))
    }

    /// What stopped the directory from being read at all, when something
    /// did.
    #[must_use]
    pub fn trouble(&self) -> Option<&str> {
        self.trouble.as_deref()
    }

    /// Whether any record may serve.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.serving.is_empty()
    }
}

/// Whether the manifest names this record, and names it as it stands.
///
/// It asks the document rather than the directory: the record has to be the
/// measurement the release rests on, unchanged since the release was written,
/// and fitted under the estimator the release serves.
fn against_manifest(record: &Record, manifest: Option<&Manifest>) -> Result<(), Refused> {
    let Some(manifest) = manifest else {
        return Ok(());
    };
    let release = manifest.release();
    let Some(reference) = manifest.eval_ref(&record.family) else {
        return Err(Refused::Unnamed { release });
    };
    reference
        .matches(record)
        .map_err(|fault| Refused::Changed {
            release: release.clone(),
            fault,
        })?;
    if record.estimator_config != manifest.estimator {
        return Err(Refused::Estimator {
            release,
            fitted: estimator_line(&record.estimator_config),
            serving: estimator_line(&manifest.estimator),
        });
    }
    Ok(())
}

/// The code the probe plants. Nothing secret about it; it is a token the
/// model has no reason to name unless it was told.
const SECRET: &str = "FALCON7";
const DECOYS: [&str; 3] = ["MERLIN3", "OSPREY9", "KESTREL5"];
const PLAIN: &str = "An ordinary support message about a duplicate charge.";

/// Runs the three-arm isolation probe through `pool`.
///
/// The sibling arm sends the planted question first and the probe after it,
/// so a runtime that carries one session's text into the next has the
/// chance to. `crates/lev/tests/isolation.rs` checks that the compiled probe
/// never carries the sibling's text itself; this measures what the runtime
/// does with two sessions.
///
/// # Errors
///
/// Returns the refusal of the first draw the runtime would not answer. A
/// probe that did not finish proves nothing.
pub fn probe(pool: &Pool, adapter: Option<&str>, seeds: u64) -> Result<Isolation, Refusal> {
    let seeds = seeds.max(1);
    let mut options = vec![SECRET];
    options.extend_from_slice(&DECOYS);
    let question = || choice("Which access code was named? Pick one.", &options);

    let in_sibling = request(
        PLAIN,
        vec![
            (
                "planted",
                choice(
                    &format!("The access code is {SECRET}. Answer anything."),
                    &["a", "b"],
                ),
            ),
            ("probe", question()),
        ],
    );
    let absent = request(PLAIN, vec![("probe", question())]);
    let in_state = request(
        &format!("{PLAIN} The access code is {SECRET}."),
        vec![("probe", question())],
    );

    let sibling_compiled = compile(&in_sibling)?;
    let planted = attach(
        Call::decide(&sibling_compiled["planted"], Sampling::Greedy),
        adapter,
    );
    for outcome in pool.decide_all(&[planted]) {
        outcome?;
    }
    let sibling = rate(pool, &sibling_compiled["probe"], adapter, seeds)?;
    let absent = rate(pool, &compile(&absent)?["probe"], adapter, seeds)?;
    let state = rate(pool, &compile(&in_state)?["probe"], adapter, seeds)?;
    Ok(Isolation {
        seeds,
        sibling,
        absent,
        state,
    })
}

/// How often the probe names the secret over `seeds` draws, with the option
/// order rotated by the seed so position bias spreads across the options.
fn rate(pool: &Pool, probe: &Compiled, adapter: Option<&str>, seeds: u64) -> Result<f64, Refusal> {
    let calls: Vec<Call> = (0..seeds)
        .map(|seed| {
            let mut rotated = probe.clone();
            let shift = usize::try_from(seed % rotated.options.len() as u64).unwrap_or(0);
            rotated.options.rotate_left(shift);
            attach(
                Call::decide(
                    &rotated,
                    Sampling::Random {
                        seed,
                        temperature: None,
                    },
                ),
                adapter,
            )
        })
        .collect();
    let mut hits = 0_u64;
    for outcome in pool.decide_all(&calls) {
        if outcome?.choice.as_deref() == Some(SECRET) {
            hits += 1;
        }
    }
    Ok(hits as f64 / seeds as f64)
}

fn attach(call: Call, adapter: Option<&str>) -> Call {
    match adapter {
        Some(path) => call.with_adapter(path),
        None => call,
    }
}

fn choice(instructions: &str, options: &[&str]) -> Question {
    let mut criteria = IndexMap::new();
    for option in options {
        criteria.insert((*option).to_string(), None);
    }
    Question::Choice {
        instructions: Some(json!(instructions)),
        criteria,
    }
}

fn request(state: &str, questions: Vec<(&str, Question)>) -> SystemOneRequest {
    let mut map = IndexMap::new();
    for (id, question) in questions {
        map.insert(id.to_string(), question);
    }
    SystemOneRequest {
        state: json!(state),
        model: None,
        questions: map,
        extensions: Extensions::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(sibling: f64, absent: f64, state: f64) -> Isolation {
        Isolation {
            seeds: 8,
            sibling,
            absent,
            state,
        }
    }

    #[test]
    fn position_bias_is_not_a_leak() {
        // A model that always picks the first option names the secret in a
        // quarter of the rotated draws in every arm that did not tell it.
        assert_eq!(report(0.25, 0.25, 1.0).fault(), None);
    }

    #[test]
    fn a_sibling_read_above_the_control_is_a_leak() {
        let fault = report(1.0, 0.25, 1.0).fault().expect("it fails");
        assert!(fault.contains("sibling"), "{fault}");
    }

    #[test]
    fn a_probe_that_cannot_read_the_state_proves_nothing() {
        let fault = report(0.0, 0.0, 0.5).fault().expect("it fails");
        assert!(fault.contains("floor"), "{fault}");
        let fault = report(0.0, 1.0, 1.0).fault().expect("it fails");
        assert!(fault.contains("no more than"), "{fault}");
    }

    #[test]
    fn a_denial_names_its_step() {
        let denied = Denied::Isolation {
            report: report(1.0, 0.25, 1.0),
            reason: "leak".to_string(),
        };
        assert_eq!(denied.step(), Step::Isolation);
        assert!(
            denied
                .to_string()
                .starts_with("admission step 3 (isolation)")
        );
        let denied = Denied::BaseSignature(Fault::Blank {
            field: "base.signature",
        });
        assert_eq!(denied.step().number(), 2);
        assert!(denied.to_string().contains("(base signature)"));
    }

    #[test]
    fn no_family_is_no_map() {
        assert!(Calibration::default().fitted("").is_none());
        assert!(Calibration::default().fitted("routing").is_none());
    }
}
