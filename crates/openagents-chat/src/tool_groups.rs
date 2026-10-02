//! A Coder run's tool calls, grouped the way Grok Build groups them
//! (#10117; `docs/coder/design/2026-10-01-tool-call-groups.md`).
//!
//! A run's consecutive commands, tool calls, and thoughts form a
//! [`Stretch`]. Within it, a run of calls that only look (read, search,
//! list, fetch) folds into one group whose label counts them by verb,
//! "Read 3 files, Searched 2 patterns", and claims the thoughts between
//! them. A call that acts (run, edit, delete, move, any other tool) stands
//! alone as one line: verb, target, and its result when it failed. A long
//! stretch of such lines shows its last [`MAX_VISIBLE`] and folds the rest
//! under one label ("Ran 6 commands"). Command output and file contents
//! show only when a surface expands.
//!
//! Every surface (OpenAgents Terminal, the desktop and phone Coder run
//! view, `openagents chat`'s text) groups with this one module, from the
//! typed [`Call`] on each step and the typed [`Output`] of a command, never
//! from a step's words.
//!
//! [`Output`]: crate::coder_events::Output

use crate::coder_events::{Call, CoderEvent, StepKind, Verb};

/// The most standalone lines a condensed stretch shows before it folds the
/// earlier ones under a label, as Grok Build's default
/// `group_max_visible`.
pub const MAX_VISIBLE: usize = 10;

/// One command or tool call as a stretch keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// The event that named the call: a surface's row key.
    pub seq: u64,
    pub call: Call,
    /// A command still waiting for its output.
    pub running: bool,
    /// The command's exit when it did not succeed: `exit 1`, `timed out`.
    pub status: Option<String>,
    /// What the call returned, bounded as the event carried it; shown only
    /// when a surface expands.
    pub output: String,
}

impl Shown {
    /// Whether the call failed: the agent said so, or the command exited
    /// other than 0.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.call.failed || self.status.is_some()
    }

    /// The verb a line leads with (`Read`, `Run`), or `None` for a tool
    /// the agent named in its own words.
    #[must_use]
    pub fn verb(&self) -> Option<&'static str> {
        Some(match self.call.verb {
            Verb::Read => "Read",
            Verb::Search => "Search",
            Verb::List => "List",
            Verb::Fetch => "Fetch",
            Verb::Edit => "Edit",
            Verb::Delete => "Delete",
            Verb::Move => "Move",
            Verb::Run => "Run",
            Verb::Other => return None,
        })
    }

    /// What the line names after its verb: a quoted pattern, a command's
    /// description or its first line, a path, a URL.
    #[must_use]
    pub fn target(&self) -> String {
        let target = self.call.target.trim();
        match self.call.verb {
            Verb::Search => format!("\"{target}\""),
            Verb::Run => match &self.call.about {
                Some(about) => about.clone(),
                None => target.lines().next().unwrap_or("").to_owned(),
            },
            _ => target.to_owned(),
        }
    }

    /// The result a condensed line ends with, only when there is one to
    /// see: `exit 1`, `timed out`, `failed`.
    #[must_use]
    pub fn result(&self) -> Option<String> {
        match (&self.status, self.call.failed) {
            (Some(status), _) => Some(status.clone()),
            (None, true) => Some("failed".to_owned()),
            (None, false) => None,
        }
    }

    /// The whole condensed line: "Run cargo test · exit 1".
    #[must_use]
    pub fn line(&self) -> String {
        let mut line = match self.verb() {
            Some(verb) => format!("{verb} {}", self.target()),
            None => self.target(),
        };
        if let Some(result) = self.result() {
            line.push_str(" · ");
            line.push_str(&result);
        }
        line
    }

    /// For a command, the command itself, as an expanded call shows it
    /// above its output.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        (self.call.verb == Verb::Run).then_some(self.call.target.as_str())
    }
}

/// One entry of a stretch: a call, or a thought between calls.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Call(Shown),
    Thought { seq: u64, text: String },
}

impl Entry {
    #[must_use]
    pub fn seq(&self) -> u64 {
        match self {
            Entry::Call(shown) => shown.seq,
            Entry::Thought { seq, .. } => *seq,
        }
    }
}

/// A group's or a fold's label, counted by verb in the order the verbs
/// first came: "Read 3 files, Searched 2 patterns", present tense while a
/// member runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub running: bool,
    pub failed: usize,
}

