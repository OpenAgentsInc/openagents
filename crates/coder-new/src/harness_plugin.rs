//! Harness plugins: the engines a Coder turn can run on, each declared the
//! same way so a router can choose among them (#11451, #11452).
//!
//! A harness is what runs a piece of work: Claude Code on this computer's
//! Claude plan, OpenCode on this computer (#11507), Coder's own loop on the
//! OpenAgents gateway, Codex on this computer's ChatGPT login. Each one declares where it runs, who pays,
//! what it can do, and a cheap health check. Solver and background agents
//! are not harnesses: they are Coder tools any harness can call.
//!
//! Candidates are built in code before anything ranks them
//! (`docs/api/2026-10-02-agentic-execution-router.md`, section 4):
//! [`candidates`] keeps the harnesses that are on, healthy and runnable
//! here, and gives every other one a typed [`Exclusion`]. Choosing among the
//! candidates is router v0's job ([`crate::router`], #11460); [`fixed_rule`]
//! takes the first one when the router is off or System One doesn't answer. [`App::submit_on`] starts a turn on whichever is
//! chosen.
//!
//! There is no mode switch (#11453): a chat runs on the harness chosen for
//! its first turn ([`crate::live::Chat::runs_on`]) and stays on it for
//! follow-ups, so its session and prompt cache carry on.

use crate::{App, Mode, plugin_tools::ExecutionSettings};

// The harness contract itself (identity, place, abilities, turn, and the
// declarations) lives in `route_contract::harness`, so the phone and the
// web read the same table (#11456).
pub use route_contract::harness::{Agent, Can, DECLARATIONS, Declaration, Harness, Place, Turn};

/// Who pays for a harness's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Payer {
    /// The person's Claude subscription: no per-turn dollars.
    ClaudePlan,
    /// The person's ChatGPT subscription: no per-turn dollars.
    ChatGptLogin,
    /// OpenAgents gateway credits.
    GatewayCredits,
    /// The person's own OpenRouter key.
    OpenRouterKey,
    /// The person's OpenCode: its free models, or the providers they set
    /// up in it (#11507).
    #[serde(rename = "opencode")]
    OpenCode,
    /// An ACP agent's own login (Grok Build, Devin, Cursor...), for a turn
    /// or a delegation on one (#11535).
    AgentLogin,
    /// The person's own Gemini API key, for a no-harness turn (#11548).
    GeminiKey,
    /// Coder's own free answers while signed out (#11645): an anonymous
    /// agent account's free tier, no dollars to the person.
    FreeTier,
}

/// Why a harness is not a candidate for this chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Exclusion {
    /// Its plugin is off.
    PluginOff,
    /// It needs a sign-in this computer doesn't hold.
    NotSignedIn,
    /// It isn't installed on this computer.
    NotInstalled,
    /// Its plan reported a window at its limit.
    AtPlanLimit,
    /// It runs on this computer, and the chat is somewhere else.
    NotHere,
    /// It takes no chat turns yet.
    NoChatTurn,
    /// The person said never to use it ([`crate::routing_preferences`]).
    PersonSaidNever,
    /// The person said never to run the model it would run on it.
    PersonSaidNeverWithModel,
    /// It failed before answering earlier in this run (#11645), so Auto
    /// leaves it out until its plugin is turned on again.
    FailedHere,
}

/// The harnesses that failed before answering in this run (#11645).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Failed {
    pub claude: bool,
    pub codex: bool,
    pub opencode: bool,
}

impl Failed {
    /// The harnesses in `failed`.
    #[must_use]
    pub fn of(failed: &[Harness]) -> Self {
        Failed {
            claude: failed.contains(&Harness::ClaudeCode),
            codex: failed.contains(&Harness::Codex),
            opencode: failed.contains(&Harness::OpenCode),
        }
    }

    /// Whether `harness` failed.
    #[must_use]
    pub fn get(self, harness: Harness) -> bool {
        match harness {
            Harness::ClaudeCode => self.claude,
            Harness::Codex => self.codex,
            Harness::OpenCode => self.opencode,
            Harness::CoderLoop => false,
        }
    }
}

/// What a harness's health check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub signed_in: bool,
    pub at_limit: bool,
}

/// What the candidate check reads about the chat and this computer, so the
/// rule is a pure function a test can drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    /// The chat runs on this computer (the terminal), not on a phone or the
    /// web.
    pub local: bool,
    pub claude_plugin_on: bool,
    pub claude: Health,
    /// The Codex plugin is on here (#11510).
    pub codex_on: bool,
    pub codex: Health,
    /// The OpenCode plugin is on (#11507).
    pub opencode_on: bool,
    /// OpenCode is installed here (`signed_in`), and whether its free
    /// model is rate limited now (`at_limit`).
    pub opencode: Health,
    /// The person's OpenRouter key is on and set; the Coder loop then pays
    /// with it rather than gateway credits.
    pub openrouter_key: bool,
    /// What the person's routing preferences say about each harness
    /// (#11455).
    pub leaning: crate::routing_preferences::Leaning,
    /// The harnesses that failed before answering in this run (#11645).
    pub failed: Failed,
}

impl Facts {
    /// The facts for `app`'s chat on this computer. The checks are cheap:
    /// the sign-in answers are cached ([`crate::claude::available`]) and
    /// the plan windows are what Claude Code last reported.
    #[must_use]
    pub fn read(app: &App) -> Self {
        let execution = app.plugins.execution_settings(
            app.cwd
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
        );
        Self::from_parts(app, &execution)
    }

