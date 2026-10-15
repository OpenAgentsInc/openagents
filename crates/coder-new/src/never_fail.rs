//! A turn never dead-ends (#11645).
//!
//! A harness or agent that fails before it answers (it didn't start, it
//! crashed, it stayed silent, its login is out) hands its message to Auto,
//! and one grey line says so. A harness that is off, missing, or failed
//! earlier in this run is never chosen again. Only when nothing at all can
//! run does a turn end, with one message that names the fix.

use crate::harness_pick::Pick;
use crate::harness_plugin::{self, Harness};
use crate::{App, Draft, Mode, live};

/// The grey line after Auto answered for `name`, which failed with `why`:
/// what happened, then why, so the real reason is there to read (#11690).
#[must_use]
pub fn fallback_note(name: &str, why: &str) -> String {
    let what = fallback_what(name, why);
    let why = why
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    if why.is_empty() {
        return what;
    }
    let mut why: String = why.chars().take(200).collect();
    if !why.ends_with(['.', '!', '?']) {
        why.push('.');
    }
    format!("{what} {why}")
}

fn fallback_what(name: &str, why: &str) -> String {
    let started = !(why.contains("could not start")
        || why.contains("couldn't start")
        || why.contains("didn't start")
        || why.contains("not installed")
        || why.contains("no process")
        || why.contains("finish setting up"));
    if started {
        format!("{name} couldn't answer, so Auto answered.")
    } else {
        format!("{name} didn't start, so Auto answered.")
    }
}

impl App {
    /// The name of the harness or agent whose turn just failed, when it
    /// failed before answering anything and Auto may take the message: a
    /// live chat on Claude Code, Codex, OpenCode or an ACP agent, with no
    /// words or tool calls after the person's message, that isn't already
    /// a fallback.
    pub(crate) fn failed_before_answering(&self) -> Option<String> {
        if self.mode != Mode::Live || self.fallback_from.is_some() {
            return None;
        }
        let name = if self.live.agent.is_some() {
            crate::harness_pick::name(&Pick::of_chat(&self.live, &self.pick_agents()))
        } else {
            match self.live.runs_on? {
                Harness::CoderLoop => return None,
                harness => crate::harness_pick::name(&Pick::Harness(harness)),
            }
        };
        if !matches!(self.live.entries.last(), Some(live::Entry::User(_))) {
            return None;
        }
        let partial = self.live.partial.trim();
        if !partial.is_empty() && !crate::agent_models::raw_failure(partial) {
            return None;
        }
        Some(name)
    }

    /// Sends the person's last message, which `name` failed with `why`, to
    /// Auto on Coder's own loop. The harness is left out for the rest of
    /// this run, and the chat goes on with Auto.
    pub(crate) fn fall_back_to_auto(&mut self, name: String, why: String) {
        if self.live.agent.is_none()
            && let Some(harness) = self.live.runs_on
            && harness != Harness::CoderLoop
            && !self.start_failed.contains(&harness)
        {
            self.start_failed.push(harness);
        }
        let Some(live::Entry::User(text)) = self.live.entries.pop() else {
            return;
        };
        self.leave_for_auto();
        self.notice = Some(format!("{name} didn't answer; Auto is answering…"));
        self.fallback_from = Some((name, why));
        let cursor = text.len();
        self.draft = Draft {
            text,
            cursor,
            ..Default::default()
        };
        self.submit_coder_loop();
    }

    /// The chat can't go on where it ran (#11563): its harness, agent or
    /// payer isn't here now. It goes on with Auto, and `why` says so in
    /// grey (#11645).
    pub(crate) fn go_on_with_auto(&mut self, why: String) {
        self.leave_for_auto();
        self.live.ran = None;
        self.notice = Some(why);
    }

    /// The chat leaves its harness, agent and door for Auto.
    fn leave_for_auto(&mut self) {
        self.live.agent = None;
        self.live.pinned = false;
        self.live.pinned_model = None;
        self.live.before_pin = None;
        self.live.door = None;
        self.live.runs_on = Some(Harness::CoderLoop);
        self.next_pick = None;
    }

    /// The harnesses whose plugins are on now.
    pub(crate) fn harness_plugins_on(&self) -> Vec<Harness> {
        let bundled = &self.plugins.bundled;
        [
            (Harness::ClaudeCode, bundled.claude),
            (Harness::Codex, bundled.codex),
            (Harness::OpenCode, bundled.opencode && bundled.acp),
        ]
        .into_iter()
        .filter_map(|(harness, on)| on.then_some(harness))
        .collect()
    }

    /// [`App::route_turn`], held to the rule that a harness that is off,
    /// missing, signed out, or failed in this run is never started
    /// (#11645): Auto picks again, and one grey line says why.
    pub(crate) fn admitted_turn(&mut self) -> Harness {
        let harness = self.route_turn();
        if harness == Harness::CoderLoop {
            return harness;
        }
        let found = harness_plugin::candidates(&harness_plugin::Facts::read(self));
        // Off, missing, or failed here: never started. A sign-in or a plan
        // limit is the harness's own answer to give, and its failure falls
        // back like any other.
        let gone = found.excluded.iter().any(|(excluded, why)| {
            *excluded == harness
                && matches!(
                    why,
                    harness_plugin::Exclusion::PluginOff
                        | harness_plugin::Exclusion::NotInstalled
                        | harness_plugin::Exclusion::FailedHere
                )
        });
        if !gone {
            return harness;
        }
        let name = crate::harness_pick::name(&Pick::Harness(harness));
        let next = harness_plugin::fixed_rule(&found);
        self.live.pinned = false;
        self.live.pinned_model = None;
        self.live.door = None;
        self.live.runs_on = Some(next);
        self.notice = Some(format!("{name} can't run here now, so Auto is answering."));
        next
    }
}
