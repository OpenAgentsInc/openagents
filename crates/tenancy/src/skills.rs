//! The skill directory — versioned `SKILL.md` submissions, staged
//! review, publication, and moderation.
//!
//! A [`Draft`] names its author, license, category and tags, semver, and
//! content digest, and carries the author's publication consent. The
//! book bounds what a submission may claim (name shape, tag count,
//! consent), deduplicates on `(name, version, digest)`, and refuses a
//! version an author does not own. Review happens in recorded stages —
//! `static` mechanical checks, a `decision` model pass, and a
//! `reasoning` admission synthesis — each pinned to its reviewer and
//! policy version with score, rationale, cost, and failure state. A
//! version publishes only when the book sees the stages admission
//! requires; a failed review leaves the version `rejected`, and a
//! transport failure leaves it `under_review` for an idempotent retry.
//!
//! Markdown content never enters this store: the adapter writes each
//! digest to an object file, and the book records only the digest. The
//! book never executes submitted text — a `SKILL.md` is data here, and
//! every read path treats it as such.
//!
//! Publication is the only state a reader sees: `published` versions
//! browse, search, and serve raw Markdown; `rejected`, `withdrawn`, and
//! `taken_down` versions are invisible outside the author's own
//! submission view. A newer published version marks the older ones
//! `superseded_by` without hiding them — a pinned version URL keeps
//! resolving. Moderation — takedown, reinstate, appeal-granted admit —
//! is an operator act, appended to the audit trail with its actor and
//! reason.
//!
//! # What this is not
//!
//! There is no transport and no clock here: every timestamp arrives as
//! an argument in Unix seconds, and the only entropy is minting
//! submission ids. Model review is a recorded judgment, not a security
//! guarantee — the book's static checks screen for credential-shaped
//! literals, and the host's execution policy still governs anything a
//! skill later tells an agent to do.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::accounts::Trouble;
use crate::sessions::Access;

/// The store's schema tag.
pub const SCHEMA: &str = "openagents.skill-directory.v1";

/// The static validation policy version — bump when the mechanical
/// checks change so a recorded stage can be read against its rules.
pub const STATIC_POLICY: &str = "openagents.skill-static.v1";

/// The decision-review policy version — bump when the question set or
/// the admission gates change.
pub const REVIEW_POLICY: &str = "openagents.skill-review.v1";

/// The moderation policy version recorded on operator admissions.
pub const MODERATION_POLICY: &str = "openagents.skill-moderation.v1";

const SKILLS: &str = "skills.json";
const HISTORY_DIR: &str = "skills-history";
const LOCK: &str = "skills.lock";
const LOCK_RETRIES: u32 = 200;
const STORE_BYTES: u64 = 16 * 1024 * 1024;
const EVENTS_MAX: usize = 8192;
const TAGS_MAX: usize = 8;
const REVIEW_MAX: usize = 64;
const APPEALS_MAX: usize = 16;

/// The admission policy a directory opens with — set at install from
/// the deployment's `skills` config and held in the store so a reopened
/// book admits under the same declared bounds.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Policy {
    /// The largest Markdown body a submission may carry, in bytes.
    pub max_body_bytes: usize,
    /// The submissions one author may open per day.
    pub submissions_per_day: u32,
    /// The under-review submissions one author may hold at once.
    pub pending_per_author: usize,
    /// The quality score a decision review must reach to admit —
    /// applied by the `reasoning` stage, recorded under `review_policy`.
    pub admit_score: f64,
}

/// Where one submitted version stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionState {
    /// Recorded; review is running or a retryable failure left it open.
    UnderReview,
    /// Admitted — discoverable and fetchable.
    Published,
    /// A review stage failed; the record is kept, the content is not
    /// published.
    Rejected,
    /// The author withdrew it after publication.
    Withdrawn,
    /// An operator took it down.
    TakenDown,
}

impl VersionState {
    /// The string form served in API documents.
    pub fn name(&self) -> &'static str {
        match self {
            Self::UnderReview => "under_review",
            Self::Published => "published",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
            Self::TakenDown => "taken_down",
        }
    }
}

/// How one review stage concluded.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageOutcome {
    /// The stage ran and passed.
    Pass,
    /// The stage ran and its checks failed.
    Fail,
    /// The stage could not run — a transport or internal failure that a
    /// resubmission may retry.
    Error,
}

/// One recorded review stage — who ran it, under which policy version,
/// and what it concluded.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReviewStage {
    /// `static`, `decision`, `reasoning`, or `moderation`.
    pub stage: String,
    /// The validator or model that ran the stage.
    pub reviewer: String,
    /// The policy version the stage ran under.
    pub policy: String,
    /// How the stage concluded.
    pub outcome: StageOutcome,
    /// Why a stage failed or errored — bounded, publishable.
    pub detail: Option<String>,
    /// The score the stage produced, when it produces one.
    pub score: Option<f64>,
    /// The stage's rationale — the decision stage records its answer
    /// values; the reasoning stage records its gate outcomes.
    pub rationale: Option<String>,
    /// The stage's cost in backend tokens, when known.
    pub cost: Option<u64>,
    /// When the stage ran, Unix seconds.
    pub at: u64,
}

/// A pinned measurement attached to a version — the suite and report a
/// caller can verify, kept apart from the review's assessed quality.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Evidence {
    /// The pinned suite name or digest.
    pub suite: String,
    /// The report reference.
    pub report: String,
    /// The report's content digest.
    pub digest: String,
    /// When the measurement ran, Unix seconds.
    pub measured_at: u64,
}

