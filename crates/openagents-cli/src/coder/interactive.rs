//! The interactive Coder session.
//!
//! One session, one thread, one frame. The session is
//! [`crate::coder::runtime::Session`] and it is held for the life of the process
//! rather than built per turn — a session rebuilt on every Enter forgets the
//! conversation and leaves a thread open behind it, which is what this used to
//! do — and it is revoked on the way out.
//!
//! Key handling follows the pattern used in grok-build's ratatui-textarea and
//! grok-pager: destructure `crossterm::event::KeyEvent` by `code` and
//! `modifiers` so control chords do not fall through to plain character input.
//! Everything the composer claims is either handled here or handed to
//! [`crate::composer`], which handles it; `/help` lists exactly that
//! set and nothing else.

use crate::auth::{CredentialStore, DeviceClient, Secret, open_browser};
use crate::coder::commands;
use crate::coder::export::{export_trajectory, git_info};
use crate::coder::runtime::{Control, Session, tool_title};
use crate::coder::tui::{CoderUi, Entry, Role, ToolCall};
use crate::coder::turn::{TurnAction, TurnEffect, TurnId, TurnPhase, TurnState};
use crate::composer::ComposerAction;
use crate::composer::complete::{Completion, complete};
use crate::composer::history::History;
use crate::runtime::{ImageAttachment, Lane};
use crossterm::{
    ExecutableCommand,
    cursor::SetCursorStyle,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent,
        MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::collections::VecDeque;
use std::io::{Write, stderr, stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use tokio::sync::Mutex;

struct ActiveTurn {
    id: TurnId,
    task: tokio::task::JoinHandle<()>,
    router: crate::coder::runtime::SharedTurnRouter,
}

struct QueuedPrompt {
    text: String,
    images: Vec<ImageAttachment>,
}

/// How long the exit waits for a thread to be revoked before it says it could
/// not be. A turn still streaming would otherwise hold the exit for as long as
/// the model wants.
const REVOCATION_GRACE: Duration = Duration::from_secs(10);

/// How long a clipboard toast stays up, in render ticks. The frame advances
/// only when something redraws, so a quiet session holds the message until
/// the next event — which is the behavior wanted: a "Copied!" that vanishes
/// before it is read is a message that was never shown.
const CLIPBOARD_TOAST_TICKS: u64 = 60;

/// Palette amber (`TEXT_COLOR` `#FFB000`). OSC 12 colours the hardware caret
/// so it is not the terminal's default yellow sitting inside a painted block.
const CURSOR_COLOR_SET: &str = "\x1b]12;#FFB000\x07";
/// Restore the terminal's own caret colour on the way out.
const CURSOR_COLOR_RESET: &str = "\x1b]112\x07";

/// Hosted lanes need an OpenAgents account. Local does not (#325).
pub const HOSTED_NEEDS_SIGN_IN: &str = "This lane talks to OpenAgents. Sign in with /login, or use Coder Local if Ollama is installed.";

/// What `--lane` and `--reasoning` settled on before the screen was entered.
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    /// The lane name as it was typed, for the delegation gate and refusals.
    pub lane_name: String,
    /// Whether `--lane` should replace the lane stored by a resumed session.
    pub lane_explicit: bool,
    /// `--reasoning`, recorded on the thread at open. `None` leaves the
    /// deployment's own default, which is a different answer from naming one.
    pub reasoning: Option<String>,
    /// `--dev` talks to the local Rust coder API on this machine.
    pub dev: bool,
    /// `Some("")` resumes the most recent local session for this directory;
    /// any other value names a local session id. `None` starts a new session.
    pub resume: Option<String>,
    /// Upload transcript events and outcome text. Local-only is the default.
    pub cloud_history: bool,
    /// Send one prompt and exit. Used for headless tests.
    pub prompt: Option<String>,
}

pub async fn run_tui(options: SessionOptions) -> Result<(), Box<dyn std::error::Error>> {
    // A one-shot prompt must not depend on a terminal: its whole reason to
    // exist is headless tests and scripts, where no TTY will ever be attached
    // (#338). The refusal below is for an interactive Coder with nothing to
    // run, not for `--prompt`. (Session registration happens inside
    // `run_one_shot`, so the swarm still sees exactly one registration.)
    // Clone the prompt before the match: `SessionBoot::OneShot` borrows it,
    // and the one-shot path then moves `options` into `run_one_shot`.
    let prompt = options.prompt.clone();
    match session_boot(prompt.as_deref(), atty_is_terminal()) {
        SessionBoot::RefuseNoTty => {
            println!("Non-interactive terminal detected. Run `openagents` from a terminal.");
            return Ok(());
        }
        SessionBoot::OneShot(_) => {
            return run_one_shot(options, prompt.expect("OneShot means a prompt was set")).await;
        }
        SessionBoot::FullScreen => {
            // A TTY with a prompt keeps the full-screen path: the reader
            // watches the turn run and the TUI exits at its boundary.
        }
    }

    // Take the terminal before any await. 0.2.0-rc1 ran git + issue list
    // first (#316); during that window keys still belonged to the invoking
    // shell, and after keyboard enhancement they arrived as CSI-u on the
    // prompt (`7441;1:3u`).
    enable_raw_mode()?;
    let mut stdout = stdout();
    if let Err(error) = stdout.execute(EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(error.into());
    }
    let _terminal_cleanup = TerminalCleanup;
    let mouse_error = enable_session_mouse(&mut stdout).err();
    let flags = event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
    let _ = crossterm::execute!(
        stderr(),
        PushKeyboardEnhancementFlags(flags),
        EnableBracketedPaste
    );
    let _ = stdout.execute(SetCursorStyle::BlinkingBlock);
    let _ = stdout.write_all(CURSOR_COLOR_SET.as_bytes());
    let _ = stdout.flush();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;

    let (tx, rx) = mpsc::channel::<Control>();
    let mut ui = CoderUi::new();
    let mut history = History::load(History::default_path());

    let (repo, branch) = git_info().unwrap_or(("unknown".to_string(), "unknown".to_string()));
    ui.repo = repo;
    ui.branch = branch;
    // The endpoint does not depend on a credential.
    ui.endpoint = crate::coder::runtime::api_base();

    // Only agents that are actually installed. `find_agents` checks each one
    // before reporting it, so the `delegate` tool's external-agent path runs
    // over a list of programs that exist on this machine rather than a
    // registry's wish list. This discovery feeds the welcome box; the
    // session's gate discovers separately in `session_tools`.
    let agents = crate::coder::acp::find_agents().await.unwrap_or_default();
    ui.agents = agents.clone();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    ui.cwd = cwd.display().to_string();
    let root = crate::session_store::default_root();
    let mut loaded = match options.resume.as_deref() {
        None => crate::session_store::LocalSessionStore::create(
            &root,
            &cwd,
            &options.lane_name,
            options.reasoning.clone(),
            options.cloud_history,
        )?,
        Some("") => crate::session_store::LocalSessionStore::load_last(&root, &cwd)?
            .ok_or_else(|| format!("No saved Coder session exists for {}.", cwd.display()))?,
        Some(id) => crate::session_store::LocalSessionStore::load_id(&root, &cwd, id)?
            .ok_or_else(|| format!("No saved Coder session has id `{id}`."))?,
    };
    let resumed = options.resume.is_some();
    let lane_name = if resumed && !options.lane_explicit {
        loaded.summary.lane.clone()
    } else {
        options.lane_name.clone()
    };
    let reasoning = if resumed && options.reasoning.is_none() {
        loaded.summary.reasoning.clone()
    } else {
        options.reasoning.clone()
    };
    loaded.store.set_lane(&lane_name)?;
    // Mutable because shift+tab moves it.
    let mut lane = Lane::from_str(&lane_name);
    let reasoning = reasoning.or_else(|| lane.default_reasoning().map(str::to_string));
    loaded.store.set_reasoning(reasoning.as_deref())?;
    let cloud_history = options.cloud_history && !lane.is_local();
    loaded.store.set_cloud_history(cloud_history)?;
    ui.lane = lane.label();
    let restored_events = loaded.events;
    ui.local_session_id = Some(loaded.summary.id.clone());
    ui.local_session_path = Some(loaded.store.directory().display().to_string());
    ui.cloud_history = cloud_history;
    // The local lane's walk membership is decided once, here, from one
    // bounded probe (issue #291). `Some` names the model the lane resolves
    // to; `None` covers no server, a refusal, a timeout, and an empty
    // library, and none of those is an error the reader sees. A probe that
    // found nothing leaves shift+tab walking exactly the hosted lanes.
    //
    // The probe is skipped when the reader named the lane explicitly: someone
    // who pinned `--lane local` or `--model ollama:qwen3.8:…` has already made
    // the choice the probe would make for them. A resume still probes — its
    // stored lane is restored either way, and the walk should know whether
    // `local` is real on this machine before offering it.
    let local_lane_model = if options.lane_explicit {
        None
    } else {
        crate::runtime::CoderRuntimeSession::probe_local_lane().await
    };
    if resumed {
        restore_entries(&mut ui, &restored_events);
        ui.model = loaded.summary.last_model.unwrap_or_default();
        for entry in &mut ui.entries {
            if entry.role == Role::Assistant && entry.model.is_none() && !ui.model.is_empty() {
                entry.model = Some(ui.model.clone());
            }
        }
        // The model's own last milestone note, ahead of the replayed
        // transcript (#189): what landed, what is broken, what is next. A
        // resume that has to re-derive this from raw tool records is the
        // failure the checkpoint exists to prevent.
        if let Some(note) = &loaded.summary.last_checkpoint
            && !note.trim().is_empty()
        {
            ui.entries.push(Entry::new(
                Role::Notice,
                format!("Last checkpoint:\n{note}"),
            ));
        }
        ui.show_welcome = false;
    }
    let checkpoint = loaded.summary.last_checkpoint.clone();
    let snapshot_text =
        crate::coder::snapshot::workspace_snapshot(&cwd, checkpoint.as_deref()).await;
    // Same as the workspace snapshot (#316): this text seeds the first
    // model request. It is not a transcript Notice. 0.2.0-rc4 painted
    // "Installed capabilities (host search, not loaded)" as the first
    // system message (#322).
    let capability_notice = crate::plugins::session_start_capability_notice(
        &crate::plugins::discover_catalog(&cwd),
        crate::plugins::Approval {
            mounts_allowed: true,
        },
    );
    let atif_directory = loaded.store.directory().to_path_buf();
    // This session joins the local swarm: other tabs and, later, delegate
    // children discover it through this registration. Failing to register is
    // a notice, never a session-fatal error — a swarm that cannot see one
    // session is degraded, not broken.
    let swarm_session_id = loaded.summary.id.clone();
    let swarm_home = crate::swarm::default_home();
    let swarm_registration = crate::swarm::Registration {
        schema: crate::swarm::REGISTRATION_SCHEMA.to_string(),
        session_id: swarm_session_id.clone(),
        pid: std::process::id(),
        cwd: cwd.display().to_string(),
        lane: lane_name.clone(),
        model: None,
        role: "root".to_string(),
        parent: None,
        worktree: None,
        status: None,
        inbox: loaded
            .store
            .directory()
            .join("inbox.jsonl")
            .display()
            .to_string(),
        alive_after_ms: crate::swarm::DEFAULT_ALIVE_AFTER_MS,
        started_at_ms: crate::swarm::now_ms(),
        heartbeat_at_ms: crate::swarm::now_ms(),
    };
    if let Err(why) = crate::swarm::register(&swarm_home, &swarm_registration) {
        ui.entries.push(Entry::new(
            Role::Notice,
            format!("Swarm registration failed: {why}"),
        ));
    }
    let mut local_store = Some(loaded.store);
    let mut has_account = false;
    if options.dev {
        ui.entries.push(Entry::new(
            Role::Notice,
            "Local coder API. Provider keys: AI_GATEWAY_API_KEY and OPENROUTER_API_KEY.",
        ));
        if let Some(token) = crate::coder::runtime::user_token() {
            let origin = crate::coder::runtime::api_base();
            let origin = origin.trim_end_matches("/api/v1").trim_end_matches('/');
            ui.identity = resolve_identity(origin, &Secret::new(token)).await;
            has_account = true;
        }
    } else if let Some(token) = crate::coder::runtime::user_token() {
        let endpoint = crate::auth::resolve_endpoint(None, None)?;
        let token = Secret::new(token);
        if validate_token(&endpoint.origin, &token).await.is_ok() {
            // Who, before the screen opens. A token that authenticates is not
            // yet an account name, and the row may only show one the server
            // gave.
            ui.identity = resolve_identity(&endpoint.origin, &token).await;
            has_account = true;
        } else {
            // A stored token the deployment refused. The row says the
            // credential is unverified rather than naming whoever it was
            // issued to: a login drawn over a dead token is a session that
            // looks live while every turn fails.
            ui.identity = crate::coder::tui::Identity::Unverified;
            ui.entries.push(Entry::new(
                Role::Notice,
                "Stored token did not authenticate. Sign in with /login.",
            ));
        }
    }
    if !options.lane_explicit && local_lane_model.is_none() && !has_account {
        ui.entries.push(Entry::new(
            Role::Notice,
            crate::runtime::OLLAMA_INSTALL_SIGN.to_string(),
        ));
    }
    if let Some(error) = mouse_error {
        ui.entries.push(Entry::new(Role::Notice, error));
    }
    if options.cloud_history && lane.is_local() {
        ui.entries.push(Entry::new(
            Role::Notice,
            "Local chats stay on this machine. --cloud-history does not upload a local session.",
        ));
    }
    if !has_account && !lane.is_local() && !lane.uses_nitro_origin() && !options.dev {
        ui.entries
            .push(Entry::new(Role::Notice, HOSTED_NEEDS_SIGN_IN));
    }
    let opened = attach_session(
        &lane,
        &lane_name,
        &reasoning,
        &agents,
        &tx,
        &mut local_store,
        &restored_events,
        cloud_history,
        &snapshot_text,
        capability_notice.as_deref(),
    );
    let mut session: Option<Arc<Mutex<Session>>> = Some(Arc::new(Mutex::new(opened)));

    // What the account holds before this session has spent anything, so the
    // bottom row carries a balance from the first frame rather than only after
    // a turn.
    if session.is_some() {
        refresh_credit(&tx);
    }

    // Lane changes describe the current selection, not a sequence of events.
    // Keep its one transcript entry so rapid Shift+Tab presses do not bury the
    // conversation beneath stale selections.
    let mut lane_notice = None;
    let mut turns = TurnState::default();
    // Session-level autopilot mode (`coder/autopilot.rs`). Session-only by
    // design: nothing persists it, so a new session opens human-steered.
    let mut autopilot_frame = AutopilotFrame {
        state: crate::coder::autopilot::AutopilotState::default(),
        closed: Vec::new(),
        last_picked: None,
        last_heartbeat: None,
    };
    let mut active_turn: Option<ActiveTurn> = None;
    let mut prompt_queue = VecDeque::new();
    // The frame-loop half of the swarm heartbeat (#339); the turn loop holds
    // the other half.
    let mut last_swarm_beat = std::time::Instant::now();
    let prompt = options.prompt;
    let one_shot = prompt.is_some();
    if let Some(text) = prompt {
        prompt_queue.push_back(QueuedPrompt {
            text,
            images: Vec::new(),
        });
    }
    if !prompt_queue.is_empty() {
        start_next_prompt(
            &mut ui,
            &mut prompt_queue,
            session.as_ref(),
            &tx,
            &mut turns,
            &mut active_turn,
            options.dev,
        )
        .await;
    }
    let mut login_pending = false;
    let mut exit_after_cancel = false;
    // ToolDone from this drain is applied after the next draw. Output+Done
    // otherwise land in one try_recv burst and the active rail is never
    // painted (#256).
    let mut held_tool_done: Vec<Control> = Vec::new();
    // Read keys through EventStream, not poll(0)+read. poll(0) on macOS
    // with use-dev-tty never reports a pending key, so the composer paints
    // and typing never reaches it (0.2.0-rc.10, install.sh exec). A
    // blocking poll(50ms) did see keys, and it also parked the tokio
    // runtime so spawned HTTP (#334 served_models, --dev SSE) could not
    // run. EventStream waits for input without blocking the runtime.
    let mut events = EventStream::new();

    'frame: loop {
        let flushed_done = std::mem::take(&mut held_tool_done);
        let mut rest = Vec::new();
        while let Ok(control) = rx.try_recv() {
            if control.settles_tool() {
                held_tool_done.push(control);
            } else {
                rest.push(control);
            }
        }
        for control in flushed_done.into_iter().chain(rest) {
            match control {
                Control::Turn { id, event } => {
                    if !turns.accepts(id) {
                        continue;
                    }
                    let terminal = matches!(*event, Control::Done);
                    apply(&mut ui, *event);
                    if terminal {
                        if let Err(error) = write_atif_snapshot(&ui, &atif_directory) {
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!("The local ATIF snapshot could not be written: {error}"),
                            ));
                        }
                        turns.apply(TurnAction::ObserveTerminal(id));
                        active_turn = None;
                        if one_shot {
                            break 'frame;
                        }
                        refresh_credit(&tx);
                        // Autopilot's minimal loop (spec §13 slice 1): a
                        // turn ended with the mode engaged, so the next
                        // iteration starts in this same motion rather than
                        // waiting on a human. Disengage commands are read at
                        // the boundary by exactly this check — a mode that
                        // was switched off mid-turn does not start another
                        // unit, which is what makes the chord a real off
                        // switch. Queued human prompts win over the loop:
                        // they were typed while the turn ran, and the loop
                        // works for the reader, not instead of them.
                        if autopilot_frame.state.engaged && prompt_queue.is_empty() {
                            // The budget and stop conditions are checked
                            // before next-unit selection, never mid-unit
                            // (spec §7). The goal ledger is the primary
                            // budget signal: `goal_budget_exhausted` is the
                            // store's own BudgetLimited state, which the
                            // turn just updated with its usage.
                            let mail = session
                                .as_ref()
                                .and_then(|s| s.try_lock().ok())
                                .and_then(|s| s.latest_boundary_mail());
                            if let Some(receipt) = mail.as_ref()
                                && !receipt.consumed.is_empty()
                            {
                                ui.entries
                                    .push(Entry::new(Role::Notice, receipt.announce_line()));
                            }
                            let mail_stop = mail
                                .as_ref()
                                .and_then(|receipt| autopilot_frame.state.observe_mail(receipt));
                            let goal_budget_exhausted = session
                                .as_ref()
                                .and_then(|s| s.try_lock().ok())
                                .and_then(|s| s.goal())
                                .is_some_and(|goal| {
                                    goal.status == crate::coder::goal::GoalStatus::BudgetLimited
                                });
                            match mail_stop.or_else(|| {
                                autopilot_frame
                                    .state
                                    .stops
                                    .should_stop(goal_budget_exhausted)
                            }) {
                                Some(reason) => {
                                    // A stop is a report, not a halt: the
                                    // ledger state above is current, the
                                    // mode hands the wheel back, and the
                                    // session waits like a normal one.
                                    autopilot_frame.state.engaged = false;
                                    autopilot_frame.state.directive = None;
                                    ui.autopilot_engaged = false;
                                    ui.entries.push(Entry::new(Role::Notice, reason.line()));
                                }
                                None => {
                                    // The boundary heartbeat (spec §5):
                                    // what closed here, what is about to be
                                    // picked. A failed send is counted, not
                                    // swallowed; enough failures stop the
                                    // mode at the next boundary.
                                    send_heartbeat(
                                        &mut ui,
                                        crate::coder::autopilot::Heartbeat {
                                            closed: autopilot_frame.closed.last().cloned(),
                                            picked: autopilot_frame.last_picked.clone(),
                                            claim: None,
                                        },
                                        &mut autopilot_frame.state.stops,
                                        &mut autopilot_frame.last_heartbeat,
                                    );
                                    let live =
                                        session.as_ref().expect("a turn ran, so a session exists");
                                    refresh_workspace_snapshot(live, &cwd).await;
                                    let prompt = autopilot_frame.state.iteration_prompt();
                                    start_prompt(
                                        &mut ui,
                                        prompt,
                                        Vec::new(),
                                        live,
                                        &tx,
                                        &mut turns,
                                        &mut active_turn,
                                        options.dev,
                                    )
                                    .await;
                                    update_activity(&mut ui, &turns, prompt_queue.len());
                                    continue;
                                }
                            }
                        }
                        start_next_prompt(
                            &mut ui,
                            &mut prompt_queue,
                            session.as_ref(),
                            &tx,
                            &mut turns,
                            &mut active_turn,
                            options.dev,
                        )
                        .await;
                    }
                }
                Control::CancelComplete { id, diagnostic } => {
                    if matches!(
                        turns.apply(TurnAction::CompleteCancel(id)),
                        TurnEffect::ReturnedIdle(_)
                    ) {
                        ui.loading = false;
                        ui.turn_settled();
                        ui.waiting = None;
                        let mut canceled = Entry::new(Role::Notice, "Turn canceled.");
                        canceled.turn_id = Some(id.get());
                        ui.entries.push(canceled);
                        if let Some(diagnostic) = diagnostic {
                            ui.entries.push(Entry::new(Role::Notice, diagnostic));
                        }
                        ui.entries.push(Entry::new(
                            Role::Notice,
                            commands::CONTINUE_HINT.to_string(),
                        ));
                        if let Err(error) = write_atif_snapshot(&ui, &atif_directory) {
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!("The local ATIF snapshot could not be written: {error}"),
                            ));
                        }
                        refresh_credit(&tx);
                        if exit_after_cancel {
                            break 'frame;
                        }
                        start_next_prompt(
                            &mut ui,
                            &mut prompt_queue,
                            session.as_ref(),
                            &tx,
                            &mut turns,
                            &mut active_turn,
                            options.dev,
                        )
                        .await;
                    }
                }
                Control::Login(result) => {
                    login_pending = false;
                    match result {
                        Ok(message) => {
                            ui.entries.push(Entry::new(Role::Notice, message));
                            // A successful device authorization supplies a
                            // credential, not an account name. Ask the server
                            // which account it belongs to before changing the
                            // identity row.
                            ui.identity = current_identity().await;
                            if let Some(existing) = &session {
                                existing.lock().await.apply_login_credential();
                            } else {
                                let opened = attach_session(
                                    &lane,
                                    &lane_name,
                                    &reasoning,
                                    &agents,
                                    &tx,
                                    &mut local_store,
                                    &restored_events,
                                    cloud_history,
                                    &snapshot_text,
                                    capability_notice.as_deref(),
                                );
                                session = Some(Arc::new(Mutex::new(opened)));
                            }
                            refresh_credit(&tx);
                        }
                        Err(error) => ui
                            .entries
                            .push(Entry::new(Role::Notice, format!("Login failed: {error}"))),
                    }
                }
                control => apply(&mut ui, control),
            }
        }

        let completed_frame = terminal.draw(|f| {
            let size = f.area();
            ui.render(f, size);
        })?;

        // Keep this session's swarm registration believable between turns.
        // The other beat is per-turn (in `drain_swarm_inbox`), so an open but
        // idle session would otherwise age out of its alive window and read
        // stale while sitting right there on screen (#339). Once a minute is
        // far inside the 30-minute window and costs one file rewrite.
        if last_swarm_beat.elapsed() >= std::time::Duration::from_secs(60) {
            let _ = crate::swarm::heartbeat(&swarm_home, &swarm_session_id);
            last_swarm_beat = std::time::Instant::now();
        }

        // ratatui has no hyperlink concept, so repaint the link runs as OSC 8
        // sequences over the frame it just flushed. `emit` re-reads the text
        // out of the buffer, so this can never change what a cell says.
        if !ui.links.is_empty() {
            // `draw` swaps buffers before returning. `current_buffer_mut()` is
            // therefore the cleared buffer for the next frame, not the frame
            // the reader can see. Repainting from it erases every link run.
            let buffer = completed_frame.buffer.clone();
            let mut out = std::io::stdout();
            let _ = crate::coder::osc8::emit(&mut out, &ui.links, &buffer);
            // The link pass parks the cursor at the last link it repaints.
            // Put it back where the frame left it — the composer caret — or
            // the terminal cursor sits mid-transcript and flickers on every
            // spinner tick (#187). Reading the position back instead would
            // return exactly the wrong place we just moved it to.
            if let Some(position) = ui.cursor {
                let _ = write!(out, "{}", crate::coder::osc8::cursor_restore(position));
                let _ = out.flush();
            }
        }

        let event = tokio::select! {
            event = events.next() => match event {
                Some(Ok(ev)) => ev,
                Some(Err(error)) => return Err(error.into()),
                None => break 'frame,
            },
            _ = async {
                if let Some(active) = active_turn.as_mut() {
                    tokio::select! {
                        _ = &mut active.task => {}
                        _ = tokio::time::sleep(Duration::from_millis(20)) => {}
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            } => {
                if active_turn.as_ref().is_some_and(|active| active.task.is_finished()) {
                    active_turn = None;
                }
                continue;
            }
        };
        let key = match event {
            Event::Paste(text) => {
                // Paste is one editing operation, even when it contains
                // newlines. In particular, do not pass its newlines through
                // the Enter path below: they belong in the prompt.
                match ui.attach_dropped_images(&text) {
                    Ok(true) => {}
                    Ok(false) => ui.composer.insert_str(&normalize_paste(&text)),
                    Err(error) => ui.entries.push(Entry::new(Role::Notice, error)),
                }
                history.stop_walking();
                continue;
            }
            Event::Key(key) => key,
            Event::Mouse(mouse) => {
                apply_mouse(&mut ui, mouse);
                continue;
            }
            _ => continue,
        };
        if !is_editing_key_event(&key) {
            continue;
        }

        let width = composer_width(terminal.size()?.width);

        // The model picker owns the keyboard while it is open (issues
        // #323/#324): type-to-filter, arrows to move, Enter to commit, Esc
        // to dismiss. It runs before every other chord because a modal that
        // leaks keystrokes to what is underneath is a modal that edits a
        // draft the reader cannot see.
        if ui.model_picker.is_some() {
            let Some(picker) = ui.model_picker.as_mut() else {
                unreachable!("just checked");
            };
            match key.code {
                KeyCode::Esc => {
                    ui.model_picker = None;
                }
                KeyCode::Enter => {
                    let selected = picker.selected_item().map(|item| item.id.clone());
                    if let Some(id) = selected {
                        commit_model_picker(&session, &mut ui, &mut lane, &id);
                    }
                }
                KeyCode::Up => picker.move_selection(-1),
                KeyCode::Down => picker.move_selection(1),
                KeyCode::Backspace => picker.pop_char(),
                KeyCode::Char(character) => {
                    picker.push_char(character);
                }
                _ => {}
            }
            continue;
        }

        // Escape requests turn cancellation. It never exits Coder, and the
        // reducer makes a repeated request idempotent. A visible selection
        // steals Esc first, matching grok-build: dismissing what is on
        // screen outranks canceling what is running in the background.
        if key.code == KeyCode::Esc {
            if ui.selection.selection().is_some() {
                ui.selection.clear();
                continue;
            }
            request_cancel(
                &mut ui,
                &mut turns,
                &mut active_turn,
                session.as_ref(),
                &tx,
                prompt_queue.len(),
            );
            continue;
        }

        // Leaving is checked before the composer sees the key, so Ctrl+C is
        // never swallowed by an editing chord. Exit remains a separate action
        // from Escape's turn cancellation.
        if is_quit(&key) {
            if matches!(turns.phase(), TurnPhase::Idle) {
                break;
            }
            prompt_queue.clear();
            exit_after_cancel = true;
            update_activity(&mut ui, &turns, prompt_queue.len());
            request_cancel(
                &mut ui,
                &mut turns,
                &mut active_turn,
                session.as_ref(),
                &tx,
                prompt_queue.len(),
            );
            continue;
        }

        // Ctrl+Y copies the selection, grok-build's `y`-copies-the-focused-
        // block idea moved onto the modifier plane this TUI already uses for
        // its non-editing chords. Esc below clears the selection instead of
        // canceling a turn when a selection is what Esc would dismiss.
        if key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(text) = ui.selection.copy_text() {
                let outcome = crate::coder::clipboard::copy_text_with_backup(&text);
                ui.toast = Some((outcome.toast_message(), ui.tick + CLIPBOARD_TOAST_TICKS));
            }
            continue;
        }

        // Meta+A / Alt+A toggles autopilot (spec: `docs/coder/autopilot.md`
        // §3). It is handled here, beside Ctrl+Y, rather than in the
        // composer: it is a mode chord, not an edit, and it must reach the
        // frame even while a turn is running — the reader who wants the wheel
        // back mid-turn gets it at the next boundary, and a mode that could
        // not be told to stop until the model stopped talking would not be a
        // mode a person walks away from.
        if key.code == KeyCode::Char('a')
            && key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::META)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            autopilot_frame.state.engaged = !autopilot_frame.state.engaged;
            if !autopilot_frame.state.engaged {
                autopilot_frame.state.directive = None;
            }
            ui.autopilot_engaged = autopilot_frame.state.engaged;
            ui.entries.push(Entry::new(
                Role::Notice,
                if autopilot_frame.state.engaged {
                    "Autopilot engaged: the loop keeps steering between turns. \
                     Meta+A or /autopilot off hands the wheel back."
                        .to_string()
                } else {
                    "Autopilot disengaged: the session waits for you after each turn.".to_string()
                },
            ));
            continue;
        }

        // Ctrl+P opens the model picker (issues #323/#324), grok-build's
        // second entry into the same surface. Two departures from grok,
        // both forced: our composer is always the focused widget, so grok's
        // "multiline when focused, picker otherwise" split has no second
        // half; and Ctrl+M is unusable — crossterm maps the raw Ctrl+M byte
        // (0x0D) to Enter before the app ever sees it, so a Ctrl+M binding
        // would be dead code in every terminal. Ctrl+P is unbound in the
        // composer and the frame, and the /help table carries it.
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let resolved = (!ui.model.is_empty()).then(|| ui.model.clone());
            open_model_picker(&lane, resolved.as_deref(), local_lane_model.as_deref(), &tx);
            ui.model_picker = Some(seed_model_picker(&lane, local_lane_model.as_deref()));
            continue;
        }

        match ui.composer.handle_key(&key, width) {
            ComposerAction::Submit(text) => {
                history.record(&text);
                history.stop_walking();
                let mut asked_to_log_out = false;
                if let Some(session) = &session {
                    if !text.trim().is_empty() {
                        let outcome = submit(
                            &mut ui,
                            text,
                            &mut lane,
                            session,
                            &tx,
                            &cwd,
                            &mut turns,
                            &mut active_turn,
                            &mut prompt_queue,
                            &mut autopilot_frame,
                            options.dev,
                            local_lane_model.as_deref(),
                        )
                        .await;
                        match outcome {
                            commands::Outcome::Login => {
                                begin_login(&mut login_pending, &mut ui, &tx);
                            }
                            commands::Outcome::Logout => asked_to_log_out = true,
                            commands::Outcome::QueueStatus => {
                                ui.entries.push(Entry::new(
                                    Role::Output,
                                    queue_status(prompt_queue.len()),
                                ));
                            }
                            commands::Outcome::ClearQueue => {
                                let removed = prompt_queue.len();
                                prompt_queue.clear();
                                update_activity(&mut ui, &turns, 0);
                                ui.entries.push(Entry::new(
                                    Role::Output,
                                    format!(
                                        "Cleared {removed} queued prompt{}.",
                                        if removed == 1 { "" } else { "s" }
                                    ),
                                ));
                            }
                            commands::Outcome::ContinueFrom { spec } => {
                                if !matches!(turns.phase(), TurnPhase::Idle) {
                                    ui.entries.push(Entry::new(
                                        Role::Notice,
                                        "The running turn must finish before `/continue` can start a new session.",
                                    ));
                                } else {
                                    match session.try_lock() {
                                        Ok(mut live) => {
                                            let root = crate::session_store::default_root();
                                            match live.continue_from_checkpoint(&root, &cwd, &spec)
                                            {
                                                Ok(notice) => {
                                                    ui.entries.clear();
                                                    ui.scroll_override = None;
                                                    ui.show_welcome = false;
                                                    if let Some(summary) =
                                                        live.local_session_summary()
                                                    {
                                                        ui.local_session_path = Some(
                                                            crate::session_store::cwd_session_directory(
                                                                &root, &cwd,
                                                            )
                                                            .join(&summary.id)
                                                            .display()
                                                            .to_string(),
                                                        );
                                                    }
                                                    ui.entries
                                                        .push(Entry::new(Role::Notice, notice));
                                                }
                                                Err(why) => {
                                                    ui.entries.push(Entry::new(Role::Notice, why));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            ui.entries.push(Entry::new(
                                                Role::Notice,
                                                "The running turn must finish before `/continue` can start a new session.",
                                            ));
                                        }
                                    }
                                }
                            }
                            commands::Outcome::Done => {}
                        }
                    }
                } else if text.trim() == "/login" {
                    begin_login(&mut login_pending, &mut ui, &tx);
                } else if !text.trim().is_empty() {
                    ui.entries
                        .push(Entry::new(Role::Notice, HOSTED_NEEDS_SIGN_IN));
                }

                // `/logout` is run here rather than in the dispatch because the
                // session is owned here: the credential goes, and the thread
                // that credential opened has to be ended by reporting first, on
                // the same grace the exit uses. The session is taken before the
                // work and put back if it could not be done, so a `/logout` that
                // did not happen never leaves the frame believing it did.
                if asked_to_log_out && let Some(active) = session.take() {
                    let notice = tokio::time::timeout(REVOCATION_GRACE, async {
                        commands::logout(&mut *active.lock().await).await
                    })
                    .await;
                    match notice {
                        Ok(notice) => {
                            // The credential is gone, so the row must stop
                            // naming the account it belonged to. Neither of
                            // these lines existed in the commit that added
                            // `/logout`: the row carried token counts then, and
                            // nothing on it survived a logout. Now it carries
                            // the two facts that do — who you are and what the
                            // account has left — and leaving either up after
                            // the token went is the failure both fields were
                            // built to prevent. An account name nobody can
                            // authenticate reads as a working session, and a
                            // balance that has stopped refreshing looks exactly
                            // like a current one.
                            //
                            // `Unread` rather than `Unavailable`: there is no
                            // account to have failed to read. The field goes
                            // absent, which is what it says before the first
                            // read and what it should say once there is nothing
                            // to read.
                            ui.identity = crate::coder::tui::Identity::Anonymous;
                            ui.credit = crate::coder::credit::CreditField::Unread;
                            active.lock().await.clear_credential();
                            session = Some(active);
                            ui.entries.push(Entry::new(Role::Notice, notice));
                        }
                        Err(_) => {
                            session = Some(active);
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!(
                                    "Still logged in: this session was working after {}s, so its \
                                     thread could not be ended and its token was left alone. Run \
                                     `/logout` again once the turn has finished.",
                                    REVOCATION_GRACE.as_secs()
                                ),
                            ));
                        }
                    }
                }
            }
            ComposerAction::Redraw => history.stop_walking(),
            ComposerAction::Moved => {}
            // Shift+Tab moves between the switchable lanes. It is handled here
            // rather than in `handle_session_key` because it has to reach the
            // session, and the session is behind an async lock.
            //
            // The walk is gated (`cycle_gated`, #291): `local` is a member
            // only when the open-time probe found an Ollama server with
            // models on it. A machine without one walks flash, pro, free,
            // flash (#298).
            ComposerAction::Ignored if key.code == KeyCode::BackTab => {
                lane = lane.cycle_gated(local_lane_model.clone());
                ui.lane = lane.label();
                // Nothing has answered on the new lane yet. Carrying the old
                // model across would leave the row naming a model this lane
                // never asked for, which is the one thing the row must not do.
                ui.model.clear();
                if let Some(session) = &session {
                    let mut live = session.lock().await;
                    live.set_lane(lane.clone());
                    live.set_cloud_history(options.cloud_history);
                }
                ui.cloud_history = options.cloud_history && !lane.is_local();
                record_lane_notice(&mut ui, &mut lane_notice, &lane);
            }
            // The composer did not want it, so it is the session's.
            ComposerAction::Ignored => {
                handle_session_key(&mut ui, &mut history, &key, width, &cwd);
            }
        }
    }

    terminal.hide_cursor()?;
    if let Err(error) = write_atif_snapshot(&ui, &atif_directory) {
        eprintln!("Coder: the local ATIF snapshot could not be written: {error}");
    }

    // The screen is gone, so these land on the normal one. Ending the thread is
    // the point: one left open holds its grant's remaining budget, and the
    // `Drop` backstop can only spawn an ending this process may exit before
    // polling. It ends by reporting what the session did, so leaving is not
    // recorded as a cancellation and the thread can be resumed later.
    if let Some(session) = &session {
        match tokio::time::timeout(REVOCATION_GRACE, async {
            session.lock().await.finish().await
        })
        .await
        {
            Ok(Ok(Some(line))) => println!("{line}"),
            Ok(Ok(None)) => {}
            Ok(Err(error)) => eprintln!("Coder: the thread was not ended: {error}"),
            Err(_) => eprintln!(
                "Coder: the session was still working after {}s, so its thread was left \
                 to the best-effort ending.",
                REVOCATION_GRACE.as_secs()
            ),
        }
    }
    // Leave the swarm before returning: a registration naming a session that
    // no longer exists is the stale state every other member then has to
    // interpret. Best-effort — the exit path owns neither the filesystem nor
    // a network.
    let _ = crate::swarm::unregister(&crate::swarm::default_home(), &swarm_session_id);
    Ok(())
}

