//! Streaming a run: every event to the terminal as it happens, and to a
//! JSON Lines record.

use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::Path;

use crate::run::{Ending, Event, Observer};

/// `mm:ss`.
#[must_use]
pub fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", whole / 60, whole % 60)
}

/// Prints events to standard output.
pub struct Terminal {
    color: bool,
    /// Lines of command output shown per command; the record keeps the
    /// whole cut.
    pub output_lines: usize,
}

impl Terminal {
    /// Color when standard output is a terminal and `NO_COLOR` is unset.
    #[must_use]
    pub fn new() -> Self {
        Terminal {
            color: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
            output_lines: 4,
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

/// A command or output line on one line: its first line, cut to `max`
/// characters, with a note when more lines follow.
fn one_line(text: &str, max: usize) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    let rest = lines.count();
    let mut out = if first.chars().count() > max {
        first.chars().take(max).collect::<String>() + "…"
    } else {
        first.to_string()
    };
    if rest > 0 {
        out.push_str(&format!(" (+{rest} lines)"));
    }
    out
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
                        if !action.view.is_empty() {
                            println!(
                                "        {}",
                                self.paint("36", &format!("view: {}", action.view.join(", ")))
                            );
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
                        one_line(&result.command, 160),
                        result.seconds
                    ),
                );
                let lines: Vec<&str> = result
                    .output
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect();
                if !lines.is_empty() {
                    let shown: Vec<String> = lines
                        .iter()
                        .take(self.output_lines)
                        .map(|l| one_line(l, 200))
                        .collect();
                    println!("{}", self.paint("2", &indent(&shown.join("\n"))));
                    if lines.len() > self.output_lines {
                        println!(
                            "{}",
                            self.paint(
                                "2",
                                &format!(
                                    "        … {} more lines",
                                    lines.len() - self.output_lines
                                )
                            )
                        );
                    }
                }
            }
            Event::Reviewed { step, judgment } => {
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
                        "{} {answers} · ${:.5}",
                        self.paint("1;35", &format!("step {step} · jev test review")),
                        judgment.usd
                    ),
                );
            }
            Event::Tested {
                step,
                froze,
                results,
            } => {
                let passing = results.iter().filter(|r| r.ok()).count();
                let failing: Vec<&str> = results
                    .iter()
                    .filter(|r| !r.ok())
                    .map(|r| r.command.as_str())
                    .collect();
                let head = if *froze {
                    format!("step {step} · froze {} acceptance tests", results.len())
                } else {
                    format!("step {step} · acceptance tests")
                };
                let mut text = format!(
                    "{} {passing} of {} pass",
                    self.paint("1;35", &head),
                    results.len()
                );
                if !failing.is_empty() {
                    text.push_str(&format!(" · failing: {}", failing.join(", ")));
                }
                self.line(seconds, &text);
            }
            Event::Ended { outcome } => {
                let why = match &outcome.ending {
                    Ending::Finished => "the model finished".to_string(),
                    Ending::StepLimit => "the step limit".to_string(),
                    Ending::TimeLimit => "the time limit".to_string(),
                    Ending::SpendLimit => "the spend limit".to_string(),
                    Ending::BadReplies(error) => format!("unusable replies ({error})"),
                    Ending::Idle => "replies that ran nothing".to_string(),
                    Ending::Unaccepted => {
                        "finished replies refused while acceptance tests failed".to_string()
                    }
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

#[cfg(test)]
mod tests {
    use super::one_line;

    #[test]
    fn a_long_or_multi_line_command_prints_on_one_line() {
        assert_eq!(one_line("ls -la", 160), "ls -la");
        assert_eq!(
            one_line("cat > f <<'EOF'\na\nb\nEOF", 160),
            "cat > f <<'EOF' (+3 lines)"
        );
        assert_eq!(one_line(&"x".repeat(10), 4), "xxxx…");
    }
}
