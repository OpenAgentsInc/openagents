//! The Coder agent's binary: a terminal to talk in, and `--print` to drive
//! one turn from a script.
//!
//! Both modes run the same turn — [`coder::turn::run`] — so what an
//! episode records headlessly is what a person sees interactively. The
//! terminal is below; the headless mode is in `headless.rs` and the flags
//! that choose between them are in `cli.rs`.
//!
//! ```sh
//! coder                                  # the terminal
//! coder -p "count the crates"            # one turn, the reply on stdout
//! coder -p --json --trace out.jsonl "…"  # one turn, one JSON object
//! ```
//!
//! # The terminal
//!
//! A conversation where Classify judges each turn inline and Generate
//! answers through the configured door.
//!
//! `Enter` submits, `Alt-Enter`/`Ctrl-J` puts a newline in the draft,
//! `Up`/`Down` walk wrapped rows then history, `PageUp`/`PageDown` walk the
//! scrollback, `Ctrl-C` or an empty `Ctrl-D` quits. `Alt-V` or the
//! `/verbose` command toggles the detail lines — classify verdicts,
//! command whys, exit codes, shell judgments — off by default, so a turn
//! shows only what ran and what answered. The env reads
//! `TYPESAFE_API_KEY` for Classify and `CODER_DOOR_KEY` (or
//! `CODER_AI_GATEWAY_KEY`), `CODER_DOOR_URL`, and `CODER_MODEL` for
//! Generate; with no key set, `CODER_WORKER` + `CODER_RELAY` route the
//! turn through the NIP-CJ job protocol on the relay; otherwise the stub
//! door answers.
//!
//! Every conversation records itself to
//! `~/.openagents/traces/<session>.atif.jsonl` as it runs, one file per
//! terminal invocation. `CODER_TRACE_DIR` moves that directory,
//! `--trace <PATH>` names the file outright, and `CODER_TRACE=off` turns
//! recording off; the session's first detail line says which. See
//! `docs/coder/traces.md`.
//!
//! The terminal is held by a [`Guard`] that hands it back on every exit
//! path, the turn reports on two lanes so a command outcome is never lost
//! behind streamed text, and the scrollback is bounded and wraps each line
//! once per width. `docs/coder/terminal.md` covers all three.

mod cli;
mod headless;

use std::io::{self, stdout};
use std::path::Path;
use std::process::ExitCode;

use coder::turn::{self, Event as TurnEvent};
use coder::{Agent, Classified, Route, ShellEvent, Usage, Verdict};
use coder_terminal::events::{self, Feed, Lane};
use coder_terminal::{
    Composer, ComposerAction, Editor, Guard, Intensity, Ladder, Marked, Marks, Rendered,
    Scrollback, frame_for, handle_key, markdown, wrap_rows,
};
use std::time::Instant;

use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures_util::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::Style;
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
    /// `true` draws the line only in verbose mode — verdicts, whys, exit
    /// codes, and judge lines. Filtering happens at draw time, so toggling
    /// verbose reveals and hides the detail retroactively.
    detail: bool,
    /// Extra cells continuation rows indent past the prefix — a list
    /// item's wraps sit under its text, not its marker.
    hang: usize,
    prefix: &'static str,
    marked: Marked,
}

/// One drawn row — a wrapped segment of a [`Line`] as styled spans: the
/// prefix or hanging indent leads, marked runs follow.
struct Row {
    intensity: Intensity,
    loud: bool,
    spans: Vec<(String, Marks)>,
}

/// The rows one scrollback [`Line`] draws at `width` — what the
/// [`Scrollback`] caches per line.
fn wrap_line(line: &Line, width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    expand(
        &mut rows,
        line.intensity,
        line.loud,
        line.prefix,
        &line.marked,
        line.hang,
        width,
    );
    rows
}

