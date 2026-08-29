//! The session's own slash commands.
//!
//! [`COMMANDS`] is one list read by three things — `/help`, Tab completion,
//! and the dispatch in [`run`] — so a command cannot be listed without being
//! handled, and `every_listed_command_is_handled` in
//! [`crate::coder::interactive`] fails if one ever is. A bare, unknown command
//! token is refused so a mistyped `/diff` gets a useful answer. Slash-prefixed
//! paths and messages go to the model.
//!
//! None of these reaches a model. They run here, print into the transcript as
//! [`Role::Output`], and are exported as notices rather than as model steps.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use crate::tools::{OUTPUT_LIMIT, check_shell_refusal};

use crate::coder::runtime::Control;
use crate::coder::tui::{CoderUi, Entry, Role, ToolCall};

/// The commands, and what each one does. The second column is what `/help`
/// prints, so it says what the command actually does and nothing more.
pub const COMMANDS: &[(&str, &str)] = &[
    ("clear", "clear the transcript"),
    (
        "diff",
        "what changed since HEAD: /diff, /diff --staged, /diff <path>…",
    ),
    (
        "export",
        "write the transcript to ~/.openagents/exports as an ATIF document",
    ),
    ("help", "list these commands and the keys"),
    ("goal", "set, inspect, or manage the task goal"),
    (
        "model",
        "pick the model on Pro or Local: /model opens the picker, /model <id> commits",
    ),
    (
        "autopilot",
        "AFK mode: the loop keeps steering between turns: Meta+A, /autopilot [directive], /autopilot off",
    ),
    (
        "info",
        "what this session has spent: tokens, model, lane, thread",
    ),
    ("login", "sign in to OpenAgents and store the token"),
    (
        "logout",
        "end this session's thread and remove the stored token",
    ),
    (
        "queue",
        "inspect or clear waiting prompts: /queue, /queue clear",
    ),
    (
        "resume",
        "coding-agent sessions other tools left on this machine: /resume, /resume <number>",
    ),
    (
        "continue",
        "new session from a checkpoint, no transcript replay: /continue, /continue last, /continue <id>",
    ),
    (
        "run",
        "run a command here and show its output: /run cargo test",
    ),
    (
        "swarm",
        "the local swarm: /swarm list, /swarm tree, /swarm inbox [id], /swarm send <id> <text>, /swarm mute <id>",
    ),
    (
        "gym",
        "show or hide the gym pane: /gym, /gym <suite>, /gym run <id-or-json>, /gym close",
    ),
    ("load", "load a local GGUF in this process: /load <path>"),
    ("unload", "release in-process GGUF weights"),
];

/// The keys the frame handles. Listed by `/help`, and every one of them is
/// wired in [`crate::coder::interactive`] — this is the hint text, and a key in it
/// that does nothing is the defect this list exists to prevent.
const KEYS: &[(&str, &str)] = &[
    ("Enter", "send"),
    ("Alt+Enter / Ctrl+J", "newline"),
    (
        "Up / Down",
        "move the caret, then walk history, then scroll",
    ),
    ("Scroll wheel / trackpad", "scroll the transcript"),
    (
        "PageUp / PageDown",
        "scroll a page (Fn+Up / Fn+Down on a Mac)",
    ),
    (
        "Drag / double-click / triple-click",
        "select transcript text; Shift+drag uses your terminal's own copy",
    ),
    ("Ctrl+Y", "copy the selection (falls back to a saved file)"),
    ("Tab", "complete a command or a path"),
    (
        "Ctrl+A / Ctrl+E / Ctrl+W / Ctrl+K / Ctrl+U / Alt+B / Alt+F",
        "edit the line",
    ),
    (
        "Esc",
        "clear the selection, then cancel the active turn and keep queued prompts",
    ),
    (
        "Ctrl+C / Ctrl+D / Ctrl+Q",
        "leave Coder; an active turn is canceled first",
    ),
    ("Ctrl+P", "open the model picker (Pro and Local)"),
];

/// The command names, for Tab completion.
pub fn names() -> Vec<&'static str> {
    COMMANDS.iter().map(|(name, _)| *name).collect()
}

/// Whether `name` is one this module runs.
///
/// `/autopilot` is absent on purpose: like `/goal`, it is claimed earlier in
/// the submit path (`coder/autopilot::parse_command` in
/// `coder/interactive.rs`), before this dispatch runs — but unlike `/goal`
/// it is not routed through this module at all, because its state lives in
/// the frame loop. `every_listed_command_is_handled` accepts the exception
/// by name, and the parse test in `coder/autopilot.rs` holds the surface.
pub fn handles(name: &str) -> bool {
    matches!(
        name,
        "clear"
            | "diff"
            | "export"
            | "goal"
            | "help"
            | "info"
            | "login"
            | "logout"
            | "queue"
            | "resume"
            | "continue"
            | "run"
            | "swarm"
            | "gym"
            | "load"
            | "unload"
    )
}

/// What the dispatch could not finish on its own.
///
/// `/logout` has to end the live thread and drop the credential, and the
/// session is owned by the frame loop rather than by this module, so the line
/// is recognised here and carried out there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The line ran here. Nothing is left to do.
    Done,
    /// Log this session out: see [`logout`].
    Logout,
    /// Report the number of prompts waiting behind the active turn.
    QueueStatus,
    /// Remove prompts waiting behind the active turn.
    ClearQueue,
    /// Start a new local session from a checkpoint (#315). Empty spec is
    /// this session; `last` is the most recent other one; otherwise an id.
    ContinueFrom { spec: String },
    /// Start OpenAgents device sign-in without blocking the frame.
    Login,
}

/// Hint after a dead turn: pick the work up without replaying the dump.
pub const CONTINUE_HINT: &str =
    "To pick this up in a new session without the transcript: `/continue`.";