    fn from_parts(app: &App, execution: &ExecutionSettings) -> Self {
        let now = atif::now_ms() / 1000;
        // A forced no-harness run counts every harness plugin as off
        // (#11548).
        let forced = crate::no_harness::forced();
        let claude_plugin_on = app.plugins.bundled.claude && !forced;
        let codex_on = app.plugins.bundled.codex && !forced;
        // OpenCode is an installed coding agent: the ACP Subagents switch
        // ("other coding agents installed on this computer") turns it off
        // too (#11645).
        let opencode_on = app.plugins.bundled.opencode && app.plugins.bundled.acp && !forced;
        // The sign-in and the plan windows are read only for a plugin
        // that is on.
        let claude_signed_in = claude_plugin_on && claude_signed_in();
        Self {
            local: true,
            claude_plugin_on,
            claude: Health {
                signed_in: claude_signed_in,
                at_limit: claude_signed_in && claude_at_limit(now),
            },
            codex_on,
            // The sign-in is read once and cached ([`crate::codex::available`]);
            // the windows are what Codex last reported.
            codex: Health {
                signed_in: codex_on && crate::codex::available(),
                at_limit: crate::codex::at_limit(now),
            },
            opencode_on,
            // Whether it is installed is read only for a plugin that is on.
            opencode: {
                let installed = opencode_on && crate::opencode_chat::installed();
                Health {
                    signed_in: installed,
                    at_limit: installed && crate::opencode_chat::rate_limited(),
                }
            },
            openrouter_key: app.plugins.enabled && app.plugins.key_configured,
            leaning: crate::routing_preferences::Leaning::read(
                execution.routing.as_deref(),
                |harness| match harness {
                    Harness::ClaudeCode => Some(crate::claude::choice().model),
                    Harness::OpenCode => Some(crate::opencode_chat::model(None)),
                    Harness::CoderLoop => Some(app.plugins.model.clone()),
                    Harness::Codex => Some(crate::codex::choice().model),
                },
            ),
            failed: Failed::of(&app.start_failed),
        }
    }
}

/// Whether this computer holds a Claude Code login. A unit test never asks
/// the computer: its chats run on the Coder loop unless the test signs one
/// in.
#[cfg(not(test))]
fn claude_signed_in() -> bool {
    crate::claude::available()
}

#[cfg(test)]
thread_local! {
    static TEST_CLAUDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn claude_signed_in() -> bool {
    TEST_CLAUDE.with(std::cell::Cell::get)
}

/// Whether Claude Code may be offered for a pick (#11524), from cheap
/// checks only: its program is on disk and no check this run found it
/// signed out. The sign-in check itself runs `claude auth status`, so it
/// waits for a refresh off the input's thread.
#[cfg(not(test))]
pub(crate) fn claude_present() -> bool {
    crate::claude::known() != Some(false) && crate::bundled_runtime::claude_print::agent().is_some()
}

#[cfg(test)]
pub(crate) fn claude_present() -> bool {
    claude_signed_in()
}

/// Makes this test thread's computer hold a Claude Code login, or not.
#[cfg(test)]
pub(crate) fn sign_in_claude(signed_in: bool) {
    TEST_CLAUDE.with(|cell| cell.set(signed_in));
}

/// Whether a Claude plan window Claude Code reported is at its limit now.
fn claude_at_limit(now: u64) -> bool {
    crate::claude_plan::plan_usage()
        .and_then(|usage| usage.current(now))
        .is_some_and(|usage| usage.windows().any(|window| window.used_percent >= 100))
}

/// The share of the Claude plan's five-hour window used, as Claude Code
/// last reported it.
fn claude_five_hour(now: u64) -> Option<u8> {
    crate::claude_plan::plan_usage()
        .and_then(|usage| usage.current(now))
        .and_then(|usage| usage.five_hour)
        .map(|window| window.used_percent)
}

/// The share of the Claude plan's five-hour window used now, as Claude Code
/// last reported it.
pub(crate) fn claude_five_hour_now() -> Option<u8> {
    claude_five_hour(atif::now_ms() / 1000)
}

/// Whether a Claude plan window Claude Code reported is at its limit now.
pub(crate) fn claude_at_limit_now() -> bool {
    claude_at_limit(atif::now_ms() / 1000)
}

/// What the person reads when every engine that could run here is one
/// they said never to use.
pub const NEVER_LEFT: &str =
    "You've said never to use every agent that can run here. Change that in /preferences.";

/// Who pays when `harness` runs under `facts`.
#[must_use]
pub fn payer(harness: Harness, facts: &Facts) -> Payer {
    match harness {
        Harness::ClaudeCode => Payer::ClaudePlan,
        Harness::Codex => Payer::ChatGptLogin,
        Harness::OpenCode => Payer::OpenCode,
        Harness::CoderLoop if facts.openrouter_key => Payer::OpenRouterKey,
        Harness::CoderLoop => Payer::GatewayCredits,
    }
}

/// Who actually paid for a turn that ran on `harness` and was answered by
/// `model` (#11550), set when the turn ends so it always agrees with the
/// model: the Coder loop's turn is the Codex plan's when a Codex model
/// answered on this computer's login (`codex:` or a bare `gpt-` id), the
/// Claude plan's for a bare `claude-` id, OpenCode's for `opencode/`, and
/// otherwise the person's own OpenRouter key when `byok` says the turn
/// went there, else gateway credit. `agent` turns are the agent's login's.
#[must_use]
pub fn paid_by(harness: Harness, agent: bool, model: &str, byok: bool) -> Payer {
    if agent {
        return Payer::AgentLogin;
    }
    match harness {
        Harness::ClaudeCode => Payer::ClaudePlan,
        Harness::Codex => Payer::ChatGptLogin,
        Harness::OpenCode => Payer::OpenCode,
        Harness::CoderLoop => {
            let bare = !model.contains('/');
            if model.starts_with("codex:") || (bare && model.starts_with("gpt-")) {
                Payer::ChatGptLogin
            } else if bare && model.starts_with("claude-") {
                Payer::ClaudePlan
            } else if bare && model.starts_with("gemini-") {
                // The Gemini API names its model bare; the gateway names
                // it `google/…` (#11548).
                Payer::GeminiKey
            } else if model.starts_with("opencode/") {
                Payer::OpenCode
            } else if byok && !model.starts_with("openagents/") {
                Payer::OpenRouterKey
            } else {
                Payer::GatewayCredits
            }
        }
    }
}

/// Why `harness` is left out under `facts`, or `None` when it is a
/// candidate.
#[must_use]
pub fn exclusion(harness: Harness, facts: &Facts) -> Option<Exclusion> {
    let declaration = harness.declaration();
    // The person's word comes first: a harness they said never to use is
    // never started, whatever else holds (#11455).
    if facts.leaning.stances.get(harness) == crate::routing_preferences::Stance::Never {
        return Some(Exclusion::PersonSaidNever);
    }
    if facts.leaning.never_with_model.get(harness) {
        return Some(Exclusion::PersonSaidNeverWithModel);
    }
    if declaration.place == Place::ThisComputer && !facts.local {
        return Some(Exclusion::NotHere);
    }
    if facts.failed.get(harness) {
        return Some(Exclusion::FailedHere);
    }
    match harness {
        Harness::ClaudeCode if !facts.claude_plugin_on => Some(Exclusion::PluginOff),
        Harness::ClaudeCode if !facts.claude.signed_in => Some(Exclusion::NotSignedIn),
        Harness::ClaudeCode if facts.claude.at_limit => Some(Exclusion::AtPlanLimit),
        Harness::OpenCode if !facts.opencode_on => Some(Exclusion::PluginOff),
        Harness::OpenCode if !facts.opencode.signed_in => Some(Exclusion::NotInstalled),
        Harness::OpenCode if facts.opencode.at_limit => Some(Exclusion::AtPlanLimit),
        Harness::Codex if !facts.codex_on => Some(Exclusion::PluginOff),
        Harness::Codex if !facts.codex.signed_in => Some(Exclusion::NotSignedIn),
        Harness::Codex if facts.codex.at_limit => Some(Exclusion::AtPlanLimit),
        _ if declaration.turn == Turn::NotYet => Some(Exclusion::NoChatTurn),
        // The gateway is always there; a failed call is the turn's error.
        _ => None,
    }
}

/// The harnesses that can take this chat's next turn, the ones the person
/// prefers first and the ones they avoid last (declaration order within
/// each), and why each other one was left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidates {
    pub admitted: Vec<Harness>,
    pub excluded: Vec<(Harness, Exclusion)>,
}

