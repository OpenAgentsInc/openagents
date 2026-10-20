//! Independent review evidence over a pinned diff.
//!
//! A reviewer is a pinned checker capability, run under the same read-only
//! candidate boundary [`crate::verification`] checks run under. Its complete
//! stdout is a typed findings document that echoes the inspected artifact's
//! identities, and the host anchors every finding to the diff it captured
//! before a `decide` step may judge one. A reviewer that is unapproved,
//! crashes, hangs, truncates, or answers for another input leaves `refused`,
//! `failed`, or `unverifiable` evidence — never an empty review — and a
//! finding whose path or span the captured diff does not contain is rejected
//! as `unanchored` rather than asked about.
//!
//! Semantic judgment is the `per_finding` `decide` step's business: it asks
//! one typed question per anchored finding and dispositions each answer under
//! an explicit host [`Policy`]. The raw probability is retained; a score is
//! not a correctness claim, and the band between the two thresholds is
//! `unresolved`, not rounded to a verdict. Mechanical failure comes first in
//! the program and cannot be overridden by anything this module reports.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use coder_boundary::{Boundary, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use supervise::{Job, Limits};

use crate::capability::{self, Entry, Trust, Verified};

/// The findings document a reviewer writes as its complete stdout.
pub const FINDINGS_SCHEMA: &str = "openagents.review-findings.v1";

/// The schema the host's collected review evidence carries.
pub const EVIDENCE_SCHEMA: &str = "openagents.review-evidence.v1";

/// The most findings one evidence document may carry.
pub const FINDINGS_MAX: usize = 256;

/// The most of one file's diff text a review state carries.
const FILE_TEXT_CAP: usize = 32 * 1024;

/// The name of a collected evidence record in a trace.
pub const REVIEW_CALL: &str = "review_evidence";

/// A reviewer, pinned the way a verification check is pinned: the manifest
/// by path and digest, the arguments verbatim, and the process bounds.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reviewer {
    /// The capability manifest's absolute path.
    pub manifest: PathBuf,
    /// The digest of the manifest's exact bytes.
    pub manifest_digest: String,
    /// The argv after the adapter, verbatim.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// The wall deadline the reviewer runs under.
    pub seconds: u64,
    /// The most output the run keeps, per stream.
    pub output_bytes: usize,
}

impl Reviewer {
    /// Whether the pin and bounds are ones this host can hold.
    ///
    /// # Errors
    ///
    /// Returns the first rule the specification breaks.
    pub fn validate(&self) -> Result<(), String> {
        if !self.manifest.is_absolute()
            || self.manifest_digest.is_empty()
            || !(1..=3600).contains(&self.seconds)
            || !(1..=1024 * 1024).contains(&self.output_bytes)
            || self.arguments.len() > 128
            || self
                .arguments
                .iter()
                .any(|arg| arg.len() > 65536 || arg.contains('\0'))
        {
            return Err("reviewer identity, arguments, or bounds are invalid".into());
        }
        Ok(())
    }
}

/// The host's disposition policy for a judged finding.
///
/// Both thresholds are the operator's, stated in the plan rather than
/// invented here: `confirm_at` is the probability at or above which a
/// finding stands confirmed, and `dismiss_below` is the probability at or
/// below which it is dismissed. Between them the answer is `unresolved` —
/// an ambiguous review stays ambiguous rather than being rounded to a
/// verdict. There is no universal threshold; what a probability means is a
/// measured property of a door and a workload, which a fixture cannot show.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// The probability at or above which a finding is confirmed.
    pub confirm_at: f64,
    /// The probability at or below which a finding is dismissed.
    pub dismiss_below: f64,
}

impl Policy {
    /// Whether the two thresholds are ones this host can apply.
    ///
    /// # Errors
    ///
    /// Returns why the policy cannot judge.
    pub fn validate(&self) -> Result<(), String> {
        let valid = self.confirm_at.is_finite()
            && self.dismiss_below.is_finite()
            && (0.0..=1.0).contains(&self.confirm_at)
            && (0.0..=1.0).contains(&self.dismiss_below)
            && self.dismiss_below < self.confirm_at;
        match valid {
            true => Ok(()),
            false => Err("finding policy requires 0 <= dismiss_below < confirm_at <= 1".into()),
        }
    }

