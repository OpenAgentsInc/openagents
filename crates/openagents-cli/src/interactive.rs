//! The interactive `oa coder` session.
//!
//! Three pieces, kept apart on purpose:
//!
//! - [`CoderApp`] is the state machine. It takes keys and turn events and
//!   produces a view. It touches no terminal and no network, so a test can
//!   drive it directly and assert on the frame it renders.
//! - [`run_loop`] joins that state machine to a stream of terminal events and
//!   a channel of turn events. It is generic over both, so the loop a test
//!   runs is the loop production runs.
//! - [`runtime_actor`] owns the [`CoderRuntimeSession`] and does the turns.
//!   It is a task rather than a call inside the loop because
//!   `execute_turn` borrows the session for the length of a turn, and the
//!   frame has to keep drawing while that turn streams.
//!
//! The chunk callback `execute_turn` takes is `FnMut(&str) + Send + 'static`,
//! which cannot borrow the transcript. It sends each chunk down a channel
//! instead, and the loop appends it on arrival — so the reply appears as it is
//! written rather than in one block at the end.

use crate::cli::CoderArgs;
use crate::composer::{Composer, ComposerAction};
use crate::runtime::{CoderRuntimeSession, Lane};
use crate::tools::{DelegationGate, HarnessToolRegistry};
use crate::tui::{composer_text_width, BoxFrame, ChromeView, Entry, Role};

use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::{Stream, StreamExt};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use std::io::{stdout, IsTerminal};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// A message for the runtime task.
#[derive(Debug, Clone)]
pub enum Control {
    /// Run a turn on this prompt.
    Prompt(String),
}

/// A message from the runtime task.
#[derive(Debug, Clone)]
pub enum TurnEvent {
    /// A piece of the reply, as it was written.
    Chunk(String),
    /// The turn finished. Carries the final answer, which matters only when
    /// nothing streamed.
    Done(String),
    /// The turn failed. The session stays open.
    Failed(String),
    /// The model the server's grant named for that turn.
    ///
    /// It is reported rather than assumed because the CLI cannot choose it:
    /// `POST /api/v1/threads` publishes no model parameter, and the grant it
    /// returns pins the model that answers.
    Model(String),
}

/// How often the streaming bullet flips.
const PULSE: Duration = Duration::from_millis(400);

pub struct CoderApp {
    title: String,
    entries: Vec<Entry>,
    composer: Composer,
    /// The model the last grant named. Unknown until a turn has opened one.
    model: Option<String>,
    busy: bool,
    pulse: bool,
    scrollback: usize,
    should_exit: bool,
}

impl CoderApp {
    pub fn new(title: &str) -> Self {
        let entries = vec![Entry {
            role: Role::Notice,
            // Every claim here is one this screen keeps. The old welcome text
            // invited the reader to type into a session that discarded keys.
            text: "Type a prompt and press Enter. The reply streams in below \
                   as the model writes it."
                .to_string(),
            settled: true,
        }];
        Self {
            title: title.to_string(),
            entries,
            composer: Composer::new(),
            model: None,
            busy: false,
            pulse: true,
            scrollback: 0,
            should_exit: false,
        }
    }