/// One submitted version and its lifecycle.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Version {
    /// The semver the submission declared.
    pub version: String,
    /// The submitted content's `sha256:` digest.
    pub digest: String,
    /// The submission that introduced this version.
    pub submission: String,
    /// The account that submitted it.
    pub author: String,
    /// The declared license and rights.
    pub license: String,
    /// The declared category.
    pub category: String,
    /// The declared tags.
    pub tags: Vec<String>,
    /// The version's publication consent — always true on record; a
    /// submission without consent never reaches the book.
    pub consent: bool,
    /// Where the version stands.
    pub state: VersionState,
    /// The newer published version that supersedes this one.
    pub superseded_by: Option<String>,
    /// Measured evidence attached to the version.
    pub evidence: Option<Evidence>,
    /// The recorded review stages.
    pub review: Vec<ReviewStage>,
    /// When the version entered review, Unix seconds.
    pub submitted_at: u64,
    /// When the version reached a terminal state, Unix seconds.
    pub resolved_at: Option<u64>,
    /// Why it resolved — the rejection reason, withdrawal note, or
    /// takedown reason.
    pub resolution: Option<String>,
}

/// An author's appeal against a rejection — recorded, answered by an
/// operator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Appeal {
    /// Why the author believes the rejection is wrong.
    pub reason: String,
    /// When the appeal was filed, Unix seconds.
    pub at: u64,
}

/// A submission's record — what the author sent and where its version
/// stands. Rejected submissions are visible to their author and the
/// operator only; they never appear in the published directory.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Submission {
    /// `sub_<hex>` identity.
    pub id: String,
    /// The account that submitted.
    pub author: String,
    /// The skill name.
    pub name: String,
    /// The declared semver.
    pub version: String,
    /// The declared license and rights.
    pub license: String,
    /// The declared category.
    pub category: String,
    /// The declared tags.
    pub tags: Vec<String>,
    /// The content's `sha256:` digest.
    pub digest: String,
    /// The content's size in bytes.
    pub bytes: usize,
    /// The author's publication consent.
    pub consent: bool,
    /// Measured evidence declared at submission.
    pub evidence: Option<Evidence>,
    /// Appeals filed against a rejection.
    pub appeals: Vec<Appeal>,
    /// When the submission arrived, Unix seconds.
    pub at: u64,
}

/// A directory entry — one skill name and every version submitted for
/// it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Entry {
    /// The skill name.
    pub name: String,
    /// The account that owns the name — only they may submit further
    /// versions.
    pub author: String,
    /// The entry's category — the latest published version's.
    pub category: String,
    /// Every submitted version, by semver string.
    pub versions: BTreeMap<String, Version>,
    /// When the entry was created, Unix seconds.
    pub created_at: u64,
}

impl Entry {
    /// The newest published version — the entry's public face.
    pub fn latest(&self) -> Option<&Version> {
        self.versions
            .values()
            .filter(|version| version.state == VersionState::Published)
            .max_by(|a, b| semver_key(&a.version).cmp(&semver_key(&b.version)))
    }
}

/// One audit event — every lifecycle change, in order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DirectoryEvent {
    /// The event's position in the trail.
    pub seq: u64,
    /// When it happened, Unix seconds.
    pub at: u64,
    /// Who acted — an account id or `operator`.
    pub actor: String,
    /// What happened — `submitted`, `published`, `rejected`,
    /// `withdrawn`, `taken_down`, `reinstated`, `appealed`, `evidence`.
    pub kind: String,
    /// The entry the event touches.
    pub name: String,
    /// The version the event touches.
    pub version: Option<String>,
    /// A bounded note — the rejection reason or moderation rationale.
    pub detail: Option<String>,
}

/// What [`DirectoryBook::submit`] decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Submitted {
    /// A new submission entered review.
    Review(String),
    /// The same `(name, version, digest)` was already submitted by this
    /// author and is still under review — the caller should retry the
    /// outstanding review rather than open a second one.
    Retry(String),
    /// The same `(name, version, digest)` already resolved — the
    /// duplicate is stable and nothing reopens.
    Duplicate(String),
}

/// A submission's declared fields — the adapter's digest of the content
/// it stored, never the content itself.
#[derive(Clone, Debug)]
pub struct Draft {
    /// The submitting account.
    pub author: String,
    /// The skill name — `[a-z0-9][a-z0-9-]*`, at most 63 bytes.
    pub name: String,
    /// The semver — `MAJOR.MINOR.PATCH` with an optional pre-release.
    pub version: String,
    /// The license and rights declaration.
    pub license: String,
    /// The category.
    pub category: String,
    /// The tags — at most [`TAGS_MAX`].
    pub tags: Vec<String>,
    /// The content's `sha256:` digest.
    pub digest: String,
    /// The content's size in bytes.
    pub bytes: usize,
    /// The author's publication consent — required.
    pub consent: bool,
    /// Measured evidence declared with the submission.
    pub evidence: Option<Evidence>,
}

/// The skill directory book — every entry, submission, and the audit
/// trail, plus the policy it admits under.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DirectoryBook {
    /// Entries by skill name.
    #[serde(default)]
    pub entries: BTreeMap<String, Entry>,
    /// Submissions by id.
    #[serde(default)]
    pub submissions: BTreeMap<String, Submission>,
    /// The audit trail — appended, never rewritten.
    #[serde(default)]
    pub events: Vec<DirectoryEvent>,
    /// Author → day index → submissions opened, for the daily bound.
    #[serde(default)]
    pub daily: BTreeMap<String, BTreeMap<u64, u32>>,
    /// The admission policy installed with the store.
    #[serde(default)]
    pub policy: Option<Policy>,
}

