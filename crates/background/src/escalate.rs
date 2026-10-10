//! When a disk cleanup run falls short (phase 3): judge the largest
//! unknown folders ([`crate::judged`]), and, for a rule that asks for it
//! (`escalate`), start one Coder run a day with a briefing code assembles
//! in the Jev-probe pattern of episode 287: sizes, ownership evidence,
//! what was skipped and why, and what Jev proposed. The run proposes rule
//! changes; it deletes nothing and applies nothing.

use crate::engine::{Judge, Step, StepOutcome};
use crate::judged::{self, Judgment, Proposals};
use crate::paths::{bytes, show};
use crate::plan::Env;
use crate::rule::{Rule, expand};
use crate::run::{Outcome, Record, Report};
use crate::services::{CoderRun, Services};
use crate::store::{self, State};

/// At most one escalation per rule in this long.
pub const EVERY_SECS: u64 = 86_400;

/// The Coder run's instructions; the briefing follows.
#[must_use]
pub fn prompt(rule: &Rule) -> String {
    format!(
        "The background rule `{id}` ({name}) on this computer fell short of its goal. Read the \
briefing below and propose changes that would reach the goal safely: edits to the rule \
(`openagents background show {id}` prints it; `openagents background edit {id} --message` \
drafts a change) or new rules. Do not delete, move, or change any file outside your worktree, \
and do not apply a rule change: write the proposal, with the dry run you expect, as your \
answer.",
        id = rule.id,
        name = rule.name
    )
}

/// Whether a run fell short: some volume it planned for is still below
/// its start level.
#[must_use]
pub fn short(report: &Report) -> bool {
    let Some(record) = &report.record else {
        return false;
    };
    report
        .plan
        .volumes
        .iter()
        .zip(&record.observation)
        .any(|(volume, seen)| volume.needed > 0 && seen.free_after < volume.start)
}

/// The briefing: facts code read, never a transcript.
#[must_use]
pub fn briefing(env: &Env<'_>, rule: &Rule, report: &Report, judgments: &[Judgment]) -> String {
    let home = &env.layout.home;
    let mut lines = vec![format!(
        "Rule: {} ({}), version {}, {}",
        rule.id,
        rule.name,
        rule.version,
        rule.digest()
    )];
    lines.push(String::new());
    lines.push("Volumes:".into());
    let observed = report.record.as_ref().map(|r| r.observation.clone());
    for (n, volume) in report.plan.volumes.iter().enumerate() {
        let after = observed
            .as_ref()
            .and_then(|o| o.get(n))
            .map_or(volume.space.free, |o| o.free_after);
        lines.push(format!(
            "- {}: {} free of {} after the run (was {}); cleans below {}, aims for {}",
            show(&volume.root, home),
            bytes(after),
            bytes(volume.space.total),
            bytes(volume.space.free),
            bytes(volume.start),
            bytes(volume.stop)
        ));
    }
    if let Some(record) = &report.record {
        let done: Vec<String> = record
            .actions
            .iter()
            .filter(|a| {
                matches!(
                    a.outcome,
                    Outcome::Deleted | Outcome::Removed | Outcome::Trashed | Outcome::Collected
                )
            })
            .map(|a| {
                format!(
                    "- class {} {} {} ({}{})",
                    a.class.number(),
                    bytes(a.bytes),
                    show(&a.path, home),
                    a.reason,
                    a.evidence
                        .task
                        .as_ref()
                        .map_or_else(String::new, |task| format!(", task {task}"))
                )
            })
            .collect();
        lines.push(String::new());
        lines.push(format!("Done this run: {} freed.", bytes(record.freed_sum)));
        lines.extend(done);
        let skipped: Vec<String> = record
            .actions
            .iter()
            .filter(|a| matches!(a.outcome, Outcome::Skipped | Outcome::Failed))
            .map(|a| format!("- {} ({})", show(&a.path, home), a.reason))
            .collect();
        if !skipped.is_empty() {
            lines.push("Skipped at deletion:".into());
            lines.extend(skipped);
        }
    }
    if !report.plan.kept.is_empty() {
        lines.push(String::new());
        lines.push("Kept, and why:".into());
        for kept in report.plan.kept.iter().take(40) {
            lines.push(format!(
                "- class {} {} ({})",
                kept.class.number(),
                show(&kept.path, home),
                kept.why
            ));
        }
        if report.plan.kept.len() > 40 {
            lines.push(format!("- and {} more", report.plan.kept.len() - 40));
        }
    }
    if !report.plan.not_cleaned.is_empty() {
        lines.push(String::new());
        lines.push("Measured, not a known cache:".into());
        for (path, size) in &report.plan.not_cleaned {
            lines.push(format!("- {} {}", show(path, home), bytes(*size)));
        }
    }
    let waiting = Proposals::load(env.layout);
    let waiting = waiting.waiting();
    if !judgments.is_empty() || !waiting.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Unknown folders Jev judged ({}, {:.2}):",
            judged::CACHE_DIR.name,
            judged::CACHE_DIR.default
        ));
        for judgment in judgments {
            lines.push(format!(
                "- {} {}: cache {:.2}, kind {} {:.2}{}",
                judgment.path,
                bytes(judgment.bytes),
                judgment.probability,
                judgment.kind,
                judgment.kind_probability,
                if judgment.proposed {
                    ", proposed to the person"
                } else {
                    ""
                }
            ));
        }
        for proposal in waiting {
            lines.push(format!(
                "- waiting for the person: {}",
                judged::line(proposal)
            ));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "Never touched: {}",
        env.layout
            .deny(rule)
            .iter()
            .map(|path| show(path, home))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    lines.join("\n")
}

