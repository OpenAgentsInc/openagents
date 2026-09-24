//! One short Luna session: a rebuilt context, a tool loop, and its record.
//!
//! # The stable prefix comes first
//!
//! The provider caches the longest prefix two requests share, and Luna's
//! cached input costs a tenth of its uncached input. So the input is laid
//! out from most to least stable:
//!
//! 1. [`INSTRUCTIONS`], the same for every session of every task.
//! 2. The tool declarations, the same for every session.
//! 3. The brief's task, guidance, and evidence, the same for every session
//!    of one requirement.
//! 4. The brief's state: what earlier sessions changed and what the checks
//!    say now. This is the part code and Jev rebuild each session.
//! 5. This session's own turns, which only grow.
//!
//! Sessions of one task share [`Config::cache_key`], so the provider can
//! route them to the same cache.
//!
//! # The record
//!
//! Every reply and every tool call becomes an ATIF step, as it happens.
//! A reply's step carries its tokens and, under [`USAGE_EXTENSION`], the
//! cached and reasoning tokens and the list-price cost.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::price;
use crate::tools::{self, Finish, Workspace};
use crate::transport::{Request, TokenUsage, Transport};

/// The instructions every session starts with.
///
/// They hold facts and constraints only. Whether the task is done is the
/// host's call, made from its own checks where it has them, so nothing
/// here asks the model to certify completion. Issue #9591 and
/// `docs/coder/design/prompt-audit.md` record each line's purpose.
pub const INSTRUCTIONS: &str = "You are a coding agent working in one \
workspace directory. Act only through the tools: read_file reads a region of a file, \
run_command runs searches, builds, and tests, apply_patch edits existing files, and \
write_file creates new ones. A patch's context lines must match the file as it is now, so \
read a region before you patch it. Keep each change as small as the task allows, and run \
something that exercises it when you can. Call finish once to end this session, with a \
typed status, a short summary of what you changed and what you ran, and the answer if the \
task asked a question.";

/// The step extension that holds a reply's full usage and cost.
pub const USAGE_EXTENSION: &str = "microluna.usage.v1";

/// How many times a request that failed transiently is sent again.
pub const RETRIES: u32 = 2;

/// The wait before a retry, times the retry's number.
pub const RETRY_WAIT: Duration = Duration::from_secs(3);

/// What the model is told when it answers without calling a tool.
pub const NUDGE: &str = "Continue with a tool call. When you're done, call finish.";

/// A piece of evidence for the task: a file region, a data record, or an
/// example, with a label that says what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    /// What the evidence is, such as `src/main.rs lines 10-40`.
    pub label: String,
    /// The evidence itself.
    pub text: String,
}

/// What one session is given, from most to least stable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Brief {
    /// The task or requirement, in words.
    pub task: String,
    /// Guidance code or Jev chose for this task, if any.
    pub guidance: String,
    /// The evidence that decides the task.
    pub evidence: Vec<Evidence>,
    /// What's true now: earlier sessions' changes and check results.
    pub state: Vec<String>,
}

impl Brief {
    /// A brief with only a task.
    #[must_use]
    pub fn task(task: &str) -> Self {
        Brief {
            task: task.to_string(),
            ..Brief::default()
        }
    }

    /// The input items the brief becomes: the stable message, then the
    /// state message when there is state.
    #[must_use]
    pub fn input(&self) -> Vec<Value> {
        let mut stable = format!("# Task\n\n{}\n", self.task.trim());
        if !self.guidance.trim().is_empty() {
            stable.push_str(&format!("\n# Guidance\n\n{}\n", self.guidance.trim()));
        }
        if !self.evidence.is_empty() {
            stable.push_str("\n# Evidence\n");
            for evidence in &self.evidence {
                stable.push_str(&format!(
                    "\n## {}\n\n{}\n",
                    evidence.label.trim(),
                    evidence.text.trim_end()
                ));
            }
        }
        let mut input = vec![user(&stable)];
        if !self.state.is_empty() {
            let mut state = String::from("# Current state\n");
            for line in &self.state {
                state.push_str(&format!("\n- {}", line.trim()));
            }
            input.push(user(&state));
        }
        input
    }
}