/// [`Candidates`] under `facts`.
#[must_use]
pub fn candidates(facts: &Facts) -> Candidates {
    let mut admitted = Vec::new();
    let mut excluded = Vec::new();
    for declaration in DECLARATIONS {
        match exclusion(declaration.harness, facts) {
            None => admitted.push(declaration.harness),
            Some(why) => excluded.push((declaration.harness, why)),
        }
    }
    admitted.sort_by_key(|harness| facts.leaning.stances.get(*harness).rank());
    Candidates { admitted, excluded }
}

/// The selector when router v0 is off or has no answer: the first admitted
/// candidate, which is Claude Code when its plugin is on and healthy, then
/// OpenCode when it is installed and on (#11507), and otherwise the Coder
/// loop, which is always there.
#[must_use]
pub fn fixed_rule(candidates: &Candidates) -> Harness {
    candidates
        .admitted
        .first()
        .copied()
        .unwrap_or(Harness::CoderLoop)
}

impl App {
    /// The harness this chat's next turn runs on: router v0's call when it
    /// judged this message ([`App::ask_router`]), otherwise the one the
    /// chat already runs on, or, for a chat's first turn with no router,
    /// the fixed rule's pick. Each new piece of work is routed; follow-ups
    /// System One judges a continuation stay on the chat's harness (#11451,
    /// owner decision 1). A chat that moves to Claude Code starts its
    /// session from the transcript ([`crate::live::Chat::claude_resume`]).
    pub fn route_turn(&mut self) -> Harness {
        if let Some(mut routed) = self.routed.take() {
            // A chat never hops (#11556): once its harness answered, Auto
            // keeps it there, on the same path (cheap or not), so one chat
            // runs on one harness and one provider. The one way out is
            // forward (#11696): an Auto chat on Coder's own loop moves to
            // the harness the router picks for work beyond a small answer,
            // which starts from the conversation so far. A harness never
            // hands a chat on.
            let hands_on = self.live.runs_on == Some(Harness::CoderLoop)
                && routed.harness != Harness::CoderLoop
                && !self.live.pinned
                && !self.live.model_chosen
                && self.live.agent.is_none();
            if hands_on && self.live.answered() {
                self.rail_notice(
                    format!(
                        "This chat moves to {} for this work.",
                        crate::harness_pick::name(&crate::harness_pick::Pick::Harness(
                            routed.harness
                        ))
                    ),
                    crate::RAIL_NOTICE_TTL,
                );
            }
            if let Some(harness) = self.live.runs_on
                && self.live.answered()
                && !hands_on
            {
                self.routing_rule = Some(routed.rule);
                routed.trace.follow_up = true;
                self.routing_router = Some((routed.probabilities, routed.trace));
                return harness;
            }
            self.live.runs_on = Some(routed.harness);
            self.live.pinned = false;
            // A pin's model holds until another routing call, a follow-up
            // that continues keeps it.
            // A cheap answer runs on a cheap model, and a follow-up that
            // continues it stays cheap (#11537).
            let step = routed.trace.step.as_deref();
            if step != Some("continue") {
                self.live.cheap = step == Some(crate::router::CHEAP_ANSWER);
            }
            if routed.rule == crate::routing_record::Rule::Pin {
                self.live.pinned_model = routed.model;
            } else if step != Some("continue") {
                self.live.pinned_model = None;
            }
            self.routing_rule = Some(routed.rule);
            self.routing_router = Some((routed.probabilities, routed.trace));
            return routed.harness;
        }
        if let Some(harness) = self.live.runs_on {
            return harness;
        }
        // A pin for every kind of work decides a new chat's harness even
        // with no router (#11475).
        let found = candidates(&Facts::read(self));
        let pin = self
            .pins()
            .into_iter()
            .rev()
            .find(|pin| pin.kind.is_none() && found.admitted.contains(&pin.harness));
        if let Some(pin) = pin {
            self.live.runs_on = Some(pin.harness);
            self.live.pinned = false;
            self.live.pinned_model = pin.model;
            self.live.cheap = false;
            self.routing_rule = Some(crate::routing_record::Rule::Pin);
            return pin.harness;
        }
        let harness = fixed_rule(&found);
        self.live.runs_on = Some(harness);
        self.live.pinned = false;
        self.live.cheap = false;
        // A chat with earlier turns lost its harness (its plugin went off)
        // and is routed again (#11454).
        self.routing_rule = Some(if self.live.user_count() > 0 {
            crate::routing_record::Rule::Reroute
        } else {
            crate::routing_record::Rule::FirstAdmitted
        });
        harness
    }

