//! Asking Coder One from the Runs pane: `?` opens a composer, and the
//! question goes to `coder-one ask` with what the pane shows: the selected
//! or open run, the active filter, and the order.
//!
//! The pane stays a reader. It runs `coder-one ask --events` as a child
//! process through `supervise`, on a thread of its own, and reads the
//! child's events as they arrive: each progress line, then the record with
//! the answer, its claims, and the citation check's marks. The answer is
//! drawn the way the Coder terminal draws a reply, and the runs its claims
//! cite are listed under it: `↑↓` chooses one and `Enter` opens it. The
//! proposals the answer carries follow the runs, each with its status:
//! with the cursor on one, `a` approves it and `x` rejects it, as `gym
//! coder proposals approve|reject` does. `Esc` goes back to the list; `?`
//! and an empty question shows the last answer again.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use coder_terminal::{Intensity, frame, markdown, rail, wrap_rows};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::Value;

use super::{Key, Open, Order, Pane, Tab};
use crate::runs_story::Detail;

/// How long an ask may run before the pane's supervisor ends it.
pub const DEADLINE: Duration = Duration::from_secs(600);

/// What the child reported.
#[derive(Clone, Debug)]
pub(super) enum Event {
    Line(String),
    Record(Value),
    /// The child ended: how, and what it printed on standard error.
    Ended(String, String),
}

/// One question and what came back.
pub(super) struct Asking {
    pub(super) question: String,
    pub(super) context: Vec<String>,
    started: Instant,
    pub(super) lines: Vec<String>,
    pub(super) record: Option<Value>,
    pub(super) ended: Option<(String, String)>,
    events: Option<Receiver<Event>>,
    /// The cited run, or after the runs the proposal, the arrows are on.
    pub(super) cursor: usize,
    scroll: Cell<usize>,
    /// Each proposal's status, read when the record arrives and after a
    /// decision.
    pub(super) statuses: Vec<String>,
    /// What the last decision did.
    pub(super) notice: Option<String>,
}

impl Asking {
    /// Whether the child is still running.
    pub(super) fn running(&self) -> bool {
        self.events.is_some()
    }

    /// Every run the claims cite, in the order they're first cited.
    pub(super) fn cited(&self) -> Vec<String> {
        let mut runs: Vec<String> = Vec::new();
        let Some(record) = &self.record else {
            return runs;
        };
        for claim in record["claims"].as_array().into_iter().flatten() {
            let named = claim["runs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .chain(
                    claim["steps"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|s| s["run"].as_str()),
                );
            for run in named {
                if !runs.iter().any(|r| r == run) {
                    runs.push(run.to_owned());
                }
            }
        }
        runs
    }

    /// The proposals the answer carries: ID and title, and the directory
    /// that holds them.
    pub(super) fn proposals(&self) -> Vec<(String, String, Option<PathBuf>)> {
        let Some(record) = &self.record else {
            return Vec::new();
        };
        record["proposals"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                (
                    p["id"].as_str().unwrap_or("?").to_owned(),
                    format!(
                        "[{}] {}",
                        p["kind"].as_str().unwrap_or("?"),
                        p["title"].as_str().unwrap_or("")
                    ),
                    p["dir"]
                        .as_str()
                        .and_then(|dir| PathBuf::from(dir).parent().map(PathBuf::from)),
                )
            })
            .collect()
    }

    /// Reads each proposal's status from its directory.
    fn refresh(&mut self) {
        self.statuses = self
            .proposals()
            .iter()
            .map(|(id, _, root)| {
                root.as_ref()
                    .and_then(|root| {
                        crate::coder_proposals::load(root)
                            .into_iter()
                            .find(|entry| entry.id() == id)
                    })
                    .map_or_else(|| "unrecorded".to_owned(), |e| e.status().to_owned())
            })
            .collect();
    }

    /// Approves or rejects the proposal under the cursor.
    fn decide(&mut self, verdict: &str) {
        let at = self.cursor.checked_sub(self.cited().len());
        let proposals = self.proposals();
        let Some((id, _, root)) = at.and_then(|at| proposals.get(at)) else {
            self.notice = Some("Move ↓ to a proposal first.".to_owned());
            return;
        };
        let Some(root) = root else {
            self.notice = Some(format!("{id} wasn't recorded, so it can't be decided."));
            return;
        };
        self.notice = Some(
            match crate::coder_proposals::decide(
                root,
                id,
                verdict,
                "",
                &crate::coder_proposals::who(),
            ) {
                Ok(_) if verdict == "approved" => {
                    format!("{id} approved. Measure it with `coder-one proposal run {id}`.")
                }
                Ok(_) => format!("{id} rejected."),
                Err(why) => why,
            },
        );
        self.refresh();
    }
}