impl DirectoryBook {
    /// Record a submission: bound it, deduplicate it, and open its
    /// version for review.
    ///
    /// A resubmitted `(name, version, digest)` returns the existing
    /// submission — `Retry` while it is still under review so the caller
    /// re-runs the outstanding stages, `Duplicate` once it resolved. A
    /// `(name, version)` at a different digest is a conflict: the author
    /// supersedes by bumping the version, not by rewriting it.
    pub fn submit(&mut self, draft: Draft, now: u64) -> Result<Submitted, Refusal> {
        self.check_draft(&draft)?;
        if let Some(entry) = self.entries.get(&draft.name) {
            if entry.author != draft.author {
                return Err(Refusal::Forbidden);
            }
            if let Some(version) = entry.versions.get(&draft.version) {
                if version.digest == draft.digest {
                    let submission = version.submission.clone();
                    return Ok(if version.state == VersionState::UnderReview {
                        Submitted::Retry(submission)
                    } else {
                        Submitted::Duplicate(submission)
                    });
                }
                return Err(Refusal::Conflict(format!(
                    "version {} of {} already exists at a different digest — submit a new version",
                    draft.version, draft.name
                )));
            }
        }
        let day = now / 86_400;
        let count = self
            .daily
            .get(&draft.author)
            .and_then(|days| days.get(&day))
            .copied()
            .unwrap_or(0);
        if count >= self.policy().submissions_per_day {
            return Err(Refusal::RateLimited);
        }
        let pending = self
            .entries
            .values()
            .flat_map(|entry| entry.versions.values())
            .filter(|version| {
                version.author == draft.author && version.state == VersionState::UnderReview
            })
            .count();
        if pending >= self.policy().pending_per_author {
            return Err(Refusal::TooManyPending);
        }
        let id = format!(
            "sub_{}",
            fresh_id().map_err(|trouble| Refusal::Store(trouble.to_string()))?
        );
        let submission = Submission {
            id: id.clone(),
            author: draft.author.clone(),
            name: draft.name.clone(),
            version: draft.version.clone(),
            license: draft.license.clone(),
            category: draft.category.clone(),
            tags: draft.tags.clone(),
            digest: draft.digest.clone(),
            bytes: draft.bytes,
            consent: draft.consent,
            evidence: draft.evidence.clone(),
            appeals: Vec::new(),
            at: now,
        };
        let version = Version {
            version: draft.version.clone(),
            digest: draft.digest,
            submission: id.clone(),
            author: draft.author.clone(),
            license: draft.license,
            category: draft.category.clone(),
            tags: draft.tags,
            consent: draft.consent,
            evidence: draft.evidence,
            state: VersionState::UnderReview,
            superseded_by: None,
            review: Vec::new(),
            submitted_at: now,
            resolved_at: None,
            resolution: None,
        };
        self.submissions.insert(id.clone(), submission);
        self.entries
            .entry(draft.name.clone())
            .or_insert_with(|| Entry {
                name: draft.name.clone(),
                author: draft.author.clone(),
                category: draft.category.clone(),
                versions: BTreeMap::new(),
                created_at: now,
            })
            .versions
            .insert(draft.version.clone(), version);
        self.daily
            .entry(draft.author.clone())
            .or_default()
            .insert(day, count + 1);
        self.event(
            now,
            &draft.author,
            "submitted",
            &draft.name,
            Some(&draft.version),
            None,
        );
        Ok(Submitted::Review(id))
    }

