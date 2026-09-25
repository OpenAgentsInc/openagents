//! The fire loop's report: what the run did, how it compared with the
//! winners' strategy, and why it stopped, in one document a person or a
//! model can read without the logs.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::card::Card;
use super::judge::{Run, Stop, clip};
use super::show::clock;

/// How a watched run ended.
#[derive(Clone, Debug)]
pub enum Ending {
    Stopped(Stop),
    /// The episode ended on its own, with the verifier's reward when
    /// known.
    Finished(Option<f64>),
    /// The logs ended without an end record or a stop.
    Unknown,
}

/// Where each of the card's phases was first judged to start, against
/// the winners' start.
fn phase_times(run: &Run, card: &Card) -> Vec<(String, Option<f64>, f64)> {
    card.phases
        .iter()
        .map(|phase| {
            let first = run
                .judgments
                .iter()
                .find(|j| j.phase.as_deref() == Some(phase.id.as_str()))
                .map(|j| j.t);
            (phase.id.clone(), first, phase.fable_seconds.start)
        })
        .collect()
}

/// The report as Markdown.
#[must_use]
pub fn markdown(
    run: &Run,
    card: &Card,
    ending: &Ending,
    trial: &str,
    reward: Option<f64>,
) -> String {
    let mut out = String::new();
    let t = run.seconds(run.last);
    let _ = writeln!(out, "# Fire loop report: {} ({trial})\n", card.task);
    match ending {
        Ending::Stopped(stop) => {
            let _ = writeln!(
                out,
                "**Stopped at {} by the `{}` rule.** {}\n",
                clock(stop.t),
                stop.rule,
                stop.why
            );
        }
        Ending::Finished(reward) => {
            let _ = writeln!(
                out,
                "**The run finished on its own at {}.** Verifier reward: {}.\n",
                clock(t),
                reward.map_or("not known yet".to_string(), |r| format!("{r}"))
            );
        }
        Ending::Unknown => {
            let _ = writeln!(
                out,
                "**The logs ended at {} without an end record.**\n",
                clock(t)
            );
        }
    }
    if let (Ending::Stopped(_), Some(reward)) = (ending, reward) {
        let _ = writeln!(
            out,
            "In replay, the real trial went on and scored **{reward}**{}.\n",
            if reward >= 1.0 {
                ", so this stop would have killed a passing run"
            } else {
                ""
            }
        );
    }
    let _ = writeln!(out, "## The run against the winners\n");
    let _ = writeln!(
        out,
        "| Measure | This run | Fable 5.1 {} (median of passing runs) |\n| --- | --- | --- |",
        card.fable.effort
    );
    let _ = writeln!(
        out,
        "| Time | {} | {} to done |",
        clock(t),
        clock(card.fable.median_seconds)
    );
    let _ = writeln!(
        out,
        "| Actions | {} | {:.0} steps |",
        run.actions.len(),
        card.fable.median_steps
    );
    let _ = writeln!(
        out,
        "| First edit | {} | {} |",
        run.first_edit.map_or("none".to_string(), clock),
        clock(card.budget.first_edit_s)
    );
    let _ = writeln!(
        out,
        "| Cost | ${:.4} (model ${:.4}, Jev ${:.4}; judge ${:.4} not counted) | ${:.2} |",
        run.usd(),
        run.model_usd,
        run.jev_usd,
        run.judge_usd,
        card.fable.median_cost_usd
    );
    let _ = writeln!(
        out,
        "| Finishes the host turned back | {} | |\n",
        run.refused_finishes
    );
    let _ = writeln!(
        out,
        "## Phases\n\n| Phase | First judged in it | Winners started it |\n| --- | --- | --- |"
    );
    for (id, first, fable) in phase_times(run, card) {
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} |",
            first.map_or("never".to_string(), clock),
            clock(fable)
        );
    }
    let _ = writeln!(out, "\n## Judgments\n");
    let _ = writeln!(
        out,
        "| At | Action | Phase | On track | Deviation | Pitfall | Stop | Vote |\n| --- | ---: | --- | ---: | --- | --- | ---: | --- |"
    );
    let number = |p: Option<f64>| p.map_or("?".to_string(), |p| format!("{p:.2}"));
    for j in &run.judgments {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            clock(j.t),
            j.actions,
            j.phase.as_deref().unwrap_or("?"),
            number(j.on_track),
            j.deviation.as_deref().unwrap_or("?"),
            j.pitfall.as_deref().unwrap_or("none"),
            number(j.stop),
            if j.vote { "stop" } else { "" }
        );
    }
    let _ = writeln!(out, "\n## Components that ran\n");
    for component in &run.components {
        let _ = writeln!(out, "- `{component}`");
    }
    if !run.notes.is_empty() {
        let _ = writeln!(out, "\n## Host notes\n");
        for note in &run.notes {
            let _ = writeln!(out, "- {}", note.replace('\n', " "));
        }
    }
    let _ = writeln!(out, "\n## Last actions\n");
    let from = run.actions.len().saturating_sub(15);
    for (n, action) in run.actions.iter().enumerate().skip(from) {
        let _ = writeln!(
            out,
            "### {} · action {} · `{}`{}\n\n```text\n{}\n```\n\n```text\n{}\n```\n",
            clock(action.t),
            n + 1,
            action.tool,
            if action.edit { " · edit" } else { "" },
            clip(&action.input, 3_000),
            clip(&action.output, 3_000)
        );
    }
    let _ = writeln!(out, "## The winners' strategy\n\n{}\n", card.strategy);
    for phase in &card.phases {
        let _ = writeln!(
            out,
            "- `{}` ({}–{}): {} Done when: {}",
            phase.id,
            clock(phase.fable_seconds.start),
            clock(phase.fable_seconds.end),
            phase.what,
            phase.done_when
        );
    }
    let _ = writeln!(out, "\nIndependent check: {}", card.independent_check);
    if !card.pitfalls.is_empty() {
        let _ = writeln!(out, "\nKnown pitfalls:\n");
        for pitfall in &card.pitfalls {
            let _ = writeln!(out, "- `{}`: {}", pitfall.id, pitfall.what);
        }
    }
    out
}

/// The report as JSON.
#[must_use]
pub fn json(run: &Run, card: &Card, ending: &Ending, trial: &str, reward: Option<f64>) -> Value {
    let (outcome, stop) = match ending {
        Ending::Stopped(stop) => ("stopped", serde_json::to_value(stop).unwrap_or_default()),
        Ending::Finished(_) => ("finished", Value::Null),
        Ending::Unknown => ("unknown", Value::Null),
    };
    json!({
        "schema": "openagents.fire.report.v1",
        "task": card.task,
        "trial": trial,
        "outcome": outcome,
        "stop": stop,
        "reward": reward,
        "seconds": run.seconds(run.last),
        "actions": run.actions.len(),
        "first_edit_s": run.first_edit,
        "usd": {"model": run.model_usd, "jev": run.jev_usd, "judge": run.judge_usd},
        "refused_finishes": run.refused_finishes,
        "components": run.components,
        "judgments": run.judgments,
        "phases": phase_times(run, card).into_iter().map(|(id, first, fable)| json!({"id": id, "first_s": first, "winners_s": fable})).collect::<Vec<_>>(),
    })
}