/// After a run of `rule` that fell short: judge unknown folders (with a
/// judge), then escalate once a day (when the rule asks and the host can
/// start Coder runs). Recorded in the log as its own entry. `None` when
/// nothing was done.
#[must_use]
pub fn after(
    env: &Env<'_>,
    rule: &Rule,
    report: &Report,
    judge: Option<&dyn Judge>,
    services: Option<&dyn Services>,
) -> Option<Record> {
    if report.dry_run || !rule.cleans() || !short(report) {
        return None;
    }
    // Folders are looked at and judged at most once a day per rule.
    let state = State::load(env.layout)
        .rules
        .get(&rule.id)
        .cloned()
        .unwrap_or_default();
    let look = state
        .last_judged
        .is_none_or(|last| env.now.saturating_sub(last) >= EVERY_SECS);
    let judgments = match judge {
        Some(judge) if look => {
            let now = env.now;
            State::update(env.layout, &rule.id, |state| state.last_judged = Some(now));
            judged::consider(env, rule, judge).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    let mut steps = Vec::new();
    let proposed = judgments.iter().filter(|j| j.proposed).count();
    if proposed > 0 {
        steps.push(Step {
            kind: "judge".into(),
            target: None,
            outcome: StepOutcome::Done,
            detail: format!(
                "{proposed} folder{} may be caches; confirm with openagents background proposals.",
                if proposed == 1 { "" } else { "s" }
            ),
        });
    }
    let mut escalated = false;
    let last = state.last_escalation;
    if let (Some(escalate), Some(services)) = (&rule.escalate, services)
        && last.is_none_or(|last| env.now.saturating_sub(last) >= EVERY_SECS)
    {
        let run = CoderRun {
            title: format!("Background rule {} fell short", rule.id),
            prompt: format!(
                "{}\n\n{}",
                prompt(rule),
                briefing(env, rule, report, &judgments)
            ),
            workspace: escalate
                .workspace
                .as_ref()
                .map(|w| expand(w, &env.layout.home).display().to_string()),
            chat: None,
        };
        let started = services.start_coder_run(&run);
        let now = env.now;
        State::update(env.layout, &rule.id, |state| {
            state.last_escalation = Some(now)
        });
        steps.push(match started {
            Ok(task) => {
                escalated = true;
                Step {
                    kind: "escalate".into(),
                    target: Some(task.clone()),
                    outcome: StepOutcome::Done,
                    detail: format!(
                        "Asked Coder ({}) to propose rule changes.",
                        task.get(..12).unwrap_or(&task)
                    ),
                }
            }
            Err(why) => Step {
                kind: "escalate".into(),
                target: None,
                outcome: StepOutcome::Failed,
                detail: why,
            },
        });
    }
    if judgments.is_empty() && steps.is_empty() {
        return None;
    }
    let mut record = Record::empty(&crate::run::new_id(env.now), &rule.id);
    record.rule_version = rule.version;
    record.rule_digest = rule.digest();
    record.trigger = report
        .record
        .as_ref()
        .map_or(crate::run::Cause::Manual, |r| r.trigger);
    record.started = env.now;
    record.ended = crate::paths::now();
    record.escalated = escalated;
    record.judgments = judgments;
    let said: Vec<String> = steps
        .iter()
        .filter(|s| s.outcome == StepOutcome::Done)
        .map(|s| s.detail.clone())
        .collect();
    record.notified = (!said.is_empty()).then(|| said.join(" "));
    record.steps = steps;
    store::append(env.layout, &record);
    Some(record)
}
