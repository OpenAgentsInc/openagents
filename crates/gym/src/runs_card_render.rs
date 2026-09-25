//! The run card as rows, as one Markdown page, and as a diff of two cards;
//! and `gym runs characterize`, the command that prints them.
//!
//! Every number on the card is a row with a stable ID, such as
//! `session.1.model_share` or `provenance.suspect_line_hits`. The JSON
//! record carries the rows beside the typed sections, `coder-one ask` cites
//! them, and the diff mode compares two cards row by row. A row whose
//! record is missing has a `null` value and says why: `unknown`, or `not
//! recorded` for a record no policy writes yet.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::runs::{Agent, Catalog, Outcome, Run, Sources, read_json};
use crate::runs_analysis::{span, usd};
use crate::runs_card::{Card, Options, SCHEMA, characterize};

/// One number or fact on the card.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    pub id: String,
    pub label: String,
    /// A number, a string, a Boolean, or `null` when unknown.
    pub value: Value,
    /// The value as the card prints it.
    pub text: String,
}

const UNKNOWN: &str = "unknown";
const NOT_RECORDED: &str = "not recorded";

fn seconds(ms: u64) -> f64 {
    (ms as f64 / 100.0).round() / 10.0
}

fn percent(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

fn round(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}

fn ratio((hits, total): (usize, usize)) -> (Value, String) {
    (json!([hits, total]), format!("{hits} of {total}"))
}

struct Rows(Vec<Row>);

impl Rows {
    fn push(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        value: Value,
        text: String,
    ) {
        self.0.push(Row {
            id: id.into(),
            label: label.into(),
            value,
            text,
        });
    }

    fn ms(&mut self, id: impl Into<String>, label: &str, ms: Option<u64>) {
        match ms {
            Some(ms) => self.push(id, label, json!(seconds(ms)), span(ms)),
            None => self.push(id, label, Value::Null, UNKNOWN.to_owned()),
        }
    }

    fn share(&mut self, id: impl Into<String>, label: &str, share: Option<f64>) {
        match share {
            Some(share) => self.push(id, label, json!(round(share, 3)), percent(share)),
            None => self.push(id, label, Value::Null, UNKNOWN.to_owned()),
        }
    }

    fn count(&mut self, id: impl Into<String>, label: &str, count: usize) {
        self.push(id, label, json!(count), count.to_string());
    }

    fn ratio(
        &mut self,
        id: impl Into<String>,
        label: &str,
        value: Option<(usize, usize)>,
        missing: &str,
    ) {
        match value {
            Some(pair) => {
                let (value, text) = ratio(pair);
                self.push(id, label, value, text);
            }
            None => self.push(id, label, Value::Null, missing.to_owned()),
        }
    }

    fn text(&mut self, id: impl Into<String>, label: &str, value: Option<String>) {
        match value {
            Some(text) => self.push(id, label, json!(text), text),
            None => self.push(id, label, Value::Null, UNKNOWN.to_owned()),
        }
    }
}

fn score_text(score: Option<(u64, u64)>) -> String {
    score.map_or_else(|| UNKNOWN.to_owned(), |(p, t)| format!("{p} of {t}"))
}

/// Every row on the card, in the order the page shows them.
#[must_use]
pub fn rows(card: &Card) -> Vec<Row> {
    let mut rows = Rows(Vec::new());
    let id = &card.identity;
    rows.text("identity.task", "Task", Some(id.task.clone()));
    rows.text("identity.revision", "Task revision", id.revision.clone());
    rows.text("identity.policy", "Policy", id.policy.clone());
    rows.text(
        "identity.policy_digest",
        "Policy digest",
        id.policy_digest.clone(),
    );
    rows.text("identity.binary", "Binary", id.binary.clone());
    rows.text("identity.arm", "Arm", id.arm.clone());
    rows.text(
        "identity.attempt",
        "Attempt",
        id.attempt_id
            .as_ref()
            .map(|attempt| match &id.attempt_kind {
                Some(kind) => format!("{attempt} ({kind})"),
                None => attempt.clone(),
            }),
    );
    match id.reward {
        Some(reward) => rows.push(
            "identity.reward",
            "Reward",
            json!(reward),
            format!("{reward}"),
        ),
        None => rows.push("identity.reward", "Reward", Value::Null, UNKNOWN.to_owned()),
    }
    match (id.tests_passed, id.tests_total) {
        (Some(p), Some(t)) => rows.push(
            "identity.tests",
            "Verifier tests",
            json!([p, t]),
            format!("{p} of {t}"),
        ),
        _ => rows.push(
            "identity.tests",
            "Verifier tests",
            Value::Null,
            UNKNOWN.to_owned(),
        ),
    }
    match id.cost_usd {
        Some(cost) => rows.push(
            "identity.cost_usd",
            "Cost",
            json!(round(cost, 6)),
            format!(
                "{}{}",
                usd(cost),
                if id.cost_estimated {
                    " (list-price estimate)"
                } else {
                    ""
                }
            ),
        ),
        None => rows.push("identity.cost_usd", "Cost", Value::Null, UNKNOWN.to_owned()),
    }
    rows.ms("identity.trial_s", "Trial time", id.trial_ms);
    rows.ms("identity.agent_s", "Agent time", id.agent_ms);
    match id.development {
        Some(yes) => rows.push(
            "identity.development",
            "In the policy's development set",
            json!(yes),
            format!(
                "{}{}",
                if yes { "yes" } else { "no" },
                id.development_source
                    .as_ref()
                    .map(|s| format!(" ({s})"))
                    .unwrap_or_default()
            ),
        ),
        None => rows.push(
            "identity.development",
            "In the policy's development set",
            Value::Null,
            "unknown: no pins name the policy".to_owned(),
        ),
    }
    for phase in &card.phases {
        rows.ms(
            format!("phase.{}.s", phase.key),
            &phase.name,
            phase.duration_ms,
        );
        rows.share(
            format!("phase.{}.share", phase.key),
            &format!("{} share of trial time", phase.name),
            phase.share,
        );
    }
    for s in &card.sessions {
        let n = s.number;
        let p = |name: &str| format!("session.{n}.{name}");
        rows.text(p("role"), "Role", Some(s.role.clone()));
        rows.count(p("turns"), "Turns", s.turns);
        rows.count(p("calls"), "Calls", s.calls);
        rows.ms(p("s"), "Session time", Some(s.duration_ms));
        rows.ms(p("model_s"), "Model latency", Some(s.model_ms));
        rows.share(p("model_share"), "Model latency share", s.model_share);
        rows.ms(p("command_s"), "Command time", Some(s.command_ms));
        rows.share(p("command_share"), "Command time share", s.command_share);
        rows.ms(p("overhead_s"), "Tool overhead", Some(s.overhead_ms));
        rows.count(p("input_tokens"), "Input tokens", s.input_tokens as usize);
        rows.share(p("cached_share"), "Cached share of input", s.cached_share);
        rows.count(
            p("output_tokens"),
            "Output tokens",
            s.output_tokens as usize,
        );
        match s.cost_usd {
            Some(cost) => rows.push(p("cost_usd"), "Cost", json!(round(cost, 6)), usd(cost)),
            None => rows.push(p("cost_usd"), "Cost", Value::Null, UNKNOWN.to_owned()),
        }
        rows.ms(p("first_read_s"), "First read", s.moments.first_read_ms);
        rows.ms(
            p("first_command_s"),
            "First command",
            s.moments.first_command_ms,
        );
        rows.ms(p("first_edit_s"), "First edit", s.moments.first_edit_ms);
        rows.ms(p("last_edit_s"), "Last edit", s.moments.last_edit_ms);
        rows.ms(p("finish_s"), "Finish", s.moments.finish_ms);
        match &s.tail {
            Some(tail) => {
                rows.ms(
                    p("tail_s"),
                    "Verification tail, last edit to finish",
                    tail.to_finish_ms,
                );
                rows.count(p("tail_turns"), "Verification tail turns", tail.turns);
                rows.count(p("tail_runs"), "Program runs in the tail", tail.runs);
            }
            None => {
                rows.push(
                    p("tail_s"),
                    "Verification tail, last edit to finish",
                    Value::Null,
                    "no edit".to_owned(),
                );
                rows.push(
                    p("tail_turns"),
                    "Verification tail turns",
                    Value::Null,
                    "no edit".to_owned(),
                );
                rows.push(
                    p("tail_runs"),
                    "Program runs in the tail",
                    Value::Null,
                    "no edit".to_owned(),
                );
            }
        }
        rows.count(p("edit_rounds"), "Edit rounds", s.edit_rounds.len());
        rows.count(
            p("edit_rounds_checked"),
            "Edit rounds followed by a program run",
            s.edit_rounds.iter().filter(|r| r.checked).count(),
        );
        rows.text(
            p("finish"),
            "Finish status",
            Some(
                s.finish_status
                    .clone()
                    .unwrap_or_else(|| "no finish".to_owned()),
            ),
        );
    }
    if !card.sessions.is_empty() {
        let turns: usize = card.sessions.iter().map(|s| s.turns).sum();
        let input: u64 = card.sessions.iter().map(|s| s.input_tokens).sum();
        let cached: u64 = card.sessions.iter().map(|s| s.cached_tokens).sum();
        let cost: Option<f64> = card.sessions.iter().map(|s| s.cost_usd).sum();
        rows.count("sessions.turns", "Turns over all sessions", turns);
        rows.share(
            "sessions.cached_share",
            "Cached share of session input",
            (input > 0).then(|| cached as f64 / input as f64),
        );
        match cost {
            Some(cost) => rows.push(
                "sessions.cost_usd",
                "Session cost",
                json!(round(cost, 6)),
                usd(cost),
            ),
            None => rows.push(
                "sessions.cost_usd",
                "Session cost",
                Value::Null,
                UNKNOWN.to_owned(),
            ),
        }
    }
    let prov = &card.provenance;
    let no_suspects = if card.sessions.is_empty() {
        UNKNOWN
    } else {
        "no suspects recorded"
    };
    rows.count("provenance.suspects", "Suspects", prov.suspects.len());
    rows.ratio(
        "provenance.suspect_line_hits",
        "Suspects whose line the submitted workspace changed",
        prov.suspect_line_hits,
        if prov.suspects.is_empty() {
            no_suspects
        } else {
            "unknown: no untouched text"
        },
    );
    rows.ratio(
        "provenance.suspect_file_hits",
        "Suspects that named an edited file",
        prov.suspect_file_hits,
        no_suspects,
    );
    rows.ratio(
        "provenance.pointer_coverage",
        "Edited files a suspect named",
        prov.pointer_coverage,
        if card.sessions.is_empty() {
            UNKNOWN
        } else {
            "no suspects or no edits"
        },
    );
    rows.ratio(
        "provenance.briefing_coverage",
        "Edited files any briefing item named",
        prov.briefing_coverage,
        UNKNOWN,
    );
    let sites = (!prov.sites.is_empty()).then_some(&prov.sites);
    rows.ratio(
        "provenance.defect_sites_named",
        "Defect sites a suspect named",
        sites.map(|s| (s.iter().filter(|x| x.named).count(), s.len())),
        "unknown: no defect-site record for the task",
    );
    rows.ratio(
        "provenance.defect_sites_edited",
        "Defect sites edited",
        sites.map(|s| (s.iter().filter(|x| x.edited).count(), s.len())),
        "unknown: no defect-site record for the task",
    );
    let checks = &card.checks;
    if checks.path.is_some() {
        rows.count("checks.versions", "Check versions", checks.versions.len());
        rows.count(
            "checks.rewrites_after_edit",
            "Check rewrites after a code edit",
            checks.versions.iter().filter(|v| v.after_code_edit).count(),
        );
        rows.text(
            "checks.untouched_score",
            "Check score on the untouched workspace",
            checks
                .versions
                .iter()
                .find_map(|v| v.untouched_score)
                .map(|s| score_text(Some(s))),
        );
        rows.count("checks.runs", "Check runs in sessions", checks.runs.len());
    } else {
        for (id, label) in [
            ("checks.versions", "Check versions"),
            (
                "checks.rewrites_after_edit",
                "Check rewrites after a code edit",
            ),
            (
                "checks.untouched_score",
                "Check score on the untouched workspace",
            ),
            ("checks.runs", "Check runs in sessions"),
        ] {
            rows.push(
                id,
                label,
                Value::Null,
                "unknown: no session-written check".to_owned(),
            );
        }
    }
    rows.text(
        "checks.frozen_score",
        "Host's final score on the check",
        card.claims.final_score.map(|s| score_text(Some(s))),
    );
    match &checks.grades {
        Some(grades) => {
            let lines = grades["lines"].as_array().map_or(0, Vec::len);
            let advisory = grades["lines"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|line| line["grade"] == "advisory")
                .count();
            rows.push(
                "checks.line_grades",
                "Check lines graded, advisory",
                json!([lines, advisory]),
                format!("{lines} lines, {advisory} advisory"),
            );
        }
        None => rows.push(
            "checks.line_grades",
            "Check lines graded, advisory",
            Value::Null,
            NOT_RECORDED.to_owned(),
        ),
    }
    let exec = &card.executed;
    rows.count(
        "executed.host_operations",
        "Host operations",
        exec.host_operations.len(),
    );
    match &exec.host_commands {
        Some(commands) => rows.count(
            "executed.host_commands",
            "Host-executed commands",
            commands.len(),
        ),
        None => rows.push(
            "executed.host_commands",
            "Host-executed commands",
            Value::Null,
            NOT_RECORDED.to_owned(),
        ),
    }
    let session_steps: usize = exec.session_phases.values().sum();
    rows.count("executed.session_steps", "Session steps", session_steps);
    rows.count(
        "executed.unplaced",
        "Session steps no rule or cached Jev answer placed",
        exec.session_phases.get("unplaced").copied().unwrap_or(0),
    );
    let waste = &card.waste;
    let has_sessions = !card.sessions.is_empty();
    let waste_count = |rows: &mut Rows, id: &str, label: &str, count: usize| {
        if has_sessions {
            rows.count(id, label, count);
        } else {
            rows.push(id, label, Value::Null, UNKNOWN.to_owned());
        }
    };
    waste_count(
        &mut rows,
        "waste.not_found_turns",
        "Turns lost to a missing program",
        waste.not_found_turns(),
    );
    for missing in &waste.not_found {
        rows.push(
            format!("waste.not_found.{}", missing.program),
            format!("Turns lost to `{}`", missing.program),
            json!(missing.turns.len()),
            format!("{} ({})", missing.turns.len(), missing.turns.join(", ")),
        );
    }
    waste_count(
        &mut rows,
        "waste.refused",
        "Refused tool calls",
        waste.refused.len(),
    );
    waste_count(
        &mut rows,
        "waste.briefing_rereads",
        "Reads of files the briefing carried in full",
        waste.briefing_rereads,
    );
    waste_count(
        &mut rows,
        "waste.turns_without_call",
        "Turns with no call",
        waste.turns_without_call,
    );
    waste_count(
        &mut rows,
        "waste.runs_after_full",
        "Program runs after the score was full",
        waste.runs_after_full,
    );
    if has_sessions {
        rows.ms(
            "waste.slow_s",
            "Time in commands over 5 seconds",
            Some(waste.slow_ms),
        );
    } else {
        rows.ms("waste.slow_s", "Time in commands over 5 seconds", None);
    }
    let rev = &card.reversals;
    match &rev.between_sessions {
        Some(found) => rows.count(
            "reversals.between_sessions",
            "Files whose digest returned between sessions",
            found.len(),
        ),
        None => rows.push(
            "reversals.between_sessions",
            "Files whose digest returned between sessions",
            Value::Null,
            "unknown: no snapshots".to_owned(),
        ),
    }
    rows.count(
        "reversals.patches",
        "Patch-level reversals between sessions",
        rev.patches.len(),
    );
    rows.push(
        "reversals.within_sessions",
        "Files whose digest returned within a session",
        Value::Null,
        "not recorded: no per-edit digests".to_owned(),
    );
    for delta in &card.review {
        rows.push(
            format!("review.{}.files", delta.session),
            format!("Files the {} changed", delta.role),
            json!(delta.files.len()),
            delta.files.len().to_string(),
        );
        rows.push(
            format!("review.{}.change", delta.session),
            format!("What the {} changed", delta.role),
            json!(delta.words()),
            delta.words(),
        );
        let behavior = delta.files.len() == delta.changes.len()
            && delta.changes.iter().all(|c| c.behavior_unchanged());
        rows.push(
            format!("review.{}.code_changed", delta.session),
            format!("Whether the {} changed code", delta.role),
            if delta.files.len() == delta.changes.len() {
                json!(!behavior)
            } else {
                Value::Null
            },
            if delta.files.len() != delta.changes.len() {
                UNKNOWN.to_owned()
            } else if behavior {
                "no".to_owned()
            } else {
                "yes".to_owned()
            },
        );
    }
    match &card.review_rule {
        Some(rule) => {
            rows.push(
                "review.rule.trigger",
                "What started the review",
                json!(rule.trigger),
                rule.trigger.clone(),
            );
            rows.push(
                "review.rule.ran",
                "Whether the review ran",
                json!(rule.review),
                if rule.review { "yes" } else { "no" }.to_owned(),
            );
            for (trigger, reading, detail) in &rule.readings {
                rows.push(
                    format!("review.rule.{trigger}"),
                    format!("Review trigger `{trigger}`"),
                    json!(reading),
                    format!("{reading}: {detail}"),
                );
            }
            match &rule.concerns {
                Some(concerns) => rows.count(
                    "review.rule.concerns",
                    "Concerns the review reported",
                    concerns.len(),
                ),
                None => rows.push(
                    "review.rule.concerns",
                    "Concerns the review reported",
                    Value::Null,
                    if rule.review {
                        NOT_RECORDED.to_owned()
                    } else {
                        "no review".to_owned()
                    },
                ),
            }
        }
        None => rows.push(
            "review.rule.trigger",
            "What started the review",
            Value::Null,
            "not recorded: the policy runs the review unconditionally".to_owned(),
        ),
    }
    let claims = &card.claims;
    match claims.self_score_agrees {
        Some(agrees) => rows.push(
            "claims.self_score_agrees",
            "Self-score agrees with the verifier",
            json!(agrees),
            if agrees { "yes" } else { "no" }.to_owned(),
        ),
        None => rows.push(
            "claims.self_score_agrees",
            "Self-score agrees with the verifier",
            Value::Null,
            UNKNOWN.to_owned(),
        ),
    }
    match claims.close_p {
        Some(p) => rows.push(
            "claims.close_p",
            "Jev's close probability",
            json!(p),
            format!("{p:.2}"),
        ),
        None => rows.push(
            "claims.close_p",
            "Jev's close probability",
            Value::Null,
            "not asked".to_owned(),
        ),
    }
    match &claims.lines {
        Some(lines) => {
            let agree = lines.iter().filter(|l| l["agrees"] == true).count();
            let known = lines.iter().filter(|l| l["agrees"].is_boolean()).count();
            rows.push(
                "claims.line_agreement",
                "Check lines that agree with the verifier",
                json!([agree, known]),
                format!("{agree} of {known}"),
            );
        }
        None => rows.push(
            "claims.line_agreement",
            "Check lines that agree with the verifier",
            Value::Null,
            NOT_RECORDED.to_owned(),
        ),
    }
    match &card.reference {
        Some(r) => {
            rows.push(
                "reference.id",
                "Reference trajectory",
                json!(r.id),
                format!("Fable 5.1 {} {}", r.effort, r.id),
            );
            rows.push(
                "reference.cost_usd",
                "Reference cost",
                json!(r.cost_usd),
                usd(r.cost_usd),
            );
            rows.push(
                "reference.s",
                "Reference time",
                json!(round(r.seconds, 1)),
                format!("{:.1} s", r.seconds),
            );
            rows.push(
                "reference.steps",
                "Reference steps",
                json!(r.steps),
                r.steps.to_string(),
            );
            rows.ms(
                "reference.first_edit_s",
                "Reference first edit",
                r.first_edit_ms,
            );
            match r.edit_runs {
                Some(runs) => rows.count("reference.edit_rounds", "Reference edit rounds", runs),
                None => rows.push(
                    "reference.edit_rounds",
                    "Reference edit rounds",
                    Value::Null,
                    UNKNOWN.to_owned(),
                ),
            }
        }
        None => rows.push(
            "reference.id",
            "Reference trajectory",
            Value::Null,
            "unknown: no passing public attempt".to_owned(),
        ),
    }
    rows.0
}

/// A few lines for `gym runs show`: where the time went, the first
/// session's anatomy, the pointers, the waste, and the review.
#[must_use]
pub fn summary_lines(card: &Card) -> Vec<String> {
    let all = rows(card);
    let t = |id: &str| {
        all.iter()
            .find(|r| r.id == id)
            .map_or_else(|| UNKNOWN.to_owned(), |r| r.text.clone())
    };
    let mut lines = Vec::new();
    let phases: Vec<String> = card
        .phases
        .iter()
        .filter(|p| p.share.is_some_and(|share| share >= 0.05))
        .map(|p| {
            format!(
                "{} {}",
                p.name.to_lowercase(),
                t(&format!("phase.{}.share", p.key))
            )
        })
        .collect();
    if !phases.is_empty() {
        lines.push(format!(
            "Trial time {}: {}.",
            t("identity.trial_s"),
            phases.join(", ")
        ));
    }
    for s in &card.sessions {
        let n = s.number;
        let st = |name: &str| t(&format!("session.{n}.{name}"));
        lines.push(format!(
            "Session {n} ({}): {} turns in {}, model {} of it, first edit at {}, {} after the last edit.",
            st("role"),
            st("turns"),
            st("s"),
            st("model_share"),
            st("first_edit_s"),
            st("tail_s"),
        ));
    }
    if !card.sessions.is_empty() {
        lines.push(format!(
            "Suspects named {} defect sites; their lines changed {}. Turns lost to missing programs: {}. Check lines graded: {}.",
            t("provenance.defect_sites_named"),
            t("provenance.suspect_line_hits"),
            t("waste.not_found_turns"),
            t("checks.line_grades"),
        ));
    }
    for delta in &card.review {
        lines.push(format!("The {} changed {}.", delta.role, delta.words()));
    }
    lines
}

/// The card's JSON record: the typed sections and the rows.
#[must_use]
pub fn card_json(card: &Card) -> Value {
    let mut value = serde_json::to_value(card).unwrap_or(Value::Null);
    value["rows"] = serde_json::to_value(rows(card)).unwrap_or(Value::Null);
    value
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// The card as one Markdown page.
#[must_use]
pub fn markdown(card: &Card) -> String {
    let all = rows(card);
    let by: BTreeMap<&str, &Row> = all.iter().map(|r| (r.id.as_str(), r)).collect();
    let t = |id: &str| {
        by.get(id)
            .map_or_else(|| UNKNOWN.to_owned(), |r| r.text.clone())
    };
    let id = &card.identity;
    let mut out = String::new();
    let mut line = |text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    line(format!("# Run card: {}", id.task));
    line(String::new());
    line(format!("`{}`", id.run));
    line(String::new());
    line(format!(
        "**Identity.** Policy {} (`{}`), binary {}, arm {}, attempt {}. Task revision {}; in the policy's development set: {}. Reward {} ({} tests), cost {}, trial {}, agent {}.",
        t("identity.policy"),
        id.policy_digest
            .as_deref()
            .map_or(UNKNOWN.to_owned(), |d| d.chars().take(12).collect()),
        t("identity.binary"),
        t("identity.arm"),
        t("identity.attempt"),
        t("identity.revision"),
        t("identity.development"),
        t("identity.reward"),
        t("identity.tests"),
        t("identity.cost_usd"),
        t("identity.trial_s"),
        t("identity.agent_s"),
    ));
    line(String::new());
    line("## Phase timeline".to_owned());
    line(String::new());
    line("| Phase | Start | Duration | Share |".to_owned());
    line("| --- | ---: | ---: | ---: |".to_owned());
    for phase in &card.phases {
        line(format!(
            "| {} | {} | {} | {} |",
            phase.name,
            phase.start_ms.map_or(UNKNOWN.to_owned(), |ms| format!(
                "{:+.1} s",
                ms as f64 / 1000.0
            )),
            t(&format!("phase.{}.s", phase.key)),
            t(&format!("phase.{}.share", phase.key)),
        ));
    }
    if !card.sessions.is_empty() {
        line(String::new());
        line("## Sessions".to_owned());
        line(String::new());
        let header: Vec<String> = card
            .sessions
            .iter()
            .map(|s| format!("{} `{}`", s.number, s.id))
            .collect();
        line(format!("| | {} |", header.join(" | ")));
        line(format!("| --- |{}", " ---: |".repeat(card.sessions.len())));
        let row = |label: &str, f: &dyn Fn(usize) -> String| {
            let cells: Vec<String> = card.sessions.iter().map(|s| cell(&f(s.number))).collect();
            format!("| {label} | {} |", cells.join(" | "))
        };
        let st = |n: usize, name: &str| t(&format!("session.{n}.{name}"));
        line(row("Role", &|n| st(n, "role")));
        line(row("Turns, calls", &|n| {
            format!("{}, {}", st(n, "turns"), st(n, "calls"))
        }));
        line(row("Time", &|n| st(n, "s")));
        line(row("Model latency", &|n| {
            format!("{} ({})", st(n, "model_s"), st(n, "model_share"))
        }));
        line(row("Commands", &|n| {
            format!("{} ({})", st(n, "command_s"), st(n, "command_share"))
        }));
        line(row("Tool overhead", &|n| st(n, "overhead_s")));
        line(row("Input tokens, cached", &|n| {
            format!("{}, {}", st(n, "input_tokens"), st(n, "cached_share"))
        }));
        line(row("Cost", &|n| st(n, "cost_usd")));
        line(row("First read, command, edit", &|n| {
            format!(
                "{}, {}, {}",
                st(n, "first_read_s"),
                st(n, "first_command_s"),
                st(n, "first_edit_s")
            )
        }));
        line(row("Last edit, finish", &|n| {
            format!("{}, {}", st(n, "last_edit_s"), st(n, "finish_s"))
        }));
        line(row("Tail after the last edit", &|n| {
            format!(
                "{}, {} turns, {} runs",
                st(n, "tail_s"),
                st(n, "tail_turns"),
                st(n, "tail_runs")
            )
        }));
        line(row("Edit rounds, checked", &|n| {
            format!("{}, {}", st(n, "edit_rounds"), st(n, "edit_rounds_checked"))
        }));
        line(row("Steps by phase", &|n| {
            card.sessions[n - 1]
                .phases
                .iter()
                .map(|(phase, count)| format!("{phase} {count}"))
                .collect::<Vec<_>>()
                .join(", ")
        }));
        line(row("Finish", &|n| st(n, "finish")));
        line(String::new());
        for s in &card.sessions {
            if let Some(summary) = &s.finish_summary {
                line(format!(
                    "- Session {} finish summary: {}",
                    s.number, summary
                ));
            }
        }
    }
    line(String::new());
    line("## Evidence provenance".to_owned());
    line(String::new());
    let prov = &card.provenance;
    line(format!(
        "Suspects whose line the submitted workspace changed: {}. Suspects that named an edited file: {}. Edited files a suspect named: {}; any briefing item: {}. Defect sites a suspect named: {}; edited: {}.",
        t("provenance.suspect_line_hits"),
        t("provenance.suspect_file_hits"),
        t("provenance.pointer_coverage"),
        t("provenance.briefing_coverage"),
        t("provenance.defect_sites_named"),
        t("provenance.defect_sites_edited"),
    ));
    if !prov.suspects.is_empty() {
        line(String::new());
        line("| Suspect | p | File edited | Line changed |".to_owned());
        line("| --- | ---: | --- | --- |".to_owned());
        for s in &prov.suspects {
            line(format!(
                "| `{}:{}` {} | {} | {} | {} |",
                s.file,
                s.line.map_or("?".to_owned(), |l| l.to_string()),
                cell(&crate::runs::clip_words(&s.text, 60)),
                s.p.map_or(UNKNOWN.to_owned(), |p| format!("{p:.2}")),
                if s.file_edited { "yes" } else { "no" },
                match s.line_changed {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => UNKNOWN,
                },
            ));
        }
    }
    if !prov.edited.is_empty() {
        line(String::new());
        line("| Edited file | Briefing file item | Suspects | Changed lines |".to_owned());
        line("| --- | --- | ---: | ---: |".to_owned());
        for f in &prov.edited {
            line(format!(
                "| `{}` | {} | {} | {} |",
                f.path,
                f.file_item.as_deref().unwrap_or("none"),
                f.suspects,
                f.changed_lines
                    .map_or(UNKNOWN.to_owned(), |n| n.to_string()),
            ));
        }
    }
    if !prov.sites.is_empty() {
        line(String::new());
        let named: Vec<String> = prov
            .sites
            .iter()
            .map(|s| {
                format!(
                    "{} `{}` ({}){}",
                    s.id,
                    s.file,
                    s.what,
                    if s.named { ", named by a suspect" } else { "" }
                )
            })
            .collect();
        line(format!(
            "Defect sites from {}: {}.",
            prov.sites_source.as_deref().unwrap_or("the task's record"),
            named.join("; ")
        ));
    }
    line(String::new());
    line("## Check lineage".to_owned());
    line(String::new());
    let checks = &card.checks;
    match &checks.path {
        Some(path) => {
            line(format!(
                "The session-written check is `{path}`: {} versions, {} rewritten after a code edit; score on the untouched workspace {}; the host's final score {}. Line grades: {}.",
                t("checks.versions"),
                t("checks.rewrites_after_edit"),
                t("checks.untouched_score"),
                t("checks.frozen_score"),
                t("checks.line_grades"),
            ));
            for v in &checks.versions {
                line(format!(
                    "- Session {} turn {} at {}: {}{}{}{}",
                    v.session,
                    v.turn,
                    span(v.at_ms),
                    v.how,
                    if v.after_code_edit {
                        ", after a code edit"
                    } else {
                        ""
                    },
                    if v.after_failing_score {
                        ", after a failing score"
                    } else {
                        ""
                    },
                    v.untouched_score
                        .map(|s| format!(", untouched score {}", score_text(Some(s))))
                        .unwrap_or_default(),
                ));
            }
            if !checks.runs.is_empty() {
                let runs: Vec<String> = checks
                    .runs
                    .iter()
                    .map(|r| format!("S{} T{} {}/{}", r.session, r.turn, r.passed, r.total))
                    .collect();
                line(format!("- Runs: {}", runs.join(", ")));
            }
        }
        None => line("No session-written check was found in the briefing.".to_owned()),
    }
    for f in &checks.frozen {
        line(format!(
            "- Host after session {}{}: score {}, {}, hard-coded p {}",
            f.after_session,
            if f.self_check { " (self-check)" } else { "" },
            score_text(f.score),
            match f.kept {
                Some(true) => "kept",
                Some(false) => "not kept",
                None => "kept unknown",
            },
            f.hardcoded_p
                .map_or(UNKNOWN.to_owned(), |p| format!("{p:.2}")),
        ));
    }
    line(String::new());
    line("## Executed evidence".to_owned());
    line(String::new());
    let exec = &card.executed;
    let ops: Vec<String> = exec
        .host_operations
        .iter()
        .map(|op| {
            format!(
                "{} (exit {}{})",
                op.label,
                op.exit.map_or(UNKNOWN.to_owned(), |e| e.to_string()),
                op.output_sha256
                    .as_ref()
                    .map(|d| format!(", output `{}`", &d[..d.len().min(12)]))
                    .unwrap_or_default()
            )
        })
        .collect();
    line(format!(
        "Host operations: {}. Host-executed commands: {}. Session steps by phase: {} ({}).",
        if ops.is_empty() {
            "none recorded".to_owned()
        } else {
            ops.join("; ")
        },
        t("executed.host_commands"),
        exec.session_phases
            .iter()
            .map(|(p, c)| format!("{p} {c}"))
            .collect::<Vec<_>>()
            .join(", "),
        exec.placement,
    ));
    line(String::new());
    line("## Waste".to_owned());
    line(String::new());
    let waste = &card.waste;
    line(format!(
        "- Turns lost to a missing program: {}",
        t("waste.not_found_turns")
    ));
    for m in &waste.not_found {
        line(format!(
            "  - `{}`: session.turn {}",
            m.program,
            m.turns.join(", ")
        ));
    }
    line(format!("- Refused tool calls: {}", t("waste.refused")));
    for r in &waste.refused {
        line(format!(
            "  - S{} T{} `{}`: {}",
            r.session, r.turn, r.tool, r.cause
        ));
    }
    line(format!(
        "- Reads of files the briefing carried in full: {}{}",
        t("waste.briefing_rereads"),
        if waste.briefing_reread_paths.is_empty() {
            String::new()
        } else {
            format!(" ({})", waste.briefing_reread_paths.join(", "))
        }
    ));
    line(format!(
        "- Turns with no call: {}",
        t("waste.turns_without_call")
    ));
    line(format!(
        "- Program runs after the score was full: {}",
        t("waste.runs_after_full")
    ));
    line(format!(
        "- Time in commands over 5 seconds: {}",
        t("waste.slow_s")
    ));
    for s in waste.slow.iter().take(6) {
        line(format!(
            "  - {} × `{}`: {}",
            s.runs,
            cell(&s.command),
            span(s.ms)
        ));
    }
    line(String::new());
    line("## Reversals".to_owned());
    line(String::new());
    line(format!(
        "Between sessions, by digest: {}. By patch: {}. Within sessions: {}.",
        t("reversals.between_sessions"),
        t("reversals.patches"),
        t("reversals.within_sessions"),
    ));
    for r in card.reversals.between_sessions.iter().flatten() {
        line(format!(
            "- `{}` after session {} returned to its digest after session {} (`{}`)",
            r.file, r.at_session, r.matches_session, r.digest
        ));
    }
    for r in &card.reversals.patches {
        line(format!(
            "- `{}`: {} undid {} lines {} added ({})",
            r.file, r.later, r.undone, r.earlier, r.kind
        ));
    }
    line(String::new());
    line("## Review delta".to_owned());
    line(String::new());
    if card.review.is_empty() {
        line("No review or self-check session with snapshots on both sides.".to_owned());
    }
    for delta in &card.review {
        line(format!(
            "- Session {} ({}): {}. Score {} before, {} after. Executed checks before and after: {}.",
            delta.session,
            delta.role,
            delta.words(),
            score_text(delta.score_before),
            score_text(delta.score_after),
            match (&delta.executed_before, &delta.executed_after) {
                (Some(b), Some(a)) => format!("{} and {} records", b.len(), a.len()),
                _ => NOT_RECORDED.to_owned(),
            }
        ));
        for (path, before, after) in &delta.files {
            let short = |d: &Option<String>| {
                d.as_deref()
                    .map_or("absent".to_owned(), |d| d.chars().take(12).collect())
            };
            line(format!(
                "  - `{path}`: `{}` to `{}`",
                short(before),
                short(after)
            ));
        }
    }
    match &card.review_rule {
        Some(rule) => {
            line(format!(
                "- Review rule on session {}'s candidate: {}. Trigger: {}.",
                rule.session
                    .map_or_else(|| UNKNOWN.to_owned(), |n| n.to_string()),
                rule.reason,
                rule.trigger
            ));
            for (trigger, reading, detail) in &rule.readings {
                line(format!("  - `{trigger}`: {reading}, {detail}"));
            }
            for concern in rule.concerns.iter().flatten() {
                line(format!(
                    "  - Concern{}: {}{}",
                    concern["requirement"]
                        .as_str()
                        .map_or_else(String::new, |id| format!(" on {id}")),
                    concern["concern"].as_str().unwrap_or_default(),
                    concern["command"]
                        .as_str()
                        .map_or_else(String::new, |c| format!(" (`{c}`)"))
                ));
            }
        }
        None => line(
            "- Review rule: not recorded; the policy runs the review unconditionally.".to_owned(),
        ),
    }
    line(String::new());
    line("## Claims against outcomes".to_owned());
    line(String::new());
    for s in &card.sessions {
        line(format!(
            "- Session {} finished {}.",
            s.number,
            s.finish_status
                .as_deref()
                .unwrap_or("without a finish call")
        ));
    }
    line(format!(
        "- Verifier: reward {}, tests {}. The host's final score {}; agrees with the verifier: {}. Jev's close probability {} beside reward {}. Check lines against the verifier: {}.",
        t("identity.reward"),
        t("identity.tests"),
        t("checks.frozen_score"),
        t("claims.self_score_agrees"),
        t("claims.close_p"),
        t("identity.reward"),
        t("claims.line_agreement"),
    ));
    line(String::new());
    line("## Against the reference".to_owned());
    line(String::new());
    match &card.reference {
        Some(r) => {
            let turns: usize = card.sessions.iter().map(|s| s.turns).sum();
            line(format!(
                "Fable 5.1 {} ({} of {} public attempts passed): {} steps, {:.1} s, {}; first edit {}, edit rounds {}{}. This run: {} turns, {}, {}.",
                r.effort,
                r.passes,
                r.attempts,
                r.steps,
                r.seconds,
                usd(r.cost_usd),
                t("reference.first_edit_s"),
                t("reference.edit_rounds"),
                r.sequence
                    .as_ref()
                    .map(|s| format!(", phases `{s}`"))
                    .unwrap_or_default(),
                turns,
                t("identity.trial_s"),
                t("identity.cost_usd"),
            ));
        }
        None => line("No passing public attempt on this task.".to_owned()),
    }
    if !card.notes.is_empty() {
        line(String::new());
        for note in &card.notes {
            line(format!("- {note}"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

/// One row compared across two cards.
#[derive(Clone, Debug, Serialize)]
pub struct Delta {
    pub id: String,
    pub label: String,
    pub a: Value,
    pub b: Value,
    pub a_text: String,
    pub b_text: String,
    /// `b - a` for numbers and number pairs' first members over their
    /// second, or `None` when the values aren't comparable numbers.
    pub delta: Option<f64>,
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        let pair = value.as_array()?;
        let (hits, total) = (pair.first()?.as_f64()?, pair.get(1)?.as_f64()?);
        (total > 0.0).then(|| hits / total)
    })
}

/// Compares two cards' rows. Every row in either card is listed.
#[must_use]
pub fn diff(a: &[Row], b: &[Row]) -> Vec<Delta> {
    let mut ids: Vec<&str> = a.iter().map(|r| r.id.as_str()).collect();
    for row in b {
        if !ids.contains(&row.id.as_str()) {
            ids.push(&row.id);
        }
    }
    ids.into_iter()
        .map(|id| {
            let left = a.iter().find(|r| r.id == id);
            let right = b.iter().find(|r| r.id == id);
            let value = |row: Option<&Row>| row.map_or(Value::Null, |r| r.value.clone());
            let shown =
                |row: Option<&Row>| row.map_or_else(|| "absent".to_owned(), |r| r.text.clone());
            let (av, bv) = (value(left), value(right));
            Delta {
                id: id.to_owned(),
                label: left.or(right).map(|r| r.label.clone()).unwrap_or_default(),
                delta: number(&av).zip(number(&bv)).map(|(x, y)| round(y - x, 4)),
                a: av,
                b: bv,
                a_text: shown(left),
                b_text: shown(right),
            }
        })
        .collect()
}

/// The rows of a card JSON record, or of a card computed from it.
fn rows_of(value: &Value) -> Result<(String, String, Vec<Row>), String> {
    if value["schema"] != SCHEMA {
        return Err("not a run card record".to_owned());
    }
    let rows: Vec<Row> = value["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            Some(Row {
                id: row["id"].as_str()?.to_owned(),
                label: row["label"].as_str().unwrap_or_default().to_owned(),
                value: row["value"].clone(),
                text: row["text"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect();
    Ok((
        value
            .pointer("/identity/run")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        value
            .pointer("/identity/task")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        rows,
    ))
}

// ---------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------

const USAGE: &str = "\
gym runs characterize: one trial's run card, computed from its retained records.

Usage:
  gym runs characterize RUN [--json] [--out DIR] [--no-jev] [--no-reference]
  gym runs characterize --all [--out DIR] [--no-jev] [--no-reference]
  gym runs characterize diff A B [--json] [--changed]

RUN is a job name, job/trial, a trial name, or a piece of a job name that
only one job has. The card covers the run's identity, the phase timeline,
each session's anatomy, which briefing items pointed at the edited code,
the session-written check's versions, what the host and the sessions ran,
waste, reversals, what a review session changed, each session's claim
beside the verifier, and the cheapest passing public trajectory on the
task. A number whose record is missing is unknown; a record no policy
writes yet is not recorded.

It reads the trial directory, the experiments' pins, the task-level
defect-site record, and the public-attempt manifest. Jev isn't asked:
--no-jev also skips the fingerprint store's cached phase answers, and
--no-reference skips loading the reference trajectory's body.

It prints Markdown, or JSON with --json. --out DIR writes
JOB--TRIAL.card.md and JOB--TRIAL.card.json there. --all characterizes
every Coder One run and prints one line each; a run that can't be
characterized makes the command exit 1.

diff compares two cards on the same task, row by row: A and B are runs or
card JSON files. --changed lists only the rows that differ.

--jobs-dir PATH and --traces-dir PATH read other directories; --no-jobs and
--no-traces skip one; --sites PATH reads another defect-site record.";

/// Whether `gym runs WORD` is this command.
#[must_use]
pub fn handles(word: &str) -> bool {
    word == "characterize"
}

fn write_card(dir: &Path, card: &Card) -> Result<(PathBuf, PathBuf), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let stem = card.identity.run.replace('/', "--");
    let md = dir.join(format!("{stem}.card.md"));
    let js = dir.join(format!("{stem}.card.json"));
    std::fs::write(&md, markdown(card))
        .map_err(|e| format!("cannot write {}: {e}", md.display()))?;
    let text = serde_json::to_string_pretty(&card_json(card)).map_err(|e| e.to_string())?;
    std::fs::write(&js, text + "\n").map_err(|e| format!("cannot write {}: {e}", js.display()))?;
    Ok((md, js))
}

/// The card for `name`: a card JSON file, or a run in the catalog.
fn card_value(name: &str, catalog: &Catalog, options: &Options) -> Result<Value, String> {
    let path = Path::new(name);
    if path.is_file() {
        return read_json(path).ok_or_else(|| format!("{name} isn't JSON"));
    }
    let run = catalog
        .find(name)
        .ok_or_else(|| format!("no run matches {name}"))?;
    Ok(card_json(&characterize(run, options)))
}

/// Runs `gym runs characterize`.
///
/// # Errors
///
/// Returns a message for bad arguments, a run that can't be found, or a
/// file that can't be written.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let args = &args[usize::from(args.first().is_some_and(|w| handles(w)))..];
    let mut sources = Sources::standard();
    let mut options = Options::standard();
    let (mut json_out, mut all, mut changed_only) = (false, false, false);
    let mut out_dir: Option<PathBuf> = None;
    let mut names: Vec<String> = Vec::new();
    let mut diffing = false;
    let mut index = 0;
    let value = |index: usize| args.get(index + 1).cloned().ok_or_else(|| USAGE.to_owned());
    while index < args.len() {
        match args[index].as_str() {
            "diff" if index == 0 => diffing = true,
            "--json" => json_out = true,
            "--all" => all = true,
            "--changed" => changed_only = true,
            "--no-jev" => options.store = None,
            "--no-reference" => options.reference_body = false,
            "--out" => {
                out_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--sites" => {
                let path = PathBuf::from(value(index)?);
                options.sites = Some(
                    read_json(&path).ok_or_else(|| format!("cannot read {}", path.display()))?,
                );
                index += 1;
            }
            "--jobs-dir" => {
                sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-jobs" => sources.jobs = None,
            "--no-traces" => sources.traces = None,
            "--help" | "-h" => {
                writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            other if !other.starts_with("--") => names.push(other.to_owned()),
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    sources.tasks.clear();
    let catalog = Catalog::load(sources);
    if diffing {
        let [a, b] = names.as_slice() else {
            return Err(format!("diff takes two runs or card files\n\n{USAGE}"));
        };
        let (a_run, a_task, a_rows) = rows_of(&card_value(a, &catalog, &options)?)?;
        let (b_run, b_task, b_rows) = rows_of(&card_value(b, &catalog, &options)?)?;
        if a_task != b_task {
            return Err(format!(
                "the cards are on different tasks, {a_task} and {b_task}; diff compares cards on one task"
            ));
        }
        let mut deltas = diff(&a_rows, &b_rows);
        if changed_only {
            deltas.retain(|d| d.a != d.b);
        }
        if json_out {
            let value = json!({
                "schema": "openagents.gym.run-card-diff.v1",
                "task": a_task,
                "a": a_run,
                "b": b_run,
                "rows": deltas,
            });
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
            return Ok(0);
        }
        write(out, &format!("# Run card diff: {a_task}"))?;
        write(out, "")?;
        write(out, &format!("A is `{a_run}`; B is `{b_run}`."))?;
        write(out, "")?;
        write(out, "| Row | A | B | B − A |")?;
        write(out, "| --- | ---: | ---: | ---: |")?;
        for d in deltas {
            write(
                out,
                &format!(
                    "| {} `{}` | {} | {} | {} |",
                    cell(&d.label),
                    d.id,
                    cell(&d.a_text),
                    cell(&d.b_text),
                    d.delta.map_or(
                        if d.a == d.b { "same" } else { "changed" }.to_owned(),
                        |x| format!("{x:+}")
                    ),
                ),
            )?;
        }
        return Ok(0);
    }
    if all {
        let mut failed = 0;
        let mut count = 0;
        for run in catalog
            .runs
            .iter()
            .filter(|run| run.agent == Agent::CoderOne && run.outcome != Outcome::Running)
        {
            count += 1;
            let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                characterize(run, &options)
            }));
            match made {
                Ok(card) => {
                    let rows = rows(&card);
                    let unknown = rows.iter().filter(|r| r.value.is_null()).count();
                    if let Some(dir) = &out_dir {
                        write_card(dir, &card)?;
                    }
                    write(
                        out,
                        &format!(
                            "ok      {:<90} {} sessions, {} rows, {} unknown or not recorded",
                            run.id(),
                            card.sessions.len(),
                            rows.len(),
                            unknown
                        ),
                    )?;
                }
                Err(_) => {
                    failed += 1;
                    write(out, &format!("failed  {}", run.id()))?;
                }
            }
        }
        write(out, &format!("{count} runs, {failed} failed"))?;
        return Ok(i32::from(failed > 0));
    }
    let [name] = names.as_slice() else {
        return Err(USAGE.to_owned());
    };
    let run: &Run = catalog
        .find(name)
        .ok_or_else(|| format!("no run matches {name}"))?;
    if run.outcome == Outcome::Running {
        return Err(format!(
            "{} is still running; characterize it when it ends",
            run.id()
        ));
    }
    let card = characterize(run, &options);
    if let Some(dir) = &out_dir {
        let (md, js) = write_card(dir, &card)?;
        write(out, &format!("Wrote {}", md.display()))?;
        write(out, &format!("Wrote {}", js.display()))?;
        return Ok(0);
    }
    if json_out {
        write(
            out,
            &serde_json::to_string_pretty(&card_json(&card)).map_err(|e| e.to_string())?,
        )?;
    } else {
        write(out, markdown(&card).trim_end())?;
    }
    Ok(0)
}
