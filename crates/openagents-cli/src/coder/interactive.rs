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
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::collections::VecDeque;
use std::io::{stderr, stdout, Write};
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
    /// `--dev` routes to the OpenResponses streaming surface.
    pub dev: bool,
    /// `Some("")` resumes the most recent local session for this directory;
    /// any other value names a local session id. `None` starts a new session.
    pub resume: Option<String>,
    /// Upload transcript events and outcome text. Local-only is the default.
    pub cloud_history: bool,
}

pub async fn run_tui(options: SessionOptions) -> Result<(), Box<dyn std::error::Error>> {
    if !atty_is_terminal() {
        println!("Non-interactive terminal detected. Run `openagents` from a terminal.");
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<Control>();
    let mut ui = CoderUi::new();
    let mut history = History::load(History::default_path());

    let (repo, branch) = git_info().unwrap_or(("unknown".to_string(), "unknown".to_string()));
    ui.repo = repo;
    ui.branch = branch;
    // The endpoint does not depend on a credential.
    ui.endpoint = crate::coder::runtime::api_base();

    // Only agents that are actually installed. `find_agents` checks each one
    // before reporting it, so the `acp` tool is declared over a list of
    // programs that exist on this machine rather than a registry's wish list.
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
    loaded.store.set_reasoning(reasoning.as_deref())?;
    loaded.store.set_cloud_history(options.cloud_history)?;
    // Mutable because shift+tab moves it.
    let mut lane = Lane::from_str(&lane_name);
    ui.lane = lane.label();
    let restored_events = loaded.events;
    ui.local_session_id = Some(loaded.summary.id.clone());
    ui.local_session_path = Some(loaded.store.directory().display().to_string());
    ui.cloud_history = options.cloud_history;
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
        if let Some(note) = &loaded.summary.last_checkpoint {
            if !note.trim().is_empty() {
                ui.entries.push(Entry::new(
                    Role::Notice,
                    format!("Last checkpoint:\n{note}"),
                ));
            }
        }
        ui.show_welcome = false;
    }
    let atif_directory = loaded.store.directory().to_path_buf();
    let mut local_store = Some(loaded.store);
    let mut session: Option<Arc<Mutex<Session>>> = None;

    if let Some(token) = crate::coder::runtime::user_token() {
        let endpoint = crate::auth::resolve_endpoint(None, None)?;
        let token = Secret::new(token);
        if validate_token(&endpoint.origin, &token).await.is_ok() {
            // Who, before the screen opens. A token that authenticates is not
            // yet an account name, and the row may only show one the server
            // gave.
            ui.identity = resolve_identity(&endpoint.origin, &token).await;
            let opened = Session::open(
                lane.clone(),
                &lane_name,
                reasoning.clone(),
                agents.clone(),
                options.dev,
                tx.clone(),
            );
            let opened = match local_store.take() {
                Some(store) => {
                    opened.with_local_session(store, &restored_events, options.cloud_history)
                }
                None => opened,
            };
            session = Some(Arc::new(Mutex::new(opened)));
        } else {
            // A stored token the deployment refused. The row says the
            // credential is unverified rather than naming whoever it was
            // issued to: a login drawn over a dead token is a session that
            // looks live while every turn fails.
            ui.identity = crate::coder::tui::Identity::Unverified;
            ui.entries.push(Entry::new(
                Role::Notice,
                "Stored token did not authenticate. Press Enter to sign in to OpenAgents.",
            ));
        }
    } else {
        ui.entries.push(Entry::new(
            Role::Notice,
            "Press Enter to sign in to OpenAgents.",
        ));
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    if let Err(error) = stdout.execute(EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(error.into());
    }
    // This guard owns every terminal mode after the alternate screen opens.
    // A failed event reader must not strand mouse reporting or raw mode in the
    // invoking shell.
    let _terminal_cleanup = TerminalCleanup;
    // Match grok-build: capture ordinary mouse gestures for in-app trackpad
    // scrolling, while Shift-drag remains native terminal selection and copy.
    // Terminal emulators bypass application mouse reporting for that modified
    // gesture, so both interactions remain available at the same time.
    let _ = stdout.execute(EnableMouseCapture);

    let mut stderr = stderr();
    let flags = event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
    // Ask the terminal to wrap clipboard input in paste markers. Without
    // this, a newline in a pasted list arrives as an Enter key and submits
    // the first line before the rest reaches the composer. grok-build uses
    // the same terminal protocol for its text inputs.
    let _ = crossterm::execute!(
        stderr,
        PushKeyboardEnhancementFlags(flags),
        EnableBracketedPaste
    );

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;

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
    let mut active_turn: Option<ActiveTurn> = None;
    let mut prompt_queue = VecDeque::new();
    let mut login_pending = false;
    let mut exit_after_cancel = false;

    'frame: loop {
        while let Ok(control) = rx.try_recv() {
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
                        refresh_credit(&tx);
                        start_next_prompt(
                            &mut ui,
                            &mut prompt_queue,
                            session.as_ref(),
                            &tx,
                            &mut turns,
                            &mut active_turn,
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
                            let opened = Session::open(
                                lane.clone(),
                                &lane_name,
                                reasoning.clone(),
                                agents.clone(),
                                options.dev,
                                tx.clone(),
                            );
                            let opened = match local_store.take() {
                                Some(store) => opened.with_local_session(
                                    store,
                                    &restored_events,
                                    options.cloud_history,
                                ),
                                None => opened,
                            };
                            session = Some(Arc::new(Mutex::new(opened)));
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

        // ratatui has no hyperlink concept, so repaint the link runs as OSC 8
        // sequences over the frame it just flushed. `emit` re-reads the text
        // out of the buffer, so this can never change what a cell says.
        if !ui.links.is_empty() {
            // `draw` swaps buffers before returning. `current_buffer_mut()` is
            // therefore the cleared buffer for the next frame, not the frame
            // the reader can see. Repainting from it erases every link run.
            let buffer = completed_frame.buffer.clone();
            drop(completed_frame);
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

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let key = match event::read()? {
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
                match mouse.kind {
                    MouseEventKind::ScrollUp => ui.scroll_by(-3),
                    MouseEventKind::ScrollDown => ui.scroll_by(3),
                    _ => {}
                }
                continue;
            }
            _ => continue,
        };
        if !is_editing_key_event(&key) {
            continue;
        }

        let width = composer_width(terminal.size()?.width);

        // Escape requests turn cancellation. It never exits Coder, and the
        // reducer makes a repeated request idempotent.
        if key.code == KeyCode::Esc {
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
                            session,
                            &tx,
                            &cwd,
                            &mut turns,
                            &mut active_turn,
                            &mut prompt_queue,
                        )
                        .await;
                        match outcome {
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
                            commands::Outcome::Done => {}
                        }
                    }
                } else if text.trim().is_empty() || text.trim() == "/login" {
                    if login_pending {
                        ui.entries.push(Entry::new(
                            Role::Notice,
                            "OpenAgents sign-in is already in progress.",
                        ));
                    } else {
                        login_pending = true;
                        ui.entries
                            .push(Entry::new(Role::Notice, "Starting OpenAgents sign-in..."));
                        spawn_session_login(&tx);
                    }
                } else {
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        "Press Enter to sign in to OpenAgents.",
                    ));
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
            ComposerAction::Ignored if key.code == KeyCode::BackTab => {
                lane = lane.cycle();
                ui.lane = lane.label();
                // Nothing has answered on the new lane yet. Carrying the old
                // model across would leave the row naming a model this lane
                // never asked for, which is the one thing the row must not do.
                ui.model.clear();
                if let Some(session) = &session {
                    session.lock().await.set_lane(lane.clone());
                }
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
            _ => continue,
        };
        entry.at = event.at_ms;
        ui.entries.push(entry);
    }
}