impl Label {
    /// The label with its failures: "Ran 6 commands · 1 failed".
    #[must_use]
    pub fn line(&self) -> String {
        if self.failed == 0 {
            self.text.clone()
        } else {
            format!("{} · {} failed", self.text, self.failed)
        }
    }
}

/// What a stretch shows, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Item<'a> {
    /// Consecutive looking calls and the thoughts among them, under one
    /// label. `key` is the first entry's event. Expanded, a surface lists
    /// `members` under the label.
    Group {
        key: u64,
        label: Label,
        members: Vec<&'a Entry>,
    },
    /// A call that stands alone.
    Call(&'a Shown),
    /// A thought outside any group.
    Thought { seq: u64, text: &'a str },
    /// The earlier standalone lines of a long stretch, folded under one
    /// label. Only a condensed stretch folds.
    More {
        key: u64,
        label: Label,
        hidden: Vec<Item<'a>>,
    },
}

/// A run's consecutive commands, tool calls, and thoughts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stretch {
    entries: Vec<Entry>,
}

impl Stretch {
    /// Takes `event` into the stretch, returning whether it belongs: a
    /// thought, a command, a tool call, what a call returned, a command's
    /// output. Anything else (a reply, a note, a switch, the turn's end)
    /// ends the stretch, and the caller shows it on its own. Progress
    /// neither belongs nor ends a stretch; the caller shows it apart.
    pub fn push(&mut self, seq: u64, event: &CoderEvent) -> bool {
        match event {
            CoderEvent::Step(step) => match step.kind {
                StepKind::Thinking => {
                    self.entries.push(Entry::Thought {
                        seq,
                        text: step.text.trim().to_owned(),
                    });
                    true
                }
                StepKind::Command | StepKind::ToolCall => {
                    let command = step.kind == StepKind::Command;
                    let call = step.call.clone().unwrap_or_else(|| Call {
                        verb: if command { Verb::Run } else { Verb::Other },
                        target: step.text.trim().to_owned(),
                        about: None,
                        failed: false,
                    });
                    self.entries.push(Entry::Call(Shown {
                        seq,
                        call,
                        running: command,
                        status: None,
                        output: String::new(),
                    }));
                    true
                }
                StepKind::Observation => match self.entries.last_mut() {
                    Some(Entry::Call(shown)) => {
                        // A command that waits gets its own output event;
                        // its summary line ("exit 0 in 1.0s") says less.
                        if !shown.running && shown.output.is_empty() {
                            shown.output = as_shown(&step.text);
                        }
                        true
                    }
                    _ => false,
                },
                StepKind::Message | StepKind::Reply | StepKind::Note => false,
            },
            CoderEvent::Output(output) => {
                let running = |shown: &&mut Shown| shown.running && shown.call.verb == Verb::Run;
                let mut calls = self
                    .entries
                    .iter_mut()
                    .rev()
                    .filter_map(|entry| match entry {
                        Entry::Call(shown) => Some(shown),
                        Entry::Thought { .. } => None,
                    });
                let found = {
                    let mut waiting: Vec<&mut Shown> = calls.by_ref().filter(running).collect();
                    let at = waiting
                        .iter()
                        .position(|shown| shown.call.target == output.command)
                        .or(if waiting.is_empty() {
                            None
                        } else {
                            Some(waiting.len() - 1)
                        });
                    at.map(|at| waiting.swap_remove(at))
                };
                let Some(shown) = found else {
                    return false;
                };
                shown.running = false;
                shown.status = match (output.timed_out, output.exit) {
                    (true, _) => Some("timed out".to_owned()),
                    (false, Some(0)) => None,
                    (false, Some(code)) => Some(format!("exit {code}")),
                    (false, None) => Some("stopped".to_owned()),
                };
                let mut text = as_shown(&output.text);
                if output.truncated {
                    text.push_str("\n…");
                }
                shown.output = text;
                true
            }
            _ => false,
        }
    }