    /// The person's pins that apply in this chat's repository (#11475).
    pub(crate) fn pins(&self) -> Vec<crate::routing_preferences::Pin> {
        let cwd = self.context_cwd();
        let Some(path) = self.plugins.execution_settings(cwd.clone()).routing else {
            return Vec::new();
        };
        crate::routing_preferences::RoutingPreferences::load(&path)
            .map(|preferences| {
                preferences.pins_in(crate::routing_preferences::repo_name(&cwd).as_deref())
            })
            .unwrap_or_default()
    }

    /// A harness the person said never to use is never started (#11475):
    /// a chat on one moves to another at its next message, and with none
    /// left the message is refused and stays in the input. Returns whether
    /// it was refused.
    pub(crate) fn honor_never(&mut self) -> bool {
        let facts = Facts::read(self);
        let candidates = candidates(&facts);
        let never = |why: &Exclusion| {
            matches!(
                why,
                Exclusion::PersonSaidNever | Exclusion::PersonSaidNeverWithModel
            )
        };
        if let Some(current) = self.live.runs_on
            && candidates
                .excluded
                .iter()
                .any(|(harness, why)| *harness == current && never(why))
        {
            self.live.runs_on = None;
        }
        if candidates.admitted.is_empty() && candidates.excluded.iter().any(|(_, why)| never(why)) {
            self.live.notice = Some(NEVER_LEFT.into());
            return true;
        }
        false
    }

    /// What router v0's policy reads besides System One's judgment.
    fn situation(&self, facts: &Facts, candidates: &Candidates) -> crate::router::Situation {
        let admitted = candidates.admitted.clone();
        let rank = |harness: Harness| facts.leaning.stances.get(harness).rank();
        let preferred = admitted.first().copied().filter(|first| {
            admitted
                .iter()
                .skip(1)
                .all(|other| rank(*other) > rank(*first))
        });
        let claude_five_hour = admitted
            .contains(&Harness::ClaudeCode)
            .then(|| claude_five_hour(atif::now_ms() / 1000))
            .flatten();
        crate::router::Situation {
            payers: admitted
                .iter()
                .map(|harness| (*harness, payer(*harness, facts)))
                .collect(),
            admitted,
            preferred,
            current: self.live.runs_on,
            claude_five_hour,
            time_weight: self.preferences.time_weight(),
            pins: self.pins(),
        }
    }

    /// At a routing point, holds the message in the input and asks System
    /// One about it on a thread of its own (#11460); [`App::poll_route`]
    /// sends it once the answer or the budget is in. A routing point is a
    /// chat's first message, or a follow-up while no turn runs. The route
    /// question is asked when more than one harness is admitted, and for a
    /// follow-up also when the briefing would be chosen again for new work
    /// (#11473). A chat's first message also asks for its briefing
    /// ([`crate::context`]) at the same time; a follow-up asks for one only
    /// once the router judged it new work. Pins, commands, and messages
    /// sent while a turn runs are not routing points; a pinned chat's
    /// list-shaped message still asks whether it is several pieces of work
    /// ([`App::ask_pinned_parts`]). Returns whether it took the message.
    pub(crate) fn ask_router(&mut self) -> bool {
        if self.routed.is_some()
            || self.route_settled
            || self.pending_route.is_some()
            || self.live.busy
            || self.request.is_some()
            || self.draft.text.trim_start().starts_with('/')
            // A checkpoint already moved the work: the message the person
            // sends next goes where it moved (#11568).
            || self.routing_rule == Some(crate::routing_record::Rule::Escalated)
        {
            return false;
        }
        if self.routing_rule == Some(crate::routing_record::Rule::Pin)
            || (self.live.pinned && (self.live.runs_on.is_some() || self.live.agent.is_some()))
        {
            return self.ask_pinned_parts();
        }
        if self.ask_redo() {
            return true;
        }
        let facts = Facts::read(self);
        let candidates = candidates(&facts);
        // A chat's first message, or a plan's part: a new piece of work
        // whatever the chat holds.
        let first = self.live.user_count() == 0 || self.fresh_work;
        // Only a person's chat chooses a briefing: the terminal and `coder
        // chat`, never an App a helper or an integration test builds.
        let context_on = crate::context::enabled() && (cfg!(test) || self.routing_records);
        let route =
            crate::router::enabled() && (candidates.admitted.len() > 1 || (context_on && !first));
        let library = (context_on && first)
            .then(|| crate::context::library(&self.context_cwd()))
            .flatten();
        let plan_shaped =
            self.dispatch_asks(first) && crate::dispatch::parts(&self.draft.text).is_some();
        if !route && library.is_none() && !plan_shaped {
            return false;
        }
        let mut situation = self.situation(&facts, &candidates);
        if self.fresh_work {
            situation.current = None;
        }
        let earlier = crate::router::earlier(&self.live.entries);
        let receiver =
            route.then(|| crate::router::ask(&earlier, &self.draft.text, situation.follow_up()));
        let context =
            library.map(|library| crate::context::ask(library, &earlier, &self.draft.text));
        let dispatch = self
            .dispatch_asks(first)
            .then(|| crate::dispatch::ask(&earlier, &self.draft.text))
            .flatten();
        self.pending_route = Some(crate::router::Pending {
            receiver,
            answer: None,
            context,
            context_considered: first,
            escalation: None,
            dispatch,
            started: std::time::Instant::now(),
            situation,
            session: self.session_id().map(str::to_owned),
            draft: std::mem::take(&mut self.draft),
            composer: std::mem::take(&mut self.composer),
        });
        true
    }