    /// The model the last turn's grant named, if a turn has opened one.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The transcript as text, for `--export`.
    pub fn transcript(&self) -> String {
        self.entries
            .iter()
            .filter(|e| !e.text.is_empty())
            .map(|e| {
                let who = match e.role {
                    Role::You => "you",
                    Role::Assistant => "coder",
                    Role::Tool => "tool",
                    Role::Notice => "note",
                    Role::Error => "error",
                };
                format!("[{who}] {}", e.text)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn push(&mut self, role: Role, text: impl Into<String>) {
        self.entries.push(Entry::new(role, text));
        // New material at the bottom pulls the view back to the bottom.
        self.scrollback = 0;
    }

    /// Send a prompt, whatever its source: the composer, or `--prompt`.
    pub fn submit(&mut self, prompt: String, control: &UnboundedSender<Control>) {
        let prompt = prompt.trim_end().to_string();
        if prompt.is_empty() {
            return;
        }
        self.push(Role::You, prompt.clone());
        self.entries.push(Entry::streaming(Role::Assistant));
        self.busy = true;
        self.scrollback = 0;
        if control.send(Control::Prompt(prompt)).is_err() {
            self.finish_turn();
            self.push(
                Role::Error,
                "The runtime task is gone, so this prompt was not sent. Restart the session.",
            );
        }
    }

    /// Settle whatever was streaming and take the composer off hold.
    fn finish_turn(&mut self) {
        if let Some(last) = self.entries.last_mut() {
            if !last.settled {
                last.settled = true;
            }
        }
        self.busy = false;
    }

    pub fn on_turn_event(&mut self, event: TurnEvent) {
        match event {
            TurnEvent::Chunk(chunk) => {
                match self.entries.last_mut() {
                    Some(last) if !last.settled => last.text.push_str(&chunk),
                    // A chunk with no open turn to attach to still belongs on
                    // the transcript rather than in the bin.
                    _ => self.entries.push(Entry {
                        role: Role::Assistant,
                        text: chunk,
                        settled: false,
                    }),
                }
                self.scrollback = 0;
            }
            TurnEvent::Done(answer) => {
                // `execute_turn` returns the last step's text, which has
                // already streamed. It is the fallback for the paths that
                // return without streaming anything.
                if let Some(last) = self.entries.last_mut() {
                    if !last.settled && last.text.is_empty() {
                        last.text = answer;
                    }
                }
                self.finish_turn();
            }
            TurnEvent::Failed(message) => {
                self.finish_turn();
                self.push(Role::Error, format!("Turn failed: {message}"));
            }
            TurnEvent::Model(model) => self.model = Some(model),
        }
    }

    pub fn tick(&mut self) {
        if self.busy {
            self.pulse = !self.pulse;
        } else {
            self.pulse = true;
        }
    }

    pub fn on_key(&mut self, key: &KeyEvent, width: u16, control: &UnboundedSender<Control>) {
        // A key release reported by an enhanced protocol is not a keystroke.
        if key.kind == KeyEventKind::Release {
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.should_exit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_exit = true;
                return;
            }
            KeyCode::PageUp => {
                self.scrollback = self.scrollback.saturating_add(5);
                return;
            }
            KeyCode::PageDown => {
                self.scrollback = self.scrollback.saturating_sub(5);
                return;
            }
            _ => {}
        }

        if self.busy {
            // The composer is on hold. Saying so is the point of the pane's
            // title; swallowing the key here is what makes that true.
            return;
        }

        match self.composer.handle_key(key, composer_text_width(width)) {
            ComposerAction::Submit(text) => self.submit(text, control),
            ComposerAction::Redraw => self.scrollback = 0,
            ComposerAction::Ignored => match key.code {
                // Up and Down reach the transcript once the caret has run out
                // of composer to move through.
                KeyCode::Up => self.scrollback = self.scrollback.saturating_add(1),
                KeyCode::Down => self.scrollback = self.scrollback.saturating_sub(1),
                _ => {}
            },
        }
    }

    /// Draw one frame.
    pub fn draw<B: Backend>(&self, terminal: &mut Terminal<B>) -> std::io::Result<()> {
        let frame = BoxFrame::new(&self.title);
        terminal.draw(|f| {
            let area = f.area();
            let rows = self.composer.rows(composer_text_width(area.width));
            let cursor = self.composer.cursor_rowcol(composer_text_width(area.width));
            let view = ChromeView {
                title: &self.title,
                entries: &self.entries,
                composer_rows: &rows,
                composer_cursor: cursor,
                model: self.model.as_deref(),
                busy: self.busy,
                pulse: self.pulse,
                scrollback: self.scrollback,
            };
            frame.render(f, area, &view);
        })?;
        Ok(())
    }
}

/// Drive the session until the reader exits or the terminal event stream ends.
///
/// `keepalive` is a sender for the turn channel that this function holds for
/// its own lifetime, so `turns.recv()` cannot resolve to `None` and spin the
/// loop when the runtime task ends.
pub async fn run_loop<B, S>(
    terminal: &mut Terminal<B>,
    app: &mut CoderApp,
    events: &mut S,
    control: UnboundedSender<Control>,
    turns: &mut UnboundedReceiver<TurnEvent>,
    keepalive: UnboundedSender<TurnEvent>,
) -> std::io::Result<()>
where
    B: Backend,
    S: Stream<Item = std::io::Result<Event>> + Unpin,
{
    let _keepalive = keepalive;
    let mut ticker = tokio::time::interval(PULSE);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        app.draw(terminal)?;
        if app.should_exit() {
            return Ok(());
        }

        let width = terminal.size()?.width;

        tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) => app.on_key(&key, width, &control),
                Some(Ok(_)) => {}
                // A terminal that has gone away is an exit, not an error to
                // report into a screen nobody can see.
                Some(Err(_)) | None => return Ok(()),
            },
            turn = turns.recv() => {
                if let Some(event) = turn {
                    app.on_turn_event(event);
                }
            },
            _ = ticker.tick() => app.tick(),
        }
    }
}

