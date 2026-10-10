//! Coding runs for the person's own API key on this computer (#11080).
//!
//! A request to the OpenAgents API for `openagents/code` with
//! `pay: "mine"` may run here, with this computer's own Codex or Claude
//! Code sign-in (`docs/inference/gateway.md`, section 4, "Own coding
//! capacity"). While sync is on, [`Host`] reports each agent installed
//! here as one subscription account, with how many more runs it can take
//! now, and takes the runs the website hands this computer
//! ([`coder_sync::own_runs`]). Each run is the agent's own headless mode
//! ([`bundled_runtime::acp`]) in the folder Coder was opened in, on the
//! sign-in the agent already holds here; its progress lines and answer go
//! back as it runs, and it stops when the caller leaves.
//!
//! - **How many at once.** One run per account by default; set
//!   `CODER_OWN_RUN_SESSIONS` to take more. An account whose run ended on a
//!   usage limit offers none for [`LIMIT_REST`].
//! - **Off.** `CODER_OWN_RUNS=off` offers nothing.
//! - **Only the owner's.** The website hands runs out under this
//!   computer's own sign-in, and the gateway starts them only for that
//!   account's own key. Never anyone else's request.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use coder_sync::Answer;
use coder_sync::own_runs::{self as wire, Account, Agent, End, Run};
use openagents_login::Saved;
use serde_json::Value;

use crate::bundled_runtime::{self, AcpAgent, RuntimeEvent};

/// How often this computer reports its accounts and takes runs.
const TAKE_EVERY: Duration = Duration::from_secs(5);
/// How often, against a website without these routes, it asks again.
const QUIET_EVERY: Duration = Duration::from_secs(120);
/// How often a run's new progress goes up.
const REPORT_EVERY: Duration = Duration::from_secs(2);
/// A run with nothing new still reports this often, to hear of a cancel.
const PING_EVERY: Duration = Duration::from_secs(6);
/// How long an account that hit its usage limit offers no runs.
pub const LIMIT_REST: Duration = Duration::from_secs(30 * 60);
/// The longest task handed to an agent, in bytes (the runtime's limit is
/// 64 KiB).
const MAX_TASK: usize = 60 * 1024;
/// The longest progress line, in characters.
const LINE_CHARS: usize = 200;

/// What the host tells the terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Started { agent: Agent },
    Finished { agent: Agent, done: bool },
}

/// What the terminal shows for `event`.
#[must_use]
pub(crate) fn notice(event: &Event) -> String {
    match event {
        Event::Started { agent } => format!(
            "Running a coding request from your API key with {} here.",
            agent.label()
        ),
        Event::Finished { agent, done: true } => format!(
            "Finished a coding request from your API key with {}.",
            agent.label()
        ),
        Event::Finished { agent, done: false } => format!(
            "A coding request from your API key with {} stopped.",
            agent.label()
        ),
    }
}

