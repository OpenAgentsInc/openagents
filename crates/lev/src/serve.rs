//! The door: the System One contract over HTTP.
//!
//! A `crates/jev` client reaches this with a `base_url` change and no other
//! edit, which is the point of building a third implementation of one
//! contract.
//!
//! **What the numbers mean.** The `probabilities` a Lev answer carries are
//! the frequency with which the model selected each option across `N` seeded
//! samples. They measure how consistently it answers, not how often it is
//! right, and the behavior record in `docs/lev/measurements/` shows the model
//! holding a wrong answer at 0.81 as steadily as a right one. Every response
//! says so in `extensions.calibration`, and `GET /v1/models` says so too. A
//! caller that will not accept an uncalibrated number sends
//! `extensions.require_calibration` and gets a typed refusal instead.
//!
//! **Unless a map is fitted for what it is asking.** A door started with
//! `--calibration <dir>` reads the records in that directory, checks each one
//! against what this door is actually running, and serves the calibrated
//! distribution for any question family a surviving record covers. The caller
//! names the family in `extensions.family`, because the contract carries a
//! state and a question and nothing that says which fitted map applies.
//!
//! The checking is the point. Until 2026-09-19 this door reported
//! `"calibration": "none"` as a constant and never opened a record, while the
//! repository held three maps fitted against a door nobody could identify. A
//! record that does not match is refused by field — the operating system
//! build, the base model signature, or the adapter — and the reason is
//! published in `GET /v1/models` rather than logged and forgotten.
//!
//! **And unless the release names the map.** A door started with
//! `--manifest` takes its model name, artifact, estimator, and sample count
//! from one document, and serves only the records that document names, with
//! the digest and verdict it recorded. A record that matches the runtime and
//! is absent from the manifest does not serve: a family without a measured
//! `evalRef` does not admit, and dropping a file into a directory is not a
//! measurement. See [`crate::manifest`] and `docs/lev/manifest.md`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use indexmap::IndexMap;
use serde_json::{Value, json};

use gym::calibrate::{EstimatorConfig, Mismatch, Record};
use gym::row::DoorIdentity;

use crate::adapter::Metadata;
use crate::api::{MAX_CHOICE_OPTIONS, MAX_SCORE_LEVELS, SystemOneRequest, SystemOneResponse, Usage};
use crate::bridge::Pool;
use crate::error::{Refusal, RefusalCode};
use crate::estimator::{Estimator, answer, l2_pool_with};
use crate::manifest::{Fault, Manifest};
use crate::schema::compile;

/// How many seeded samples one question draws by default.
pub const DEFAULT_SAMPLES: u64 = 8;