/// Own the session and run the turns it is asked for.
pub async fn runtime_actor(
    mut session: CoderRuntimeSession,
    mut control: UnboundedReceiver<Control>,
    events: UnboundedSender<TurnEvent>,
) {
    while let Some(message) = control.recv().await {
        match message {
            Control::Prompt(prompt) => {
                let sink = events.clone();
                let result = session
                    .execute_turn(&prompt, move |chunk| {
                        let _ = sink.send(TurnEvent::Chunk(chunk.to_string()));
                    })
                    .await;
                // Report the grant's model before the turn settles, so the
                // status bar names what answered rather than what was asked.
                if let Some(grant) = &session.last_grant {
                    let _ = events.send(TurnEvent::Model(grant.model.clone()));
                }
                let event = match result {
                    Ok(answer) => TurnEvent::Done(answer),
                    Err(error) => TurnEvent::Failed(error.to_string()),
                };
                if events.send(event).is_err() {
                    return;
                }
            }
        }
    }
}

/// Put the terminal back however this function is left, including by a panic
/// or by an error on the way out of a turn.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = stdout().execute(crossterm::cursor::Show);
    }
}

/// A panic inside the alternate screen would otherwise leave the terminal in
/// raw mode with the backtrace painted somewhere the reader cannot scroll to.
fn install_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);
            let _ = stdout().execute(crossterm::cursor::Show);
            previous(info);
        }));
    });
}

/// The tools a session a person sits in gets.
///
/// An interactive session may start children on the same terms a headless one
/// does: same lane, same credential, and the children do not get the tool
/// themselves. This lives in one function so both entry points here share it —
/// they each built the registry separately before, both passed `None`, and the
/// result was that `delegate` worked headless and was missing from the only
/// session anyone actually types into.
fn session_tools(lane_name: &str, token: &Option<String>) -> HarnessToolRegistry {
    HarnessToolRegistry::with_delegation(
        None,
        DelegationGate {
            lane: lane_name.to_string(),
            user_token: token.clone(),
            max_count: crate::delegate::MAX_DELEGATE_COUNT,
        },
    )
}

pub async fn run_tui(
    args: CoderArgs,
    api_base: String,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let lane_name = args.lane.clone().unwrap_or_else(|| "ox-alpha".to_string());
    let lane = Lane::from_str(&lane_name);

    // `--plain` asks for the line-oriented path even on a terminal. Without a
    // terminal there is no full-screen session to run either way.
    if args.plain || !is_terminal() {
        return run_without_a_terminal(args, api_base, token, lane).await;
    }

    let tools = session_tools(&lane_name, &token);
    let session = CoderRuntimeSession::new(lane.clone(), Some(api_base), token, tools);

    let (control_tx, control_rx) = unbounded_channel::<Control>();
    let (event_tx, mut event_rx) = unbounded_channel::<TurnEvent>();
    let runtime = tokio::spawn(runtime_actor(session, control_rx, event_tx.clone()));

    let mut app = CoderApp::new("openagents coder");
    if let Some(prompt) = args.prompt.clone() {
        app.submit(prompt, &control_tx);
    }

    install_panic_hook();
    let result = {
        let _guard = TerminalGuard::enter()?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;
        let mut events = EventStream::new();
        run_loop(
            &mut terminal,
            &mut app,
            &mut events,
            control_tx,
            &mut event_rx,
            event_tx,
        )
        .await
    };

    runtime.abort();
    result?;

    if let Some(path) = args.export {
        std::fs::write(&path, app.transcript())?;
        println!("Transcript written to {path}");
    }
    Ok(())
}