/// The ask state the pane holds.
pub(super) struct Asker {
    /// `coder-one`, when it was found.
    pub(super) program: Option<PathBuf>,
    /// The question being typed.
    pub(super) composer: Option<String>,
    pub(super) asking: Option<Asking>,
    /// Whether the answer view is showing.
    pub(super) showing: bool,
}

impl Asker {
    pub(super) fn found() -> Self {
        Asker {
            program: find_coder_one(),
            composer: None,
            asking: None,
            showing: false,
        }
    }
}

/// `coder-one`: `$CODER_ONE_BIN`, the one beside this binary, or the one
/// on `PATH`.
#[must_use]
pub fn find_coder_one() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CODER_ONE_BIN").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("coder-one")))
        .filter(|path| path.is_file())
        .or_else(|| {
            std::env::var_os("PATH").and_then(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join("coder-one"))
                    .find(|candidate| candidate.is_file())
            })
        })
}

/// Runs `program args` under `supervise` on a thread of its own, and
/// sends each event it prints as it arrives.
fn spawn(program: PathBuf, args: Vec<String>) -> Receiver<Event> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = sender.send(Event::Ended(format!("no runtime: {error}"), String::new()));
                return;
            }
        };
        runtime.block_on(async move {
            let mut command = std::process::Command::new(&program);
            command.args(&args);
            let live = supervise::Job::from_command(command)
                .bounded(supervise::Limits::within(DEADLINE).keeping(4 * 1024 * 1024))
                .start(supervise::live::Input::Null);
            let live = match live {
                Ok(live) => live,
                Err(error) => {
                    let _ = sender.send(Event::Ended(
                        format!("couldn't start {}: {error}", program.display()),
                        String::new(),
                    ));
                    return;
                }
            };
            let mut pending: Vec<u8> = Vec::new();
            let deliver = |bytes: &[u8], pending: &mut Vec<u8>| {
                pending.extend_from_slice(bytes);
                while let Some(end) = pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = pending.drain(..=end).collect();
                    let line = String::from_utf8_lossy(&line).trim().to_owned();
                    let event = match serde_json::from_str::<Value>(&line) {
                        Ok(value) if value["event"] == "answer" => {
                            Event::Record(value["record"].clone())
                        }
                        Ok(value) if value["event"] == "progress" => {
                            Event::Line(value["text"].as_str().unwrap_or_default().to_owned())
                        }
                        _ if line.is_empty() => continue,
                        _ => Event::Line(line),
                    };
                    let _ = sender.send(event);
                }
            };
            while !live.finished() {
                let delivery = live.take();
                deliver(&delivery.bytes, &mut pending);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let stopped = live.wait().await;
            deliver(&stopped.rest.bytes, &mut pending);
            deliver(b"\n", &mut pending);
            let _ = sender.send(Event::Ended(
                stopped.ending.to_string(),
                stopped.stderr.text.trim().to_owned(),
            ));
        });
    });
    receiver
}

impl Pane {
    /// The same pane asking `program` instead of the `coder-one` it found.
    #[must_use]
    pub fn with_coder_one(mut self, program: Option<PathBuf>) -> Self {
        self.asker.program = program;
        self
    }

    /// Whether an ask is running, so the terminal reads its events often.
    #[must_use]
    pub fn asking(&self) -> bool {
        self.asker.asking.as_ref().is_some_and(Asking::running)
    }

    /// Whether the ask composer is open.
    #[must_use]
    pub fn composing_ask(&self) -> bool {
        self.asker.composer.is_some()
    }