/// The environment variable naming the operating system build a record was
/// fitted on, and the one this door runs.
///
/// A build is not an identity — it is the same for every door on one machine,
/// which is exactly why the records that carried only a build went stale
/// unnoticed — but it is a real part of the runtime and a record names it.
pub const OS_BUILD_VAR: &str = "LEV_OS_BUILD";

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
    let EstimatorConfig { estimator, samples, seed_base } = config;
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
        self.refused.iter().find(|(named, _)| named == family).map(|(_, reason)| reason)
    }

    /// Every record that may not serve, with the field that refused it.
    pub fn refusals(&self) -> impl Iterator<Item = (&str, &Refused)> {
        self.refused.iter().map(|(named, reason)| (named.as_str(), reason))
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
/// This is the third of the four checks that used to answer to nobody. It
/// asks the document rather than the directory: the record has to be the
/// measurement the release rests on, unchanged since the release was written,
/// and fitted under the estimator the release serves.
fn against_manifest(record: &Record, manifest: Option<&Manifest>) -> Result<(), Refused> {
    let Some(manifest) = manifest else { return Ok(()) };
    let release = manifest.release();
    let Some(reference) = manifest.eval_ref(&record.family) else {
        return Err(Refused::Unnamed { release });
    };
    reference
        .matches(record)
        .map_err(|fault| Refused::Changed { release: release.clone(), fault })?;
    if record.estimator_config != manifest.estimator {
        return Err(Refused::Estimator {
            release,
            fitted: estimator_line(&record.estimator_config),
            serving: estimator_line(&manifest.estimator),
        });
    }
    Ok(())
}

/// What the door serves.
pub struct Door {
    pool: Pool,
    model: String,
    samples: u64,
    seed_base: u64,
    adapter: Option<String>,
    pinned: Option<Metadata>,
    manifest: Option<Manifest>,
    os_build: String,
    base_signature: String,
    calibration: Calibration,
}

impl Door {
    /// Builds a door over a pool of helper processes.
    ///
    /// The base model signature is read from the runtime here rather than per
    /// request: it is what a calibration record has to match, and a door that
    /// cannot say what it is running serves no calibrated probabilities at
    /// all.
    #[must_use]
    pub fn new(pool: Pool, model: impl Into<String>, samples: u64) -> Self {
        let base_signature = pool.base_signature_prefix().unwrap_or_default();
        Self {
            pool,
            model: model.into(),
            samples: samples.max(1),
            seed_base: 0,
            adapter: None,
            pinned: None,
            manifest: None,
            os_build: std::env::var(OS_BUILD_VAR).unwrap_or_default(),
            base_signature,
            calibration: Calibration::default(),
        }
    }

    /// Serves the release a manifest describes.
    ///
    /// The manifest supplies the model name, the artifact, the estimator, and
    /// the sample count, all of which used to be separate flags that nothing
    /// compared. Check it with [`Manifest::check_artifact`] before calling
    /// this: a door reads the document, and the caller decides whether a
    /// document that does not check out is worth starting for.
    ///
    /// Call it before [`Door::with_calibration`].
    #[must_use]
    pub fn with_manifest(mut self, manifest: Manifest) -> Self {
        self.model = manifest.name.clone();
        self.samples = manifest.estimator.samples.max(1);
        self.seed_base = manifest.estimator.seed_base;
        if let Some(artifact) = &manifest.artifact {
            self = self.with_adapter(artifact.resolved_path().display().to_string());
        }
        self.manifest = Some(manifest);
        self
    }

    /// Reads the calibration records in `dir` and keeps the ones that match
    /// this door.
    ///
    /// Call it after [`Door::with_adapter`] and [`Door::with_manifest`]:
    /// attaching an adapter changes the door, a map fitted against the base
    /// must not survive the change, and the manifest is what says which maps
    /// this release rests on.
    #[must_use]
    pub fn with_calibration(mut self, dir: impl AsRef<Path>) -> Self {
        let identity = self.identity();
        self.calibration =
            Calibration::load_for(dir.as_ref(), &self.os_build, &identity, self.manifest.as_ref());
        self
    }

    /// The release this door serves, when it serves one.
    #[must_use]
    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    /// The operating system build this door reports, for a record to match.
    #[must_use]
    pub fn with_os_build(mut self, build: impl Into<String>) -> Self {
        self.os_build = build.into();
        self
    }

    /// What this door is running, as far as it can be verified.
    #[must_use]
    pub fn identity(&self) -> DoorIdentity {
        DoorIdentity::published(self.model.clone(), self.base_signature(), self.adapter_id())
    }

    /// What this door calls the adapter it serves.
    ///
    /// The release from the manifest, when the release has an artifact —
    /// `lev-adapted@2` names one of three packages that a filesystem path
    /// could not, since the path is machine-local and the three differ only
    /// by run directory. Without a manifest, the identifier the package
    /// itself declares, which is at least portable. Never the path.
    ///
    /// Empty when no adapter is attached, including for a release that names
    /// no artifact. The field says which adapter a record was fitted against,
    /// and a door serving the base was fitted against none; the release it
    /// runs under is published separately, as `manifest`.
    #[must_use]
    pub fn adapter_id(&self) -> String {
        match &self.manifest {
            Some(manifest) if manifest.artifact.is_some() => manifest.release(),
            _ => self
                .pinned
                .as_ref()
                .map(|metadata| metadata.adapter_identifier.clone())
                .unwrap_or_default(),
        }
    }

    /// What this door found in its calibration directory.
    #[must_use]
    pub fn calibration(&self) -> &Calibration {
        &self.calibration
    }

    /// The base signature this door is pinned to.
    ///
    /// An attached package pins it exactly, and the package is read once when
    /// it is attached rather than on every call that asks. With no adapter
    /// the runtime still publishes the prefix it accepts adapters for, which
    /// is read once at startup, so a base door is identifiable too.
    #[must_use]
    pub fn base_signature(&self) -> String {
        self.pinned
            .as_ref()
            .map(|metadata| metadata.base_model_signature.clone())
            .unwrap_or_else(|| self.base_signature.clone())
    }

    /// Draws every question from a different block of seeds.
    ///
    /// Block 0 is the default and reproduces the recorded numbers. A door
    /// asked for another block answers the same questions with seeds it has
    /// not drawn before, which is the only honest way to get a fresh trial
    /// out of a runtime whose seeds reproduce exactly. A caller comparing two
    /// doors should move this together with nothing else.
    #[must_use]
    pub fn with_seed_base(mut self, seed_base: u64) -> Self {
        self.seed_base = seed_base;
        self
    }

    /// Serves every call through a `.fmadapter` package.
    ///
    /// The package is checked and pinned before the door starts, not per
    /// request: a signature mismatch is a deployment error, not a caller
    /// error.
    #[must_use]
    pub fn with_adapter(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.pinned = crate::adapter::Package::open(&path).ok().map(|package| package.metadata);
        self.adapter = Some(path);
        self
    }

    /// How many helpers back this door.
    #[must_use]
    pub fn pool_width(&self) -> usize {
        self.pool.width()
    }

    /// How many seeded draws one estimate rests on.
    #[must_use]
    pub const fn samples(&self) -> u64 {
        self.samples
    }

    /// The seed block every question draws from.
    #[must_use]
    pub const fn seed_base(&self) -> u64 {
        self.seed_base
    }

    /// The router, ready to serve.
    #[must_use]
    pub fn router(self: Arc<Self>) -> axum::Router {
        axum::Router::new()
            .route("/v1/systemone", post(system_one))
            .route("/v1/models", get(models))
            .with_state(self)
    }
}

/// A refusal on the wire.
struct Wire(Refusal);

impl IntoResponse for Wire {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.code.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = json!({
            "error": {
                "code": self.0.code.label(),
                "message": self.0.message,
                "question": self.0.question,
            }
        });
        (status, Json(body)).into_response()
    }
}

