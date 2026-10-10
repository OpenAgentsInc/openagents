//! `/loop INTERVAL PROMPT` (#11177): a prompt the chat repeats on an
//! interval while Coder is open, such as `/loop 5m check the deploy`.
//!
//! Each time it is due the prompt joins the chat as the next input, the
//! same way a background agent's notice does: a reply in progress reads it
//! after its current step; an idle chat starts a turn with it. A run that
//! has not started yet is not queued twice. A usage-limit pause (#11179)
//! holds it like any other prompt. Loops end with the session; a prompt
//! that should run while Coder is closed is a `/schedule` instead.
//!
//! This module also routes the commands that take arguments: `/loop`,
//! `/schedule` ([`crate::schedule`]) and `/shells` ([`crate::shells`]).

use crate::{App, Draft, Mode};

/// The shortest interval.
const MIN_SECS: u64 = 10;
/// The longest interval: a week.
const MAX_SECS: u64 = 7 * 24 * 60 * 60;

/// How to start one.
pub const USAGE: &str = "Use /loop INTERVAL PROMPT, for example /loop 5m check the deploy. /loop lists loops; /loop stop N or /loop stop all ends them.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Loop {
    pub id: u32,
    pub every_secs: u64,
    pub prompt: String,
    pub next_ms: u64,
    pub runs: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Loops {
    pub items: Vec<Loop>,
    next_id: u32,
}

/// An interval such as `30s`, `5m`, `2h` or `1d`.
pub(crate) fn interval(text: &str) -> Option<u64> {
    let text = text.trim().to_ascii_lowercase();
    let split = text.find(|c: char| !c.is_ascii_digit())?;
    let (number, unit) = text.split_at(split);
    let number: u64 = number.parse().ok()?;
    let scale = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
        "d" | "day" | "days" => 86_400,
        _ => return None,
    };
    number.checked_mul(scale)
}