    /// Takes the running ask's events. Returns whether any arrived.
    pub fn poll_ask(&mut self) -> bool {
        let Some(asking) = &mut self.asker.asking else {
            return false;
        };
        let Some(events) = &asking.events else {
            return false;
        };
        let mut changed = false;
        let mut recorded = false;
        loop {
            match events.try_recv() {
                Ok(Event::Line(line)) => asking.lines.push(line),
                Ok(Event::Record(record)) => {
                    asking.record = Some(record);
                    recorded = true;
                }
                Ok(Event::Ended(ending, stderr)) => {
                    asking.ended = Some((ending, stderr));
                    asking.events = None;
                    changed = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    asking.events = None;
                    changed = true;
                    break;
                }
            }
            changed = true;
        }
        if recorded {
            asking.refresh();
        }
        changed
    }

    /// What the question is sent with: the run in view, the filter, the
    /// order, and the open tab.
    pub(super) fn ask_context(&self) -> (Option<String>, Vec<String>) {
        let mut context = Vec::new();
        let run = match &self.open {
            Some(open) => {
                if open.tab == Tab::Transcript && !open.blocks().is_empty() {
                    context.push(format!(
                        "the operator is reading step {} of this run's transcript",
                        open.selected.min(open.blocks().len() - 1) + 1
                    ));
                }
                Some(open.id.clone())
            }
            None => self.selected_run().map(crate::runs::Run::id),
        };
        if let Some(filter) = self.filter.describe() {
            context.push(format!("the list's filter: {filter}"));
        }
        context.push(format!(
            "the list's order: {}",
            match self.order() {
                Order::Newest => "newest first",
                Order::Learning => "most worth learning from first",
            }
        ));
        (run, context)
    }

    /// Sends `question` to `coder-one ask`.
    fn ask(&mut self, question: String) {
        let (run, context) = self.ask_context();
        let Some(program) = self.asker.program.clone() else {
            self.asker.asking = Some(Asking {
                question,
                context,
                started: Instant::now(),
                lines: Vec::new(),
                record: None,
                ended: Some((
                    "coder-one isn't built: run `cargo build -p coder-one`, or set CODER_ONE_BIN"
                        .to_owned(),
                    String::new(),
                )),
                events: None,
                cursor: 0,
                scroll: Cell::new(0),
                statuses: Vec::new(),
                notice: None,
            });
            self.asker.showing = true;
            return;
        };
        let mut args = vec!["ask".to_owned(), question.clone(), "--events".to_owned()];
        if let Some(run) = &run {
            args.extend(["--run".to_owned(), run.clone()]);
        }
        for line in &context {
            args.extend(["--context".to_owned(), line.clone()]);
        }
        self.asker.asking = Some(Asking {
            question,
            context: run
                .iter()
                .map(|run| format!("selected run: {run}"))
                .chain(context)
                .collect(),
            started: Instant::now(),
            lines: Vec::new(),
            record: None,
            ended: None,
            events: Some(spawn(program, args)),
            cursor: 0,
            scroll: Cell::new(0),
            statuses: Vec::new(),
            notice: None,
        });
        self.asker.showing = true;
        self.open = None;
    }

    /// Handles the ask's keys: `?`, the composer, and the answer view.
    /// Returns whether it took the key.
    pub(super) fn ask_key(&mut self, key: Key) -> bool {
        if let Some(draft) = &mut self.asker.composer {
            match key {
                Key::Char(c) => draft.push(c),
                Key::Backspace => {
                    draft.pop();
                }
                Key::Back => self.asker.composer = None,
                Key::Enter => {
                    let question = draft.trim().to_owned();
                    self.asker.composer = None;
                    if question.is_empty() {
                        self.asker.showing = self.asker.asking.is_some();
                        if self.asker.showing {
                            self.open = None;
                        }
                    } else {
                        self.ask(question);
                    }
                }
                _ => {}
            }
            return true;
        }
        if key == Key::Char('?') && self.typing.is_none() && !self.composing() {
            self.asker.composer = Some(String::new());
            return true;
        }
        if !self.asker.showing || self.open.is_some() {
            return false;
        }
        let Some(asking) = &mut self.asker.asking else {
            self.asker.showing = false;
            return false;
        };
        let cited = asking.cited();
        let items = cited.len() + asking.proposals().len();
        match key {
            Key::Back => self.asker.showing = false,
            Key::Up | Key::Char('k') => asking.cursor = asking.cursor.saturating_sub(1),
            Key::Down | Key::Char('j') => {
                asking.cursor = (asking.cursor + 1).min(items.saturating_sub(1));
            }
            Key::Char('a') => asking.decide("approved"),
            Key::Char('x') => asking.decide("rejected"),
            Key::PageUp => asking.scroll.set(asking.scroll.get().saturating_sub(10)),
            Key::PageDown => asking.scroll.set(asking.scroll.get() + 10),
            Key::Enter => {
                if let Some(name) = cited.get(asking.cursor)
                    && let Some(run) = self
                        .catalog
                        .runs
                        .iter()
                        .find(|run| run.id() == *name || run.job == *name)
                {
                    self.open = Some(Open::new(Detail::load(run), Tab::Summary));
                }
            }
            Key::Char('q') => return false,
            _ => {}
        }
        true
    }