/// Whether the person turned these runs off.
fn off() -> bool {
    std::env::var("CODER_OWN_RUNS").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

/// How many runs each account takes at once.
fn sessions() -> u32 {
    std::env::var("CODER_OWN_RUN_SESSIONS")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .min(16)
}

/// One agent installed here, offered as one account.
#[derive(Clone)]
struct Offered {
    id: &'static str,
    agent: Agent,
    runner: AcpAgent,
}

/// The agents installed here: Codex and Claude Code, each on the sign-in
/// it already holds.
fn installed() -> Vec<Offered> {
    let mut offered = Vec::new();
    let found = crate::acp_discovery::discover(&|name| std::env::var_os(name));
    if let Some(codex) = found
        .into_iter()
        .find(|agent| agent.id == "codex" && agent.enabled)
    {
        offered.push(Offered {
            id: "codex",
            agent: Agent::Codex,
            runner: codex,
        });
    }
    if let Some(claude) = bundled_runtime::claude_print::agent() {
        offered.push(Offered {
            id: "claude-code",
            agent: Agent::ClaudeCode,
            runner: claude,
        });
    }
    offered
}

/// What runs now, per account, and which accounts rest after a limit.
#[derive(Default)]
struct Load {
    running: BTreeMap<String, u32>,
    resting: BTreeMap<String, Instant>,
}

/// Takes runs for this computer while it lives; dropping it stops taking
/// and stops the runs going.
pub(crate) struct Host {
    stop: Arc<AtomicBool>,
    events: mpsc::Receiver<Event>,
}

impl Host {
    /// Start taking runs as `saved`, for `computer`, run in `cwd`.
    pub(crate) fn start(saved: Saved, computer: String, cwd: Option<PathBuf>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (outbox, events) = mpsc::channel();
        if !off() {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let cwd = cwd
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("."));
                serve(&saved, &computer, &cwd, &stop, &outbox);
            });
        }
        Self { stop, events }
    }

    /// What happened since the last call.
    pub(crate) fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn accounts(offered: &[Offered], load: &Mutex<Load>) -> Vec<Account> {
    let most = sessions();
    let Ok(mut load) = load.lock() else {
        return Vec::new();
    };
    let now = Instant::now();
    load.resting.retain(|_, until| *until > now);
    offered
        .iter()
        .map(|offer| {
            let running = load.running.get(offer.id).copied().unwrap_or(0);
            let free = if load.resting.contains_key(offer.id) {
                0
            } else {
                most.saturating_sub(running)
            };
            Account {
                id: offer.id.to_owned(),
                label: offer.agent.label().to_owned(),
                agent: offer.agent,
                free_sessions: free,
            }
        })
        .collect()
}