/// Whether a composer line is an ATIF export path. Advisory only (#341): the
/// line still goes to the model, which can read the dump with the shell tool
/// whenever it is asked to. Callers attach [`ATIF_EXPORT_ADVICE`] and send —
/// never refuse, redirect, or drop the prompt.
pub fn looks_like_atif_export(text: &str) -> bool {
    let text = text.trim().trim_matches('`').trim_matches('"');
    let lower = text.to_ascii_lowercase();
    if lower.ends_with("-atif.json") {
        return true;
    }
    let in_exports = lower.contains(".openagents/exports/") || lower.contains("/exports/");
    in_exports && (lower.contains("atif") || lower.ends_with(".json"))
}

/// The advisory that rides along when a composer line matches
/// [`looks_like_atif_export`] (#341). Advice attached to the send, never a
/// refusal: the user's text reaches the model either way, and `/continue` is
/// named in case a checkpoint resume was what the line meant to do.
pub const ATIF_EXPORT_ADVICE: &str = "Note: that looks like an ATIF export path. If you meant to resume the checkpoint rather than have this session read the dump, `/continue` (or `/continue last`) starts a new session from it.";

/// The advisory for a composer line that looks like an ATIF export path, or
/// `None` for an ordinary prompt. The caller sends either way (#341): `Some`
/// means "send, and show the advice beside it", never "send instead".
pub fn atif_export_advice(text: &str) -> Option<&'static str> {
    looks_like_atif_export(text).then_some(ATIF_EXPORT_ADVICE)
}

/// Run one `/` line. `line` still carries its leading slash.
pub fn run(ui: &mut CoderUi, line: &str, tx: &Sender<Control>, cwd: &Path) -> Outcome {
    let body = line.trim_start_matches('/');
    let mut words = body.split_whitespace();
    let Some(name) = words.next() else {
        output(ui, "A command needs a name. Try `/help`.");
        return Outcome::Done;
    };
    let arguments: Vec<String> = words.map(str::to_string).collect();
    let rest = body[name.len()..].trim().to_string();

    match name {
        "help" => output(ui, &help()),
        "info" => {
            let text = info(ui);
            output(ui, &text);
        }
        "clear" => {
            ui.entries.clear();
            ui.scroll_override = None;
        }
        "export" => crate::coder::interactive::export(ui),
        "login" => return Outcome::Login,
        // Nothing is printed here: the notice is written by `logout` once the
        // thread has been ended and the credential removed, so it says what
        // actually happened rather than what was asked for.
        "logout" => return Outcome::Logout,
        "queue" if arguments.is_empty() => return Outcome::QueueStatus,
        "queue" if arguments == ["clear"] => return Outcome::ClearQueue,
        "queue" => output(ui, "Use `/queue` or `/queue clear`."),
        "diff" => spawn_diff(ui, arguments, tx, cwd),
        "run" => spawn_run(ui, &rest, tx, cwd),
        "swarm" => run_swarm_command(ui, &arguments, &rest),
        "gym" => run_gym_command(ui, &arguments),
        "load" => run_load_command(ui, &arguments, tx),
        "unload" => run_unload_command(ui),
        "resume" => spawn_resume(ui, &arguments, tx, cwd),
        "continue" => {
            if arguments.len() > 1 {
                output(
                    ui,
                    "`/continue` takes at most one argument: `/continue`, `/continue last`, or `/continue <id>`.",
                );
                return Outcome::Done;
            }
            return Outcome::ContinueFrom {
                spec: arguments.first().cloned().unwrap_or_default(),
            };
        }
        other => output(
            ui,
            &format!("There is no `/{other}`. `/help` lists the commands."),
        ),
    }
    Outcome::Done
}

fn output(ui: &mut CoderUi, text: &str) {
    ui.entries.push(Entry::new(Role::Output, text));
    ui.scroll_override = None;
}