    /// The declared fields checked against the book's bounds.
    fn check_draft(&self, draft: &Draft) -> Result<(), Refusal> {
        if !draft.consent {
            return Err(Refusal::ConsentRequired);
        }
        if draft.bytes > self.policy().max_body_bytes {
            return Err(Refusal::Invalid(format!(
                "submission exceeds {} bytes",
                self.policy().max_body_bytes
            )));
        }
        valid_name(&draft.name).map_err(Refusal::Invalid)?;
        valid_version(&draft.version).map_err(Refusal::Invalid)?;
        valid_field(&draft.license, "license", 128).map_err(Refusal::Invalid)?;
        valid_name_field(&draft.category, "category").map_err(Refusal::Invalid)?;
        if draft.tags.len() > TAGS_MAX {
            return Err(Refusal::Invalid(format!("at most {TAGS_MAX} tags")));
        }
        for tag in &draft.tags {
            valid_name_field(tag, "tag").map_err(Refusal::Invalid)?;
        }
        if !draft
            .digest
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(Refusal::Invalid(
                "digest must be a `sha256:` content identity".into(),
            ));
        }
        if let Some(evidence) = &draft.evidence {
            for (field, label) in [
                (&evidence.suite, "evidence.suite"),
                (&evidence.report, "evidence.report"),
                (&evidence.digest, "evidence.digest"),
            ] {
                valid_field(field, label, 256).map_err(Refusal::Invalid)?;
            }
        }
        Ok(())
    }

    /// Append a review stage to an under-review version.
    pub fn record_stage(
        &mut self,
        name: &str,
        version: &str,
        stage: ReviewStage,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if target.state != VersionState::UnderReview {
            return Err(Refusal::State(format!(
                "{name} {version} is {}, not under review",
                target.state.name()
            )));
        }
        if target.review.len() >= REVIEW_MAX {
            return Err(Refusal::Invalid("review stage bound reached".into()));
        }
        target.review.push(stage);
        let _ = now;
        Ok(())
    }

    /// Admit an under-review version. The book requires a passed
    /// `static` stage and either a passed `reasoning` stage (the
    /// decision review's admission synthesis) or a passed `moderation`
    /// stage (an operator's appeal-granted admit) — a version cannot
    /// publish on a failed or missing review.
    ///
    /// Publishing marks every older published version `superseded_by`
    /// the new one; a pinned version keeps resolving.
    pub fn publish(
        &mut self,
        name: &str,
        version: &str,
        actor: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        {
            let target = self.version_mut(name, version)?;
            if target.state != VersionState::UnderReview {
                return Err(Refusal::State(format!(
                    "{name} {version} is {}, not under review",
                    target.state.name()
                )));
            }
            let passed = |wanted: &str| {
                target
                    .review
                    .iter()
                    .any(|stage| stage.stage == wanted && stage.outcome == StageOutcome::Pass)
            };
            if !passed("static") {
                return Err(Refusal::State(format!(
                    "{name} {version} has no passed static review"
                )));
            }
            if !passed("reasoning") && !passed("moderation") {
                return Err(Refusal::State(format!(
                    "{name} {version} has no passed admission review"
                )));
            }
            target.state = VersionState::Published;
            target.resolved_at = Some(now);
            let category = target.category.clone();
            if let Some(entry) = self.entries.get_mut(name) {
                entry.category = category;
            }
        }
        let key = semver_key(version);
        if let Some(entry) = self.entries.get_mut(name) {
            for other in entry.versions.values_mut() {
                if other.version != version
                    && other.state == VersionState::Published
                    && semver_key(&other.version) < key
                {
                    other.superseded_by = Some(version.to_string());
                }
            }
        }
        self.event(now, actor, "published", name, Some(version), None);
        Ok(())
    }

    /// Reject an under-review version with the reason a caller may read.
    pub fn reject(
        &mut self,
        name: &str,
        version: &str,
        reason: &str,
        actor: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if target.state != VersionState::UnderReview {
            return Err(Refusal::State(format!(
                "{name} {version} is {}, not under review",
                target.state.name()
            )));
        }
        target.state = VersionState::Rejected;
        target.resolved_at = Some(now);
        target.resolution = Some(bound(reason, 256));
        self.event(now, actor, "rejected", name, Some(version), Some(reason));
        Ok(())
    }

    /// Withdraw a version — the author's own act, from `under_review`
    /// or `published`. A withdrawn version stops resolving in the
    /// published directory.
    pub fn withdraw(
        &mut self,
        name: &str,
        version: &str,
        actor: &str,
        reason: Option<&str>,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if target.author != actor {
            return Err(Refusal::Forbidden);
        }
        if !matches!(
            target.state,
            VersionState::UnderReview | VersionState::Published
        ) {
            return Err(Refusal::State(format!(
                "{name} {version} is {} and cannot be withdrawn",
                target.state.name()
            )));
        }
        target.state = VersionState::Withdrawn;
        target.resolved_at = Some(now);
        target.resolution = reason.map(|text| bound(text, 256));
        self.event(now, actor, "withdrawn", name, Some(version), reason);
        Ok(())
    }

    /// Take a published version down — the operator's moderation act.
    pub fn takedown(
        &mut self,
        name: &str,
        version: &str,
        actor: &str,
        reason: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if target.state != VersionState::Published {
            return Err(Refusal::State(format!(
                "{name} {version} is {}, not published",
                target.state.name()
            )));
        }
        target.state = VersionState::TakenDown;
        target.resolved_at = Some(now);
        target.resolution = Some(bound(reason, 256));
        self.event(now, actor, "taken_down", name, Some(version), Some(reason));
        Ok(())
    }

    /// Reinstate a taken-down or withdrawn version — the operator's act.
    pub fn reinstate(
        &mut self,
        name: &str,
        version: &str,
        actor: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if !matches!(
            target.state,
            VersionState::TakenDown | VersionState::Withdrawn
        ) {
            return Err(Refusal::State(format!(
                "{name} {version} is {} and cannot be reinstated",
                target.state.name()
            )));
        }
        target.state = VersionState::Published;
        target.resolved_at = None;
        target.resolution = None;
        self.event(now, actor, "reinstated", name, Some(version), None);
        Ok(())
    }

    /// An operator's appeal-granted admission: records a `moderation`
    /// stage with its reason and publishes the version. A rejected
    /// version reopens through the recorded stage — the audit trail
    /// keeps both the rejection and the reversal.
    pub fn moderate_admit(
        &mut self,
        name: &str,
        version: &str,
        actor: &str,
        reason: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let stage = ReviewStage {
            stage: "moderation".into(),
            reviewer: actor.to_string(),
            policy: MODERATION_POLICY.into(),
            outcome: StageOutcome::Pass,
            detail: Some(bound(reason, 256)),
            score: None,
            rationale: Some(bound(reason, 256)),
            cost: None,
            at: now,
        };
        {
            let target = self.version_mut(name, version)?;
            match target.state {
                VersionState::Rejected => {
                    target.state = VersionState::UnderReview;
                    target.resolved_at = None;
                    target.resolution = None;
                }
                VersionState::UnderReview => {}
                _ => {
                    return Err(Refusal::State(format!(
                        "{name} {version} is {} — only an under-review or rejected \
                         version may be admitted",
                        target.state.name()
                    )));
                }
            }
            if target.review.len() >= REVIEW_MAX {
                return Err(Refusal::Invalid("review stage bound reached".into()));
            }
            target.review.push(stage);
        }
        self.publish(name, version, actor, now)
    }

    /// Record an author's appeal against a rejection.
    pub fn appeal(
        &mut self,
        submission: &str,
        actor: &str,
        reason: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let record = self
            .submissions
            .get_mut(submission)
            .ok_or(Refusal::NotFound(submission.to_string()))?;
        if record.author != actor {
            return Err(Refusal::Forbidden);
        }
        let rejected = self
            .entries
            .get(&record.name)
            .and_then(|entry| entry.versions.get(&record.version))
            .is_some_and(|version| version.state == VersionState::Rejected);
        if !rejected {
            return Err(Refusal::State(
                "only a rejected submission may be appealed".into(),
            ));
        }
        if record.appeals.len() >= APPEALS_MAX {
            return Err(Refusal::Invalid("appeal bound reached".into()));
        }
        if reason.len() > 1024 {
            return Err(Refusal::Invalid("appeal exceeds 1024 bytes".into()));
        }
        record.appeals.push(Appeal {
            reason: reason.to_string(),
            at: now,
        });
        let name = record.name.clone();
        let version = record.version.clone();
        self.event(now, actor, "appealed", &name, Some(&version), None);
        Ok(())
    }

    /// Attach measured evidence to a version — the author's at
    /// submission, the operator's at any time.
    pub fn attach_evidence(
        &mut self,
        name: &str,
        version: &str,
        evidence: Evidence,
        actor: &str,
        now: u64,
    ) -> Result<(), Refusal> {
        let target = self.version_mut(name, version)?;
        if target.author != actor && actor != "operator" {
            return Err(Refusal::Forbidden);
        }
        target.evidence = Some(evidence);
        self.event(now, actor, "evidence", name, Some(version), None);
        Ok(())
    }

    /// The caller's own submissions, newest first — every state,
    /// including the rejected ones the public directory never shows.
    pub fn submissions_for(&self, author: &str) -> Vec<&Submission> {
        let mut out: Vec<&Submission> = self
            .submissions
            .values()
            .filter(|submission| submission.author == author)
            .collect();
        out.sort_by_key(|submission| std::cmp::Reverse(submission.at));
        out
    }

    /// Published entries for the browse view — every returned entry has
    /// a published version.
    pub fn published(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .values()
            .filter(|entry| entry.latest().is_some())
    }

    /// A published version by name and semver — the only versions the
    /// public directory resolves.
    pub fn published_version<'b>(&'b self, name: &str, version: &str) -> Option<&'b Version> {
        self.entries
            .get(name)
            .and_then(|entry| entry.versions.get(version))
            .filter(|version| version.state == VersionState::Published)
    }

    /// The version a moderation act or review stage touches.
    fn version_mut(&mut self, name: &str, version: &str) -> Result<&mut Version, Refusal> {
        self.entries
            .get_mut(name)
            .and_then(|entry| entry.versions.get_mut(version))
            .ok_or_else(|| Refusal::NotFound(format!("{name} {version}")))
    }

    /// The installed policy — every store installs one; a missing
    /// policy on an old genesis falls back to the compile-time bounds.
    fn policy(&self) -> Policy {
        self.policy.clone().unwrap_or(Policy {
            max_body_bytes: 65_536,
            submissions_per_day: 20,
            pending_per_author: 10,
            admit_score: 0.6,
        })
    }

    /// Append an audit event, bounded.
    fn event(
        &mut self,
        now: u64,
        actor: &str,
        kind: &str,
        name: &str,
        version: Option<&str>,
        detail: Option<&str>,
    ) {
        if self.events.len() >= EVENTS_MAX {
            self.events.remove(0);
        }
        self.events.push(DirectoryEvent {
            seq: self.events.last().map(|event| event.seq + 1).unwrap_or(0),
            at: now,
            actor: actor.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            version: version.map(str::to_string),
            detail: detail.map(|text| bound(text, 256)),
        });
    }
}

