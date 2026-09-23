//! The tunable composition in a Terminal-Bench episode.
//!
//! Every component ran alone or in a mini-task episode; a Terminal-Bench
//! episode ran requirements, evidence, and one executor. This module runs
//! the whole composition inside one episode deadline:
//!
//! ```text
//! control.route    task.profile's features pick the first executor and its budget
//! control.handoff  planner-worker: a planner writes a plan in a scratch copy first
//! exec.session     the first executor, watched by control.monitor
//! verify.checks    requirement-derived and generic scenarios on the live workspace
//! verify.support   Jev's paired support and contradiction judgments
//! control.handoff  escalate: on a stall, a failed check, a contradicted
//!                  requirement, or no answer, the second executor continues
//!                  from a handoff brief
//! verify.repair    one fresh session from the diagnostic packets, then a recheck
//! verify.second    when the checks can't confirm the result, a second
//!                  executor on the task's original state; the candidate
//!                  whose checks confirm more requirements stays
//! control.persist  on a long task with time left, fresh rounds from a
//!                  continue brief until a round changes nothing, the
//!                  checks confirm the result, or the round cap
//! ```
//!
//! The policy manifest turns each part on: `control.route`,
//! `control.horizon`, `control.handoff`, `control.persist`, and `verify`. `control.horizon`
//! sizes each dispatch from the episode deadline, so an eight-hour task
//! gives its executors hours instead of the ten-minute default. The
//! episode writes the whole account to `artifacts/composition.json`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::Ended;
use crate::agent::{Generate, Shell};
use crate::checks::{self, Budget, Subject, TaskText, generic};
use crate::component::jev::JevMode;
use crate::deadline::Deadline;
use crate::delegate::{
    self, Briefing, Cli, Delegated, Delegation, Executor, Explorer, Mode, Plan, Reason, Report,
    Status,
};
use crate::handoff::{self, Pattern, Tier};
use crate::monitor::{Acting, Params as MonitorParams, Setup as MonitorSetup};
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};
use crate::scripted::Scripted;
use crate::session::Intent;
use crate::state::State;

/// The schema of `artifacts/composition.json`. v2: each
/// `control.persist` round records its class, executor, own-tests run, and
/// delta, and the rounds record their totals and spending cap.
pub const SCHEMA: &str = "openagents.coder-one.composition.v2";

/// Where the episode keeps the composition's record.
pub const FILE: &str = "artifacts/composition.json";

/// The routing component's ID.
pub const ROUTE: &str = "control.route";

/// Where the check after an escalation writes its report.
pub const ESCALATED_CHECKS: &str = "verification/checks-escalated.json";

/// Where the check of the second executor's candidate writes its report.
pub const SECOND_CHECKS: &str = "verification/checks-second.json";

/// Where `verify.support` writes its judgment of the second candidate.
pub const SECOND_SUPPORT: &str = "verification/support-second.json";

/// The component that runs the second executor.
pub const SECOND: &str = "verify.second";

pub mod persist;
pub use persist::PersistPolicy;

fn half() -> f64 {
    0.5
}
fn seven_tenths() -> f64 {
    0.7
}
fn hard_features() -> Vec<String> {
    vec!["builds_code".to_string(), "concurrency".to_string()]
}
fn yes() -> bool {
    true
}

/// `control.route`: which executor starts, from task.profile's features.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePolicy {
    /// The rule's name; only `profile-v1` in this build.
    pub rule: String,
    /// The executor for cheap, likely-easy tasks.
    pub cheap: Tier,
    /// The executor for hard tasks, and for any task the profile can't
    /// read.
    pub strong: Tier,
    /// A difficulty at or above this starts strong.
    #[serde(default = "half")]
    pub hard_at: f64,
    /// Features that start strong at or above `feature_at`.
    #[serde(default = "hard_features")]
    pub hard_features: Vec<String>,
    #[serde(default = "seven_tenths")]
    pub feature_at: f64,
    /// An episode deadline at least this long is a long task. Under
    /// `profile-v1` a long task starts strong; under `profile-v2` it only
    /// lowers the difficulty bar to `long_hard_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_after_sec: Option<u64>,
    /// `profile-v2`: the difficulty at or above which a long task starts
    /// strong; `hard_at` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_hard_at: Option<f64>,
    /// Per-family executor picks from an outcome table: when the task
    /// matches a family whose table has enough trials, the profile that
    /// passed most starts instead of the rule's pick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub families: Option<Families>,
}

/// `control.route.families`: executor profiles picked per task family from
/// an outcome table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Families {
    /// Where the outcome rows come from, in words: the table is data, and
    /// the record names its source.
    pub source: String,
    /// The executors a row can name, by profile name.
    pub profiles: std::collections::BTreeMap<String, Tier>,
    /// The Terminal-Bench leaderboard row each profile's outcomes were
    /// read from, by profile name, such as `Claude Code / Opus 5 (xhigh)`:
    /// `gym coder families` recomputes the table from it.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reference_rows: std::collections::BTreeMap<String, String>,
    /// The trials a profile's row needs before it can decide.
    pub min_trials: u32,
    /// How much higher its pass rate must be than the rule's pick's.
    pub min_gap: f64,
    /// The families, first match wins.
    pub table: Vec<Family>,
}

/// One task family: the words that recognize it in the instruction, and
/// each profile's outcomes on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub name: String,
    /// The instruction must contain at least one of these phrases, as
    /// words, ignoring case.
    pub any: Vec<String>,
    /// And one phrase of each of these groups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all: Vec<Vec<String>>,
    pub outcomes: Vec<FamilyOutcome>,
}

/// One profile's graded trials on a family.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyOutcome {
    pub profile: String,
    pub passes: u32,
    pub trials: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_cost_usd: Option<f64>,
}

impl FamilyOutcome {
    fn rate(&self) -> f64 {
        if self.trials == 0 {
            0.0
        } else {
            f64::from(self.passes) / f64::from(self.trials)
        }
    }
}

/// Whether `phrase` occurs in `lower` as whole words.
fn has_phrase(lower: &str, phrase: &str) -> bool {
    let phrase = phrase.to_lowercase();
    if phrase.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = lower[from..].find(&phrase) {
        let at = from + i;
        let end = at + phrase.len();
        let before = lower[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

impl Family {
    /// Whether `instruction` belongs to this family.
    #[must_use]
    pub fn matches(&self, instruction: &str) -> bool {
        let lower = instruction.to_lowercase();
        self.any.iter().any(|p| has_phrase(&lower, p))
            && self
                .all
                .iter()
                .all(|group| group.iter().any(|p| has_phrase(&lower, p)))
    }
}

impl Families {
    /// The first family `instruction` belongs to.
    #[must_use]
    pub fn family(&self, instruction: &str) -> Option<&Family> {
        self.table.iter().find(|f| f.matches(instruction))
    }

    /// The profile name whose tier is `tier`'s agent and model.
    fn profile_of(&self, tier: &Tier) -> Option<&str> {
        self.profiles
            .iter()
            .find(|(_, t)| t.agent == tier.agent && t.model == tier.model)
            .map(|(name, _)| name.as_str())
    }

    /// The family pick for `instruction` against the rule's `tier`: the
    /// profile whose row has enough trials and the highest pass rate (the
    /// lower cost on a tie), when it beats the rule's profile by
    /// `min_gap`. Returns the record either way.
    #[must_use]
    pub fn pick(&self, instruction: &str, tier: &Tier) -> (Option<(String, Tier)>, Value) {
        let Some(family) = self.family(instruction) else {
            return (
                None,
                json!({ "family": null, "why": "no family's words match the instruction" }),
            );
        };
        let rows: Vec<&FamilyOutcome> = family
            .outcomes
            .iter()
            .filter(|o| o.trials >= self.min_trials && self.profiles.contains_key(&o.profile))
            .collect();
        let best = rows.iter().copied().max_by(|a, b| {
            a.rate().total_cmp(&b.rate()).then_with(|| {
                b.mean_cost_usd
                    .unwrap_or(f64::MAX)
                    .total_cmp(&a.mean_cost_usd.unwrap_or(f64::MAX))
            })
        });
        let current = self.profile_of(tier);
        let current_rate = current
            .and_then(|name| rows.iter().find(|o| o.profile == name))
            .map(|o| o.rate());
        let mut record = json!({
            "family": family.name,
            "source": self.source,
            "rule_profile": current,
            "rule_rate": current_rate,
            "rows": rows.iter().map(|o| json!({ "profile": o.profile, "passes": o.passes, "trials": o.trials, "mean_cost_usd": o.mean_cost_usd })).collect::<Vec<_>>(),
        });
        let Some(best) = best else {
            record["why"] = json!(format!(
                "no profile has {} trials on this family",
                self.min_trials
            ));
            return (None, record);
        };
        if Some(best.profile.as_str()) == current {
            record["why"] = json!("the rule's pick already has the family's best pass rate");
            return (None, record);
        }
        let gap = best.rate() - current_rate.unwrap_or(0.0);
        if current_rate.is_some() && gap + 1e-9 < self.min_gap {
            record["why"] = json!(format!(
                "{} passed {:.2} against the rule's {:.2}, less than the gap of {:.2}",
                best.profile,
                best.rate(),
                current_rate.unwrap_or(0.0),
                self.min_gap
            ));
            return (None, record);
        }
        record["picked"] = json!(best.profile);
        record["why"] = json!(format!(
            "{} passed {} of {} on {}{}",
            best.profile,
            best.passes,
            best.trials,
            family.name,
            current_rate.map_or_else(
                || " and the rule's pick has no row".to_string(),
                |rate| format!(" against the rule's {rate:.2}")
            )
        ));
        let tier = self.profiles[&best.profile].clone();
        (Some((best.profile.clone(), tier)), record)
    }
}

impl RoutePolicy {
    /// Refuses a route this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !matches!(self.rule.as_str(), "profile-v1" | "profile-v2") {
            problems.push(format!(
                "control.route.rule must be profile-v1 or profile-v2, not {}",
                self.rule
            ));
        }
        if self.long_hard_at.is_some() && self.rule != "profile-v2" {
            problems.push("control.route.long_hard_at needs the profile-v2 rule".to_string());
        }
        if let Some(at) = self.long_hard_at
            && !(0.0..=1.0).contains(&at)
        {
            problems.push("control.route.long_hard_at must be 0 to 1".to_string());
        }
        if let Some(families) = &self.families {
            for (name, tier) in &families.profiles {
                problems.extend(tier.validate(&format!("control.route.families.profiles.{name}")));
            }
            if !(0.0..=1.0).contains(&families.min_gap) {
                problems.push("control.route.families.min_gap must be 0 to 1".to_string());
            }
            for family in &families.table {
                if family.any.is_empty() {
                    problems.push(format!(
                        "control.route.families: {} names no words",
                        family.name
                    ));
                }
                for outcome in &family.outcomes {
                    if !families.profiles.contains_key(&outcome.profile) {
                        problems.push(format!(
                            "control.route.families: {} names the unknown profile {}",
                            family.name, outcome.profile
                        ));
                    }
                    if outcome.passes > outcome.trials {
                        problems.push(format!(
                            "control.route.families: {} has more passes than trials",
                            family.name
                        ));
                    }
                }
            }
        }
        problems.extend(self.cheap.validate("control.route.cheap"));
        problems.extend(self.strong.validate("control.route.strong"));
        if !(0.0..=1.0).contains(&self.hard_at) || !(0.0..=1.0).contains(&self.feature_at) {
            problems.push("control.route.hard_at and feature_at must be 0 to 1".to_string());
        }
        for feature in &self.hard_features {
            if !crate::profile::NOULS.iter().any(|(id, _)| id == feature) {
                problems.push(format!(
                    "control.route.hard_features names {feature}, which task.profile doesn't ask"
                ));
            }
        }
        problems
    }
}

fn first_share() -> f64 {
    0.55
}
fn later_share() -> f64 {
    0.75
}
fn min_dispatch() -> u64 {
    300
}
fn check_share() -> f64 {
    0.05
}

/// `control.horizon`: how dispatches, checks, and effort scale with the
/// episode deadline, the task's own timeout less the harness's margins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Horizon {
    /// The share of the time left that the first dispatch asks for when
    /// an escalation can follow it.
    #[serde(default = "first_share")]
    pub first_share: f64,
    /// The share a dispatch asks for when only a repair can follow it.
    #[serde(default = "later_share")]
    pub later_share: f64,
    /// The least a dispatch asks for, in seconds.
    #[serde(default = "min_dispatch")]
    pub min_dispatch_sec: u64,
    /// The share of the whole deadline the checks may spend.
    #[serde(default = "check_share")]
    pub check_share: f64,
    /// A deadline at least this long is a long task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_after_sec: Option<u64>,
    /// The effort every executor runs at on a long task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_effort: Option<String>,
    /// On a long task, the most seconds one Claude Code shell command may
    /// run (`BASH_MAX_TIMEOUT_MS`); the CLI's own ten-minute cap otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_command_sec: Option<u64>,
}