fn run_load_command(ui: &mut CoderUi, arguments: &[String], tx: &Sender<Control>) {
    let Some(path) = arguments.first() else {
        output(ui, "Use `/load <path-to-gguf>`.");
        return;
    };
    let path = PathBuf::from(path);
    if !path.is_file() {
        output(ui, &format!("GGUF not found at {}.", path.display()));
        return;
    }
    // Progress lives on the status row (`load_line`), not the chatting
    // spinner. The frame loop must keep painting while mmap and Metal wrap.
    ui.load_line = Some("Looking for GGUF".to_string());
    ui.loading = false;
    ui.waiting = None;

    let tx = tx.clone();
    std::thread::spawn(move || {
        let last = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sent_fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let last_paint = std::sync::Arc::new(std::sync::Mutex::new(None::<std::time::Instant>));
        let on_step = {
            let last = last.clone();
            let sent_fail = sent_fail.clone();
            let last_paint = last_paint.clone();
            let tx = tx.clone();
            std::sync::Arc::new(move |id: &str, message: &str, state: &str| {
                if let Ok(mut slot) = last.lock() {
                    *slot = message.to_string();
                }
                if state == "skip" || matches!(id, "run.start" | "run.until" | "run.teach") {
                    return;
                }
                let fail = state == "fail";
                let terminal = fail || id == "map.done";
                let due = {
                    let Ok(mut painted) = last_paint.lock() else {
                        return;
                    };
                    let now = std::time::Instant::now();
                    let elapsed_ok = painted
                        .map(|prev| {
                            now.duration_since(prev) >= std::time::Duration::from_millis(50)
                        })
                        .unwrap_or(true);
                    if terminal || elapsed_ok {
                        *painted = Some(now);
                        true
                    } else {
                        false
                    }
                };
                if !due && !fail {
                    return;
                }
                if fail {
                    sent_fail.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let _ = tx.send(Control::LoadStatus {
                    message: message.to_string(),
                    fail,
                });
            }) as std::sync::Arc<dyn Fn(&str, &str, &str) + Send + Sync>
        };
        match crate::inference::load_gguf_with_steps(path, false, true, Some(on_step)) {
            Ok(()) => {
                let _ = tx.send(Control::MemoryLine(crate::inference::memory_status_line()));
            }
            Err(_) => {
                if !sent_fail.load(std::sync::atomic::Ordering::SeqCst) {
                    let message = last
                        .lock()
                        .ok()
                        .map(|slot| slot.clone())
                        .unwrap_or_default();
                    let message = if message.is_empty() {
                        "Load failed: load did not finish".to_string()
                    } else {
                        message
                    };
                    let _ = tx.send(Control::LoadStatus {
                        message,
                        fail: true,
                    });
                }
            }
        }
    });
}

fn run_unload_command(ui: &mut CoderUi) {
    match crate::inference::unload_gguf(false) {
        Ok(()) => {
            ui.load_line = Some("Weights unloaded".to_string());
            ui.memory_line = None;
            ui.loading = false;
        }
        Err(error) => {
            let message = match error {
                crate::inference::InferenceExit::Usage(message) => message,
                crate::inference::InferenceExit::Failed => {
                    "Unload failed: weights could not be released".to_string()
                }
            };
            ui.load_line = Some(message.clone());
            output(ui, &message);
        }
    }
}

fn run_gym_command(ui: &mut CoderUi, arguments: &[String]) {
    if arguments.first().map(String::as_str) == Some("close") {
        ui.gym_panel = None;
        output(ui, "Closed the gym pane.");
        return;
    }
    if arguments.first().map(String::as_str) == Some("run") {
        let Some(source) = arguments.get(1).map(String::as_str) else {
            output(
                ui,
                "Use `/gym run <run-id>` or `/gym run <run_status.json>`. `/gym close` hides the pane.",
            );
            return;
        };
        match load_gym_run_panel(source) {
            Ok(status) => {
                let run_id = status.run_id.clone();
                let state = status.state.clone();
                ui.gym_panel = Some(crate::gym::views::GymPanel::Run(status));
                output(
                    ui,
                    &format!(
                        "Opened the gym pane for run `{run_id}` (state={state}). `/gym close` hides it."
                    ),
                );
            }
            Err(error) => output(ui, &format!("Could not load gym run `{source}`: {error}")),
        }
        return;
    }
    let suite = arguments.first().map(String::as_str).unwrap_or("tb2-quick");
    match crate::gym::results::load_suite_trend(suite) {
        Ok(trend) => {
            let verified = trend.verified;
            ui.gym_panel = Some(crate::gym::views::GymPanel::Trend(trend));
            output(
                ui,
                &format!(
                    "Opened the gym pane for `{suite}` (chain={}). `/gym close` hides it.",
                    if verified { "verified" } else { "broken" }
                ),
            );
        }
        Err(error) => output(
            ui,
            &format!("Could not load gym results for `{suite}`: {error}"),
        ),
    }
}

fn load_gym_run_panel(source: &str) -> Result<crate::gym::schemas::RunStatus, String> {
    let path = Path::new(source);
    if path.is_file() {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        return serde_json::from_str(&text).map_err(|e| e.to_string());
    }
    Err(format!(
        "pass a run_status JSON path, or write one with `openagents gym run status {source} --json`"
    ))
}

// ───────────────────────────────────────────────────────────────── /logout

/// The environment variables that carry a credential into this process.
///
/// `do_login` writes the first one so the runtime spends the token without a
/// second store lookup, and [`crate::auth::CredentialStore::find_token`]
/// reads the second one ahead of every store. Left set, either would hand the
/// next session the credential that was just removed, and the store deletion
/// would have been theatre.
const ENVIRONMENT_CREDENTIALS: [&str; 2] = ["OPENAGENTS_API_KEY", "OPENAGENTS_TOKEN"];

/// What the session is after a `/logout`. It is not over: the transcript is
/// still here. Hosted lanes refuse a prompt until `/login`. Local still
/// answers on this machine if Ollama is installed. Nothing is left that can
/// fail somewhere far away holding a credential that is gone.
const STAYS_OPEN: &str = "- This session stays open and unauthenticated. The transcript is still \
                          here. Hosted lanes need `/login`. Coder Local still answers on this \
                          machine if Ollama is installed.";

/// Log this session out of the API it is pointed at.
///
/// The notice it returns is the whole answer, and it names which credential
/// was removed. A logout that says only "logged out" is what makes a token
/// that went missing untraceable to any command (issue #128).
pub async fn logout(session: &mut crate::coder::runtime::Session) -> String {
    match crate::auth::resolve_endpoint(None, None) {
        Ok(endpoint) => {
            let store = crate::auth::CredentialStore::for_origin(&endpoint.origin);
            logout_at(session, &store).await
        }
        Err(error) => {
            // Which store to open could not be decided, so nothing is opened:
            // guessing production here is exactly how a token disappears from
            // an endpoint nobody asked about.
            let mut lines = vec![
                "Logged out. No stored token was removed: which API this session is pointed at \
                 could not be resolved, so no credential store was opened."
                    .to_string(),
                format!("- {error}"),
            ];
            lines.extend(end_thread(session).await);
            lines.extend(clear_environment());
            lines.push(STAYS_OPEN.to_string());
            lines.join("\n")
        }
    }
}

/// [`logout`] against a named store.
///
/// Split out so a test can drive the real removal against a store confined to
/// a directory rather than against the owner's credential file.
pub async fn logout_at(
    session: &mut crate::coder::runtime::Session,
    store: &crate::auth::CredentialStore,
) -> String {
    let origin = store.origin().to_string();
    let mut lines = vec![format!("Logged out of `{origin}`.")];
    // The thread first, while the credential is still good: ending it is a
    // report to the server, and a session whose token has already been taken
    // away cannot make one. A thread left open holds its grant's remaining
    // budget (#106, #107), so this is the part that is not optional.
    lines.extend(end_thread(session).await);
    lines.extend(clear_environment());

    let held = store.find_token();
    let source = match &held {
        Ok(Some(stored)) => stored.source.label(),
        _ => "store",
    };
    match store.remove() {
        Ok(true) => lines.push(format!(
            "- Removed the OpenAgents token stored for `{origin}` (from the {source}). No other \
             endpoint's token was touched."
        )),
        Ok(false) => lines.push(format!(
            "- No OpenAgents token was stored for `{origin}`, so there was none to remove."
        )),
        Err(error) => lines.push(format!(
            "- The token stored for `{origin}` was NOT removed: {error}. It is still there."
        )),
    }
    if let Err(error) = held {
        lines.push(format!(
            "- Where that token was held could not be read: {error}"
        ));
    }

    lines.push(STAYS_OPEN.to_string());
    lines.join("\n")
}

/// End the session's thread by reporting what it did, and say so.
async fn end_thread(session: &mut crate::coder::runtime::Session) -> Vec<String> {
    let ended = session.finish().await;
    // `finish` falls back to a cancellation when the report is refused, and
    // records why. Nothing drains that outside a turn, so it is read here:
    // otherwise the notice would claim a report that did not happen.
    let recorded = session.take_record_failures();
    let mut lines = Vec::new();
    match ended {
        Ok(spent) => {
            if recorded.is_empty() {
                let billed = spent.map(|line| format!(" {line}.")).unwrap_or_default();
                lines.push(format!(
                    "- The thread was ended by reporting what this session did, so nothing is \
                     left holding its grant's remaining budget.{billed}"
                ));
            } else {
                lines.extend(recorded.into_iter().map(|note| format!("- {note}")));
            }
        }
        Err(error) => {
            lines.push(format!(
                "- The thread was NOT ended: {error}. It may still be open and holding its \
                 grant's remaining budget."
            ));
            lines.extend(recorded.into_iter().map(|note| format!("- {note}")));
        }
    }
    lines
}

/// Drop the credentials this process carries in its environment, and say which.
fn clear_environment() -> Vec<String> {
    let mut lines = Vec::new();
    for name in ENVIRONMENT_CREDENTIALS {
        if std::env::var_os(name).is_none() {
            continue;
        }
        // SAFETY: the same process-global write `do_login` makes when it puts
        // the token here, undone. This runs on the frame's own task, between
        // turns, with no session holding a credential read in flight.
        unsafe { std::env::remove_var(name) };
        lines.push(format!(
            "- Cleared `{name}` from this session. The shell that set it still has it, so a new \
             session started from that shell would use it again."
        ));
    }
    lines
}

fn help() -> String {
    let mut lines = vec!["**Commands**".to_string(), String::new()];
    for (name, what) in COMMANDS {
        lines.push(format!("- `/{name}` — {what}"));
    }
    lines.push(String::new());
    lines.push("**Keys**".to_string());
    lines.push(String::new());
    for (key, what) in KEYS {
        lines.push(format!("- `{key}` — {what}"));
    }
    lines.join("\n")
}

/// What `/info` prints: the numbers that used to sit under the composer, plus
/// the facts that say which session produced them.
///
/// Every line is read from a value the session was *given* — `ui.model` is the
/// model the grant named, `ui.thread` is the thread the server opened,
/// `ui.last_usage` and `ui.total_usage` are what the server reported per turn.
/// Nothing here re-parses a transcript line, and nothing here is a name
/// compiled into this file: a default model that moved in the catalog would
/// otherwise be reported by a session that was never given it.
///
/// A field nothing has reported says so in words. A zero printed where no
/// measurement exists reads as a measurement, and this is the pane someone
/// opens precisely because they want the real figure.
fn info(ui: &CoderUi) -> String {
    let mut lines = vec!["**This session**".to_string(), String::new()];

    lines.push(format!("- Account — {}", ui.identity.line()));
    lines.push(match ui.endpoint.is_empty() {
        true => "- Endpoint — not resolved".to_string(),
        false => format!("- Endpoint — {}", ui.endpoint),
    });
    if let crate::coder::tui::Identity::Named {
        id,
        namespaces,
        expires_at,
        ..
    } = &ui.identity
    {
        lines.push(format!("- Account id — {id}"));
        lines.push(match namespaces.is_empty() {
            true => "- Namespaces — none".to_string(),
            false => format!("- Namespaces — {}", namespaces.join(", ")),
        });
        lines.push(format!("- Credential expires — {expires_at}"));
    }

    lines.push(match ui.model.is_empty() {
        // What answered, or nothing. Never the lane's preference: that is what
        // was asked for, and the two are not the same fact.
        true => "- Model — no model has answered yet".to_string(),
        false => format!("- Model — `{}`", ui.model),
    });
    lines.push(match ui.lane.is_empty() {
        true => "- Lane — not recorded".to_string(),
        false => format!("- Lane — {}", ui.lane),
    });
    lines.push(match &ui.thread {
        Some(thread) => format!("- Thread — `{thread}`"),
        None => "- Thread — none open".to_string(),
    });
    lines.push(match &ui.local_session_id {
        Some(id) => format!("- Local session — `{id}`"),
        None => "- Local session — unavailable".to_string(),
    });
    lines.push(match &ui.local_session_path {
        Some(path) => format!("- Local record — `{path}`"),
        None => "- Local record — unavailable".to_string(),
    });
    lines.push(format!(
        "- Cloud history — {}",
        if ui.cloud_history {
            "enabled"
        } else {
            "disabled"
        }
    ));

    lines.push(match ui.last_usage.reported() {
        true => format!("- Last turn — {}", ui.last_usage.line()),
        false => "- Last turn — nothing reported".to_string(),
    });
    lines.push(match ui.total_usage.reported() {
        true => format!("- This session counted — {}", ui.total_usage.line()),
        false => "- This session counted — nothing reported".to_string(),
    });
    lines.push(billed_line(ui));

    lines.join("\n")
}

/// The server's billed figure against what this session counted.
///
/// The two disagree often enough that the gap is worth a sentence, and until
/// now it was only visible for the half-second after the screen closed. The
/// server's figure is the one the account is charged against, so it is named
/// as such rather than left for the reader to rank.
fn billed_line(ui: &CoderUi) -> String {
    let Some(billed) = ui.billed else {
        return "- Billed by the server — not reported yet; the figure arrives when the thread \
                ends"
            .to_string();
    };
    let counted = ui.total_usage.total_tokens;
    if billed == counted {
        return format!(
            "- Billed by the server — {billed} tokens, which is what this session counted"
        );
    }
    format!(
        "- Billed by the server — {billed} tokens; this session counted {counted}, a difference \
         of {}. The server's figure is the one the account is charged against.",
        billed.abs_diff(counted)
    )
}

/// The call id a command's own output box is filed under.
fn command_call_id(ui: &CoderUi) -> String {
    format!("command-{}", ui.entries.len())
}

// ────────────────────────────────────────────────────────────────── /diff

fn spawn_diff(_ui: &mut CoderUi, arguments: Vec<String>, tx: &Sender<Control>, cwd: &Path) {
    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    tokio::spawn(async move {
        let text = match crate::interactive::collect_diff(&arguments, &cwd).await {
            Err(why) => why,
            Ok(files) if files.is_empty() => "Nothing has changed.".to_string(),
            Ok(files) => render_diff(&files),
        };
        let _ = tx.send(Control::Output(text));
    });
}

/// A diff as markdown: a summary line per file, then the unified body in a
/// fenced block so the transcript's own renderer highlights it in the palette
/// it highlights everything else in.
fn render_diff(files: &[crate::diff::FileDiff]) -> String {
    use crate::diff::Tag;

    let mut summary = Vec::new();
    let mut body = Vec::new();
    for file in files {
        let (added, removed) = file.stats();
        let named = match &file.renamed_from {
            Some(from) => format!("{from} → {}", file.path),
            None => file.path.clone(),
        };
        summary.push(format!("- `{named}` +{added} −{removed}"));

        body.push(format!(
            "--- a/{}",
            file.renamed_from.as_ref().unwrap_or(&file.path)
        ));
        body.push(format!("+++ b/{}", file.path));
        if let Some(note) = &file.note {
            body.push(format!("# {note}"));
            continue;
        }
        for hunk in &file.hunks {
            body.push(hunk.header());
            for line in &hunk.lines {
                let marker = match line.tag {
                    Tag::Insert => '+',
                    Tag::Delete => '-',
                    Tag::Equal => ' ',
                };
                body.push(format!("{marker}{}", line.text));
            }
        }
    }

    let mut out = summary.join("\n");
    out.push_str("\n\n```diff\n");
    out.push_str(&bounded(body.join("\n")));
    out.push_str("\n```");
    out
}

/// The same ceiling the `shell` tool holds its output to, for the same reason:
/// a diff of a vendored directory is megabytes and the frame has to stay up.
fn bounded(text: String) -> String {
    if text.len() <= OUTPUT_LIMIT {
        return text;
    }
    // A diff is the repository's own bytes. Floored to a character boundary
    // rather than sliced at a byte index, which is the defect `c48fa5b138`
    // and `28704f72ff` went through this tree removing.
    let cut = crate::tracker::floor_char_boundary(&text, OUTPUT_LIMIT);
    format!(
        "{}\n[truncated: {} characters, limit is {OUTPUT_LIMIT}]",
        &text[..cut],
        text.len()
    )
}

// ─────────────────────────────────────────────────────────────────── /run

/// Run a command here and stream its output into a box.
///
/// Not a pseudoterminal: there is no pane to attach to and nothing takes keys
/// while it runs, so it is the same non-interactive spawn the `shell` tool
/// makes, held to the same refusal list and the same output ceiling. `/help`
/// says "run a command here and show its output" rather than anything about a
/// terminal, because that is what it does.
fn spawn_run(ui: &mut CoderUi, command: &str, tx: &Sender<Control>, cwd: &Path) {
    if command.is_empty() {
        output(ui, "`/run` needs a command: `/run git status`.");
        return;
    }
    if let Some(refusal) = check_shell_refusal(command) {
        output(ui, &refusal);
        return;
    }

    let call_id = command_call_id(ui);
    let mut entry = Entry::tool_call(format!("run {command}"));
    entry.tool = Some(ToolCall {
        call_id: call_id.clone(),
        function_name: "run".to_string(),
        arguments: serde_json::json!({ "command": command }),
        output: None,
        error: None,
        done: false,
        duration_ms: None,
    });
    ui.entries.push(entry);
    ui.scroll_override = None;

    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    let command = command.to_string();
    let started = std::time::Instant::now();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;

        let argv = crate::pty::shell_command(&command);
        let mut spawn = tokio::process::Command::new(&argv[0]);
        spawn
            .args(&argv[1..])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        spawn.process_group(0);

        let mut child = match spawn.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = tx.send(Control::ToolOutput {
                    call_id: call_id.clone(),
                    chunk: format!("could not start it: {error}"),
                });
                let _ = tx.send(Control::ToolDone {
                    call_id,
                    is_error: true,
                    duration_ms: 0,
                });
                return;
            }
        };

        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        let mut printed = 0usize;
        let mut buffer = [0u8; 4096];
        loop {
            let read = match (&mut stdout, &mut stderr) {
                (Some(out), _) => out.read(&mut buffer).await,
                (None, Some(err)) => err.read(&mut buffer).await,
                (None, None) => break,
            };
            match read {
                Ok(0) => {
                    // stdout first, then stderr, then done.
                    if stdout.is_some() {
                        stdout = None;
                    } else {
                        stderr = None;
                    }
                }
                Ok(n) => {
                    if printed < OUTPUT_LIMIT {
                        let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        printed += n;
                        let _ = tx.send(Control::ToolOutput {
                            call_id: call_id.clone(),
                            chunk,
                        });
                    }
                }
                Err(_) => break,
            }
        }

        let status = child.wait().await;
        let (note, failed) = match status {
            Ok(status) if status.success() => (String::new(), false),
            Ok(status) => (
                format!("\n[exited with code {}]", status.code().unwrap_or(1)),
                true,
            ),
            Err(error) => (format!("\n[it did not finish: {error}]"), true),
        };
        if !note.is_empty() {
            let _ = tx.send(Control::ToolOutput {
                call_id: call_id.clone(),
                chunk: note,
            });
        }
        let _ = tx.send(Control::ToolDone {
            call_id,
            is_error: failed,
            duration_ms: started.elapsed().as_millis() as u64,
        });
    });
}