/// Wraps `marked` so `prefix` plus each segment fits `width` cells, and
/// pushes one [`Row`] per segment: the first carries `prefix`, the rest
/// its width plus `hang` in spaces, so a wrapped line hangs under its own
/// start.
fn expand(
    rows: &mut Vec<Row>,
    intensity: Intensity,
    loud: bool,
    prefix: &str,
    marked: &Marked,
    hang: usize,
    width: usize,
) {
    let indent = " ".repeat(prefix.chars().count() + hang);
    let inner = width.saturating_sub(1 + indent.len()).max(1);
    for (index, range) in wrap_rows(&marked.text, inner).iter().enumerate() {
        let lead = if index == 0 {
            prefix.to_string()
        } else {
            indent.clone()
        };
        let mut spans = vec![(lead, Marks::default())];
        spans.extend(marked.runs_in(range.clone()));
        rows.push(Row {
            intensity,
            loud,
            spans,
        });
    }
}

/// What the worker task reports back to the draw loop.
enum Work {
    /// Classify finished; the verdict (or the skip note) is for display.
    Classified(Classified),
    /// A remote worker's judgment feedback line (NIP-CJ), drawn dim.
    Judgment(String),
    /// A shell-loop event: a proposal, an outcome, or the judge's verdict.
    Shell(ShellEvent),
    /// A reply delta streamed in.
    Delta(String),
    /// A program was selected; the turn runs it instead of answering.
    Program(String),
    /// The turn ended; the reply text and usage are final.
    Finished(Result<(String, Option<Usage>), String>),
}

/// Deltas are preview text and may coalesce or, under pressure, drop;
/// everything else changes what the transcript says and must arrive.
impl events::Event for Work {
    fn lane(&self) -> Lane {
        match self {
            Work::Delta(_) => Lane::Text,
            _ => Lane::Control,
        }
    }

    fn text_len(&self) -> usize {
        match self {
            Work::Delta(delta) => delta.len(),
            _ => 0,
        }
    }

    fn coalesce(&mut self, next: &Self) -> bool {
        match (self, next) {
            (Work::Delta(mine), Work::Delta(theirs)) => {
                mine.push_str(theirs);
                true
            }
            _ => false,
        }
    }
}

/// The bounded transcript with its wrap cache.
type Lines = Scrollback<Line, Row, fn(&Line, usize) -> Vec<Row>>;

struct App {
    editor: Editor,
    lines: Lines,
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
    /// When the turn in flight started, for the status rail's stopwatch.
    busy_since: Option<Instant>,
    /// Draw detail lines (verdicts, whys, exit codes, judge lines) —
    /// toggled by `⌥V` or `/verbose`, off by default.
    verbose: bool,
}

impl App {
    fn push(&mut self, intensity: Intensity, prefix: &'static str, text: impl Into<String>) {
        self.lines.push(Line {
            intensity,
            loud: false,
            detail: false,
            hang: 0,
            prefix,
            marked: Marked::plain(text.into()),
        });
    }

    /// A rendered markdown line of a finished reply.
    fn push_rendered(&mut self, rendered: Rendered) {
        self.lines.push(Line {
            intensity: rendered.intensity,
            loud: false,
            detail: false,
            hang: rendered.hang,
            prefix: "  ",
            marked: rendered.marked,
        });
    }

    /// A detail line: drawn only while verbose mode is on.
    fn push_detail(&mut self, prefix: &'static str, text: impl Into<String>) {
        self.lines.push(Line {
            intensity: Intensity::Half,
            loud: false,
            detail: true,
            hang: 0,
            prefix,
            marked: Marked::plain(text.into()),
        });
    }

    fn push_loud(&mut self, text: impl Into<String>) {
        self.lines.push(Line {
            intensity: Intensity::Full,
            loud: true,
            detail: false,
            hang: 0,
            prefix: "  ",
            marked: Marked::plain(text.into()),
        });
    }

    /// The dim lines a verdict earns in the transcript: the route and the
    /// numbers behind it. Both are detail — verbose mode only.
    fn show_verdict(&mut self, verdict: &Verdict) {
        let route = match &verdict.route {
            Route::Respond => "respond".to_string(),
            Route::Clarify => "clarify".to_string(),
            Route::End => "end".to_string(),
            Route::Halt(why) => format!("halt — {why}"),
        };
        self.push_detail("  ", format!("classify → {route}"));
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
        self.push_detail("    ", detail);
    }