async fn models(State(door): State<Arc<Door>>) -> Response {
    let availability = door.pool.availability();
    let (status, reason) = match availability {
        Ok(availability) => (availability.status, availability.reason),
        Err(refusal) => ("unknown".to_string(), Some(refusal.message)),
    };
    Json(json!({
        "models": [{
            "name": door.model,
            // `jev::ModelCard` reads name, description, and release_date, so a
            // client's `models().list()` works against this door too.
            "release_date": "2026-09-19",
            "description": "Apple's on-device foundation model, answering the System One contract. \
                            Its probabilities are seeded-sampling frequencies, not calibrated \
                            predictive probabilities.",
            "availability": status,
            "unavailable_reason": reason,
            "estimator": Estimator::L2.label(),
            "samples": door.samples,
            "seed_base": door.seed_base,
            "pool_width": door.pool.width(),
            "resolution": 1.0 / door.samples as f64,
            // The release, not the path. A path is machine-local, and three
            // adapters that differ only by run directory are three paths and
            // one name. Every row and every record this door's answers
            // produce carry this string.
            "adapter": door.adapter_id(),
            // The document the rest of this card is keyed off, when the door
            // was started from one.
            "manifest": door.manifest.as_ref().map(|manifest| json!({
                "schema": manifest.schema,
                "release": manifest.release(),
                "version": manifest.version,
                "artifact_sha256": manifest.artifact.as_ref().map(|artifact| &artifact.sha256),
                "min_os_build": manifest.base.min_os_build,
                "admitted_families": manifest.admitted_families(),
            })),
            // The signature a calibration record has to match to serve here.
            "base_model_signature": door.base_signature(),
            "os_build": door.os_build,
            // Read from the records this door actually opened, not declared.
            "calibration": if door.calibration.is_empty() { "none" } else { "fitted" },
            "calibrated_families": door.calibration.families(),
            // Every record that did not survive the check, with the field
            // that refused it. A door holding maps it may not serve says so.
            "calibration_refused": door
                .calibration
                .refused
                .iter()
                .map(|(named, reason)| json!({
                    "record": named,
                    "reason": reason.to_string(),
                }))
                .collect::<Vec<Value>>(),
            "question_types": ["noul", "choice", "score"],
            "max_options": MAX_CHOICE_OPTIONS,
            "max_levels": MAX_SCORE_LEVELS,
        }]
    }))
    .into_response()
}