/// One prompt, no terminal: the `--prompt` path for pipes, CI, and headless
/// tests (#338, built on #333).
///
/// This mirrors the session setup the TUI performs — local store, snapshot
/// seed, swarm registration, lane resolution — and then drives one turn the
/// same way the frame does: mint the turn id through the reducer, stream
/// [`Control`] events to stdout as plain lines, and end the thread before
/// exiting. What it deliberately does not do is draw. Nothing here touches
/// the alternate screen, raw mode, or the event queue, so the same binary is
/// safe to call from a script that captures stdout.
async fn run_one_shot(
    options: SessionOptions,
    prompt: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = crate::session_store::default_root();
    let mut loaded = match options.resume.as_deref() {
        None => crate::session_store::LocalSessionStore::create(
            &root,
            &cwd,
            &options.lane_name,
            options.reasoning.clone(),
            options.cloud_history,
        )?,
        Some("") => crate::session_store::LocalSessionStore::load_last(&root, &cwd)?
            .ok_or_else(|| format!("No saved Coder session exists for {}.", cwd.display()))?,
        Some(id) => crate::session_store::LocalSessionStore::load_id(&root, &cwd, id)?
            .ok_or_else(|| format!("No saved Coder session has id `{id}`."))?,
    };
    let resumed = options.resume.is_some();
    let lane_name = if resumed && !options.lane_explicit {
        loaded.summary.lane.clone()
    } else {
        options.lane_name.clone()
    };
    let reasoning = if resumed && options.reasoning.is_none() {
        loaded.summary.reasoning.clone()
    } else {
        options.reasoning.clone()
    };
    let lane = Lane::from_str(&lane_name);
    let reasoning = reasoning.or_else(|| lane.default_reasoning().map(str::to_string));
    loaded.store.set_lane(&lane_name)?;
    loaded.store.set_reasoning(reasoning.as_deref())?;
    let cloud_history = options.cloud_history && !lane.is_local();
    loaded.store.set_cloud_history(cloud_history)?;

    let (tx, rx) = mpsc::channel::<Control>();
    let agents = crate::coder::acp::find_agents().await.unwrap_or_default();
    let mut session = Session::open(
        lane.clone(),
        &lane_name,
        reasoning.clone(),
        agents,
        crate::coder_dev::door_speaks_openresponses(),
        tx.clone(),
    );
    // Kept before the store moves into the session: the swarm registration
    // and the inbox path are keyed on this directory.
    let store_directory = loaded.store.directory().to_path_buf();
    let swarm_session_id = loaded.summary.id.clone();
    let checkpoint = loaded.summary.last_checkpoint.clone();
    session = session.with_local_session(loaded.store, &loaded.events, cloud_history);
    let snapshot_text =
        crate::coder::snapshot::workspace_snapshot(&cwd, checkpoint.as_deref()).await;
    session.seed_workspace_snapshot(&snapshot_text);
    if let Some(notice) = crate::plugins::session_start_capability_notice(
        &crate::plugins::discover_catalog(&cwd),
        crate::plugins::Approval {
            mounts_allowed: true,
        },
    ) {
        session.seed_capability_notice(&notice);
    }

    // The swarm registration the TUI would have made. A headless one-shot
    // is still a session a neighbor might want to reach, and the same
    // best-effort rule holds: failure is a printed warning, never fatal.
    let swarm_home = crate::swarm::default_home();
    let swarm_registration = crate::swarm::Registration {
        schema: crate::swarm::REGISTRATION_SCHEMA.to_string(),
        session_id: swarm_session_id.clone(),
        pid: std::process::id(),
        cwd: cwd.display().to_string(),
        lane: lane_name.clone(),
        model: None,
        role: "root".to_string(),
        parent: None,
        worktree: None,
        status: None,
        inbox: store_directory.join("inbox.jsonl").display().to_string(),
        alive_after_ms: crate::swarm::DEFAULT_ALIVE_AFTER_MS,
        started_at_ms: crate::swarm::now_ms(),
        heartbeat_at_ms: crate::swarm::now_ms(),
    };
    if let Err(why) = crate::swarm::register(&swarm_home, &swarm_registration) {
        eprintln!("Coder: swarm registration failed: {why}");
    }

    if !options.dev && !lane.is_local() && !lane.uses_nitro_origin() && !session.has_user_token() {
        eprintln!("{HOSTED_NEEDS_SIGN_IN}");
        let spent_line = tokio::time::timeout(REVOCATION_GRACE, session.finish())
            .await
            .map_err(|_| "Coder: the session was still working; its thread was left open.")?;
        match spent_line {
            Ok(Some(line)) => println!("{line}"),
            Ok(None) => {}
            Err(error) => eprintln!("Coder: the thread was not ended: {error}"),
        }
        let _ = crate::swarm::unregister(&crate::swarm::default_home(), &swarm_session_id);
        return Ok(());
    }

    if let Some(advice) = commands::atif_export_advice(&prompt) {
        eprintln!("Coder: {advice}");
    }

    // Mint the turn through the same reducer the frame uses, then run it to
    // completion on the runtime task. The stream ends when `execute_turn`
    // returns, which is always after exactly one Done.
    let mut turns = TurnState::default();
    let TurnEffect::Started(id) = turns.apply(TurnAction::Start) else {
        return Err("the turn could not be started".into());
    };
    let turn_task = tokio::spawn(async move {
        session
            .execute_turn_with_id_and_images(id, &prompt, &[], tx.clone())
            .await;
        session
    });
    // Drain the stream, printing as the turn speaks. Chunk lines go out as
    // they arrive so a caller following the output sees the answer stream;
    // everything else is a status line the transcript also carries.
    //
    // Stop at Done. The session still holds a Sender after the turn, so
    // waiting for the channel to disconnect never returns (#343). The TUI
    // loop already breaks on Done; this path has to as well.
    //
    // Flush every write. stdout is fully buffered when it is not a TTY.
    while let Ok(control) = rx.recv() {
        let ended = one_shot_control_ends_the_turn(&control);
        match control {
            Control::Turn { event, .. } => match *event {
                Control::Chunk(chunk) => {
                    print!("{chunk}");
                    let _ = stdout().flush();
                }
                Control::Failed(error) => {
                    eprintln!("Coder: {error}");
                    let _ = stderr().flush();
                }
                Control::Notice(notice) => {
                    eprintln!("Coder: {notice}");
                    let _ = stderr().flush();
                }
                _ => {}
            },
            Control::Failed(error) => {
                eprintln!("Coder: {error}");
                let _ = stderr().flush();
            }
            Control::Notice(notice) => {
                eprintln!("Coder: {notice}");
                let _ = stderr().flush();
            }
            _ => {}
        }
        if ended {
            break;
        }
    }
    let mut returned_session = turn_task
        .await
        .map_err(|error| format!("turn task: {error}"))?;
    println!();

    // The screen is gone before this would matter, and a thread left open
    // holds its grant's remaining budget. The session came back from the
    // turn task, so its thread can be ended properly rather than dropped.
    let spent_line = tokio::time::timeout(REVOCATION_GRACE, returned_session.finish())
        .await
        .map_err(|_| "Coder: the session was still working; its thread was left open.")?;
    match spent_line {
        Ok(Some(line)) => println!("{line}"),
        Ok(None) => {}
        Err(error) => eprintln!("Coder: the thread was not ended: {error}"),
    }
    let _ = crate::swarm::unregister(&crate::swarm::default_home(), &swarm_session_id);
    Ok(())
}

