//! Cross-run queries over Jev's learning judgments: keep the runs whose
//! judgment is at or above a threshold, and group runs by reason, task,
//! agent, policy, or outcome.
//!
//! Code computes every group from the answers the learning store already
//! keeps; nothing here asks Jev. `gym runs --reason ID[=P]` and
//! `gym runs group --by …` are the command line over it, and an agent that
//! reads the Gym, such as `coder-one ask`, reads the same JSON.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::runs::{Run, clip_words};
use crate::runs_learning::{Answer, JUDGMENTS, REASON_AT, Rarity, judgment};

/// One `--reason ID[=P]` condition: the judgment's probability must be at
/// or above `at`.
#[derive(Clone, Debug, PartialEq)]
pub struct Reason {
    pub id: String,
    pub at: f64,
}

impl Reason {
    /// Parses `ID` or `ID=P`. The ID must name a judgment in the question
    /// set, and `P` must be a probability.
    ///
    /// # Errors
    ///
    /// Returns a message naming the valid IDs when the ID is unknown, and
    /// a message when `P` isn't a number from 0 to 1.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (id, at) = match text.split_once('=') {
            Some((id, at)) => {
                let at: f64 = at
                    .trim()
                    .parse()
                    .map_err(|_| format!("--reason {text}: {at} is not a probability"))?;
                if !(0.0..=1.0).contains(&at) {
                    return Err(format!("--reason {text}: {at} is not from 0 to 1"));
                }
                (id.trim(), at)
            }
            None => (text.trim(), REASON_AT),
        };
        if judgment(id).is_none() {
            return Err(format!(
                "unknown judgment {id}; the judgments are {}",
                JUDGMENTS
                    .iter()
                    .map(|j| j.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(Reason {
            id: id.to_owned(),
            at,
        })
    }

    /// Whether `answer` meets the condition. A run with no answer never
    /// does.
    #[must_use]
    pub fn holds(&self, answer: Option<&Answer>) -> bool {
        answer
            .and_then(|answer| answer.nouls.get(&self.id))
            .is_some_and(|p| *p >= self.at)
    }

    /// `unearned_success ≥ 0.50`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{} ≥ {:.2}", self.id, self.at)
    }
}

/// What `gym runs group --by` groups by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    /// Every judgment at or above its threshold; a run can be in several.
    Reason,
    Task,
    /// The agent: Coder One, Claude Code, and so on.
    Agent,
    /// The agent with its variant or model, such as `Coder One ·
    /// tunable-v6`: the configuration that ran.
    Policy,
    Outcome,
}

impl By {
    /// Parses the `--by` value.
    ///
    /// # Errors
    ///
    /// Returns a message naming the choices.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "reason" => Ok(By::Reason),
            "task" => Ok(By::Task),
            "agent" => Ok(By::Agent),
            "policy" => Ok(By::Policy),
            "outcome" => Ok(By::Outcome),
            other => Err(format!(
                "unknown grouping {other}; --by takes reason, task, agent, policy, or outcome"
            )),
        }
    }

    /// The word the JSON and the heading use.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            By::Reason => "reason",
            By::Task => "task",
            By::Agent => "agent",
            By::Policy => "policy",
            By::Outcome => "outcome",
        }
    }
}

/// One group: its key, its members, and each judgment's mean probability
/// over the members Jev has judged.
#[derive(Clone, Debug)]
pub struct Group<'a> {
    pub key: String,
    /// A reason group's tag, such as `claimed unearned success`.
    pub tag: Option<&'static str>,
    pub members: Vec<&'a Run>,
    /// Members with an answer.
    pub judged: usize,
    /// Each judgment's mean probability over the judged members, highest
    /// first; ties keep the question set's order.
    pub means: Vec<(&'static str, f64)>,
}

