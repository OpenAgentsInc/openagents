//! Calibrating decision verification, and a machine's self-check.
//!
//! `run` sends the seeded items of [`crate::items`] to a System One server
//! one at a time and records each answer with the server's identity.
//! `compare` reads two such runs and reports the distance distribution,
//! top-answer agreement, and the share of simulated rounds that would
//! pass or strike under [`crate::verify::Limits`]. Running the same items
//! on two machines (or two backends, or a substitute model) is how the
//! limits were chosen; see `docs/research/decisionmodels/calibration.md`.
//!
//! `selfcheck` is the same comparison against a reference set shipped in
//! this crate: before a computer serves decisions it answers those items
//! locally and refuses to serve when its answers would lose rounds. REAX's
//! `reax-selfcheck` showed the value of the check (one of their rented
//! hosts drifted far more than its card type); this is our version.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::items::{self, Item};
use crate::verify::{self, Limits, Spread};

/// One answered item of a calibration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answered {
    pub id: String,
    pub seed: u64,
    pub index: u32,
    /// The question (also regenerable from `seed` and `index`).
    #[serde(default)]
    pub question: Value,
    /// The answer object for the one question, or `null` when the server
    /// refused or failed (checked-in runs drop it; `probabilities` keeps
    /// what the comparison needs).
    #[serde(default)]
    pub answer: Value,
    /// The answer's probabilities in option order, when it is consistent.
    pub probabilities: Option<Vec<f64>>,
    /// Why the answer is unusable, when it is.
    pub problem: Option<String>,
    pub ms: u64,
    /// The server's artifact digest and backend, as it reported them.
    pub artifact_digest: Option<String>,
    pub backend: Option<String>,
}

/// A System One server to ask.
pub struct Server {
    base: String,
    model: String,
    client: reqwest::Client,
}

impl Server {
    /// The server at `base` serving `model`.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(base: &str, model: &str) -> Result<Self, String> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            model: model.to_string(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?,
        })
    }

    /// Ask one item.
    pub async fn answer(&self, item: &Item, seed: u64, index: u32) -> Answered {
        let started = Instant::now();
        let result = self
            .client
            .post(format!("{}/v1/systemone", self.base))
            .json(&item.body(&self.model))
            .send()
            .await;
        let ms = started.elapsed().as_millis() as u64;
        let body: Value = match result {
            Ok(response) => response.json().await.unwrap_or(Value::Null),
            Err(e) => json!({"error": {"message": e.to_string()}}),
        };
        answered(item, seed, index, &body, ms)
    }
}

/// Turn a `/v1/systemone` response body into an [`Answered`].
#[must_use]
pub fn answered(item: &Item, seed: u64, index: u32, body: &Value, ms: u64) -> Answered {
    let answer = body["answers"][&item.question_id].clone();
    let (probabilities, problem) = if answer.is_null() {
        let why = body["error"]["message"]
            .as_str()
            .unwrap_or("no answer")
            .to_string();
        (None, Some(why))
    } else {
        match verify::consistent(&item.question, &answer) {
            Ok(p) => (Some(p), None),
            Err(why) => (None, Some(why)),
        }
    };
    let digest = body["psionic"]["artifact_digest"]
        .as_str()
        .or_else(|| body["service"]["identity"].as_str())
        .map(str::to_string);
    Answered {
        id: item.id.clone(),
        seed,
        index,
        question: item.question.clone(),
        answer,
        probabilities,
        problem,
        ms,
        artifact_digest: digest,
        backend: body["psionic"]["backend"].as_str().map(str::to_string),
    }
}