fn write_atif_snapshot(ui: &CoderUi, directory: &std::path::Path) -> std::io::Result<usize> {
    let Some(_) = ui.local_session_id.as_deref() else {
        return Ok(0);
    };
    // Derive the durable ATIF from the authoritative journal rather than the
    // visible viewport. `/clear` may remove entries from the screen, but it
    // must not erase history from the portable session artifact.
    let loaded = crate::session_store::LocalSessionStore::load_path(directory)?;
    let mut transcript = CoderUi::new();
    restore_entries(&mut transcript, &loaded.events);
    let model = loaded
        .summary
        .last_model
        .as_deref()
        .filter(|model| !model.is_empty())
        .or_else(|| (!ui.model.is_empty()).then_some(ui.model.as_str()))
        .unwrap_or("unknown");
    crate::coder::export::write_session_trajectory(
        &transcript.entries,
        model,
        &ui.repo,
        &ui.branch,
        &loaded.summary.id,
        directory,
    )
}

/// Ask the deployment what the account has left, off the frame loop.
///
/// Spawned rather than awaited, because a slow deployment must not hold a
/// frame: the answer arrives as a [`Control`] like everything else. The
/// outcome is sent either way, including when there is no credential to ask
/// with — the bottom row has to stop showing a figure it can no longer
/// confirm, and `None` is what tells it to.
fn refresh_credit(tx: &Sender<Control>) {
    let token = crate::coder::runtime::user_token();
    let base = crate::coder::runtime::api_base();
    let tx = tx.clone();

    tokio::spawn(async move {
        let outcome = match token {
            Some(token) => crate::coder::credit::fetch(&base, &token).await,
            None => None,
        };
        let _ = tx.send(Control::Credit(outcome));
    });
}