/// Groups `runs` by `by`. `reason_at` is the threshold a reason group
/// admits a run at, per judgment; a judgment it doesn't name takes
/// [`REASON_AT`]. Groups come largest first, then by key; members come
/// strongest first, a reason group's by that reason's probability and any
/// other's by the learning value weighed by `rarity`.
#[must_use]
pub fn group<'a>(
    runs: &[&'a Run],
    answers: &std::collections::HashMap<String, &Answer>,
    rarity: &Rarity,
    by: By,
    reason_at: &[Reason],
) -> Vec<Group<'a>> {
    let mut keyed: BTreeMap<String, Vec<&'a Run>> = BTreeMap::new();
    for run in runs {
        let keys: Vec<String> = match by {
            By::Reason => {
                let Some(answer) = answers.get(&run.id()) else {
                    continue;
                };
                JUDGMENTS
                    .iter()
                    .filter(|j| {
                        let at = reason_at
                            .iter()
                            .find(|r| r.id == j.id)
                            .map_or(REASON_AT, |r| r.at);
                        answer.nouls.get(j.id).is_some_and(|p| *p >= at)
                    })
                    .map(|j| j.id.to_owned())
                    .collect()
            }
            By::Task => vec![run.task.clone()],
            By::Agent => vec![run.agent.name().to_owned()],
            By::Policy => vec![run.agent_label()],
            By::Outcome => vec![run.outcome.word().to_owned()],
        };
        for key in keys {
            keyed.entry(key).or_default().push(run);
        }
    }
    let mut groups: Vec<Group<'a>> = keyed
        .into_iter()
        .map(|(key, mut members)| {
            // The strongest members first: a reason group by its own
            // probability, any other by the learning value.
            let strength = |run: &Run| -> f64 {
                answers.get(&run.id()).map_or(-1.0, |answer| match by {
                    By::Reason => answer.nouls.get(&key).copied().unwrap_or(0.0),
                    _ => answer.learning(rarity),
                })
            };
            members.sort_by(|a, b| strength(b).total_cmp(&strength(a)));
            let judged: Vec<&Answer> = members
                .iter()
                .filter_map(|run| answers.get(&run.id()).copied())
                .collect();
            let mut means: Vec<(&'static str, f64)> = JUDGMENTS
                .iter()
                .filter_map(|j| {
                    let values: Vec<f64> = judged
                        .iter()
                        .filter_map(|a| a.nouls.get(j.id).copied())
                        .collect();
                    (!values.is_empty())
                        .then(|| (j.id, values.iter().sum::<f64>() / values.len() as f64))
                })
                .collect();
            means.sort_by(|a, b| b.1.total_cmp(&a.1));
            Group {
                tag: (by == By::Reason)
                    .then(|| judgment(&key).map(|j| j.tag))
                    .flatten(),
                key,
                judged: judged.len(),
                members,
                means,
            }
        })
        .collect();
    groups.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(a.key.cmp(&b.key))
    });
    groups
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The groups as `gym runs group --json` prints them.
#[must_use]
pub fn groups_json(
    groups: &[Group<'_>],
    by: By,
    answers: &std::collections::HashMap<String, &Answer>,
    rarity: &Rarity,
    total: usize,
) -> Value {
    json!({
        "schema": "openagents.gym.runs-group.v1",
        "by": by.word(),
        "runs": total,
        "judged": answers.len(),
        "reason_at": REASON_AT,
        "groups": groups.iter().map(|group| json!({
            "key": group.key,
            "tag": group.tag,
            "count": group.members.len(),
            "judged": group.judged,
            "mean_probability": group.means.iter().map(|(id, p)| json!({
                "id": id,
                "mean": round2(*p),
            })).collect::<Vec<_>>(),
            "members": group.members.iter().map(|run| {
                let answer = answers.get(&run.id());
                json!({
                    "run": run.id(),
                    "task": run.task,
                    "agent": run.agent_label(),
                    "outcome": run.outcome.word(),
                    "learning": answer.map(|a| round2(a.learning(rarity))),
                    "probability": match (by, answer) {
                        (By::Reason, Some(a)) => a.nouls.get(&group.key).copied().map(round2),
                        _ => None,
                    },
                })
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// The groups as text: a heading line per group with its strongest mean
/// reasons, then up to `members` members.
#[must_use]
pub fn groups_text(
    groups: &[Group<'_>],
    by: By,
    answers: &std::collections::HashMap<String, &Answer>,
    rarity: &Rarity,
    members: usize,
) -> Vec<String> {
    let mut lines = vec![format!(
        "Terminal-Bench runs grouped by {}: {} groups",
        by.word(),
        groups.len()
    )];
    if by == By::Reason {
        lines.push(format!(
            "A run is in every reason Jev gave it at or above {REASON_AT:.2}; runs Jev hasn't judged are left out."
        ));
    }
    for group in groups {
        lines.push(String::new());
        let name = match group.tag {
            Some(tag) => format!("{} ({tag})", group.key),
            None => group.key.clone(),
        };
        lines.push(format!(
            "{name}: {} runs, {} judged",
            group.members.len(),
            group.judged
        ));
        let strongest: Vec<String> = group
            .means
            .iter()
            .take(3)
            .map(|(id, p)| format!("{id} {p:.2}"))
            .collect();
        if !strongest.is_empty() {
            lines.push(format!("  mean: {}", strongest.join(" · ")));
        }
        for run in group.members.iter().take(members) {
            let learning = answers.get(&run.id()).map_or_else(
                || "    ".to_owned(),
                |a| format!("{:.2}", a.learning(rarity)),
            );
            lines.push(format!(
                "  {learning}  {:<11} {:<34} {}",
                run.outcome.word(),
                clip_words(&run.agent_label(), 34),
                run.id()
            ));
        }
        if group.members.len() > members {
            lines.push(format!("  and {} more", group.members.len() - members));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_parses_with_and_without_a_threshold() {
        assert_eq!(
            Reason::parse("unearned_success").unwrap(),
            Reason {
                id: "unearned_success".to_owned(),
                at: REASON_AT
            }
        );
        assert_eq!(Reason::parse("near_miss=0.8").unwrap().at, 0.8);
        assert!(Reason::parse("near_miss=2").is_err());
        let unknown = Reason::parse("nope").unwrap_err();
        assert!(unknown.contains("unearned_success"), "{unknown}");
        assert_eq!(By::parse("policy").unwrap(), By::Policy);
        assert!(By::parse("color").is_err());
    }
}
