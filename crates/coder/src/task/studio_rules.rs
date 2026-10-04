//! Standing approval rules: **Always allow for this seat**
//! (`docs/verse/agent-studio.md`, "Approvals are ours").
//!
//! A rule names one seat and one exact step: the tool, the command, and the
//! working directory an approval's engine named
//! ([`interaction::Step`](super::super::interaction::Step)). It has no
//! wildcard, so it admits that step and nothing wider. Its exact text
//! ([`text`]) is what the approval offered the person; the host records a
//! rule only for the text the person saw, and only for a step that is not
//! high risk. The rules are a file of their own beside the studio document,
//! separate from any single-use approval record.
//!
//! The host applies the rules, never a client: [`Studio::standing_answers`]
//! lists the waiting approvals a rule admits, and the host consumes the
//! rule for each wait once ([`Studio::note_applied`]) before it answers,
//! under a command identity bound to the rule, the task, and its revision
//! ([`Due::command`]), as the rule's author, whose grant it rechecks
//! first. An answer is data for the engine: the next turn runs
//! under a fresh grant with every usual check, so a rule never widens a
//! task's grant, boundary, routes, or spend. The bounded ledger of
//! consumed waits also says which rule approved which step.

use serde::{Deserialize, Serialize};

use super::super::interaction::{self, Step};
use super::intents::read;
use super::{Error, Inbox, Studio};
use coder_host::access::studio::Risk;

/// The standing rules and the answers they gave, beside the studio
/// document.
const RULES_FILE: &str = "rules.json";
const RULES_SCHEMA: &str = "openagents.coder.studio-rules.v1";
/// The most standing rules a studio keeps.
pub const MAX_RULES: usize = 64;
/// The most answers the ledger keeps; the oldest go first.
const MAX_APPLIED: usize = 256;

/// Who recorded a rule: the device whose `operate` right the host checked,
/// under its grant and epoch. The host answers under this identity and
/// rechecks it before each answer, so revoking the device stops its rules.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    pub device: String,
    pub grant: Option<String>,
    pub epoch: Option<u64>,
}

/// One standing rule.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// A digest of the seat and the step: the rule's identity.
    pub id: String,
    pub seat: String,
    pub tool: String,
    pub command: String,
    pub cwd: String,
    /// The exact text the approval offered.
    pub text: String,
    pub by: Author,
    pub created_at: u64,
}

impl Rule {
    /// Whether this rule admits `step` for seat `seat`: the same seat and
    /// exactly the same tool, command, and directory, for a step that is
    /// not high risk.
    #[must_use]
    pub fn admits(&self, seat: &str, step: &Step) -> bool {
        self.seat == seat
            && self.tool == step.tool
            && self.command == step.command
            && self.cwd == step.cwd
            && step.risk() != Risk::High
    }
}

/// One answer a rule gave.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Applied {
    pub rule: String,
    pub task: String,
    pub revision: u64,
    pub at: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    schema: String,
    rules: Vec<Rule>,
    applied: Vec<Applied>,
}

/// A waiting approval a standing rule admits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Due {
    pub task: String,
    /// The task revision the answer is based on.
    pub revision: u64,
    pub rule: Rule,
}

impl Due {
    /// The answer's command identity: a digest of the rule, the task, and
    /// the revision, so the host answers each wait once and a retry
    /// replays the same command.
    #[must_use]
    pub fn command(&self) -> String {
        hex(&[
            b"openagents.studio.standing-answer.v1".as_slice(),
            self.rule.id.as_bytes(),
            self.task.as_bytes(),
            self.revision.to_string().as_bytes(),
        ])
    }

    /// The answer the engine reads.
    #[must_use]
    pub fn answer(&self) -> String {
        format!("Approved by a standing rule: {}", self.rule.text)
    }
}

/// The exact text of the standing rule for `step` in seat `seat`, which
/// an approval offers and **Always allow for this seat** echoes back.
#[must_use]
pub fn text(seat: &str, step: &Step) -> String {
    format!(
        "{seat} may run {} {} in {} without asking again",
        step.tool,
        code(&step.command),
        step.cwd
    )
}

/// `value` as a Markdown code span, fenced with one more backtick than the
/// longest run inside it.
fn code(value: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in value.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest + 1);
    let pad = if longest > 0 { " " } else { "" };
    format!("{fence}{pad}{value}{pad}{fence}")
}