/// Ask the first `count` items of `seed` and append each answer to `out`
/// as one JSON line. Returns the answers.
///
/// # Errors
///
/// When `out` cannot be written.
pub async fn run(
    server: &Server,
    seed: u64,
    count: u32,
    out: Option<&Path>,
    mut progress: impl FnMut(&Answered),
) -> Result<Vec<Answered>, String> {
    let mut file = match out {
        Some(path) => Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("cannot open {}: {e}", path.display()))?,
        ),
        None => None,
    };
    let mut all = Vec::new();
    for index in 0..count {
        let item = items::item(seed, index);
        let one = server.answer(&item, seed, index).await;
        if let Some(file) = file.as_mut() {
            use std::io::Write;
            let line = serde_json::to_string(&one).map_err(|e| e.to_string())?;
            writeln!(file, "{line}").map_err(|e| e.to_string())?;
        }
        progress(&one);
        all.push(one);
    }
    Ok(all)
}

/// Read a run written by [`run`].
///
/// # Errors
///
/// When the file cannot be read or a line is not an [`Answered`].
pub fn load(path: &Path) -> Result<Vec<Answered>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

/// Two runs compared item by item.
#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    /// Items both runs answered.
    pub paired: usize,
    /// Items one run answered and the other did not, or answered
    /// inconsistently.
    pub unusable: usize,
    pub distance: Spread,
    /// Share of items whose most likely option agrees.
    pub top_agree: f64,
    /// Share of simulated rounds that pass and that strike.
    pub round_pass: f64,
    pub round_strike: f64,
}

/// Compare `candidate` with `reference` under `limits`.
#[must_use]
pub fn compare(reference: &[Answered], candidate: &[Answered], limits: &Limits) -> Comparison {
    let by_id: std::collections::BTreeMap<&str, &Answered> =
        candidate.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut distances = Vec::new();
    let (mut agree, mut unusable) = (0usize, 0usize);
    for r in reference {
        let Some(c) = by_id.get(r.id.as_str()) else {
            continue;
        };
        match (&r.probabilities, &c.probabilities) {
            (Some(p), Some(q)) => {
                distances.push(verify::distance(p, q));
                if argmax(p) == argmax(q) {
                    agree += 1;
                }
            }
            (Some(_), None) => {
                unusable += 1;
                distances.push(f64::INFINITY);
            }
            _ => unusable += 1,
        }
    }
    let (round_pass, round_strike) = verify::simulate(&distances, limits, 20_000, 0x5eed);
    let paired = distances.iter().filter(|d| d.is_finite()).count();
    Comparison {
        paired,
        unusable,
        distance: verify::spread(&distances),
        top_agree: if paired == 0 {
            0.0
        } else {
            agree as f64 / paired as f64
        },
        round_pass,
        round_strike,
    }
}

fn argmax(p: &[f64]) -> usize {
    p.iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i)
}

/// A reference set a computer checks itself against before serving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub schema: String,
    /// The model it was measured on.
    pub model: String,
    /// The artifact it was measured on (`sha256:…`).
    pub artifact_digest: String,
    /// Where it was measured (backend and machine), for the record.
    pub measured_on: String,
    pub seed: u64,
    /// Per item: its index under `seed` and its probabilities.
    pub items: Vec<(u32, Vec<f64>)>,
}

impl Reference {
    /// Build a reference set from a run.
    #[must_use]
    pub fn from_run(model: &str, measured_on: &str, run: &[Answered]) -> Option<Self> {
        let first = run.first()?;
        let items: Vec<(u32, Vec<f64>)> = run
            .iter()
            .filter_map(|a| a.probabilities.clone().map(|p| (a.index, p)))
            .collect();
        Some(Self {
            schema: "openagents-decision-reference/1".into(),
            model: model.into(),
            artifact_digest: first.artifact_digest.clone().unwrap_or_default(),
            measured_on: measured_on.into(),
            seed: first.seed,
            items,
        })
    }