fn begin_login(login_pending: &mut bool, ui: &mut CoderUi, tx: &Sender<Control>) {
    if *login_pending {
        ui.entries.push(Entry::new(
            Role::Notice,
            "OpenAgents sign-in is already in progress.",
        ));
        return;
    }
    *login_pending = true;
    ui.entries
        .push(Entry::new(Role::Notice, "Starting OpenAgents sign-in..."));
    spawn_session_login(tx);
}

#[allow(clippy::too_many_arguments)]
fn attach_session(
    lane: &Lane,
    lane_name: &str,
    reasoning: &Option<String>,
    agents: &[crate::coder::acp::Agent],
    tx: &Sender<Control>,
    local_store: &mut Option<crate::session_store::LocalSessionStore>,
    restored_events: &[crate::session_store::StoredEvent],
    cloud_history: bool,
    snapshot_text: &str,
    capability_notice: Option<&str>,
) -> Session {
    let mut opened = Session::open(
        lane.clone(),
        lane_name,
        reasoning.clone(),
        agents.to_vec(),
        crate::coder_dev::door_speaks_openresponses(),
        tx.clone(),
    );
    opened = match local_store.take() {
        Some(store) => opened.with_local_session(store, restored_events, cloud_history),
        None => opened,
    };
    opened.seed_workspace_snapshot(snapshot_text);
    if let Some(notice) = capability_notice {
        opened.seed_capability_notice(notice);
    }
    opened
}

/// Open the model picker for the session's lane (issues #323/#324).
///
/// The surface follows grok-build's `/model`: one picker per lane, rows from
/// a per-lane source, opened input-focused. Only Pro and Local have a
/// per-model choice today; any other lane refuses by name rather than
/// offering a gateway tier as if it were a choice. The item source is
/// fetched off the frame loop — a slow catalog or Ollama probe must not
/// hold a frame — so the picker opens `loading` and fills in, or reports
/// the refusal as a notice like every other Control.
fn open_model_picker(
    lane: &Lane,
    resolved_model: Option<&str>,
    cached_local: Option<&str>,
    tx: &Sender<Control>,
) {
    let tx = tx.clone();
    let lane = lane.clone();
    let resolved_model = resolved_model.map(str::to_string);
    let cached_local = cached_local.map(str::to_string);
    tokio::spawn(async move {
        let outcome =
            model_picker_items(&lane, resolved_model.as_deref(), cached_local.as_deref()).await;
        let _ = tx.send(Control::ModelPicker(outcome));
    });
}

/// The per-lane item fetch behind [`open_model_picker`]. Errors are
/// refusals the picker shows as a notice, not panics — a deployment that is
/// briefly unreachable must not take the session down for asking.
async fn model_picker_items(
    lane: &Lane,
    resolved_model: Option<&str>,
    cached_local: Option<&str>,
) -> Result<crate::coder::model_picker::PickerState, String> {
    use crate::coder::model_picker::{LocalModel, PickerState};
    match lane {
        Lane::Local(_) => {
            let details = crate::runtime::installed_local_model_details().await;
            let resolved = match lane {
                Lane::Local(tag) if !tag.is_empty() => Some(tag.as_str()),
                _ => cached_local,
            };
            match details {
                Ok(details) => Ok(PickerState::new(crate::coder::model_picker::local_items(
                    &details
                        .into_iter()
                        .map(|detail| LocalModel {
                            tag: detail.tag,
                            size_bytes: detail.size_bytes,
                            quantization: detail.quantization,
                        })
                        .collect::<Vec<_>>(),
                    resolved,
                ))
                .local()),
                Err(why) => Err(why),
            }
        }
        lane if lane.uses_pro_origin() => {
            let served = probe_served_models_for_picker().await?;
            Ok(PickerState::new(crate::coder::model_picker::pro_items(
                &served,
                resolved_model,
            )))
        }
        _ => Err(format!(
            "The {} lane has no model list to pick from — models are chosen \
             per lane. /model works on Pro and Local.",
            lane.label()
        )),
    }
}

fn model_picker_loading(lane: &Lane) -> crate::coder::model_picker::PickerState {
    let label = if lane.is_local() {
        "probing Ollama…"
    } else {
        "loading models…"
    };
    let picker = crate::coder::model_picker::PickerState::loading(label);
    if lane.is_local() {
        picker.local()
    } else {
        picker
    }
}

fn seed_model_picker(
    lane: &Lane,
    cached_local: Option<&str>,
) -> crate::coder::model_picker::PickerState {
    use crate::coder::model_picker::LocalModel;
    if lane.is_local()
        && let Some(tag) = cached_local
    {
        let resolved = match lane {
            Lane::Local(pinned) if !pinned.is_empty() => Some(pinned.as_str()),
            _ => Some(tag),
        };
        return crate::coder::model_picker::PickerState::new(
            crate::coder::model_picker::local_items(
                &[LocalModel {
                    tag: tag.to_string(),
                    size_bytes: None,
                    quantization: None,
                }],
                resolved,
            ),
        )
        .local();
    }
    model_picker_loading(lane)
}

/// The served-models read behind the Pro picker, run against the origin the
/// lane itself uses. Free-standing: the frame holds no session of its own,
/// and a credential-less session gets the catalog's honest refusal here
/// rather than an empty picker.
async fn probe_served_models_for_picker() -> Result<Vec<crate::coder::runtime::ServedModel>, String>
{
    let base = crate::coder::runtime::api_base();
    let token = crate::coder::runtime::user_token();
    let url = format!("{base}/models");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("could not build an HTTP client: {error}"))?;
    let mut request = client.get(&url);
    if let Some(token) = token {
        request = request.bearer_auth(&token);
    }
    let resp = request
        .send()
        .await
        .map_err(|error| format!("{url} could not be reached: {error}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "{url} refused the model list: {}. Run /login if the credential expired.",
            resp.status()
        ));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|error| format!("{url} sent a body that was not JSON: {error}"))?;
    parse_served_models(&body)
}

/// Parse `GET /api/v1/models`' body into the picker's rows source. Split
/// from the fetch so a fixture test can hold the shape without a wire.
fn parse_served_models(
    body: &serde_json::Value,
) -> Result<Vec<crate::coder::runtime::ServedModel>, String> {
    let models = body
        .get("models")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{}/models sent no model list", "GET"))?;
    Ok(models
        .iter()
        .filter_map(|m| {
            Some(crate::coder::runtime::ServedModel {
                id: m.get("id").and_then(|v| v.as_str())?.to_string(),
                available: m
                    .get("availability")
                    .and_then(|v| v.as_str())
                    .map(|availability| availability == "available")
                    .unwrap_or(false),
                default: m.get("default").and_then(|v| v.as_bool()).unwrap_or(false),
            })
        })
        .collect())
}

/// Commit a picker row: re-pin the session's lane and drop the picker.
///
/// The commit is `set_lane` with the committed pin — the same path shift+tab
/// uses — so the thread is dropped for exactly the reason the shift+tab path
/// documents: the old thread's grant pinned the old model for its whole
/// life, and a model switch that carried the grant over would be a label
/// claiming a model the wire is not talking to.
fn commit_model_picker(
    session: &Option<Arc<Mutex<Session>>>,
    ui: &mut CoderUi,
    lane: &mut Lane,
    id: &str,
) {
    match crate::coder::model_picker::commit_lane(lane, id) {
        Ok(committed) => {
            let Some(session) = session else {
                ui.entries.push(Entry::new(
                    Role::Notice,
                    "No session is open, so the model cannot be pinned.",
                ));
                ui.model_picker = None;
                ui.scroll_override = None;
                return;
            };
            match session.try_lock() {
                Ok(mut session) => {
                    session.set_lane(committed.clone());
                    *lane = committed.clone();
                    ui.lane = committed.label();
                    // Nothing has answered on the committed model yet — the
                    // row must not name a model this lane never asked for.
                    ui.model.clear();
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        model_commit_notice(&committed, id),
                    ));
                }
                Err(_) => ui.entries.push(Entry::new(
                    Role::Notice,
                    "The running turn must finish before the model can change.".to_string(),
                )),
            }
        }
        Err(error) => ui.entries.push(Entry::new(Role::Notice, error.to_string())),
    }
    ui.model_picker = None;
    ui.scroll_override = None;
}

fn model_commit_notice(committed: &Lane, id: &str) -> String {
    if matches!(committed, Lane::Local(_)) {
        format!("Model set to {id}. The next turn talks to Ollama on this machine.")
    } else {
        format!("Model set to {id}. The next turn opens its own thread on it.")
    }
}