    /// The turn ended: a command still waiting never gets its output.
    pub fn settle(&mut self) {
        for entry in &mut self.entries {
            if let Entry::Call(shown) = entry {
                shown.running = false;
            }
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The calls in the stretch.
    pub fn calls(&self) -> impl Iterator<Item = &Shown> {
        self.entries.iter().filter_map(|entry| match entry {
            Entry::Call(shown) => Some(shown),
            Entry::Thought { .. } => None,
        })
    }

    /// What the stretch shows. `fold` folds a long run of standalone
    /// lines (a condensed view); an expanded view passes `false`.
    #[must_use]
    pub fn items(&self, fold: bool) -> Vec<Item<'_>> {
        let mut items = Vec::new();
        let mut at = 0;
        while at < self.entries.len() {
            let mut end = at;
            let mut members = 0;
            while let Some(entry) = self.entries.get(end) {
                match entry {
                    Entry::Call(shown) if looks(shown.call.verb) => members += 1,
                    Entry::Thought { .. } => {}
                    Entry::Call(_) => break,
                }
                end += 1;
            }
            if members > 0 {
                let run: Vec<&Entry> = self.entries[at..end].iter().collect();
                items.push(Item::Group {
                    key: self.entries[at].seq(),
                    label: label(run.iter().filter_map(|entry| match entry {
                        Entry::Call(shown) => Some(shown),
                        Entry::Thought { .. } => None,
                    })),
                    members: run,
                });
                at = end;
                continue;
            }
            // Thoughts with no looking call after them stand alone, and so
            // does the acting call that ended the scan.
            let stop = end.max(at + 1);
            for entry in &self.entries[at..stop] {
                items.push(match entry {
                    Entry::Call(shown) => Item::Call(shown),
                    Entry::Thought { seq, text } => Item::Thought { seq: *seq, text },
                });
            }
            at = stop;
        }
        if fold { truncate(items) } else { items }
    }
}

/// A stretch for a log that only appends, such as `openagents chat`'s
/// text: each item prints once, when it can no longer change. A group
/// prints its label when something other than a looking call comes; a
/// call prints when it has its result; a thought prints when the next
/// thing shows it is not part of a group.
#[derive(Clone, Debug, Default)]
pub struct Stream {
    pending: Stretch,
    /// The last line printed was a call: what it returned may still come.
    called: bool,
}

impl Stream {
    /// Takes `event` when it is tool activity, returning the lines now
    /// final; `None` when it is not, after which the caller prints
    /// [`Stream::flush`]'s lines and then the event its own way. Progress
    /// and a status are neither: `Some` with nothing to print.
    pub fn push(&mut self, seq: u64, event: &CoderEvent) -> Option<Vec<String>> {
        if matches!(event, CoderEvent::Progress(_) | CoderEvent::Status(_)) {
            return Some(Vec::new());
        }
        if !self.pending.push(seq, event) {
            let returned = matches!(
                event,
                CoderEvent::Step(step) if step.kind == StepKind::Observation
            );
            if returned && self.called && self.pending.is_empty() {
                return Some(Vec::new());
            }
            self.called = false;
            return None;
        }
        let mut out = Vec::new();
        loop {
            let items = self.pending.items(false);
            let last = items.len() == 1;
            let Some(first) = items.first() else { break };
            let done = !last
                || matches!(first, Item::Call(shown) if !shown.running && !looks(shown.call.verb));
            if !done {
                break;
            }
            let (line, used) = print(first);
            self.called = !matches!(first, Item::Thought { .. });
            out.push(line);
            drop(items);
            self.pending.entries.drain(..used);
        }
        Some(out)
    }