impl Default for Horizon {
    fn default() -> Self {
        Horizon {
            first_share: first_share(),
            later_share: later_share(),
            min_dispatch_sec: min_dispatch(),
            check_share: check_share(),
            long_after_sec: None,
            long_effort: None,
            long_command_sec: None,
        }
    }
}

impl Horizon {
    /// Refuses a horizon this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (field, share) in [
            ("first_share", self.first_share),
            ("later_share", self.later_share),
            ("check_share", self.check_share),
        ] {
            if !(share > 0.0 && share <= 1.0) {
                problems.push(format!(
                    "control.horizon.{field} must be above 0 and at most 1"
                ));
            }
        }
        if let Some(effort) = &self.long_effort
            && (effort.is_empty() || !effort.chars().all(|c| c.is_ascii_lowercase()))
        {
            problems.push("control.horizon.long_effort must be one lowercase word".to_string());
        }
        problems
    }

    /// Whether an episode of `total` seconds is a long task.
    #[must_use]
    pub fn long(&self, total: Option<u64>) -> bool {
        matches!((total, self.long_after_sec), (Some(total), Some(after)) if total >= after)
    }

    /// The seconds one dispatch asks for: `fallback` without a deadline;
    /// otherwise `share` of what is left, at least the minimum, or all of
    /// it when nothing can follow.
    #[must_use]
    pub fn dispatch_sec(&self, remaining: Option<Duration>, fallback: u64, share: f64) -> u64 {
        let Some(remaining) = remaining else {
            return fallback;
        };
        let left = remaining.as_secs();
        let asked = (left as f64 * share).round() as u64;
        asked.max(self.min_dispatch_sec).min(left).max(1)
    }

    /// The checks' budget for an episode of `total` seconds, and the bound
    /// on each command a check runs.
    #[must_use]
    pub fn checks(&self, total: Option<u64>) -> (Budget, u64) {
        match total {
            None => (
                Budget {
                    max_scenarios: 12,
                    seconds: 300,
                },
                60,
            ),
            Some(total) => {
                let seconds = ((total as f64 * self.check_share) as u64).clamp(180, 1_800);
                (
                    Budget {
                        max_scenarios: 12,
                        seconds,
                    },
                    (total / 60).clamp(60, 900),
                )
            }
        }
    }
}

/// `verify` in a Terminal-Bench episode: which checks run after the
/// executor, and whether one repair follows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyPolicy {
    /// `verify.checks` on the live workspace.
    #[serde(default = "yes")]
    pub checks: bool,
    /// `verify.support`, which needs Jev.
    #[serde(default)]
    pub support: bool,
    /// `verify.repair`: one fresh session from the packets; needs checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<RepairPolicy>,
    /// `verify.support`'s budget and order; three requirements, scenario
    /// first, when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_budget: Option<SupportBudget>,
    /// Count a failure the executor reports itself as a failed check
    /// (`generic.self-report`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub self_report: bool,
    /// Let an output the task asks for only when needed be empty, and
    /// don't count a missing output against a requirement the extraction
    /// was unsure binds.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional_outputs: bool,
    /// Admit only the outputs a requirement tells the executor to write,
    /// and run the behavior scenarios the instruction's words justify
    /// (`checks::behavior`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub behavior: bool,
    /// `verify.second`: a second executor when the checks can't confirm
    /// the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second: Option<SecondPolicy>,
    /// `verify.snapshot`: save the workspace and the check's subject right
    /// after the first executor, so checks, repair briefs, and the
    /// verifier can be replayed without a model call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<crate::snapshot::SnapshotPolicy>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

impl VerifyPolicy {
    /// The policy with every check on and nothing else: what runs when a
    /// composed manifest has no `verify`.
    #[must_use]
    pub fn checks_only() -> Self {
        VerifyPolicy {
            checks: true,
            support: false,
            repair: None,
            support_budget: None,
            self_report: false,
            optional_outputs: false,
            behavior: false,
            second: None,
            snapshot: None,
        }
    }

    /// Refuses a verify policy this build can't run.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if let Some(budget) = &self.support_budget {
            if budget.max_requirements == 0 {
                problems
                    .push("verify.support_budget.max_requirements must be at least 1".to_string());
            }
            if !self.support {
                problems.push("verify.support_budget needs verify.support".to_string());
            }
        }
        if (self.self_report || self.optional_outputs || self.behavior) && !self.checks {
            problems.push(
                "verify.self_report, optional_outputs, and behavior need verify.checks".to_string(),
            );
        }
        if let Some(second) = &self.second {
            if !self.checks {
                problems
                    .push("verify.second needs verify.checks to choose a candidate".to_string());
            }
            if second.to.is_empty() {
                problems.push("verify.second.to names no executor".to_string());
            }
            for (i, tier) in second.to.iter().enumerate() {
                problems.extend(tier.validate(&format!("verify.second.to[{i}]")));
            }
            if second.on.is_empty() {
                problems.push("verify.second.on names no trigger".to_string());
            }
            for on in &second.on {
                if !matches!(on.as_str(), "unconfirmed" | "failed") {
                    problems.push(format!(
                        "verify.second.on must be unconfirmed or failed, not {on}"
                    ));
                }
            }
            if !(second.share > 0.0 && second.share <= 1.0) {
                problems.push("verify.second.share must be above 0 and at most 1".to_string());
            }
        }
        problems
    }

    /// `verify.support`'s parameters for an episode that is `long` or not.
    #[must_use]
    pub fn support_params(&self, long: bool) -> crate::support::Params {
        let mut params = crate::support::Params::default();
        if let Some(budget) = &self.support_budget {
            params.max_requirements = if long {
                budget
                    .long_max_requirements
                    .unwrap_or(budget.max_requirements)
            } else {
                budget.max_requirements
            };
            params.order = budget.order;
        }
        params
    }

    /// The generic scenarios' options.
    #[must_use]
    pub fn check_options(&self) -> generic::Options {
        generic::Options {
            self_report: self.self_report,
            optional_outputs: self.optional_outputs,
            behavior: self.behavior,
        }
    }
}