    /// The answer view's rows: the question, progress, the answer, each
    /// claim with its mark, the cost, and the cited runs. The second value
    /// is the row of the cited run under the cursor.
    fn ask_rows(
        &self,
        asking: &Asking,
        width: usize,
    ) -> (Vec<(String, Intensity, bool)>, Option<usize>) {
        let mut rows: Vec<(String, Intensity, bool)> = Vec::new();
        let push = |text: &str, intensity: Intensity, rows: &mut Vec<(String, Intensity, bool)>| {
            for range in wrap_rows(text, width.max(8)) {
                rows.push((text[range].to_owned(), intensity, false));
            }
        };
        push(
            &format!("You ▸ {}", asking.question),
            Intensity::Full,
            &mut rows,
        );
        for line in &asking.context {
            push(&format!("  with {line}"), Intensity::Quarter, &mut rows);
        }
        rows.push((String::new(), Intensity::Half, false));
        let shown = if asking.record.is_some() { 0 } else { 12 };
        let skip = asking.lines.len().saturating_sub(shown);
        if skip > 0 && asking.record.is_some() {
            push(
                &format!("Coder One read the Gym in {} steps.", asking.lines.len()),
                Intensity::Quarter,
                &mut rows,
            );
        }
        for line in asking.lines.iter().skip(skip) {
            push(&format!("  {line}"), Intensity::Half, &mut rows);
        }
        if asking.running() {
            push(
                &format!(
                    "  {} asking… {:.0}s",
                    coder_terminal::frame_for(self.tick),
                    asking.started.elapsed().as_secs_f64()
                ),
                Intensity::ThreeQuarters,
                &mut rows,
            );
        }
        let mut selected_row = None;
        match &asking.record {
            Some(record) => {
                rows.push((String::new(), Intensity::Half, false));
                match record["answer"].as_str() {
                    Some(answer) => {
                        for rendered in markdown::render(answer) {
                            let text =
                                format!("{}{}", " ".repeat(rendered.hang), rendered.marked.text);
                            push(&text, rendered.intensity, &mut rows);
                        }
                    }
                    None => push(
                        &format!("No answer: {}", record["status"].as_str().unwrap_or("?")),
                        Intensity::Full,
                        &mut rows,
                    ),
                }
                rows.push((String::new(), Intensity::Half, false));
                for (index, claim) in record["claims"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    let verified = claim["verified"] == true;
                    push(
                        &format!(
                            "{} {}. {}",
                            if verified { '✓' } else { '?' },
                            index + 1,
                            claim["claim"].as_str().unwrap_or("")
                        ),
                        if verified {
                            Intensity::ThreeQuarters
                        } else {
                            Intensity::Full
                        },
                        &mut rows,
                    );
                    for problem in claim["problems"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        push(
                            &format!("     unverified: {problem}"),
                            Intensity::Half,
                            &mut rows,
                        );
                    }
                }
                if let Some(change) = record["proposed_change"].as_str() {
                    rows.push((String::new(), Intensity::Half, false));
                    push(
                        &format!("Proposed, for you to decide: {change}"),
                        Intensity::ThreeQuarters,
                        &mut rows,
                    );
                }
                let citations = &record["citations"];
                let cost = &record["cost"];
                rows.push((String::new(), Intensity::Half, false));
                push(
                    &format!(
                        "{} of {} citations valid · {} of {} claims verified · {} · {:.1}s",
                        citations["valid_citations"],
                        citations["citations"],
                        citations["verified"],
                        citations["claims"],
                        cost["usd"]
                            .as_f64()
                            .map_or_else(|| "cost unknown".to_owned(), |usd| format!("${usd:.4}")),
                        record["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0
                    ),
                    Intensity::Half,
                    &mut rows,
                );
                let cited = asking.cited();
                if !cited.is_empty() {
                    rows.push((String::new(), Intensity::Half, false));
                    rows.push((
                        "Cited runs: ↑↓ choose, enter opens".to_owned(),
                        Intensity::Half,
                        false,
                    ));
                    for (index, run) in cited.iter().enumerate() {
                        let here = index == asking.cursor;
                        let known = self
                            .catalog
                            .runs
                            .iter()
                            .any(|r| r.id() == *run || r.job == *run);
                        if here {
                            selected_row = Some(rows.len());
                        }
                        rows.push((
                            format!(
                                "{} {run}{}",
                                if here { '▸' } else { ' ' },
                                if known { "" } else { "  (not in this list)" }
                            ),
                            if here {
                                Intensity::Full
                            } else {
                                Intensity::ThreeQuarters
                            },
                            here,
                        ));
                    }
                }
                let proposals = asking.proposals();
                if !proposals.is_empty() {
                    rows.push((String::new(), Intensity::Half, false));
                    rows.push((
                        "Proposals: ↑↓ choose, a approves, x rejects; nothing runs until approved"
                            .to_owned(),
                        Intensity::Half,
                        false,
                    ));
                    for (index, (id, title, _)) in proposals.iter().enumerate() {
                        let here = cited.len() + index == asking.cursor;
                        if here {
                            selected_row = Some(rows.len());
                        }
                        let status = asking.statuses.get(index).map_or("?", String::as_str);
                        rows.push((
                            format!(
                                "{} {id} · {status} · {}",
                                if here { '▸' } else { ' ' },
                                crate::runs::clip_words(title, width.saturating_sub(40).max(20))
                            ),
                            if here {
                                Intensity::Full
                            } else {
                                Intensity::ThreeQuarters
                            },
                            here,
                        ));
                    }
                }
                if let Some(notice) = &asking.notice {
                    rows.push((String::new(), Intensity::Half, false));
                    push(notice, Intensity::Full, &mut rows);
                }
            }
            None => {
                if let Some((ending, stderr)) = &asking.ended {
                    rows.push((String::new(), Intensity::Half, false));
                    push(
                        &format!("The ask ended without an answer: {ending}"),
                        Intensity::Full,
                        &mut rows,
                    );
                    for line in stderr
                        .lines()
                        .rev()
                        .take(6)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                    {
                        push(&format!("  {line}"), Intensity::Half, &mut rows);
                    }
                }
            }
        }
        (rows, selected_row)
    }

    /// Draws the answer view.
    pub(super) fn render_ask(&self, area: Rect, buf: &mut Buffer) {
        let Some(asking) = &self.asker.asking else {
            return;
        };
        let right = if asking.running() {
            "asking Coder One…".to_owned()
        } else {
            asking
                .record
                .as_ref()
                .and_then(|r| r["executor"].as_str())
                .map_or_else(String::new, |executor| format!("answered by {executor}"))
        };
        self.header(
            area,
            buf,
            &[("Ask Coder One".to_owned(), Intensity::Full)],
            &right,
        );
        let inner = self.framed(
            area,
            buf,
            ("Answer", "read from the Gym; citations checked by code"),
            (
                "↑↓ choose · enter open · a approve · x reject · pgup pgdn scroll · ? ask again · esc back · q quit",
                "↑↓ enter a x ? esc q",
            ),
        );
        let width = usize::from(inner.width);
        let (rows, selected) = self.ask_rows(asking, width);
        let height = usize::from(inner.height);
        let mut scroll = asking.scroll.get();
        let bottom = rows.len().saturating_sub(height);
        if asking.running() {
            scroll = bottom;
        } else if let Some(row) = selected {
            if row < scroll {
                scroll = row;
            } else if row >= scroll + height {
                scroll = row + 1 - height;
            }
        }
        scroll = scroll.min(bottom);
        asking.scroll.set(scroll);
        for (offset, (text, intensity, here)) in rows.iter().skip(scroll).take(height).enumerate() {
            let y = inner.top() + offset as u16;
            let mut style = self.style(*intensity);
            if *here {
                style = style.bg(self.ladder.selection());
            }
            buf.set_stringn(inner.left(), y, text, width, style);
        }
    }

    /// Draws the ask composer over the lower part of `area`.
    pub(super) fn render_ask_composer(&self, area: Rect, buf: &mut Buffer) {
        let Some(draft) = &self.asker.composer else {
            return;
        };
        let height: u16 = 6;
        if area.height < height + 2 || area.width < 40 {
            return;
        }
        let boxed = Rect::new(
            area.left() + 1,
            area.bottom() - height - 1,
            area.width - 2,
            height,
        );
        for y in boxed.top()..boxed.bottom() {
            for x in boxed.left()..boxed.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(self.style(Intensity::Half));
            }
        }
        frame(boxed, buf, self.style(Intensity::ThreeQuarters));
        let (run, _) = self.ask_context();
        let about = run.map_or_else(
            || "the runs".to_owned(),
            |run| crate::runs::clip_words(&run, 60),
        );
        rail(
            boxed,
            buf,
            0,
            Some(("Ask Coder One", self.style(Intensity::Full))),
            Some((&format!("with {about}"), self.style(Intensity::Half))),
        );
        rail(
            boxed,
            buf,
            boxed.height - 1,
            Some((
                if self.asker.asking.is_some() {
                    "enter asks · empty shows the last answer · esc cancels"
                } else {
                    "enter asks · esc cancels"
                },
                self.style(Intensity::ThreeQuarters),
            )),
            None,
        );
        let inner = Rect::new(
            boxed.left() + 2,
            boxed.top() + 1,
            boxed.width.saturating_sub(4),
            boxed.height.saturating_sub(2),
        );
        let width = usize::from(inner.width);
        let text = format!("› {draft}▏");
        for (offset, range) in wrap_rows(&text, width)
            .into_iter()
            .take(usize::from(inner.height))
            .enumerate()
        {
            buf.set_stringn(
                inner.left(),
                inner.top() + offset as u16,
                &text[range],
                width,
                self.style(Intensity::Full),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::pane;
    use super::*;

    fn type_text(pane: &mut Pane, text: &str) {
        for c in text.chars() {
            pane.key(Key::Char(c));
        }
    }

    /// A stand-in `coder-one` that prints its arguments as progress, then
    /// a record citing `run` and a run that doesn't exist.
    fn fake(dir: &std::path::Path, run: &str) -> PathBuf {
        let record = serde_json::json!({
            "event": "answer",
            "record": {
                "executor": "luna",
                "status": "answered",
                "answer": "The run claimed success it **didn't** earn.",
                "claims": [
                    {"claim": "It reported success.", "runs": [run], "steps": [], "verified": true, "problems": []},
                    {"claim": "Another agrees.", "runs": ["tb4--nowhere/x__1"], "steps": [], "verified": false, "problems": ["the Gym has no run tb4--nowhere/x__1"]},
                ],
                "citations": {"citations": 2, "valid_citations": 1, "claims": 2, "verified": 1},
                "cost": {"usd": 0.0085},
                "milliseconds": 50_100,
            }
        });
        let saved = dir.join("record.json");
        std::fs::write(&saved, format!("{record}\n")).unwrap();
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '{{\"event\":\"progress\",\"text\":\"arg %s\"}}\\n' \"$a\"; done\nsleep 0.2\ncat '{}'\n",
            saved.display()
        );
        let path = dir.join("coder-one");
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn a_question_goes_to_coder_one_with_the_selection_and_its_cited_runs_open() {
        let (_dir, mut pane) = pane();
        // Only passed runs: the selection is the passed Coq run.
        pane.key(Key::Char('o'));
        let run = pane.selected_run().unwrap().id();
        let scripts = tempfile::tempdir().unwrap();
        let mut pane = pane.with_coder_one(Some(fake(scripts.path(), &run)));
        assert!(pane.key(Key::Char('?')) == super::super::Reply::Handled);
        assert!(pane.composing_ask());
        type_text(&mut pane, "why does this one rank?");
        let text = pane.to_text(120, 30);
        assert!(text.contains("Ask Coder One"), "{text}");
        assert!(text.contains("› why does this one rank?"), "{text}");
        pane.key(Key::Enter);
        assert!(pane.asking());
        let started = Instant::now();
        while pane.asking() && started.elapsed() < Duration::from_secs(10) {
            pane.poll_ask();
            std::thread::sleep(Duration::from_millis(20));
        }
        pane.poll_ask();
        let asking = pane.asker.asking.as_ref().unwrap();
        let args = asking.lines.join("\n");
        assert!(args.contains("arg why does this one rank?"), "{args}");
        assert!(args.contains("arg --events"), "{args}");
        assert!(args.contains(&format!("arg {run}")), "{args}");
        assert!(args.contains("arg the list's filter: passed"), "{args}");
        assert!(
            args.contains("arg the list's order: newest first"),
            "{args}"
        );
        let text = pane.to_text(120, 40);
        for needle in [
            "You ▸ why does this one rank?",
            "The run claimed success it didn't earn.",
            "✓ 1. It reported success.",
            "? 2. Another agrees.",
            "unverified: the Gym has no run tb4--nowhere/x__1",
            "1 of 2 citations valid",
            "Cited runs",
            "(not in this list)",
        ] {
            assert!(text.contains(needle), "{needle}:\n{text}");
        }
        // Enter opens the cited run; Esc comes back to the answer.
        pane.key(Key::Enter);
        assert!(pane.is_open());
        pane.key(Key::Back);
        assert!(!pane.is_open());
        assert!(pane.to_text(120, 40).contains("You ▸"));
        // Esc leaves the answer; `?` and an empty question shows it again.
        pane.key(Key::Back);
        assert!(!pane.to_text(120, 40).contains("You ▸"));
        pane.key(Key::Char('?'));
        pane.key(Key::Enter);
        assert!(pane.to_text(120, 40).contains("You ▸"));
    }

    #[test]
    fn a_proposal_in_the_answer_is_approved_with_a() {
        let (_dir, pane) = pane();
        let scripts = tempfile::tempdir().unwrap();
        let root = scripts.path().join("proposals");
        let id = "prop-1790000000000-1";
        std::fs::create_dir_all(root.join(id)).unwrap();
        let mut proposal = serde_json::json!({
            "schema": crate::coder_proposals::PROPOSAL_SCHEMA,
            "id": id,
            "ask": {"id": "ask-1790000000000", "question": "why?", "dir": "/a"},
            "created_at": "2026-09-23T00:00:00Z",
            "kind": "check",
            "title": "Run the behavior scenarios",
            "source_runs": [], "expected_tasks": [],
            "valid": true, "problems": [],
        });
        proposal["digest"] = serde_json::json!(crate::coder_proposals::digest(&proposal));
        std::fs::write(root.join(id).join("proposal.json"), proposal.to_string()).unwrap();
        let record = serde_json::json!({
            "event": "answer",
            "record": {
                "executor": "luna", "status": "answered", "answer": "A.", "claims": [],
                "proposals": [{"id": id, "kind": "check", "title": "Run the behavior scenarios", "valid": true, "dir": root.join(id).display().to_string()}],
                "citations": {"citations": 0, "valid_citations": 0, "claims": 0, "verified": 0},
                "cost": {"usd": 0.001}, "milliseconds": 1000,
            }
        });
        let saved = scripts.path().join("record.json");
        std::fs::write(&saved, format!("{record}\n")).unwrap();
        let path = scripts.path().join("coder-one");
        std::fs::write(&path, format!("#!/bin/sh\ncat '{}'\n", saved.display())).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        let mut pane = pane.with_coder_one(Some(path));
        pane.key(Key::Char('?'));
        type_text(&mut pane, "why?");
        pane.key(Key::Enter);
        let started = Instant::now();
        while pane.asking() && started.elapsed() < Duration::from_secs(10) {
            pane.poll_ask();
            std::thread::sleep(Duration::from_millis(20));
        }
        pane.poll_ask();
        let text = pane.to_text(140, 40);
        assert!(text.contains("a approves, x rejects"), "{text}");
        assert!(text.contains(&format!("{id} · proposed")), "{text}");
        pane.key(Key::Char('a'));
        let text = pane.to_text(140, 40);
        assert!(text.contains(&format!("{id} · approved")), "{text}");
        assert!(text.contains("coder-one proposal run"), "{text}");
        let decision: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(id).join("decision.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(decision["verdict"], "approved");
    }

    #[test]
    fn without_coder_one_the_answer_view_says_how_to_build_it() {
        let (_dir, pane) = pane();
        let mut pane = pane.with_coder_one(None);
        pane.key(Key::Char('?'));
        type_text(&mut pane, "why?");
        pane.key(Key::Enter);
        let text = pane.to_text(120, 30);
        assert!(text.contains("cargo build -p coder-one"), "{text}");
    }
}