    /// Everything still pending, as lines: the stretch ended.
    pub fn flush(&mut self) -> Vec<String> {
        let mut pending = std::mem::take(&mut self.pending);
        pending.settle();
        pending
            .items(false)
            .iter()
            .map(|item| print(item).0)
            .collect()
    }
}

/// One item as a log line, and how many entries it holds.
fn print(item: &Item<'_>) -> (String, usize) {
    match item {
        Item::Group { label, members, .. } => (format!("◈ {}", label.line()), members.len()),
        Item::Call(shown) => (format!("◆ {}", shown.line()), 1),
        Item::Thought { text, .. } => (format!("· {}", text.lines().next().unwrap_or("")), 1),
        Item::More { label, hidden, .. } => (format!("◈ {}", label.line()), hidden.len()),
    }
}

/// Output as a terminal would have left it: a line rewritten in place
/// with carriage returns (a progress bar) keeps its last state.
fn as_shown(text: &str) -> String {
    text.trim_end()
        .lines()
        .map(|line| {
            line.trim_end_matches('\r')
                .rsplit('\r')
                .next()
                .unwrap_or("")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether a verb only looks, and so folds into a group.
#[must_use]
pub fn looks(verb: Verb) -> bool {
    matches!(verb, Verb::Read | Verb::Search | Verb::List | Verb::Fetch)
}

/// The verb (past, present) and the noun (one, many) a label counts with.
fn words(verb: Verb) -> (&'static str, &'static str, &'static str, &'static str) {
    match verb {
        Verb::Read => ("Read", "Reading", "file", "files"),
        Verb::Search => ("Searched", "Searching", "pattern", "patterns"),
        Verb::List => ("Listed", "Listing", "dir", "dirs"),
        Verb::Fetch => ("Fetched", "Fetching", "website", "websites"),
        Verb::Edit => ("Edited", "Editing", "file", "files"),
        Verb::Delete => ("Deleted", "Deleting", "file", "files"),
        Verb::Move => ("Moved", "Moving", "file", "files"),
        Verb::Run => ("Ran", "Running", "command", "commands"),
        Verb::Other => ("Ran", "Running", "tool", "tools"),
    }
}

/// The label for `calls`, counted by verb in first-appearance order.
#[must_use]
pub fn label<'a>(calls: impl Iterator<Item = &'a Shown>) -> Label {
    let mut counts: Vec<(Verb, usize)> = Vec::new();
    let mut running = false;
    let mut failed = 0;
    for shown in calls {
        match counts.iter_mut().find(|(verb, _)| *verb == shown.call.verb) {
            Some((_, count)) => *count += 1,
            None => counts.push((shown.call.verb, 1)),
        }
        running |= shown.running;
        failed += usize::from(shown.failed());
    }
    let text = counts
        .iter()
        .map(|(verb, count)| {
            let (past, present, one, many) = words(*verb);
            format!(
                "{} {count} {}",
                if running { present } else { past },
                if *count == 1 { one } else { many }
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Label {
        text,
        running,
        failed,
    }
}

/// Folds each run of more than `MAX_VISIBLE + 1` standalone lines (calls
/// and thoughts; a group ends a run) to its last `MAX_VISIBLE`, the rest
/// under one label that counts the calls it hides.
fn truncate(items: Vec<Item<'_>>) -> Vec<Item<'_>> {
    let mut out = Vec::with_capacity(items.len());
    let mut run = Vec::new();
    for item in items {
        match item {
            Item::Group { .. } | Item::More { .. } => {
                fold(&mut run, &mut out);
                out.push(item);
            }
            Item::Call(_) | Item::Thought { .. } => run.push(item),
        }
    }
    fold(&mut run, &mut out);
    out
}

/// Moves `run` into `out`, folded when it is longer than `MAX_VISIBLE + 1`.
fn fold<'a>(run: &mut Vec<Item<'a>>, out: &mut Vec<Item<'a>>) {
    if run.len() <= MAX_VISIBLE + 1 {
        out.append(run);
        return;
    }
    let shown = run.split_off(run.len() - MAX_VISIBLE);
    let hidden = std::mem::take(run);
    let key = match hidden.first() {
        Some(Item::Call(shown)) => shown.seq,
        Some(Item::Thought { seq, .. }) => *seq,
        _ => 0,
    };
    let mut label = label(hidden.iter().filter_map(|item| match item {
        Item::Call(shown) => Some(*shown),
        _ => None,
    }));
    if label.text.is_empty() {
        label.text = format!("{} more", hidden.len());
    }
    out.push(Item::More { key, label, hidden });
    out.extend(shown);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coder_events::{Output, Step};

    fn call(verb: Verb, target: &str) -> CoderEvent {
        CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: if verb == Verb::Run {
                StepKind::Command
            } else {
                StepKind::ToolCall
            },
            source: "agent".into(),
            text: target.into(),
            call: Some(Call {
                verb,
                target: target.into(),
                about: None,
                failed: false,
            }),
        })
    }

    fn thought(text: &str) -> CoderEvent {
        CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: StepKind::Thinking,
            source: "agent".into(),
            text: text.into(),
            call: None,
        })
    }

    fn output(command: &str, exit: i32) -> CoderEvent {
        CoderEvent::Output(Output {
            turn: 1,
            step_id: 1,
            command: command.into(),
            exit: Some(exit),
            timed_out: false,
            seconds: 0.1,
            text: "out".into(),
            truncated: false,
        })
    }

    fn stretch(events: &[CoderEvent]) -> Stretch {
        let mut stretch = Stretch::default();
        for (seq, event) in events.iter().enumerate() {
            assert!(stretch.push(seq as u64 + 1, event), "{event:?}");
        }
        stretch
    }

    fn shape(items: &[Item<'_>]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                Item::Group { label, members, .. } => {
                    format!("◈ {} [{}]", label.line(), members.len())
                }
                Item::Call(shown) => format!("◆ {}", shown.line()),
                Item::Thought { text, .. } => format!("· {text}"),
                Item::More { label, hidden, .. } => {
                    format!("◈ {} [{} hidden]", label.line(), hidden.len())
                }
            })
            .collect()
    }

