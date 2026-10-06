//! Reviewed, fixed paired checks for exact knowledge candidates.
//! This local profile does not assert population transfer, publish, or award XP.
use crate::study::{Arm, Case, Partition};
use crate::{Entry, Status, digest};
use secp256k1::{Keypair, Secp256k1, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Candidate,
    Admitted,
    Retired,
}

pub const PROFILE: &str = "openagents.knowledge-reviewed-pairs.v1";
pub const POLICY: &str = "Fixed paired checks only: complete known-cost assignments, all subject checks pass, strict pass-count improvement in each independent partition; low-stakes reversible guidance in the declared scope only. No population inference or XP.";
const LIMIT: usize = 4 * 1024 * 1024;

fn canonical<T: Serialize>(record: &T) -> Result<Vec<u8>, String> {
    nostr::contracts::jcs(&serde_json::to_value(record).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn signature_message<T: Serialize>(record: &T) -> Result<[u8; 32], String> {
    let bytes = canonical(record)?;
    if bytes.len() > LIMIT {
        return Err("Prospective record exceeds 4 MiB".into());
    }
    let mut hash = Sha256::new();
    hash.update(PROFILE);
    hash.update([0]);
    hash.update(bytes);
    Ok(hash.finalize().into())
}
fn valid_digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}
fn key(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && XOnlyPublicKey::from_str(s).is_ok()
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn artifact(bytes: &[u8], schema: &str) -> Value {
    json!({"digest":digest(bytes),"size":bytes.len(),"media_type":"application/json","schema":schema})
}

/// Retained local artifact signature using the same BIP-340 identities as Nostr.
/// This is not a new event kind or a public evaluation publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed<T> {
    pub record: T,
    pub signer: String,
    pub signature: String,
}
impl<T: Serialize> Signed<T> {
    pub fn create(record: T, key: &Keypair) -> Result<Self, String> {
        let signature = Secp256k1::signing_only()
            .sign_schnorr_no_aux_rand(&signature_message(&record)?, key)
            .to_string();
        Ok(Self {
            record,
            signer: key.x_only_public_key().0.to_string(),
            signature,
        })
    }
    pub fn verify(&self) -> Result<(), String> {
        let signature = Signature::from_str(&self.signature).map_err(|e| e.to_string())?;
        let public = XOnlyPublicKey::from_str(&self.signer).map_err(|e| e.to_string())?;
        Secp256k1::verification_only()
            .verify_schnorr(&signature, &signature_message(&self.record)?, &public)
            .map_err(|_| "Prospective record signature does not verify".into())
    }
    pub fn digest(&self) -> Result<String, String> {
        Ok(digest(&canonical(self)?))
    }
}

/// Commitment is signed before every assigned trial starts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frozen {
    pub v: String,
    pub candidate_digest: String,
    pub candidate_id: String,
    pub candidate_version: u32,
    pub author: String,
    pub operator: String,
    pub evaluator: String,
    pub configuration_digest: String,
    pub source_tasks: Vec<String>,
    pub source_groups: Vec<String>,
    pub cases: Vec<Case>,
    pub committed_at: u64,
    pub expires_at: u64,
    pub max_total_usd: f64,
    pub scope: String,
    pub policy_digest: String,
}
impl Frozen {
    fn validate(&self, candidate: &str) -> Result<Entry, String> {
        let entry = Entry::parse(candidate)?;
        if self.v != PROFILE
            || self.candidate_digest != digest(candidate.as_bytes())
            || self.candidate_id != entry.id
            || self.candidate_version == 0
            || self.candidate_version != entry.version
            || entry.status != Status::Candidate
            || !key(&self.author)
            || !key(&self.operator)
            || !key(&self.evaluator)
            || self.evaluator == self.author
            || self.evaluator == self.operator
            || !valid_digest(&self.configuration_digest)
            || self.policy_digest != digest(POLICY.as_bytes())
            || self.expires_at <= self.committed_at
            || !self.max_total_usd.is_finite()
            || self.max_total_usd <= 0.0
            || !token(&self.scope)
            || self.cases.len() > 32
            || self.cases.len() < 2
            || self.source_tasks.len() > 64
            || self.source_groups.len() > 64
            || self
                .source_tasks
                .iter()
                .chain(self.source_groups.iter())
                .any(|s| !token(s))
        {
            return Err("Unsupported prospective identity, policy, authority, or bounds".into());
        }
        let mut source_tasks: BTreeSet<_> = self
            .source_tasks
            .iter()
            .flat_map(|s| [s.clone(), crate::evidence::task_of(s)])
            .collect();
        source_tasks.extend(
            entry
                .written_from
                .iter()
                .filter(|s| s.as_str() != "reference")
                .flat_map(|s| [s.clone(), crate::evidence::task_of(s)]),
        );
        for source in &entry.written_from {
            if source != "reference"
                && !self.source_tasks.contains(source)
                && !self.source_groups.contains(source)
            {
                return Err("Frozen source lineage omits candidate construction material".into());
            }
        }
        let mut tasks = BTreeSet::new();
        let mut groups = BTreeMap::new();
        let mut development = false;
        let mut confirmation = false;
        for case in &self.cases {
            if !token(&case.task)
                || !token(&case.group)
                || !valid_digest(&case.workload_digest)
                || !valid_digest(&case.environment_digest)
                || !tasks.insert(&case.task)
                || source_tasks.contains(&case.task)
                || source_tasks.contains(&case.group)
                || self.source_groups.contains(&case.task)
                || self.source_groups.contains(&case.group)
            {
                return Err("Source leakage, duplicate task, or invalid case pin".into());
            }
            if let Some(partition) = groups.insert(&case.group, case.partition)
                && partition != case.partition
            {
                return Err("Development and confirmation groups overlap".into());
            }
            match case.partition {
                Partition::Development => development = true,
                Partition::Confirmation => confirmation = true,
            }
        }
        if !development || !confirmation {
            return Err("Independent development and confirmation partitions are required".into());
        }
        Ok(entry)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub task: String,
    pub arm: Arm,
    pub started_at: u64,
    pub finished_at: u64,
    pub loaded_candidate: Option<String>,
    pub configuration_digest: String,
    pub workload_digest: String,
    pub environment_digest: String,
    pub receipt_digest: String,
    pub receipt: Value,
    pub passed: Option<bool>,
    pub charged_usd: Option<f64>,
}
impl Trial {
    /// Exact retained observation receipt. Signature is the evaluator's report signature.
    pub fn observation(&self) -> Value {
        json!({"v":"openagents.knowledge-check-receipt.v1","task":self.task,"arm":self.arm,
            "started_at":self.started_at,"finished_at":self.finished_at,"loaded_candidate":self.loaded_candidate,
            "configuration_digest":self.configuration_digest,"workload_digest":self.workload_digest,
            "environment_digest":self.environment_digest,"passed":self.passed,"charged_usd":self.charged_usd})
    }
    pub fn seal_receipt(&mut self) -> Result<(), String> {
        self.receipt = self.observation();
        self.receipt_digest = digest(&canonical(&self.receipt)?);
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub v: String,
    pub plan_digest: String,
    pub completed_at: u64,
    pub trials: Vec<Trial>,
    /// Includes acquisition/setup/checks/runtime; unknown required cost refuses admission.
    pub costs: BTreeMap<String, Option<f64>>,
    pub limitations: Vec<String>,
}
impl Report {
    fn validate(&self, plan: &Signed<Frozen>) -> Result<(), String> {
        let frozen = &plan.record;
        if self.v != "openagents.knowledge-paired-report.v1"
            || self.plan_digest != plan.digest()?
            || self.trials.len() != frozen.cases.len() * 2
            || self.completed_at >= frozen.expires_at
        {
            return Err("Changed commitment or incomplete assignment denominator".into());
        }
        let mut assignments = BTreeSet::new();
        let mut receipts = BTreeSet::new();
        let mut passes = [[0usize; 2]; 2];
        let mut runtime = 0.0;
        for trial in &self.trials {
            let case = frozen
                .cases
                .iter()
                .find(|c| c.task == trial.task)
                .ok_or("Trial is outside the frozen assignment set")?;
            let arm = match trial.arm {
                Arm::Subject => 0,
                Arm::Baseline => 1,
            };
            let partition = match case.partition {
                Partition::Development => 0,
                Partition::Confirmation => 1,
            };
            let cost = trial.charged_usd.ok_or("Unknown required trial cost")?;
            let passed = trial.passed.ok_or("Unknown assigned outcome")?;
            if !assignments.insert((&trial.task, arm))
                || !receipts.insert(&trial.receipt_digest)
                || !valid_digest(&trial.receipt_digest)
                || trial.receipt != trial.observation()
                || trial.receipt_digest != digest(&canonical(&trial.receipt)?)
                || trial.configuration_digest != frozen.configuration_digest
                || trial.workload_digest != case.workload_digest
                || trial.environment_digest != case.environment_digest
                || trial.started_at <= frozen.committed_at
                || trial.finished_at < trial.started_at
                || trial.finished_at > self.completed_at
                || trial.loaded_candidate
                    != (if arm == 0 {
                        Some(frozen.candidate_digest.clone())
                    } else {
                        None
                    })
                || !cost.is_finite()
                || cost < 0.0
            {
                return Err(
                    "Changed loaded candidate, assignment, receipt, timing, or cost identity"
                        .into(),
                );
            }
            if arm == 0 && !passed {
                return Err("Subject failed an assigned check; report remains inconclusive".into());
            }
            passes[partition][arm] += usize::from(passed);
            runtime += cost;
        }
        if passes.iter().any(|counts| counts[0] <= counts[1]) {
            return Err("Declared paired improvement rule did not pass in both partitions".into());
        }
        for category in ["acquisition_usd", "setup_usd", "checks_usd", "runtime_usd"] {
            if !self.costs.contains_key(category) {
                return Err("Required study cost category is missing".into());
            }
        }
        let mut total = 0.0;
        for cost in self.costs.values() {
            let cost = cost.ok_or("Unknown required study cost")?;
            if !cost.is_finite() || cost < 0.0 {
                return Err("Invalid study cost".into());
            }
            total += cost;
        }
        if !total.is_finite()
            || total > frozen.max_total_usd
            || self.costs["runtime_usd"] != Some(runtime)
        {
            return Err(
                "Complete study charges exceed the commitment or omit trial charges".into(),
            );
        }
        Ok(())
    }
}

/// Explicit operator review carries the existing NIP-EVAL admission shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub v: String,
    pub plan_digest: String,
    pub report_digest: String,
    pub reviewed_at: u64,
    pub admission: Value,
}

/// Prepare review bytes, never sign or approve them automatically.
pub fn review_draft(
    plan: &Signed<Frozen>,
    report: &Signed<Report>,
    candidate: &str,
    reviewed_at: u64,
) -> Result<Review, String> {
    plan.verify()?;
    report.verify()?;
    plan.record.validate(candidate)?;
    report.record.validate(plan)?;
    let frozen = &plan.record;
    if plan.signer != frozen.operator || report.signer != frozen.evaluator {
        return Err("Commitment or report signer lacks the frozen role".into());
    }
    let mut subject = artifact(candidate.as_bytes(), "openagents.kb-entry.v1");
    subject["media_type"] = json!("text/markdown");
    let report_ref = artifact(&canonical(report)?, "openagents.knowledge-paired-report.v1");
    let mut policy_ref = artifact(POLICY.as_bytes(), PROFILE);
    policy_ref["media_type"] = json!("text/plain");
    Ok(Review {
        v: "openagents.knowledge-operator-review.v1".into(),
        plan_digest: plan.digest()?,
        report_digest: report.digest()?,
        reviewed_at,
        admission: json!({"v":nostr::eval_ext::ADMISSION_SCHEMA,"requires":[],"subject":{"id":format!("{}:knowledge/{}",frozen.author,frozen.candidate_id.replace('.',"-")),"artifact":subject},
            "reports":[report_ref.clone()],"validation":[report_ref],"policy":{"id":format!("{}:knowledge/reviewed-pairs",frozen.operator),"artifact":policy_ref},
            "scope":artifact(&canonical(&frozen.scope)?, "openagents.knowledge-scope.v1"),"decision":"admit","issuer":frozen.operator,"expires_at":frozen.expires_at,
            "stakes":{"severity":"low","authority":"none","reversibility":"reversible"}}),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retirement {
    pub v: String,
    pub admission_digest: String,
    pub retired_at: u64,
    pub reason: String,
}

/// Reader-selected identities; publisher declarations cannot choose their own trust.
#[derive(Clone, Debug)]
pub struct Trust {
    pub operator: String,
    pub evaluator: String,
}

/// Complete retained evidence, including negative/unknown reports and explicit review.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub plan: Signed<Frozen>,
    pub report: Signed<Report>,
    pub review: Option<Signed<Review>>,
    pub retirements: Vec<Signed<Retirement>>,
}
impl Bundle {
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > LIMIT as u64 {
            return Err("Evidence must be a bounded regular file".into());
        }
        let value = nostr::contracts::parse_strict_bounded(
            &std::fs::read(path).map_err(|e| e.to_string())?,
            LIMIT,
        )
        .map_err(|e| e.to_string())?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        let bytes = canonical(self)?;
        if bytes.len() > LIMIT {
            return Err("Evidence exceeds 4 MiB".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())
    }
    /// Verify the independent report without requiring an operator admission.
    /// Local study results cannot satisfy this signed prospective profile.
    pub fn check_report(&self, candidate: &str, trust: &Trust) -> Result<(), String> {
        if trust.operator != self.plan.record.operator
            || trust.evaluator != self.plan.record.evaluator
        {
            return Err("Reader does not trust the declared operator/evaluator".into());
        }
        self.plan.verify()?;
        self.report.verify()?;
        let frozen = &self.plan.record;
        frozen.validate(candidate)?;
        self.report.record.validate(&self.plan)?;
        if self.plan.signer != frozen.operator || self.report.signer != frozen.evaluator {
            return Err("Evidence signer does not hold its precommitted role".into());
        }
        Ok(())
    }
    /// Read-only assessment. A local kb-study result cannot satisfy this profile.
    pub fn state(&self, candidate: &str, now: u64, trust: &Trust) -> Result<State, String> {
        self.check_report(candidate, trust)?;
        let frozen = &self.plan.record;
        let review = self
            .review
            .as_ref()
            .ok_or("Operator review is missing; candidate remains inconclusive")?;
        review.verify()?;
        let expected = review_draft(
            &self.plan,
            &self.report,
            candidate,
            review.record.reviewed_at,
        )?;
        if review.signer != frozen.operator
            || review.record.v != expected.v
            || review.record.plan_digest != expected.plan_digest
            || review.record.report_digest != expected.report_digest
            || review.record.admission != expected.admission
            || review.record.reviewed_at < self.report.record.completed_at
            || review.record.reviewed_at > now
        {
            return Err(
                "Unreviewed or changed candidate, report, scope, or operator decision".into(),
            );
        }
        let admission = nostr::eval_ext::parse_admission(&canonical(&review.record.admission)?)
            .map_err(|e| e.to_string())?;
        if admission.decision != "admit" {
            return Err("Operator did not admit this exact candidate".into());
        }
        for retirement in &self.retirements {
            retirement.verify()?;
            if retirement.signer != frozen.operator
                || retirement.record.v != "openagents.knowledge-retirement.v1"
                || retirement.record.admission_digest != review.digest()?
                || retirement.record.retired_at < review.record.reviewed_at
                || retirement.record.retired_at > now
                || retirement.record.reason.trim().is_empty()
            {
                return Err("Invalid retirement record".into());
            }
        }
        Ok(
            if now >= frozen.expires_at || !self.retirements.is_empty() {
                State::Retired
            } else {
                State::Admitted
            },
        )
    }
    /// Explicit owner activation; a report, review, or pane projection does not invoke it.
    pub fn activate(
        &self,
        candidate: &str,
        grant: &ActivationGrant,
        now: u64,
    ) -> Result<Activated, String> {
        let admission_digest = self
            .review
            .as_ref()
            .ok_or("Operator review is missing")?
            .digest()?;
        if grant.configuration_digest != self.plan.record.configuration_digest
            || grant.candidate_digest != self.plan.record.candidate_digest
            || grant.admission_digest != admission_digest
            || grant.retired_admissions.contains(&admission_digest)
        {
            return Err("Current candidate/admission pin is missing or retired".into());
        }
        if !grant.instructions
            || !grant.disclosure
            || grant.operator != self.plan.record.operator
            || grant.scope != self.plan.record.scope
            || now >= grant.expires_at
        {
            return Err("Current owner instruction and disclosure authority is required".into());
        }
        if self.state(
            candidate,
            now,
            &Trust {
                operator: grant.operator.clone(),
                evaluator: grant.evaluator.clone(),
            },
        )? != State::Admitted
        {
            return Err("Admission is expired or retired".into());
        }
        Ok(Activated {
            entry: Entry::parse(candidate)?,
            candidate_digest: self.plan.record.candidate_digest.clone(),
        })
    }
}

/// Trusted local admission context, supplied by the owner after its existing grant check.
/// This is intentionally not a deserializable remote permission.
pub struct ActivationGrant {
    pub operator: String,
    pub evaluator: String,
    pub scope: String,
    pub expires_at: u64,
    pub candidate_digest: String,
    pub configuration_digest: String,
    pub admission_digest: String,
    /// Current owner retirement index, independent of the supplied historical bundle.
    pub retired_admissions: BTreeSet<String>,
    pub instructions: bool,
    pub disclosure: bool,
}
pub struct Activated {
    entry: Entry,
    pub candidate_digest: String,
}
impl Activated {
    pub fn guidance(&self) -> &str {
        &self.entry.body
    }
}

#[cfg(test)]
mod tests;

/// The workbench displays review state without activating instructions.
pub struct Adapter {
    pub candidate: crate::workbench::Adapter,
    pub evidence: Bundle,
    pub trust: Trust,
}
impl ::workbench::pane::PaneAdapter for Adapter {
    fn kind(&self) -> ::workbench::pane::PaneKind {
        ::workbench::pane::PaneKind::Knowledge
    }
    fn describe(&self, subject: &::workbench::pane::Subject) -> ::workbench::pane::Description {
        use ::workbench::pane::PaneState;
        let mut description = self.candidate.describe(subject);
        if description.state != PaneState::Ready {
            return description;
        }
        let Some(candidate) = self.candidate.session.candidates.last() else {
            return description;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let assessment = self.evidence.state(&candidate.bytes, now, &self.trust);
        match assessment {
            Ok(state) => {
                let label = match state {
                    State::Candidate => "Candidate",
                    State::Admitted => "Admitted",
                    State::Retired => "Retired",
                };
                description.title = format!("Knowledge: {label}");
                description.detail = crate::workbench::bounded_detail(&format!(
                    "{label}: exact candidate {}. Scope: {}. Viewing does not activate instructions or award XP.\n{}",
                    candidate.digest, self.evidence.plan.record.scope, description.detail
                ));
                description.actions = vec!["inspect".into(), "review-records".into()];
            }
            Err(error) => {
                description.title = "Knowledge: Candidate".into();
                description.detail = crate::workbench::bounded_detail(&format!(
                    "Prospective evidence is inconclusive: {error}. All retained outcomes remain inspectable.\n{}",
                    description.detail
                ));
                description.actions = vec!["inspect".into(), "review-records".into()];
            }
        }
        description
    }
}