// ──────────────────────────────────────────────────────────────── /resume

fn spawn_resume(ui: &mut CoderUi, arguments: &[String], tx: &Sender<Control>, cwd: &Path) {
    // A bare `/resume` lists; `/resume <n>` picks. A word that is not a
    // positive number is refused rather than read as a list request, because
    // silently listing after a mistyped pick is how someone resumes the wrong
    // session.
    let selection = match arguments.first() {
        None => None,
        Some(word) => match word.parse::<usize>() {
            Ok(number) if number >= 1 => Some(number),
            _ => {
                output(
                    ui,
                    &format!(
                        "`/resume` takes a number from the list: `/resume 1`. `{word}` is not one."
                    ),
                );
                return;
            }
        },
    };

    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    tokio::spawn(async move {
        // The scan compiles and runs a wasm guest and walks two state
        // directories, all of it synchronous. On a blocking thread so the
        // frame keeps drawing while it works.
        let home = crate::auth::home_directory();
        let scanned = tokio::task::spawn_blocking(move || {
            crate::foreign_resume::foreign_resume_turn(&cwd, &home, selection)
        })
        .await;
        let text = match scanned {
            Ok(text) => text,
            Err(error) => format!("The scan did not finish: {error}"),
        };
        let _ = tx.send(Control::Output(text));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_every_command_and_nothing_else() {
        let text = help();
        for (name, what) in COMMANDS {
            assert!(text.contains(&format!("`/{name}`")), "{text}");
            assert!(text.contains(what), "{text}");
        }
    }

    /// A key in the hint text that nothing handles is the defect the rule in
    /// issue #105 is about. These are the ones `interactive` wires.
    #[test]
    fn the_key_hints_name_only_keys_the_session_handles() {
        let listed: Vec<&str> = KEYS.iter().map(|(key, _)| *key).collect();
        assert!(listed.contains(&"Enter"));
        assert!(listed.contains(&"Tab"));
        assert!(listed.contains(&"Esc"));
        assert!(listed.contains(&"Ctrl+C / Ctrl+D / Ctrl+Q"));
        // Nothing about a pane, a diff inspector, or a detach key: none of
        // those exist here.
        let text = help();
        for absent in ["Ctrl+]", "detach", "inspector"] {
            assert!(!text.contains(absent), "`{absent}` is claimed: {text}");
        }
    }

    #[test]
    fn a_diff_renders_as_a_summary_and_a_fenced_body() {
        let files = crate::diff::parse_unified(
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n line\n-old\n+new\n",
        );
        assert!(!files.is_empty(), "the fixture parsed to nothing");
        let text = render_diff(&files);
        assert!(text.contains("`a.txt` +1 −1"), "{text}");
        assert!(text.contains("```diff"), "{text}");
        assert!(text.contains("+new"), "{text}");
        assert!(text.contains("-old"), "{text}");
    }

    #[test]
    fn a_long_diff_is_cut_at_the_same_ceiling_the_shell_tool_uses() {
        let text = bounded("x".repeat(OUTPUT_LIMIT + 100));
        assert!(text.contains("truncated"), "it was not cut");
        assert!(text.len() < OUTPUT_LIMIT + 200);
    }

    fn continue_outcome(line: &str) -> Outcome {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut ui = CoderUi::new();
        run(&mut ui, line, &tx, Path::new("/work/repo"))
    }

    #[test]
    fn continue_takes_an_empty_spec_last_or_an_id() {
        assert_eq!(
            continue_outcome("/continue"),
            Outcome::ContinueFrom {
                spec: String::new()
            }
        );
        assert_eq!(
            continue_outcome("/continue last"),
            Outcome::ContinueFrom {
                spec: "last".into()
            }
        );
        assert_eq!(
            continue_outcome("/continue abc123"),
            Outcome::ContinueFrom {
                spec: "abc123".into()
            }
        );
        assert_eq!(continue_outcome("/continue last extra"), Outcome::Done);
    }

    #[test]
    fn an_atif_export_path_is_recognised_and_ordinary_prompts_are_not() {
        assert!(looks_like_atif_export(
            "~/.openagents/exports/1a0434b26a4-atif.json"
        ));
        assert!(looks_like_atif_export(
            "`/Users/me/.openagents/exports/session-atif.json`"
        ));
        assert!(looks_like_atif_export(
            "/Users/me/.openagents/exports/dead-tab.json"
        ));
        assert!(!looks_like_atif_export("continue from the last checkpoint"));
        assert!(!looks_like_atif_export(
            "read crates/openagents-cli/src/runtime.rs"
        ));
        assert!(!looks_like_atif_export("/help"));
    }

    /// #341: a matching line is annotated and sent, never refused. The
    /// advisory is advice beside the action; the detector on its own drops
    /// nothing.
    #[test]
    fn an_atif_export_line_is_annotated_and_sent_not_refused() {
        let export = "~/.openagents/exports/1a0434b26a4-atif.json";
        let advice = atif_export_advice(export).expect("an export path gets the advisory");
        assert!(
            advice.contains("/continue"),
            "the advice names the resume path"
        );
        assert!(
            advice.contains("Note:"),
            "the advice reads as a note, not a veto"
        );

        // An ordinary prompt is sent with no annotation attached.
        assert_eq!(
            atif_export_advice("read crates/openagents-cli/src/runtime.rs"),
            None
        );
        // And the detection itself never consumes the line: `submit` sends
        // `text` unchanged whether or not the advisory fired.
        assert_eq!(export, "~/.openagents/exports/1a0434b26a4-atif.json");
    }

    fn drain_load(ui: &mut CoderUi, rx: &std::sync::mpsc::Receiver<Control>) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                !left.is_zero(),
                "load did not settle; load_line={:?}",
                ui.load_line
            );
            match rx.recv_timeout(left) {
                Ok(control) => {
                    let fail = matches!(&control, Control::LoadStatus { fail: true, .. });
                    let memory = matches!(&control, Control::MemoryLine(_));
                    crate::coder::interactive::apply(ui, control);
                    if fail || memory {
                        return;
                    }
                }
                Err(_) => panic!("load did not settle; load_line={:?}", ui.load_line),
            }
        }
    }

    #[test]
    fn load_fixture_shows_weights_ready_and_unload_clears_memory() {
        let _lock = crate::inference::serialize_load_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qwen35.gguf");
        psionic_gguf::write_qwen35_fixture(&path).unwrap();
        let mut ui = CoderUi::new();
        ui.show_welcome = false;
        let (tx, rx) = std::sync::mpsc::channel();
        assert_eq!(
            run(
                &mut ui,
                &format!("/load {}", path.display()),
                &tx,
                Path::new(".")
            ),
            Outcome::Done
        );
        drain_load(&mut ui, &rx);
        let line = ui.load_line.clone().expect("load line");
        assert!(line.contains("Weights ready"), "load line was {line:?}");
        assert!(ui.memory_line.is_some(), "memory meter after load");
        assert!(!ui.loading, "must not look like a chatting turn");
        assert_eq!(run(&mut ui, "/unload", &tx, Path::new(".")), Outcome::Done);
        assert_eq!(ui.load_line.as_deref(), Some("Weights unloaded"));
        assert!(ui.memory_line.is_none());
    }

    #[test]
    fn load_bad_magic_shows_canonical_fail_and_does_not_hang() {
        let _lock = crate::inference::serialize_load_tests();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not.gguf");
        std::fs::write(&path, b"XXXX\x03\x00\x00\x00").unwrap();
        let mut ui = CoderUi::new();
        ui.show_welcome = false;
        let (tx, rx) = std::sync::mpsc::channel();
        run(
            &mut ui,
            &format!("/load {}", path.display()),
            &tx,
            Path::new("."),
        );
        drain_load(&mut ui, &rx);
        let line = ui.load_line.clone().expect("fail line");
        assert!(
            line.contains("Not a GGUF file") || line.contains("magic"),
            "{line}"
        );
        assert!(!ui.loading);
        assert!(ui.memory_line.is_none());
        assert!(
            ui.entries
                .iter()
                .any(|entry| entry.text.contains("Not a GGUF") || entry.text.contains("magic")),
            "{:?}",
            ui.entries.iter().map(|e| &e.text).collect::<Vec<_>>()
        );
    }
}