/// Apply one message from the runtime to the frame.
///
/// Split out of the loop so a test can drive it without a terminal.
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
                ui.scroll_override = None;
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
                ui.scroll_override = None;
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
            ui.scroll_override = None;
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
            let parsed = serde_json::from_str(&arguments)
                .unwrap_or_else(|_| serde_json::json!({ "unparsed_arguments": arguments.clone() }));
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
            ui.scroll_override = None;
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
            ui.scroll_override = None;
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
            ui.scroll_override = None;
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
                ui.scroll_override = None;
            }
        }
        Control::Waiting(message) => {
            ui.waiting = message;
            ui.scroll_override = None;
        }
        Control::Output(text) => {
            if !text.trim().is_empty() {
                ui.entries.push(Entry::new(Role::Output, text));
                ui.scroll_override = None;
            }
        }
        Control::Failed(why) => {
            // The streaming entry settles as whatever did arrive before the
            // failure, and the failure goes next to it rather than into it.
            if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                last.finish_text();
            }
            ui.entries.push(Entry::new(Role::Notice, why));
            ui.loading = false;
            ui.turn_settled();
            ui.waiting = None;
            ui.scroll_override = None;
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
async fn submit(
    ui: &mut CoderUi,
    text: String,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    cwd: &std::path::Path,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
    prompt_queue: &mut VecDeque<QueuedPrompt>,
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

    if crate::composer::is_local_slash_input(&text, commands::COMMANDS) {
        return commands::run(ui, text.trim(), tx, cwd);
    }

    if !matches!(turns.phase(), TurnPhase::Idle) {
        prompt_queue.push_back(QueuedPrompt { text, images });
        update_activity(ui, turns, prompt_queue.len());
        return commands::Outcome::Done;
    }

    start_prompt(ui, text, images, session, tx, turns, active_turn).await;
    update_activity(ui, turns, prompt_queue.len());
    commands::Outcome::Done
}

async fn start_prompt(
    ui: &mut CoderUi,
    text: String,
    images: Vec<ImageAttachment>,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    turns: &mut TurnState,
    active_turn: &mut Option<ActiveTurn>,
) {
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
        if active_tools == 0 {
            task.abort();
            let _ = task.await;
        } else if tokio::time::timeout(
            crate::signals::KILL_GRACE + Duration::from_secs(1),
            &mut task,
        )
        .await
        .is_err()
        {
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

/// Restore terminal state even when an input or draw operation returns early.
struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = crossterm::execute!(
            std::io::stderr(),
            DisableBracketedPaste,
            PopKeyboardEnhancementFlags
        );
        let _ = std::io::stdout().execute(DisableMouseCapture);
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
    fn every_listed_command_is_handled() {
        for (name, _) in crate::coder::commands::COMMANDS {
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
        assert_eq!(
            ui.entries[3].text,
            "later failure (0 tool calls, 0 tokens)"
        );
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