    /// Grok Build's own capture (the design note): three reads, two
    /// searches, and a listing fold into one labelled row; the command
    /// after them stands alone.
    #[test]
    fn looking_calls_fold_and_acting_calls_stand_alone() {
        let s = stretch(&[
            call(Verb::Read, "src/lib.rs"),
            call(Verb::Read, "src/main.rs"),
            call(Verb::Read, "Cargo.toml"),
            call(Verb::Search, "TODO"),
            call(Verb::Search, "fn"),
            call(Verb::List, "src"),
            call(Verb::Run, "ls -la && git log --oneline"),
            output("ls -la && git log --oneline", 0),
        ]);
        assert_eq!(
            shape(&s.items(true)),
            [
                "◈ Read 3 files, Searched 2 patterns, Listed 1 dir [6]",
                "◆ Run ls -la && git log --oneline",
            ]
        );
    }

    #[test]
    fn a_thought_among_looking_calls_folds_in_and_is_never_counted() {
        let s = stretch(&[
            thought("look first"),
            call(Verb::Read, "a.rs"),
            thought("and the test"),
            call(Verb::Read, "b.rs"),
            thought("now run it"),
            call(Verb::Run, "cargo test"),
        ]);
        // The command waits for its output, so its own row runs; the group
        // only looked and is done.
        assert_eq!(
            shape(&s.items(true)),
            ["◈ Read 2 files [5]", "◆ Run cargo test"]
        );
    }

    #[test]
    fn a_failed_command_says_how_and_a_group_counts_failures() {
        let mut s = stretch(&[call(Verb::Run, "cargo test"), output("cargo test", 101)]);
        assert_eq!(shape(&s.items(true)), ["◆ Run cargo test · exit 101"]);
        let mut failed = call(Verb::Read, "gone.rs");
        if let CoderEvent::Step(step) = &mut failed {
            step.call.as_mut().unwrap().failed = true;
        }
        assert!(s.push(9, &failed));
        assert!(s.push(10, &call(Verb::Read, "here.rs")));
        assert_eq!(
            shape(&s.items(true)),
            [
                "◆ Run cargo test · exit 101",
                "◈ Read 2 files · 1 failed [2]"
            ]
        );
    }

    #[test]
    fn a_running_member_puts_the_label_in_the_present() {
        let mut s = Stretch::default();
        assert!(s.push(1, &call(Verb::Read, "a.rs")));
        let mut fetch = s.clone();
        // A looking call that waits (a command-shaped read does not exist
        // today; mark one running by hand).
        if let Some(Entry::Call(shown)) = fetch.entries.last_mut() {
            shown.running = true;
        }
        assert_eq!(shape(&fetch.items(true)), ["◈ Reading 1 file [1]"]);
        fetch.settle();
        assert_eq!(shape(&fetch.items(true)), ["◈ Read 1 file [1]"]);
    }

    #[test]
    fn commands_match_their_output_by_command() {
        let s = stretch(&[
            call(Verb::Run, "cat > a.py"),
            call(Verb::Run, "python3 -m unittest"),
            output("python3 -m unittest", 1),
            output("cat > a.py", 0),
        ]);
        assert_eq!(
            shape(&s.items(true)),
            ["◆ Run cat > a.py", "◆ Run python3 -m unittest · exit 1"]
        );
        assert!(s.calls().all(|shown| !shown.running));
    }

    #[test]
    fn a_long_run_of_commands_folds_all_but_the_last_ten() {
        let mut events = Vec::new();
        for n in 0..14 {
            events.push(call(Verb::Run, &format!("step {n}")));
            events.push(output(&format!("step {n}"), i32::from(n == 1)));
        }
        events.push(call(Verb::Edit, "src/lib.rs"));
        let s = stretch(&events);
        let folded = shape(&s.items(true));
        assert_eq!(folded.len(), 1 + MAX_VISIBLE);
        assert_eq!(folded[0], "◈ Ran 5 commands · 1 failed [5 hidden]");
        assert_eq!(folded[1], "◆ Run step 5");
        assert_eq!(folded[MAX_VISIBLE], "◆ Edit src/lib.rs");
        // Expanded, nothing folds.
        assert_eq!(s.items(false).len(), 15);
        // Eleven lines show whole: the fold hides at least two.
        let eleven = stretch(&events[..22]);
        assert_eq!(eleven.items(true).len(), 11);
    }

