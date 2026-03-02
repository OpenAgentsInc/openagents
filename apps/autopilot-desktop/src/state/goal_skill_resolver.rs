//! Goal-driven skill relevance resolver for autonomous earnings flows.

use crate::app_state::SkillRegistryDiscoveredSkill;
use crate::state::autopilot_goals::{GoalObjective, GoalRecord};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalSkillCandidate {
    pub name: String,
    pub path: String,
    pub score: i32,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalSkillResolution {
    pub goal_id: String,
    pub objective_tag: String,
    pub candidates: Vec<GoalSkillCandidate>,
    pub skipped_disabled: Vec<String>,
}

pub fn resolve_goal_skill_candidates(
    goal: &GoalRecord,
    discovered_skills: &[SkillRegistryDiscoveredSkill],
) -> GoalSkillResolution {
    let objective_tag = objective_tag(&goal.objective);
    let objective_text = objective_text(&goal.objective);
    let mut candidates = Vec::<GoalSkillCandidate>::new();
    let mut skipped_disabled = Vec::<String>::new();

    for skill in discovered_skills {
        let skill_name = skill.name.trim();
        if skill_name.is_empty() {
            continue;
        }

        if !skill.enabled {
            skipped_disabled.push(skill_name.to_string());
            continue;
        }

        let normalized_name = skill_name.to_ascii_lowercase();
        let mut score = baseline_priority_score(&normalized_name);
        let mut reasons = Vec::<String>::new();

        if score > 0 {
            reasons.push(format!(
                "baseline priority '{}' for earnings stack",
                normalized_name
            ));
        }

        let objective_boost = objective_specific_boost(&goal.objective, &normalized_name);
        if objective_boost > 0 {
            score += objective_boost;
            reasons.push(format!(
                "objective boost {} for {}",
                objective_boost, objective_tag
            ));
        }

        if objective_text.contains(&normalized_name) {
            score += 15;
            reasons.push("custom objective text contains skill name".to_string());
        }
        if skill.path.to_ascii_lowercase().contains(&normalized_name) {
            score += 5;
            reasons.push("skill path affinity".to_string());
        }

        if score <= 0 {
            continue;
        }

        candidates.push(GoalSkillCandidate {
            name: skill.name.clone(),
            path: skill.path.clone(),
            score,
            reason: reasons.join("; "),
        });
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
            .then_with(|| left.path.cmp(&right.path))
    });

    skipped_disabled.sort();
    skipped_disabled.dedup();

    GoalSkillResolution {
        goal_id: goal.goal_id.clone(),
        objective_tag: objective_tag.to_string(),
        candidates,
        skipped_disabled,
    }
}

fn objective_tag(objective: &GoalObjective) -> &'static str {
    match objective {
        GoalObjective::EarnBitcoin { .. } => "earn_bitcoin",
        GoalObjective::SwapBtcToUsd { .. } => "swap_btc_to_usd",
        GoalObjective::SwapUsdToBtc { .. } => "swap_usd_to_btc",
        GoalObjective::Custom { .. } => "custom",
    }
}

fn objective_text(objective: &GoalObjective) -> String {
    match objective {
        GoalObjective::EarnBitcoin { note, .. } => {
            let mut text = String::from("earn bitcoin");
            if let Some(note) = note {
                text.push(' ');
                text.push_str(note);
            }
            text.to_ascii_lowercase()
        }
        GoalObjective::SwapBtcToUsd { note, .. } => {
            let mut text = String::from("swap btc to usd");
            if let Some(note) = note {
                text.push(' ');
                text.push_str(note);
            }
            text.to_ascii_lowercase()
        }
        GoalObjective::SwapUsdToBtc { note, .. } => {
            let mut text = String::from("swap usd to btc");
            if let Some(note) = note {
                text.push(' ');
                text.push_str(note);
            }
            text.to_ascii_lowercase()
        }
        GoalObjective::Custom { instruction } => instruction.to_ascii_lowercase(),
    }
}

fn baseline_priority_score(skill_name: &str) -> i32 {
    match skill_name {
        "blink" => 100,
        "l402" => 90,
        "moneydevkit" => 80,
        "neutronpay" => 70,
        _ => 0,
    }
}

fn objective_specific_boost(objective: &GoalObjective, skill_name: &str) -> i32 {
    match objective {
        GoalObjective::SwapBtcToUsd { .. } | GoalObjective::SwapUsdToBtc { .. } => {
            if skill_name == "blink" { 25 } else { 0 }
        }
        GoalObjective::EarnBitcoin { .. } => {
            if skill_name == "l402" {
                10
            } else {
                0
            }
        }
        GoalObjective::Custom { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use crate::state::autopilot_goals::{
        GoalConstraints, GoalLifecycleStatus, GoalObjective, GoalRecord, GoalRetryPolicy,
        GoalScheduleConfig, GoalStopCondition,
    };

    use super::resolve_goal_skill_candidates;

    fn sample_goal(objective: GoalObjective) -> GoalRecord {
        GoalRecord {
            goal_id: "goal-skill-resolver".to_string(),
            title: "Skill resolver goal".to_string(),
            objective,
            constraints: GoalConstraints::default(),
            stop_conditions: vec![GoalStopCondition::WalletDeltaSatsAtLeast { sats: 1 }],
            retry_policy: GoalRetryPolicy::default(),
            schedule: GoalScheduleConfig::default(),
            lifecycle_status: GoalLifecycleStatus::Queued,
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
            attempt_count: 0,
            last_failure_reason: None,
            terminal_reason: None,
            last_receipt_id: None,
            recovery_replay_pending: false,
        }
    }

    fn discovered_skill(
        name: &str,
        enabled: bool,
    ) -> crate::app_state::SkillRegistryDiscoveredSkill {
        crate::app_state::SkillRegistryDiscoveredSkill {
            name: name.to_string(),
            path: format!("/repo/skills/{name}/SKILL.md"),
            scope: "user".to_string(),
            enabled,
            interface_display_name: None,
            dependency_count: 0,
        }
    }

    #[test]
    fn resolver_orders_priority_skills_deterministically_for_earn_goal() {
        let goal = sample_goal(GoalObjective::EarnBitcoin {
            min_wallet_delta_sats: 1_000,
            note: None,
        });
        let discovered = vec![
            discovered_skill("neutronpay", true),
            discovered_skill("moneydevkit", true),
            discovered_skill("blink", true),
            discovered_skill("l402", true),
        ];

        let resolution = resolve_goal_skill_candidates(&goal, &discovered);
        let ordered = resolution
            .candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["blink", "l402", "moneydevkit", "neutronpay"]);
        assert!(
            resolution
                .candidates
                .iter()
                .all(|candidate| !candidate.reason.is_empty())
        );
    }

    #[test]
    fn resolver_tracks_disabled_priority_skills() {
        let goal = sample_goal(GoalObjective::SwapBtcToUsd {
            sell_sats: 5_000,
            note: None,
        });
        let discovered = vec![
            discovered_skill("blink", false),
            discovered_skill("l402", true),
            discovered_skill("moneydevkit", true),
        ];

        let resolution = resolve_goal_skill_candidates(&goal, &discovered);
        assert_eq!(resolution.candidates[0].name, "l402");
        assert!(resolution.skipped_disabled.contains(&"blink".to_string()));
    }
}