/// `/swarm` — the local swarm, from inside a session.
///
/// `list` and `tree` read the registrations; `inbox` reads this (or another)
/// session's inbox; `send` delivers from this session, so the outbox lands in
/// this session's own store directory and the exchange is attributable in
/// both transcripts.
fn run_swarm_command(ui: &mut CoderUi, arguments: &[String], rest: &str) {
    use crate::swarm;
    let home = crate::swarm::default_home();
    let Some(sub) = arguments.first().map(String::as_str) else {
        output(
            ui,
            "Use `/swarm list`, `/swarm tree`, `/swarm inbox [id]`, `/swarm send <id> <text>`, or `/swarm mute <id>`.",
        );
        return;
    };
    match sub {
        "list" => match swarm::list(&home) {
            Ok(registrations) if registrations.is_empty() => {
                output(ui, "No sessions are registered.");
            }
            Ok(registrations) => {
                let mut lines = vec![format!(
                    "{:<6}  {:<38}  {:<6}  {}",
                    "state", "session", "role", "cwd"
                )];
                for registration in &registrations {
                    lines.push(format!(
                        "{:<6}  {:<38}  {:<6}  {}",
                        registration.state().as_str(),
                        registration.session_id,
                        registration.role,
                        registration.cwd,
                    ));
                }
                output(ui, &lines.join("\n"));
            }
            Err(why) => output(ui, &why),
        },
        "tree" => match swarm::list(&home) {
            Ok(registrations) if registrations.is_empty() => {
                output(ui, "No sessions are registered.");
            }
            Ok(registrations) => {
                let mut lines = Vec::new();
                fn render(
                    session: &swarm::Registration,
                    depth: usize,
                    all: &[swarm::Registration],
                    lines: &mut Vec<String>,
                ) {
                    let indent = "  ".repeat(depth);
                    lines.push(format!(
                        "{indent}{}  {}  {}",
                        session.state().as_str(),
                        session.session_id,
                        session.cwd,
                    ));
                    for child in all.iter().filter(|candidate| {
                        candidate.role == "child"
                            && candidate.parent.as_deref() == Some(session.session_id.as_str())
                    }) {
                        render(child, depth + 1, all, lines);
                    }
                }
                for registration in &registrations {
                    if registration.role != "child" {
                        render(registration, 0, &registrations, &mut lines);
                    }
                }
                output(ui, &lines.join("\n"));
            }
            Err(why) => output(ui, &why),
        },
        "inbox" => {
            // Default to this session, identified by the store that carries
            // the composer's transcript — the caller asked from here.
            let session_id = arguments.get(1).cloned();
            match resolve_inbox_directory(&session_id) {
                Ok((directory, named)) => match swarm::read_inbox(&directory) {
                    Ok(messages) if messages.is_empty() => {
                        output(ui, &format!("The inbox of {named} is empty."));
                    }
                    Ok(messages) => {
                        let mut lines = Vec::new();
                        for message in &messages {
                            lines.push(format!(
                                "#{} [{}] from {} {}{}",
                                message
                                    .sequence
                                    .map(|sequence| sequence.to_string())
                                    .unwrap_or_else(|| "?".to_string()),
                                message.kind,
                                message.from,
                                message.id,
                                if message.read_at_ms.is_some() {
                                    String::new()
                                } else {
                                    "  · unread".to_string()
                                },
                            ));
                            lines.push(message.body.clone());
                        }
                        output(ui, &lines.join("\n"));
                    }
                    Err(why) => output(ui, &why),
                },
                Err(why) => output(ui, &why),
            }
        }
        "send" => {
            // `/swarm send <id> <text...>`: everything after the target is
            // the body, verbatim.
            let Some(target) = arguments.get(1) else {
                output(ui, "Name a destination: `/swarm send <session-id> <text>`.");
                return;
            };
            let body = rest
                .strip_prefix(target.as_str())
                .map(str::trim_start)
                .unwrap_or_default()
                .to_string();
            if body.is_empty() {
                output(ui, "The message body is empty.");
                return;
            }
            let Some((directory, from_id)) = own_inbox_directory() else {
                output(
                    ui,
                    "This session is not registered with the swarm, so it cannot send.",
                );
                return;
            };
            match swarm::send(
                &crate::swarm::default_home(),
                &from_id,
                &directory,
                target,
                "status",
                None,
                false,
                &body,
                None,
                None,
            ) {
                Ok(report) => {
                    let mut lines = Vec::new();
                    for delivery in &report.deliveries {
                        lines.push(format!(
                            "Delivered {} to {} (sequence {}).",
                            report.message_id, delivery.to, delivery.sequence
                        ));
                    }
                    for missed in &report.undeliverable {
                        lines.push(format!("Not delivered to {}: {}.", missed.to, missed.why));
                    }
                    if report.deliveries.is_empty() {
                        lines.push("No recipient accepted the message.".to_string());
                    }
                    output(ui, &lines.join("\n"));
                }
                Err(why) => output(ui, &why),
            }
        }
        "mute" => {
            let Some(target) = arguments.get(1) else {
                output(ui, "Name a session: `/swarm mute <session-id>`.");
                return;
            };
            match own_inbox_directory() {
                Some((directory, owner)) => {
                    match crate::swarm::SwarmBinding::new(
                        crate::swarm::default_home(),
                        owner,
                        directory,
                    )
                    .mute(target)
                    {
                        Ok(()) => output(
                            ui,
                            &format!(
                                "Muted {target}. Their messages stay unread and will not be injected."
                            ),
                        ),
                        Err(why) => output(ui, &why),
                    }
                }
                None => output(
                    ui,
                    "This session is not registered with the swarm, so it cannot mute anyone.",
                ),
            }
        }
        "unmute" => {
            let Some(target) = arguments.get(1) else {
                output(ui, "Name a session: `/swarm unmute <session-id>`.");
                return;
            };
            match own_inbox_directory() {
                Some((directory, owner)) => {
                    match crate::swarm::SwarmBinding::new(
                        crate::swarm::default_home(),
                        owner,
                        directory,
                    )
                    .unmute(target)
                    {
                        Ok(()) => output(ui, &format!("Unmuted {target}.")),
                        Err(why) => output(ui, &why),
                    }
                }
                None => output(
                    ui,
                    "This session is not registered with the swarm, so it cannot unmute anyone.",
                ),
            }
        }
        other => output(
            ui,
            &format!(
                "`/swarm {other}` is not a swarm command. Use list, tree, inbox, send, mute, or unmute."
            ),
        ),
    }
}