    /// Flips verbose mode and says so. The scroll offset resets because
    /// the visible row count changes with the filter.
    fn toggle_verbose(&mut self) {
        self.verbose = !self.verbose;
        self.scroll = 0;
        self.push(
            Intensity::Half,
            "  ",
            format!("verbose {}", if self.verbose { "on" } else { "off" }),
        );
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse(&arguments) {
        Ok(cli::Invocation::Help) => {
            println!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
        Ok(cli::Invocation::Print(options)) => ExitCode::from(headless::print(options).await),
        Ok(cli::Invocation::Interactive { trace }) => match interactive(trace.as_deref()).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("coder: {error}");
                ExitCode::from(headless::EXIT_FAILED)
            }
        },
        Err(why) => {
            eprintln!("coder: {why}\n\n{}", cli::USAGE);
            ExitCode::from(cli::EXIT_USAGE)
        }
    }
}

/// The terminal: the guard takes raw mode, the alternate screen, and the
/// cursor, the draw loop runs, and the guard hands them back however the
/// loop ends — a setup step that fails, a quit, an error, or a panic.
async fn interactive(trace: Option<&Path>) -> io::Result<()> {
    let guard = Guard::full_screen()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut terminal, trace).await;

    // Restoring by hand reports what dropping the guard would swallow.
    let restored = guard.restore();
    result.and(restored)
}

/// One turn, on the worker task: [`turn::run`] does the turn and this
/// puts each phase on the draw loop's channel.
///
/// The turn itself is not here on purpose. `--print` runs the same one,
/// and a turn written twice is two turns that drift.
///
/// Control events — verdicts, shell proposals and outcomes, the program
/// choice, the finish — cannot be dropped for pressure; a delta can, and
/// the draw loop says so when one is. The only way a control event does
/// not land is that the draw loop is gone, and then there is nobody to
/// tell.
async fn work_turn(agent: &mut Agent, draft: String, feed: &Feed<Work>) {
    let finished = turn::run(agent, draft, &mut |event| {
        let work = match event {
            TurnEvent::Classified(classified) => Work::Classified(classified),
            TurnEvent::Judgment(line) => Work::Judgment(line),
            TurnEvent::Shell(shell) => Work::Shell(shell),
            TurnEvent::Delta(delta) => Work::Delta(delta),
            TurnEvent::Program(slug) => Work::Program(slug),
        };
        feed.send(work);
    })
    .await;
    feed.send(Work::Finished(
        finished
            .map(|finished| (finished.reply, finished.usage))
            .map_err(|failure| failure.reason),
    ));
}

