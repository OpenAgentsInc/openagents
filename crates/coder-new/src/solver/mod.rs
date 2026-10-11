//! `coder solver N` (#11336): four constrained Claude Agent SDK sessions
//! take an issue to a pull request, each holding only the tools its hand
//! of the work needs. `docs/coder/solver.md` is the spec.
//!
//! - **Intaker** (Haiku, `environment` and `issue` tools): checks that
//!   `git`, `gh` and the Claude Code login are present and whether the
//!   checkout is clean, reads the issue, and answers `INTAKE: ready` or
//!   `INTAKE: blocked: why`. The harness then makes the fresh worktree
//!   ([`issue_run::setup::prepare`]).
//! - **Briefer** (Haiku, Read, Grep, Glob): the existing code-brief
//!   algorithm runs first ([`issue_run::decide`]); the Briefer then reads
//!   the listed files and writes an analysis of what the change should
//!   look like, which joins the briefing's system prompt.
//! - **Coder** (Opus, Read, Edit, Write, Grep, Glob): codes only.
//! - **Tester** (Sonnet, Read, Grep, Glob, `run_check`): runs the checks and
//!   ends with `VERDICT: pass`, `VERDICT: coder: notes` or `VERDICT:
//!   briefer: notes`. The harness reruns the required checks itself and
//!   judges ([`issue_run::verdict`]); hand-backs are bounded by `--rounds`.
//!
//! The run shares `issue_run`'s transcript, cards, run folder, summary and
//! pull-request step, so the terminal draws it with the same widgets and
//! `coder solver --replay` plays a recording again.

pub mod hands;

use std::{path::PathBuf, time::Instant};

use serde_json::{Value, json};

use crate::issue_run::{
    self, Feed, Transcript,
    agent::Work,
    decide::Briefing,
    setup::{self, Base},
    verdict::{self, Outcome, Status},
};
use crate::live::Entry;

/// The mode's name, in the rail and in the conversation.
pub const LABEL: &str = "solver";

/// What the solver conversation answers when the message names no issue.
pub const ASK_ISSUE: &str =
    "Which issue should I solve? Give its number, or owner/repo#N for another repository.";

pub const USAGE: &str = "Usage: coder solver N [OPTIONS]

Solves issue N to a pull request in four hands, each a Claude agent with
only the tools it needs: the Intaker checks the environment and reads the
issue, the Briefer runs the code-brief algorithm and writes what the change
should look like, the Coder (Opus) codes, and the Tester runs the checks and
hands back to the Coder or the Briefer when they fail. When the checks pass
and the issue is open, the run opens a pull request.

Options:
  --repo PATH           The repository checkout to read (default: this directory).
  --github OWNER/R      The GitHub repository of the issue (default: OpenAgentsInc/openagents).
  --rounds R            How many times the Tester may hand work back (default 2).
  --no-pr               Stop after the checks; open no pull request.
  --coder-model NAME    The Coder's model (default claude-opus-5-5).
  --briefer-model NAME  The Intaker's and Briefer's model (default haiku).
  --tester-model NAME   The Tester's model (default sonnet).
  --decider WHO         auto, jev, clef, or off for the briefing's questions (default auto).
  --files K             Files in the briefing (default 6).
  --timeout SECS        How long each hand may work (default 1800).
  --plain               Print the run as text instead of opening the terminal screen.
  --json                With --plain: one JSON line per transcript change.
  --snapshot FILE       Also save a picture of the whole run as an SVG file.
  --replay FILE         Play a recorded run (its events.jsonl) again.
  --speed X             Play a recording X times faster (default 1).
  --state DIR           Where runs live (default ~/.openagents/coder-new/solver-runs).";

/// Which hand works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Intaker,
    Briefer,
    Coder,
    Tester,
}

impl Hand {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intaker => "intaker",
            Self::Briefer => "briefer",
            Self::Coder => "coder",
            Self::Tester => "tester",
        }
    }
}