fn hex(parts: &[&[u8]]) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for part in parts {
        hash.update(part);
        hash.update(b"\0");
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The identity of the rule for `step` in seat `seat`.
fn rule_id(seat: &str, step: &Step) -> String {
    hex(&[
        b"openagents.studio.rule.v1".as_slice(),
        seat.as_bytes(),
        step.tool.as_bytes(),
        step.command.as_bytes(),
        step.cwd.as_bytes(),
    ])
}

impl Studio {
    /// The standing rules, oldest first.
    #[must_use]
    pub fn rules(&self) -> Vec<Rule> {
        read::<Rules>(&self.dir, RULES_FILE).rules
    }

    /// The answers the rules gave, oldest first.
    #[must_use]
    pub fn applied(&self) -> Vec<Applied> {
        read::<Rules>(&self.dir, RULES_FILE).applied
    }

    /// The seat that holds studio task `task_id`, if any.
    #[must_use]
    pub fn seat_of(&self, task_id: &str) -> Option<String> {
        self.state.goals.iter().find_map(|goal| {
            std::iter::once(&goal.lead)
                .chain(goal.plan.iter().map(|entry| &entry.slot))
                .find(|slot| slot.task_id == task_id)
                .map(|slot| slot.seat.clone())
        })
    }

    /// The standing rule offered for `step` in seat `seat`, as its exact
    /// text, or `None` for a high-risk step, which is approved once at a
    /// time.
    #[must_use]
    pub fn offer(seat: &str, step: &Step) -> Option<String> {
        (step.risk() != Risk::High).then(|| text(seat, step))
    }

    /// The standing rule that admits `step` for seat `seat`, if one does.
    #[must_use]
    pub fn standing_rule(&self, seat: &str, step: &Step) -> Option<Rule> {
        self.rules()
            .into_iter()
            .find(|rule| rule.admits(seat, step))
    }

    /// Keep a standing rule for `step` in seat `seat`, recorded by `by`.
    /// `offered` is the rule text the person saw; it must equal the text
    /// for this step now. Recording the same rule again keeps the first.
    ///
    /// # Errors
    /// No such seat, the offered text differs from the step's (the step
    /// moved on), the step is high risk, the studio holds the most rules,
    /// or the rules cannot be written.
    pub fn allow_always(
        &mut self,
        seat: &str,
        step: &Step,
        offered: &str,
        by: Author,
        now: u64,
    ) -> Result<Rule, Error> {
        if self.state.seat(seat).is_none() {
            return Err(Error::UnknownSeat(seat.into()));
        }
        if !step.valid() {
            return Err(Error::Invalid("the approval names no step".into()));
        }
        let Some(text) = Self::offer(seat, step) else {
            return Err(Error::State(
                "a high-risk step is approved once at a time".into(),
            ));
        };
        if text != offered {
            return Err(Error::State(
                "the standing rule differs from the one the approval offered".into(),
            ));
        }
        let mut rules = read::<Rules>(&self.dir, RULES_FILE);
        rules.schema = RULES_SCHEMA.into();
        let id = rule_id(seat, step);
        if let Some(rule) = rules.rules.iter().find(|rule| rule.id == id) {
            return Ok(rule.clone());
        }
        if rules.rules.len() >= MAX_RULES {
            return Err(Error::LimitExceeded("standing rules"));
        }
        let rule = Rule {
            id,
            seat: seat.into(),
            tool: step.tool.clone(),
            command: step.command.clone(),
            cwd: step.cwd.clone(),
            text,
            by,
            created_at: now,
        };
        rules.rules.push(rule.clone());
        self.write_sidecar(RULES_FILE, &rules)?;
        Ok(rule)
    }

    /// Remove the standing rule `id`. Returns whether there was one.
    ///
    /// # Errors
    /// The rules cannot be written.
    pub fn revoke_rule(&mut self, id: &str) -> Result<bool, Error> {
        let mut rules = read::<Rules>(&self.dir, RULES_FILE);
        let before = rules.rules.len();
        rules.rules.retain(|rule| rule.id != id);
        if rules.rules.len() == before {
            return Ok(false);
        }
        rules.schema = RULES_SCHEMA.into();
        self.write_sidecar(RULES_FILE, &rules)?;
        Ok(true)
    }

    /// The studio's waiting approvals that a standing rule admits: a task
    /// of the rule's seat whose turn ended asking approval of a step
    /// (`asked` answers a task identity with what it asked) that the rule
    /// matches exactly. A paused seat's approvals wait for the person.
    #[must_use]
    pub fn standing_answers(
        &self,
        tasks: &dyn Inbox,
        asked: &dyn Fn(&str) -> Option<String>,
    ) -> Vec<Due> {
        let Rules { rules, applied, .. } = read::<Rules>(&self.dir, RULES_FILE);
        if rules.is_empty() {
            return Vec::new();
        }
        let mut due = Vec::new();
        for goal in &self.state.goals {
            let slots =
                std::iter::once(&goal.lead).chain(goal.plan.iter().map(|entry| &entry.slot));
            for slot in slots {
                if self.paused_seat(&slot.seat) {
                    continue;
                }
                let Some(task) = tasks.task(&slot.task_id) else {
                    continue;
                };
                if interaction::pending(&task) != Some(interaction::Kind::Approval)
                    || applied
                        .iter()
                        .any(|done| done.task == slot.task_id && done.revision == task.revision)
                {
                    continue;
                }
                let Some(step) = asked(&slot.task_id).as_deref().and_then(Step::in_reply) else {
                    continue;
                };
                if let Some(rule) = rules.iter().find(|rule| rule.admits(&slot.seat, &step)) {
                    due.push(Due {
                        task: slot.task_id.clone(),
                        revision: task.revision,
                        rule: rule.clone(),
                    });
                }
            }
        }
        due
    }

    /// Consume `due`'s rule for its wait at `now`, before the host
    /// answers it: a wait is answered under a rule at most once, and a
    /// failed answer leaves the wait to the person. Returns whether this
    /// call consumed it; `false` when an earlier one did.
    ///
    /// # Errors
    /// The ledger cannot be written.
    pub fn note_applied(&mut self, due: &Due, now: u64) -> Result<bool, Error> {
        let mut rules = read::<Rules>(&self.dir, RULES_FILE);
        if rules
            .applied
            .iter()
            .any(|applied| applied.task == due.task && applied.revision == due.revision)
        {
            return Ok(false);
        }
        rules.schema = RULES_SCHEMA.into();
        rules.applied.push(Applied {
            rule: due.rule.id.clone(),
            task: due.task.clone(),
            revision: due.revision,
            at: now,
        });
        let excess = rules.applied.len().saturating_sub(MAX_APPLIED);
        rules.applied.drain(..excess);
        self.write_sidecar(RULES_FILE, &rules)?;
        Ok(true)
    }
}