/// The mechanical checks a submission passes before a model sees it —
/// frontmatter shape and credential screening. Returns the failures; an
/// empty list is a pass.
///
/// The checks read the document as data. Nothing in a `SKILL.md` runs:
/// there is no interpreter here and no code path that could evaluate
/// one. The credential patterns screen live-shaped literals —
/// `oak_<id>.<secret>` tokens, `sess_` sessions, PEM private keys — not
/// the names of the environment variables that carry them.
pub fn static_failures(markdown: &str, expected_name: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let frontmatter = markdown
        .strip_prefix("---\n")
        .and_then(|rest| rest.split("\n---").next());
    match frontmatter {
        Some(front) => {
            let name = front
                .lines()
                .find_map(|line| line.strip_prefix("name:").map(str::trim));
            let description = front
                .lines()
                .find_map(|line| line.strip_prefix("description:").map(str::trim));
            match name {
                Some(name) if name == expected_name => {}
                Some(name) => failures.push(format!(
                    "frontmatter name `{name}` does not match the submission name"
                )),
                None => failures.push("frontmatter lacks a `name` field".into()),
            }
            if description.is_none_or(|text| text.is_empty()) {
                failures.push("frontmatter lacks a `description` field".into());
            }
        }
        None => failures.push("document lacks YAML frontmatter".into()),
    }
    for (pattern, label) in [
        ("-----BEGIN", "a PEM block"),
        ("PRIVATE KEY-----", "a private key"),
    ] {
        if markdown.contains(pattern) {
            failures.push(format!("document contains {label}"));
        }
    }
    if has_token_shape(markdown, "oak_") {
        failures.push("document contains an `oak_` credential".into());
    }
    if has_token_shape(markdown, "sess_") {
        failures.push("document contains a `sess_` session token".into());
    }
    if markdown.trim().is_empty() {
        failures.push("document is empty".into());
    }
    failures
}

