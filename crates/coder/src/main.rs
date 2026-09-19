//! The Coder terminal, live: a conversation where Classify judges each
//! turn inline and Generate answers through the configured door.
//!
//! `Enter` submits, `Alt-Enter`/`Ctrl-J` puts a newline in the draft,
//! `Up`/`Down` walk wrapped rows then history, `PageUp`/`PageDown` walk the
//! scrollback, `Ctrl-C` or an empty `Ctrl-D` quits. The env reads
//! `TYPESAFE_API_KEY` for Classify and `CODER_DOOR_KEY` (or
//! `CODER_AI_GATEWAY_KEY`), `CODER_DOOR_URL`, and `CODER_MODEL` for
//! Generate; with neither set the shell still runs on the stub door.
//!
//! ```sh
//! cargo run -p coder
//! ```

use std::io::{self, stdout};

use coder::{Agent, Classified, Route, Usage, Verdict};
use coder_terminal::{
    Composer, ComposerAction, Editor, Intensity, Ladder, frame_for, handle_key, wrap_rows,
};
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

/// One scrollback line with the tone it draws at. `prefix` leads the first
/// row (`> ` for a user turn, two spaces for everything else) and its
/// width is the hanging indent when the text wraps.
struct Line {
    /// `Full` for user turns, `ThreeQuarters` for replies, `Half` for
    /// judgments and notes, `Full` underlined for errors.
    intensity: Intensity,
    /// `true` draws the line underlined — errors only.
    loud: bool,
    prefix: &'static str,
    text: String,
}

/// One drawn row — a wrapped segment of a [`Line`], prefix already placed.
struct Row {
    intensity: Intensity,
    loud: bool,
    text: String,
}

/// Wraps `text` so `prefix` plus each segment fits `width` cells, and
/// pushes one [`Row`] per segment: the first carries `prefix`, the rest
/// its width in spaces, so a wrapped line hangs under its own start.
fn expand(
    rows: &mut Vec<Row>,
    intensity: Intensity,
    loud: bool,
    prefix: &str,
    text: &str,
    width: usize,
) {
    let indent = " ".repeat(prefix.chars().count());
    let inner = width.saturating_sub(1 + indent.len()).max(1);
    for (index, range) in wrap_rows(text, inner).iter().enumerate() {
        let segment = &text[range.clone()];
        rows.push(Row {
            intensity,
            loud,
            text: if index == 0 {
                format!("{prefix}{segment}")
            } else {
                format!("{indent}{segment}")
            },
        });
    }
}

/// What the worker task reports back to the draw loop.
enum Work {
    /// Classify finished; the verdict (or the skip note) is for display.
    Classified(Classified),
    /// A reply delta streamed in.
    Delta(String),
    /// The turn ended; the reply text and usage are final.
    Finished(Result<(String, Option<Usage>), String>),
}

struct App {
    editor: Editor,
    lines: Vec<Line>,
    /// The reply streaming in, drawn live under the last settled line.
    pending: String,
    /// A turn is in flight.
    busy: bool,
    /// What the status rail says.
    status: String,
    /// What the token rail says.
    tokens: String,
    /// A draft submitted while a turn ran, taken when it finishes.
    queued: Option<String>,
    scroll: usize,
    tick: u64,
}

impl App {
    fn push(&mut self, intensity: Intensity, prefix: &'static str, text: impl Into<String>) {
        self.lines.push(Line {
            intensity,
            loud: false,
            prefix,
            text: text.into(),
        });
    }

    fn push_loud(&mut self, text: impl Into<String>) {
        self.lines.push(Line {
            intensity: Intensity::Full,
            loud: true,
            prefix: "  ",
            text: text.into(),
        });
    }