    /// The disposition a probability earns under this policy.
    #[must_use]
    pub fn judge(&self, probability: f64) -> Disposition {
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            Disposition::Unanswered
        } else if probability >= self.confirm_at {
            Disposition::Confirmed
        } else if probability <= self.dismiss_below {
            Disposition::Dismissed
        } else {
            Disposition::Unresolved
        }
    }
}

/// A span of new-side lines, inclusive and one-based.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// The first line the finding names.
    pub start: u32,
    /// The last line the finding names.
    pub end: u32,
}

impl Span {
    /// Whether the span names a real range of lines.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.start >= 1 && self.end >= self.start && self.end <= 1_000_000
    }
}

/// One finding a reviewer reported.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// The repository-relative path the finding names.
    pub path: String,
    /// The new-side lines it names, when it names any. A finding without a
    /// span anchors to the file as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    /// How serious the reviewer took it to be — a label, not a verdict.
    pub severity: String,
    /// What the finding says is wrong.
    pub summary: String,
    /// The source evidence the reviewer cited, when it cited any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// The typed document a reviewer writes as its complete stdout.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doc {
    /// [`FINDINGS_SCHEMA`], echoed.
    pub schema: String,
    /// The recorded base, echoed.
    pub base: String,
    /// The inspected tip, echoed.
    pub tip: String,
    /// The inspected artifact digest, echoed.
    pub input_digest: String,
    /// The findings, each anchored against the captured diff.
    pub findings: Vec<Finding>,
}

impl Doc {
    /// Whether the document binds the artifact the host inspected.
    ///
    /// # Errors
    ///
    /// Returns why the document is not evidence for this input: a wrong
    /// schema, an identity it does not echo, more findings than the bound,
    /// or a field no consumer could anchor.
    pub fn check(&self, scope: &Scope) -> Result<(), String> {
        if self.schema != FINDINGS_SCHEMA
            || self.base != scope.base
            || self.tip != scope.tip
            || self.input_digest != scope.input_digest
        {
            return Err("findings document does not bind the inspected artifact".into());
        }
        if self.findings.len() > FINDINGS_MAX {
            return Err(format!("findings document exceeds {FINDINGS_MAX} findings"));
        }
        for finding in &self.findings {
            if finding.path.is_empty()
                || finding.path.len() > 1024
                || finding.severity.len() > 64
                || finding.summary.is_empty()
                || finding.summary.len() > 4096
                || finding.evidence.as_ref().is_some_and(|e| e.len() > 8192)
            {
                return Err("findings document carries a finding outside its bounds".into());
            }
        }
        Ok(())
    }
}

/// One file's section of the captured diff.
#[derive(Clone, Debug)]
pub struct FileDiff {
    /// The new-side path the section names.
    pub path: String,
    /// Inclusive new-side line ranges the section adds.
    pub added: Vec<(u32, u32)>,
    /// The section text, held to a bound.
    pub text: String,
    /// Whether the section text was cut at the bound.
    pub truncated: bool,
}

/// Parse a complete unified hunk header into old and new line counts.
fn hunk(line: &str) -> Option<(u32, u32, u32)> {
    fn range(value: &str, prefix: char) -> Option<(u32, u32)> {
        let value = value.strip_prefix(prefix)?;
        let (start, count) = value.split_once(',').unwrap_or((value, "1"));
        let start: u32 = start.parse().ok()?;
        let count: u32 = count.parse().ok()?;
        start.checked_add(count)?;
        Some((start, count))
    }
    let mut parts = line.strip_prefix("@@ ")?.split_whitespace();
    let (_, old_count) = range(parts.next()?, '-')?;
    let (new_start, new_count) = range(parts.next()?, '+')?;
    (parts.next()? == "@@").then_some((old_count, new_start, new_count))
}

/// Keep only complete UTF-8 characters within the per-file evidence bound.
fn append_line(file: &mut FileDiff, line: &str) {
    for part in [line, "\n"] {
        let available = FILE_TEXT_CAP.saturating_sub(file.text.len());
        let mut end = available.min(part.len());
        while !part.is_char_boundary(end) {
            end -= 1;
        }
        file.text.push_str(&part[..end]);
        if end < part.len() {
            file.truncated = true;
            return;
        }
    }
}

