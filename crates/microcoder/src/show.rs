//! Streaming a run: every event to the terminal as it happens, and to a
//! JSON Lines record.

use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::Path;

use crate::run::{Ending, Event, Observer, Retrieval};

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
                        "{} {answers} · {:.1} s · {}",
                        self.paint("1;33", &format!("step {step} · jev")),
                        judgment.milliseconds as f64 / 1000.0,
                        dollars(judgment.usd, 5)
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
                    "prompt {prompt_chars} chars · {} in, {} out · {:.1} s · {}",
                    generated.prompt_tokens,
                    generated.completion_tokens,
                    generated.milliseconds as f64 / 1000.0,
                    dollars(generated.usd, 5)
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
            Event::Assessed { judgment, strong } => {
                let answers = if let Some(error) = &judgment.error {
                    format!("no answer: {error}")
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
                        "{} {answers} · {} · {}",
                        self.paint("1;35", "jev task check"),
                        dollars(judgment.usd, 5),
                        if *strong {
                            "the stronger model writes the acceptance tests"
                        } else {
                            "the default model does every step"
                        }
                    ),
                );
            }
            Event::Retrieved { step, retrieval } => {
                self.line(
                    seconds,
                    &format!(
                        "{} {}",
                        self.paint("1;36", &format!("step {step} · kb")),
                        retrieval_line(retrieval)
                    ),
                );
            }
            Event::Covered {
                step,
                judgment,
                uncovered,
            } => {
                let answers = judgment.error.clone().unwrap_or_else(|| {
                    judgment
                        .answers
                        .iter()
                        .map(|(id, p)| format!("{id} {p:.2}"))
                        .collect::<Vec<_>>()
                        .join(" · ")
                });
                let verdict = if *uncovered {
                    "some requirement has no test: add tests"
                } else {
                    "the tests cover the task"
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {answers} · {} · {verdict}",
                        self.paint("1;35", &format!("step {step} · jev checks test coverage")),
                        dollars(judgment.usd, 5)
                    ),
                );
            }
            Event::Conformed {
                step,
                judgment,
                flagged,
            } => {
                let answers = judgment.error.clone().unwrap_or_else(|| {
                    judgment
                        .answers
                        .iter()
                        .map(|(id, p)| format!("{id} {p:.2}"))
                        .collect::<Vec<_>>()
                        .join(" · ")
                });
                let verdict = if flagged.is_empty() {
                    "no contradiction".to_string()
                } else {
                    format!("sent back: the code contradicts {}", flagged.join(", "))
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {answers} · {} · {verdict}",
                        self.paint(
                            "1;35",
                            &format!(
                                "step {step} · jev checks the code against the knowledge base"
                            )
                        ),
                        dollars(judgment.usd, 5)
                    ),
                );
            }
            Event::Disputed {
                step,
                judgment,
                dropped,
            } => {
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
                let verdict = if dropped.is_empty() {
                    "no test dropped".to_string()
                } else {
                    format!("dropped as wrong: {}", dropped.join(", "))
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {answers} · {} · {verdict}",
                        self.paint(
                            "1;35",
                            &format!("step {step} · jev checks the failing tests")
                        ),
                        dollars(judgment.usd, 5)
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
            Event::Gated { step, checked } => {
                let answers: Vec<String> = checked
                    .judgments
                    .iter()
                    .flat_map(|j| j.answers.iter())
                    .filter(|(_, p)| *p >= 0.5)
                    .map(|(id, p)| format!("{id} {p:.2}"))
                    .collect();
                let verdict = if checked.refused {
                    "sent back"
                } else {
                    "passed"
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {verdict}{}",
                        self.paint(
                            "1;35",
                            &format!("step {step} · {} check before ending", checked.check)
                        ),
                        if answers.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", answers.join(" · "))
                        }
                    ),
                );
                for detail in &checked.detail {
                    println!("        {}", one_line(detail, 160));
                }
            }
            Event::OracleStep(step) => {
                let why = match &step.generated.action {
                    Ok(action) => action.rationale.clone(),
                    Err(error) => format!("unusable reply: {error}"),
                };
                self.line(
                    seconds,
                    &format!(
                        "{} {} · {}",
                        self.paint("1;36", &format!("oracle step {}", step.step)),
                        one_line(&why, 200),
                        dollars(step.generated.usd, 5)
                    ),
                );
                for result in &step.results {
                    println!(
                        "        $ {} → {}",
                        one_line(&result.command, 100),
                        if result.ok() { "ok" } else { "failed" }
                    );
                }
            }
            Event::Oracle { report } => {
                self.line(
                    seconds,
                    &format!(
                        "{} {} steps · kept {} · dropped as already passing {}{}",
                        self.paint("1;36", "oracle"),
                        report.steps,
                        if report.kept.is_empty() {
                            "none".to_string()
                        } else {
                            report.kept.join(", ")
                        },
                        if report.trivial.is_empty() {
                            "none".to_string()
                        } else {
                            report.trivial.join(", ")
                        },
                        report
                            .stopped
                            .as_ref()
                            .map(|why| format!(" · stopped by {why}"))
                            .unwrap_or_default()
                    ),
                );
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
                    Ending::TestsHeld => {
                        "every acceptance test passing for several steps in a row".to_string()
                    }
                };
                let embeddings = if outcome.embedding_usd != Some(0.0) {
                    format!(" · embeddings {}", dollars(outcome.embedding_usd, 6))
                } else {
                    String::new()
                };
                let unknown = if outcome.cost_unknown.is_empty() {
                    String::new()
                } else {
                    format!(
                        " · {} calls of unknown cost, at least ${:.4} known",
                        outcome.cost_unknown.len(),
                        outcome.known_usd
                    )
                };
                self.line(
                    seconds,
                    &self.paint(
                        "1;36",
                        &format!(
                            "loop ended: {why} · {} steps · model {} · jev {}{embeddings}{unknown}{}",
                            outcome.steps,
                            dollars(outcome.model_usd, 4),
                            dollars(outcome.jev_usd, 5),
                            if outcome.knowledge_assisted {
                                " · knowledge-assisted"
                            } else {
                                ""
                            }
                        ),
                    ),
                );
            }
        }
    }
}

/// A retrieval on one line: how many candidates, the entries kept with
/// Jev's relevance, the bodies shown, and the cost.
#[must_use]
pub fn retrieval_line(retrieval: &Retrieval) -> String {
    let mut parts = vec![format!(
        "{} candidates{}",
        retrieval.candidates.len(),
        match (&retrieval.lexical_only, retrieval.cached) {
            (_, true) => " (unchanged query, reused)",
            (Some(_), false) => " by words alone",
            (None, false) => " by words and embeddings",
        }
    )];
    if let Some(error) = &retrieval.error {
        parts.push(format!("no Jev answers: {error}"));
    } else if retrieval.kept.is_empty() {
        parts.push("none kept".to_string());
    } else {
        parts.push(format!(
            "kept {}",
            retrieval
                .kept
                .iter()
                .map(|k| format!("{} {:.2}", k.id, k.relevance))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !retrieval.expanded.is_empty() {
        parts.push(format!(
            "in full: {}",
            retrieval
                .expanded
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    parts.push(dollars(
        retrieval
            .jev_usd
            .zip(retrieval.embedding_usd)
            .map(|(j, e)| j + e),
        5,
    ));
    parts.join(" · ")
}

/// Dollars to `places` decimals, or `cost unknown`: an unknown cost is
/// never shown as $0.
#[must_use]
pub fn dollars(usd: Option<f64>, places: usize) -> String {
    usd.map_or("cost unknown".to_string(), |usd| format!("${usd:.places$}"))
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