/// `verify.support_budget`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportBudget {
    /// The most requirements one support run judges.
    pub max_requirements: usize,
    /// The same on a long task; `max_requirements` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_max_requirements: Option<usize>,
    /// Which requirements come first.
    pub order: crate::support::Order,
}

fn second_share() -> f64 {
    0.6
}
fn second_min_remaining() -> u64 {
    1_800
}
fn second_max_copy_mb() -> u64 {
    256
}

/// `verify.second`: verify by a second executor. When the checks can't
/// confirm the first line's result, the host puts that candidate aside in
/// a scratch copy, restores the task's original state, runs a second
/// executor from the same briefing, checks what it leaves, and keeps the
/// candidate whose checks confirm more requirements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondPolicy {
    /// Executors to try, in order: the first whose agent and model differ
    /// from the one that produced the candidate runs.
    pub to: Vec<Tier>,
    /// `unconfirmed`: no scenario other than the self-report passed, or
    /// `verify.support` left a requirement unresolved. `failed`: a check
    /// still fails or a requirement is still contradicted.
    pub on: Vec<String>,
    /// The share of the time left that the second executor asks for.
    #[serde(default = "second_share")]
    pub share: f64,
    /// The least seconds the episode must have left.
    #[serde(default = "second_min_remaining")]
    pub min_remaining_sec: u64,
    /// The largest workspace the host copies aside, in MiB.
    #[serde(default = "second_max_copy_mb")]
    pub max_copy_mb: u64,
}

/// One repair's brief and trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairPolicy {
    pub brief: crate::repair::BriefKind,
    pub trigger: crate::repair::Trigger,
}

/// The executor the manifest itself names, as a tier.
#[must_use]
pub fn manifest_tier(manifest: &crate::policy::Manifest) -> Tier {
    let executor = &manifest.policy.executor;
    Tier {
        agent: executor.agent.agent().word().to_string(),
        model: executor.model.clone(),
        effort: executor.effort.clone(),
        tools: executor.tools.clone(),
        prompt_cache_ttl: executor.prompt_cache_ttl.clone(),
        version: executor.version.clone(),
    }
}

/// Every tier a manifest can dispatch to: its executor, the route's two,
/// and the handoff's second.
#[must_use]
pub fn tiers(manifest: &crate::policy::Manifest) -> Vec<Tier> {
    let mut out = vec![manifest_tier(manifest)];
    let control = &manifest.policy.control;
    if let Some(route) = &control.route {
        out.push(route.cheap.clone());
        out.push(route.strong.clone());
    }
    if let Some(to) = control.handoff.as_ref().and_then(|h| h.to.clone()) {
        out.push(to);
    }
    if let Some(families) = control.route.as_ref().and_then(|r| r.families.as_ref()) {
        out.extend(families.profiles.values().cloned());
    }
    if let Some(second) = manifest
        .policy
        .verify
        .as_ref()
        .and_then(|v| v.second.as_ref())
    {
        out.extend(second.to.iter().cloned());
    }
    if let Some(persist) = &control.persist {
        out.extend(persist.tiers());
    }
    out
}

/// Whether a manifest runs the composition rather than one executor.
#[must_use]
pub fn composes(manifest: &crate::policy::Manifest) -> bool {
    let control = &manifest.policy.control;
    manifest.policy.verify.is_some()
        || control.route.is_some()
        || control.horizon.is_some()
        || control.persist.is_some()
        || control
            .handoff
            .as_ref()
            .is_some_and(|h| h.pattern != Pattern::Single)
}

/// Where the route started, and why.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Routed {
    /// `cheap`, `strong`, `family`, or `manifest` when no route ran.
    pub start: String,
    pub tier: Tier,
    pub reason: String,
}

/// The rule's pick. `profile-v1`: strong when the profile can't be read,
/// the task is long, its difficulty is high, or a hard feature is likely;
/// cheap otherwise. `profile-v2`: the same, except that a long deadline
/// only lowers the difficulty bar to `long_hard_at`, so the deadline alone
/// never decides.
#[must_use]
pub fn decide(
    route: &RoutePolicy,
    profile: &crate::profile::Profile,
    total_sec: Option<u64>,
) -> Routed {
    let strong = |reason: String| Routed {
        start: "strong".to_string(),
        tier: route.strong.clone(),
        reason,
    };
    let long =
        matches!((route.long_after_sec, total_sec), (Some(after), Some(total)) if total >= after);
    let v2 = route.rule == "profile-v2";
    if long && !v2 {
        let (after, total) = (
            route.long_after_sec.unwrap_or_default(),
            total_sec.unwrap_or_default(),
        );
        return strong(format!(
            "the episode deadline of {total} s is at least the long-task bound of {after} s"
        ));
    }
    let Some(difficulty) = profile.difficulty else {
        return strong("task.profile gave no difficulty, so the safe start is strong".to_string());
    };
    let bar = if long && v2 {
        route.long_hard_at.unwrap_or(route.hard_at)
    } else {
        route.hard_at
    };
    if difficulty >= bar {
        return strong(format!(
            "difficulty {difficulty:.2} is at least {bar:.2}{}",
            if long && v2 && (bar - route.hard_at).abs() > f64::EPSILON {
                " (the bar for a long task)"
            } else {
                ""
            }
        ));
    }
    for feature in &route.hard_features {
        if let Some(Some(p)) = profile.features.get(feature)
            && *p >= route.feature_at
        {
            return strong(format!(
                "{feature} is likely ({p:.2} at least {:.2})",
                route.feature_at
            ));
        }
    }
    Routed {
        start: "cheap".to_string(),
        tier: route.cheap.clone(),
        reason: format!(
            "difficulty {difficulty:.2} is below {bar:.2} and no hard feature is likely{}",
            if long {
                ", though the deadline is long"
            } else {
                ""
            }
        ),
    }
}

/// [`decide`], then the family table: a family pick with enough trials
/// replaces the rule's executor. Returns the family record too.
#[must_use]
pub fn decide_with_families(
    route: &RoutePolicy,
    profile: &crate::profile::Profile,
    total_sec: Option<u64>,
    instruction: &str,
) -> (Routed, Value) {
    let routed = decide(route, profile, total_sec);
    let Some(families) = &route.families else {
        return (routed, Value::Null);
    };
    let (picked, record) = families.pick(instruction, &routed.tier);
    match picked {
        Some((name, tier)) => {
            let reason = format!(
                "{}; the family table overrides it: {}",
                routed.reason,
                record["why"].as_str().unwrap_or_default()
            );
            let _ = name;
            (
                Routed {
                    start: "family".to_string(),
                    tier,
                    reason,
                },
                record,
            )
        }
        None => (routed, record),
    }
}

// ---------------------------------------------------------------------------
// Executors.
// ---------------------------------------------------------------------------

/// A dispatch's executor: a real CLI or a script.
pub enum Exec {
    Cli(Box<Cli>),
    Scripted(Box<Scripted>),
}

impl Exec {
    /// The last session's host-loop record.
    #[must_use]
    pub fn last(&self) -> Value {
        match self {
            Exec::Cli(cli) => cli.control.last.clone().unwrap_or(Value::Null),
            Exec::Scripted(scripted) => scripted.last.clone().unwrap_or(Value::Null),
        }
    }

    /// Sets the monitor that watches its sessions.
    pub fn watch(&mut self, monitor: Option<MonitorSetup>) {
        match self {
            Exec::Cli(cli) => cli.control.monitor = monitor,
            Exec::Scripted(scripted) => scripted.monitor = monitor,
        }
    }

    /// Dispatches run so far.
    #[must_use]
    pub fn runs(&self) -> u32 {
        match self {
            Exec::Cli(cli) => cli.runs,
            Exec::Scripted(scripted) => scripted.runs,
        }
    }
}

impl Executor for Exec {
    fn agent(&self) -> &str {
        match self {
            Exec::Cli(cli) => cli.agent(),
            Exec::Scripted(scripted) => scripted.agent(),
        }
    }
    fn cost_provenance(&self) -> &'static str {
        match self {
            Exec::Cli(cli) => cli.cost_provenance(),
            Exec::Scripted(scripted) => scripted.cost_provenance(),
        }
    }
    fn model(&self) -> &str {
        match self {
            Exec::Cli(cli) => cli.model(),
            Exec::Scripted(scripted) => scripted.model(),
        }
    }
    fn deadline(&self) -> Duration {
        match self {
            Exec::Cli(cli) => cli.deadline(),
            Exec::Scripted(scripted) => scripted.deadline(),
        }
    }
    fn describe(&self) -> Map<String, Value> {
        match self {
            Exec::Cli(cli) => cli.describe(),
            Exec::Scripted(scripted) => scripted.describe(),
        }
    }
    async fn execute(&mut self, briefing: &Briefing) -> Report {
        match self {
            Exec::Cli(cli) => cli.execute(briefing).await,
            Exec::Scripted(scripted) => scripted.execute(briefing).await,
        }
    }
    fn system_options(&self) -> Vec<String> {
        match self {
            Exec::Cli(cli) => cli.system_options(),
            Exec::Scripted(scripted) => scripted.system_options(),
        }
    }
    fn select_system(&mut self, answers: Vec<(String, Option<f64>)>) {
        match self {
            Exec::Cli(cli) => cli.select_system(answers),
            Exec::Scripted(scripted) => scripted.select_system(answers),
        }
    }
}

/// Makes the executor for a tier.
pub trait Factory {
    /// An executor for `tier` that asks for `deadline` and has `runs`
    /// dispatches before it, so its files don't overwrite theirs.
    ///
    /// # Errors
    ///
    /// Returns why the tier can't run here.
    fn make(&mut self, tier: &Tier, deadline: Duration, runs: u32) -> Result<Exec, String>;
}

