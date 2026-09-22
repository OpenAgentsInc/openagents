//! The loop: judge, generate, run, repeat.
//!
//! The three steps are traits so the loop runs the same against Jev, a
//! real generator, and a real shell as it does against the fakes in the
//! tests.

use std::future::Future;

use serde_json::json;

use crate::action::Action;
use crate::delegate::Reason;
use crate::state::{Observation, State, Turn};

/// Typed judgments over the state, rendered for the prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum Judgments {
    /// Hints derived from the judge's answers, one line each.
    Answered(Vec<String>),
    /// The judge could not answer. The loop continues with the
    /// deterministic view, and the reason goes on the record.
    Unavailable(String),
    /// The host's policy ends the loop here, before generating. Only code
    /// returns this; it never reaches the prompt.
    Stop(Reason),
}

/// The step that asks Jev about the state.
pub trait Judge {
    fn judge(&mut self, state: &State) -> impl Future<Output = Judgments>;
}

/// The step that proposes the next action.
pub trait Generate {
    /// The generator's raw reply to `prompt`, or why there is none.
    fn generate(&mut self, prompt: &str) -> impl Future<Output = Result<String, String>>;
}

/// The step that runs a command in the task checkout.
pub trait Shell {
    fn run(&mut self, command: &str) -> impl Future<Output = Observation>;
}

/// The limits code enforces on a run.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// The most steps a run takes before it stops unfinished.
    pub max_steps: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Self { max_steps: 30 }
    }
}

/// Why a run stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Ended {
    /// The generator said the work is done.
    Finished {
        title: String,
        summary: String,
        steps: usize,
    },
    /// The step limit ran out first.
    StepLimit { steps: usize },
    /// The generator failed, so no step could be taken.
    GenerationFailed { error: String, steps: usize },
    /// The host's escalation policy stopped the loop before this step.
    Stopped { reason: Reason, steps: usize },
    /// The host delegated the task, and the delegate ended with `status`.
    Delegated {
        /// Whether the delegate answered.
        answered: bool,
        status: String,
        title: String,
        summary: String,
        steps: usize,
    },
}

/// Runs the loop until the generator finishes or a bound ends the run.
/// Each command and each malformed reply is appended to `state.history`,
/// so the next step's judge and generator see it.
pub async fn run<J, G, S>(
    state: &mut State,
    prompt: &str,
    bounds: Bounds,
    judge: &mut J,
    generator: &mut G,
    shell: &mut S,
) -> Ended
where
    J: Judge,
    G: Generate,
    S: Shell,
{
    for step in 1..=bounds.max_steps {
        let judgments = judge.judge(state).await;
        if let Judgments::Stop(reason) = judgments {
            return Ended::Stopped {
                reason,
                steps: step - 1,
            };
        }
        let ai_prompt = render_prompt(state, prompt, &judgments, (step, bounds.max_steps));
        let reply = match generator.generate(&ai_prompt).await {
            Ok(reply) => reply,
            Err(error) => {
                return Ended::GenerationFailed {
                    error,
                    steps: step - 1,
                };
            }
        };
        match Action::parse(&reply) {
            Ok(Action::Finished { title, summary }) => {
                return Ended::Finished {
                    title,
                    summary,
                    steps: step,
                };
            }
            Ok(Action::Shell { command, reason }) => {
                if let Some(reason) = &reason {
                    println!("  why ▸ {reason}");
                }
                let observation = shell.run(&command).await;
                state.history.push(Turn::Shell {
                    command,
                    reason,
                    observation,
                });
            }
            Err(error) => state.history.push(Turn::Malformed { reply, error }),
        }
    }
    Ended::StepLimit {
        steps: bounds.max_steps,
    }
}

/// The system prompt: how the agent works in the checkout.
pub const INSTRUCTIONS: &str = "You are Coder One, a coding agent working in a fresh \
clone of a GitHub repository. Your job is to resolve the GitHub issue in the state. \
You act by calling exactly one tool per step: `shell` runs one bash command in the \
repository root, without a terminal or standard input, and its output appears in \
`state.history` on the next step; `finished` ends the run. Investigate before you edit. Edit files with non-interactive tools \
such as heredocs, sed, or short Python scripts; never open an editor or pager. \
Work efficiently: once you understand the fix, write it into the files, add the \
tests the issue asks for, run the test suite, and finish. Do not keep re-checking \
the same behavior with throwaway scripts. \
Do not commit, push, or create branches: the host does that when you finish. \
`judgments` holds hints from a fast classifier about which files look relevant, \
what the last command showed, and which requirements look satisfied; treat them as \
evidence, not orders.";