    /// A pinned chat is never routed, but a list-shaped message in it
    /// still asks System One whether it is several pieces of work and
    /// which part needs which (#11544). The answer runs as a plan or a
    /// fan-out with every part on the pin ([`App::fanout_target`]); no
    /// part moves to another harness. A plan's part keeps the pin's rule
    /// in its routing record. Returns whether it took the message.
    fn ask_pinned_parts(&mut self) -> bool {
        if self.sending_part && self.plan.is_some() {
            self.routing_rule = Some(crate::routing_record::Rule::Pin);
        }
        if !self.dispatch_asks(true) || crate::dispatch::parts(&self.draft.text).is_none() {
            return false;
        }
        let earlier = crate::router::earlier(&self.live.entries);
        let Some(dispatch) = crate::dispatch::ask(&earlier, &self.draft.text) else {
            return false;
        };
        let facts = Facts::read(self);
        let candidates = candidates(&facts);
        let situation = self.situation(&facts, &candidates);
        self.pending_route = Some(crate::router::Pending {
            receiver: None,
            answer: None,
            context: None,
            context_considered: true,
            escalation: None,
            dispatch: Some(dispatch),
            started: std::time::Instant::now(),
            situation,
            session: self.session_id().map(str::to_owned),
            draft: std::mem::take(&mut self.draft),
            composer: std::mem::take(&mut self.composer),
        });
        true
    }

    /// A message that asks for the same thing as the chat's last turn is a
    /// checkpoint (#11474): it waits for the escalation call instead of a
    /// route, and a move hands the work over with a briefing.
    fn ask_redo(&mut self) -> bool {
        let Some(current) = self.live.runs_on else {
            return false;
        };
        let same = self.last_routed.as_ref().is_some_and(|last| {
            !last.prompt.trim().is_empty() && last.prompt.trim() == self.draft.text.trim()
        });
        if !same
            || self.live.moved
            || !crate::escalate::enabled()
            || !(cfg!(test) || self.routing_records)
        {
            return false;
        }
        let facts = Facts::read(self);
        let candidates = candidates(&facts);
        if crate::escalate::stronger(current, &candidates.admitted).is_none() {
            return false;
        }
        let situation = self.situation(&facts, &candidates);
        let entries = &self.live.entries;
        let evidence = crate::escalate::evidence(crate::escalate::last_turn(entries));
        let task = self.draft.text.clone();
        let earlier = crate::router::earlier(entries);
        let receiver = crate::escalate::ask(crate::escalate::state(
            &earlier,
            &task,
            &crate::escalate::harness_words(self, current),
            crate::escalate::Checkpoint::Redo,
            &evidence,
        ));
        let escalation = crate::escalate::Pending {
            receiver: Some(receiver),
            answer: None,
            started: std::time::Instant::now(),
            checkpoint: crate::escalate::Checkpoint::Redo,
            situation: crate::escalate::Situation {
                current,
                admitted: candidates.admitted.clone(),
                claude_five_hour: situation.claude_five_hour,
                retried: self.live.retried,
                moved: self.live.moved,
            },
            task,
            evidence,
            turn: self.last_routed.as_ref().map(|last| last.id.clone()),
            session: self.session_id().map(str::to_owned),
        };
        self.pending_route = Some(crate::router::Pending {
            receiver: None,
            answer: None,
            context: None,
            context_considered: true,
            escalation: Some(escalation),
            dispatch: None,
            started: std::time::Instant::now(),
            situation,
            session: self.session_id().map(str::to_owned),
            draft: std::mem::take(&mut self.draft),
            composer: std::mem::take(&mut self.composer),
        });
        true
    }

    /// The folder a chat's briefing is chosen for.
    pub(crate) fn context_cwd(&self) -> std::path::PathBuf {
        self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
    }

    /// Once the route's answer judged a follow-up new work, asks for its
    /// briefing (#11473); at most once per held message.
    fn consider_context(&mut self) {
        let Some(pending) = self.pending_route.as_ref() else {
            return;
        };
        if pending.context_considered {
            return;
        }
        let new_work = pending.new_work();
        let latest = pending.draft.text.clone();
        let library =
            (new_work && crate::context::enabled() && (cfg!(test) || self.routing_records))
                .then(|| crate::context::library(&self.context_cwd()))
                .flatten();
        let earlier = crate::router::earlier(&self.live.entries);
        let dispatch = (new_work && self.dispatch_asks(true))
            .then(|| crate::dispatch::ask(&earlier, &latest))
            .flatten();
        let Some(pending) = self.pending_route.as_mut() else {
            return;
        };
        pending.context_considered = true;
        pending.context = library.map(|library| crate::context::ask(library, &earlier, &latest));
        pending.dispatch = dispatch;
    }

    /// Whether a routing point for new work asks if the message is several
    /// pieces of work: a person's chat, no plan under way.
    fn dispatch_asks(&self, new_work: bool) -> bool {
        new_work
            && self.plan.is_none()
            && !self.sending_part
            && crate::dispatch::enabled()
            && (cfg!(test) || self.routing_records)
    }

    /// Whether a message waits for System One's judgment.
    #[must_use]
    pub fn routing_pending(&self) -> bool {
        self.pending_route.is_some()
    }

