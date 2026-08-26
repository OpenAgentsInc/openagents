//! The interactive coder-lite session.
//!
//! One session, one thread, one frame. The session is
//! [`crate::runtime::Session`] and it is held for the life of the process
//! rather than built per turn — a session rebuilt on every Enter forgets the
//! conversation and leaves a thread open behind it, which is what this used to
//! do — and it is revoked on the way out.
//!
//! Key handling follows the pattern used in grok-build's ratatui-textarea and
//! grok-pager: destructure `crossterm::event::KeyEvent` by `code` and
//! `modifiers` so control chords do not fall through to plain character input.
//! Everything the composer claims is either handled here or handed to
//! [`openagents_cli::composer`], which handles it; `/help` lists exactly that
//! set and nothing else.

use crate::commands;
use crate::export::{export_trajectory, git_info};
use crate::runtime::{Control, Session, tool_title};
use crate::tui::{CoderUi, Entry, Role, ToolCall};
use crossterm::{
    ExecutableCommand,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use openagents_cli::auth::{CredentialStore, DeviceClient, Secret, open_browser};
use openagents_cli::composer::ComposerAction;
use openagents_cli::composer::complete::{Completion, complete};
use openagents_cli::composer::history::History;
use openagents_cli::runtime::Lane;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{stderr, stdout};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use tokio::sync::Mutex;

/// How long the exit waits for a thread to be revoked before it says it could
/// not be. A turn still streaming would otherwise hold the exit for as long as
/// the model wants.
const REVOCATION_GRACE: Duration = Duration::from_secs(10);

/// What `--lane` and `--reasoning` settled on before the screen was entered.
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    /// The lane name as it was typed, for the delegation gate and refusals.
    pub lane_name: String,
    /// `--reasoning`, recorded on the thread at open. `None` leaves the
    /// deployment's own default, which is a different answer from naming one.
    pub reasoning: Option<String>,
    /// `--dev` routes to the OpenResponses streaming surface.
    pub dev: bool,
}