/// Start device sign-in without holding the frame loop while the server waits
/// for approval.
fn spawn_session_login(tx: &Sender<Control>) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let progress = tx.clone();
        let result = do_login_with_progress(move |message| {
            let _ = progress.send(Control::Output(message));
        })
        .await
        .map_err(|error| error.to_string());
        let _ = tx.send(Control::Login(result));
    });
}

/// Check that a token is accepted by the deployment without calling GitHub.
/// `GET /api/v1/models` is a light, non-GitHub endpoint that still requires a
/// valid bearer token, so a 200 here means the token is good to spend.
async fn validate_token(origin: &str, token: &Secret) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let url = format!("{}/api/v1/models", origin.trim_end_matches('/'));
    let response = client
        .get(&url)
        .bearer_auth(token.expose())
        .send()
        .await
        .map_err(|error| format!("could not reach {origin}: {error}"))?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    Err(format!("token rejected by {origin} ({status}): {body}").into())
}

/// Who the credential now in hand belongs to, resolved from scratch.
///
/// Called after a login, where the row has just stopped being able to say
/// "not signed in" and has to be told what to say instead. The endpoint and
/// the token are re-read rather than carried, because the login wrote both.
async fn current_identity() -> crate::coder::tui::Identity {
    let Ok(endpoint) = crate::auth::resolve_endpoint(None, None) else {
        return crate::coder::tui::Identity::Unverified;
    };
    let Some(token) = crate::coder::runtime::user_token() else {
        return crate::coder::tui::Identity::Anonymous;
    };
    resolve_identity(&endpoint.origin, &Secret::new(token)).await
}

/// Ask the server who this credential belongs to.
///
/// The same `GET /api/v1/user` that `openagents auth status` reads, so the row
/// under the composer and that command cannot disagree about who is signed in.
/// A refusal or an unreachable server is [`crate::coder::tui::Identity::Unverified`]
/// — the credential exists, nobody confirmed it, and that is what the row will
/// say. It is never an account name.
async fn resolve_identity(origin: &str, token: &Secret) -> crate::coder::tui::Identity {
    let client = crate::repo::RepoClient::new(origin, Some(token.clone()));
    match client.authenticated_user().await {
        Ok(user) => crate::coder::tui::Identity::Named {
            login: user.login,
            id: user.id,
            namespaces: user
                .namespaces
                .into_iter()
                .map(|namespace| namespace.login)
                .collect(),
            expires_at: user.token_expires_at,
        },
        Err(_) => crate::coder::tui::Identity::Unverified,
    }
}

/// Start the GitHub device-authorization flow against the current endpoint,
/// open the approval URL in the browser, poll for the token, store it, and
/// verify it against the model catalog. The token is also placed in
/// `OPENAGENTS_API_KEY` so the runtime spends it without a second store lookup.
///
/// This is called from inside the TUI, so it must not print to stdout; the
/// caller is responsible for showing any message in the transcript.
pub async fn do_login_with_progress(
    progress: impl FnOnce(String),
) -> Result<String, Box<dyn std::error::Error>> {
    let endpoint = crate::auth::resolve_endpoint(None, None)?;
    let client = DeviceClient::new(&endpoint.origin);
    let scopes: &[String] = &[];
    let auth = client.start(scopes).await?;

    progress(open_login_page(&auth, open_browser));

    let token = client.wait(&auth).await?;
    validate_token(&endpoint.origin, &token).await?;

    let store = CredentialStore::for_origin(&endpoint.origin);
    let _ = store.store(&token);

    // SAFETY: this process owns the environment; the TUI has not started here.
    unsafe { std::env::set_var("OPENAGENTS_API_KEY", token.expose()) };

    Ok("Authenticated.".to_string())
}

/// Open one device-authorization page and describe the exact manual fallback.
///
/// The URL and code are present in both branches. A launcher failure changes
/// only the status sentence, so the reader can finish signing in from an SSH
/// session, a container, or a detached terminal.
fn open_login_page(
    auth: &crate::auth::DeviceAuthorization,
    launch: impl FnOnce(&str) -> bool,
) -> String {
    let opened = launch(&auth.verification_uri_complete);
    let status = if opened {
        "Opened the OpenAgents sign-in page in your browser."
    } else {
        "Coder could not open a browser. Open the URL below to sign in to OpenAgents."
    };

    format!(
        "{status}\n\nURL: {}\nCode: {}",
        auth.verification_uri_complete, auth.user_code
    )
}

/// The columns the composer soft-wraps to: the frame's width less its border
/// and the `" > "` gutter. It has to match what `CoderUi::render` uses, or the
/// caret is drawn a column off from where the next character lands.
pub fn composer_width(frame_width: u16) -> usize {
    (frame_width as usize)
        .saturating_sub(2)
        .saturating_sub(3)
        .max(1)
}

fn is_quit(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('c' | 'd' | 'q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    )
}

/// Whether a terminal key event represents an action the frame should apply.
///
/// Terminals that support the enhanced keyboard protocol report held keys as
/// [`KeyEventKind::Repeat`]. Repeat editing and navigation keys so holding an
/// arrow behaves like a normal text field. Keep one-shot session actions such
/// as Tab completion, lane switching, cancellation, and exit on their initial
/// press.
fn is_editing_key_event(key: &KeyEvent) -> bool {
    match key.kind {
        KeyEventKind::Press => true,
        KeyEventKind::Repeat => matches!(
            key.code,
            KeyCode::Char(_)
                | KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
        ),
        KeyEventKind::Release => false,
    }
}

/// Normalize the line endings terminal emulators use for bracketed paste.
///
/// The composer stores hard breaks as `\n`; keeping `\r` would put the caret
/// and wrapping logic out of agreement with the text the submitted turn uses.
fn normalize_paste(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn restore_entries(ui: &mut CoderUi, events: &[crate::session_store::StoredEvent]) {
    for event in events {
        let payload = &event.record.payload;
        let text = |key: &str| {
            payload
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let mut entry = match event.record.event_type.as_str() {
            "turn.user" => Entry::new(Role::You, text("text")),
            "turn.reasoning" => {
                let mut entry = Entry::new(Role::Reasoning, text("text"));
                entry.finish_text();
                entry
            }
            "turn.assistant" => {
                let answer = text("text");
                if answer.is_empty() {
                    continue;
                }
                let mut entry = Entry::new(Role::Assistant, answer);
                entry.model = payload
                    .get("model")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                // A restored answer carries its measured duration only when
                // the store recorded one; older snapshots show the model
                // alone (#216).
                entry.duration_seconds = payload
                    .get("duration_seconds")
                    .and_then(serde_json::Value::as_u64);
                entry.finish_text();
                entry
            }
            "tool.ran" => {
                let call_id = text("call_id");
                let name = text("tool");
                let arguments = text("arguments");
                let output = payload
                    .get("output")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| payload.get("error").and_then(serde_json::Value::as_str))
                    .unwrap_or("")
                    .to_string();
                let parsed = serde_json::from_str(&arguments).unwrap_or_else(
                    |_| serde_json::json!({ "unparsed_arguments": arguments.clone() }),
                );
                let mut entry = Entry::tool_call(tool_title(&name, &arguments));
                entry.output = Some(output.clone());
                entry.tool = Some(ToolCall {
                    call_id,
                    function_name: name,
                    arguments: parsed,
                    output: Some(output),
                    error: payload
                        .get("error")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    done: true,
                    duration_ms: payload
                        .get("duration_ms")
                        .and_then(serde_json::Value::as_u64),
                });
                entry
            }
            "turn.failed" => {
                // The failure record carries what the ATIF notice used to
                // drop (#188): the calls spent and the usage, so a diagnosis
                // reads one line instead of a session-store dig.
                let mut line = text("error");
                let calls = payload.get("calls").and_then(serde_json::Value::as_u64);
                let usage = payload.get("usage");
                if calls.is_some() || usage.is_some() {
                    line.push_str(" (");
                    if let Some(calls) = calls {
                        line.push_str(&format!("{calls} tool calls"));
                    }
                    if let Some(usage) = usage.and_then(|u| u.as_object()) {
                        let total = usage
                            .get("total_tokens")
                            .and_then(serde_json::Value::as_u64);
                        if let Some(total) = total {
                            if calls.is_some() {
                                line.push_str(", ");
                            }
                            line.push_str(&format!("{total} tokens"));
                        }
                    }
                    line.push(')');
                }
                Entry::new(Role::Notice, line)
            }
            "turn.budget" => {
                let phase = text("phase");
                let calls = payload.get("calls").and_then(serde_json::Value::as_u64);
                match phase.as_str() {
                    "reached" => Entry::new(
                        Role::Notice,
                        format!(
                            "Tool-call budget reached ({} calls). The model was asked to finish and report.",
                            calls.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                        ),
                    ),
                    "spent" => Entry::new(
                        Role::Notice,
                        format!(
                            "Turn spending: {} tool calls.",
                            calls.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                        ),
                    ),
                    _ => continue,
                }
            }
            "turn.checkpoint" => {
                let note = text("text");
                if note.trim().is_empty() {
                    continue;
                }
                Entry::new(Role::Notice, format!("Checkpoint: {note}"))
            }
            "turn.notice" => {
                let note = text("text");
                if note.trim().is_empty() {
                    continue;
                }
                Entry::new(Role::Notice, note)
            }
            _ => continue,
        };
        entry.at = event.at_ms;
        ui.entries.push(entry);
    }
}

/// Apply a drained batch the way the live frame loop does.
///
/// [`Control::ToolDone`] from this batch is held in `pending_done` and applied
/// on the next call, so a tool that starts and finishes in one drain still
/// paints one active rail frame.
pub fn apply_drained(
    ui: &mut CoderUi,
    pending_done: &mut Vec<Control>,
    incoming: impl IntoIterator<Item = Control>,
) {
    let flushed = std::mem::take(pending_done);
    let mut rest = Vec::new();
    for control in incoming {
        if control.settles_tool() {
            pending_done.push(control);
        } else {
            rest.push(control);
        }
    }
    for control in flushed.into_iter().chain(rest) {
        apply(ui, control);
    }
}

/// Apply one message from the runtime to the frame.
///
/// Split out of the loop so a test can drive it without a terminal.
///
/// Follow is `scroll_override.is_none()`, the same rule as grok-build's
/// `follow_mode`. Live deltas (tokens, tool output, status) must not clear
/// a pin. The reader resumes follow by scrolling to the bottom or by
/// starting a new prompt.
pub fn apply(ui: &mut CoderUi, control: Control) {
    match control {
        Control::Turn { .. } | Control::CancelComplete { .. } | Control::Login(_) => {}
        Control::Chunk(chunk) => {
            if !chunk.is_empty() {
                // Create the answer where its text actually arrives. A turn
                // can run tools first; filling an assistant placeholder made
                // the final answer appear above those calls on screen and in
                // the exported trajectory even though it was produced last.
                if let Some(last) = ui
                    .entries
                    .last_mut()
                    .filter(|entry| entry.role == Role::Assistant)
                {
                    last.push_text(&chunk);
                } else {
                    ui.entries.push(Entry::new(Role::Assistant, chunk));
                }
            }
        }
        Control::Reasoning(chunk) => {
            if !chunk.is_empty() {
                if let Some(last) = ui
                    .entries
                    .last_mut()
                    .filter(|entry| entry.role == Role::Reasoning)
                {
                    last.push_text(&chunk);
                } else {
                    ui.entries.push(Entry::new(Role::Reasoning, chunk));
                }
            }
        }
        Control::DiscardReply => {
            if ui
                .entries
                .last()
                .is_some_and(|entry| entry.role == Role::Assistant)
            {
                ui.entries.pop();
            }
        }
        Control::CommitReply => {
            if let Some(last) = ui
                .entries
                .last_mut()
                .filter(|entry| entry.role == Role::Assistant || entry.role == Role::Reasoning)
            {
                last.finish_text();
            }
        }
        Control::Tool {
            call_id,
            name,
            arguments,
        } => {
            let nested = ui.entries.iter().rev().any(|entry| {
                entry.role == Role::Tool
                    && entry
                        .tool
                        .as_ref()
                        .is_some_and(|tool| tool.call_id == call_id && !tool.done)
            });
            if nested {
                // A child tool of an in-flight delegate: keep the parent's
                // box and record the child as a live line inside it.
                if let Some(entry) = tool_entry(ui, &call_id) {
                    entry.push_subagent_line(tool_title(&name, &arguments));
                }
            } else {
                let parsed = serde_json::from_str(&arguments).unwrap_or_else(
                    |_| serde_json::json!({ "unparsed_arguments": arguments.clone() }),
                );
                let mut entry = Entry::tool_call(tool_title(&name, &arguments));
                entry.tool = Some(ToolCall {
                    call_id,
                    function_name: name,
                    arguments: parsed,
                    output: None,
                    error: None,
                    done: false,
                    duration_ms: None,
                });
                ui.entries.push(entry);
            }
        }
        Control::SubagentOutput { call_id, line } => {
            if let Some(entry) = tool_entry(ui, &call_id) {
                entry.push_subagent_line(line);
            }
        }
        Control::ToolOutput { call_id, chunk } => {
            if let Some(entry) = tool_entry(ui, &call_id) {
                entry
                    .output
                    .get_or_insert_with(String::new)
                    .push_str(&chunk);
                let seen = entry.output.clone();
                if let Some(tool) = entry.tool.as_mut() {
                    tool.output = seen;
                }
            }
        }
        Control::ToolDone {
            call_id,
            is_error,
            duration_ms,
        } => {
            let settled_at = ui.tick;
            if let Some(entry) = tool_entry(ui, &call_id) {
                entry.settle_tool(settled_at);
                if let Some(tool) = entry.tool.as_mut() {
                    tool.done = true;
                    tool.duration_ms = Some(duration_ms);
                }
                if is_error {
                    // Said on the header, where the reader is looking. A
                    // failed call that reads like a successful one is how a
                    // session comes to believe a build passed.
                    if !entry.text.ends_with(" — failed") {
                        entry.text.push_str(" — failed");
                    }
                    let seen = entry.output.clone();
                    if let Some(tool) = entry.tool.as_mut() {
                        tool.error = seen;
                    }
                }
            }
        }
        // What answered, from the grant. Recorded on the frame's own field,
        // which stays empty until something has actually answered.
        Control::Model(model) => {
            ui.model = model.clone();
            if let Some(entry) = ui
                .entries
                .iter_mut()
                .rfind(|entry| entry.role == Role::Assistant)
            {
                entry.model = Some(model);
            }
        }
        // The thread the server opened, so `/info` can name it without asking
        // the session for it behind a lock the turn is holding.
        Control::Thread(thread) => ui.thread = Some(thread),
        // The model picker's items arrived (issues #323/#324). An `Err` is
        // the per-lane source's refusal — no Ollama server, a catalog the
        // credential cannot read — shown as a notice, with the picker
        // closed: an empty picker that stays open is a lie with a border.
        Control::ModelPicker(outcome) => match outcome {
            Ok(picker) if !picker.items.is_empty() => ui.model_picker = Some(picker),
            Ok(picker) => {
                ui.model_picker = None;
                ui.entries.push(Entry::new(
                    Role::Notice,
                    if picker.local {
                        crate::runtime::OLLAMA_INSTALL_SIGN.to_string()
                    } else {
                        "No models to pick from: the deployment serves none this lane \
                         can use."
                            .to_string()
                    },
                ));
            }
            Err(why) => {
                ui.model_picker = None;
                ui.entries.push(Entry::new(Role::Notice, why));
            }
        },
        // What the server billed. Only ever a figure it sent.
        Control::Billed(billed) => ui.billed = Some(billed),
        Control::Usage(usage) => {
            if usage.reported() {
                ui.add_usage(usage);
            }
        }
        // Replaced rather than accumulated: it is the account's balance as of
        // that read, not a delta this session can add up. A read that found
        // nothing replaces the figure too — see `crate::coder::credit`.
        Control::Credit(outcome) => ui.credit.record(outcome),
        Control::Notice(text) => {
            if !text.trim().is_empty() {
                ui.entries.push(Entry::new(Role::Notice, text));
            }
        }
        Control::Waiting(message) => {
            ui.waiting = message;
        }
        Control::LoadStatus { message, fail } => {
            ui.load_line = Some(message.clone());
            ui.loading = false;
            ui.waiting = None;
            if fail {
                ui.memory_line = None;
                ui.entries.push(Entry::new(Role::Notice, message));
            }
        }
        Control::MemoryLine(line) => {
            ui.memory_line = line;
        }
        Control::Output(text) => {
            if !text.trim().is_empty() {
                ui.entries.push(Entry::new(Role::Output, text));
            }
        }
        Control::Failed(why) => {
            // The streaming entry settles as whatever did arrive before the
            // failure, and the failure goes next to it rather than into it.
            if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                last.finish_text();
            }
            ui.entries.push(Entry::new(Role::Notice, why));
            ui.entries.push(Entry::new(
                Role::Notice,
                commands::CONTINUE_HINT.to_string(),
            ));
            ui.loading = false;
            ui.turn_settled();
            ui.waiting = None;
        }
        Control::Done => {
            if let Some(last) = ui
                .entries
                .last_mut()
                .filter(|e| e.role == Role::Assistant || e.role == Role::Reasoning)
            {
                last.finish_text();
            }
            ui.loading = false;
            ui.turn_settled();
            ui.waiting = None;
        }
        Control::Goal(goal) => ui.goal = goal,
    }
}

/// Show the current lane once, replacing the prior lane selection when it is
/// still present in the transcript.
fn record_lane_notice(ui: &mut CoderUi, lane_notice: &mut Option<usize>, lane: &Lane) {
    let text = format!("Lane: {}", lane.label());
    let updated = lane_notice
        .and_then(|index| ui.entries.get_mut(index))
        .filter(|entry| entry.role == Role::Notice && entry.text.starts_with("Lane: "))
        .is_some_and(|entry| {
            entry.text = text.clone();
            true
        });

    if !updated {
        ui.entries.push(Entry::new(Role::Notice, text));
        *lane_notice = Some(ui.entries.len() - 1);
    }

    ui.scroll_override = None;
}

fn tool_entry<'a>(ui: &'a mut CoderUi, call_id: &str) -> Option<&'a mut Entry> {
    ui.entries.iter_mut().rev().find(|entry| {
        entry.role == Role::Tool
            && entry
                .tool
                .as_ref()
                .is_some_and(|tool| tool.call_id == call_id)
    })
}

