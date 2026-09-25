//! How the fire loop prints what it reads: every event in full, the
//! judge's answers, and the stop.

use std::io::IsTerminal;

use serde_json::Value;

use super::card::Card;
use super::events::{Event, Kind};
use super::judge::{Judgment, Stop, clip};

/// How to print.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Characters kept of each long field; 0 keeps everything.
    pub clip: usize,
    pub color: bool,
}

impl Style {
    /// Color when stdout is a terminal and `NO_COLOR` is unset.
    #[must_use]
    pub fn detect(clip: usize) -> Style {
        Style {
            clip,
            color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn cut(self, text: &str) -> String {
        if self.clip == 0 {
            text.to_string()
        } else {
            clip(text, self.clip)
        }
    }

    fn json(self, value: &Value) -> String {
        self.cut(&serde_json::to_string_pretty(value).unwrap_or_default())
    }
}

/// `mm:ss` for a number of seconds.
#[must_use]
pub fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", whole / 60, whole % 60)
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("      {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One event, as the terminal shows it.
#[must_use]
pub fn event(style: Style, event: &Event, seconds: f64) -> String {
    let head = style.paint("2", &format!("[{}] {:<14}", clock(seconds), event.log));
    let body = match &event.kind {
        Kind::Session {
            id,
            model,
            directive,
        } => style.paint(
            "1;36",
            &format!("== session {id} · model {model} · {directive}"),
        ),
        Kind::Start {
            id,
            parent,
            component,
            name,
            implementation,
            digest,
        } => format!(
            "{} {} · {name} · implementation \"{implementation}\" · parameters {} · {id}{}",
            style.paint("35", ">> start"),
            style.paint("1;35", component),
            digest.get(..12).unwrap_or(digest),
            parent
                .as_ref()
                .map(|parent| format!(" under {parent}"))
                .unwrap_or_default(),
        ),
        Kind::Finish {
            id,
            component,
            outcome,
            summary,
        } => {
            let mut line = format!(
                "{} {component} · {outcome} · {id}",
                style.paint("35", "<< end")
            );
            if !summary.is_null() {
                line.push('\n');
                line.push_str(&indent(&style.json(summary)));
            }
            line
        }
        Kind::Jev {
            name,
            state,
            questions,
            answers,
            milliseconds,
            input_tokens,
        } => format!(
            "{} {name}{}{}\n{}\n{}\n{}\n{}\n{}\n{}",
            style.paint("1;33", "?? jev"),
            milliseconds
                .map(|ms| format!(" · {ms} ms"))
                .unwrap_or_default(),
            input_tokens
                .map(|tokens| format!(" · {tokens} input tokens"))
                .unwrap_or_default(),
            style.paint("33", "      state:"),
            indent(&style.json(state)),
            style.paint("33", "      questions:"),
            indent(&style.json(questions)),
            style.paint("33", "      answers:"),
            indent(&style.json(answers)),
        ),
        Kind::Think {
            headline,
            input,
            output,
            milliseconds,
            usd,
        } => format!(
            "{} {} · {input} in, {output} out · {:.1} s · ${usd:.4}",
            style.paint("34", ".. model"),
            if headline.is_empty() {
                "(no reasoning summary)".to_string()
            } else {
                style.cut(&headline.replace('\n', " "))
            },
            *milliseconds as f64 / 1000.0,
        ),
        Kind::Tool {
            name,
            arguments,
            output,
        } => {
            let input = super::judge::input_of(name, arguments);
            let mut line = format!(
                "{}\n{}",
                style.paint("1;32", &format!("$ {name}")),
                indent(&style.cut(&input))
            );
            if !output.is_empty() {
                line.push('\n');
                line.push_str(&style.paint("2", &indent(&style.cut(output))));
            }
            line
        }
        Kind::Say { source, text } => {
            let text = style.cut(text);
            if text.contains('\n') {
                format!("-- {source}:\n{}", indent(&text))
            } else {
                format!("-- {source}: {text}")
            }
        }
        Kind::End { state } => style.paint("1;36", &format!("== end · {state}")),
    };
    format!("{head} {body}")
}

/// The judge's answers after an action.
#[must_use]
pub fn judgment(style: Style, j: &Judgment) -> String {
    let number = |p: Option<f64>| p.map_or("?".to_string(), |p| format!("{p:.2}"));
    let line = format!(
        "   judge #{}: phase {} · on track {} · deviation {} · pitfall {} · stop {} · {} · ${:.5}{}",
        j.actions,
        j.phase.as_deref().unwrap_or("?"),
        number(j.on_track),
        j.deviation.as_deref().unwrap_or("?"),
        j.pitfall.as_deref().unwrap_or("none"),
        number(j.stop),
        j.milliseconds
            .map_or("?".to_string(), |ms| format!("{:.1} s", ms as f64 / 1000.0)),
        j.usd,
        j.error
            .as_ref()
            .map(|error| format!(" · error: {error}"))
            .unwrap_or_default(),
    );
    let code = if j.vote {
        "1;31"
    } else if j.on_track.is_some_and(|p| p >= 0.6) {
        "1;32"
    } else {
        "1;33"
    };
    let mut out = style.paint(code, &line);
    if j.vote {
        out.push_str(&style.paint("1;31", "  ← votes to stop"));
    }
    out
}

/// The card, as the run's reference.
#[must_use]
pub fn card(style: Style, card: &Card) -> String {
    let mut out = vec![
        style.paint("1", &format!("Winning strategy for {}", card.task)),
        format!(
            "  Fable 5.1 {}: {} passed, median {} to pass, {:.0} steps, ${:.2}",
            card.fable.effort,
            card.fable.passes,
            clock(card.fable.median_seconds),
            card.fable.median_steps,
            card.fable.median_cost_usd
        ),
        format!("  {}", card.strategy),
    ];
    for phase in &card.phases {
        out.push(format!(
            "  {}–{}  {}: {}",
            clock(phase.fable_seconds.start),
            clock(phase.fable_seconds.end),
            phase.id,
            phase.what
        ));
    }
    out.push(format!("  Independent check: {}", card.independent_check));
    out.push(format!(
        "  Budget: first edit by {}, first check by {}, done by {}",
        clock(card.budget.first_edit_s),
        clock(card.budget.first_check_s),
        clock(card.budget.done_s)
    ));
    out.join("\n")
}

/// The stop banner.
#[must_use]
pub fn stop(style: Style, stop: &Stop) -> String {
    style.paint(
        "1;41;97",
        &format!(
            " FIRE LOOP STOPPED THE RUN at {} · rule {} ",
            clock(stop.t),
            stop.rule
        ),
    ) + "\n"
        + &style.paint("1;31", &format!("  {}", stop.why))
}