fn serve(
    saved: &Saved,
    computer: &str,
    cwd: &std::path::Path,
    stop: &Arc<AtomicBool>,
    outbox: &mpsc::Sender<Event>,
) {
    let offered = installed();
    if offered.is_empty() {
        return;
    }
    let (Some(http), Ok(runtime)) = (
        wire::client(),
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build(),
    ) else {
        return;
    };
    let load = Arc::new(Mutex::new(Load::default()));
    while !stop.load(Ordering::SeqCst) {
        let offer = accounts(&offered, &load);
        let wait = match runtime.block_on(wire::take(&http, saved, computer, &offer)) {
            Ok(runs) => {
                for run in runs {
                    let Some(chosen) = offered
                        .iter()
                        .find(|offer| offer.id == run.account && offer.agent == run.agent)
                        .cloned()
                    else {
                        // Not an account here: say so, so the caller hears.
                        let _ = runtime.block_on(wire::report(
                            &http,
                            saved,
                            computer,
                            &run.id,
                            &[],
                            Some(&End::Failed {
                                why: "That account isn't on this computer now.".into(),
                                limited: false,
                            }),
                        ));
                        continue;
                    };
                    begin(&load, chosen.id);
                    let job = Job {
                        saved: saved.clone(),
                        computer: computer.to_owned(),
                        cwd: cwd.to_path_buf(),
                        run,
                        offer: chosen,
                        load: load.clone(),
                        stop: stop.clone(),
                        outbox: outbox.clone(),
                    };
                    std::thread::spawn(move || job.go());
                }
                TAKE_EVERY
            }
            Err(Answer::SignedOut) => return,
            Err(Answer::Unknown) => QUIET_EVERY,
            Err(_) => TAKE_EVERY * 2,
        };
        let until = Instant::now() + wait;
        while Instant::now() < until && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

fn begin(load: &Mutex<Load>, account: &str) {
    if let Ok(mut load) = load.lock() {
        *load.running.entry(account.to_owned()).or_default() += 1;
    }
}

fn end(load: &Mutex<Load>, account: &str, limited: bool) {
    if let Ok(mut load) = load.lock() {
        if let Some(running) = load.running.get_mut(account) {
            *running = running.saturating_sub(1);
        }
        if limited {
            load.resting
                .insert(account.to_owned(), Instant::now() + LIMIT_REST);
        }
    }
}

/// A progress line for `event`, for people; `None` for most events.
fn line_of(event: &RuntimeEvent) -> Option<String> {
    let text = match event {
        RuntimeEvent::Tool {
            name,
            running: true,
            ..
        } => format!("Using {}.", name.trim()),
        RuntimeEvent::Progress { step, complete } => RuntimeEvent::progress_line(*step, *complete),
        RuntimeEvent::Delegation { event, .. } => return line_of(event),
        _ => return None,
    };
    let text: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(LINE_CHARS)
        .collect();
    (!text.is_empty()).then_some(text)
}

/// The answer's token counts, when the agent reported them.
fn tokens(value: &Value) -> (Option<u64>, Option<u64>) {
    let usage = &value["usage"];
    let read = |keys: &[&str]| keys.iter().find_map(|key| usage[*key].as_u64());
    (
        read(&["input_tokens", "inputTokens", "prompt_tokens"]),
        read(&["output_tokens", "outputTokens", "completion_tokens"]),
    )
}

/// How a run's result ends it.
fn ending(result: Result<Value, String>, canceled: bool) -> End {
    match result {
        Ok(value) => {
            let (input_tokens, output_tokens) = tokens(&value);
            End::Done {
                text: value["reply"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
                input_tokens,
                output_tokens,
            }
        }
        Err(_) if canceled => End::Failed {
            why: "The run was cancelled.".into(),
            limited: false,
        },
        Err(why) => End::Failed {
            limited: coder_delegate::limit::says_limited(&why),
            why,
        },
    }
}

/// One run, on its own thread.
struct Job {
    saved: Saved,
    computer: String,
    cwd: PathBuf,
    run: Run,
    offer: Offered,
    load: Arc<Mutex<Load>>,
    stop: Arc<AtomicBool>,
    outbox: mpsc::Sender<Event>,
}

impl Job {
    fn go(self) {
        let agent = self.run.agent;
        let _ = self.outbox.send(Event::Started { agent });
        let end_state = self.drive();
        let limited = matches!(end_state, End::Failed { limited: true, .. });
        end(&self.load, self.offer.id, limited);
        let _ = self.outbox.send(Event::Finished {
            agent,
            done: matches!(end_state, End::Done { .. }),
        });
    }

    fn drive(&self) -> End {
        let (Some(http), Ok(runtime)) = (
            wire::client(),
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build(),
        ) else {
            return End::Failed {
                why: "Coder couldn't start the run.".into(),
                limited: false,
            };
        };
        let Some(task) = self.run.brief.text(MAX_TASK) else {
            let end = End::Failed {
                why: "The task is longer than Coder takes (60 KiB).".into(),
                limited: false,
            };
            self.finish(&runtime, &http, &[], &end);
            return end;
        };
        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let work = {
            let lines = lines.clone();
            let cancel = cancel.clone();
            let finished = finished.clone();
            let runner = self.offer.runner.clone();
            let cwd = self.cwd.clone();
            async move {
                let mut last: Option<String> = None;
                let mut emit = |event: RuntimeEvent| {
                    if let Some(line) = line_of(&event)
                        && last.as_deref() != Some(line.as_str())
                        && let Ok(mut lines) = lines.lock()
                    {
                        lines.push(line.clone());
                        last = Some(line);
                    }
                };
                let result =
                    bundled_runtime::acp(&runner, &task, &cwd, None, &cancel, &mut emit).await;
                finished.store(true, Ordering::SeqCst);
                result
            }
        };
        let reports = async {
            let mut sent_at = Instant::now();
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if finished.load(Ordering::SeqCst) {
                    return;
                }
                if self.stop.load(Ordering::SeqCst) {
                    cancel.store(true, Ordering::SeqCst);
                }
                let pending = lines.lock().map(|lines| lines.len()).unwrap_or(0);
                let due = (pending > 0 && sent_at.elapsed() >= REPORT_EVERY)
                    || sent_at.elapsed() >= PING_EVERY;
                if !due {
                    continue;
                }
                let batch: Vec<String> = lines
                    .lock()
                    .map(|mut lines| std::mem::take(&mut *lines))
                    .unwrap_or_default();
                sent_at = Instant::now();
                match wire::report(
                    &http,
                    &self.saved,
                    &self.computer,
                    &self.run.id,
                    &batch,
                    None,
                )
                .await
                {
                    Ok(true) | Err(Answer::Unknown | Answer::SignedOut | Answer::Deleted) => {
                        cancel.store(true, Ordering::SeqCst);
                    }
                    Ok(false) => {}
                    // Not reached: keep the lines for the next report.
                    Err(_) => {
                        if let Ok(mut lines) = lines.lock() {
                            let newer = std::mem::take(&mut *lines);
                            *lines = batch;
                            lines.extend(newer);
                        }
                    }
                }
            }
        };
        let (result, ()) = runtime.block_on(async { tokio::join!(work, reports) });
        let end = ending(result, cancel.load(Ordering::SeqCst));
        let rest: Vec<String> = lines
            .lock()
            .map(|mut lines| std::mem::take(&mut *lines))
            .unwrap_or_default();
        self.finish(&runtime, &http, &rest, &end);
        end
    }

    /// The last report: the lines left and how it ended, tried a few times.
    fn finish(
        &self,
        runtime: &tokio::runtime::Runtime,
        http: &reqwest::Client,
        lines: &[String],
        end: &End,
    ) {
        for attempt in 0..5u32 {
            let sent = runtime.block_on(wire::report(
                http,
                &self.saved,
                &self.computer,
                &self.run.id,
                lines,
                Some(end),
            ));
            match sent {
                Ok(_) | Err(Answer::Unknown | Answer::SignedOut | Answer::Refused(_)) => return,
                Err(_) => std::thread::sleep(Duration::from_secs(2u64.pow(attempt))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progress_lines_come_from_tools_and_steps_only() {
        let tool = RuntimeEvent::Tool {
            name: "shell".into(),
            input: json!({"command": "cargo test"}),
            output: Value::Null,
            running: true,
        };
        assert_eq!(line_of(&tool).as_deref(), Some("Using shell."));
        let finished = RuntimeEvent::Tool {
            name: "shell".into(),
            input: Value::Null,
            output: Value::Null,
            running: false,
        };
        assert_eq!(line_of(&finished), None);
        assert_eq!(line_of(&RuntimeEvent::Text("hello".into())), None);
        assert_eq!(
            line_of(&RuntimeEvent::Progress {
                step: 3,
                complete: None
            })
            .as_deref(),
            Some("step 3")
        );
    }

    #[test]
    fn a_result_becomes_the_answer_or_why_it_stopped() {
        let done = ending(
            Ok(
                json!({"reply": " Opened the PR. ", "usage": {"input_tokens": 10, "output_tokens": 4}}),
            ),
            false,
        );
        assert_eq!(
            done,
            End::Done {
                text: "Opened the PR.".into(),
                input_tokens: Some(10),
                output_tokens: Some(4),
            }
        );
        let limited = ending(
            Err("Codex reached a usage limit or rate limit.".into()),
            false,
        );
        assert!(matches!(limited, End::Failed { limited: true, .. }));
        let cancelled = ending(Err("The ACP task was canceled.".into()), true);
        assert!(matches!(cancelled, End::Failed { limited: false, .. }));
    }

    #[test]
    fn free_sessions_count_running_runs_and_rest_after_a_limit() {
        let offered = vec![Offered {
            id: "codex",
            agent: Agent::Codex,
            runner: AcpAgent {
                id: "codex".into(),
                name: "Codex".into(),
                program: PathBuf::from("/bin/true"),
                transport: bundled_runtime::AgentTransport::CodexCli,
                arguments: Vec::new(),
                mode: None,
                enabled: true,
            },
        }];
        let load = Mutex::new(Load::default());
        assert_eq!(accounts(&offered, &load)[0].free_sessions, sessions());
        begin(&load, "codex");
        assert_eq!(
            accounts(&offered, &load)[0].free_sessions,
            sessions().saturating_sub(1)
        );
        end(&load, "codex", true);
        assert_eq!(accounts(&offered, &load)[0].free_sessions, 0);
    }
}