// ---------------------------------------------------------------------------
// The composition.
// ---------------------------------------------------------------------------

/// Everything the composition reads besides the executors.
pub struct Setup<'a> {
    pub manifest: &'a crate::policy::Manifest,
    pub instruction: &'a str,
    pub workdir: &'a Path,
    /// The episode's output directory: `verification/` and `artifacts/`
    /// go under it.
    pub dir: &'a Path,
    pub recorder: &'a Recorder,
    pub deadline: &'a Deadline,
    pub jev: JevMode,
    /// task.profile's answer when the caller already has one; the route
    /// asks Jev otherwise.
    pub profile: Option<crate::profile::Profile>,
    /// The workdir's Git base, for the handoff brief's diff.
    pub base: Option<&'a str>,
}

/// What the composition leaves.
pub struct Composed {
    pub ended: Ended,
    pub delegated: Option<Delegated>,
    pub record: Value,
}

/// The usage limit a delegate session in this episode hit, as its record.
pub(crate) fn limited(recorder: &Recorder) -> Option<Value> {
    crate::limit::from_steps(&recorder.steps())
}

/// One dispatch's ledger row.
fn row(role: &str, tier: &Tier, report: &Report, last: &Value, granted: u64) -> Value {
    let (charge, _) = delegate::charge(report);
    json!({
        "role": role,
        "tier": tier,
        "status": report.status.word(),
        "milliseconds": report.milliseconds,
        "requested_sec": granted,
        "usd": (charge == "priced").then_some(report.summary.total_cost_usd).flatten(),
        "charge": charge,
        "turns": report.summary.num_turns,
        "stopped_by": last["stopped_by"],
        "session_id": report.summary.session_id,
    })
}

/// What `generic.self-report` found in a check: `null` when it didn't
/// run, else its verdict and findings.
#[must_use]
pub fn self_reported(report: &checks::Report) -> Value {
    report
        .verdicts
        .iter()
        .find(|v| v.scenario == "generic.self-report")
        .map_or(Value::Null, |v| {
            json!({
                "verdict": v.verdict,
                "findings": v.observations,
                "requirement": report
                    .scenarios
                    .iter()
                    .find(|s| s.id == v.scenario)
                    .and_then(|s| s.requirements.first()),
            })
        })
}

/// The commands every executor session so far reported, in order.
#[must_use]
pub fn claimed(recorder: &Recorder) -> Vec<generic::Claimed> {
    recorder
        .steps()
        .iter()
        .filter_map(|step| {
            let event = step
                .extensions
                .get(crate::session::EVENT_KEY)?
                .get("event")?;
            (event.get("kind")?.as_str()? == "command_completed").then(|| generic::Claimed {
                command: crate::stream::unwrap_shell(event["command"].as_str().unwrap_or_default()),
                exit_code: event.get("exit_code").and_then(Value::as_i64),
            })
        })
        .collect()
}

/// The last failing commands since step `from`.
fn last_errors(recorder: &Recorder, from: usize, n: usize) -> Vec<String> {
    let steps = recorder.steps();
    let mut errors: Vec<String> = steps[from.min(steps.len())..]
        .iter()
        .rev()
        .filter_map(|step| {
            let event = step
                .extensions
                .get(crate::session::EVENT_KEY)?
                .get("event")?;
            (event.get("kind")?.as_str()? == "command_completed"
                && event.get("exit_code")?.as_i64()? != 0)
                .then(|| {
                    format!(
                        "`{}` exited {}: {}",
                        crate::stream::unwrap_shell(event["command"].as_str().unwrap_or_default()),
                        event["exit_code"],
                        crate::judge::clip_tail(
                            event["output"].as_str().unwrap_or_default().trim(),
                            400
                        )
                    )
                })
        })
        .take(n)
        .collect();
    errors.reverse();
    errors
}

/// The handoff section a second executor reads after the first briefing.
fn handoff_section(
    from: &Tier,
    trigger: &str,
    report: Option<&checks::Report>,
    support: Option<&crate::support::Report>,
    changes: &str,
    errors: &[String],
) -> String {
    let mut text = format!(
        "\n\n## Handoff from {} ({trigger})\n\nAnother executor worked on this task first. Continue from the workspace as it is now; don't start over, and don't repeat what failed.\n",
        from.label()
    );
    if let Some(report) = report {
        text.push_str("\n### Requirement states from the host's checks\n\n");
        for covered in &report.coverage {
            text.push_str(&format!(
                "- {} ({}): {}\n",
                covered.id,
                covered.state,
                crate::judge::clip(&covered.text, 240)
            ));
        }
        for packet in report.packets.iter().take(3) {
            text.push_str(&format!(
                "\nThe check `{}` expected: {}\nIt observed:\n```json\n{}\n```\n",
                packet.scenario,
                packet.expected.statement,
                crate::judge::clip(
                    &crate::support::scrub(
                        &serde_json::to_string_pretty(&packet.observations).unwrap_or_default()
                    ),
                    1_500
                )
            ));
        }
    }
    if let Some(support) = support {
        let contradicted: Vec<&crate::support::State> = support
            .states
            .iter()
            .filter(|s| s.state == "contradicted")
            .collect();
        if !contradicted.is_empty() {
            text.push_str("\n### Requirements a judge read as contradicted\n\n");
            for state in contradicted {
                text.push_str(&format!(
                    "- {}: {}\n",
                    state.id,
                    crate::judge::clip(&state.text, 240)
                ));
            }
        }
    }
    if !errors.is_empty() {
        text.push_str("\n### The last failing commands\n\n");
        for error in errors {
            text.push_str(&format!("- {error}\n"));
        }
    }
    text.push_str(&format!(
        "\n### What changed in the workspace\n\n```\n{}\n```\n",
        crate::judge::clip(changes, 3_000)
    ));
    text
}

fn record_decision(recorder: &Recorder, component: &str, name: &str, record: &Value) {
    let id = recorder.begin(
        Start::new(
            component,
            Implementation::new(component, name, &json!({ "version": 1 })),
        )
        .named(name)
        .with_effects(),
    );
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(record.clone())
            .cost(Cost::none()),
    );
}

/// Asks task.profile and routes, or keeps the manifest's executor.
async fn route(setup: &Setup<'_>, total: Option<u64>) -> (Routed, Value) {
    let Some(policy) = &setup.manifest.policy.control.route else {
        let tier = manifest_tier(setup.manifest);
        return (
            Routed {
                start: "manifest".to_string(),
                tier,
                reason: "no control.route: the manifest's executor starts".to_string(),
            },
            Value::Null,
        );
    };
    let profile = match &setup.profile {
        Some(profile) => profile.clone(),
        None => {
            let issue = json!({ "title": crate::judge::clip(setup.instruction.lines().next().unwrap_or("Task"), 120), "body": setup.instruction });
            match crate::profile::request(&issue, &[]) {
                Ok((state, questions)) => {
                    let asked = crate::component::jev::ask(
                        &setup.jev,
                        setup.recorder,
                        crate::component::jev::Ask {
                            component: "task.profile",
                            name: "jev_profile",
                            id: "jev-profile".to_string(),
                            state,
                            questions,
                            parent: None,
                            deadline: Some(setup.deadline.clone()),
                        },
                    )
                    .await;
                    crate::profile::read(asked.answers.as_ref())
                }
                Err(_) => crate::profile::Profile::default(),
            }
        }
    };
    let (routed, family) = decide_with_families(policy, &profile, total, setup.instruction);
    let mut record = json!({
        "rule": policy.rule,
        "profile": profile,
        "start": routed.start,
        "tier": routed.tier,
        "reason": routed.reason,
        "episode_deadline_sec": total,
    });
    if !family.is_null() {
        record["family"] = family;
    }
    record_decision(
        setup.recorder,
        ROUTE,
        &format!("start {}", routed.start),
        &record,
    );
    println!(
        "  route ▸ {} {} · {}",
        routed.start,
        routed.tier.label(),
        routed.reason
    );
    (routed, record)
}

/// Asks the effort battery over the task and the files the workspace
/// starts with, and picks the effort a long task runs at.
async fn choose_effort(setup: &Setup<'_>, policy: &crate::effort::EffortPolicy) -> (String, Value) {
    let workspace = crate::effort::workspace_files(setup.workdir, 400);
    let (state, questions) = crate::effort::request(setup.instruction, &workspace);
    let asked = crate::component::jev::ask(
        &setup.jev,
        setup.recorder,
        crate::component::jev::Ask {
            component: crate::effort::COMPONENT,
            name: "jev_effort",
            id: "jev-effort".to_string(),
            state,
            questions,
            parent: None,
            deadline: Some(setup.deadline.clone()),
        },
    )
    .await;
    let features = crate::effort::read(asked.answers.as_ref());
    let decided = policy.decide(&features);
    let record = json!({
        "rule": policy.rule,
        "features": features,
        "score": decided.score,
        "at": policy.at,
        "effort": decided.effort,
        "base": policy.base,
        "raised": policy.raised,
        "reason": decided.reason,
        "jev": asked.how,
        "workspace_files": workspace.len(),
    });
    record_decision(
        setup.recorder,
        crate::effort::COMPONENT,
        &format!("effort {}", decided.effort),
        &record,
    );
    println!("  effort ▸ {} · {}", decided.effort, decided.reason);
    (decided.effort, record)
}