/// The solver's settings: `issue_run`'s, which the shared steps read, plus
/// the hands' models and the hand-back bound.
#[derive(Clone, Debug)]
pub struct Options {
    pub inner: issue_run::Options,
    pub rounds: usize,
    pub json: bool,
    pub briefer_model: String,
    pub tester_model: String,
}

impl Options {
    /// Reads `coder solver`'s arguments.
    ///
    /// # Errors
    /// An option is unknown, lacks its value, or the value is not valid.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut rounds = 2;
        let mut json = false;
        let mut open_pr = true;
        let mut briefer_model = "haiku".to_owned();
        let mut tester_model = "sonnet".to_owned();
        let mut inner_args: Vec<String> =
            vec!["--state".into(), default_state().display().to_string()];
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or(format!("{name} needs a value. Run coder solver --help."))
            };
            match arg.as_str() {
                "--rounds" => {
                    rounds = value("--rounds")?
                        .parse::<usize>()
                        .map_err(|_| "--rounds takes a number.".to_owned())?
                        .min(10);
                }
                "--no-pr" => open_pr = false,
                "--json" => json = true,
                "--coder-model" => {
                    inner_args.push("--model".into());
                    inner_args.push(value("--coder-model")?);
                }
                "--briefer-model" => briefer_model = value("--briefer-model")?,
                "--tester-model" => tester_model = value("--tester-model")?,
                "--model" | "--open-pr" | "--fix" => {
                    return Err(format!("Unknown option {arg}. Run coder solver --help."));
                }
                other => inner_args.push(other.to_owned()),
            }
        }
        let mut inner = issue_run::Options::parse(&inner_args)
            .map_err(|why| why.replace("coder issue-run", "coder solver"))?;
        inner.open_pr = open_pr;
        Ok(Self {
            inner,
            rounds,
            json,
            briefer_model,
            tester_model,
        })
    }
}

/// `~/.openagents/coder-new/solver-runs`.
fn default_state() -> PathBuf {
    model_access::store::openagents_dir()
        .map(|root| root.join("coder-new").join("solver-runs"))
        .unwrap_or_else(|| std::env::temp_dir().join("coder-solver-runs"))
}

/// An issue the solver conversation was given: `N`, `#N`, or
/// `owner/repo#N`. `None` when the text names none.
#[must_use]
pub fn issue_reference(text: &str) -> Option<(Option<String>, u64)> {
    let text = text.trim();
    if let Some((repo, number)) = text.split_once('#') {
        let number = number.trim().parse::<u64>().ok()?;
        let repo = repo.trim();
        if repo.is_empty() {
            return Some((None, number));
        }
        let (owner, name) = repo.split_once('/')?;
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        return Some((Some(repo.to_owned()), number));
    }
    text.parse::<u64>().ok().map(|number| (None, number))
}

/// Starts a run and returns the feed the terminal draws from.
#[must_use]
pub fn start(options: Options) -> Feed {
    let replay = options.inner.replay.clone();
    let speed = options.inner.speed;
    let made = if replay.is_some() {
        Err(String::new())
    } else {
        issue_run::new_run(&options.inner)
    };
    let log = made
        .as_ref()
        .ok()
        .and_then(|run| std::fs::File::create(run.folder.join("events.jsonl")).ok());
    let (feed, mut transcript) = issue_run::channel(log);
    std::thread::spawn(move || {
        if let Some(replay) = replay {
            transcript.replay(&replay, speed);
            return;
        }
        let run = match made {
            Ok(run) => run,
            Err(why) => {
                transcript.notice(format!("The run could not start: {why}"));
                return;
            }
        };
        transcript.busy(true);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => runtime.block_on(drive(&options, &run, &mut transcript)),
            Err(_) => transcript.notice("The run could not start its worker threads."),
        }
        transcript.busy(false);
    });
    feed
}

/// What one hand's session did, for the summary.
#[derive(Clone, Debug)]
pub struct Turn {
    pub hand: Hand,
    pub round: usize,
    pub work: Work,
    /// The Tester's verdict, the Intaker's intake, or nothing.
    pub verdict: Option<String>,
}