/// The `--export` transcript for a one-turn run.
///
/// The same two-role, `[who] text` shape [`CoderApp::transcript`] writes, so a
/// transcript from a plain or headless run reads like one from the session.
pub fn transcript_of(prompt: &str, answer: &str) -> String {
    let mut parts = Vec::new();
    if !prompt.is_empty() {
        parts.push(format!("[you] {prompt}"));
    }
    if !answer.trim().is_empty() {
        parts.push(format!("[coder] {}", answer.trim_end()));
    }
    parts.join("\n\n")
}

/// Line-oriented output: one turn, printed as lines, with no cursor control.
///
/// This is both the no-terminal path and what `--plain` asks for on a
/// terminal. It emits no escape sequences at all, so it can be piped to a file
/// or read by something that is not a terminal emulator.
///
/// The version this replaces printed `Coder response: Interactive session
/// initialized in non-TTY mode.` and never called the runtime at all.
async fn run_without_a_terminal(
    args: CoderArgs,
    api_base: String,
    token: Option<String>,
    lane: Lane,
) -> Result<(), Box<dyn std::error::Error>> {
    let lane_name = args.lane.clone().unwrap_or_else(|| "ox-alpha".to_string());
    let Some(prompt) = args.prompt.clone() else {
        eprintln!(
            "`oa coder` needs a terminal for an interactive session. \
             Give it a prompt, or use `--headless`, to run one turn here."
        );
        return Ok(());
    };

    let tools = session_tools(&lane_name, &token);
    let mut session = CoderRuntimeSession::new(lane, Some(api_base), token, tools);
    // The reply is printed as it streams. `execute_turn` also returns the last
    // step's text, which is the same text — so it is printed only when nothing
    // streamed, which is how the offline paths still say something.
    let streamed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let saw = std::sync::Arc::clone(&streamed);
    let answer = session
        .execute_turn(&prompt, move |chunk| {
            use std::io::Write;
            saw.store(true, std::sync::atomic::Ordering::Relaxed);
            print!("{chunk}");
            let _ = std::io::stdout().flush();
        })
        .await
        .map_err(|e| e.to_string())?;
    if !streamed.load(std::sync::atomic::Ordering::Relaxed) {
        print!("{answer}");
    }
    println!();

    // `--export` used to be read only by the full-screen session, so a plain,
    // piped, or headless run that asked for a transcript got none and was told
    // nothing.
    if let Some(path) = args.export.as_deref() {
        std::fs::write(path, transcript_of(&prompt, &answer))
            .map_err(|error| format!("could not write the transcript to {path}: {error}"))?;
        println!("Transcript written to {path}");
    }
    Ok(())
}

fn is_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both entry points in this file once built their own registry with
    /// `HarnessToolRegistry::new(None)`, which carries no delegation gate. The
    /// symptom was subtle: `oa coder --headless` could start children and the
    /// interactive session silently could not, because a missing tool looks
    /// exactly like a model choosing not to call it.
    #[test]
    fn an_interactive_session_can_delegate() {
        let tools = session_tools("ox-alpha", &Some("token".to_string()));
        let names: Vec<String> = tools.list_tools().into_iter().map(|t| t.name).collect();
        assert!(
            names.iter().any(|n| n == "delegate"),
            "an interactive session got no `delegate` tool; it has {:?}",
            names
        );
    }

    /// The gate carries the lane and credential children spend against. A gate
    /// that exists but names the wrong lane would start children on the default
    /// lane while the session runs on another, which is worse than no gate.
    #[test]
    fn the_gate_carries_this_sessions_lane_and_credential() {
        let tools = session_tools("claude", &Some("secret-token".to_string()));
        let gate = tools
            .delegation
            .as_ref()
            .expect("an interactive session must carry a delegation gate");
        assert_eq!(gate.lane, "claude");
        assert_eq!(gate.user_token.as_deref(), Some("secret-token"));
        assert_eq!(gate.max_count, crate::delegate::MAX_DELEGATE_COUNT);
    }
}