pub async fn run_tui(options: SessionOptions) -> Result<(), Box<dyn std::error::Error>> {
    if !atty_is_terminal() {
        println!("Non-interactive terminal detected. Run coder-lite from a TTY.");
        return Ok(());
    }

    // Mutable because shift+tab moves it.
    let mut lane = Lane::from_str(&options.lane_name);
    let (tx, rx) = mpsc::channel::<Control>();
    let mut ui = CoderUi::new();
    let mut history = History::load(History::default_path());

    let (repo, branch) = git_info().unwrap_or(("unknown".to_string(), "unknown".to_string()));
    ui.repo = repo;
    ui.branch = branch;
    ui.reasoning = options.reasoning.clone();
    // The two facts the row under the composer needs that do not depend on a
    // credential: where this session is talking, and what it asked for.
    ui.endpoint = crate::runtime::api_base();
    ui.lane = lane.label();

    // Only agents that are actually installed. `find_agents` checks each one
    // before reporting it, so the `acp` tool is declared over a list of
    // programs that exist on this machine rather than a registry's wish list.
    let agents = crate::acp::find_agents().await.unwrap_or_default();
    let acp_line = if agents.is_empty() {
        "no ACP agents installed".to_string()
    } else {
        format!(
            "acp: {}",
            agents
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    ui.agents = agents.clone();

    let mut session: Option<Arc<Mutex<Session>>> = None;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(token) = crate::runtime::user_token() {
        let endpoint = openagents_cli::auth::resolve_endpoint(None, None)?;
        let token = Secret::new(token);
        if validate_token(&endpoint.origin, &token).await.is_ok() {
            // Who, before the screen opens. A token that authenticates is not
            // yet an account name, and the row may only show one the server
            // gave.
            ui.identity = resolve_identity(&endpoint.origin, &token).await;
            session = Some(Arc::new(Mutex::new(Session::open(
                lane.clone(),
                &options.lane_name,
                options.reasoning.clone(),
                agents.clone(),
                options.dev,
                tx.clone(),
            ))));
            ui.entries.push(Entry::new(
                Role::Notice,
                format!(
                    "Coder v{} · {} · {} · {acp_line} · /help",
                    openagents_cli::VERSION,
                    lane.label(),
                    crate::runtime::api_base()
                ),
            ));
        } else {
            // A stored token the deployment refused. The row says the
            // credential is unverified rather than naming whoever it was
            // issued to: a login drawn over a dead token is a session that
            // looks live while every turn fails.
            ui.identity = crate::tui::Identity::Unverified;
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
    stdout.execute(EnterAlternateScreen)?;
    // Keep mouse reporting off so the terminal owns drag-selection and copying
    // throughout the transcript. This follows grok-build's native-selection
    // mode: application mouse capture and normal terminal text selection are
    // mutually exclusive. Keyboard scrolling remains available.

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

    loop {
        while let Ok(control) = rx.try_recv() {
            // A turn that has stopped, either way, is the moment the server's
            // figure can have moved. Read it again rather than adjusting the
            // one on screen: this terminal is not the only thing spending.
            let settled = matches!(control, Control::Done | Control::Failed(_));
            apply(&mut ui, control);
            if settled {
                refresh_credit(&tx);
            }
        }

        terminal.draw(|f| {
            let size = f.area();
            ui.render(f, size);
        })?;

        // ratatui has no hyperlink concept, so repaint the link runs as OSC 8
        // sequences over the frame it just flushed. `emit` re-reads the text
        // out of the buffer, so this can never change what a cell says.
        if !ui.links.is_empty() {
            let buffer = terminal.current_buffer_mut().clone();
            let mut out = std::io::stdout();
            let _ = crate::osc8::emit(&mut out, &ui.links, &buffer);
            let cursor = terminal.get_cursor_position()?;
            terminal.set_cursor_position(cursor)?;
        }

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let key = match event::read()? {
            Event::Paste(text) => {
                // Paste is one editing operation, even when it contains
                // newlines. In particular, do not pass its newlines through
                // the Enter path below: they belong in the prompt.
                ui.composer.insert_str(&normalize_paste(&text));
                history.stop_walking();
                continue;
            }
            Event::Key(key) => key,
            Event::Mouse(_) => continue,
            _ => continue,
        };
        if key.kind == KeyEventKind::Repeat
            && !matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
            )
        {
            continue;
        }
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }

        let width = composer_width(terminal.size()?.width);

        // Leaving is checked before the composer sees the key, so Ctrl+C is
        // never swallowed by an editing chord.
        if is_quit(&key) {
            break;
        }

        match ui.composer.handle_key(&key, width) {
            ComposerAction::Submit(text) => {
                history.record(&text);
                history.stop_walking();
                let mut asked_to_log_out = false;
                if let Some(session) = &session {
                    if !text.trim().is_empty() {
                        asked_to_log_out =
                            submit(&mut ui, text, session, &tx, &cwd) == commands::Outcome::Logout;
                    }
                } else if text.trim().is_empty() || text.trim() == "/login" {
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        "Opening the OpenAgents sign-in page in your browser...",
                    ));
                    match do_login().await {
                        Ok(message) => {
                            ui.entries.push(Entry::new(Role::Notice, message));
                            // The row was saying "not signed in" a moment ago,
                            // and now it has an account to name — asked for
                            // once, here, rather than assumed from the login
                            // having returned.
                            ui.identity = current_identity().await;
                            session = Some(Arc::new(Mutex::new(Session::open(
                                lane.clone(),
                                &options.lane_name,
                                options.reasoning.clone(),
                                agents.clone(),
                                options.dev,
                                tx.clone(),
                            ))));
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!(
                                    "Coder v{} · {} · {} · {acp_line} · /help",
                                    openagents_cli::VERSION,
                                    lane.label(),
                                    crate::runtime::api_base()
                                ),
                            ));
                            // The account is only now known, so this is the
                            // first read that can answer.
                            refresh_credit(&tx);
                        }
                        Err(error) => {
                            ui.entries
                                .push(Entry::new(Role::Notice, format!("Login failed: {error}")));
                        }
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
                            ui.identity = crate::tui::Identity::Anonymous;
                            ui.credit = crate::credit::CreditField::Unread;
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
    let _ = crossterm::execute!(stderr, DisableBracketedPaste, PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;

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
            Ok(Err(error)) => eprintln!("coder-lite: the thread was not ended: {error}"),
            Err(_) => eprintln!(
                "coder-lite: the session was still working after {}s, so its thread was left \
                 to the best-effort ending.",
                REVOCATION_GRACE.as_secs()
            ),
        }
    }
    Ok(())
}

/// Ask the deployment what the account has left, off the frame loop.
///
/// Spawned rather than awaited, because a slow deployment must not hold a
/// frame: the answer arrives as a [`Control`] like everything else. The
/// outcome is sent either way, including when there is no credential to ask
/// with — the bottom row has to stop showing a figure it can no longer
/// confirm, and `None` is what tells it to.
fn refresh_credit(tx: &Sender<Control>) {
    let token = crate::runtime::user_token();
    let base = crate::runtime::api_base();
    let tx = tx.clone();

    tokio::spawn(async move {
        let outcome = match token {
            Some(token) => crate::credit::fetch(&base, &token).await,
            None => None,
        };
        let _ = tx.send(Control::Credit(outcome));
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
async fn current_identity() -> crate::tui::Identity {
    let Ok(endpoint) = openagents_cli::auth::resolve_endpoint(None, None) else {
        return crate::tui::Identity::Unverified;
    };
    let Some(token) = crate::runtime::user_token() else {
        return crate::tui::Identity::Anonymous;
    };
    resolve_identity(&endpoint.origin, &Secret::new(token)).await
}

/// Ask the server who this credential belongs to.
///
/// The same `GET /api/v1/user` that `openagents auth status` reads, so the row
/// under the composer and that command cannot disagree about who is signed in.
/// A refusal or an unreachable server is [`crate::tui::Identity::Unverified`]
/// — the credential exists, nobody confirmed it, and that is what the row will
/// say. It is never an account name.
async fn resolve_identity(origin: &str, token: &Secret) -> crate::tui::Identity {
    let client = openagents_cli::repo::RepoClient::new(origin, Some(token.clone()));
    match client.authenticated_user().await {
        Ok(user) => crate::tui::Identity::Named {
            login: user.login,
            id: user.id,
            namespaces: user
                .namespaces
                .into_iter()
                .map(|namespace| namespace.login)
                .collect(),
            expires_at: user.token_expires_at,
        },
        Err(_) => crate::tui::Identity::Unverified,
    }
}

/// Start the GitHub device-authorization flow against the current endpoint,
/// open the approval URL in the browser, poll for the token, store it, and
/// verify it against the model catalog. The token is also placed in
/// `OPENAGENTS_API_KEY` so the runtime spends it without a second store lookup.
///
/// This is called from inside the TUI, so it must not print to stdout; the
/// caller is responsible for showing any message in the transcript.
pub async fn do_login() -> Result<String, Box<dyn std::error::Error>> {
    let endpoint = openagents_cli::auth::resolve_endpoint(None, None)?;
    let client = DeviceClient::new(&endpoint.origin);
    let scopes: &[String] = &[];
    let auth = client.start(scopes).await?;

    open_browser(&auth.verification_uri_complete);

    let token = client.wait(&auth).await?;
    validate_token(&endpoint.origin, &token).await?;

    let store = CredentialStore::for_origin(&endpoint.origin);
    let _ = store.store(&token);

    // SAFETY: this process owns the environment; the TUI has not started here.
    unsafe { std::env::set_var("OPENAGENTS_API_KEY", token.expose()) };

    Ok("Authenticated.".to_string())
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
            code: KeyCode::Esc,
            ..
        } | KeyEvent {
            code: KeyCode::Char('c' | 'd' | 'q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    )
}

/// Normalize the line endings terminal emulators use for bracketed paste.
///
/// The composer stores hard breaks as `\n`; keeping `\r` would put the caret
/// and wrapping logic out of agreement with the text the submitted turn uses.
fn normalize_paste(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Apply one message from the runtime to the frame.
///
/// Split out of the loop so a test can drive it without a terminal.
pub fn apply(ui: &mut CoderUi, control: Control) {
    match control {
        Control::Chunk(chunk) => {
            if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                last.push_text(&chunk);
                ui.scroll_override = None;
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
        Control::ToolDone { call_id, is_error } => {
            if is_error {
                if let Some(entry) = tool_entry(ui, &call_id) {
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
        Control::Model(model) => ui.model = model,
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
        // nothing replaces the figure too — see `crate::credit`.
        Control::Credit(outcome) => ui.credit.record(outcome),
        Control::Notice(text) => {
            if !text.trim().is_empty() {
                ui.entries.push(Entry::new(Role::Notice, text));
                ui.scroll_override = None;
            }
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
            ui.scroll_override = None;
        }
        Control::Done => {
            if let Some(last) = ui.entries.iter_mut().rfind(|e| e.role == Role::Assistant) {
                last.finish_text();
            }
            ui.loading = false;
        }
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
fn submit(
    ui: &mut CoderUi,
    text: String,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    cwd: &std::path::Path,
) -> commands::Outcome {
    ui.scroll_override = None;
    ui.entries.push(Entry::new(Role::You, text.clone()));

    if text.trim_start().starts_with('/') {
        return commands::run(ui, text.trim(), tx, cwd);
    }

    ui.entries.push(Entry::new(Role::Assistant, String::new()));
    ui.loading = true;

    let session = Arc::clone(session);
    let tx = tx.clone();
    tokio::spawn(async move {
        session.lock().await.execute_turn(&text, tx).await;
    });
    commands::Outcome::Done
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
    std::io::IsTerminal::is_terminal(&std::io::stdin())
        && std::io::IsTerminal::is_terminal(&std::io::stdout())
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
        for (name, _) in crate::commands::COMMANDS {
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
        let mut listed: Vec<&str> = crate::commands::COMMANDS
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
}
