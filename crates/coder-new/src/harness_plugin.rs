//! Harness plugins: the engines a Coder turn can run on, each declared the
//! same way so a router can choose among them (#11451, #11452).
//!
//! A harness is what runs a piece of work: Claude Code on this computer's
//! Claude plan, Coder's own loop on the OpenAgents gateway, Codex on this
//! computer's ChatGPT login. Each one declares where it runs, who pays,
//! what it can do, and a cheap health check. Solver and background agents
//! are not harnesses: they are Coder tools any harness can call.
//!
//! Candidates are built in code before anything ranks them
//! (`docs/api/2026-10-02-agentic-execution-router.md`, section 4):
//! [`candidates`] keeps the harnesses that are on, healthy and runnable
//! here, and gives every other one a typed [`Exclusion`]. Choosing among the
//! candidates is the router's job (phase 2); until it lands, [`fixed_rule`]
//! takes the first one. [`App::submit_on`] starts a turn on whichever is
//! chosen.
//!
//! There is no mode switch (#11453): a chat runs on the harness chosen for
//! its first turn ([`crate::live::Chat::runs_on`]) and stays on it for
//! follow-ups, so its session and prompt cache carry on.

use crate::{App, Mode, plugin_tools::ExecutionSettings};

/// A harness's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    /// Claude Code through the Claude Agent SDK (`crate::claude`,
    /// `crate::claude_chat`): one long-lived process per chat, every call
    /// approved, the person's settings loaded, Coder's tools over MCP.
    ClaudeCode,
    /// Coder's own loop (`crate::live`) on the OpenAgents gateway, or on
    /// the person's OpenRouter key, with Coder's tools.
    CoderLoop,
    /// Codex on this computer's ChatGPT login. Today it runs only as a
    /// background agent (`crate::fleet`, an ACP agent with the id `codex`);
    /// a chat turn on it is not built yet ([`Turn::NotYet`]).
    Codex,
}

/// Where a harness runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    ThisComputer,
    LinkedComputer,
    Gateway,
}

/// Who pays for a harness's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
}

/// What a harness can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Can {
    Answer,
    RepoEdits,
    Shell,
    IssueToPullRequest,
    BackgroundWork,
    Images,
}

/// How a harness takes a chat's turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Turn {
    /// A long-lived session per chat, which later turns continue.
    Session,
    /// One request per turn over the chat's transcript.
    Transcript,
    /// No chat turn yet; the harness runs only as background work.
    NotYet,
}

/// One harness's declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Declaration {
    pub harness: Harness,
    pub name: &'static str,
    /// The plugin in /plugins that turns it on, when one does.
    pub plugin: Option<&'static str>,
    pub place: Place,
    pub can: &'static [Can],
    pub turn: Turn,
}

/// Every harness Coder knows, in the order a fixed rule prefers them.
pub const DECLARATIONS: [Declaration; 3] = [
    Declaration {
        harness: Harness::ClaudeCode,
        name: "Claude Code",
        plugin: Some(crate::claude::PLUGIN),
        place: Place::ThisComputer,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
            Can::Images,
        ],
        turn: Turn::Session,
    },
    Declaration {
        harness: Harness::CoderLoop,
        name: "Coder",
        plugin: None,
        place: Place::Gateway,
        can: &[
            Can::Answer,
            Can::RepoEdits,
            Can::Shell,
            Can::IssueToPullRequest,
            Can::BackgroundWork,
        ],
        turn: Turn::Transcript,
    },
    Declaration {
        harness: Harness::Codex,
        name: "Codex",
        plugin: Some("acp-subagents"),
        place: Place::ThisComputer,
        can: &[Can::RepoEdits, Can::Shell, Can::BackgroundWork],
        turn: Turn::NotYet,
    },
];

impl Harness {
    /// This harness's declaration.
    #[must_use]
    pub fn declaration(self) -> &'static Declaration {
        DECLARATIONS
            .iter()
            .find(|declaration| declaration.harness == self)
            .expect("every harness is declared")
    }
}

/// Why a harness is not a candidate for this chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Exclusion {
    /// Its plugin is off.
    PluginOff,
    /// It needs a sign-in this computer doesn't hold.
    NotSignedIn,
    /// Its plan reported a window at its limit.
    AtPlanLimit,
    /// It runs on this computer, and the chat is somewhere else.
    NotHere,
    /// It takes no chat turns yet.
    NoChatTurn,
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
    /// Codex is an enabled background engine here.
    pub codex_on: bool,
    pub codex: Health,
    /// The person's OpenRouter key is on and set; the Coder loop then pays
    /// with it rather than gateway credits.
    pub openrouter_key: bool,
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
        let claude_plugin_on = app.plugins.bundled.claude;
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
            codex_on: crate::fleet::engines(execution)
                .iter()
                .any(|engine| engine == "codex"),
            // An enabled, valid ACP agent is one whose program resolved.
            codex: Health {
                signed_in: true,
                at_limit: false,
            },
            openrouter_key: app.plugins.enabled && app.plugins.key_configured,
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