/// Parse a unified diff into one section per file.
///
/// Only added lines anchor line findings. Context lines do not. Quoted Git
/// paths remain unsupported and cannot anchor line findings; the host's
/// changed-path inventory still supports findings for those files as a whole.
#[must_use]
pub fn parse_diff(diff: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut current: Option<FileDiff> = None;
    let mut remaining: Option<(u32, u32, u32)> = None;
    for line in diff.lines() {
        if let Some((old, next, new)) = remaining.as_mut()
            && (*old > 0 || *new > 0)
        {
            if let Some(file) = current.as_mut() {
                append_line(file, line);
                match line.as_bytes().first() {
                    Some(b'+') if *new > 0 => {
                        if let Some((_, end)) = file
                            .added
                            .last_mut()
                            .filter(|(_, end)| end.checked_add(1) == Some(*next))
                        {
                            *end = *next;
                        } else {
                            file.added.push((*next, *next));
                        }
                        *next += 1;
                        *new -= 1;
                    }
                    Some(b'-') if *old > 0 => *old -= 1,
                    Some(b' ') if *old > 0 && *new > 0 => {
                        *old -= 1;
                        *new -= 1;
                        *next += 1;
                    }
                    Some(b'\\') => {}
                    _ => {
                        file.added.clear();
                        remaining = None;
                    }
                }
            }
            continue;
        }
        remaining = None;
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            let path = if rest.starts_with('"') {
                String::new()
            } else {
                rest.split_once(" b/")
                    .map(|(_, path)| path.to_owned())
                    .unwrap_or_default()
            };
            let mut file = FileDiff {
                path,
                added: Vec::new(),
                text: String::new(),
                truncated: false,
            };
            append_line(&mut file, line);
            current = Some(file);
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        append_line(file, line);
        if let Some(path) = line.strip_prefix("+++ b/") {
            file.path = path.to_owned();
        } else if let Some(header) = hunk(line) {
            remaining = Some(header);
        }
    }
    if let Some(mut file) = current {
        if remaining.is_some_and(|(old, _, new)| old > 0 || new > 0) {
            file.added.clear();
        }
        files.push(file);
    }
    files
}

/// How a finding sits against the captured diff and the review's scope.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    /// The path is in the captured diff, is not excluded, and any span it
    /// names lies inside lines the diff adds.
    Anchored,
    /// The path is in the captured diff but the operator excluded it from
    /// review scope.
    Excluded,
    /// The path is not in the captured diff, or the span names lines the
    /// diff does not add.
    Unanchored,
}

/// What collecting the reviewer's evidence came to.
///
/// Four states, kept distinct because they mean different things: the
/// reviewer `answered` with a well-formed document (which may carry zero
/// findings); it was `refused` — unapproved, changed, or unrunnable — and
/// never ran; it ran and `failed`; or what it produced is `unverifiable`.
/// Only `answered` may be judged, and an answered document with no findings
/// is a result, not a gap.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The reviewer produced a well-formed document bound to the artifact.
    Answered,
    /// The reviewer never ran: approval, identity, or transport refused it.
    Refused,
    /// The reviewer ran and did not produce the document — a nonzero exit
    /// or a candidate that changed underneath it.
    Failed,
    /// What the reviewer produced cannot be verified: truncated, malformed,
    /// or bound to different identities.
    Unverifiable,
}

impl Outcome {
    /// The word a report or trace records this under.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Answered => "answered",
            Outcome::Refused => "refused",
            Outcome::Failed => "failed",
            Outcome::Unverifiable => "unverifiable",
        }
    }

    /// The refusal code a `per_finding` step stops with over this evidence.
    #[must_use]
    pub fn refusal(&self) -> &'static str {
        match self {
            Outcome::Answered => "review_answered",
            Outcome::Refused => "review_refused",
            Outcome::Failed => "review_failed",
            Outcome::Unverifiable => "review_unverifiable",
        }
    }
}

/// The scope a review is pinned to: the inspected artifact's identities,
/// the paths the diff touches, and the paths the operator excluded.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The recorded task base.
    pub base: String,
    /// The inspected artifact tip.
    pub tip: String,
    /// The inspected artifact digest.
    pub input_digest: String,
    /// The digest of the captured diff text.
    pub diff_digest: String,
    /// The captured diff, verbatim.
    pub diff: String,
    /// Every path the artifact changes.
    pub paths: Vec<String>,
    /// Paths the operator excluded from review.
    pub excluded: Vec<String>,
    /// The diff, parsed per file.
    pub files: Vec<FileDiff>,
}