impl Turn {
    fn json(&self) -> Value {
        json!({
            "hand": self.hand.as_str(),
            "round": self.round,
            "wall_ms": self.work.wall_ms,
            "turns": self.work.turns,
            "cost_usd": self.work.cost_usd,
            "input_tokens": self.work.input_tokens,
            "output_tokens": self.work.output_tokens,
            "checks": self.work.checks,
            "error": self.work.error,
            "verdict": self.verdict,
        })
    }
}

/// The whole run: intake, briefing, then coding and testing rounds.
async fn drive(options: &Options, run: &issue_run::Run, transcript: &mut Transcript) {
    transcript.push(Entry::User(format!(
        "Solve issue #{} in {} to a pull request.",
        options.inner.issue, options.inner.github
    )));
    let started = Instant::now();
    let folder = run.folder.as_path();
    let mut end = End {
        options,
        run,
        base: None,
        briefing: None,
        turns: Vec::new(),
        checks: Vec::new(),
        rounds: 0,
        started,
    };

    // Intaker.
    let intake = hands::intake(options, folder, transcript).await;
    end.turns.push(intake.clone());
    if let Some(why) = hands::blocked(&intake) {
        return end
            .conclude(Outcome::stopped(Status::SetupFailed, why), transcript)
            .await;
    }
    let Some(base) = setup::prepare(&options.inner, folder, transcript).await else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(Status::Cancelled, "The run was stopped during intake.")
        } else {
            Outcome::stopped(
                Status::SetupFailed,
                "The issue, its base commit, or the worktree could not be prepared.",
            )
        };
        return end.conclude(outcome, transcript).await;
    };
    end.base = Some(base.clone());

    // Briefer.
    let Some(mut briefing) =
        issue_run::decide::decide(&options.inner, &base, folder, transcript).await
    else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(Status::Cancelled, "The run was stopped in the briefing.")
        } else {
            Outcome::stopped(
                Status::DecisionFailed,
                "The code-brief steps stopped before a briefing existed.",
            )
        };
        return end.conclude(outcome, transcript).await;
    };
    let mut notes: Option<String> = None;
    let briefer = hands::brief(
        options,
        &base,
        &mut briefing,
        notes.as_deref(),
        0,
        folder,
        transcript,
    )
    .await;
    end.turns.push(briefer);
    end.briefing = Some(briefing.clone());

    // Coder and Tester, with bounded hand-backs.
    let required = setup::required_checks(&briefing);
    let mut round = 0;
    let outcome = loop {
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        end.rounds = round;
        let coder = hands::code(
            options,
            &base,
            &briefing,
            notes.as_deref(),
            round,
            folder,
            transcript,
        )
        .await;
        let coder_error = coder.work.error.clone();
        end.turns.push(coder);
        if let Some(error) = coder_error {
            break Outcome::stopped(Status::Failed, format!("The Coder stopped: {error}"));
        }
        let tester = hands::test(options, &base, &briefing, round, folder, transcript).await;
        let verdict = hands::Verdict::parse(tester.verdict.as_deref().unwrap_or_default());
        end.turns.push(tester);
        if transcript.cancelled() {
            break Outcome::stopped(Status::Cancelled, "The run was stopped.");
        }
        end.checks = issue_run::agent::final_checks(&base, &briefing, transcript).await;
        let judged = verdict::judge(transcript.cancelled(), None, &required, &end.checks);
        if judged.status == Status::Passed || judged.status == Status::Unchecked {
            break judged;
        }
        let failed: Vec<String> = end
            .checks
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(id, _)| id.clone())
            .collect();
        let back = match verdict {
            hands::Verdict::Pass | hands::Verdict::Coder(_) if failed.is_empty() => {
                hands::Verdict::Coder(judged.reason.clone().unwrap_or_default())
            }
            hands::Verdict::Pass => hands::Verdict::Coder(format!(
                "The harness's final checks failed: {}.",
                failed.join(", ")
            )),
            other => other,
        };
        if round >= options.rounds {
            break Outcome::stopped(
                Status::Failed,
                format!(
                    "{} after {} hand-back{}.",
                    judged.reason.unwrap_or_default().trim_end_matches('.'),
                    options.rounds,
                    if options.rounds == 1 { "" } else { "s" }
                ),
            );
        }
        round += 1;
        match back {
            hands::Verdict::Briefer(text) => {
                transcript.notice(format!("Round {round}: back to the Briefer. {text}"));
                notes = Some(text);
                let briefer = hands::brief(
                    options,
                    &base,
                    &mut briefing,
                    notes.as_deref(),
                    round,
                    folder,
                    transcript,
                )
                .await;
                end.turns.push(briefer);
                end.briefing = Some(briefing.clone());
            }
            hands::Verdict::Coder(text) => {
                transcript.notice(format!("Round {round}: back to the Coder. {text}"));
                notes = Some(text);
            }
            hands::Verdict::Pass => unreachable!("a pass never hands back"),
        }
    };
    end.conclude(outcome, transcript).await;
}