/// The system prompt for a headless episode: a task instruction instead
/// of an issue, the task's own working directory instead of a fresh
/// clone, and an automated grader instead of a pull request review.
pub const EPISODE_INSTRUCTIONS: &str = "You are Coder One, a coding agent working \
inside a task environment. Your job is to complete the task whose instruction is \
`state.issue.body`. You act by calling exactly one tool per step: `shell` runs one \
bash command in the task's working directory, without a terminal or standard input, \
and its output appears in `state.history` on the next step; `finished` ends the \
episode. Nobody answers questions: decide from the instruction and the environment. \
Investigate before you edit, and edit files with non-interactive tools such as \
heredocs, sed, or short Python scripts; never open an editor or pager. Use git, \
package managers, and builds when the task needs them. An automated checker grades \
the final state of the environment against the instruction, so before you call \
`finished`, verify every requirement in the instruction, including exact paths, \
names, and formats. Keep long-running commands within the command deadline. \
`judgments` holds hints from a fast classifier about which files look relevant, \
what the last command showed, and which requirements look satisfied; treat them as \
evidence, not orders.";

/// The reminder every prompt ends with, nearest the model's answer.
const ACTION_CONTRACT: &str = "Call exactly one tool now: `shell` with the next \
command, or `finished` with a title and summary once the task is done and checked.";

/// How many of the most recent turns show their output at length.
const RECENT_TURNS: usize = 3;

/// The generator's input: the state, the user's prompt, and the
/// judgments, with the action contract last so it is nearest the reply.
/// Recent turns keep up to 6,000 characters of output; older turns keep
/// a short tail, so the prompt stays bounded as the run grows.
pub fn render_prompt(
    state: &State,
    prompt: &str,
    judgments: &Judgments,
    (step, max_steps): (usize, usize),
) -> String {
    let judgments = match judgments {
        Judgments::Answered(hints) => json!({ "hints": hints }),
        Judgments::Unavailable(reason) => json!({ "unavailable": reason }),
        Judgments::Stop(reason) => json!({ "unavailable": reason.to_string() }),
    };
    let recent = state.history.len().saturating_sub(RECENT_TURNS);
    let history: Vec<_> = state
        .history
        .iter()
        .enumerate()
        .map(|(index, turn)| match turn {
            Turn::Shell {
                command,
                reason,
                observation,
            } => {
                let output = if index >= recent {
                    head_and_tail(&observation.output, 1_500, 4_500)
                } else {
                    head_and_tail(&observation.output, 0, 300)
                };
                json!({
                    "step": index + 1,
                    "command": command,
                    "reason": reason,
                    "exit": observation.exit,
                    "output": output,
                })
            }
            Turn::Malformed { reply, error } => json!({
                "step": index + 1,
                "malformed_reply": head_and_tail(reply, 300, 0),
                "error": error,
            }),
        })
        .collect();
    let input = json!({
        "task": prompt,
        "budget": {
            "step": step,
            "max_steps": max_steps,
            "steps_left_after_this": max_steps - step,
        },
        "state": {
            "environment": state.environment,
            "issue": state.issue,
            "history": history,
        },
        "judgments": judgments,
    });
    format!(
        "{}\n\n{ACTION_CONTRACT}",
        serde_json::to_string_pretty(&input).unwrap_or_else(|_| input.to_string())
    )
}