/// An interval in words: `30s`, `5m`, `2h`, `1d`, or minutes and seconds.
pub(crate) fn every(seconds: u64) -> String {
    match seconds {
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3600 == 0 => format!("{}h", s / 3600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

impl App {
    /// The commands with arguments this module routes. `true` when the
    /// composer held one of them.
    pub(crate) fn long_work_command(&mut self) -> bool {
        let text = self.draft.text.trim().to_owned();
        let (word, argument) = text
            .split_once(char::is_whitespace)
            .map_or((text.as_str(), ""), |(word, rest)| (word, rest.trim()));
        if argument.is_empty() {
            // The bare words go through the slash command list.
            return false;
        }
        match word {
            "/loop" => {
                self.record_prompt();
                self.loop_command(argument);
            }
            "/schedule" => {
                self.record_prompt();
                self.schedule_command(argument);
            }
            "/shells" => {
                self.record_prompt();
                self.shells_command_with(argument);
            }
            _ => return false,
        }
        self.draft = Draft::default();
        self.composer = Default::default();
        self.composer_history.reset();
        true
    }

    /// `/loop`, `/loop INTERVAL PROMPT`, `/loop stop N|all`.
    pub(crate) fn loop_command(&mut self, argument: &str) {
        if self.mode != Mode::Live {
            self.notice = Some("Loops run in live mode only.".into());
            return;
        }
        let argument = argument.trim();
        if argument.is_empty() || argument == "list" {
            self.notice = Some(self.loop_listing());
            return;
        }
        if let Some(which) = argument.strip_prefix("stop").map(str::trim) {
            self.notice = Some(match which {
                "all" => {
                    let count = self.loops.items.len();
                    self.loops.items.clear();
                    format!("Stopped {count} loops.")
                }
                id => match id.trim_start_matches('#').parse::<u32>() {
                    Ok(id) if self.loops.items.iter().any(|item| item.id == id) => {
                        self.loops.items.retain(|item| item.id != id);
                        format!("Stopped loop {id}.")
                    }
                    _ => format!("No loop {id}. {USAGE}"),
                },
            });
            return;
        }
        let Some((first, prompt)) = argument
            .split_once(char::is_whitespace)
            .map(|(first, prompt)| (first, prompt.trim()))
            .filter(|(_, prompt)| !prompt.is_empty())
        else {
            self.notice = Some(USAGE.into());
            return;
        };
        let Some(seconds) = interval(first) else {
            self.notice = Some(format!("`{first}` is not an interval. {USAGE}"));
            return;
        };
        if !(MIN_SECS..=MAX_SECS).contains(&seconds) {
            self.notice = Some("A loop's interval is 10 seconds to 7 days.".into());
            return;
        }
        self.loops.next_id += 1;
        let id = self.loops.next_id;
        // The first run is now; the next after the interval.
        self.loops.items.push(Loop {
            id,
            every_secs: seconds,
            prompt: prompt.to_owned(),
            next_ms: atif::now_ms(),
            runs: 0,
        });
        self.notice = Some(format!(
            "Loop {id}: every {} while Coder is open, starting now. /loop stop {id} ends it.",
            every(seconds)
        ));
        self.poll_loops();
    }

    fn loop_listing(&self) -> String {
        if self.loops.items.is_empty() {
            return format!("No loops. {USAGE}");
        }
        let now = atif::now_ms();
        let mut text = String::from("Loops:\n");
        for item in &self.loops.items {
            text.push_str(&format!(
                "{}  every {}  next in {}  ran {}×  {}\n",
                item.id,
                every(item.every_secs),
                every(item.next_ms.saturating_sub(now).div_ceil(1000).max(1)),
                item.runs,
                crate::long_session::clip(&item.prompt, 80)
            ));
        }
        text.push_str("/loop stop N or /loop stop all ends them.");
        text
    }

    /// Queues each loop that is due, unless its last run has not started.
    pub(crate) fn poll_loops(&mut self) {
        if self.mode != Mode::Live || self.loops.items.is_empty() {
            return;
        }
        let now = atif::now_ms();
        let mut due = Vec::new();
        for item in &mut self.loops.items {
            if now < item.next_ms {
                continue;
            }
            item.next_ms = now.saturating_add(item.every_secs.saturating_mul(1000));
            due.push(item.prompt.clone());
            item.runs += 1;
        }
        for prompt in due {
            let waiting = self
                .queued_prompts
                .iter()
                .any(|queued| queued.notice && queued.text == prompt);
            if !waiting {
                self.queue_notice(prompt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_app() -> App {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app
    }

    #[test]
    fn intervals_read_and_print() {
        assert_eq!(interval("30s"), Some(30));
        assert_eq!(interval("5m"), Some(300));
        assert_eq!(interval("2h"), Some(7200));
        assert_eq!(interval("1d"), Some(86_400));
        assert_eq!(interval("10 minutes"), None);
        assert_eq!(interval("10minutes"), Some(600));
        assert_eq!(interval("m"), None);
        assert_eq!(interval("5"), None);
        assert_eq!(every(300), "5m");
        assert_eq!(every(90), "90s");
    }

    #[test]
    fn a_loop_runs_now_then_on_its_interval_without_piling_up() {
        let mut app = live_app();
        app.loop_command("5m check the deploy");
        assert_eq!(app.loops.items.len(), 1);
        // The first run starts at once.
        assert!(app.request.is_none());
        app.process_prompt_queue();
        let request = app.request.take().expect("the loop's first run starts");
        let crate::live::Work::Microcoder { messages, .. } = request.kind else {
            panic!("a keyless chat uses the local loop")
        };
        assert_eq!(messages.last().unwrap().content, "check the deploy");
        // Not due again yet.
        app.poll_loops();
        assert!(app.queued_prompts.is_empty());
        // Due twice while a reply runs: queued once.
        app.loops.items[0].next_ms = 0;
        app.poll_loops();
        app.loops.items[0].next_ms = 0;
        app.poll_loops();
        assert_eq!(app.queued_prompts.len(), 1);
        assert_eq!(app.loops.items[0].runs, 3);
        app.loop_command("stop 1");
        assert!(app.loops.items.is_empty());
    }

    #[test]
    fn a_bad_loop_explains_itself() {
        let mut app = live_app();
        app.loop_command("soon check it");
        assert!(app.notice.as_deref().unwrap().contains("not an interval"));
        app.loop_command("1s check it");
        assert!(app.notice.as_deref().unwrap().contains("10 seconds"));
        app.loop_command("5m");
        assert_eq!(app.notice.as_deref(), Some(USAGE));
        assert!(app.loops.items.is_empty());
    }

    #[test]
    fn the_composer_routes_loop_with_arguments() {
        let mut app = live_app();
        app.draft.text = "/loop 1h summarize the open issues".into();
        assert!(app.long_work_command());
        assert!(app.draft.text.is_empty());
        assert_eq!(app.loops.items[0].prompt, "summarize the open issues");
        app.draft.text = "/loop".into();
        assert!(!app.long_work_command());
    }
}