/// Whether the text carries a live-shaped `prefix` credential — the
/// prefix followed by at least six identifier bytes. The environment
/// variable names that hold such a token are not flagged.
fn has_token_shape(text: &str, prefix: &str) -> bool {
    text.match_indices(prefix).any(|(at, _)| {
        let rest = &text[at + prefix.len()..];
        rest.bytes()
            .take(6)
            .filter(|b| b.is_ascii_alphanumeric())
            .count()
            == 6
    })
}

/// A skill name — lowercase alphanumerics and dashes, starting with an
/// alphanumeric, at most 63 bytes.
fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 63 {
        return Err("name must be 1–63 bytes".into());
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err("name must be lowercase alphanumerics and dashes".into());
    }
    Ok(())
}

/// A bounded free-form field.
fn valid_field(value: &str, label: &str, max: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max {
        return Err(format!("{label} must be 1–{max} bytes"));
    }
    Ok(())
}

/// A category or tag — the name shape, shorter.
fn valid_name_field(value: &str, label: &str) -> Result<(), String> {
    valid_field(value, label, 48)?;
    if !value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!(
            "{label} must be lowercase alphanumerics and dashes"
        ));
    }
    Ok(())
}

/// A semver — `MAJOR.MINOR.PATCH` with an optional `-pre.release`.
fn valid_version(version: &str) -> Result<(), String> {
    if version.len() > 32 {
        return Err("version exceeds 32 bytes".into());
    }
    let (release, pre) = version.split_once('-').unwrap_or((version, ""));
    let mut parts = release.split('.');
    for _ in 0..3 {
        match parts.next() {
            Some(part) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) => {}
            _ => return Err("version must be MAJOR.MINOR.PATCH".into()),
        }
    }
    if parts.next().is_some() {
        return Err("version must be MAJOR.MINOR.PATCH".into());
    }
    if !pre.is_empty()
        && !pre
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        return Err("version pre-release must be alphanumerics, dashes, and dots".into());
    }
    Ok(())
}

/// A semver ordering key — `(major, minor, patch)`; pre-releases order
/// before their release.
fn semver_key(version: &str) -> (u64, u64, u64, u8, String) {
    let (release, pre) = version.split_once('-').unwrap_or((version, ""));
    let mut parts = release.split('.').map(|part| part.parse().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        if pre.is_empty() { 1 } else { 0 },
        pre.to_string(),
    )
}

/// A field bounded to `max` bytes on a char boundary.
fn bound(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Why the book refused an act.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Refusal {
    /// A field failed its bound.
    Invalid(String),
    /// The store could not be read or written.
    Store(String),
    /// The name, version, or submission does not exist.
    NotFound(String),
    /// The actor does not own the name or submission.
    Forbidden,
    /// The `(name, version)` exists at a different digest.
    Conflict(String),
    /// The author exhausted the day's submission bound.
    RateLimited,
    /// The author holds too many under-review submissions.
    TooManyPending,
    /// The submission lacks publication consent.
    ConsentRequired,
    /// The version is in the wrong state for the act.
    State(String),
}

impl Refusal {
    /// The stable code an API document carries.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid_submission",
            Self::Store(_) => "store_unavailable",
            Self::NotFound(_) => "unknown_skill",
            Self::Forbidden => "forbidden",
            Self::Conflict(_) => "version_conflict",
            Self::RateLimited => "rate_limited",
            Self::TooManyPending => "too_many_pending",
            Self::ConsentRequired => "consent_required",
            Self::State(_) => "invalid_state",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(detail) => write!(f, "{detail}"),
            Self::Store(detail) => write!(f, "skill store: {detail}"),
            Self::NotFound(what) => write!(f, "{what} is not in the directory"),
            Self::Forbidden => write!(f, "the credential does not own this entry"),
            Self::Conflict(detail) => write!(f, "{detail}"),
            Self::RateLimited => write!(f, "the day's submission bound is reached"),
            Self::TooManyPending => {
                write!(f, "too many submissions are still under review")
            }
            Self::ConsentRequired => write!(f, "publication consent is required"),
            Self::State(detail) => write!(f, "{detail}"),
        }
    }
}

/// The sealed store — the book plus its own revision identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Store {
    /// The schema tag.
    pub v: String,
    /// The revision's position.
    pub sequence: u64,
    /// The revision this one replaces.
    pub supersedes: Option<String>,
    /// The directory book.
    pub book: DirectoryBook,
    /// The access trail — which actors touched the store and when.
    #[serde(default)]
    pub access: Vec<Access>,
    /// This revision's `sha256:` identity.
    pub digest: String,
}

impl Store {
    /// Parse and validate a store document.
    pub fn parse(text: &str, name: &str) -> Result<Self, String> {
        let store: Self = serde_json::from_str(text).map_err(|error| format!("{name}: {error}"))?;
        store.validate(name)?;
        Ok(store)
    }

    /// The store's structural checks against its declared schema.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.v != SCHEMA {
            return Err(format!("{name}: schema {} is not {SCHEMA}", self.v));
        }
        if self.digest.len() != 71 || !self.digest.starts_with("sha256:") {
            return Err(format!("{name}: revision identity malformed"));
        }
        Ok(())
    }

    /// The revision's content identity over everything but the digest.
    pub fn compute_digest(&self) -> String {
        let mut body = self.clone();
        body.digest = String::new();
        let text = serde_json::to_string(&body).unwrap_or_default();
        format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
    }

    /// Seal the revision with its digest.
    fn seal(&mut self) {
        self.digest = self.compute_digest();
    }
}