impl Scope {
    /// Whether one path falls inside the operator's exclusions.
    fn excludes(&self, path: &str) -> bool {
        self.excluded.iter().any(|excluded| {
            path == excluded
                || path
                    .strip_prefix(excluded.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        })
    }

    /// Where one finding stands against the captured diff.
    ///
    /// A path the diff does not touch is unanchored; an excluded path is
    /// excluded; a span must lie inside one added range, and a finding
    /// with no span anchors to the file as a whole.
    #[must_use]
    pub fn anchor(&self, finding: &Finding) -> Anchor {
        if !self.paths.iter().any(|path| path == &finding.path) {
            return Anchor::Unanchored;
        }
        if self.excludes(&finding.path) {
            return Anchor::Excluded;
        }
        match &finding.span {
            None => Anchor::Anchored,
            Some(span) if !span.valid() => Anchor::Unanchored,
            Some(span) => {
                let inside = self
                    .files
                    .iter()
                    .find(|file| file.path == finding.path)
                    .is_some_and(|file| {
                        file.added
                            .iter()
                            .any(|(start, end)| span.start >= *start && span.end <= *end)
                    });
                match inside {
                    true => Anchor::Anchored,
                    false => Anchor::Unanchored,
                }
            }
        }
    }
}

/// One finding with the anchor the host gave it.
#[derive(Clone, Debug, Serialize)]
pub struct AnchoredFinding {
    /// The evidence-local identifier a question is asked under, `f1` up.
    pub id: String,
    /// Where the finding sits against the captured diff.
    pub anchor: Anchor,
    /// The finding as the reviewer emitted it, verbatim.
    pub finding: Finding,
}

/// The host's record of one reviewer run.
///
/// Every identity is the host's own: the pins it inspected, the snapshots
/// it took, the digests of the output it captured. Nothing here is the
/// reviewer's word about itself.
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    /// The operator-pinned reviewer specification.
    pub reviewer: Reviewer,
    /// The explicit host disposition policy, retained for replay.
    pub policy: Policy,
    /// [`EVIDENCE_SCHEMA`].
    pub schema: String,
    /// The recorded task base the review was pinned to.
    pub base: String,
    /// The inspected artifact tip.
    pub tip: String,
    /// The inspected artifact digest.
    pub input_digest: String,
    /// The digest of the captured diff the findings anchor to.
    pub diff_digest: String,
    /// The candidate snapshot before the reviewer ran.
    pub before_snapshot: String,
    /// The candidate snapshot after.
    pub after_snapshot: String,
    /// What collecting came to.
    pub outcome: Outcome,
    /// Why, in words an operator can act on.
    pub reason: String,
    /// The paths the operator excluded from review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<String>,
    /// Every finding the document carried, anchored, in emitted order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<AnchoredFinding>,
    /// How long the reviewer ran.
    pub elapsed_ms: u64,
    /// The digest of the reviewer's captured stdout.
    pub stdout_digest: String,
    /// The digest of its captured stderr.
    pub stderr_digest: String,
    /// Whether either stream was cut at its cap.
    pub output_truncated: bool,
}

/// What a `per_finding` decide step runs against: the pinned reviewer, the
/// captured scope, the disposition policy, and the trust that approves.
///
/// Program text cannot set any of this; the operator installs it beside the
/// verification plan, and a `per_finding` step on a runtime without one
/// refuses at admission.
pub struct Context {
    /// The retained candidate workspace.
    pub workspace: PathBuf,
    /// The pinned reviewer.
    pub reviewer: Reviewer,
    /// The captured scope the review is pinned to.
    pub scope: Scope,
    /// The operator's disposition policy.
    pub policy: Policy,
    /// The trust that approves the reviewer's manifest.
    pub trust: Trust,
}