fn user(text: &str) -> Value {
    json!({
        "type": "message",
        "role": "user",
        "content": [{ "type": "input_text", "text": text }],
    })
}

/// How a session is run.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The model slug.
    pub model: String,
    /// The reasoning effort, or `None` for the provider's default.
    pub effort: Option<String>,
    /// The most model requests one session makes.
    pub max_turns: usize,
    /// The prompt-cache key the task's sessions share.
    pub cache_key: String,
    /// The session's wall-time bound, or `None` for no bound. It is
    /// checked before each request, and a request waits no longer than
    /// what is left of it.
    pub deadline: Option<Duration>,
    /// The reasoning effort before the session's first edit, when set:
    /// orienting and reading turns spend less, and `effort` holds from the
    /// first `apply_patch` or `write_file` on. The effort is set per
    /// request, so no request is sent twice.
    pub orient_effort: Option<String>,
    /// Let the model call several tools in one turn. When every call in a
    /// turn only reads ([`crate::tools::reads_only`]), they run at the same
    /// time; otherwise in order.
    pub parallel_tools: bool,
    /// When the host turns a `finish` back and the session keeps working,
    /// or `None` to let every finish stand.
    pub persist: Option<Persist>,
    /// The session's spend bound in dollars, at list price: the session
    /// ends before a request once it has spent this much. `None` for no
    /// bound.
    pub spend_usd: Option<f64>,
}

/// When the host turns a session's `finish` back. The model decides when
/// it is done least reliably of all: sessions on hard tasks finished with
/// most of their turns unused, at a partial result their own checks
/// measured. The host decides instead, from a program state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persist {
    /// The most finishes the host turns back in one session.
    pub max_returns: u32,
    /// Turn back a finish whose status isn't `done`, unless its cause is a
    /// missing tool or missing information.
    pub not_done: bool,
    /// A shell command the host runs in the workspace on each finish. When
    /// it prints a last `SCORE <passed> <total>` line with `passed` below
    /// `total`, the finish is turned back.
    pub score_command: Option<String>,
    /// A finish is turned back only while at least this many turns and
    /// this many seconds of the session are left.
    pub reserve_turns: usize,
    pub reserve_sec: u64,
}

/// The last `SCORE <passed> <total>` line in `output`, with `total` above 0.
#[must_use]
pub fn parse_score(output: &str) -> Option<(u64, u64)> {
    output.lines().rev().find_map(|line| {
        let mut words = line.split_whitespace();
        (words.next()? == "SCORE").then_some(())?;
        let passed: u64 = words.next()?.parse().ok()?;
        let total: u64 = words.next()?.parse().ok()?;
        (total > 0).then_some((passed.min(total), total))
    })
}

impl Config {
    /// Luna, the provider's default effort, 24 turns, `cache_key`, and no
    /// wall-time bound.
    #[must_use]
    pub fn luna(cache_key: &str) -> Self {
        Config {
            model: "gpt-6-luna".to_string(),
            effort: None,
            max_turns: 24,
            cache_key: cache_key.to_string(),
            deadline: None,
            orient_effort: None,
            parallel_tools: false,
            persist: None,
            spend_usd: None,
        }
    }
}

/// How a session ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The model called `finish`.
    Finished,
    /// The model stopped calling tools, even after a nudge.
    Stopped,
    /// The session used every turn it had.
    TurnLimit,
    /// The wall-time bound passed before the session finished. A request
    /// the bound cut off may have been billed without reporting usage.
    Deadline,
    /// A request got no reply; the text says why.
    Transport(String),
}

/// What one session did and cost.
#[derive(Clone, Debug)]
pub struct Report {
    /// How it ended.
    pub ending: Ending,
    /// The model's typed finish, when it called `finish`.
    pub finish: Option<Finish>,
    /// Model requests made.
    pub turns: usize,
    /// Tool calls run.
    pub calls: usize,
    /// Tokens across every request.
    pub usage: TokenUsage,
    /// The list-price cost, or `None` for a model with no known price.
    pub cost_usd: Option<f64>,
    /// Wall time for the whole session.
    pub milliseconds: u64,
}

