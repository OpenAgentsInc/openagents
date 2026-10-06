//! Bounded onboarding over facts the owning adapters already verified.
//! Local progress grants no XP, money, execution rights, or publication authority.

#[cfg(feature = "onboarding-host")]
pub mod host;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Real,
    Simulated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub host: String,
    pub workspace: String,
    pub lane: Lane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    Terminal,
    ScratchGoal,
    AnsweredDecision,
    ReviewedMerge,
    Contribution,
}

/// An owner projection with retained source references, never a saved completion flag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub scope: Scope,
    pub objective: Objective,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub objective: Objective,
    pub title: String,
    pub guide: String,
    pub prerequisites: Vec<String>,
    pub expected_cost: String,
    pub authority: String,
    pub complete: bool,
    pub references: Vec<String>,
}

fn identity(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'@')
        })
}

impl Scope {
    pub fn validate(&self) -> Result<(), String> {
        if !identity(&self.host, 128) || !identity(&self.workspace, 128) {
            return Err("Onboarding requires bounded host and workspace identities".into());
        }
        Ok(())
    }
}

/// Reopening recomputes the same progress and never dispatches work.
pub fn evaluate(scope: &Scope, facts: &[Fact]) -> Result<Vec<Row>, String> {
    scope.validate()?;
    if facts.len() > 64 {
        return Err("Onboarding accepts at most 64 owner facts".into());
    }
    for fact in facts {
        fact.scope.validate()?;
        if fact.references.is_empty()
            || fact.references.len() > 8
            || fact.references.iter().any(|r| !identity(r, 256))
        {
            return Err("Onboarding facts require one to eight bounded source references".into());
        }
    }
    let cost = match scope.lane {
        Lane::Simulated => "$0; simulated outcomes only",
        Lane::Real => {
            "Unknown until the existing studio shows its admitted plan and spending limit"
        }
    };
    let authority = match scope.lane {
        Lane::Simulated => {
            "Scratch simulation only; no model, host, payment, or publication authority"
        }
        Lane::Real => {
            "Use existing host grants and explicit confirmations; this tracker authorizes nothing"
        }
    };
    let definitions = [
        (
            Objective::Terminal,
            "First terminal",
            "Open an admitted terminal in the scratch workspace. A log or a pane click is not a shell.",
            "docs/terminal/smart-terminal.md",
        ),
        (
            Objective::ScratchGoal,
            "First scratch goal",
            "Submit one bounded goal to the existing studio in the isolated starter workspace. Inspect its task identity before continuing.",
            "docs/verse/agent-studio.md",
        ),
        (
            Objective::AnsweredDecision,
            "Answer one decision",
            "Read the exact decision and confirm its answer. Only a successful matching owner acknowledgment completes this step.",
            "docs/verse/agent-studio.md",
        ),
        (
            Objective::ReviewedMerge,
            "Review and merge locally",
            "Inspect the exact diff and checks. Merge the reviewed revisions locally in the scratch workspace; push nothing.",
            "docs/verse/agent-studio.md",
        ),
        (
            Objective::Contribution,
            "Inspect contribution evidence",
            "Inspect an exact contribution or reproduction, its retained check, and its credit scope. Inspection earns no XP.",
            "docs/coder/guides/xp.md",
        ),
    ];
    definitions
        .into_iter()
        .map(|(objective, title, guide, prerequisite)| {
            let references: BTreeSet<String> = facts
                .iter()
                .filter(|fact| fact.scope == *scope && fact.objective == objective)
                .flat_map(|fact| fact.references.iter().cloned())
                .collect();
            if references.len() > 8 {
                return Err("Onboarding objective exceeds eight retained references".into());
            }
            Ok(Row {
                objective,
                title: title.into(),
                guide: guide.into(),
                prerequisites: vec![prerequisite.into()],
                expected_cost: cost.into(),
                authority: authority.into(),
                complete: !references.is_empty(),
                references: references.into_iter().collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(lane: Lane) -> Scope {
        Scope {
            host: "scratch-host".into(),
            workspace: "scratch-workspace".into(),
            lane,
        }
    }

    #[test]
    fn exact_owner_facts_skip_completed_steps_without_activity_or_replay() {
        let simulated = scope(Lane::Simulated);
        let mut facts: Vec<Fact> = [
            Objective::Terminal,
            Objective::ScratchGoal,
            Objective::AnsweredDecision,
            Objective::ReviewedMerge,
            Objective::Contribution,
        ]
        .into_iter()
        .map(|objective| Fact {
            scope: simulated.clone(),
            objective,
            references: vec![format!("receipt:{objective:?}")],
        })
        .collect();
        let first = evaluate(&simulated, &facts).unwrap();
        assert!(first.iter().all(|row| row.complete));
        facts.extend(facts.clone());
        facts.reverse();
        assert_eq!(evaluate(&simulated, &facts).unwrap(), first);
        assert!(
            evaluate(&scope(Lane::Real), &facts)
                .unwrap()
                .iter()
                .all(|row| !row.complete)
        );
        assert!(evaluate(&simulated, &[]).unwrap().iter().all(|row| {
            !row.complete && !row.prerequisites.is_empty() && row.expected_cost.starts_with("$0")
        }));
    }

    #[test]
    fn changed_host_or_workspace_never_reuses_completion() {
        let real = scope(Lane::Real);
        let fact = Fact {
            scope: real.clone(),
            objective: Objective::ScratchGoal,
            references: vec!["task:retained".into()],
        };
        assert!(evaluate(&real, &[fact.clone()]).unwrap()[1].complete);
        let mut other = real.clone();
        other.host = "restarted-host".into();
        assert!(!evaluate(&other, &[fact.clone()]).unwrap()[1].complete);
        other = real;
        other.workspace = "other-workspace".into();
        assert!(!evaluate(&other, &[fact]).unwrap()[1].complete);
    }

    #[test]
    fn completion_requires_bounded_nonempty_evidence() {
        let real = scope(Lane::Real);
        for references in [vec![], vec!["private text\n".into()], vec!["x".repeat(257)]] {
            assert!(
                evaluate(
                    &real,
                    &[Fact {
                        scope: real.clone(),
                        objective: Objective::Terminal,
                        references,
                    }]
                )
                .is_err()
            );
        }
        let fact = Fact {
            scope: real.clone(),
            objective: Objective::Terminal,
            references: vec!["terminal:retained".into()],
        };
        assert!(evaluate(&real, &vec![fact; 65]).is_err());
    }
}