/// The monitor the first session runs under: acting for an escalation,
/// shadow otherwise, with its silence scaled to the dispatch.
fn monitor_for(
    setup: &Setup<'_>,
    escalating: Option<&handoff::Policy>,
    dispatch_sec: u64,
) -> Option<MonitorSetup> {
    let control = &setup.manifest.policy.control;
    let params = escalating
        .and_then(|h| h.monitor.clone())
        .or_else(|| control.monitor.clone())
        .or_else(|| {
            escalating.map(|_| MonitorParams {
                jev: false,
                silence_ms: Some(60_000),
                ..MonitorParams::default()
            })
        })?;
    let mut params = params;
    // A long command is not a stall: the silence scales with the dispatch.
    if let Some(silence) = params.silence_ms {
        params.silence_ms = Some(silence.max(dispatch_sec * 1_000 / 12));
    }
    Some(MonitorSetup {
        jev: if params.jev {
            setup.jev.clone()
        } else {
            JevMode::Off
        },
        params,
        task: setup.instruction.to_string(),
        acting: escalating.map(|h| Acting {
            intent: Intent::Stop,
            after: h.after,
            on: h.on.clone(),
            by: handoff::COMPONENT.to_string(),
        }),
    })
}

/// Runs the composition: route, an optional planner, the first executor
/// through the explore-then-delegate path, the checks and support, an
/// escalation, and one repair.
///
/// # Errors
///
/// Returns a message when a first executor can't be made or a record
/// can't be written. A failed session is an outcome, not an error.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn run<J, G, S, F>(
    setup: &Setup<'_>,
    state: &mut State,
    plan: &Plan<'_>,
    judge: &mut J,
    generator: &mut G,
    shell: &mut S,
    factory: &mut F,
    checkpoint: &mut dyn FnMut(&State),
) -> Result<Composed, String>
where
    J: Explorer,
    G: Generate,
    S: Shell,
    F: Factory,
{
    let manifest = setup.manifest;
    let control = &manifest.policy.control;
    let mut horizon = control.horizon.clone().unwrap_or_default();
    let verify = manifest
        .policy
        .verify
        .clone()
        .unwrap_or_else(VerifyPolicy::checks_only);
    let total = setup.deadline.remaining().map(|left| left.as_secs());
    let long = horizon.long(total);
    let fallback = manifest.policy.executor.deadline_sec;
    let (routed, route_record) = route(setup, total).await;
    // control.effort picks the long-task effort every later tier reads.
    let effort_record = match &control.effort {
        Some(policy) if long => {
            let (effort, record) = choose_effort(setup, policy).await;
            horizon.long_effort = Some(effort);
            record
        }
        _ => Value::Null,
    };
    let mut first = routed.tier.clone();
    let handoff = control
        .handoff
        .clone()
        .filter(|h| matches!(h.pattern, Pattern::Escalate | Pattern::PlannerWorker));
    // An escalation to the executor that already started is no escalation.
    let escalate = handoff
        .clone()
        .filter(|h| h.pattern == Pattern::Escalate)
        .and_then(|h| h.to.clone().map(|to| (h, to)))
        .filter(|(_, to)| to.agent != first.agent || to.model != first.model);
    let mut escalate_to = escalate.as_ref().map(|(_, to)| to.clone());
    if long && let Some(effort) = &horizon.long_effort {
        first.effort = Some(effort.clone());
        if let Some(to) = &mut escalate_to {
            to.effort = Some(effort.clone());
        }
    }
    let mut runs: u32 = 0;
    let mut branches: Vec<Value> = Vec::new();
    let mut handoffs: Vec<Value> = Vec::new();
    let (budget, command_sec) = horizon.checks(total);
    let subject = Subject {
        label: "terminal-bench task".to_string(),
        task: TaskText {
            title: state.issue.title.clone(),
            instruction: setup.instruction.to_string(),
        },
        requirements: Some(judge.jev().requirements.clone()),
        provided: Vec::new(),
        inputs: (setup.instruction.contains("logs/") && setup.workdir.join("logs").is_dir())
            .then(|| "logs".to_string()),
        budget,
        live: Some(generic::Workspace {
            dir: setup.workdir.to_string_lossy().into_owned(),
            claimed: Vec::new(),
            command_sec,
            report: None,
            options: verify.check_options(),
            root: None,
            collected: Vec::new(),
        }),
    };
    let support_params = verify.support_params(long);

    // verify.second needs the task's original state to run a second
    // executor from, so it is set aside before anything runs.
    let base = match &verify.second {
        Some(second) => Some(Snapshot::take(
            setup.workdir,
            &output_paths(subject.requirements.as_ref(), setup.workdir),
            second.max_copy_mb,
            "base",
        )),
        None => None,
    };

    // control.persist lists what the executors changed, so it reads the
    // workspace before any of them runs.
    let initial = control
        .persist
        .as_ref()
        .map(|_| persist::files_of(&subject, setup.workdir));

    // planner-worker: the planner writes a plan in a scratch copy first.
    let mut directions = plan.directions.to_string();
    let mut planned = Value::Null;
    if let Some(h) = handoff
        .as_ref()
        .filter(|h| h.pattern == Pattern::PlannerWorker)
        && let Some(planner) = h.to.clone()
    {
        planned = plan_first(setup, &planner, factory, &mut runs, &horizon, &mut branches).await;
        if let Some(limit) = limited(setup.recorder) {
            println!("  usage limit ▸ the planner's session was throttled; stopping");
            let record = json!({ "schema": SCHEMA, "route": route_record, "planner": planned, "branches": branches, "usage_limited": limit });
            let ended = Ended::Delegated {
                answered: false,
                status: format!("refused: {}", delegate::USAGE_LIMIT),
                title: state.issue.title.clone(),
                summary: limit["message"].as_str().unwrap_or_default().to_string(),
                steps: state.history.len(),
            };
            return Ok(Composed {
                ended,
                delegated: None,
                record,
            });
        }
        if let Some(text) = planned["plan"].as_str().filter(|t| !t.trim().is_empty()) {
            directions.push_str(&format!(
                "\n\n## The plan from {}\n\nA planner read this task and wrote the plan below. Follow it, and check each scenario it names before you finish.\n\n{}\n",
                planner.label(),
                text.trim()
            ));
            handoffs
                .push(json!({ "action": "plan", "from": planner.label(), "to": first.label() }));
        }
    }

    // The first executor.
    let follows = if escalate.is_some() {
        horizon.first_share
    } else if verify.repair.is_some() {
        horizon.later_share
    } else {
        1.0
    };
    let first_sec = horizon.dispatch_sec(setup.deadline.allowance(), fallback, follows);
    let mut exec = factory.make(&first, Duration::from_secs(first_sec), runs)?;
    exec.watch(monitor_for(
        setup,
        escalate.as_ref().map(|(h, _)| h),
        first_sec,
    ));
    let steps_before = setup.recorder.steps().len();
    let plan_first = Plan {
        mode: plan.mode,
        policy: plan.policy,
        max_steps: plan.max_steps,
        prompt: plan.prompt,
        instruction: plan.instruction,
        directions: &directions,
        cap: plan.cap,
        packer: plan.packer,
        pack: plan.pack,
        isolation: plan.isolation,
        base: plan.base,
    };
    let (mut ended, delegated) = delegate::explore_then_delegate(
        state,
        &plan_first,
        judge,
        generator,
        shell,
        &mut exec,
        setup.recorder,
        checkpoint,
    )
    .await;
    runs = exec.runs();
    let last = exec.last();
    if let Some(d) = &delegated {
        branches.push(row("primary", &first, &d.report, &last, first_sec));
    }
    drop(exec);
    let Some(first_delegation) = delegated.as_ref() else {
        // The explorer finished without delegating: nothing to verify here.
        let record = json!({ "schema": SCHEMA, "route": route_record, "branches": branches, "note": "the explorer ended without delegating" });
        return Ok(Composed {
            ended,
            delegated,
            record,
        });
    };

    // A throttled executor stops the composition: every later dispatch
    // would draw on the same exhausted quota.
    let mut usage_limited = limited(setup.recorder);
    if usage_limited.is_some() {
        println!("  usage limit ▸ the first executor's session was throttled; stopping");
    }

    // verify.snapshot: what the first executor left, with the subject the
    // first check reads, before any check or repair changes it.
    let snapshot = verify.snapshot.as_ref().map(|policy| {
        let mut subject = subject.clone();
        if let Some(live) = &mut subject.live {
            live.claimed = claimed(setup.recorder);
            live.report = Some(first_delegation.report.output());
        }
        let outside = output_paths(subject.requirements.as_ref(), setup.workdir);
        let taken = crate::snapshot::take(setup.dir, setup.workdir, &outside, &subject, policy);
        println!(
            "  snapshot ▸ {}",
            if taken["taken"] == true {
                format!(
                    "{} files, {} bytes archived",
                    taken["files"], taken["archive"]["bytes"]
                )
            } else {
                format!(
                    "subject only: {}",
                    taken["reason"].as_str().unwrap_or("not archived")
                )
            }
        );
        taken
    });

    // verify.checks and verify.support on what the first executor left.
    let mut checks_log = Vec::new();
    let check = |file: &'static str, report: Option<String>| {
        let mut subject = subject.clone();
        if let Some(live) = &mut subject.live {
            live.claimed = claimed(setup.recorder);
            live.report = report;
        }
        async move {
            checks::check_subject_as(&subject, setup.workdir, setup.dir, setup.recorder, file).await
        }
    };
    let mut checked = if verify.checks && usage_limited.is_none() {
        Some(
            check(
                checks::COVERAGE_FILE,
                Some(first_delegation.report.output()),
            )
            .await,
        )
    } else {
        None
    };
    if let Some((_, report)) = &checked {
        checks_log.push(json!({ "after": "primary", "file": checks::COVERAGE_FILE, "summary": report.summary(), "self_report": self_reported(report) }));
    }
    let mut support = if usage_limited.is_none() {
        judge_support(
            setup,
            &verify,
            checked.as_ref(),
            support_params,
            crate::support::FILE,
        )
        .await?
    } else {
        None
    };
    // The final report of the session that produced the candidate.
    let mut previous = first_delegation.report.output();

    // control.handoff escalate.
    let mut escalated = false;
    if let Some((policy, to)) = escalate.as_ref().filter(|_| usage_limited.is_none()) {
        let to = escalate_to.clone().unwrap_or_else(|| to.clone());
        let stopped = last["stopped_by"].as_str() == Some(handoff::COMPONENT);
        let failed_check = checked.as_ref().is_some_and(|(_, r)| r.detected());
        let contradicted = support
            .as_ref()
            .is_some_and(|s| s.states.iter().any(|x| x.state == "contradicted"));
        let unanswered = first_delegation.report.status != Status::Answered && !stopped;
        let trigger = [
            (stopped, "the monitor stopped a stalled session"),
            (failed_check, "a check failed"),
            (contradicted, "a requirement was judged contradicted"),
            (unanswered, "the session ended without an answer"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, why)| *why)
        .collect::<Vec<_>>();
        let left = setup.deadline.allowance();
        let room = left.is_none_or(|left| left.as_secs() >= horizon.min_dispatch_sec.min(60));
        if !trigger.is_empty() && policy.max_handoffs > 0 && room {
            let trigger = trigger.join("; ");
            let section = handoff_section(
                &first,
                &trigger,
                checked.as_ref().map(|(_, r)| r),
                support.as_ref(),
                &delegate::changes(setup.workdir, setup.base),
                &last_errors(setup.recorder, steps_before, 2),
            );
            let mut briefing = first_delegation.briefing.clone();
            briefing.text.push_str(&section);
            briefing.included.push("handoff brief".to_string());
            let follows = if verify.repair.is_some() {
                horizon.later_share
            } else {
                1.0
            };
            let sec = horizon.dispatch_sec(setup.deadline.allowance(), fallback, follows);
            let record = json!({
                "pattern": "escalate",
                "action": "escalate",
                "from": first.label(),
                "to": to.label(),
                "trigger": trigger,
                "brief_chars": section.chars().count(),
                "requested_sec": sec,
                "remaining_ms": setup.deadline.allowance().map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            });
            record_decision(setup.recorder, handoff::COMPONENT, "escalate", &record);
            println!(
                "  handoff ▸ escalate {} → {} ({trigger})",
                first.label(),
                to.label()
            );
            handoffs.push(record);
            let mut exec = factory.make(&to, Duration::from_secs(sec), runs)?;
            // The second executor runs under a shadow monitor: it records,
            // and nothing follows it to hand off to.
            exec.watch(monitor_for(setup, None, sec));
            let session = setup.recorder.enter(
                Start::new(
                    "exec.session",
                    Implementation::new(
                        "exec.session",
                        &format!("{} {}", exec.agent(), exec.model()),
                        &json!({ "agent": exec.agent(), "model": exec.model(), "deadline_sec": sec, "role": "escalation" }),
                    ),
                )
                .named(&format!("escalation · {} ({})", exec.agent(), exec.model()))
                .reading_digest(briefing.sha256())
                .with_effects(),
            );
            let reason = Reason::Handoff(trigger.clone());
            let report = delegate::delegate(
                &mut exec,
                &briefing,
                &Delegation {
                    mode: Mode::Always,
                    reason: &reason,
                    isolation: plan.isolation,
                },
                setup.recorder,
                runs,
            )
            .await;
            setup.recorder.end(
                &session,
                Finish::new(if report.status == Status::Answered {
                    Outcome::Completed
                } else {
                    Outcome::Failed
                })
                .summary(
                    json!({ "status": report.status.word(), "milliseconds": report.milliseconds }),
                ),
            );
            runs = exec.runs();
            branches.push(row("escalation", &to, &report, &exec.last(), sec));
            ended = delegate::ending(&ended, &report, state.history.len(), &state.issue.title);
            previous = report.output();
            first = to.clone();
            escalated = true;
            usage_limited = limited(setup.recorder);
            if verify.checks && usage_limited.is_none() {
                checked = Some(check(ESCALATED_CHECKS, Some(report.output())).await);
                if let Some((_, report)) = &checked {
                    checks_log.push(json!({ "after": "escalation", "file": ESCALATED_CHECKS, "summary": report.summary(), "self_report": self_reported(report) }));
                }
                support = judge_support(
                    setup,
                    &verify,
                    checked.as_ref(),
                    support_params,
                    crate::support::FILE,
                )
                .await?;
            }
        } else if !trigger.is_empty() {
            handoffs.push(json!({ "action": "none", "why": "no time left for an escalation", "trigger": trigger.join("; ") }));
        }
    }

    // verify.repair: one fresh session from the packets.
    let mut repaired = Value::Null;
    if let (Some(policy), Some((input, report)), None) =
        (verify.repair, checked.as_ref(), &usage_limited)
    {
        let place = crate::repair::Place {
            task: None,
            subject: &subject,
            work: setup.workdir,
            dir: setup.dir,
            artifacts: &setup.dir.join("artifacts"),
            recorder: setup.recorder,
            deadline: setup.deadline,
            jev: verify.support.then(|| setup.jev.clone()),
            previous_session: branches
                .last()
                .and_then(|b| b["session_id"].as_str().map(str::to_string)),
            support_params: Some(support_params),
        };
        let allowance =
            Duration::from_secs(horizon.dispatch_sec(setup.deadline.allowance(), fallback, 1.0));
        let tier = first.clone();
        let made_runs = runs;
        let result = crate::repair::attempt(
            &place,
            (input, report),
            support.as_ref(),
            crate::repair::Policy {
                kind: policy.brief,
                trigger: policy.trigger,
                allowance,
            },
            |granted| factory.make(&tier, granted, made_runs),
        )
        .await?;
        if result.changed
            && let Some(said) = result.record["session"]["result"].as_str()
        {
            previous = said.to_string();
        }
        repaired = json!({
            "tier": first,
            "triggered": result.record["triggered"],
            "ran": result.ran,
            "changed": result.changed,
            "skipped": result.record["skipped"],
            "session": result.record["session"],
            "recheck": result.record["recheck"],
            "cost_usd": result.cost_usd,
        });
        usage_limited = limited(setup.recorder);
        if let Some(recheck) = result.recheck {
            checks_log.push(json!({ "after": "repair", "file": crate::repair::RECHECK_FILE, "summary": recheck.1.summary(), "self_report": self_reported(&recheck.1) }));
            checked = Some(recheck);
        }
        // With Jev off, the rejudgment's states are unanswered, not judged.
        if let Some(after) = result
            .support_after
            .filter(|_| !matches!(setup.jev, JevMode::Off))
        {
            support = Some(after);
        }
    }

    // verify.second: a second executor when the checks can't confirm the
    // result.
    let mut second_record = Value::Null;
    if let (Some(policy), Some(base), None) = (&verify.second, base, &usage_limited) {
        let context = SecondContext {
            setup,
            subject: &subject,
            verify: &verify,
            horizon: &horizon,
            long,
            support_params,
            fallback,
            isolation: plan.isolation,
            briefing: &first_delegation.briefing,
        };
        let outcome = verify_by_second(
            &context,
            policy,
            base,
            &first,
            checked.as_ref(),
            support.as_ref(),
            factory,
            &mut runs,
        )
        .await?;
        if let Some((tier, report, last, sec)) = &outcome.branch {
            branches.push(row("second", tier, report, last, *sec));
        }
        if let Some((summary, reported)) = &outcome.checks_summary {
            checks_log.push(json!({ "after": "second", "file": SECOND_CHECKS, "summary": summary, "self_report": reported }));
        }
        if outcome.kept_second {
            if let Some((tier, report, _, _)) = &outcome.branch {
                ended = delegate::ending(&ended, report, state.history.len(), &state.issue.title);
                previous = report.output();
                first = tier.clone();
            }
            checked = outcome.checked;
            support = outcome.support;
        }
        second_record = outcome.record;
        usage_limited = limited(setup.recorder);
    }

    // control.persist: fresh rounds while a long task has time left.
    let mut persist_record = Value::Null;
    if let (Some(policy), Some(initial), None) = (&control.persist, &initial, &usage_limited) {
        let outside = output_paths(subject.requirements.as_ref(), setup.workdir);
        let context = persist::Context {
            setup,
            subject: &subject,
            verify: &verify,
            horizon: &horizon,
            long,
            support_params,
            fallback,
            isolation: plan.isolation,
            initial,
            outside: &outside,
        };
        let current = persist::Current {
            tier: first.clone(),
            checked: checked.take(),
            support: support.take(),
            previous,
        };
        let persisted = persist::run(&context, policy, current, factory, &mut runs).await?;
        branches.extend(persisted.branches);
        checks_log.extend(persisted.checks_log);
        if let Some(report) = &persisted.report {
            ended = delegate::ending(&ended, report, state.history.len(), &state.issue.title);
        }
        first = persisted.current.tier;
        checked = persisted.current.checked;
        support = persisted.current.support;
        persist_record = persisted.record;
        usage_limited = limited(setup.recorder);
    }

    let record = json!({
        "schema": SCHEMA,
        "route": route_record,
        "first": routed,
        "effort": effort_record,
        "horizon": {
            "policy": horizon,
            "episode_deadline_sec": total,
            "long": long,
            "first_dispatch_sec": first_sec,
            "check_budget": budget,
            "command_sec": command_sec,
        },
        "planner": planned,
        "handoffs": handoffs,
        "escalated": escalated,
        "branches": branches,
        "checks": checks_log,
        "support": support.as_ref().map(crate::support::Report::summary),
        "repair": repaired,
        "second": second_record,
        "persist": persist_record,
        "final_tier": first,
        "final_checks": checked.as_ref().map(|(_, report)| report.summary()),
        "verify": verify,
        "snapshot": snapshot,
        "usage_limited": usage_limited,
    });
    Ok(Composed {
        ended,
        delegated,
        record,
    })
}