    /// As [`Answered`] rows, for [`compare`].
    #[must_use]
    pub fn rows(&self) -> Vec<Answered> {
        self.items
            .iter()
            .map(|(index, p)| {
                let item = items::item(self.seed, *index);
                Answered {
                    id: item.id,
                    seed: self.seed,
                    index: *index,
                    question: item.question,
                    answer: Value::Null,
                    probabilities: Some(p.clone()),
                    problem: None,
                    ms: 0,
                    artifact_digest: Some(self.artifact_digest.clone()),
                    backend: None,
                }
            })
            .collect()
    }
}

/// The reference set for Clef-Flash Q4_K_M (Hugging Face
/// `ggml-org/Clef-Flash-GGUF` at `6b4fe4c5`, sha256 `36261166…6868a2`),
/// measured with Psionic on Metal.
pub const CLEF_FLASH_REFERENCE: &str = include_str!("../data/selfcheck/clef-flash-q4_k_m.json");

/// The shipped reference set for `artifact_digest`, if there is one.
#[must_use]
pub fn shipped_reference(artifact_digest: &str) -> Option<Reference> {
    let reference: Reference = serde_json::from_str(CLEF_FLASH_REFERENCE).ok()?;
    (reference.artifact_digest == artifact_digest).then_some(reference)
}

/// A self-check's result, kept in `selfcheck.json` under the pylon home.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfCheck {
    pub at: u64,
    pub model: String,
    pub artifact_digest: String,
    pub backend: Option<String>,
    pub items: usize,
    pub unusable: usize,
    pub median: f64,
    pub p99: f64,
    pub max: f64,
    pub top_agree: f64,
    pub round_pass: f64,
    pub round_strike: f64,
    pub decisions_per_second: f64,
    pub pass: bool,
}

/// The pass line: every simulated round strikes never, and at least this
/// share passes.
pub const SELFCHECK_PASS: f64 = 0.99;

/// The file the last self-check is kept in.
pub const SELFCHECK_FILE: &str = "selfcheck.json";

/// Check the server at `server` against `reference`, at most `limit`
/// items. Inconsistent or missing answers fail the check.
pub async fn selfcheck(
    server: &Server,
    reference: &Reference,
    limit: usize,
    limits: &Limits,
    mut progress: impl FnMut(&Answered),
) -> SelfCheck {
    let started = Instant::now();
    let mut answers = Vec::new();
    let mut backend = None;
    for (index, _) in reference.items.iter().take(limit) {
        let item = items::item(reference.seed, *index);
        let one = server.answer(&item, reference.seed, *index).await;
        if backend.is_none() {
            backend.clone_from(&one.backend);
        }
        progress(&one);
        answers.push(one);
    }
    let rows: Vec<Answered> = reference.rows().into_iter().take(limit).collect();
    let cmp = compare(&rows, &answers, limits);
    let seconds = started.elapsed().as_secs_f64().max(1e-9);
    let pass = cmp.unusable == 0
        && cmp.paired > 0
        && cmp.round_strike == 0.0
        && cmp.round_pass >= SELFCHECK_PASS;
    SelfCheck {
        at: crate::now(),
        model: reference.model.clone(),
        artifact_digest: reference.artifact_digest.clone(),
        backend,
        items: answers.len(),
        unusable: cmp.unusable,
        median: cmp.distance.median,
        p99: cmp.distance.p99,
        max: cmp.distance.max,
        top_agree: cmp.top_agree,
        round_pass: cmp.round_pass,
        round_strike: cmp.round_strike,
        decisions_per_second: answers.len() as f64 / seconds,
        pass,
    }
}

/// Keep `check` as the last self-check.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save(home: &Path, check: &SelfCheck) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let path = home.join(SELFCHECK_FILE);
    let tmp = home.join(format!("{SELFCHECK_FILE}.tmp"));
    let text = serde_json::to_string_pretty(check).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// The last self-check, if any.