/// What a run has when it ends.
struct End<'a> {
    options: &'a Options,
    run: &'a issue_run::Run,
    base: Option<Base>,
    briefing: Option<Briefing>,
    turns: Vec<Turn>,
    checks: Vec<(String, bool)>,
    rounds: usize,
    started: Instant,
}

impl End<'_> {
    /// Writes the summary, opens the pull request when the run passed and
    /// the options ask, and captures the run as a trace.
    async fn conclude(self, outcome: Outcome, transcript: &mut Transcript) {
        let folder = self.run.folder.as_path();
        let coder = self
            .turns
            .iter()
            .rev()
            .find(|turn| turn.hand == Hand::Coder)
            .map(|turn| turn.work.clone());
        let mut summary = setup::summary(&setup::Report {
            options: &self.options.inner,
            run: self.run,
            base: self.base.as_ref(),
            briefing: self.briefing.as_ref(),
            work: coder.as_ref(),
            checks: &self.checks,
            outcome: &outcome,
            started: self.started,
            folder,
        });
        let url = if self.options.inner.open_pr {
            match &self.base {
                Some(base) => setup::open_pr(&self.options.inner, base, &outcome, transcript).await,
                None => {
                    transcript.notice("No pull request: the run has no working copy.");
                    None
                }
            }
        } else {
            None
        };
        summary["solver"] = json!({
            "hands": self.turns.iter().map(Turn::json).collect::<Vec<_>>(),
            "rounds": self.rounds,
            "cost_usd": hands::total_cost(&self.turns),
            "pull_request": url,
        });
        if self.base.is_some() && self.briefing.is_some() {
            let diff = self
                .base
                .as_ref()
                .map(|base| setup::working_diff(&base.worktree, &base.base))
                .unwrap_or_default();
            transcript.push(Entry::Tool {
                name: issue_run::SUMMARY.into(),
                input: summary.clone(),
                output: json!({"diff": diff}),
                running: false,
            });
        }
        if let Err(why) = setup::write_atomic(
            &folder.join("summary.json"),
            &serde_json::to_vec_pretty(&summary).unwrap_or_default(),
        ) {
            transcript.notice(format!("{why}. The worktree is kept."));
        }
        issue_run::capture(&self.options.inner, folder, transcript).await;
        let worktree = self
            .base
            .as_ref()
            .map(|base| format!(" Working copy: {}.", base.worktree.display()))
            .unwrap_or_default();
        let pr = url
            .map(|url| format!(" Pull request: {url}."))
            .unwrap_or_default();
        transcript.notice(format!(
            "Solver ended: {}{}.{pr}{worktree} Recording: {}.",
            outcome.status.as_str(),
            outcome
                .reason
                .as_deref()
                .map(|why| format!(" ({why})"))
                .unwrap_or_default(),
            folder.join("events.jsonl").display()
        ));
    }
}

#[cfg(test)]
mod tests;