/// A host's own handler for each step, such as its trajectory.
pub type Sink = Box<dyn FnMut(&atif::Step)>;

/// Where a session's steps go: kept in memory, appended to an ATIF log
/// when one is open, and echoed to standard error when asked.
#[derive(Default)]
pub struct Recorder {
    steps: Vec<atif::Step>,
    log: Option<atif::Log>,
    echo: bool,
    faults: Vec<String>,
    sink: Option<Sink>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("steps", &self.steps.len())
            .field("log", &self.log.as_ref().map(atif::Log::path))
            .field("echo", &self.echo)
            .field("faults", &self.faults)
            .field("sink", &self.sink.is_some())
            .finish()
    }
}

impl Recorder {
    /// A recorder that keeps steps in memory only.
    #[must_use]
    pub fn new() -> Self {
        Recorder::default()
    }

    /// Also appends every step to `log`.
    #[must_use]
    pub fn logging(mut self, log: atif::Log) -> Self {
        self.log = Some(log);
        self
    }

    /// Also prints a line per step to standard error.
    #[must_use]
    pub fn echoing(mut self) -> Self {
        self.echo = true;
        self
    }

    /// Also hands every step to `sink` as it is recorded, such as a
    /// host's own trajectory.
    #[must_use]
    pub fn forwarding(mut self, sink: impl FnMut(&atif::Step) + 'static) -> Self {
        self.sink = Some(Box::new(sink));
        self
    }

    /// Records one step.
    pub fn record(&mut self, step: atif::Step) {
        if self.echo {
            eprintln!("{}", line(&step));
        }
        if let Some(sink) = &mut self.sink {
            sink(&step);
        }
        if let Some(log) = &mut self.log
            && let Err(error) = log.append(&step)
        {
            self.faults
                .push(format!("the trace stopped recording: {error}"));
            self.log = None;
        }
        self.steps.push(step);
    }

    /// The steps recorded so far.
    #[must_use]
    pub fn steps(&self) -> &[atif::Step] {
        &self.steps
    }

    /// The log's path, when one is open.
    #[must_use]
    pub fn path(&self) -> Option<&std::path::Path> {
        self.log.as_ref().map(atif::Log::path)
    }

    /// Problems writing the log, which never stop a session.
    #[must_use]
    pub fn faults(&self) -> &[String] {
        &self.faults
    }

    /// Closes the log with `state`, such as [`atif::log::ENDED`].
    pub fn close(&mut self, state: &str) {
        if let Some(log) = &mut self.log
            && let Err(error) = log.finish(state)
        {
            self.faults.push(format!("the trace didn't close: {error}"));
        }
    }
}

/// One step as a line of text, as `echoing` prints it.
#[must_use]
pub fn line(step: &atif::Step) -> String {
    if let Some(call) = &step.call {
        let arguments = call.arguments.to_string();
        return format!(
            "  {} {} ({} ms, {}) -> {}",
            call.name,
            clip(&arguments, 160),
            call.milliseconds,
            call.outcome.word(),
            clip(&call.output.replace('\n', " | "), 200)
        );
    }
    let usage = step
        .extensions
        .get(USAGE_EXTENSION)
        .map(|usage| {
            format!(
                " [in {} (cached {}), out {}, ${:.6}]",
                usage["input"],
                usage["cached"],
                usage["output"],
                usage["cost_usd"].as_f64().unwrap_or_default()
            )
        })
        .unwrap_or_default();
    format!(
        "{}: {}{usage}",
        step.source.word(),
        clip(&step.message.replace('\n', " "), 200)
    )
}

fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// Runs one session to its end.
///
/// A transport failure ends the session with [`Ending::Transport`] rather
/// than an error, so the usage already spent is still reported.
pub async fn run<T: Transport>(
    transport: &T,
    workspace: &Workspace,
    brief: &Brief,
    config: &Config,
    recorder: &mut Recorder,
) -> Report {
    let started = Instant::now();
    let mut input = brief.input();
    for item in &input {
        let text = item["content"][0]["text"].as_str().unwrap_or_default();
        recorder.record(atif::Step::said(atif::Source::User, text));
    }
    let tools = tools::declarations();
    let mut report = Report {
        ending: Ending::TurnLimit,
        finish: None,
        turns: 0,
        calls: 0,
        usage: TokenUsage::default(),
        cost_usd: price::rates(&config.model).map(|_| 0.0),
        milliseconds: 0,
    };
    let mut nudged = false;
    let mut edited = false;
    let mut returned = 0u32;
    'turns: while report.turns < config.max_turns {
        if let (Some(bound), Some(spent)) = (config.spend_usd, report.cost_usd)
            && spent >= bound
        {
            recorder.record(atif::Step::said(
                atif::Source::System,
                &format!("The session's spend bound (${bound:.4}) was reached."),
            ));
            report.ending = Ending::TurnLimit;
            break;
        }
        let left = config
            .deadline
            .map(|deadline| deadline.saturating_sub(started.elapsed()));
        if left.is_some_and(|left| left.is_zero()) {
            recorder.record(atif::Step::said(
                atif::Source::System,
                "The session's time bound passed.",
            ));
            report.ending = Ending::Deadline;
            break;
        }
        let request = Request {
            model: config.model.clone(),
            instructions: INSTRUCTIONS.to_string(),
            input: input.clone(),
            tools: tools.clone(),
            effort: if edited {
                config.effort.clone()
            } else {
                config
                    .orient_effort
                    .clone()
                    .or_else(|| config.effort.clone())
            },
            cache_key: config.cache_key.clone(),
            parallel_tools: config.parallel_tools,
        };
        let asked = Instant::now();
        let mut tries = 0;
        let answered = loop {
            let answered = match config
                .deadline
                .map(|deadline| deadline.saturating_sub(started.elapsed()))
            {
                Some(left) => match tokio::time::timeout(left, transport.respond(&request)).await {
                    Ok(answered) => answered,
                    Err(_) => {
                        recorder.record(atif::Step::said(
                            atif::Source::System,
                            "The session's time bound passed while a request was open; its \
                             usage is unknown.",
                        ));
                        report.ending = Ending::Deadline;
                        break 'turns;
                    }
                },
                None => transport.respond(&request).await,
            };
            // A broken stream or a provider's transient refusal usually
            // clears on a second try; the request is sent again whole, and
            // a failed one reported no usage to count.
            match answered {
                Err(error) if error.transient() && tries < RETRIES => {
                    tries += 1;
                    recorder.record(atif::Step::said(
                        atif::Source::System,
                        &format!(
                            "The request failed ({error}); trying again, {tries} of {RETRIES}."
                        ),
                    ));
                    tokio::time::sleep(RETRY_WAIT * tries).await;
                }
                answered => break answered,
            }
        };
        let reply = match answered {
            Ok(reply) => reply,
            Err(error) => {
                recorder.record(atif::Step::said(
                    atif::Source::System,
                    &format!("The request failed: {error}"),
                ));
                report.ending = Ending::Transport(error.to_string());
                break;
            }
        };
        report.turns += 1;
        report.usage.add(reply.usage);
        let model = if reply.model.is_empty() {
            config.model.as_str()
        } else {
            reply.model.as_str()
        };
        let cost = price::cost(model, reply.usage);
        report.cost_usd = match (report.cost_usd, cost) {
            (Some(total), Some(cost)) => Some(total + cost),
            _ => None,
        };
        let mut step = atif::Step::said(atif::Source::Agent, &reply.text())
            .by(model)
            .taking(elapsed(asked))
            .noting(
                USAGE_EXTENSION,
                json!({
                    "input": reply.usage.input,
                    "cached": reply.usage.cached,
                    "output": reply.usage.output,
                    "reasoning": reply.usage.reasoning,
                    "cost_usd": cost,
                    "cost_note": price::COST_NOTE,
                    "response_id": reply.id,
                }),
            );
        let reasoning = reply.reasoning();
        if !reasoning.is_empty() {
            step.reasoning = Some(reasoning);
        }
        step.spent(atif::Usage {
            prompt: reply.usage.input,
            completion: reply.usage.output,
        });
        recorder.record(step);

        input.extend(reply.items.iter().cloned());
        let calls = reply.calls();
        if calls.is_empty() {
            if nudged {
                report.ending = Ending::Stopped;
                break;
            }
            nudged = true;
            input.push(user(NUDGE));
            recorder.record(atif::Step::said(atif::Source::System, NUDGE));
            continue;
        }
        // Reads the model asked for together run together; anything that
        // writes runs in order.
        let together = config.parallel_tools
            && calls.len() > 1
            && calls
                .iter()
                .all(|c| crate::tools::reads_only(&c.name, &c.arguments));
        let mut outcomes = if together {
            futures_util::future::join_all(
                calls.iter().map(|c| workspace.call(&c.name, &c.arguments)),
            )
            .await
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>()
        } else {
            vec![None; calls.len()]
        };
        for (index, call) in calls.into_iter().enumerate() {
            let outcome = match outcomes[index].take() {
                Some(outcome) => outcome,
                None => workspace.call(&call.name, &call.arguments).await,
            };
            if matches!(call.name.as_str(), "apply_patch" | "write_file")
                && outcome.status == atif::Outcome::Completed
            {
                edited = true;
            }
            report.calls += 1;
            let arguments = serde_json::from_str(&call.arguments)
                .unwrap_or_else(|_| Value::String(call.arguments.clone()));
            recorder.record(atif::Step::called(atif::Call {
                id: call.call_id.clone(),
                name: call.name.clone(),
                arguments,
                output: outcome.output.clone(),
                outcome: outcome.status,
                milliseconds: outcome.milliseconds,
                purpose: None,
                extra: outcome.extra.clone(),
            }));
            if let Some(finish) = &outcome.finish
                && let Some(why) = turn_back(
                    config,
                    workspace,
                    finish,
                    returned,
                    report.turns,
                    started.elapsed(),
                )
                .await
            {
                returned += 1;
                recorder.record(atif::Step::said(atif::Source::System, &why));
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": call.call_id,
                    "output": why,
                }));
                continue;
            }
            input.push(json!({
                "type": "function_call_output",
                "call_id": call.call_id,
                "output": outcome.output,
            }));
            if let Some(finish) = outcome.finish {
                report.finish = Some(finish);
                report.ending = Ending::Finished;
                break 'turns;
            }
        }
    }
    report.milliseconds = elapsed(started);
    report
}

