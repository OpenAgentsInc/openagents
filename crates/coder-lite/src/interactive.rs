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
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use openagents_cli::auth::{CredentialStore, DeviceClient, Secret, open_browser};
use openagents_cli::composer::complete::{Completion, complete};
use openagents_cli::composer::history::History;
use openagents_cli::composer::ComposerAction;
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
}

pub async fn run_tui(options: SessionOptions) -> Result<(), Box<dyn std::error::Error>> {
    if !atty_is_terminal() {
        println!("Non-interactive terminal detected. Run coder-lite from a TTY.");
        return Ok(());
    }

    let lane = Lane::from_str(&options.lane_name);
    let (tx, rx) = mpsc::channel::<Control>();
    let mut ui = CoderUi::new();
    let mut history = History::load(History::default_path());

    let (repo, branch) = git_info().unwrap_or(("unknown".to_string(), "unknown".to_string()));
    ui.repo = repo;
    ui.branch = branch;
    ui.reasoning = options.reasoning.clone();

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
        if validate_token(&endpoint.origin, &Secret::new(token)).await.is_ok() {
            session = Some(Arc::new(Mutex::new(Session::open(
                lane.clone(),
                &options.lane_name,
                options.reasoning.clone(),
                agents.clone(),
                tx.clone(),
            ))));
            ui.entries.push(Entry::new(
                Role::Notice,
                format!(
                    "{} · {} · {acp_line} · /help",
                    lane.label(),
                    crate::runtime::api_base()
                ),
            ));
        } else {
            ui.entries.push(Entry::new(
                Role::Notice,
                "Stored token did not authenticate. Press Enter to log in with GitHub.",
            ));
        }
    } else {
        ui.entries.push(Entry::new(
            Role::Notice,
            "Press Enter to log in with GitHub.",
        ));
    }

    enable_raw_mode()?;
    let mut stdout = stdout();
    stdout.execute(EnterAlternateScreen)?;

    let mut stderr = stderr();
    let flags = event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        | event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
    let _ = crossterm::execute!(stderr, PushKeyboardEnhancementFlags(flags));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.show_cursor()?;

    loop {
        while let Ok(control) = rx.try_recv() {
            apply(&mut ui, control);
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
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
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
                if let Some(session) = &session {
                    if !text.trim().is_empty() {
                        submit(&mut ui, text, session, &tx, &cwd);
                    }
                } else if text.trim().is_empty() {
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        "Opening GitHub login in your browser...",
                    ));
                    match do_login().await {
                        Ok(message) => {
                            ui.entries.push(Entry::new(Role::Notice, message));
                            session = Some(Arc::new(Mutex::new(Session::open(
                                lane.clone(),
                                &options.lane_name,
                                options.reasoning.clone(),
                                agents.clone(),
                                tx.clone(),
                            ))));
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!(
                                    "{} · {} · {acp_line} · /help",
                                    lane.label(),
                                    crate::runtime::api_base()
                                ),
                            ));
                        }
                        Err(error) => {
                            ui.entries.push(Entry::new(
                                Role::Notice,
                                format!("Login failed: {error}"),
                            ));
                        }
                    }
                } else {
                    ui.entries.push(Entry::new(
                        Role::Notice,
                        "Press Enter to log in with GitHub.",
                    ));
                }
            }
            ComposerAction::Redraw => history.stop_walking(),
            ComposerAction::Moved => {}
            // The composer did not want it, so it is the session's.
            ComposerAction::Ignored => {
                handle_session_key(&mut ui, &mut history, &key, width, &cwd);
            }
        }
    }

    terminal.hide_cursor()?;
    let _ = crossterm::execute!(stderr, PopKeyboardEnhancementFlags);
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
                entry.output.get_or_insert_with(String::new).push_str(&chunk);
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
        Control::Usage(usage) => {
            if usage.reported() {
                ui.entries.push(Entry::new(Role::Notice, usage.line()));
            }
        }
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
fn submit(
    ui: &mut CoderUi,
    text: String,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Control>,
    cwd: &std::path::Path,
) {
    ui.scroll_override = None;
    ui.entries.push(Entry::new(Role::You, text.clone()));

    if text.trim_start().starts_with('/') {
        commands::run(ui, text.trim(), tx, cwd);
        return;
    }

    ui.entries.push(Entry::new(Role::Assistant, String::new()));
    ui.loading = true;

    let session = Arc::clone(session);
    let tx = tx.clone();
    tokio::spawn(async move {
        session.lock().await.execute_turn(&text, tx).await;
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
        let mut listed: Vec<&str> =
            crate::commands::COMMANDS.iter().map(|(name, _)| *name).collect();
        listed.sort_unstable();
        assert_eq!(offered, listed);
    }
}