async fn judge_support(
    setup: &Setup<'_>,
    verify: &VerifyPolicy,
    checked: Option<&(checks::Input, checks::Report)>,
    params: crate::support::Params,
    file: &str,
) -> Result<Option<crate::support::Report>, String> {
    let (Some((input, report)), true) = (checked, verify.support) else {
        return Ok(None);
    };
    if matches!(setup.jev, JevMode::Off) {
        return Ok(None);
    }
    let judged = crate::support::judge(
        input,
        report,
        &setup.jev,
        setup.recorder,
        params,
        Some(setup.deadline.clone()),
    )
    .await;
    crate::support::save_as(&judged, setup.dir, file)?;
    Ok(Some(judged))
}

// ---------------------------------------------------------------------------
// verify.second.
// ---------------------------------------------------------------------------

/// The absolute output paths the requirements name outside `workdir`: the
/// state a second executor could clobber that a copy of the workspace
/// doesn't hold.
fn output_paths(map: Option<&crate::requirements::RequirementMap>, workdir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for requirement in map.map(|m| m.requirements.as_slice()).unwrap_or_default() {
        for path in &requirement.extracted.paths {
            let path = Path::new(path.trim());
            if path.is_absolute()
                && !path.starts_with(workdir)
                && !path.starts_with("/tests")
                && path.extension().is_some()
                && !out.iter().any(|p| p == path)
            {
                out.push(path.to_path_buf());
            }
        }
    }
    out
}

