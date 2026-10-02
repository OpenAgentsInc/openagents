//! The delegations: each Coder run this thread started, in the rail under
//! the composer (#10169).
//!
//! Ported from Coder Terminal's delegation rail (0.4.0). A row shows its
//! number, its agent, and what it is doing, from the run's own live status
//! ("Thinking…", "Run cargo test"). Up from an empty composer moves into
//! the rail and through it; Down moves back and returns to the composer;
//! Enter opens the selected run full screen, the run view, with its
//! steering composer. Alt+number and `/open <n>` open one directly. A
//! finished run keeps its row for [`KEPT_AFTER_DONE`], then leaves the
//! rail; `/open` still opens it by its number.

use coder_terminal::Scrollback;
use coder_terminal::components::rail::{MAX_ROWS, RailRow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openagents_chat::coder_events::{self, CoderEvent, Line as CoderLine, StepKind};
use ratatui::text::Line;

use crate::app::{Action, App, Wrap};
use crate::rows::{self, Row};
use crate::view::RunView;

/// How long a finished run keeps its row: 30 seconds of the 30 fps clock.
pub const KEPT_AFTER_DONE: u64 = 30 * 30;

/// One Coder run the thread started.
pub struct Delegation {
    /// Its Coder task.
    pub task: String,
    /// Who runs it, as the rail names it.
    pub agent: String,
    /// What it is doing now.
    pub doing: String,
    /// The tick `doing` began at, for its timer.
    pub since: u64,
    /// Its turn is running.
    pub running: bool,
    /// The tick it finished at.
    pub ended: Option<u64>,
    /// The run alone, every call with its output, for its full screen.
    pub log: Scrollback<Row, Line<'static>, Wrap>,
}

impl App {
    /// The delegation running `task`, added when it is new.
    fn delegation(&mut self, task: &str) -> &mut Delegation {
        if let Some(index) = self.delegations.iter().position(|held| held.task == task) {
            return &mut self.delegations[index];
        }
        let agent = self.engine.clone().unwrap_or_else(|| "Coder".to_owned());
        self.delegations.push(Delegation {
            task: task.to_owned(),
            agent,
            doing: "Starting…".to_owned(),
            since: self.tick,
            running: true,
            ended: None,
            log: crate::app::transcript(self.ladder),
        });
        self.delegations.last_mut().expect("just pushed")
    }

    /// Coder accepted a run: it has a row from now on.
    pub(crate) fn delegated(&mut self, task: &str) {
        let tick = self.tick;
        let held = self.delegation(task);
        // Following the run again says so too; its timer keeps going.
        if !held.running {
            held.running = true;
            held.ended = None;
            held.since = tick;
        }
    }

    /// One event of a run: where it stands and what it is doing. `fresh`
    /// when the event was not shown before; `ended` whether its turn ended
    /// for good.
    pub(crate) fn track(&mut self, line: &CoderLine, fresh: bool, ended: bool) {
        let tick = self.tick;
        let held = self.delegation(&line.task);
        let doing = match &line.event {
            CoderEvent::CoderStarted(started) => {
                held.agent = rows::provider(&started.provider);
                Some(coder_events::starting(&started.provider))
            }
            CoderEvent::Status(status) => Some(status.text.clone()),
            CoderEvent::Step(step)
                if matches!(step.kind, StepKind::Command | StepKind::ToolCall) =>
            {
                step.text
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .map(str::to_owned)
            }
            _ => None,
        };
        // A follow that starts again replays the run from its first
        // event: what it is doing comes only from events new here.
        if let Some(doing) = doing.filter(|doing| fresh && *doing != held.doing) {
            held.doing = doing;
            held.since = tick;
        }
        if line.event.ends_turn() {
            held.running = !ended;
            held.ended = ended.then_some(tick);
        } else if matches!(
            line.event,
            CoderEvent::CoderStarted(_) | CoderEvent::Status(_) | CoderEvent::Progress(_)
        ) {
            held.running = true;
            held.ended = None;
        }
        if fresh {
            crate::app::grow(&mut held.log, line, true);
        }
    }

    /// A row of the thread's current run, in its delegation's log too.
    pub(crate) fn track_row(&mut self, row: &Row) {
        if let Some(task) = self.task.clone()
            && let Some(held) = self.delegations.iter_mut().find(|held| held.task == task)
        {
            held.log.push(row.clone());
        }
    }

    /// The numbers the rail shows, from one: the running runs and those
    /// that finished in the last [`KEPT_AFTER_DONE`], at most
    /// [`MAX_ROWS`], the running and the newest first to stay.
    #[must_use]
    pub fn rail_numbers(&self) -> Vec<usize> {
        let mut numbers: Vec<usize> = (1..=self.delegations.len())
            .filter(|number| {
                let held = &self.delegations[number - 1];
                held.running
                    || held
                        .ended
                        .is_none_or(|ended| self.tick.saturating_sub(ended) <= KEPT_AFTER_DONE)
            })
            .collect();
        numbers.sort_by_key(|number| {
            (
                !self.delegations[number - 1].running,
                std::cmp::Reverse(*number),
            )
        });
        numbers.truncate(MAX_ROWS);
        numbers.sort_unstable();
        numbers
    }

    /// The rail's rows, as [`coder_terminal::components::rail`] draws them.
    #[must_use]
    pub fn rail_rows(&self) -> Vec<RailRow> {
        self.rail_numbers()
            .into_iter()
            .map(|number| {
                let held = &self.delegations[number - 1];
                RailRow {
                    number,
                    agent: held.agent.clone(),
                    doing: held.doing.clone(),
                    elapsed: held.running.then(|| {
                        coder_terminal::grok_spinner::elapsed(self.tick.saturating_sub(held.since))
                    }),
                    selected: self.rail == Some(number),
                }
            })
            .collect()
    }

    /// Whether the rail shows: it has rows and no full screen covers it.
    #[must_use]
    pub fn rail_shown(&self) -> bool {
        self.run_view.is_none() && !self.rail_numbers().is_empty()
    }

    /// A key for the rail, when it takes it: Up and Down move, Enter
    /// opens, Esc returns to the composer, and Alt+number opens one from
    /// anywhere. Any other key returns to the composer and does what it
    /// does there.
    pub(crate) fn rail_key(&mut self, key: &KeyEvent) -> Option<Vec<Action>> {
        if key.modifiers == KeyModifiers::ALT
            && let KeyCode::Char(digit @ '1'..='9') = key.code
        {
            self.open_delegation(usize::from(digit as u8 - b'0'));
            return Some(Vec::new());
        }
        if self.run_view.is_some() {
            return None;
        }
        let visible = self.rail_numbers();
        let at = self
            .rail
            .and_then(|number| visible.iter().position(|shown| *shown == number));
        if at.is_none() {
            self.rail = None;
        }
        if key.modifiers != KeyModifiers::NONE {
            self.rail = None;
            return None;
        }
        match (key.code, at) {
            (KeyCode::Up, None) if !visible.is_empty() && self.editor.is_empty() => {
                self.rail = Some(visible[0]);
            }
            (KeyCode::Up, Some(index)) => {
                self.rail = Some(visible[(index + 1).min(visible.len() - 1)]);
            }
            (KeyCode::Down, Some(index)) => {
                self.rail = index.checked_sub(1).map(|before| visible[before]);
            }
            (KeyCode::Enter, Some(index)) => {
                self.rail = None;
                self.open_delegation(visible[index]);
            }
            (KeyCode::Esc, Some(_)) => self.rail = None,
            (_, Some(_)) => {
                self.rail = None;
                return None;
            }
            _ => return None,
        }
        Some(Vec::new())
    }

    /// Open delegation `number` full screen, or say there is none.
    pub fn open_delegation(&mut self, number: usize) {
        self.rail = None;
        if number == 0 || number > self.delegations.len() {
            self.note(match self.delegations.len() {
                0 => "This thread has no Coder run yet.".to_owned(),
                1 => "There is one Coder run: /open 1.".to_owned(),
                runs => format!("There are {runs} Coder runs: /open 1 to /open {runs}."),
            });
            return;
        }
        self.run_view = Some(RunView {
            scroll: 0,
            number: Some(number),
        });
    }

    /// The run the full screen shows, when it shows one of the rail's.
    #[must_use]
    pub fn viewed(&self) -> Option<&Delegation> {
        let number = self.run_view?.number?;
        self.delegations.get(number - 1)
    }

    /// Whether a rail row is animating: a running one's spinner, or a
    /// finished one waiting to leave.
    #[must_use]
    pub fn rail_animating(&self) -> bool {
        !self.rail_numbers().is_empty()
    }
}

/// Persistent readers keep status refreshes from replaying each run's log.
#[derive(Default)]
pub(crate) struct Refresh {
    readers: std::collections::HashMap<String, Box<dyn openagents_chat::client::Follow>>,
}

impl Refresh {
    pub(crate) fn poll(
        &mut self,
        tasks: &[String],
        mut follow: impl FnMut(&str) -> Box<dyn openagents_chat::client::Follow>,
    ) -> Vec<(String, openagents_chat::client::Progress)> {
        self.readers.retain(|task, _| tasks.contains(task));
        tasks
            .iter()
            .filter_map(|task| {
                let reader = self
                    .readers
                    .entry(task.clone())
                    .or_insert_with(|| follow(task));
                // An unreadable or busy store is not evidence that a run ended.
                reader.poll().ok().map(|(_, state)| (task.clone(), state))
            })
            .collect()
    }
}

impl App {
    pub(crate) fn refresh_rail(
        &mut self,
        states: Vec<(String, openagents_chat::client::Progress)>,
    ) {
        for (task, state) in states {
            let Some(held) = self.delegations.iter_mut().find(|held| held.task == task) else {
                continue;
            };
            let running = state == openagents_chat::client::Progress::Running;
            if running {
                if !held.running {
                    held.since = self.tick;
                }
                held.running = true;
                held.ended = None;
            } else {
                held.running = false;
                held.ended.get_or_insert(self.tick);
            }
        }
    }
}