/// The draw loop. `trace` is the file the session records to when the
/// command line named one.
async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    trace: Option<&Path>,
) -> io::Result<()> {
    let ladder = Ladder::from_environment();
    let mut app = App {
        editor: Editor::new(),
        lines: Scrollback::new(wrap_line),
        pending: String::new(),
        busy: false,
        status: "ready".to_string(),
        tokens: String::new(),
        queued: None,
        scroll: 0,
        tick: 0,
        busy_since: None,
        verbose: false,
    };

    let (tx, mut inbox) = events::channel::<Work>();
    // The agent moves to its own task for each turn; the feed returns
    // each phase.
    // An environment that names two doors ends the session here. Picking
    // one of them quietly would put the wrong door in the trace and in
    // whatever the session was run to measure.
    let opened = match trace {
        Some(path) => Agent::recording_to(path),
        None => Agent::from_env(),
    };
    let mut agent_slot = Some(opened.map_err(io::Error::other)?);
    let mut turn: Option<tokio::task::JoinHandle<Agent>> = None;
    // The door's model name rides the composer's location rail, or the
    // door's own name when the model is not known until a worker answers.
    let model = agent_slot.as_ref().unwrap().label().to_string();
    // Where this conversation is being written down, so nobody has to guess.
    if let Some(agent) = agent_slot.as_ref() {
        match (agent.trace_path(), agent.trace_error()) {
            (Some(path), _) => app.push_detail("  ", format!("trace → {}", path.display())),
            (None, Some(error)) => app.push_detail("  ", format!("no trace — {error}")),
            (None, None) => app.push_detail("  ", "no trace — CODER_TRACE is off"),
        }
    }

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
            app.busy_since = None;
            app.status = "ready".to_string();
        }
        if turn.is_none()
            && let Some(draft) = app.queued.take()
        {
            start_turn(&mut app, &mut agent_slot, &mut turn, &tx, draft);
        }

        let dropped = inbox.take_dropped();
        if dropped > 0 {
            app.push(
                Intensity::Half,
                "  ",
                format!("preview fell behind — {dropped} bytes not drawn; the reply arrives whole"),
            );
        }
        draw(terminal, &ladder, &mut app, &model)?;

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
                            KeyCode::Char('v')
                                if key.modifiers.contains(KeyModifiers::ALT) =>
                            {
                                app.toggle_verbose();
                            }
                            _ => {
                                let width = terminal.size()?.width as usize;
                                if let ComposerAction::Submitted(draft) =
                                    handle_key(&mut app.editor, width, &key)
                                {
                                    if draft.is_empty() {
                                        continue;
                                    }
                                    // Slash commands act on the terminal
                                    // itself; they never reach the agent.
                                    if let Some(command) = draft.strip_prefix('/') {
                                        match command.trim() {
                                            "verbose" | "v" => app.toggle_verbose(),
                                            other => app.push(
                                                Intensity::Half,
                                                "  ",
                                                format!(
                                                    "unknown command /{other} — /verbose toggles detail"
                                                ),
                                            ),
                                        }
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
            Some(work) = inbox.recv() => {
                // The rest of the burst lands in this frame too, adjacent
                // deltas already merged.
                app.apply(work);
                for work in inbox.drain() {
                    app.apply(work);
                }
            }
        }
    }
    // The session closes its own trace, so the document says the session
    // ended rather than that it was cut off.
    if let Some(agent) = agent_slot.as_mut() {
        agent.finish_trace();
    }
    Ok(())
}

impl App {
    /// Folds one report from the turn into the transcript and the rails.
    fn apply(&mut self, work: Work) {
        match work {
            Work::Classified(Classified::Judged(verdict)) => {
                self.show_verdict(&verdict);
                self.status = match verdict.route {
                    Route::Respond | Route::Clarify => "generating".to_string(),
                    Route::End | Route::Halt(_) => "ready".to_string(),
                };
            }
            Work::Classified(Classified::Skipped(note)) => {
                self.push_detail("  ", note);
                self.status = "generating".to_string();
            }
            Work::Judgment(line) => {
                self.push_detail("  ", format!("classify → {line}"));
            }
            Work::Shell(event) => {
                // The plan's JSON streamed into pending; the $ lines
                // replace it.
                self.pending.clear();
                match event {
                    ShellEvent::Proposed(proposal) => {
                        self.push(Intensity::Half, "  ", format!("$ {}", proposal.command));
                        if !proposal.why.is_empty() {
                            self.push_detail("    ", proposal.why);
                        }
                    }
                    ShellEvent::Ran(outcome) => {
                        self.push_detail("    ", outcome.line());
                    }
                    ShellEvent::Verdict(line) => {
                        self.push_detail("  ", format!("shell → {line}"));
                    }
                }
            }
            Work::Program(slug) => {
                self.push_detail("  ", format!("program → {slug}"));
                self.status = format!("running {slug}");
            }
            Work::Delta(delta) => self.pending.push_str(&delta),
            Work::Finished(Ok((text, usage))) => {
                // The reply lays out as markdown lines; each is a
                // scrollback row.
                for rendered in markdown::render(&text) {
                    self.push_rendered(rendered);
                }
                self.pending.clear();
                if let Some(usage) = usage {
                    self.tokens = format!("{}/{}", usage.input_tokens, usage.output_tokens);
                }
            }
            Work::Finished(Err(why)) => {
                self.pending.clear();
                self.push_loud(why);
            }
        }
    }
}

/// Pushes the user turn into the scrollback and hands the draft to the
/// agent on its own task.
fn start_turn(
    app: &mut App,
    agent_slot: &mut Option<Agent>,
    turn: &mut Option<tokio::task::JoinHandle<Agent>>,
    tx: &Feed<Work>,
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
    app.busy_since = Some(Instant::now());
    app.status = "classifying".to_string();
    app.pending.clear();
    let tx = tx.clone();
    *turn = Some(tokio::spawn(async move {
        work_turn(&mut agent, draft, &tx).await;
        agent
    }));
}

