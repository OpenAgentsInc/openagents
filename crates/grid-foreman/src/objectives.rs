//! Validated objective authoring data. This is host policy, not a Nostr grant.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const CATALOG: &str = include_str!("../../../docs/foreman/objectives.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub v: String,
    pub requires: Vec<String>,
    pub repository: String,
    pub stage: String,
    pub selection: Selection,
    pub execution: Execution,
    pub objectives: Vec<Objective>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub trigger: String,
    pub explicit_requests_first: bool,
    pub unknown_backlog: String,
    pub equal_priority_order: String,
    pub duplicate_checks: Vec<String>,
    pub no_candidate: String,
    pub review_interval_seconds: u64,
    pub max_active_explorations: usize,
    pub excluded_issues: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub preferred_computers: Vec<String>,
    pub requires_host_admission: bool,
    pub delivery: String,
    pub limits: String,
    pub merge: String,
    pub deploy: String,
    pub new_spend: String,
    pub owner_account_tests: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Objective {
    pub id: String,
    pub title: String,
    pub desired_state: String,
    pub priority: String,
    pub intent: String,
    pub source_paths: Vec<String>,
    pub read_first: Vec<String>,
    pub outcomes: Vec<String>,
    pub measurements: Vec<String>,
    pub candidate_examples: Vec<String>,
    pub constraints: Vec<String>,
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Select the native delivery route only after the operator configures its state.
pub fn kiln_brief(brief: String, state: &str) -> Result<String, String> {
    if !state.starts_with('/')
        || state.len() > 256
        || state.split('/').any(|p| p == ".." || p == ".")
        || !state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
    {
        return Err("invalid native Kiln state path".into());
    }
    Ok(format!(
        "{brief}\n\nOWNER EXECUTION ROUTE: Native Kiln. This replaces the direct editing, local-worktree, commit/push, and gh-pr-create steps above. Keep investigation and the single issue claim in this app chat; do not edit product code yourself. First run `kiln-host profile --state {state}` and consult the actual signed objective with `kiln-host objectives --state {state} --objective OBJECTIVE_SLUG`. Choose a useful task inside that objective and the admitted crate list. Do not substitute a cosmetic task or a different objective when a profile is unavailable; report the missing capability. Save a task JSON with exactly {{\"id\":\"64-hex-stable-attempt-ID\",\"objective\":\"OBJECTIVE_SLUG\",\"issue\":123,\"base\":\"40-hex-origin-main-commit\",\"krate\":\"admitted-package\",\"text\":\"bounded problem, evidence, expected behavior, and regression/check plan\"}}. Use the same task ID on retries. Submit with `kiln-host submit --state {state} --input TASK.json`. The persistent Kiln service performs all edits, Pylon inference, remote tests, NIP-34 publication, and draft PR creation. Read `kiln-host status --state {state} --id ID` until its terminal result; report the existing PR URL and exact checks in this chat. Never create a parallel PR, bypass Kiln, or retry an unknown result under a new ID. A stopped, failed, or unknown attempt requires inspection and release of your issue claim. Do not merge or deploy.\n"
    ))
}

#[cfg(test)]
mod native_route_tests {
    #[test]
    fn native_delivery_is_explicit_and_uses_a_plain_absolute_state_path() {
        let brief = super::kiln_brief("Improve Kiln".into(), "/var/lib/kiln-foreman").unwrap();
        assert!(brief.contains("kiln-host submit"));
        assert!(brief.contains("Never create a parallel PR"));
        for path in ["", "relative", "/var/../root", "/state;echo", "/a\ncommand"] {
            assert!(super::kiln_brief("task".into(), path).is_err());
        }
    }
}
fn strings(values: &[String]) -> bool {
    !values.is_empty()
        && values.len() <= 32
        && values
            .iter()
            .all(|s| !s.trim().is_empty() && s.len() <= 8000 && !s.contains('\0'))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn paths(values: &[String]) -> bool {
    strings(values)
        && values.iter().all(|s| {
            s.len() <= 256
                && !s.contains('\\')
                && ["crates/", "apps/", "docs/"]
                    .iter()
                    .any(|prefix| s.starts_with(prefix))
                && s.trim_end_matches('/')
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
        })
}
impl Catalog {
    /// Validate before importing. Unknown fields and unsupported policies fail closed.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 256 * 1024 {
            return Err("objective catalog is too large".into());
        }
        let c: Self = serde_json::from_slice(bytes).map_err(|_| "invalid objective catalog")?;
        let s = &c.selection;
        let e = &c.execution;
        let checks: BTreeSet<&str> = s.duplicate_checks.iter().map(String::as_str).collect();
        if c.schema != "./objectives.schema.json"
            || c.v != "openagents.foreman-objective-catalog.v1"
            || !c.requires.is_empty()
            || c.repository != "OpenAgentsInc/openagents"
            || c.stage != "authoring"
            || s.trigger != "no_eligible_issue"
            || !s.explicit_requests_first
            || s.unknown_backlog != "report_and_wait"
            || s.equal_priority_order != "least_recently_explored"
            || s.no_candidate != "record_and_back_off"
            || !(60..=86400).contains(&s.review_interval_seconds)
            || s.max_active_explorations != 1
            || s.excluded_issues.len() > 64
            || s.excluded_issues.contains(&0)
            || s.excluded_issues.iter().collect::<BTreeSet<_>>().len() != s.excluded_issues.len()
            || s.duplicate_checks.len() != 4
            || checks != BTreeSet::from(["claims", "issues", "pull_requests", "objective_history"])
            || !strings(&e.preferred_computers)
            || !e.requires_host_admission
            || e.delivery != "draft_pull_request"
            || e.limits != "existing_grants"
            || e.merge != "owner_only"
            || e.deploy != "owner_only"
            || e.new_spend != "owner_only"
            || e.owner_account_tests != "forbidden"
            || c.objectives.is_empty()
            || c.objectives.len() > 64
        {
            return Err("unsupported objective policy".into());
        }
        let mut ids = BTreeSet::new();
        for o in &c.objectives {
            if o.id.is_empty()
                || o.id.len() > 64
                || !o
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
                || !o.id.as_bytes()[0].is_ascii_alphanumeric()
                || !ids.insert(&o.id)
                || o.title.trim().is_empty()
                || o.title.chars().count() > 200
                || o.intent.trim().is_empty()
                || o.intent.chars().count() > 2000
                || !["active", "paused", "proposed"].contains(&o.desired_state.as_str())
                || !["high", "normal", "low"].contains(&o.priority.as_str())
                || !paths(&o.source_paths)
                || !paths(&o.read_first)
                || [
                    &o.outcomes,
                    &o.measurements,
                    &o.candidate_examples,
                    &o.constraints,
                ]
                .iter()
                .any(|v| !strings(v))
            {
                return Err("invalid objective definition".into());
            }
        }
        Ok(c)
    }
    pub fn revision(&self) -> String {
        digest(&serde_json::to_vec(self).expect("catalog serializes"))
    }
    pub fn objective(&self, id: &str) -> Option<&Objective> {
        self.objectives.iter().find(|o| o.id == id)
    }
    pub fn brief(&self, objective: &Objective, revision: &str, attempt: &str) -> String {
        format!(
            "Work on the standing objective: {} in {}.\nObjective: {} at catalog revision {}. Attempt: {}.\n\n{}\n\n\
Read the repository AGENTS.md, query the code graph, and inspect fresh source and measurements. \
Search open issues, claim comments, pull requests, and previous objective work before selecting ONE useful, bounded improvement. \
Do not work excluded issues {:?}, duplicate claimed work, or invent a defect from an example. \
Read these first: {}. Source scope: {}.\n\nDesired outcomes:\n{}\n\nMeasurements:\n{}\n\nConstraints:\n{}\n\n\
Create or reuse one concrete GitHub issue after investigating, then claim it in THIS app chat with `openagents issue claim N`. \
If another chat holds it, choose different work. Record the exact base, scoped proposal, checks, baseline, and device needs before coding. \
Use your own git worktree and branch, targeted Rust checks, and matched before/after benchmarks for performance changes. \
Commit and push the branch, open ONE draft PR with `gh pr create --draft`, and reply with its URL, issue, checks, and measurements. \
Do not push main, merge, deploy, spend money, weaken checks, test on the owner's account, or stop production model processes. \
Release your claim if you stop without a PR. If no useful feasible task exists, record why and stop; do not make cosmetic work. \
For Kiln, reuse its agent/runner/patch contracts; use either its bridge or a direct PR for this attempt, never both.",
            objective.title,
            self.repository,
            objective.id,
            revision,
            attempt,
            objective.intent,
            self.selection.excluded_issues,
            objective.read_first.join(", "),
            objective.source_paths.join(", "),
            objective.outcomes.join("\n"),
            objective.measurements.join("\n"),
            objective.constraints.join("\n")
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_requested_objectives_and_preserves_boundaries() {
        let c = Catalog::parse(CATALOG.as_bytes()).unwrap();
        assert_eq!(
            c.objectives
                .iter()
                .filter(|o| o.desired_state == "active")
                .count(),
            5
        );
        assert_eq!(c.execution.preferred_computers, ["coderos-4080"]);
        assert_eq!(c.selection.excluded_issues, [12469]);
        assert_eq!(
            c.revision(),
            Catalog::parse(&serde_json::to_vec(&c).unwrap())
                .unwrap()
                .revision()
        );
        let brief = c.brief(&c.objectives[0], &c.revision(), "attempt");
        for text in [
            "draft PR",
            "12469",
            "claim",
            "before/after",
            "Do not push main",
        ] {
            assert!(brief.contains(text));
        }
    }
    #[test]
    fn refuses_ambiguous_or_unsafe_imports() {
        let original: serde_json::Value = serde_json::from_str(CATALOG).unwrap();
        for path in ["unknown", "duplicate", "escape", "policy", "requires"] {
            let mut v = original.clone();
            match path {
                "unknown" => v["objectives"][0]["authority"] = serde_json::json!(true),
                "duplicate" => v["objectives"][1]["id"] = v["objectives"][0]["id"].clone(),
                "escape" => {
                    v["objectives"][0]["source_paths"][0] = serde_json::json!("crates/../secrets")
                }
                "policy" => v["execution"]["merge"] = serde_json::json!("automatic"),
                _ => v["requires"] = serde_json::json!(["unknown"]),
            }
            assert!(
                Catalog::parse(&serde_json::to_vec(&v).unwrap()).is_err(),
                "{path}"
            );
        }
    }
}