/// The directory handle — opens and mutates the store beside the
/// registry. Like [`crate::Sessions`], it holds no cache: every query
/// re-reads `skills.json` so a withdrawal committed by any writer is
/// visible to the very next read.
#[derive(Clone, Debug)]
pub struct Directory {
    dir: PathBuf,
}

impl Directory {
    /// Create the store's genesis revision in a directory that does not
    /// already hold one. `policy` is the admission policy every later
    /// mutation reads back from the store — the bounds the directory
    /// admits under are pinned, not re-declared at each open.
    pub fn install(dir: &Path, policy: Policy) -> Result<Self, Trouble> {
        std::fs::create_dir_all(dir)?;
        let _lock = SkillsLock::acquire(dir)?;
        if dir.join(SKILLS).exists() {
            return Err(Trouble::Invalid(format!(
                "{} already holds a skill directory; open it rather than reinstalling",
                dir.display()
            )));
        }
        let book = DirectoryBook {
            policy: Some(policy),
            ..DirectoryBook::default()
        };
        let mut store = Store {
            v: SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            book,
            access: Vec::new(),
            digest: String::new(),
        };
        store.seal();
        store
            .validate(&dir.join(SKILLS).display().to_string())
            .map_err(Trouble::Invalid)?;
        save_skills(dir, &store)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Open the store in a directory, validating it end to end.
    pub fn open(dir: &Path) -> Result<Self, Trouble> {
        load_skills(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Read the current store — the fresh read every query makes.
    pub fn store(&self) -> Result<Store, Trouble> {
        load_skills(&self.dir)
    }

    /// One mutation: take the lock, re-read inside it, run `f` against
    /// the book and access trail, seal the new revision, and write it
    /// in one rename.
    pub fn mutate<T>(
        &self,
        f: impl FnOnce(&mut DirectoryBook, &mut Vec<Access>, u64) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let _lock = SkillsLock::acquire(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let mut store = load_skills(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let now = unix_now();
        let supersedes = store.digest.clone();
        let out = f(&mut store.book, &mut store.access, now)?;
        store.sequence += 1;
        store.supersedes = Some(supersedes);
        store.seal();
        store
            .validate(&self.dir.join(SKILLS).display().to_string())
            .map_err(Refusal::Store)?;
        save_skills(&self.dir, &store).map_err(|t| Refusal::Store(t.to_string()))?;
        Ok(out)
    }
}

/// The exclusive lock one mutation holds.
struct SkillsLock {
    path: PathBuf,
}

impl SkillsLock {
    fn acquire(dir: &Path) -> Result<Self, Trouble> {
        let path = dir.join(LOCK);
        for _ in 0..LOCK_RETRIES {
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                Ok(mut file) => {
                    writeln!(file, "pid {}", std::process::id()).ok();
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(Trouble::Io(error)),
            }
        }
        Err(Trouble::Locked(path.display().to_string()))
    }
}

impl Drop for SkillsLock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The current time as Unix seconds.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or(0)
}

/// Random hex for a submission id.
fn fresh_id() -> Result<String, Trouble> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes)
        .map_err(|error| Trouble::Invalid(format!("no randomness available: {error}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Read a store file, bounded.
fn read_skills(path: &Path) -> Result<String, Trouble> {
    let file = std::fs::File::open(path)?;
    let mut text = String::new();
    file.take(STORE_BYTES + 1).read_to_string(&mut text)?;
    if text.len() as u64 > STORE_BYTES {
        return Err(Trouble::Invalid(
            "skill directory store exceeds 16 MiB".into(),
        ));
    }
    Ok(text)
}

fn write_skills_synced(path: &Path, text: &str) -> Result<(), Trouble> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Load the store from a directory, validating it end to end.
fn load_skills(dir: &Path) -> Result<Store, Trouble> {
    let path = dir.join(SKILLS);
    let text = read_skills(&path)?;
    Store::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)
}

/// Write a sealed store: archive it by digest, then replace
/// `skills.json` in one rename.
fn save_skills(dir: &Path, store: &Store) -> Result<(), Trouble> {
    let history = dir.join(HISTORY_DIR);
    std::fs::create_dir_all(&history)?;
    let text =
        serde_json::to_string_pretty(store).map_err(|error| Trouble::Invalid(error.to_string()))?;
    let archived = history.join(format!("{}.json", store.digest));
    if text.len() as u64 + 1 > STORE_BYTES {
        return Err(Trouble::Invalid(
            "skill directory store exceeds 16 MiB".into(),
        ));
    }
    if !archived.exists() {
        write_skills_synced(&archived, &format!("{text}\n"))?;
    } else if read_skills(&archived)? != format!("{text}\n") {
        return Err(Trouble::Invalid(
            "archived revision content mismatch".into(),
        ));
    }
    std::fs::File::open(&history)?.sync_all()?;
    let staged = dir.join(format!(".{SKILLS}.{}.tmp", fresh_id()?));
    write_skills_synced(&staged, &format!("{text}\n"))?;
    std::fs::rename(&staged, dir.join(SKILLS))?;
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str, version: &str, digest: &str) -> Draft {
        Draft {
            author: "acct_a".into(),
            name: name.into(),
            version: version.into(),
            license: "MIT".into(),
            category: "filtering".into(),
            tags: vec!["triage".into()],
            digest: digest.into(),
            bytes: 1024,
            consent: true,
            evidence: None,
        }
    }

    fn stage(stage: &str, outcome: StageOutcome) -> ReviewStage {
        ReviewStage {
            stage: stage.into(),
            reviewer: "kev-0.6b".into(),
            policy: REVIEW_POLICY.into(),
            outcome,
            detail: None,
            score: Some(0.8),
            rationale: None,
            cost: None,
            at: 0,
        }
    }

    #[test]
    fn submit_opens_review_and_deduplicates() {
        let mut book = DirectoryBook::default();
        let digest = format!("sha256:{}", "a".repeat(64));
        let first = book.submit(draft("triage", "1.0.0", &digest), 100).unwrap();
        assert!(matches!(first, Submitted::Review(_)));
        let retry = book.submit(draft("triage", "1.0.0", &digest), 200).unwrap();
        assert!(matches!(retry, Submitted::Retry(_)));
        let conflict = book.submit(
            draft("triage", "1.0.0", &format!("sha256:{}", "b".repeat(64))),
            300,
        );
        assert!(matches!(conflict, Err(Refusal::Conflict(_))));
    }

    #[test]
    fn another_author_cannot_take_a_name() {
        let mut book = DirectoryBook::default();
        let digest = format!("sha256:{}", "a".repeat(64));
        book.submit(draft("triage", "1.0.0", &digest), 100).unwrap();
        let mut other = draft("triage", "2.0.0", &digest);
        other.author = "acct_b".into();
        assert_eq!(book.submit(other, 200), Err(Refusal::Forbidden));
    }

    #[test]
    fn publish_requires_passed_stages() {
        let mut book = DirectoryBook::default();
        let digest = format!("sha256:{}", "a".repeat(64));
        book.submit(draft("triage", "1.0.0", &digest), 100).unwrap();
        assert!(book.publish("triage", "1.0.0", "system", 200).is_err());
        book.record_stage("triage", "1.0.0", stage("static", StageOutcome::Pass), 200)
            .unwrap();
        assert!(book.publish("triage", "1.0.0", "system", 200).is_err());
        book.record_stage(
            "triage",
            "1.0.0",
            stage("reasoning", StageOutcome::Pass),
            200,
        )
        .unwrap();
        book.publish("triage", "1.0.0", "system", 200).unwrap();
        assert_eq!(
            book.published_version("triage", "1.0.0").unwrap().state,
            VersionState::Published
        );
    }

    #[test]
    fn newer_version_supersedes_older() {
        let mut book = DirectoryBook::default();
        for (version, byte) in [("1.0.0", "a"), ("1.1.0", "b")] {
            let digest = format!("sha256:{}", byte.repeat(64));
            book.submit(draft("triage", version, &digest), 100).unwrap();
            book.record_stage("triage", version, stage("static", StageOutcome::Pass), 100)
                .unwrap();
            book.record_stage(
                "triage",
                version,
                stage("reasoning", StageOutcome::Pass),
                100,
            )
            .unwrap();
            book.publish("triage", version, "system", 100).unwrap();
        }
        let old = book.published_version("triage", "1.0.0").unwrap();
        assert_eq!(old.superseded_by.as_deref(), Some("1.1.0"));
        assert_eq!(book.entries["triage"].latest().unwrap().version, "1.1.0");
    }

    #[test]
    fn withdrawal_stops_resolution() {
        let mut book = DirectoryBook::default();
        let digest = format!("sha256:{}", "a".repeat(64));
        book.submit(draft("triage", "1.0.0", &digest), 100).unwrap();
        book.record_stage("triage", "1.0.0", stage("static", StageOutcome::Pass), 100)
            .unwrap();
        book.record_stage(
            "triage",
            "1.0.0",
            stage("reasoning", StageOutcome::Pass),
            100,
        )
        .unwrap();
        book.publish("triage", "1.0.0", "system", 100).unwrap();
        book.withdraw("triage", "1.0.0", "acct_a", None, 200)
            .unwrap();
        assert!(book.published_version("triage", "1.0.0").is_none());
        assert_eq!(
            book.submit(draft("triage", "1.0.0", &digest), 300).unwrap(),
            Submitted::Duplicate(book.submissions.keys().next().unwrap().clone())
        );
    }

    #[test]
    fn appeals_require_rejection() {
        let mut book = DirectoryBook::default();
        let digest = format!("sha256:{}", "a".repeat(64));
        let Submitted::Review(id) = book.submit(draft("triage", "1.0.0", &digest), 100).unwrap()
        else {
            panic!()
        };
        assert!(book.appeal(&id, "acct_a", "why", 200).is_err());
        book.reject("triage", "1.0.0", "unsafe", "system", 200)
            .unwrap();
        book.appeal(&id, "acct_a", "the check misread it", 300)
            .unwrap();
        assert_eq!(book.submissions[&id].appeals.len(), 1);
        assert_eq!(
            book.appeal(&id, "acct_b", "not mine", 400),
            Err(Refusal::Forbidden)
        );
    }

    #[test]
    fn static_checks_screen_credentials() {
        let doc = "---\nname: triage\ndescription: Route tickets.\n---\n\n# Triage\n";
        assert!(static_failures(doc, "triage").is_empty());
        let leaked = format!("{doc}\nkey: oak_abcdef123456.secret\n");
        let failures = static_failures(&leaked, "triage");
        assert!(failures.iter().any(|f| f.contains("oak_")));
        assert!(
            static_failures("# no frontmatter", "triage")
                .iter()
                .any(|f| f.contains("frontmatter"))
        );
    }
}