async fn system_one(State(door): State<Arc<Door>>, body: String) -> Response {
    let request: SystemOneRequest = match serde_json::from_str(&body) {
        Ok(request) => request,
        Err(error) => {
            return Wire(Refusal::new(
                RefusalCode::InvalidRequest,
                format!("the request body did not parse: {error}"),
            ))
            .into_response();
        }
    };

    match answer_request(&door, &request) {
        Ok(response) => Json(response).into_response(),
        Err(refusal) => Wire(refusal).into_response(),
    }
}

fn answer_request(door: &Door, request: &SystemOneRequest) -> crate::error::Result<SystemOneResponse> {
    let family = request.extensions.family.as_deref().unwrap_or_default();
    let fitted = if family.is_empty() { None } else { door.calibration.record(family) };
    if fitted.is_none() && request.extensions.require_calibration {
        return Err(Refusal::new(
            RefusalCode::Uncalibrated,
            uncalibrated_reason(&door.calibration, family),
        ));
    }

    let compiled = compile(request)?;
    let availability = door.pool.availability()?;
    if !availability.is_available() {
        return Err(Refusal::new(
            RefusalCode::ModelUnavailable,
            format!(
                "the on-device model is {}{}",
                availability.status,
                availability.reason.map(|reason| format!(": {reason}")).unwrap_or_default()
            ),
        ));
    }

    let mut answers = IndexMap::new();
    let mut estimates = IndexMap::new();
    for (id, question) in &compiled {
        let raw = l2_pool_with(
            &door.pool,
            question,
            door.samples,
            door.seed_base,
            door.adapter.as_deref(),
        )
        .map_err(|refusal| with_question(refusal, id))?;
        // The map rescales the distribution the estimator observed, and the
        // typed answer is read off the rescaled one. Applying it here rather
        // than to the answer keeps one code path: a Noul, a Choice, and a
        // Score all carry a distribution, and only one of them is rescaled
        // correctly by hand.
        let distribution = match fitted {
            Some(record) => record.map.apply_distribution(&raw.frequency),
            None => raw.frequency.clone(),
        };
        let typed = answer(question.kind, &distribution, &question.legend)
            .map_err(|refusal| with_question(refusal, id))?;
        answers.insert(id.clone(), typed);
        if request.extensions.estimator {
            estimates.insert(
                id.clone(),
                json!({
                    "estimator": raw.estimator.label(),
                    "samples": door.samples,
                    "pool_width": door.pool.width(),
                    "seed_base": raw.seed_base,
                    "seeds": raw.seeds,
                    "resolution": raw.resolution,
                    "refused_draws": raw.refused,
                    "latency_ms": raw.latency_ms,
                }),
            );
        }
    }

    Ok(SystemOneResponse {
        model: door.model.clone(),
        answers,
        // Apple bills no tokens and the runtime surfaces no counts, so this
        // stays empty rather than carrying a character-count fiction.
        usage: Usage::default(),
        extensions: extensions(request, fitted, estimates),
    })
}