    #[test]
    fn what_is_not_tool_activity_ends_a_stretch() {
        let mut s = Stretch::default();
        let note = CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: StepKind::Note,
            source: "system".into(),
            text: "running without Jev".into(),
            call: None,
        });
        assert!(!s.push(1, &note));
        // An observation with no call before it is not the stretch's.
        let observation = CoderEvent::Step(Step {
            kind: StepKind::Observation,
            ..match &note {
                CoderEvent::Step(step) => step.clone(),
                _ => unreachable!(),
            }
        });
        assert!(!s.push(2, &observation));
        assert!(s.is_empty());
    }

    #[test]
    fn a_log_prints_each_item_once_when_it_is_final() {
        let mut stream = Stream::default();
        let mut log = Vec::new();
        let mut feed = |stream: &mut Stream, seq: u64, event: CoderEvent| {
            let lines = stream.push(seq, &event).expect("tool activity");
            log.extend(lines);
            log.clone()
        };
        assert!(feed(&mut stream, 1, thought("look first")).is_empty());
        assert!(feed(&mut stream, 2, call(Verb::Read, "a.rs")).is_empty());
        assert!(feed(&mut stream, 3, call(Verb::Search, "fn")).is_empty());
        // A command closes the group; it waits for its output.
        assert_eq!(
            feed(&mut stream, 4, call(Verb::Run, "cargo test")),
            ["◈ Read 1 file, Searched 1 pattern"]
        );
        assert_eq!(
            feed(&mut stream, 5, output("cargo test", 1)),
            [
                "◈ Read 1 file, Searched 1 pattern",
                "◆ Run cargo test · exit 1"
            ]
        );
        assert!(feed(&mut stream, 6, thought("fix it")).len() == 2);
        let reply = CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: StepKind::Reply,
            source: "agent".into(),
            text: "done".into(),
            call: None,
        });
        // What a printed call returned is not a line of its own.
        let returned = CoderEvent::Step(Step {
            turn: 1,
            step_id: 1,
            kind: StepKind::Observation,
            source: "system".into(),
            text: "found 3 matches".into(),
            call: None,
        });
        let mut acted = Stream::default();
        let mut edit = call(Verb::Edit, "a.rs");
        if let CoderEvent::Step(step) = &mut edit {
            step.kind = StepKind::ToolCall;
        }
        assert_eq!(acted.push(1, &edit).unwrap(), ["◆ Edit a.rs"]);
        assert_eq!(acted.push(2, &returned).unwrap(), Vec::<String>::new());
        assert!(stream.push(7, &reply).is_none());
        assert_eq!(stream.flush(), ["· fix it"]);
        assert!(stream.flush().is_empty());
    }

    #[test]
    fn a_progress_bar_keeps_its_last_state() {
        assert_eq!(
            as_shown("a\n  Building 0/2\r  Building 1/2\r  Finished\nb\r\n"),
            "a\n  Finished\nb"
        );
    }

    #[test]
    fn lines_have_grok_builds_shapes() {
        let shown = |verb: Verb, target: &str, about: Option<&str>| Shown {
            seq: 1,
            call: Call {
                verb,
                target: target.into(),
                about: about.map(str::to_owned),
                failed: false,
            },
            running: false,
            status: None,
            output: String::new(),
        };
        assert_eq!(shown(Verb::Read, "lib.rs", None).line(), "Read lib.rs");
        assert_eq!(shown(Verb::Search, "TODO", None).line(), "Search \"TODO\"");
        assert_eq!(shown(Verb::List, "src", None).line(), "List src");
        assert_eq!(
            shown(Verb::Run, "git log -1", Some("Show the latest commit")).line(),
            "Run Show the latest commit"
        );
        assert_eq!(
            shown(Verb::Run, "cargo test\n--more", None).line(),
            "Run cargo test"
        );
        assert_eq!(
            shown(Verb::Other, "Used web_search", None).line(),
            "Used web_search"
        );
    }
}