/// A key the composer did not want: history, scrolling, and Tab.
fn handle_session_key(
    ui: &mut CoderUi,
    history: &mut History,
    key: &KeyEvent,
    width: usize,
    cwd: &std::path::Path,
) {
    match key.code {
        // Off the top of the composer walks the input history, and only once
        // the history has nothing more does the transcript scroll. Reversing
        // that order makes the history unreachable on a one-line composer,
        // which is the only shape it is ever walked from.
        KeyCode::Up => match history.previous(ui.composer.text()) {
            Some(prompt) => ui.composer.set_text(&prompt),
            None => ui.scroll_by(-1),
        },
        KeyCode::Down => match history.forward() {
            Some(prompt) => ui.composer.set_text(&prompt),
            None => ui.scroll_by(1),
        },
        KeyCode::PageUp => {
            let page = ui.transcript_height.max(1) as i32;
            ui.scroll_by(-page);
        }
        KeyCode::PageDown => {
            let page = ui.transcript_height.max(1) as i32;
            ui.scroll_by(page);
        }
        KeyCode::Tab => {
            let Completion { insert, candidates } = complete(
                ui.composer.text(),
                ui.composer.cursor_byte(),
                &commands::names(),
                cwd,
            );
            if !insert.is_empty() {
                ui.composer.insert_str(&insert);
            }
            if candidates.len() > 1 {
                // Listed rather than chosen. Tab never inserts a candidate
                // that was not the only one.
                ui.entries
                    .push(Entry::new(Role::Output, candidates.join("  ")));
                ui.scroll_override = None;
            }
            let _ = width;
        }
        _ => {}
    }
}

/// Send what was typed: a command to the session, or a prompt to the model.
///
/// Returns what the caller still has to do, which for everything but
/// `/logout` is nothing.
/// The boundary heartbeat: one swarm status to `all`, naming what the loop
/// closed and what it picked (spec §5). Returns whether the send was
/// delivered anywhere — the result feeds
/// [`crate::coder::autopilot::StopConditions::record_heartbeat`], because a
/// visibility mechanism that fails silently is the #306 shape.
fn send_heartbeat(
    ui: &mut CoderUi,
    heartbeat: crate::coder::autopilot::Heartbeat,
    stops: &mut crate::coder::autopilot::StopConditions,
    last_heartbeat: &mut Option<String>,
) {
    let body = heartbeat.body();
    let delivered = match commands::own_inbox_directory() {
        Some((directory, from_id)) => crate::swarm::send(
            &crate::swarm::default_home(),
            &from_id,
            &directory,
            "all",
            "status",
            None,
            false,
            &body,
            None,
            None,
        )
        .map(|report| !report.deliveries.is_empty())
        .unwrap_or(false),
        None => false,
    };
    stops.record_heartbeat(delivered);
    if delivered {
        *last_heartbeat = Some(body.clone());
    } else if last_heartbeat.is_none() {
        // A session that never registered with the swarm records the intent
        // rather than failing loudly every boundary: the status screen still
        // shows the failure counter climbing, which is the honest state.
        *last_heartbeat = Some(format!("{body} (not delivered)"));
    }
    let _ = ui; // heartbeat failures render via the status screen, not a toast
}

/// The observer's status screen (spec §8): `/autopilot status` output,
/// assembled from the loop's boundary record.
fn autopilot_status_screen(
    autopilot: &crate::coder::autopilot::AutopilotState,
    closed: &[String],
    last_picked: Option<&str>,
    last_heartbeat: Option<&str>,
) -> String {
    let mut report = autopilot.status_report(last_heartbeat.map(str::to_string));
    report.closed = closed.to_vec();
    if let Some(picked) = last_picked {
        report.closed.push(format!("picked: {picked} (in flight)"));
    }
    report.render()
}

/// The autopilot boundary record the loop reads and updates: the mode state
/// plus what it has closed, picked, and last reported. One parameter instead
/// of four — the frame owns this bundle, `submit` and the boundary mutate it.
struct AutopilotFrame {
    state: crate::coder::autopilot::AutopilotState,
    closed: Vec<String>,
    last_picked: Option<String>,
    last_heartbeat: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn submit(
    ui: &mut CoderUi,
    text: String,
    lane: &mut Lane,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    cwd: &std::path::Path,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
    prompt_queue: &mut VecDeque<QueuedPrompt>,
    autopilot: &mut AutopilotFrame,
    dev: bool,
    cached_local: Option<&str>,
) -> commands::Outcome {
    ui.scroll_override = None;
    ui.show_welcome = false;
    ui.entries.push(Entry::new(Role::You, text.clone()));
    let images = ui.take_referenced_images(&text);

    if crate::coder::goal::is_goal_command(&text) {
        let notice = match session.try_lock() {
            Ok(mut session) => {
                let notice = session.goal_command(&text);
                ui.goal = session.goal();
                notice
            }
            Err(_) => "The running turn must finish before `/goal` can change.".to_string(),
        };
        ui.entries.push(Entry::new(Role::Output, notice));
        return commands::Outcome::Done;
    }

    // `/autopilot` is handled here, beside `/goal`, because the state lives
    // in the frame loop, not in the command dispatch. Engaging with a
    // directive starts the first iteration immediately — the mode that makes
    // the reader type a second prompt to begin is a mode that never starts.
    if let Some(command) = crate::coder::autopilot::parse_command(&text) {
        use crate::coder::autopilot::AutopilotCommand;
        // `/autopilot status` renders the observer screen (spec §8) without
        // changing the mode, whether engaged or not — and it must be
        // recognised before the engage match, or the word "status" would be
        // consumed as a pick filter and the screen would render the mode
        // that engage just armed.
        if text.trim().eq_ignore_ascii_case("/autopilot status") {
            ui.entries.push(Entry::new(
                Role::Output,
                autopilot_status_screen(
                    &autopilot.state,
                    &autopilot.closed,
                    autopilot.last_picked.as_deref(),
                    autopilot.last_heartbeat.as_deref(),
                ),
            ));
            ui.scroll_override = None;
            return commands::Outcome::Done;
        }
        match command {
            AutopilotCommand::Toggle => {
                autopilot.state.engaged = !autopilot.state.engaged;
                if !autopilot.state.engaged {
                    autopilot.state.directive = None;
                }
            }
            AutopilotCommand::Off => {
                autopilot.state.engaged = false;
                autopilot.state.directive = None;
            }
            AutopilotCommand::Engage { directive } => {
                autopilot.state.engaged = true;
                // A `--stop-word <token>` prefix arms the remote off switch
                // at engage time (spec §7); the rest is the pick filter.
                let (token, remaining) = crate::coder::autopilot::split_stop_word(&directive);
                autopilot.state.stop_word = token.map(crate::coder::autopilot::StopWord::new);
                autopilot.state.directive = (!remaining.is_empty()).then(|| remaining.to_string());
                // Engaging re-arms the condition set: the budget the reader
                // asked for starts now, not from some previous engage.
                autopilot.state.stops = crate::coder::autopilot::StopConditions::default();
            }
        }
        ui.autopilot_engaged = autopilot.state.engaged;
        ui.entries.push(Entry::new(
            Role::Notice,
            if autopilot.state.engaged {
                "Autopilot engaged: the loop keeps steering between turns. Meta+A or \
                 /autopilot off hands the wheel back."
                    .to_string()
            } else {
                "Autopilot disengaged: the session waits for you after each turn.".to_string()
            },
        ));
        if autopilot.state.engaged && matches!(turns.phase(), TurnPhase::Idle) {
            refresh_workspace_snapshot(session, cwd).await;
            let prompt = autopilot.state.iteration_prompt();
            start_prompt(ui, prompt, Vec::new(), session, tx, turns, active_turn, dev).await;
            update_activity(ui, turns, prompt_queue.len());
        }
        return commands::Outcome::Done;
    }

    // `/model` opens the model picker (issues #323/#324); `/model <id>`
    // commits directly, refusing unknown ids by name — grok's resolve rule,
    // and the honesty rule `unresolved_lane` set for the CLI: an id the
    // deployment does not serve is refused, never quietly substituted.
    if let Some(argument) = text
        .trim()
        .strip_prefix("/model")
        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        let argument = argument.trim();
        if argument.is_empty() {
            let resolved = (!ui.model.is_empty()).then(|| ui.model.clone());
            open_model_picker(lane, resolved.as_deref(), cached_local, tx);
            ui.model_picker = Some(seed_model_picker(lane, cached_local));
            return commands::Outcome::Done;
        }
        match crate::coder::model_picker::commit_lane(lane, argument) {
            Ok(committed) => {
                if let Ok(mut session) = session.try_lock() {
                    session.set_lane(committed.clone());
                    *lane = committed.clone();
                    ui.lane = committed.label();
                    ui.model.clear();
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        model_commit_notice(&committed, argument),
                    ));
                } else {
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        "The running turn must finish before the model can change.".to_string(),
                    ));
                }
            }
            Err(error) => ui.entries.push(Entry::new(Role::Notice, error.to_string())),
        }
        return commands::Outcome::Done;
    }

    if crate::composer::is_local_slash_input(&text, commands::COMMANDS) {
        return commands::run(ui, text.trim(), tx, cwd);
    }

    // #341: a line that looks like an ATIF export path still goes to the
    // model — the user may want exactly that (read the dump, summarize it,
    // cross-examine an old session). The advice rides along as a notice; it
    // never replaces the send. Annotate, don't intercept.
    if let Some(advice) = commands::atif_export_advice(&text) {
        ui.entries
            .push(Entry::new(Role::Notice, advice.to_string()));
    }

    if !matches!(turns.phase(), TurnPhase::Idle) {
        prompt_queue.push_back(QueuedPrompt { text, images });
        update_activity(ui, turns, prompt_queue.len());
        return commands::Outcome::Done;
    }

    start_prompt(ui, text, images, session, tx, turns, active_turn, dev).await;
    update_activity(ui, turns, prompt_queue.len());
    commands::Outcome::Done
}