impl Context {
    /// The state a per-finding question set reads.
    ///
    /// The identities are the host's own pins; `diff` carries each changed
    /// file's section, and `findings` carries the anchored findings being
    /// asked about, under their `f1`-style names. Unanchored and excluded
    /// findings are never asked about, so they are not in the state.
    #[must_use]
    pub fn state(&self, asked: &[AnchoredFinding]) -> Value {
        let mut findings = serde_json::Map::new();
        for finding in asked {
            findings.insert(finding.id.clone(), json!(finding.finding));
        }
        json!({
            "revision": {
                "base": self.scope.base,
                "tip": self.scope.tip,
                "input_digest": self.scope.input_digest,
                "diff_digest": self.scope.diff_digest,
            },
            "scope": {
                "changed": self.scope.paths,
                "excluded": self.scope.excluded,
            },
            "diff": self.scope.files.iter().map(|file| json!({
                "path": file.path,
                "truncated": file.truncated,
                "text": file.text,
            })).collect::<Vec<_>>(),
            "findings": findings,
        })
    }
}

/// What one finding was judged to be.
///
/// `Confirmed`, `dismissed`, and `unresolved` are the policy's reading of a
/// real probability. `Unanswered` is a door that named no answer for a
/// finding it was asked. `Excluded` and `unanchored` are the host's
/// mechanical dispositions — never asked, never overturned by one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// The judged probability met the policy's confirm threshold.
    Confirmed,
    /// The judged probability fell to the policy's dismiss threshold.
    Dismissed,
    /// The judged probability fell between the thresholds: ambiguous, and
    /// kept that way rather than rounded to a verdict.
    Unresolved,
    /// The finding was asked and the door answered nothing for it.
    Unanswered,
    /// The path is inside the operator's exclusions.
    Excluded,
    /// The path or span is not in the captured diff.
    Unanchored,
}

/// One finding after review: the original, its anchor, its disposition,
/// and the raw probability the door returned when one did.
#[derive(Clone, Debug, Serialize)]
pub struct Judged {
    /// The evidence-local identifier the question was asked under.
    pub id: String,
    /// Where the finding sits against the captured diff.
    pub anchor: Anchor,
    /// What review made of it.
    pub disposition: Disposition,
    /// The raw probability the door returned, when it returned one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    /// The finding as the reviewer emitted it, verbatim.
    pub finding: Finding,
}

/// What a `per_finding` step's run recorded: the collected evidence and
/// every finding's disposition, original beside reviewed.
#[derive(Clone, Debug, Serialize)]
pub struct Reviewed {
    /// The collected reviewer evidence — its outcome is the review's.
    pub evidence: Evidence,
    /// The model identity that answered, when one did.
    pub model: String,
    /// Every emitted finding, judged, in emitted order.
    pub findings: Vec<Judged>,
    /// How many findings were anchored and asked.
    pub asked: usize,
    /// How many were confirmed.
    pub confirmed: usize,
    /// How many were dismissed.
    pub dismissed: usize,
    /// How many came back between the policy's thresholds.
    pub unresolved: usize,
    /// How many were asked and went unanswered.
    pub unanswered: usize,
}

impl Reviewed {
    /// The review over evidence that was never judged.
    #[must_use]
    pub fn unreviewed(evidence: Evidence) -> Self {
        Reviewed {
            evidence,
            model: String::new(),
            findings: Vec::new(),
            asked: 0,
            confirmed: 0,
            dismissed: 0,
            unresolved: 0,
            unanswered: 0,
        }
    }

    /// The one-line result the step reports.
    #[must_use]
    pub fn output(&self) -> String {
        let judged = self.findings.len();
        format!(
            "{} confirmed, {} dismissed, {} unresolved, {} unanswered of {judged} findings",
            self.confirmed, self.dismissed, self.unresolved, self.unanswered,
        )
    }
}

/// The environment names a reviewer's argv reads its pins from.
///
/// The reviewer learns the identities it must echo from the host, not from
/// the artifact: it runs under a read-only candidate boundary, in the
/// workspace, with the captured diff at `CODER_REVIEW_DIFF`.
const ENV_BASE: &str = "CODER_REVIEW_BASE";
/// The inspected tip the document must echo.
const ENV_TIP: &str = "CODER_REVIEW_TIP";
/// The inspected artifact digest the document must echo.
const ENV_INPUT: &str = "CODER_REVIEW_INPUT_DIGEST";
/// The digest of the captured diff.
const ENV_DIFF_DIGEST: &str = "CODER_REVIEW_DIFF_DIGEST";
/// The scratch file the captured diff is written to.
const ENV_DIFF: &str = "CODER_REVIEW_DIFF";