    /// Sends the held message once System One answered or the budget ran
    /// out. Called on every pass of the terminal's loop.
    pub fn poll_route(&mut self) {
        if !self
            .pending_route
            .as_mut()
            .is_some_and(crate::router::Pending::poll_route)
        {
            return;
        }
        self.consider_context();
        if self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.context.as_mut())
            .is_some_and(|asking| !asking.settled())
        {
            return;
        }
        if self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.escalation.as_mut())
            .is_some_and(|escalation| !escalation.settled())
        {
            return;
        }
        if self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.dispatch.as_mut())
            .is_some_and(|asking| !asking.settled())
        {
            return;
        }
        self.finish_route();
    }

    /// Waits for the held message's judgment and sends it, for callers
    /// with no screen to keep drawing (`coder chat`).
    pub fn settle_route(&mut self) {
        let Some(pending) = self.pending_route.as_mut() else {
            return;
        };
        pending.wait_route();
        self.consider_context();
        if let Some(asking) = self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.context.as_mut())
        {
            asking.settle();
        }
        if let Some(escalation) = self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.escalation.as_mut())
        {
            escalation.settle();
        }
        if let Some(asking) = self
            .pending_route
            .as_mut()
            .and_then(|pending| pending.dispatch.as_mut())
        {
            asking.settle();
        }
        self.finish_route();
    }

    /// The message waiting for its route, as the transcript shows it
    /// until the turn starts.
    #[must_use]
    pub fn held_message(&self) -> Option<&str> {
        self.pending_route
            .as_ref()
            .map(|pending| pending.draft.text.as_str())
    }

    /// Takes back the message waiting for its route: Esc, or the chat on
    /// screen changing. It goes back in the input, ahead of anything typed
    /// meanwhile, and nothing is sent. Returns whether there was one.
    pub(crate) fn cancel_route(&mut self) -> bool {
        let Some(pending) = self.pending_route.take() else {
            return false;
        };
        let typed = std::mem::replace(&mut self.draft, pending.draft);
        let typed_composer = std::mem::replace(&mut self.composer, pending.composer);
        if !typed.text.is_empty() {
            self.draft.text.push('\n');
            self.draft.text.push_str(&typed.text);
        }
        self.draft.cursor = self.draft.text.len();
        self.composer.images.extend(typed_composer.images);
        true
    }

    /// Before another chat replaces this one (`/resume`, a new chat, a
    /// followed chat): the waiting message is not sent anywhere, and Up in
    /// the input brings it back.
    pub(crate) fn drop_route(&mut self) {
        if self.cancel_route() {
            self.record_prompt();
        }
    }

    fn finish_route(&mut self) {
        // The answer belongs to the chat the message was sent in. Another
        // chat on screen gets nothing: the message goes back to the input.
        if self.pending_route.as_ref().is_some_and(|pending| {
            self.selected_agent.is_some()
                || self.mode != Mode::Live
                || pending
                    .session
                    .as_deref()
                    .is_some_and(|session| Some(session) != self.session_id())
        }) {
            self.cancel_route();
            return;
        }
        let Some(mut pending) = self.pending_route.take() else {
            return;
        };
        // A list-shaped ask System One judged several pieces of work runs
        // as a plan, each part routed on its own (#11474).
        if let Some((plan, needs)) = pending
            .dispatch
            .take()
            .and_then(crate::dispatch::Asking::judged)
        {
            // Parts that don't need each other run at the same time, each
            // in its own worktree (#11539).
            match crate::fanout::Graph::fan(&plan.parts, needs)
                .filter(|_| crate::fanout::enabled() && self.can_fan_out())
            {
                Some(graph) => self.start_fanout(pending.draft.text, plan, graph),
                None => self.start_plan(pending.draft.text, plan),
            }
            return;
        }
        // A new piece of work starts here: its first message, a plan's
        // part, or a follow-up the router judged new (#11474).
        if pending.situation.current.is_none() || pending.new_work() {
            self.live.work_started = self.live.user_count();
            self.live.retried = false;
            self.live.moved = false;
        }
        if let Some(escalation) = pending.escalation.take() {
            pending.draft = self.redo_call(&escalation, pending.draft);
        }
        if pending.receiver.is_some() {
            self.routed = Some(crate::router::decide(
                pending
                    .answer
                    .unwrap_or_else(|| Err("System One did not answer in time.".into())),
                &pending.situation,
                pending.started.elapsed(),
            ));
        }
        // A new briefing for a new piece of work; when System One chose
        // nothing in time, each harness loads what it loaded before.
        if let Some(asking) = pending.context {
            let (briefing, trace) = asking.decide();
            self.live.briefing = briefing;
            self.routing_context = Some(trace);
        }
        // Put the held message back in the input and send it; what the
        // person typed meanwhile comes back after.
        let typed = std::mem::replace(&mut self.draft, pending.draft);
        let typed_composer = std::mem::replace(&mut self.composer, pending.composer);
        self.route_settled = true;
        self.submit_live();
        self.route_settled = false;
        // A path that did not route the message leaves no call behind.
        self.routed = None;
        if self.draft.text.is_empty() && self.composer.images.is_empty() {
            self.draft = typed;
            self.composer = typed_composer;
        } else if !typed.text.is_empty() {
            self.draft.text.push('\n');
            self.draft.text.push_str(&typed.text);
            self.draft.cursor = self.draft.text.len();
        }
    }

    /// Acts on a redo's checkpoint call: a move sends the person's own
    /// message, as they typed it, on the stronger harness. Coder never
    /// replaces it with words they didn't write (owner, 2026-10-13); the
    /// stronger harness reads the chat so far.
    fn redo_call(
        &mut self,
        escalation: &crate::escalate::Pending,
        draft: crate::Draft,
    ) -> crate::Draft {
        let (decision, probabilities, ms, missing) = escalation.decide();
        self.record_escalation(escalation, decision, probabilities.clone(), ms, missing);
        let crate::escalate::Call::To(to) = decision.call else {
            return draft;
        };
        self.live.moved = true;
        self.live.runs_on = Some(to);
        // A move to a stronger harness is no longer a cheap answer.
        self.live.cheap = false;
        self.routing_rule = Some(crate::routing_record::Rule::Escalated);
        self.routing_router = Some((
            probabilities,
            crate::router::Trace {
                question_set: crate::escalate::QUESTION_SET.into(),
                policy: crate::escalate::POLICY.into(),
                step: Some(decision.step.name()),
                follow_up: true,
                ms,
                missing: None,
            },
        ));
        draft
    }

    /// Whether this chat runs on Claude Code.
    #[must_use]
    pub fn on_claude(&self) -> bool {
        self.mode == Mode::Live && self.live.runs_on == Some(Harness::ClaudeCode)
    }

    /// Whether this chat runs on Codex (#11510).
    #[must_use]
    pub fn on_codex(&self) -> bool {
        self.mode == Mode::Live && self.live.runs_on == Some(Harness::Codex)
    }

    /// Whether this chat runs on OpenCode (#11507).
    #[must_use]
    pub fn on_opencode(&self) -> bool {
        self.mode == Mode::Live && self.live.runs_on == Some(Harness::OpenCode)
    }

    /// Whether this chat runs Coder's own loop: a live chat not on Claude
    /// Code, OpenCode or Codex, including one whose first turn has not been
    /// routed yet.
    #[must_use]
    pub fn on_coder_loop(&self) -> bool {
        self.mode == Mode::Live
            && !matches!(
                self.live.runs_on,
                Some(Harness::ClaudeCode | Harness::OpenCode | Harness::Codex)
            )
    }

    /// Starts the draft's turn on `harness` and keeps the chat on it: a
    /// pin (`coder chat --mode claude`) or the router's pick. Switching a
    /// chat that ran elsewhere onto Claude Code starts its session from the
    /// transcript ([`crate::live::Chat::claude_resume`]).
    ///
    /// # Errors
    /// The harness takes no chat turns yet.
    pub fn submit_on(&mut self, harness: Harness) -> Result<(), &'static str> {
        if harness.declaration().turn == Turn::NotYet {
            return Err("That harness takes no chat turns yet.");
        }
        if self.mode != Mode::Live {
            self.set_mode(Mode::Live);
        }
        self.live.runs_on = Some(harness);
        self.live.pinned = true;
        self.routing_rule = Some(crate::routing_record::Rule::Pin);
        self.submit_live();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            local: true,
            claude_plugin_on: true,
            claude: Health {
                signed_in: true,
                at_limit: false,
            },
            codex_on: true,
            codex: Health {
                signed_in: true,
                at_limit: false,
            },
            opencode_on: false,
            opencode: crate::harness_plugin::Health {
                signed_in: false,
                at_limit: false,
            },
            openrouter_key: false,
            leaning: crate::routing_preferences::Leaning::default(),
            failed: Failed::default(),
        }
    }

    #[test]
    fn the_shared_declaration_names_the_claude_plugin_coder_turns_on() {
        assert_eq!(
            Harness::ClaudeCode.declaration().plugin,
            Some(crate::claude::PLUGIN)
        );
    }

    #[test]
    fn every_harness_is_declared_once() {
        for harness in [
            Harness::ClaudeCode,
            Harness::OpenCode,
            Harness::CoderLoop,
            Harness::Codex,
        ] {
            assert_eq!(
                DECLARATIONS.iter().filter(|d| d.harness == harness).count(),
                1
            );
            assert_eq!(harness.declaration().harness, harness);
        }
    }

    /// The harness order (#11507): Claude Code when it is on and signed in,
    /// otherwise OpenCode when it is installed and on, otherwise Coder.
    #[test]
    fn opencode_comes_after_claude_code_and_before_the_coder_loop() {
        let mut both = facts();
        both.opencode_on = true;
        both.opencode.signed_in = true;
        assert_eq!(
            candidates(&both).admitted,
            [
                Harness::ClaudeCode,
                Harness::OpenCode,
                Harness::CoderLoop,
                Harness::Codex
            ]
        );
        assert_eq!(fixed_rule(&candidates(&both)), Harness::ClaudeCode);
        let mut claude_off = both;
        claude_off.claude_plugin_on = false;
        assert_eq!(fixed_rule(&candidates(&claude_off)), Harness::OpenCode);
        let mut claude_spent = both;
        claude_spent.claude.at_limit = true;
        assert_eq!(fixed_rule(&candidates(&claude_spent)), Harness::OpenCode);
        let mut missing = claude_off;
        missing.opencode.signed_in = false;
        assert_eq!(
            exclusion(Harness::OpenCode, &missing),
            Some(Exclusion::NotInstalled)
        );
        assert_eq!(fixed_rule(&candidates(&missing)), Harness::CoderLoop);
        let mut off = claude_off;
        off.opencode_on = false;
        assert_eq!(
            exclusion(Harness::OpenCode, &off),
            Some(Exclusion::PluginOff)
        );
        let mut limited = claude_off;
        limited.opencode.at_limit = true;
        assert_eq!(fixed_rule(&candidates(&limited)), Harness::CoderLoop);
        let mut phone = both;
        phone.local = false;
        assert_eq!(
            exclusion(Harness::OpenCode, &phone),
            Some(Exclusion::NotHere)
        );
        assert_eq!(payer(Harness::OpenCode, &both), Payer::OpenCode);
    }

    #[test]
    fn a_signed_in_computer_offers_every_harness_in_order() {
        let found = candidates(&facts());
        assert_eq!(
            found.admitted,
            [Harness::ClaudeCode, Harness::CoderLoop, Harness::Codex]
        );
        assert_eq!(found.excluded, [(Harness::OpenCode, Exclusion::PluginOff)]);
    }

    /// Codex is a bundled plugin that can be turned off, needs its sign-in,
    /// and steps aside at a plan limit (#11510).
    #[test]
    fn each_reason_leaves_codex_out() {
        let mut off = facts();
        off.codex_on = false;
        assert_eq!(exclusion(Harness::Codex, &off), Some(Exclusion::PluginOff));
        let mut signed_out = facts();
        signed_out.codex.signed_in = false;
        assert_eq!(
            exclusion(Harness::Codex, &signed_out),
            Some(Exclusion::NotSignedIn)
        );
        let mut limited = facts();
        limited.codex.at_limit = true;
        assert_eq!(
            exclusion(Harness::Codex, &limited),
            Some(Exclusion::AtPlanLimit)
        );
        assert_eq!(payer(Harness::Codex, &facts()), Payer::ChatGptLogin);
    }

    #[test]
    fn each_reason_leaves_claude_code_out() {
        let mut off = facts();
        off.claude_plugin_on = false;
        assert_eq!(
            exclusion(Harness::ClaudeCode, &off),
            Some(Exclusion::PluginOff)
        );
        let mut signed_out = facts();
        signed_out.claude.signed_in = false;
        assert_eq!(
            exclusion(Harness::ClaudeCode, &signed_out),
            Some(Exclusion::NotSignedIn)
        );
        let mut limited = facts();
        limited.claude.at_limit = true;
        assert_eq!(
            exclusion(Harness::ClaudeCode, &limited),
            Some(Exclusion::AtPlanLimit)
        );
        let mut phone = facts();
        phone.local = false;
        assert_eq!(
            exclusion(Harness::ClaudeCode, &phone),
            Some(Exclusion::NotHere)
        );
    }

    /// On a phone or the web, or with nothing local signed in, the gateway
    /// is still there (#11456).
    #[test]
    fn the_coder_loop_is_always_a_candidate() {
        let mut nothing = facts();
        nothing.local = false;
        nothing.claude_plugin_on = false;
        nothing.codex_on = false;
        assert_eq!(candidates(&nothing).admitted, [Harness::CoderLoop]);
    }

    #[test]
    fn the_coder_loop_pays_with_the_persons_key_when_set() {
        assert_eq!(payer(Harness::CoderLoop, &facts()), Payer::GatewayCredits);
        let mut keyed = facts();
        keyed.openrouter_key = true;
        assert_eq!(payer(Harness::CoderLoop, &keyed), Payer::OpenRouterKey);
        assert_eq!(payer(Harness::ClaudeCode, &keyed), Payer::ClaudePlan);
    }

    /// #11550: the payer an ended turn records is the one the model it ran
    /// says paid: the Codex plan for a Codex login's model, the Claude plan
    /// for Claude Code's, the person's key or gateway credit otherwise.
    #[test]
    fn the_payer_agrees_with_the_model_that_answered() {
        let loop_paid = |model: &str, byok: bool| paid_by(Harness::CoderLoop, false, model, byok);
        assert_eq!(loop_paid("codex:gpt-6-luna", true), Payer::ChatGptLogin);
        assert_eq!(loop_paid("gpt-6.1-sol", true), Payer::ChatGptLogin);
        assert_eq!(loop_paid("claude-fable-5-1", true), Payer::ClaudePlan);
        assert_eq!(
            loop_paid("anthropic/claude-fable-5.1", true),
            Payer::OpenRouterKey
        );
        assert_eq!(
            loop_paid("anthropic/claude-fable-5.1", false),
            Payer::GatewayCredits
        );
        assert_eq!(loop_paid("openagents/fast", true), Payer::GatewayCredits);
        assert_eq!(loop_paid("opencode/big-pickle", false), Payer::OpenCode);
        assert_eq!(
            paid_by(Harness::CoderLoop, true, "grok-4", false),
            Payer::AgentLogin
        );
        assert_eq!(
            paid_by(Harness::ClaudeCode, false, "anthropic/x", true),
            Payer::ClaudePlan
        );
    }

    #[test]
    fn a_turn_starts_on_the_chosen_harness() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.draft.text = "hello".into();
        app.draft.cursor = 5;
        app.submit_on(Harness::CoderLoop).unwrap();
        assert_eq!(app.mode, Mode::Live);
        assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
        // Codex takes chat turns (#11510): a pin starts one on it.
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.draft.text = "hello".into();
        app.draft.cursor = 5;
        app.submit_on(Harness::Codex).unwrap();
        assert_eq!(app.live.runs_on, Some(Harness::Codex));
        assert!(app.on_codex() && !app.on_coder_loop() && !app.on_claude());
        assert!(matches!(
            app.request.as_ref().map(|request| &request.kind),
            Some(crate::live::Work::Codex { session: None, .. })
        ));
    }

    #[test]
    fn the_fixed_rule_takes_claude_code_first_and_the_coder_loop_otherwise() {
        assert_eq!(fixed_rule(&candidates(&facts())), Harness::ClaudeCode);
        let mut off = facts();
        off.claude_plugin_on = false;
        assert_eq!(fixed_rule(&candidates(&off)), Harness::CoderLoop);
    }

    /// A chat's first turn is routed; its follow-ups stay where it went,
    /// and a new chat is routed again.
    #[test]
    fn a_chat_stays_on_the_harness_its_first_turn_took() {
        crate::harness_plugin::sign_in_claude(true);
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.plugins.bundled.claude = true;
        assert_eq!(app.route_turn(), Harness::ClaudeCode);
        assert!(app.on_claude());
        crate::harness_plugin::sign_in_claude(false);
        assert_eq!(app.route_turn(), Harness::ClaudeCode);
        app.live = Default::default();
        assert_eq!(app.route_turn(), Harness::CoderLoop);
        assert!(app.on_coder_loop());
    }

    /// A pinned chat reopened in a new process (`coder chat --session`
    /// without `--mode`, #11525) keeps its pin on every harness: its
    /// follow-up is not routed, even one System One would call new work.
    /// An unpinned chat's follow-up is a routing point, where a
    /// continuation keeps the harness and new work is routed.
    #[test]
    fn a_reopened_pinned_chat_stays_on_its_harness() {
        let map = |pairs: &[(&str, f64)]| {
            pairs
                .iter()
                .map(|(name, p)| ((*name).to_owned(), *p))
                .collect()
        };
        crate::router::set_test_judge(Some(Some(crate::router::Recorded {
            kind: map(&[("repo_change", 0.9), ("answer", 0.1)]),
            size: map(&[("medium", 0.9), ("small", 0.1)]),
            needs_repo: 0.9,
            own_tools: 0.9,
            risk: map(&[("low", 0.9), ("medium", 0.1)]),
            new_work: Some(0.9),
        })));
        crate::harness_plugin::sign_in_claude(true);
        for harness in [Harness::ClaudeCode, Harness::CoderLoop, Harness::Codex] {
            for pinned in [true, false] {
                let chat = crate::live::Chat {
                    runs_on: Some(harness),
                    pinned,
                    ..Default::default()
                };
                let saved =
                    crate::trajectory::document(&chat, "s", "m", std::path::Path::new("/w"));
                let mut app = App::default();
                app.set_mode(Mode::Live);
                app.plugins.bundled.claude = true;
                crate::trajectory::restore_app(&mut app, &saved).unwrap();
                assert_eq!(app.live.pinned, pinned, "{harness:?}");
                app.draft.text = "now something else".into();
                assert_eq!(app.ask_router(), !pinned, "{harness:?} pinned {pinned}");
                if pinned {
                    assert_eq!(app.route_turn(), harness);
                }
            }
        }
        crate::harness_plugin::sign_in_claude(false);
        crate::router::set_test_judge(None);
    }
}