/// Why this door holds no map for what the caller asked.
///
/// Three different facts, and a caller that cannot tell them apart will go
/// looking in the wrong place: nothing was asked for, nothing covers it, or
/// something covers it and does not match this door.
fn uncalibrated_reason(calibration: &Calibration, family: &str) -> String {
    if family.is_empty() {
        return "this door serves a fitted map only for a named question family, and the \
                request named none. Send `extensions.family`. See docs/lev/calibration.md."
            .to_string();
    }
    match calibration.refusal(family) {
        // The map exists and lost. Nothing about this door is wrong, and
        // telling a caller to refit it against this door would be advice to
        // repeat a measurement that already answered.
        Some(Refused::Door(Mismatch::NotAdmitted { verdict })) => {
            return format!(
                "a calibration map for `{family}` was fitted and the gate refused it: {verdict}. \
                 See docs/lev/calibration.md."
            );
        }
        Some(reason) => {
            return format!(
                "a calibration record covers `{family}` and does not match this door — {reason}. \
                 Refit it against this door, or serve it from the door it was fitted for."
            );
        }
        None => {}
    }
    let held = calibration.families();
    if held.is_empty() {
        format!(
            "this door serves seeded-sampling frequencies and holds no fitted calibration map \
             for `{family}`. See docs/lev/calibration.md."
        )
    } else {
        format!(
            "this door holds no fitted calibration map for `{family}`. It serves {}.",
            held.join(", ")
        )
    }
}

fn extensions(
    request: &SystemOneRequest,
    fitted: Option<&Record>,
    estimates: IndexMap<String, Value>,
) -> Option<Value> {
    let calibration = match fitted {
        Some(record) => json!({
            "state": "calibrated",
            "family": record.family,
            "map_fitted_on": record.map.fitted_on,
            "suite": record.suite,
            "suite_digest": record.suite_digest,
            "partition_id": record.partition_id,
            "gate_id": record.gate_id,
            "gate_digest": record.gate_digest,
            "fitted": record.fitted,
            "verdict": record.verdict,
            "meaning": "probabilities are the frequency with which the model selected each \
                        option, rescaled by a reliability table fitted against labelled \
                        outcomes for this family on the door named in the record.",
        }),
        None => json!({
            "state": "uncalibrated",
            "meaning": "probabilities are the frequency with which the model selected each option \
                        across seeded samples. They measure decoding consistency, not correctness. \
                        Do not gate an action on them without fitting a map on your own labelled \
                        outcomes.",
        }),
    };
    if request.extensions.estimator {
        Some(json!({ "calibration": calibration, "estimator": estimates }))
    } else {
        Some(json!({ "calibration": calibration }))
    }
}

fn with_question(mut refusal: Refusal, id: &str) -> Refusal {
    if refusal.question.is_none() {
        refusal.question = Some(id.to_string());
    }
    refusal
}

#[cfg(test)]
mod tests {
    use super::*;
    use gym::calibrate::{EstimatorConfig, Map, Metrics, Observation, RECORD_SCHEMA};

    const BASE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";

    fn record(family: &str, identity: DoorIdentity, admitted: bool) -> Record {
        Record {
            schema: RECORD_SCHEMA.to_string(),
            family: family.to_string(),
            estimator_config: EstimatorConfig::new("l2", 8, 0),
            language: "en".to_string(),
            suite: "support-v2-three-way".to_string(),
            suite_digest: "54fbf4137c".to_string(),
            partition_id: "calibration".to_string(),
            os_build: "25E246".to_string(),
            door: "lev-base".to_string(),
            door_identity: identity,
            gate_id: Some("probability-v1".to_string()),
            gate_digest: Some("gate:abc".to_string()),
            locked_reads: Vec::new(),
            fitted: "2026-09-19".to_string(),
            map: Map::fit(&[Observation::new(1.0, true), Observation::new(1.0, false)], 2),
            raw_metrics: Metrics::default(),
            calibrated_metrics: Metrics::default(),
            admitted,
            verdict: if admitted { "admitted: test" } else { "refused: Brier rose" }.to_string(),
        }
    }

    fn write(dir: &Path, record: &Record) {
        let text = serde_json::to_string_pretty(record).expect("a record serializes");
        std::fs::write(dir.join(format!("{}.json", record.family)), text).expect("it writes");
    }