/// A copy of the workspace and of the named files outside it.
struct Snapshot {
    /// The scratch copy of the workspace; `None` when it couldn't be made.
    dir: Option<PathBuf>,
    /// Each outside file's content, or `None` when it didn't exist.
    outside: Vec<(PathBuf, Option<Vec<u8>>)>,
    /// Why the copy couldn't be made.
    refused: Option<String>,
}

impl Snapshot {
    fn take(workdir: &Path, outside: &[PathBuf], max_mb: u64, label: &str) -> Snapshot {
        let outside = outside
            .iter()
            .map(|path| {
                let bytes = std::fs::metadata(path)
                    .ok()
                    .filter(|m| m.is_file() && m.len() <= 64 * 1024 * 1024)
                    .and_then(|_| std::fs::read(path).ok());
                (path.clone(), bytes)
            })
            .collect();
        let refuse = |why: String| Snapshot {
            dir: None,
            outside: Vec::new(),
            refused: Some(why),
        };
        if !safe_to_replace(workdir) {
            return refuse(format!(
                "{} is not a workspace the host may replace",
                workdir.display()
            ));
        }
        match tree_size(workdir, 20_000) {
            Some((_, bytes)) if bytes <= max_mb * 1024 * 1024 => {}
            _ => {
                return refuse(format!(
                    "the workspace is over {max_mb} MiB or 20,000 files, too large to copy aside"
                ));
            }
        }
        static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "coder-one-second-{label}-{}-{}-{}",
            std::process::id(),
            atif::now_ms(),
            TAKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match handoff::copy_tree(workdir, &dir) {
            Ok(()) => Snapshot {
                dir: Some(dir),
                outside,
                refused: None,
            },
            Err(error) => {
                let _ = std::fs::remove_dir_all(&dir);
                refuse(format!("cannot copy the workspace aside: {error}"))
            }
        }
    }

    /// Puts the snapshot back: the workspace's contents replaced by the
    /// copy, and each outside file restored or removed.
    fn restore(&self, workdir: &Path) -> Result<(), String> {
        let Some(dir) = &self.dir else {
            return Err("no copy to restore".to_string());
        };
        replace_contents(workdir, dir)?;
        for (path, bytes) in &self.outside {
            match bytes {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::write(path, bytes)
                        .map_err(|e| format!("cannot restore {}: {e}", path.display()))?;
                }
                None => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        Ok(())
    }

    fn discard(&self) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Whether the host may empty `dir` and refill it: never the root, a
/// top-level system directory, or the home directory.
fn safe_to_replace(dir: &Path) -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    dir.is_absolute()
        && dir.components().count() >= 2
        && home.as_deref() != Some(dir)
        && ![
            "/bin", "/boot", "/dev", "/etc", "/lib", "/proc", "/sys", "/usr", "/var", "/tmp",
            "/root",
        ]
        .iter()
        .any(|system| dir == Path::new(system))
}

/// Empties `dir` and copies `from`'s contents into it.
fn replace_contents(dir: &Path, from: &Path) -> Result<(), String> {
    if !safe_to_replace(dir) {
        return Err(format!(
            "{} is not a workspace the host may replace",
            dir.display()
        ));
    }
    for entry in std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let removed = if kind.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
    }
    crate::repair::copy_tree(from, dir)
}

/// How well one candidate's checks establish it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Standing {
    /// Failed scenarios, the self-report's included.
    pub failed: usize,
    /// Requirements `verify.support` read as contradicted and no scenario
    /// failed.
    pub contradicted: usize,
    /// Requirements a scenario observed passing or `verify.support` read
    /// as supported.
    pub confirmed: usize,
    /// Scenarios other than the self-report that passed.
    pub passed_scenarios: usize,
    /// Requirements `verify.support` left unresolved.
    pub unresolved: usize,
}

impl Standing {
    /// The standing of a check report and the support states that hold for
    /// its candidate.
    #[must_use]
    pub fn of(report: &checks::Report, support: Option<&crate::support::Report>) -> Standing {
        let candidate = report.candidate["digest"].as_str().unwrap_or_default();
        let fresh: Vec<&crate::support::State> = support
            .map(|s| s.states.iter().filter(|x| x.fresh_for(candidate)).collect())
            .unwrap_or_default();
        let failed = report
            .verdicts
            .iter()
            .filter(|v| v.verdict == "failed")
            .count();
        let contradicted_by_scenario: Vec<&str> = report
            .coverage
            .iter()
            .filter(|c| c.state == "contradicted")
            .map(|c| c.id.as_str())
            .collect();
        let contradicted = fresh
            .iter()
            .filter(|s| {
                s.state == "contradicted" && !contradicted_by_scenario.contains(&s.id.as_str())
            })
            .count();
        let mut confirmed: Vec<&str> = report
            .coverage
            .iter()
            .filter(|c| c.state == "observed")
            .map(|c| c.id.as_str())
            .collect();
        for state in &fresh {
            if state.state == "supported" && !confirmed.contains(&state.id.as_str()) {
                confirmed.push(&state.id);
            }
        }
        let passed_scenarios = report
            .verdicts
            .iter()
            .filter(|v| v.verdict == "passed" && !v.scenario.starts_with("generic.self-report"))
            .count();
        let unresolved = fresh.iter().filter(|s| s.state == "unresolved").count();
        Standing {
            failed,
            contradicted,
            confirmed: confirmed.len(),
            passed_scenarios,
            unresolved,
        }
    }

    /// Whether `other` is better: fewer failures and contradictions, then
    /// more confirmed requirements. A tie keeps this one.
    #[must_use]
    pub fn beaten_by(&self, other: &Standing) -> bool {
        let bad = |s: &Standing| s.failed + s.contradicted;
        bad(other) < bad(self) || (bad(other) == bad(self) && other.confirmed > self.confirmed)
    }

    /// The triggers in `on` this standing meets, with why.
    #[must_use]
    pub fn triggers(&self, on: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        if on.iter().any(|o| o == "failed") && self.failed + self.contradicted > 0 {
            out.push(format!(
                "failed: {} failed scenario(s) and {} contradicted requirement(s) remain",
                self.failed, self.contradicted
            ));
        }
        if on.iter().any(|o| o == "unconfirmed") {
            if self.passed_scenarios == 0 {
                out.push("unconfirmed: no scenario confirmed the result".to_string());
            } else if self.unresolved > 0 {
                out.push(format!(
                    "unconfirmed: verify.support left {} requirement(s) unresolved",
                    self.unresolved
                ));
            }
        }
        out
    }
}

/// What the second executor reads besides its policy.
struct SecondContext<'a> {
    setup: &'a Setup<'a>,
    subject: &'a Subject,
    verify: &'a VerifyPolicy,
    horizon: &'a Horizon,
    long: bool,
    support_params: crate::support::Params,
    fallback: u64,
    isolation: &'a str,
    briefing: &'a Briefing,
}

/// What the second executor left.
struct SecondOutcome {
    record: Value,
    /// The dispatch's tier, report, host-loop record, and the seconds it
    /// asked for.
    branch: Option<(Tier, Report, Value, u64)>,
    /// The second candidate's checks summary and self-report.
    checks_summary: Option<(Value, Value)>,
    kept_second: bool,
    checked: Option<(checks::Input, checks::Report)>,
    support: Option<crate::support::Report>,
}

impl SecondOutcome {
    fn skipped(record: Value) -> SecondOutcome {
        SecondOutcome {
            record,
            branch: None,
            checks_summary: None,
            kept_second: false,
            checked: None,
            support: None,
        }
    }
}