/// Collect one reviewer run's evidence over the pinned scope.
///
/// The reviewer runs once, under a read-only boundary over the candidate,
/// bounded in time and output, with snapshots before and after. Its
/// complete stdout is the findings document; anything less — a refusal to
/// run, a crash, a truncated or malformed document, or identities that do
/// not echo the inspected artifact — is recorded as the outcome it is, and
/// none of it is a review with no findings.
///
/// # Errors
///
/// Returns why evidence could not be collected at all: an invalid
/// specification, a workspace that cannot be pinned, an incomplete
/// snapshot, or a boundary this host cannot build.
pub async fn collect(context: &Context) -> Result<Evidence, String> {
    context.reviewer.validate()?;
    context.policy.validate()?;
    let workspace = context
        .workspace
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let before = Snapshot::observe(&workspace);
    if !before.is_complete() {
        return Err("candidate snapshot is incomplete; the reviewer did not run".into());
    }
    let evidence = |outcome: Outcome, reason: String| Evidence {
        reviewer: context.reviewer.clone(),
        policy: context.policy,
        schema: EVIDENCE_SCHEMA.into(),
        base: context.scope.base.clone(),
        tip: context.scope.tip.clone(),
        input_digest: context.scope.input_digest.clone(),
        diff_digest: context.scope.diff_digest.clone(),
        before_snapshot: before.digest(),
        after_snapshot: String::new(),
        outcome,
        reason,
        excluded: context.scope.excluded.clone(),
        findings: Vec::new(),
        elapsed_ms: 0,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        output_truncated: false,
    };
    let entry = match Entry::load(&context.reviewer.manifest, capability::Source::Operator) {
        Ok(entry) => entry,
        Err(reason) => {
            return Ok(evidence(
                Outcome::Unverifiable,
                format!("reviewer manifest cannot be read: {reason}"),
            ));
        }
    };
    if entry.digest != context.reviewer.manifest_digest
        || entry.manifest.transport != capability::SUBPROCESS
    {
        return Ok(evidence(
            Outcome::Refused,
            "reviewer adapter changed or is not a subprocess capability".into(),
        ));
    }
    let adapter = match context.trust.decide_verified(&entry, &workspace) {
        Verified::Approved(record) => record.adapter,
        Verified::Unconditional => {
            match capability::resolve(&entry.manifest.detect.binary, &capability::search_dirs()) {
                Some(adapter) => adapter,
                None => {
                    return Ok(evidence(
                        Outcome::Refused,
                        "reviewer adapter is unavailable".into(),
                    ));
                }
            }
        }
        Verified::Unapproved(reason) => return Ok(evidence(Outcome::Refused, reason)),
    };
    let boundary = Boundary::readonly()
        .protecting(&workspace)
        .sealed(&entry.path)
        .sealed(&adapter)
        .owned_scratch_under(std::env::temp_dir())
        .build()
        .map_err(|error| error.to_string())?;
    let scratch = boundary.scratch().ok_or("reviewer scratch is missing")?;
    let diff_path = scratch.join("captured.diff");
    std::fs::write(&diff_path, &context.scope.diff)
        .map_err(|error| format!("{}: {error}", diff_path.display()))?;
    let mut command = boundary
        .command(&adapter, &context.reviewer.arguments)
        .map_err(|error| error.to_string())?;
    command
        .env_clear()
        .current_dir(&workspace)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", scratch)
        .env("TMPDIR", scratch)
        .env("TMP", scratch)
        .env("TEMP", scratch)
        .env(ENV_BASE, &context.scope.base)
        .env(ENV_TIP, &context.scope.tip)
        .env(ENV_INPUT, &context.scope.input_digest)
        .env(ENV_DIFF_DIGEST, &context.scope.diff_digest)
        .env(ENV_DIFF, &diff_path);
    let begun = Instant::now();
    let ended = Job::from_command(command)
        .bounded(
            Limits::within(Duration::from_secs(context.reviewer.seconds))
                .keeping(context.reviewer.output_bytes),
        )
        .run_holding(boundary.hold())
        .await;
    let elapsed_ms = begun.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let after = Snapshot::observe(&workspace);
    let mut evidence = Evidence {
        after_snapshot: after.digest(),
        elapsed_ms,
        stdout_digest: atif::digest(&json!(ended.stdout.text)),
        stderr_digest: atif::digest(&json!(ended.stderr.text)),
        output_truncated: ended.truncated(),
        ..evidence(
            Outcome::Failed,
            "the reviewer did not produce a document".into(),
        )
    };
    if !after.is_complete() || coder_boundary::compare(&before, &after).is_unverifiable() {
        evidence.outcome = Outcome::Unverifiable;
        evidence.reason = "candidate snapshot is incomplete after the review".into();
        return Ok(evidence);
    }
    if !coder_boundary::compare(&before, &after).is_clean() {
        evidence.reason = "candidate changed while the reviewer ran".into();
        return Ok(evidence);
    }
    if ended.truncated() {
        evidence.outcome = Outcome::Unverifiable;
        evidence.reason = "reviewer output exceeded its cap".into();
        return Ok(evidence);
    }
    if !ended.ending.success() {
        evidence.reason = format!("reviewer ended: {:?}", ended.ending);
        return Ok(evidence);
    }
    let document = match serde_json::from_str::<Doc>(&ended.stdout.text) {
        Ok(document) => document,
        Err(_) => {
            evidence.outcome = Outcome::Unverifiable;
            evidence.reason = "findings document is missing or malformed".into();
            return Ok(evidence);
        }
    };
    if let Err(reason) = document.check(&context.scope) {
        evidence.outcome = Outcome::Unverifiable;
        evidence.reason = reason;
        return Ok(evidence);
    }
    evidence.outcome = Outcome::Answered;
    evidence.reason = format!(
        "reviewer answered with {} findings",
        document.findings.len()
    );
    evidence.findings = document
        .findings
        .iter()
        .enumerate()
        .map(|(n, finding)| AnchoredFinding {
            id: format!("f{}", n + 1),
            anchor: context.scope.anchor(finding),
            finding: finding.clone(),
        })
        .collect();
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_added_lines_without_treating_content_as_headers() {
        let files = parse_diff(
            "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,3 +1,4 @@\n context\n-old\n+new\n+++ b/spoof.rs\n tail\n",
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "a.rs");
        assert_eq!(files[0].added, vec![(2, 3)]);
    }

    #[test]
    fn truncates_multibyte_evidence_and_headers_without_panicking() {
        let text = "é".repeat(FILE_TEXT_CAP);
        let files = parse_diff(&format!("diff --git a/a b/a\n@@ -0,0 +1 @@\n+{text}\n"));
        assert!(files[0].truncated);
        assert!(files[0].text.len() <= FILE_TEXT_CAP);
        assert_eq!(files[0].added, vec![(1, 1)]);
        let files = parse_diff(&format!("diff --git a/{text} b/{text}\n"));
        assert!(files[0].truncated);
        assert!(files[0].text.len() <= FILE_TEXT_CAP);
    }

    #[test]
    fn incomplete_or_overflowing_hunks_cannot_anchor_lines() {
        for header in [
            "@@ -0,0 +1,3 @@\n+only-one\n",
            "@@ -0,0 +4294967295,2 @@\n+overflow\n",
        ] {
            let files = parse_diff(&format!("diff --git a/a b/a\n{header}"));
            assert!(files[0].added.is_empty());
        }
    }

    #[test]
    fn invalid_probabilities_do_not_become_verdicts() {
        let policy = Policy {
            confirm_at: 0.8,
            dismiss_below: 0.2,
        };
        for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert_eq!(policy.judge(value), Disposition::Unanswered);
        }
        assert_eq!(policy.judge(0.5), Disposition::Unresolved);
    }

    #[test]
    fn missing_findings_is_not_an_empty_review() {
        let missing = serde_json::json!({"schema": FINDINGS_SCHEMA, "base": "b", "tip": "t", "input_digest": "d"});
        assert!(serde_json::from_value::<Doc>(missing.clone()).is_err());
        let mut explicit = missing;
        explicit["findings"] = serde_json::json!([]);
        assert!(
            serde_json::from_value::<Doc>(explicit)
                .unwrap()
                .findings
                .is_empty()
        );
    }
}