/// Why the host turns `finish` back, or `None` to let it stand.
async fn turn_back(
    config: &Config,
    workspace: &Workspace,
    finish: &tools::Finish,
    returned: u32,
    turns: usize,
    spent: Duration,
) -> Option<String> {
    let persist = config.persist.as_ref()?;
    let turns_left = config.max_turns.saturating_sub(turns);
    let time_left = config
        .deadline
        .map_or(u64::MAX, |d| d.saturating_sub(spent).as_secs());
    if returned >= persist.max_returns
        || turns_left < persist.reserve_turns
        || time_left < persist.reserve_sec
    {
        return None;
    }
    let mut reasons = Vec::new();
    if persist.not_done
        && finish.status != tools::FinishStatus::Done
        && !matches!(
            finish.cause,
            tools::Cause::MissingTool | tools::Cause::NeedsInformation
        )
    {
        reasons.push(format!(
            "you finished as {} with {turns_left} turns left, and a hard task isn't a reason to \
             stop",
            serde_json::to_value(finish.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        ));
    }
    if let Some(command) = &persist.score_command {
        let ran = workspace
            .call(
                "run_command",
                &json!({ "command": command, "timeout_seconds": 120 }).to_string(),
            )
            .await;
        if let Some((passed, total)) = parse_score(&ran.output)
            && passed < total
        {
            reasons.push(format!(
                "the evaluation script scores the workspace {passed} of {total}"
            ));
        }
    }
    if reasons.is_empty() {
        return None;
    }
    Some(format!(
        "The host turned this finish back: {}. Keep working with the turns you have. Look at \
         what still fails, find the general cause, and try a different approach from the one \
         that stalled; measure after each change. Call finish again when the task is met or \
         your turns are nearly spent.",
        reasons.join("; and ")
    ))
}

fn elapsed(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}