#[must_use]
pub fn last(home: &Path) -> Option<SelfCheck> {
    let text = std::fs::read_to_string(home.join(SELFCHECK_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Whether this computer may serve decisions for `artifact_digest`: the
/// last self-check for that artifact must have passed. `Ok(None)` when
/// there is no self-check for it (the caller warns), `Err` when the last
/// one failed.
///
/// # Errors
///
/// The last self-check for this artifact failed; the message says what to
/// run.
pub fn may_serve(home: &Path, artifact_digest: &str) -> Result<Option<SelfCheck>, String> {
    match last(home) {
        Some(check) if check.artifact_digest == artifact_digest => {
            if check.pass {
                Ok(Some(check))
            } else {
                Err(format!(
                    "this computer's last self-check for {artifact_digest} failed \
                     (simulated rounds: {:.1}% pass, {:.2}% strike, worst item {:.3}); \
                     fix the model server and run `openagents pylon selfcheck` again",
                    check.round_pass * 100.0,
                    check.round_strike * 100.0,
                    check.max
                ))
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(seed: u64, index: u32, p: Vec<f64>) -> Answered {
        let item = items::item(seed, index);
        Answered {
            id: item.id,
            seed,
            index,
            question: item.question,
            answer: Value::Null,
            probabilities: Some(p),
            problem: None,
            ms: 1,
            artifact_digest: Some("sha256:x".into()),
            backend: Some("cpu".into()),
        }
    }

    #[test]
    fn compare_pairs_by_id_and_counts_unusable_answers() {
        let a = vec![fake(1, 0, vec![0.6, 0.4]), fake(1, 1, vec![0.2, 0.8])];
        let mut b = vec![fake(1, 0, vec![0.6, 0.4]), fake(1, 1, vec![0.2, 0.8])];
        let c = compare(&a, &b, &verify::CLEF_FLASH);
        assert_eq!((c.paired, c.unusable), (2, 0));
        assert!(c.round_pass > 0.999);
        b[1].probabilities = None;
        let c = compare(&a, &b, &verify::CLEF_FLASH);
        assert_eq!(c.unusable, 1);
        assert!(c.round_strike > 0.5);
    }

    #[test]
    fn the_shipped_reference_parses_and_names_the_pinned_artifact() {
        let r: Reference = serde_json::from_str(CLEF_FLASH_REFERENCE).unwrap();
        assert_eq!(
            r.artifact_digest,
            "sha256:3626116623ca8d89641952471fac9ce12af84fb3517520029a2ac6f5146868a2"
        );
        assert!(r.items.len() >= 64);
        assert!(shipped_reference(&r.artifact_digest).is_some());
        assert!(shipped_reference("sha256:other").is_none());
        // Every stored vector matches its regenerated item's options.
        for (index, p) in &r.items {
            let item = items::item(r.seed, *index);
            let n = match item.question["type"].as_str() {
                Some("choice") => item.question["criteria"].as_object().unwrap().len(),
                Some("score") => item.question["criteria"].as_array().unwrap().len(),
                _ => 2,
            };
            assert_eq!(p.len(), n, "item {index}");
        }
    }

    #[test]
    fn may_serve_follows_the_last_selfcheck() {
        let dir = tempfile::tempdir().unwrap();
        assert!(may_serve(dir.path(), "sha256:a").unwrap().is_none());
        let mut check = SelfCheck {
            at: 1,
            model: "clef-flash".into(),
            artifact_digest: "sha256:a".into(),
            backend: None,
            items: 8,
            unusable: 0,
            median: 0.0,
            p99: 0.0,
            max: 0.0,
            top_agree: 1.0,
            round_pass: 1.0,
            round_strike: 0.0,
            decisions_per_second: 1.0,
            pass: true,
        };
        save(dir.path(), &check).unwrap();
        assert!(may_serve(dir.path(), "sha256:a").unwrap().is_some());
        assert!(may_serve(dir.path(), "sha256:b").unwrap().is_none());
        check.pass = false;
        save(dir.path(), &check).unwrap();
        assert!(may_serve(dir.path(), "sha256:a").is_err());
    }
}
