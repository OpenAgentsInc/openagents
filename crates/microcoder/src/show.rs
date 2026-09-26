//! Streaming a run: every event to the terminal as it happens, and to a
//! JSON Lines record.

use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::Path;

use crate::run::{Ending, Event, Observer};
use crate::state::cut;

/// `mm:ss`.
#[must_use]
pub fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", whole / 60, whole % 60)
}

/// Prints events to standard output.
pub struct Terminal {
    color: bool,
    /// Characters of command output shown per command; the record keeps
    /// the full cut.
    pub output_chars: usize,
}

impl Terminal {
    /// Color when standard output is a terminal and `NO_COLOR` is unset.
    #[must_use]
    pub fn new() -> Self {
        Terminal {
            color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
            output_chars: 1_200,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// Prints one line with the elapsed time.
    pub fn line(&self, seconds: f64, text: &str) {
        println!(
            "{} {text}",
            self.paint("2", &format!("[{}]", clock(seconds)))
        );
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("        {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Observer for Terminal {
    fn event(&mut self, seconds: f64, event: &Event) {
        match event {
            Event::Judged { step, judgment } => {
                let answers = if let Some(error) = &judgment.error {
                    format!("no answers: {error}")
                } else {
                    judgment
                        .answers
                        .iter()
                        .map(|(id, p)| format!("{id} {p:.2}"))
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {answers} · {:.1} s · ${:.5}",
                        self.paint("1;33", &format!("step {step} · jev")),
                        judgment.milliseconds as f64 / 1000.0,
                        judgment.usd
                    ),
                );
            }
            Event::Generated {
                step,
                prompt_chars,
                generated,
            } => {
                let head = self.paint("1;34", &format!("step {step} · {}", generated.model));
                let meta = format!(
                    "prompt {prompt_chars} chars · {} in, {} out · {:.1} s · ${:.5}",
                    generated.prompt_tokens,
                    generated.completion_tokens,
                    generated.milliseconds as f64 / 1000.0,
                    generated.usd
                );
                match &generated.action {
                    Ok(action) => {
                        self.line(seconds, &format!("{head} {meta}"));
                        println!(
                            "        {}",
                            self.paint("1;97", &format!("why: {}", action.rationale))
                        );
                        for command in &action.commands {
                            println!("        {}", self.paint("32", &format!("$ {command}")));
                        }
                        if action.finished {
                            println!("        {}", self.paint("1;32", "finished"));
                        }
                    }
                    Err(error) => {
                        self.line(
                            seconds,
                            &format!(
                                "{head} {meta} · {}",
                                self.paint("1;31", &format!("unusable reply: {error}"))
                            ),
                        );
                    }
                }
            }
            Event::Ran { result, .. } => {
                let status = match (result.exit, result.timed_out) {
                    (_, true) => self.paint("1;31", "timed out"),
                    (Some(0), _) => self.paint("32", "exit 0"),
                    (Some(code), _) => self.paint("1;31", &format!("exit {code}")),
                    (None, _) => self.paint("1;31", "killed"),
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {} · {status} · {:.1} s",
                        self.paint("1;32", "$"),
                        cut(&result.command, 160, 0).replace('\n', " "),
                        result.seconds
                    ),
                );
                if !result.output.trim().is_empty() {
                    println!(
                        "{}",
                        self.paint(
                            "2",
                            &indent(&cut(
                                result.output.trim_end(),
                                self.output_chars / 2,
                                self.output_chars / 2
                            ))
                        )
                    );
                }
            }
            Event::Ended { outcome } => {
                let why = match &outcome.ending {
                    Ending::Finished => "the model finished".to_string(),
                    Ending::StepLimit => "the step limit".to_string(),
                    Ending::TimeLimit => "the time limit".to_string(),
                    Ending::SpendLimit => "the spend limit".to_string(),
                    Ending::BadReplies(error) => format!("unusable replies ({error})"),
                };
                self.line(
                    seconds,
                    &self.paint(
                        "1;36",
                        &format!(
                            "loop ended: {why} · {} steps · model ${:.4} · jev ${:.5}",
                            outcome.steps, outcome.model_usd, outcome.jev_usd
                        ),
                    ),
                );
            }
        }
    }
}

/// Appends every event to a JSON Lines file.
pub struct Record {
    file: File,
}

impl Record {
    /// A record at `path`, created new.
    ///
    /// # Errors
    ///
    /// The file can't be created.
    pub fn create(path: &Path) -> std::io::Result<Self> {
        Ok(Record {
            file: File::create(path)?,
        })
    }

    /// Writes one JSON line.
    pub fn write(&mut self, value: &serde_json::Value) {
        let _ = writeln!(self.file, "{value}");
        let _ = self.file.flush();
    }
}

impl Observer for Record {
    fn event(&mut self, seconds: f64, event: &Event) {
        let mut value = serde_json::to_value(event).unwrap_or_default();
        value["seconds"] = serde_json::json!(seconds);
        self.write(&value);
    }
}

/// Sends every event to two observers.
pub struct Both<'a, A: Observer, B: Observer>(pub &'a mut A, pub &'a mut B);

impl<A: Observer, B: Observer> Observer for Both<'_, A, B> {
    fn event(&mut self, seconds: f64, event: &Event) {
        self.0.event(seconds, event);
        self.1.event(seconds, event);
    }
}