/// `verify.second`: when the first line's checks can't confirm its
/// result, sets that candidate aside, restores the task's original state,
/// runs a second executor from the same briefing, checks what it leaves,
/// and keeps the candidate whose checks confirm more.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn verify_by_second<F: Factory>(
    context: &SecondContext<'_>,
    policy: &SecondPolicy,
    base: Snapshot,
    produced_by: &Tier,
    checked: Option<&(checks::Input, checks::Report)>,
    support: Option<&crate::support::Report>,
    factory: &mut F,
    runs: &mut u32,
) -> Result<SecondOutcome, String> {
    let setup = context.setup;
    let finish = |record: Value, base: &Snapshot| {
        base.discard();
        record_decision(setup.recorder, SECOND, "second executor", &record);
        if let Some(why) = record["skipped"].as_str() {
            println!("  second ▸ skipped: {why}");
        }
        record
    };
    let Some((_, first_report)) = checked else {
        let record = json!({ "skipped": "no check ran on the first candidate" });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    };
    let before = Standing::of(first_report, support);
    let triggers = before.triggers(&policy.on);
    if triggers.is_empty() {
        let record = json!({ "skipped": "the checks confirmed the result", "first": before });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    }
    if let Some(why) = &base.refused {
        let record = json!({ "skipped": why, "first": before, "trigger": triggers });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    }
    let left = setup.deadline.allowance().map(|d| d.as_secs());
    if left.is_some_and(|left| left < policy.min_remaining_sec) {
        let record = json!({
            "skipped": format!("less than {} s left in the episode", policy.min_remaining_sec),
            "first": before,
            "trigger": triggers,
        });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    }
    let Some(mut tier) = policy
        .to
        .iter()
        .find(|t| t.agent != produced_by.agent || t.model != produced_by.model)
        .cloned()
    else {
        let record = json!({ "skipped": "every verify.second executor is the one that produced the candidate", "first": before, "trigger": triggers });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    };
    if context.long
        && let Some(effort) = &context.horizon.long_effort
    {
        tier.effort = Some(effort.clone());
    }
    let outside: Vec<PathBuf> = base.outside.iter().map(|(p, _)| p.clone()).collect();
    let first_copy = Snapshot::take(setup.workdir, &outside, u64::MAX / (1024 * 1024), "first");
    if let Some(why) = &first_copy.refused {
        let record = json!({ "skipped": why, "first": before, "trigger": triggers });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    }
    if let Err(error) = base.restore(setup.workdir) {
        // Put the first candidate back before giving up.
        let _ = first_copy.restore(setup.workdir);
        first_copy.discard();
        let record = json!({ "skipped": format!("cannot restore the original state: {error}"), "first": before, "trigger": triggers });
        return Ok(SecondOutcome::skipped(finish(record, &base)));
    }
    let trigger = triggers.join("; ");
    let sec =
        context
            .horizon
            .dispatch_sec(setup.deadline.allowance(), context.fallback, policy.share);
    println!(
        "  second ▸ {} on the original state ({trigger})",
        tier.label()
    );
    let mut exec = factory.make(&tier, Duration::from_secs(sec), *runs)?;
    exec.watch(monitor_for(setup, None, sec));
    let session = setup.recorder.enter(
        Start::new(
            "exec.session",
            Implementation::new(
                "exec.session",
                &format!("{} {}", exec.agent(), exec.model()),
                &json!({ "agent": exec.agent(), "model": exec.model(), "deadline_sec": sec, "role": "second" }),
            ),
        )
        .named(&format!("second · {} ({})", exec.agent(), exec.model()))
        .reading_digest(context.briefing.sha256())
        .with_effects(),
    );
    let reason = Reason::Handoff(format!("verify.second: {trigger}"));
    let report = delegate::delegate(
        &mut exec,
        context.briefing,
        &Delegation {
            mode: Mode::Always,
            reason: &reason,
            isolation: context.isolation,
        },
        setup.recorder,
        *runs,
    )
    .await;
    setup.recorder.end(
        &session,
        Finish::new(if report.status == Status::Answered {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "status": report.status.word(), "milliseconds": report.milliseconds })),
    );
    *runs = exec.runs();
    let last = exec.last();
    drop(exec);

    let mut subject = context.subject.clone();
    if let Some(live) = &mut subject.live {
        live.claimed = claimed(setup.recorder);
        live.report = Some(report.output());
    }
    let second_checked = checks::check_subject_as(
        &subject,
        setup.workdir,
        setup.dir,
        setup.recorder,
        SECOND_CHECKS,
    )
    .await;
    let second_support = judge_support(
        setup,
        context.verify,
        Some(&second_checked),
        context.support_params,
        SECOND_SUPPORT,
    )
    .await?;
    let after = Standing::of(&second_checked.1, second_support.as_ref());
    let keep_second = before.beaten_by(&after);
    let restored = if keep_second {
        None
    } else {
        Some(first_copy.restore(setup.workdir))
    };
    first_copy.discard();
    let why = if keep_second {
        format!(
            "the second candidate has {} failure(s) and {} confirmed requirement(s), against the first's {} and {}",
            after.failed + after.contradicted,
            after.confirmed,
            before.failed + before.contradicted,
            before.confirmed
        )
    } else {
        "the second candidate's checks don't beat the first's, so the first stays".to_string()
    };
    let mut record = json!({
        "tier": tier,
        "trigger": trigger,
        "requested_sec": sec,
        "status": report.status.word(),
        "first": before,
        "second": after,
        "kept": if keep_second { "second" } else { "first" },
        "why": why,
        "checks_file": SECOND_CHECKS,
        "support_file": second_support.as_ref().map(|_| SECOND_SUPPORT),
    });
    if let Some(Err(error)) = &restored {
        record["restore_error"] = json!(error);
    }
    println!(
        "  second ▸ kept the {} candidate · {why}",
        if keep_second { "second" } else { "first" }
    );
    let record = finish(record, &base);
    let summary = (second_checked.1.summary(), self_reported(&second_checked.1));
    Ok(SecondOutcome {
        record,
        branch: Some((tier, report, last, sec)),
        checks_summary: Some(summary),
        kept_second: keep_second,
        checked: keep_second.then_some(second_checked),
        support: if keep_second { second_support } else { None },
    })
}

/// The planner's dispatch in a scratch copy of the workspace, where
/// nothing it writes counts. Returns its record, with the plan's text.
async fn plan_first<F: Factory>(
    setup: &Setup<'_>,
    planner: &Tier,
    factory: &mut F,
    runs: &mut u32,
    horizon: &Horizon,
    branches: &mut Vec<Value>,
) -> Value {
    let scratch = std::env::temp_dir().join(format!("coder-one-planner-{}", atif::now_ms()));
    let size = tree_size(setup.workdir, 20_000);
    let Some((files, bytes)) = size.filter(|(_, bytes)| *bytes <= 256 * 1024 * 1024) else {
        let why = "the workspace is too large to copy for a planner";
        record_decision(
            setup.recorder,
            handoff::COMPONENT,
            "plan skipped",
            &json!({ "why": why }),
        );
        return json!({ "skipped": why });
    };
    if let Err(error) = handoff::copy_tree(setup.workdir, &scratch) {
        return json!({ "skipped": format!("cannot copy the workspace: {error}") });
    }
    let sec = horizon
        .dispatch_sec(setup.deadline.allowance(), 600, 0.15)
        .min(1_800);
    let text = format!(
        "{}\n\n## Your role: planner\n\nYou are in a scratch copy of the task's workspace; nothing you change here counts. Don't change any file. Read what you need, then reply with a numbered plan an implementer can follow, and the scenarios that would show the task done, one per line starting with `Scenario:`.\n",
        setup.instruction.trim()
    );
    let briefing = Briefing {
        cap: text.chars().count(),
        text,
        included: vec!["task".to_string(), "planner role".to_string()],
        omitted: Vec::new(),
    };
    let made = factory
        .make(planner, Duration::from_secs(sec), *runs)
        .map(|mut exec| {
            match &mut exec {
                Exec::Cli(cli) => cli.workdir.clone_from(&scratch),
                Exec::Scripted(scripted) => scripted.workdir.clone_from(&scratch),
            }
            exec
        });
    let record = match made {
        Err(error) => json!({ "skipped": error }),
        Ok(mut exec) => {
            let reason = Reason::Handoff("planner-worker: the planner writes the plan".to_string());
            let report = delegate::delegate(
                &mut exec,
                &briefing,
                &Delegation {
                    mode: Mode::Always,
                    reason: &reason,
                    isolation: "a scratch copy of the workspace",
                },
                setup.recorder,
                *runs,
            )
            .await;
            *runs = exec.runs();
            branches.push(row("planner", planner, &report, &exec.last(), sec));
            let plan = (report.status == Status::Answered).then(|| report.output());
            let record = json!({
                "tier": planner,
                "status": report.status.word(),
                "plan_chars": plan.as_ref().map(|p| p.chars().count()),
                "workspace_files": files,
                "workspace_bytes": bytes,
                "plan": plan,
            });
            record_decision(setup.recorder, handoff::COMPONENT, "plan", &record);
            record
        }
    };
    let _ = std::fs::remove_dir_all(&scratch);
    record
}

/// Files and bytes under `dir`, `.git` included, or `None` past `max`
/// files.
fn tree_size(dir: &Path, max: usize) -> Option<(usize, u64)> {
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    let (mut files, mut bytes) = (0usize, 0u64);
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).ok()?.flatten() {
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                if files > max {
                    return None;
                }
            }
        }
    }
    Some((files, bytes))
}

#[cfg(test)]
mod tests;