async fn refresh_workspace_snapshot(session: &Arc<Mutex<Session>>, cwd: &std::path::Path) {
    let checkpoint = session
        .lock()
        .await
        .local_session_summary()
        .and_then(|summary| summary.last_checkpoint.clone());
    let text = crate::coder::snapshot::workspace_snapshot(cwd, checkpoint.as_deref()).await;
    session.lock().await.seed_workspace_snapshot(&text);
}

#[allow(clippy::too_many_arguments)]
async fn start_prompt(
    ui: &mut CoderUi,
    text: String,
    images: Vec<ImageAttachment>,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
    dev: bool,
) {
    if !dev {
        let live = session.lock().await;
        if !live.lane().is_local() && !live.lane().uses_nitro_origin() && !live.has_user_token() {
            drop(live);
            ui.entries
                .push(Entry::new(Role::Notice, HOSTED_NEEDS_SIGN_IN));
            return;
        }
    }

    ui.loading = true;
    ui.turn_started();
    ui.waiting = None;

    let TurnEffect::Started(id) = turns.apply(TurnAction::Start) else {
        ui.loading = false;
        ui.turn_started_at = None;
        return;
    };

    let router = session.lock().await.turn_router();

    let session = Arc::clone(session);
    let tx = tx.clone();
    let task = tokio::spawn(async move {
        session
            .lock()
            .await
            .execute_turn_with_id_and_images(id, &text, &images, tx)
            .await;
    });
    *active_turn = Some(ActiveTurn { id, task, router });
}

async fn start_next_prompt(
    ui: &mut CoderUi,
    prompt_queue: &mut VecDeque<QueuedPrompt>,
    session: Option<&Arc<Mutex<Session>>>,
    tx: &Sender<Control>,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
    dev: bool,
) {
    if let (Some(prompt), Some(session)) = (prompt_queue.pop_front(), session) {
        start_prompt(
            ui,
            prompt.text,
            prompt.images,
            session,
            tx,
            turns,
            active_turn,
            dev,
        )
        .await;
    }
    update_activity(ui, turns, prompt_queue.len());
}

fn queue_status(count: usize) -> String {
    match count {
        0 => "No prompts are queued.".to_string(),
        1 => "1 prompt is queued.".to_string(),
        count => format!("{count} prompts are queued."),
    }
}

fn update_activity(ui: &mut CoderUi, turns: &TurnState, queued: usize) {
    let phase = match turns.phase() {
        TurnPhase::Idle => "Idle",
        TurnPhase::Active(_) => "Active",
        TurnPhase::Canceling(_) => "Canceling",
    };
    ui.activity = if queued == 0 {
        phase.to_string()
    } else {
        format!(
            "{phase} · {queued} queued prompt{}",
            if queued == 1 { "" } else { "s" }
        )
    };
}

fn request_cancel(
    ui: &mut CoderUi,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
    session: Option<&Arc<Mutex<Session>>>,
    tx: &Sender<Control>,
    queued: usize,
) {
    let TurnEffect::AbortTransport(id) = turns.apply(TurnAction::RequestCancel) else {
        return;
    };

    ui.loading = true;
    ui.waiting = Some("Canceling turn...".to_string());
    update_activity(ui, turns, queued);

    let Some(active) = active_turn.take().filter(|active| active.id == id) else {
        let _ = tx.send(Control::CancelComplete {
            id,
            diagnostic: Some("The turn task was already gone.".to_string()),
        });
        return;
    };
    let active_tools = active
        .router
        .lock()
        .map(|mut router| router.cancel(id))
        .unwrap_or_default();

    let Some(session) = session.cloned() else {
        let _ = tx.send(Control::CancelComplete {
            id,
            diagnostic: None,
        });
        return;
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let mut task = active.task;
        let timed_out = active_tools != 0
            && tokio::time::timeout(
                crate::signals::KILL_GRACE + Duration::from_secs(1),
                &mut task,
            )
            .await
            .is_err();
        if active_tools == 0 || timed_out {
            task.abort();
            let _ = task.await;
        }
        let settled = tokio::time::timeout(Duration::from_secs(10), async {
            session.lock().await.settle_cancellation(id).await
        })
        .await;
        let diagnostic = match settled {
            Ok(Ok(_)) => None,
            Ok(Err(error)) => Some(format!("The canceled turn could not be settled: {error}")),
            Err(_) => Some("The canceled turn could not be settled within 10 seconds.".to_string()),
        };
        let _ = tx.send(Control::CancelComplete { id, diagnostic });
    });
}

/// `/export` is here rather than in `commands` because it reads the whole
/// transcript, which the command dispatch does not otherwise see.
pub fn export(ui: &mut CoderUi) {
    let model = if ui.model.is_empty() {
        // What actually answered, or nothing. A trajectory stamped with the
        // lane's preferred model where no model answered is a trajectory that
        // names a model that never ran.
        "unknown"
    } else {
        ui.model.as_str()
    }
    .to_string();
    let result = export_trajectory(&ui.entries, &model, &ui.repo, &ui.branch);
    ui.entries.push(Entry::new(
        Role::Notice,
        format!(
            "exported {} steps to {} (copied: {})",
            result.steps, result.path, result.copied
        ),
    ));
}

/// The commands the composer completes, so a name that is not handled cannot
/// be offered. Re-exported here because that is where the completer looks.
pub use commands::names as command_names;

/// How `run_tui` starts given a `--prompt` and whether a terminal is attached
/// (#338). A TTY always takes the full-screen path; `--prompt` without a TTY
/// is the one-shot path; no prompt and no TTY is the existing refusal.
#[derive(Debug, PartialEq, Eq)]
enum SessionBoot<'a> {
    RefuseNoTty,
    OneShot(&'a str),
    FullScreen,
}

/// True when `--prompt` has seen the turn's last event and must stop
/// draining. The session still holds a `Sender`, so waiting for disconnect
/// after `Done` never returns.
fn one_shot_control_ends_the_turn(control: &Control) -> bool {
    match control {
        Control::Turn { event, .. } => matches!(**event, Control::Done),
        Control::Done => true,
        _ => false,
    }
}

fn session_boot(prompt: Option<&str>, tty: bool) -> SessionBoot<'_> {
    match (prompt, tty) {
        (None, false) => SessionBoot::RefuseNoTty,
        (Some(prompt), false) => SessionBoot::OneShot(prompt),
        (_, true) => SessionBoot::FullScreen,
    }
}

fn atty_is_terminal() -> bool {
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        return false;
    }

    // The installer runs from a pipe. stdin is therefore exhausted, but the
    // controlling terminal remains available. Crossterm's `use-dev-tty`
    // feature opens it for input instead of trying to register the exhausted
    // pipe with the macOS event reader.
    #[cfg(unix)]
    {
        std::fs::File::open("/dev/tty").is_ok()
    }

    #[cfg(not(unix))]
    {
        std::io::IsTerminal::is_terminal(&std::io::stdin())
    }
}

/// ANSI mouse-tracking sequences Unix `EnableMouseCapture` already writes.
///
/// crossterm's Windows path only calls `SetConsoleMode` and reports that ANSI
/// is unsupported, so a ConPTY host never sees a mouse-enable request. Wheel
/// and trackpad then become Up/Down (alternate scroll) and walk input history
/// (#349). Write these on Windows after the WinAPI enable.
#[allow(dead_code)]
const MOUSE_CAPTURE_ENABLE_ANSI: &str = "\x1B[?1000h\x1B[?1002h\x1B[?1003h\x1B[?1015h\x1B[?1006h";
#[allow(dead_code)]
const MOUSE_CAPTURE_DISABLE_ANSI: &str = "\x1B[?1006l\x1B[?1015l\x1B[?1003l\x1B[?1002l\x1B[?1000l";

#[allow(dead_code)]
fn write_host_mouse_tracking(out: &mut impl Write, enable: bool) -> std::io::Result<()> {
    let seq = if enable {
        MOUSE_CAPTURE_ENABLE_ANSI
    } else {
        MOUSE_CAPTURE_DISABLE_ANSI
    };
    out.write_all(seq.as_bytes())?;
    out.flush()
}

fn enable_session_mouse(out: &mut impl Write) -> Result<(), String> {
    let winapi = out.execute(EnableMouseCapture).map(|_| ());
    let ansi = write_windows_host_mouse_tracking(out, true);
    match (winapi, ansi) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), ansi) => {
            let extra = match ansi {
                Err(ansi_error) => {
                    format!(" Host mouse sequences also failed ({ansi_error}).")
                }
                Ok(()) => String::new(),
            };
            Err(format!(
                "Mouse tracking did not start ({error}).{extra} Trackpad scroll may walk input history instead of the transcript. PageUp and PageDown still scroll."
            ))
        }
        (Ok(()), Err(error)) => Err(format!(
            "Mouse tracking did not start ({error}). Trackpad scroll may walk input history instead of the transcript. PageUp and PageDown still scroll."
        )),
    }
}

fn write_windows_host_mouse_tracking(out: &mut impl Write, enable: bool) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        write_host_mouse_tracking(out, enable)
    }
    #[cfg(not(windows))]
    {
        let _ = (out, enable);
        Ok(())
    }
}

fn disable_session_mouse(out: &mut impl Write) {
    let _ = out.execute(DisableMouseCapture);
    let _ = write_windows_host_mouse_tracking(out, false);
}

fn apply_mouse(ui: &mut CoderUi, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            ui.selection.clear();
            ui.scroll_by(-3);
        }
        MouseEventKind::ScrollDown => {
            ui.selection.clear();
            ui.scroll_by(3);
        }
        _ => {
            // The selection owns press/drag/release inside the
            // transcript area; elsewhere mouse events are inert.
            let in_transcript = ui
                .selection
                .rows()
                .iter()
                .any(|row| row.screen_y == mouse.row)
                || matches!(mouse.kind, MouseEventKind::Drag(_) | MouseEventKind::Up(_));
            if in_transcript {
                ui.selection
                    .handle_mouse(mouse.column, mouse.row, mouse.kind);
            }
        }
    }
}

/// Restore terminal state even when an input or draw operation returns early.
struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            std::io::stderr(),
            DisableBracketedPaste,
            PopKeyboardEnhancementFlags
        );
        let _ = std::io::stdout().execute(SetCursorStyle::DefaultUserShape);
        let _ = std::io::stdout().write_all(CURSOR_COLOR_RESET.as_bytes());
        disable_session_mouse(&mut std::io::stdout());
        let _ = disable_raw_mode();
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
    }
}