/// Where the named (or this) session's inbox lives.
fn resolve_inbox_directory(
    session: &Option<String>,
) -> Result<(std::path::PathBuf, String), String> {
    let home = crate::swarm::default_home();
    match session {
        Some(id) => {
            let registration = swarm_load(&home, id)?;
            let directory = std::path::Path::new(&registration.inbox)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            Ok((directory, id.clone()))
        }
        None => own_inbox_directory()
            .ok_or_else(|| "No sessions are registered, so there is no inbox to read.".to_string()),
    }
}

fn swarm_load(home: &std::path::Path, id: &str) -> Result<crate::swarm::Registration, String> {
    crate::swarm::load_registration(home, id)?
        .ok_or_else(|| format!("No session `{id}` is registered."))
}

/// This session's own swarm identity, when it has one: the registration whose
/// pid is this process and whose store directory is beside this transcript.
pub(crate) fn own_inbox_directory() -> Option<(std::path::PathBuf, String)> {
    let root = crate::swarm::default_home();
    let registrations = crate::swarm::list(&root).ok()?;
    let pid = std::process::id();
    let registration = registrations
        .iter()
        .find(|registration| registration.pid == pid && registration.role == "root")?;
    let directory = std::path::Path::new(&registration.inbox)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    Some((directory, registration.session_id.clone()))
}
