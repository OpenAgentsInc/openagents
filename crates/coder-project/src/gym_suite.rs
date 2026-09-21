//! Execute a pinned development suite against two explicitly named local doors.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gym::{
    calibrate, eval, gate,
    questions::QuestionSet,
    row::DoorIdentity,
    suite::{Partition, Suite},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// A local endpoint; the native SDK forbids credentials, redirects, and proxies.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Door {
    pub url: String,
    pub model: String,
}

/// Host-prepared identities and bounds for one measured comparison.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub suite: PathBuf,
    pub suite_digest: String,
    pub questions: PathBuf,
    pub question_digest: String,
    pub gate: PathBuf,
    pub gate_digest: String,
    pub input_digest: String,
    pub baseline: Door,
    pub candidate: Door,
    pub max_items: usize,
    pub seconds: u64,
}

/// The gate result and retained measurements. Publication is the caller's job.
#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub suite_digest: String,
    pub question_digest: String,
    pub gate_digest: String,
    pub input_digest: String,
    pub verdict: coder::verification::Verdict,
    pub outcome: gate::Outcome,
    pub measurements: serde_json::Value,
}

/// Measure both doors on the same development items and apply the pinned gate.
/// Locked and calibration partitions are never consumed by this adapter.
pub async fn run(plan: &Plan) -> Result<Report, String> {
    if plan.input_digest.is_empty()
        || !(1..=1000).contains(&plan.max_items)
        || !(1..=3600).contains(&plan.seconds)
    {
        return Err("suite measurement requires a pinned input and finite item/time bounds".into());
    }
    let suite = Suite::load(&read_document(&plan.suite)?).map_err(|e| e.to_string())?;
    let questions = QuestionSet::from_json(&read_document(&plan.questions)?, &plan.questions)
        .map_err(|e| e.to_string())?;
    let gate = gate::Gate::from_json(&read_document(&plan.gate)?, &plan.gate)
        .map_err(|e| e.to_string())?;
    if questions.suite != suite.name
        || suite.digest != plan.suite_digest
        || questions.digest() != plan.question_digest
        || gate.digest() != plan.gate_digest
    {
        return Err("suite, question, or gate content differs from its host pin".into());
    }
    let items = suite
        .partition(Partition::Development)
        .map_err(|e| e.to_string())?;
    if items.is_empty() || items.len() > plan.max_items {
        return Err("development partition is empty or exceeds the item bound".into());
    }
    for item in &items {
        questions.ask(item).map_err(|e| e.to_string())?;
    }
    // Validate both transports before the first call. Local execution cannot
    // silently acquire a hosted fallback or inherit a provider credential.
    let clients = [&plan.baseline, &plan.candidate].map(|door| {
        jev::Client::new(
            jev::Config::local(&door.url, &door.model).timeout(Duration::from_secs(plan.seconds)),
        )
        .map_err(|e| e.to_string())
    });
    let [baseline, candidate] = clients;
    let clients = [baseline?, candidate?];
    let started = Instant::now();
    let wall = Duration::from_secs(plan.seconds);
    let mut rows = [Vec::new(), Vec::new()];
    let mut lost = [0usize; 2];
    for item in &items {
        let question = questions.ask(item).map_err(|e| e.to_string())?;
        for (index, (client, door)) in clients
            .iter()
            .zip([&plan.baseline, &plan.candidate])
            .enumerate()
        {
            let remaining = wall
                .checked_sub(started.elapsed())
                .ok_or("suite measurement exceeded its total deadline")?;
            let request = jev::SystemOneRequest::new(
                item.state.clone(),
                jev::Questions::new().with("q", jev::Question::Raw(question.clone())),
            );
            let began = Instant::now();
            let response = tokio::time::timeout(remaining, client.system_one(request))
                .await
                .map_err(|_| "suite measurement exceeded its total deadline")?;
            let disposition = match response {
                Ok(answer) if answer.model == door.model => answer
                    .answers
                    .get("q")
                    .map(eval::read_answer)
                    .unwrap_or_else(|| eval::Disposition::Harness("missing suite answer".into())),
                Ok(_) => eval::Disposition::Harness("answer names a different model".into()),
                Err(error) => eval::classify(&error),
            };
            let run = eval::Run {
                suite: suite.name.clone(),
                suite_digest: suite.digest.clone(),
                question_set: Some(questions.id.clone()),
                question_digest: Some(questions.digest()),
                door: format!("{}:{index}", door.model),
                door_identity: DoorIdentity::hosted(&door.model),
                estimator: "unverified".into(),
                samples: None,
                seed_base: None,
                recorded_at: eval::now_utc(),
                gate_id: Some(gate.id.clone()),
                gate_digest: Some(gate.digest()),
            };
            if let Some(row) = run.row(
                item,
                None,
                &disposition,
                Some(began.elapsed().as_secs_f64() * 1000.0),
            ) {
                rows[index].push(row);
            } else {
                lost[index] += 1;
            }
        }
    }
    let scores = rows
        .each_ref()
        .map(|rows| calibrate::score(&eval::observations(rows)).scores());
    let comparison = gate::Comparison::new(&suite.name, scores[0], scores[1]);
    let outcome = gate.judge(&comparison);
    let verdict = if lost != [0, 0]
        || rows
            .iter()
            .any(|rows| rows.iter().any(|row| row.is_refused()))
    {
        coder::verification::Verdict::Unverifiable
    } else {
        match outcome.verdict {
            gate::Verdict::Passed => coder::verification::Verdict::Passed,
            gate::Verdict::Failed => coder::verification::Verdict::Failed,
            gate::Verdict::Unverifiable => coder::verification::Verdict::Unverifiable,
        }
    };
    Ok(Report {
        schema: "openagents.gym-suite-verification.v1",
        suite_digest: suite.digest.clone(),
        question_digest: questions.digest(),
        gate_digest: gate.digest(),
        input_digest: plan.input_digest.clone(),
        verdict,
        outcome,
        measurements: json!({"partition":"development","expected_items":items.len(),
            "lost":lost,"comparison":comparison,"rows":rows,"elapsed_ms":started.elapsed().as_millis()}),
    })
}

/// Read a regular host document without allowing an unbounded allocation.
pub fn read_document(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("suite documents must be regular files".into());
    }
    let mut text = String::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("suite document exceeds 16 MiB".into());
    }
    Ok(text)
}

/// Convert the measurement into the host's typed verification contract.
pub async fn evidence(plan: &Plan) -> coder::verification::SuiteEvidence {
    let (verdict, details) = match run(plan).await {
        Ok(report) => (report.verdict, json!(report)),
        Err(error) => (
            coder::verification::Verdict::Unverifiable,
            json!({"error":error}),
        ),
    };
    coder::verification::SuiteEvidence {
        schema: "openagents.verification.v1".into(),
        suite_digest: plan.suite_digest.clone(),
        input_digest: plan.input_digest.clone(),
        verdict,
        details: Some(details),
    }
}