/// Every command the session lists is one it handles.
///
/// The property that keeps `/help` honest. It is asserted here rather than
/// left to a reader noticing, because the list and the dispatch are in two
/// places and only a test holds them together.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_stops_on_done_not_on_disconnect() {
        let done = Control::Turn {
            id: TurnId::new(1),
            event: Box::new(Control::Done),
        };
        assert!(one_shot_control_ends_the_turn(&done));
        assert!(one_shot_control_ends_the_turn(&Control::Done));
        assert!(!one_shot_control_ends_the_turn(&Control::Chunk(
            "pong".into()
        )));
        assert!(!one_shot_control_ends_the_turn(&Control::Failed(
            "catalog".into()
        )));
    }

    #[test]
    fn a_prompt_without_a_tty_is_the_one_shot_path() {
        assert_eq!(session_boot(None, false), SessionBoot::RefuseNoTty);
        assert_eq!(
            session_boot(Some("hello"), false),
            SessionBoot::OneShot("hello")
        );
        assert_eq!(session_boot(Some("hello"), true), SessionBoot::FullScreen);
        assert_eq!(session_boot(None, true), SessionBoot::FullScreen);
    }

    #[test]
    fn every_listed_command_is_handled() {
        for (name, _) in crate::coder::commands::COMMANDS {
            // `/autopilot` and `/model` are handled, just not here: their
            // state lives in the frame loop, and `submit` claims both before
            // this dispatch runs (`coder/autopilot::parse_command` and the
            // `/model` arm; `coder/model_picker.rs` holds the picker's
            // surface). The named exceptions, not holes — the parse tests in
            // those modules hold their surfaces.
            if *name == "autopilot" {
                assert!(
                    crate::coder::autopilot::parse_command("/autopilot").is_some(),
                    "`/autopilot` left the dispatch and lost its handler"
                );
                continue;
            }
            if *name == "model" {
                assert!(
                    crate::coder::model_picker::commit_lane(
                        &crate::runtime::Lane::Pro,
                        "gpt-5.6-sol"
                    )
                    .is_ok(),
                    "`/model` left the dispatch and lost its commit path"
                );
                continue;
            }
            assert!(
                commands::handles(name),
                "`/{name}` is listed by /help and nothing runs it"
            );
        }
    }

    #[test]
    fn the_completer_offers_exactly_the_handled_commands() {
        let mut offered = command_names();
        offered.sort_unstable();
        let mut listed: Vec<&str> = crate::coder::commands::COMMANDS
            .iter()
            .map(|(name, _)| *name)
            .collect();
        listed.sort_unstable();
        assert_eq!(offered, listed);
    }

    #[test]
    fn paste_line_endings_become_composer_line_endings() {
        assert_eq!(normalize_paste("one\r\ntwo\rthree"), "one\ntwo\nthree");
    }

    fn wheel(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn scrollable_ui() -> CoderUi {
        let mut ui = CoderUi::new();
        ui.scroll_max = 20;
        ui.transcript_height = 10;
        ui
    }

    #[test]
    fn host_mouse_tracking_sequences_match_crossterm_unix() {
        let mut enable = Vec::new();
        write_host_mouse_tracking(&mut enable, true).unwrap();
        assert_eq!(enable, MOUSE_CAPTURE_ENABLE_ANSI.as_bytes());
        let mut disable = Vec::new();
        write_host_mouse_tracking(&mut disable, false).unwrap();
        assert_eq!(disable, MOUSE_CAPTURE_DISABLE_ANSI.as_bytes());
    }

    #[test]
    fn wheel_scrolls_the_transcript_even_when_history_has_entries() {
        let mut ui = scrollable_ui();
        ui.composer.set_text("draft");
        let mut history = History::new();
        history.record("first");
        history.record("second");

        apply_mouse(&mut ui, wheel(MouseEventKind::ScrollUp));
        assert_eq!(ui.scroll_override, Some(17));
        assert_eq!(ui.composer.text(), "draft");

        handle_session_key(
            &mut ui,
            &mut history,
            &up_key(),
            40,
            std::path::Path::new("."),
        );
        assert_eq!(
            ui.composer.text(),
            "second",
            "the first Up after a wheel event is still the newest history entry"
        );
        assert_eq!(ui.scroll_override, Some(17));
    }

    #[test]
    fn up_walks_history_before_the_transcript() {
        let mut ui = scrollable_ui();
        ui.composer.set_text("draft");
        let mut history = History::new();
        history.record("remembered");

        handle_session_key(
            &mut ui,
            &mut history,
            &up_key(),
            40,
            std::path::Path::new("."),
        );
        assert_eq!(ui.composer.text(), "remembered");
        assert_eq!(ui.scroll_override, None);
    }

    /// #351: grok-build leaves the viewport alone while `follow_mode` is
    /// off. Coder's pin is `scroll_override`; streaming used to clear it.
    #[test]
    fn streaming_does_not_steal_a_scrolled_up_pin() {
        let mut ui = scrollable_ui();
        apply_mouse(&mut ui, wheel(MouseEventKind::ScrollUp));
        assert_eq!(ui.scroll_override, Some(17));

        apply(&mut ui, Control::Chunk("token".into()));
        apply(&mut ui, Control::Reasoning("think".into()));
        apply(
            &mut ui,
            Control::Tool {
                call_id: "c1".into(),
                name: "bash".into(),
                arguments: "{}".into(),
            },
        );
        apply(
            &mut ui,
            Control::ToolOutput {
                call_id: "c1".into(),
                chunk: "out".into(),
            },
        );
        apply(
            &mut ui,
            Control::ToolDone {
                call_id: "c1".into(),
                is_error: false,
                duration_ms: 1,
            },
        );
        apply(&mut ui, Control::Waiting(Some("working".into())));
        apply(&mut ui, Control::Notice("note".into()));
        apply(&mut ui, Control::Failed("nope".into()));
        assert_eq!(
            ui.scroll_override,
            Some(17),
            "live deltas must not clear a pin"
        );
    }

    #[test]
    fn following_the_bottom_still_follows_after_a_chunk() {
        let mut ui = scrollable_ui();
        assert_eq!(ui.scroll_override, None);
        apply(&mut ui, Control::Chunk("token".into()));
        assert_eq!(ui.scroll_override, None);
    }

    #[test]
    fn scrolling_to_the_bottom_resumes_follow() {
        let mut ui = scrollable_ui();
        apply_mouse(&mut ui, wheel(MouseEventKind::ScrollUp));
        assert_eq!(ui.scroll_override, Some(17));
        ui.scroll_by(3);
        assert_eq!(ui.scroll_override, None);
    }

    fn up_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
    }

    #[test]
    fn local_events_restore_visible_roles_and_the_answer_model() {
        let events = vec![
            crate::session_store::StoredEvent {
                sequence: 1,
                at_ms: 10,
                record: crate::runtime::ThreadRecord::user("question"),
            },
            crate::session_store::StoredEvent {
                sequence: 2,
                at_ms: 20,
                record: crate::runtime::ThreadRecord::reasoning("working"),
            },
            crate::session_store::StoredEvent {
                sequence: 3,
                at_ms: 30,
                record: crate::runtime::ThreadRecord::assistant_on(
                    "answer",
                    crate::runtime::TurnUsage::default(),
                    0,
                    Some("model/one"),
                ),
            },
            crate::session_store::StoredEvent {
                sequence: 4,
                at_ms: 40,
                record: crate::runtime::ThreadRecord::failed(
                    "later failure",
                    crate::runtime::TurnUsage::default(),
                    0,
                ),
            },
        ];
        let mut ui = CoderUi::new();
        restore_entries(&mut ui, &events);

        assert_eq!(
            ui.entries
                .iter()
                .map(|entry| entry.role.clone())
                .collect::<Vec<_>>(),
            vec![Role::You, Role::Reasoning, Role::Assistant, Role::Notice]
        );
        assert_eq!(ui.entries[2].model.as_deref(), Some("model/one"));
        // The restored failure carries the counters it died with (#188).
        assert_eq!(ui.entries[3].text, "later failure (0 tool calls, 0 tokens)");
        assert_eq!(ui.entries[3].at, 40);
    }

    #[test]
    fn atif_snapshot_comes_from_the_journal_after_the_view_is_cleared() {
        let root = tempfile::tempdir().unwrap();
        let mut loaded = crate::session_store::LocalSessionStore::create(
            root.path(),
            std::path::Path::new("/repo"),
            "flash",
            None,
            false,
        )
        .unwrap();
        loaded
            .store
            .append(&[
                crate::runtime::ThreadRecord::user("question"),
                crate::runtime::ThreadRecord::assistant_on(
                    "answer",
                    crate::runtime::TurnUsage::default(),
                    0,
                    Some("model/one"),
                ),
            ])
            .unwrap();
        loaded.store.set_last_model(Some("model/one")).unwrap();
        let directory = loaded.store.directory().to_path_buf();

        let mut cleared = CoderUi::new();
        cleared.local_session_id = Some(loaded.summary.id);
        cleared.repo = "/repo".to_string();
        cleared.branch = "main".to_string();
        assert!(cleared.entries.is_empty());
        assert_eq!(write_atif_snapshot(&cleared, &directory).unwrap(), 2);

        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("trajectory.atif.json")).unwrap())
                .unwrap();
        assert_eq!(document["steps"][0]["message"], "question");
        assert_eq!(document["steps"][1]["message"], "answer");
        assert_eq!(document["steps"][1]["model_name"], "model/one");
    }

    fn device_authorization() -> crate::auth::DeviceAuthorization {
        crate::auth::DeviceAuthorization {
            device_code: "device-secret".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://example.test/device".to_string(),
            verification_uri_complete: "https://example.test/device?user_code=ABCD-EFGH"
                .to_string(),
            expires_in: 600,
            interval: 5,
            scope: Some("chat:account".to_string()),
        }
    }

    #[test]
    fn failed_browser_launch_leaves_a_manual_sign_in_path() {
        let authorization = device_authorization();
        let mut launched = String::new();
        let message = open_login_page(&authorization, |url| {
            launched = url.to_string();
            false
        });

        assert_eq!(launched, authorization.verification_uri_complete);
        assert!(message.contains("could not open a browser"), "{message}");
        assert!(message.contains(&authorization.verification_uri_complete));
        assert!(message.contains(&authorization.user_code));
        assert!(
            !message.contains("Opened the OpenAgents sign-in page"),
            "{message}"
        );
    }

    #[test]
    fn successful_browser_launch_still_shows_the_url_and_code() {
        let authorization = device_authorization();
        let message = open_login_page(&authorization, |_| true);

        assert!(
            message.contains("Opened the OpenAgents sign-in page"),
            "{message}"
        );
        assert!(message.contains(&authorization.verification_uri_complete));
        assert!(message.contains(&authorization.user_code));
    }

    #[test]
    fn the_status_names_every_turn_and_queue_state() {
        let mut ui = CoderUi::new();
        let mut turns = TurnState::default();
        update_activity(&mut ui, &turns, 0);
        assert_eq!(ui.activity, "Idle");

        let TurnEffect::Started(id) = turns.apply(TurnAction::Start) else {
            panic!("turn did not start");
        };
        update_activity(&mut ui, &turns, 2);
        assert_eq!(ui.activity, "Active · 2 queued prompts");

        turns.apply(TurnAction::RequestCancel);
        update_activity(&mut ui, &turns, 2);
        assert_eq!(ui.activity, "Canceling · 2 queued prompts");

        turns.apply(TurnAction::CompleteCancel(id));
        update_activity(&mut ui, &turns, 0);
        assert_eq!(ui.activity, "Idle");
    }

    #[test]
    fn lane_changes_replace_one_notice_even_after_other_output() {
        let mut ui = CoderUi::new();
        let mut lane_notice = None;

        record_lane_notice(&mut ui, &mut lane_notice, &Lane::Flash);
        ui.entries.push(Entry::new(Role::Output, "other output"));
        record_lane_notice(&mut ui, &mut lane_notice, &Lane::Free);

        assert_eq!(ui.entries.len(), 2);
        assert_eq!(ui.entries[0].text, "Lane: Coder Free");
        assert_eq!(ui.entries[1].text, "other output");
    }

    #[test]
    fn a_cleared_transcript_starts_a_fresh_lane_notice() {
        let mut ui = CoderUi::new();
        let mut lane_notice = None;

        record_lane_notice(&mut ui, &mut lane_notice, &Lane::Flash);
        ui.entries.clear();
        record_lane_notice(&mut ui, &mut lane_notice, &Lane::Free);

        assert_eq!(ui.entries.len(), 1);
        assert_eq!(ui.entries[0].text, "Lane: Coder Free");
    }

    #[test]
    fn the_waiting_status_is_replaced_and_cleared_with_the_turn() {
        let mut ui = CoderUi::new();
        ui.loading = true;

        apply(
            &mut ui,
            Control::Waiting(Some("Waiting for the model...".to_string())),
        );
        assert_eq!(ui.waiting.as_deref(), Some("Waiting for the model..."));

        apply(
            &mut ui,
            Control::Waiting(Some(
                "No response after 10 seconds. Retrying (1 of 1)...".to_string(),
            )),
        );
        assert!(ui.waiting.as_deref().unwrap().contains("Retrying"));

        apply(&mut ui, Control::Done);
        assert!(!ui.loading);
        assert_eq!(ui.waiting, None);
    }

    #[test]
    fn held_arrow_keys_remain_editing_events() {
        let repeated_left =
            KeyEvent::new_with_kind(KeyCode::Left, KeyModifiers::NONE, KeyEventKind::Repeat);
        let repeated_tab =
            KeyEvent::new_with_kind(KeyCode::Tab, KeyModifiers::NONE, KeyEventKind::Repeat);
        assert!(is_editing_key_event(&repeated_left));
        assert!(!is_editing_key_event(&repeated_tab));
    }

    #[test]
    fn the_final_answer_is_created_after_its_tool_calls() {
        let mut ui = CoderUi::new();
        ui.entries.push(Entry::new(Role::You, "test the CLI"));

        apply(
            &mut ui,
            Control::Tool {
                call_id: "call_1".to_string(),
                name: "openagents".to_string(),
                arguments: r#"{"args":["--version"]}"#.to_string(),
            },
        );
        apply(
            &mut ui,
            Control::ToolDone {
                call_id: "call_1".to_string(),
                is_error: false,
                duration_ms: 0,
            },
        );
        apply(&mut ui, Control::Chunk("All green.".to_string()));
        apply(&mut ui, Control::Model("glm-5.3-flash".to_string()));
        apply(&mut ui, Control::Done);

        assert_eq!(
            ui.entries
                .iter()
                .map(|entry| entry.role.clone())
                .collect::<Vec<_>>(),
            vec![Role::You, Role::Tool, Role::Assistant]
        );
        assert_eq!(ui.entries[2].text, "All green.");
        assert_eq!(ui.entries[2].model.as_deref(), Some("glm-5.3-flash"));
    }

    #[test]
    fn reasoning_deltas_form_a_visible_entry_before_the_answer() {
        let mut ui = CoderUi::new();

        apply(&mut ui, Control::Reasoning("Check ".to_string()));
        apply(&mut ui, Control::Reasoning("the request.".to_string()));
        apply(&mut ui, Control::Chunk("Done.".to_string()));
        apply(&mut ui, Control::CommitReply);

        assert_eq!(ui.entries.len(), 2);
        assert_eq!(ui.entries[0].role, Role::Reasoning);
        assert_eq!(ui.entries[0].text, "Check the request.");
        assert_eq!(ui.entries[1].role, Role::Assistant);
        assert_eq!(ui.entries[1].text, "Done.");
    }

    #[test]
    fn a_tool_round_removes_its_provisional_reply() {
        let mut ui = CoderUi::new();

        apply(&mut ui, Control::Chunk("I will inspect it.".to_string()));
        apply(&mut ui, Control::DiscardReply);
        apply(
            &mut ui,
            Control::Tool {
                call_id: "call_1".to_string(),
                name: "read".to_string(),
                arguments: r#"{"path":"README.md"}"#.to_string(),
            },
        );
        apply(&mut ui, Control::Chunk("The file is current.".to_string()));
        apply(&mut ui, Control::CommitReply);

        assert_eq!(
            ui.entries
                .iter()
                .map(|entry| entry.role.clone())
                .collect::<Vec<_>>(),
            vec![Role::Tool, Role::Assistant]
        );
        assert_eq!(ui.entries[1].text, "The file is current.");
    }
}