/// Who pays when `harness` runs under `facts`.
#[must_use]
pub fn payer(harness: Harness, facts: &Facts) -> Payer {
    match harness {
        Harness::ClaudeCode => Payer::ClaudePlan,
        Harness::Codex => Payer::ChatGptLogin,
        Harness::CoderLoop if facts.openrouter_key => Payer::OpenRouterKey,
        Harness::CoderLoop => Payer::GatewayCredits,
    }
}

/// Why `harness` is left out under `facts`, or `None` when it is a
/// candidate.
#[must_use]
pub fn exclusion(harness: Harness, facts: &Facts) -> Option<Exclusion> {
    let declaration = harness.declaration();
    if declaration.place == Place::ThisComputer && !facts.local {
        return Some(Exclusion::NotHere);
    }
    match harness {
        Harness::ClaudeCode if !facts.claude_plugin_on => Some(Exclusion::PluginOff),
        Harness::ClaudeCode if !facts.claude.signed_in => Some(Exclusion::NotSignedIn),
        Harness::ClaudeCode if facts.claude.at_limit => Some(Exclusion::AtPlanLimit),
        Harness::Codex if !facts.codex_on => Some(Exclusion::PluginOff),
        Harness::Codex if !facts.codex.signed_in => Some(Exclusion::NotSignedIn),
        Harness::Codex if facts.codex.at_limit => Some(Exclusion::AtPlanLimit),
        _ if declaration.turn == Turn::NotYet => Some(Exclusion::NoChatTurn),
        // The gateway is always there; a failed call is the turn's error.
        _ => None,
    }
}

/// The harnesses that can take this chat's next turn, in declaration order,
/// and why each other one was left out.
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
    Candidates { admitted, excluded }
}

/// The selector until the router lands (phase 2): the first admitted
/// candidate, which is Claude Code when its plugin is on and healthy, and
/// otherwise the Coder loop, which is always there.
#[must_use]
pub fn fixed_rule(candidates: &Candidates) -> Harness {
    candidates
        .admitted
        .first()
        .copied()
        .unwrap_or(Harness::CoderLoop)
}

impl App {
    /// The harness this chat's next turn runs on: the one it already runs
    /// on, or, for a chat's first turn, the one the selector picks. Each new
    /// chat is a new piece of work and is routed afresh; follow-ups stay on
    /// its harness (#11451, owner decision 1). Routing at other work
    /// boundaries inside a chat comes later.
    pub fn route_turn(&mut self) -> Harness {
        if let Some(harness) = self.live.runs_on {
            return harness;
        }
        let harness = fixed_rule(&candidates(&Facts::read(self)));
        self.live.runs_on = Some(harness);
        harness
    }

    /// Whether this chat runs on Claude Code.
    #[must_use]
    pub fn on_claude(&self) -> bool {
        self.mode == Mode::Live && self.live.runs_on == Some(Harness::ClaudeCode)
    }

    /// Whether this chat runs Coder's own loop: a live chat not on Claude
    /// Code, including one whose first turn has not been routed yet.
    #[must_use]
    pub fn on_coder_loop(&self) -> bool {
        self.mode == Mode::Live && self.live.runs_on != Some(Harness::ClaudeCode)
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
            return Err("Codex runs as a background agent; it takes no chat turns yet.");
        }
        if self.mode != Mode::Live {
            self.set_mode(Mode::Live);
        }
        self.live.runs_on = Some(harness);
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
            openrouter_key: false,
        }
    }

    #[test]
    fn every_harness_is_declared_once() {
        for harness in [Harness::ClaudeCode, Harness::CoderLoop, Harness::Codex] {
            assert_eq!(
                DECLARATIONS.iter().filter(|d| d.harness == harness).count(),
                1
            );
            assert_eq!(harness.declaration().harness, harness);
        }
    }

    #[test]
    fn a_signed_in_computer_offers_claude_code_and_the_coder_loop() {
        let found = candidates(&facts());
        assert_eq!(found.admitted, [Harness::ClaudeCode, Harness::CoderLoop]);
        assert_eq!(found.excluded, [(Harness::Codex, Exclusion::NoChatTurn)]);
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

    #[test]
    fn a_turn_starts_on_the_chosen_harness() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app.draft.text = "hello".into();
        app.draft.cursor = 5;
        app.submit_on(Harness::CoderLoop).unwrap();
        assert_eq!(app.mode, Mode::Live);
        assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
        assert!(app.submit_on(Harness::Codex).is_err());
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
}