    /// The dim lines a verdict earns in the transcript: the route and the
    /// numbers behind it.
    fn show_verdict(&mut self, verdict: &Verdict) {
        let route = match &verdict.route {
            Route::Respond => "respond".to_string(),
            Route::Clarify => "clarify".to_string(),
            Route::End => "end".to_string(),
            Route::Halt(why) => format!("halt — {why}"),
        };
        self.push(Intensity::Half, "  ", format!("classify → {route}"));
        let mut detail = String::new();
        if let Some(action) = &verdict.judgment.action {
            let mut pairs: Vec<String> = action
                .probabilities
                .iter()
                .map(|(name, p)| format!("{name} {p:.2}"))
                .collect();
            pairs.truncate(4);
            detail.push_str(&pairs.join(" · "));
            detail.push_str(&format!(" · conf {:.2}", action.confidence));
        }
        if let Some(risk) = verdict.judgment.risk {
            detail.push_str(&format!(" · risk {risk:.1}"));
        }
        if let Some(progress) = verdict.judgment.progress {
            detail.push_str(&format!(" · prog {progress:.1}"));
        }
        if let Some(code) = verdict.judgment.needs_code {
            detail.push_str(&format!(" · code {code:.2}"));
        }
        self.push(Intensity::Half, "    ", detail);
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// One turn, on the worker task: classify, report the verdict, reply when
/// the route says to.
async fn work_turn(agent: &mut Agent, draft: String, work: &mpsc::Sender<Work>) {
    agent.push_user(&draft);
    let classified = agent.classify().await;
    let _ = work.send(Work::Classified(classified.clone())).await;
    let route = match classified {
        Classified::Judged(verdict) => verdict.route,
        Classified::Skipped(_) => Route::Respond,
    };
    let result = match route {
        Route::Respond | Route::Clarify => {
            let tx = work.clone();
            agent
                .reply(route == Route::Clarify, &mut |delta| {
                    let _ = tx.try_send(Work::Delta(delta.to_string()));
                })
                .await
                .map_err(|error| error.to_string())
        }
        Route::End => Ok(("goodbye.".to_string(), None)),
        Route::Halt(_) => Ok((
            "I don't have a confident next step for that.".to_string(),
            None,
        )),
    };
    let _ = work.send(Work::Finished(result)).await;
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let ladder = Ladder::from_environment();
    let mut app = App {
        editor: Editor::new(),
        lines: Vec::new(),
        pending: String::new(),
        busy: false,
        status: "ready".to_string(),
        tokens: String::new(),
        queued: None,
        scroll: 0,
        tick: 0,
    };

    let (tx, mut rx) = mpsc::channel::<Work>(256);
    // The agent moves to its own task for each turn; the channel returns
    // each phase.
    let mut agent_slot = Some(Agent::from_env());
    let mut turn: Option<tokio::task::JoinHandle<Agent>> = None;

    app.push(
        Intensity::Half,
        "  ",
        format!(
            "coder — classify {}, door {}",
            if agent_slot.as_ref().unwrap().classifies() {
                "jev"
            } else {
                "off"
            },
            agent_slot.as_ref().unwrap().model()
        ),
    );

    let mut events = EventStream::new();
    let mut spinner = interval(Duration::from_millis(50));

    loop {
        // A finished turn's agent comes home; a queued draft starts the
        // next turn.
        if turn.as_ref().is_some_and(|handle| handle.is_finished()) {
            match turn.take().unwrap().await {
                Ok(agent) => agent_slot = Some(agent),
                Err(_) => app.push_loud("the turn task died"),
            }
            app.busy = false;
            app.status = "ready".to_string();
        }
        if turn.is_none()
            && let Some(draft) = app.queued.take()
        {
            start_turn(&mut app, &mut agent_slot, &mut turn, &tx, draft);
        }

        draw(terminal, &ladder, &mut app)?;

        tokio::select! {
            maybe = events.next() => {
                match maybe {
                    Some(Ok(Event::Key(key))) => {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                            KeyCode::Char('d')
                                if key.modifiers.contains(KeyModifiers::CONTROL)
                                    && app.editor.is_empty() => break,
                            KeyCode::PageUp => app.scroll = (app.scroll + 10).min(app.lines.len()),
                            KeyCode::PageDown => app.scroll = app.scroll.saturating_sub(10),
                            _ => {
                                let width = terminal.size()?.width as usize;
                                if let ComposerAction::Submitted(draft) =
                                    handle_key(&mut app.editor, width, &key)
                                {
                                    if draft.is_empty() {
                                        continue;
                                    }
                                    app.scroll = 0;
                                    if app.busy {
                                        app.queued = Some(draft);
                                        app.push(Intensity::Half, "  ", "(queued)");
                                    } else {
                                        start_turn(&mut app, &mut agent_slot, &mut turn, &tx, draft);
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(_)) | None => {}
                    Some(Err(_)) => break,
                }
            }
            _ = spinner.tick() => {
                app.tick += 1;
            }
            Some(work) = rx.recv() => {
                match work {
                    Work::Classified(Classified::Judged(verdict)) => {
                        app.show_verdict(&verdict);
                        app.status = match verdict.route {
                            Route::Respond | Route::Clarify => "generating".to_string(),
                            Route::End | Route::Halt(_) => "ready".to_string(),
                        };
                    }
                    Work::Classified(Classified::Skipped(note)) => {
                        app.push(Intensity::Half, "  ", note);
                        app.status = "generating".to_string();
                    }
                    Work::Delta(delta) => app.pending.push_str(&delta),
                    Work::Finished(Ok((text, usage))) => {
                        // A reply can carry newlines; each logical line is
                        // its own scrollback row.
                        for part in text.trim_end().split('\n') {
                            app.push(Intensity::ThreeQuarters, "  ", part);
                        }
                        app.pending.clear();
                        if let Some(usage) = usage {
                            app.tokens = format!("{}/{}", usage.input_tokens, usage.output_tokens);
                        }
                    }
                    Work::Finished(Err(why)) => {
                        app.pending.clear();
                        app.push_loud(why);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Pushes the user turn into the scrollback and hands the draft to the
/// agent on its own task.
fn start_turn(
    app: &mut App,
    agent_slot: &mut Option<Agent>,
    turn: &mut Option<tokio::task::JoinHandle<Agent>>,
    tx: &mpsc::Sender<Work>,
    draft: String,
) {
    for (i, part) in draft.lines().enumerate() {
        app.push(Intensity::Full, if i == 0 { "> " } else { "  " }, part);
    }
    let Some(mut agent) = agent_slot.take() else {
        app.push_loud("the agent is gone");
        return;
    };
    app.busy = true;
    app.status = "classifying".to_string();
    app.pending.clear();
    let tx = tx.clone();
    *turn = Some(tokio::spawn(async move {
        work_turn(&mut agent, draft, &tx).await;
        agent
    }));
}

/// One frame: scrollback above, the composer at the foot.
fn draw(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ladder: &Ladder,
    app: &mut App,
) -> io::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let prompt = if app.busy {
            frame_for(app.tick)
        } else {
            coder_terminal::PROMPT
        };
        let mut composer = Composer::new(&mut app.editor, *ladder)
            .prompt(prompt)
            .status(&app.status)
            .location("openagents")
            .tokens(&app.tokens);
        let box_height = composer.height(area.width).min(area.height);
        let log_area = Rect::new(0, 0, area.width, area.height - box_height);
        let box_area = Rect::new(0, log_area.height, area.width, box_height);

        let buf = frame.buffer_mut();
        let base = Style::new().bg(ladder.background());
        for y in log_area.top()..log_area.bottom() {
            for x in log_area.left()..log_area.right() {
                buf[(x, y)].set_style(base);
            }
        }

        // The scrollback draws newest-at-bottom. Each line wraps on word
        // boundaries at the row width, its continuation rows hanging under
        // the prefix; the pending reply wraps the same way as it streams.
        let shown = log_area.height as usize;
        let width = usize::from(area.width);
        let mut rows: Vec<Row> = Vec::new();
        for line in &app.lines {
            expand(
                &mut rows,
                line.intensity,
                line.loud,
                line.prefix,
                &line.text,
                width,
            );
        }
        if app.scroll == 0 && !app.pending.is_empty() {
            for part in app.pending.split('\n') {
                expand(
                    &mut rows,
                    Intensity::ThreeQuarters,
                    false,
                    "  ",
                    part,
                    width,
                );
            }
        }
        let end = rows.len().saturating_sub(app.scroll);
        let start = end.saturating_sub(shown);
        for (offset, row) in rows[start..end].iter().enumerate() {
            let mut style = ladder.style(row.intensity);
            if row.loud {
                style = style.add_modifier(ratatui::style::Modifier::UNDERLINED);
            }
            buf.set_string(
                log_area.left() + 1,
                log_area.top() + offset as u16,
                &row.text,
                style,
            );
        }

        let caret = composer.render(box_area, buf);
        frame.set_cursor_position(caret);
    })?;
    Ok(())
}