    #[test]
    fn a_record_fitted_on_this_door_is_served_and_the_rest_are_refused_by_field() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let serving = DoorIdentity::published("lev-base", BASE, "");
        write(dir.path(), &record("routing", serving.clone(), true));
        write(
            dir.path(),
            &record(
                "urgency",
                DoorIdentity::published("lev-adapted", BASE, "fmadapter-lev-9799725"),
                true,
            ),
        );
        write(dir.path(), &record("severity", serving.clone(), false));

        let calibration = Calibration::load(dir.path(), "25E246", &serving);
        assert_eq!(calibration.families(), vec!["routing"]);
        assert!(calibration.record("routing").is_some());

        // A record fitted against another door is refused, and the refusal
        // names the field that refused it.
        let adapter =
            calibration.refusal("urgency").expect("the adapted map is refused").to_string();
        assert!(adapter.starts_with("adapter:"), "{adapter}");
        assert!(adapter.contains("fmadapter-lev-9799725"), "{adapter}");

        let refused = calibration.refusal("severity").expect("an unadmitted map is refused");
        assert!(matches!(refused, Refused::Door(Mismatch::NotAdmitted { .. })), "{refused}");
    }

    /// A manifest naming the records in `dir` that `families` covers.
    fn manifest(dir: &Path, families: &[&str]) -> Manifest {
        let mut manifest = Manifest {
            schema: crate::manifest::MANIFEST_SCHEMA.to_string(),
            name: "lev-base".to_string(),
            version: 1,
            description: "a test release".to_string(),
            created: "2026-09-19".to_string(),
            artifact: None,
            base: crate::manifest::Base {
                signature: BASE.to_string(),
                min_os_build: "25E246".to_string(),
                runtime: "Apple FoundationModels".to_string(),
            },
            interface: crate::manifest::Interface::of_contract(Vec::new()),
            estimator: EstimatorConfig::new("l2", 8, 0),
            eval_ref: Vec::new(),
            source: dir.to_path_buf(),
        };
        for family in families {
            let path = dir.join(format!("{family}.json"));
            let digest = crate::manifest::digest_of(&path).expect("the record hashes");
            let text = std::fs::read_to_string(&path).expect("the record reads");
            let record = Record::from_json(&text).expect("the record parses");
            manifest.eval_ref.push(crate::manifest::EvalRef::of_record(
                &record,
                format!("{family}.json"),
                digest,
            ));
        }
        manifest
    }

    #[test]
    fn a_record_the_release_does_not_name_does_not_serve() {
        // Both records match the runtime exactly. The manifest names one of
        // them, and only that one serves: a family without a measured
        // `evalRef` does not admit, so dropping a file into the directory is
        // not a measurement.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let serving = DoorIdentity::published("lev-base", BASE, "");
        write(dir.path(), &record("routing", serving.clone(), true));
        write(dir.path(), &record("urgency", serving.clone(), true));

        let loose = Calibration::load(dir.path(), "25E246", &serving);
        assert_eq!(loose.families(), vec!["routing", "urgency"], "without a document, both serve");

        let manifest = manifest(dir.path(), &["routing"]);
        let held = Calibration::load_for(dir.path(), "25E246", &serving, Some(&manifest));
        assert_eq!(held.families(), vec!["routing"]);
        let refused = held.refusal("urgency").expect("the unnamed map is refused");
        assert!(matches!(refused, Refused::Unnamed { .. }), "{refused}");
        assert!(refused.to_string().contains("lev-base@1"), "{refused}");
    }

    #[test]
    fn a_record_edited_after_the_release_was_written_does_not_serve() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let serving = DoorIdentity::published("lev-base", BASE, "");
        write(dir.path(), &record("routing", serving.clone(), true));
        let manifest = manifest(dir.path(), &["routing"]);

        // Refitted in place: the file still parses, still says `admitted`,
        // and now rests on a different suite. This is the shape of the fault
        // that left a base map sitting beside two adapters.
        let mut refitted = record("routing", serving.clone(), true);
        refitted.suite_digest = "0000000000".to_string();
        write(dir.path(), &refitted);

        let held = Calibration::load_for(dir.path(), "25E246", &serving, Some(&manifest));
        assert!(held.is_empty(), "an edited record served");
        let refused = held.refusal("routing").expect("the edited map is refused");
        assert!(matches!(refused, Refused::Changed { .. }), "{refused}");
        assert!(refused.to_string().contains("suiteDigest"), "{refused}");
    }

    #[test]
    fn a_map_fitted_under_another_estimator_does_not_serve() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let serving = DoorIdentity::published("lev-base", BASE, "");
        let mut sixteen = record("routing", serving.clone(), true);
        sixteen.estimator_config = EstimatorConfig::new("l2", 16, 0);
        write(dir.path(), &sixteen);

        // The manifest serves eight draws. A map fitted on sixteen describes
        // a signal this door does not produce.
        let manifest = manifest(dir.path(), &["routing"]);
        let held = Calibration::load_for(dir.path(), "25E246", &serving, Some(&manifest));
        assert!(held.is_empty(), "a map fitted elsewhere served");
        let refused = held.refusal("routing").expect("the map is refused");
        assert!(matches!(refused, Refused::Estimator { .. }), "{refused}");
        assert!(refused.to_string().contains("16 draws"), "{refused}");
    }

    #[test]
    fn a_door_that_cannot_say_what_it_runs_serves_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        write(dir.path(), &record("routing", DoorIdentity::published("lev-base", BASE, ""), true));

        // A door whose runtime published no signature is not a door any map
        // may claim, however well the names line up.
        let nameless = DoorIdentity::published("lev-base", "", "");
        let calibration = Calibration::load(dir.path(), "25E246", &nameless);
        assert!(calibration.is_empty());
        let reason = calibration.refusal("routing").expect("it says why").to_string();
        assert!(reason.starts_with("door_identity.verified:"), "{reason}");
    }

    #[test]
    fn an_absent_directory_is_a_door_with_no_maps_rather_than_an_error() {
        let identity = DoorIdentity::published("lev-base", BASE, "");
        let absent = Path::new("/nonexistent/calibration");
        let calibration = Calibration::load(absent, "25E246", &identity);
        assert!(calibration.is_empty());
        assert!(calibration.refused.is_empty());
    }

    #[test]
    fn a_refusal_says_which_of_the_three_things_went_wrong() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let serving = DoorIdentity::published("lev-base", BASE, "");
        write(dir.path(), &record("routing", serving.clone(), true));
        write(
            dir.path(),
            &record("urgency", DoorIdentity::published("lev-base", "another-base", ""), true),
        );
        let calibration = Calibration::load(dir.path(), "25E246", &serving);

        let unnamed = uncalibrated_reason(&calibration, "");
        assert!(unnamed.contains("extensions.family"), "{unnamed}");

        let mismatched = uncalibrated_reason(&calibration, "urgency");
        assert!(mismatched.contains("base_model_signature:"), "{mismatched}");
        assert!(mismatched.contains("does not match this door"), "{mismatched}");

        let uncovered = uncalibrated_reason(&calibration, "tone");
        assert!(uncovered.contains("holds no fitted calibration map for `tone`"), "{uncovered}");
        assert!(uncovered.contains("It serves routing"), "{uncovered}");
    }

    #[test]
    fn a_calibrated_response_says_what_it_rests_on() {
        let serving = DoorIdentity::published("lev-base", BASE, "");
        let fitted = record("routing", serving, true);
        let request = SystemOneRequest {
            state: json!("a message"),
            model: None,
            questions: IndexMap::new(),
            extensions: crate::api::Extensions {
                family: Some("routing".to_string()),
                ..Default::default()
            },
        };
        let carried = extensions(&request, Some(&fitted), IndexMap::new())
            .expect("a response carries its calibration");
        let calibration = &carried["calibration"];
        assert_eq!(calibration["state"], "calibrated");
        assert_eq!(calibration["family"], "routing");
        assert_eq!(calibration["gate_id"], "probability-v1");
        assert_eq!(calibration["suite_digest"], "54fbf4137c");

        let raw =
            extensions(&request, None, IndexMap::new()).expect("and so does an uncalibrated one");
        assert_eq!(raw["calibration"]["state"], "uncalibrated");
    }
}