/// One frame: scrollback above, the composer at the foot. `model` is the
/// door's name, drawn on the location rail.
fn draw(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ladder: &Ladder,
    app: &mut App,
    model: &str,
) -> io::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        let status = if app.verbose {
            format!("{} · v", app.status)
        } else {
            app.status.clone()
        };
        let mut composer = Composer::new(&mut app.editor, *ladder)
            .prompt(coder_terminal::PROMPT)
            .status(&status)
            .location(model)
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

        // The scrollback draws newest-at-bottom. Each settled line wraps on
        // word boundaries at the row width, its continuation rows hanging
        // under the prefix, and the wrap is cached until the width changes;
        // the pending reply and the working line wrap afresh each frame.
        let shown = log_area.height as usize;
        let width = usize::from(area.width);
        let verbose = app.verbose;
        let mut live: Vec<Row> = Vec::new();
        // The streaming reply renders live through the same markdown path —
        // unless it is shaping up as a shell plan: a reply that opens as a
        // JSON object or a code fence is the plan's wire format, and the
        // $ command lines replace it when the proposals land.
        if app.scroll == 0 && !app.pending.is_empty() && !planish(&app.pending) {
            for rendered in markdown::render(&app.pending) {
                expand(
                    &mut live,
                    rendered.intensity,
                    false,
                    "  ",
                    &rendered.marked,
                    rendered.hang,
                    width,
                );
            }
        }
        // A turn in flight leaves the working line at the transcript's
        // foot, under whatever is already there — the one place the
        // spinner lives.
        if app.busy {
            let elapsed = app.busy_since.map_or(0, |since| since.elapsed().as_secs());
            let clock = if elapsed >= 60 {
                format!("{}m {}s", elapsed / 60, elapsed % 60)
            } else {
                format!("{elapsed}s")
            };
            expand(
                &mut live,
                Intensity::Half,
                false,
                "  ",
                &Marked::plain(format!("{} working ({clock})", frame_for(app.tick))),
                0,
                width,
            );
        }
        let mut rows: Vec<&Row> = app.lines.rows(width, |line| verbose || !line.detail);
        rows.extend(live.iter());
        let end = rows.len().saturating_sub(app.scroll);
        let start = end.saturating_sub(shown);
        for (offset, row) in rows[start..end].iter().enumerate() {
            let base = ladder.style(row.intensity);
            let spans: Vec<ratatui::text::Span> = row
                .spans
                .iter()
                .map(|(text, marks)| {
                    ratatui::text::Span::styled(
                        text.clone(),
                        marked_style(base, ladder, marks, row.loud),
                    )
                })
                .collect();
            buf.set_line(
                log_area.left() + 1,
                log_area.top() + offset as u16,
                &ratatui::text::Line::from(spans),
                log_area.width.saturating_sub(1),
            );
        }

        let caret = composer.render(box_area, buf);
        frame.set_cursor_position(caret);
    })?;
    Ok(())
}

/// The style one marked span draws at: `base` lifted by the span's marks.
/// Code burns at full amber so it stands out of prose; bold, italic,
/// strike, and links take their modifiers; `loud` underlines the row.
fn marked_style(base: Style, ladder: &Ladder, marks: &Marks, loud: bool) -> Style {
    let mut style = if marks.code {
        ladder.style(Intensity::Full)
    } else {
        base
    };
    if marks.bold {
        style = style.add_modifier(ratatui::style::Modifier::BOLD);
    }
    if marks.italic {
        style = style.add_modifier(ratatui::style::Modifier::ITALIC);
    }
    if marks.strike {
        style = style.add_modifier(ratatui::style::Modifier::CROSSED_OUT);
    }
    if marks.link.is_some() || marks.image.is_some() || loud {
        style = style.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }
    style
}

/// Whether the text streaming in is shaping up as a shell plan: the plan's
/// wire format is a reply that opens as a JSON object or a code fence, so
/// those hide while they stream — the `$` command lines replace them.
fn planish(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('{') || text.starts_with("```")
}