/// The first `head` and last `tail` characters of `text`, with a marker
/// naming how much was left out between them.
fn head_and_tail(text: &str, head: usize, tail: usize) -> String {
    let count = text.chars().count();
    if count <= head + tail {
        return text.to_string();
    }
    let byte = |n: usize| text.char_indices().nth(n).map_or(text.len(), |(i, _)| i);
    format!(
        "{}\n…[{} characters omitted]…\n{}",
        &text[..byte(head)],
        count - head - tail,
        &text[byte(count - tail)..]
    )
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::state::{Environment, Issue};

    fn state() -> State {
        State::new(
            Environment {
                repository: "example/repo".to_string(),
                workdir: "/tmp/checkout".to_string(),
                os: "linux".to_string(),
            },
            Issue {
                url: "https://github.com/example/repo/issues/1".to_string(),
                title: "Tests fail".to_string(),
                body: "The parser test fails on empty input.".to_string(),
                labels: vec![],
            },
        )
    }

    /// A judge that answers from a script, recording what it saw.
    struct ScriptedJudge {
        answers: VecDeque<Judgments>,
        seen_history: Vec<usize>,
    }

    impl Judge for ScriptedJudge {
        async fn judge(&mut self, state: &State) -> Judgments {
            self.seen_history.push(state.history.len());
            self.answers
                .pop_front()
                .unwrap_or_else(|| Judgments::Answered(vec![]))
        }
    }

    fn judge(answers: Vec<Judgments>) -> ScriptedJudge {
        ScriptedJudge {
            answers: answers.into(),
            seen_history: vec![],
        }
    }

    /// A generator that replies from a script and keeps every prompt.
    struct ScriptedGenerator {
        replies: VecDeque<Result<String, String>>,
        prompts: Vec<String>,
    }

    impl Generate for ScriptedGenerator {
        async fn generate(&mut self, prompt: &str) -> Result<String, String> {
            self.prompts.push(prompt.to_string());
            self.replies
                .pop_front()
                .unwrap_or_else(|| Err("script exhausted".to_string()))
        }
    }

    fn generator(replies: &[Result<&str, &str>]) -> ScriptedGenerator {
        ScriptedGenerator {
            replies: replies
                .iter()
                .map(|reply| reply.map(str::to_string).map_err(str::to_string))
                .collect(),
            prompts: vec![],
        }
    }

    /// A shell that runs nothing and records each command.
    #[derive(Default)]
    struct RecordingShell {
        commands: Vec<String>,
    }

    impl Shell for RecordingShell {
        async fn run(&mut self, command: &str) -> Observation {
            self.commands.push(command.to_string());
            Observation {
                exit: Some(0),
                output: format!("ran {command}"),
                truncated: false,
            }
        }
    }

    const LS: Result<&str, &str> = Ok(r#"{"action":"shell","command":"ls"}"#);
    const DONE: Result<&str, &str> =
        Ok(r#"{"action":"finished","title":"Fix parser","summary":"Handled empty input."}"#);

    #[tokio::test]
    async fn runs_commands_until_finished() {
        let mut state = state();
        let mut judge = judge(vec![]);
        let mut generator = generator(&[LS, LS, DONE]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds::default(),
            &mut judge,
            &mut generator,
            &mut shell,
        )
        .await;

        assert_eq!(
            ended,
            Ended::Finished {
                title: "Fix parser".to_string(),
                summary: "Handled empty input.".to_string(),
                steps: 3,
            }
        );
        assert_eq!(shell.commands, ["ls", "ls"]);
        assert_eq!(state.history.len(), 2);
        // The judge sees the state as it stands before each step.
        assert_eq!(judge.seen_history, [0, 1, 2]);
    }

    #[tokio::test]
    async fn stops_at_the_step_limit() {
        let mut state = state();
        let mut generator = generator(&[LS, LS, LS]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds { max_steps: 2 },
            &mut judge(vec![]),
            &mut generator,
            &mut shell,
        )
        .await;

        assert_eq!(ended, Ended::StepLimit { steps: 2 });
        assert_eq!(shell.commands.len(), 2);
    }

    #[tokio::test]
    async fn a_malformed_reply_costs_a_step_and_is_shown_back() {
        let mut state = state();
        let mut generator = generator(&[Ok("I'll look around first."), DONE]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds::default(),
            &mut judge(vec![]),
            &mut generator,
            &mut shell,
        )
        .await;

        assert!(matches!(ended, Ended::Finished { steps: 2, .. }));
        assert!(shell.commands.is_empty());
        assert!(matches!(state.history[0], Turn::Malformed { .. }));
        assert!(generator.prompts[1].contains("not a valid action"));
    }

    #[tokio::test]
    async fn judgments_reach_the_prompt_and_an_unavailable_judge_does_not_stop_the_run() {
        let mut state = state();
        let mut judge = judge(vec![
            Judgments::Answered(vec!["src/parser.rs is likely relevant".to_string()]),
            Judgments::Unavailable("door unreachable".to_string()),
        ]);
        let mut generator = generator(&[LS, DONE]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds::default(),
            &mut judge,
            &mut generator,
            &mut shell,
        )
        .await;

        assert!(matches!(ended, Ended::Finished { .. }));
        assert!(generator.prompts[0].contains("src/parser.rs is likely relevant"));
        assert!(generator.prompts[1].contains("door unreachable"));
    }

    #[tokio::test]
    async fn a_host_stop_ends_the_run_before_generating() {
        let mut state = state();
        let mut judge = judge(vec![
            Judgments::Answered(vec![]),
            Judgments::Stop(Reason::ErrorStreak(3)),
        ]);
        let mut generator = generator(&[LS, LS]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds::default(),
            &mut judge,
            &mut generator,
            &mut shell,
        )
        .await;

        assert_eq!(
            ended,
            Ended::Stopped {
                reason: Reason::ErrorStreak(3),
                steps: 1,
            }
        );
        assert_eq!(generator.prompts.len(), 1);
    }

    #[tokio::test]
    async fn a_generation_failure_ends_the_run_with_its_error() {
        let mut state = state();
        let mut generator = generator(&[LS, Err("401 from the door")]);
        let mut shell = RecordingShell::default();

        let ended = run(
            &mut state,
            "Solve this issue.",
            Bounds::default(),
            &mut judge(vec![]),
            &mut generator,
            &mut shell,
        )
        .await;

        assert_eq!(
            ended,
            Ended::GenerationFailed {
                error: "401 from the door".to_string(),
                steps: 1,
            }
        );
    }
}
