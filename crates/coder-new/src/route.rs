//! The doors an `auto` turn may answer through (#11374): every way this
//! computer can reach a model, in the order the person's cost, speed, and
//! intelligence preferences put them, with the ones already known to be out
//! of credit skipped. A turn walks the order until a door answers; when
//! none can, [`Plan::explain`] says why in plain words rather than
//! repeating a provider's own refusal text.

use crate::plugin_tools::ExecutionSettings;

/// What the person asked Coder to favor under `/preferences`: each of
/// cost, speed, and intelligence from 1 to 5. The default `3` keeps the
/// doors in their standing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Favor {
    pub cost: u8,
    pub speed: u8,
    pub intelligence: u8,
}

impl Default for Favor {
    fn default() -> Self {
        Favor {
            cost: crate::preferences::DEFAULT,
            speed: crate::preferences::DEFAULT,
            intelligence: crate::preferences::DEFAULT,
        }
    }
}

/// A cheap answer (#11537): the router judged the message a small answer
/// that needs no repository, so the turn runs on a cheap model, never
/// Opus or Sonnet: the gateway's quick router on the signed-in account's
/// free daily allowance, else the cheapest model on the login that
/// answers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cheap {
    /// The gateway model the turn asks for, `None` while signed out.
    pub gateway_model: Option<String>,
}

/// A no-harness turn (#11548): Auto found no harness that can run here
/// (or `CODER_NO_HARNESS` forced it), so the Coder loop runs every call on
/// [`coder_sdk::auto_policy::NoHarnessModel`].
#[derive(Clone, Debug)]
pub struct NoHarness {
    pub policy: coder_sdk::auto_policy::AutoPolicy,
    /// The person's own Gemini API key, when one is set here.
    pub gemini_key: Option<model_access::ApiKey>,
    /// The Gemini API's OpenAI-compatible endpoint
    /// ([`coder_sdk::auto_policy::GEMINI_OPENAI_BASE`]).
    pub gemini_base: String,
}

impl NoHarness {
    /// The model every call runs on.
    #[must_use]
    pub fn model(&self) -> &str {
        self.policy.model_for(coder_sdk::auto_policy::Call::Work)
    }
}

/// The Codex login's cheapest model, for a cheap answer.
pub const CHEAP_CODEX_MODEL: &str = "gpt-6-luna";

/// The Claude plan's cheapest model, for a cheap answer.
pub const CHEAP_CLAUDE_MODEL: &str = crate::claude::MODEL;

/// A picked model this computer's own plan can run (#11550): the Claude
/// plan for Anthropic's Claude models, the Codex plan for the Codex
/// models, each by the login's own model id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanPick {
    pub login: coder::local_chain::PlanLogin,
    /// The login's id for the model, such as `claude-fable-5-1`.
    pub model: String,
    /// The effort the pick named after `:`, such as `high`.
    pub effort: Option<String>,
}

/// Which plan runs `model` (an OpenRouter or gateway id such as
/// `anthropic/claude-fable-5.1:high`), when `claude` (the Claude Code
/// plugin is on and its login is here) or `codex` (the same for Codex)
/// says that plan is on this computer. The order a picked Claude or Codex
/// model is paid in is: the plan, then the person's own key, then gateway
/// credit. The id is matched exactly against the logins' model lists.
#[must_use]
pub fn plan_pick(model: &str, claude: bool, codex: bool) -> Option<PlanPick> {
    let (id, effort) = match model.split_once(':') {
        Some((id, effort)) => (id, Some(effort.to_owned()).filter(|e| !e.is_empty())),
        None => (model, None),
    };
    let (provider, name) = id.split_once('/')?;
    let local = name.replace('.', "-");
    match provider {
        "anthropic" if claude => crate::claude::MODELS
            .iter()
            .any(|(known, _, _)| *known == local)
            .then(|| PlanPick {
                login: coder::local_chain::PlanLogin::Claude,
                model: local,
                effort,
            }),
        "openai" if codex => crate::codex::MODELS
            .iter()
            .any(|(known, _, _)| *known == name)
            .then(|| PlanPick {
                login: coder::local_chain::PlanLogin::Codex,
                model: name.to_owned(),
                effort,
            }),
        _ => None,
    }
}

/// [`plan_pick`] for this chat: the plugin is on and its login is on this
/// computer. Tests never ask the computer.
#[must_use]
pub fn chat_plan_pick(model: &str, execution: &ExecutionSettings) -> Option<PlanPick> {
    if cfg!(test) {
        return None;
    }
    let claude = execution.claude && crate::claude::available();
    let codex = execution
        .harness_off(crate::harness_plugin::Harness::Codex)
        .is_none()
        && crate::codex::available();
    plan_pick(model, claude, codex)
}

/// One door an auto turn may answer through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// The OpenAgents inference gateway's own router (`openagents/auto`):
    /// the signed-in account's daily free allowance on Vertex's fast
    /// lane. First when signed in, because it is free and quick.
    Gateway,
    /// This computer's own model logins — the Codex login, then the
    /// Claude Code login, then the OpenAgents cloud — through the local
    /// coding loop, strongest where a subscription is in date.
    Local,
    /// An installed OpenCode agent's turn, on the free `cat-jam-free` model.
    OpenCode,
    /// The person's OpenRouter key, on the model the favor favors
    /// ([`openrouter_model`]).
    OpenRouter,
    /// The person's own Gemini API key, for a no-harness turn's model
    /// (#11548).
    GeminiKey,
    /// Coder's own free answers while signed out (#11645): an anonymous
    /// OpenAgents agent account's daily free tier. Always the last door,
    /// so a signed-out first run answers whatever else is missing.
    FreeTier,
}

impl Lane {
    /// The door's standing: how it scores on each favor axis from 1 to 5
    /// (cheapness, quickness, strength). A lane's score against the
    /// person's favor is the weighted sum; ties keep the standing order.
    fn standing(self) -> (u32, u32, u32) {
        match self {
            Lane::Gateway => (5, 5, 3),
            Lane::Local => (4, 3, 5),
            Lane::OpenCode => (5, 4, 3),
            Lane::OpenRouter => (2, 5, 4),
            Lane::GeminiKey => (3, 4, 4),
            Lane::FreeTier => (5, 3, 1),
        }
    }

    /// The door's score under `favor`: bigger answers first.
    fn score(self, favor: Favor) -> u32 {
        let (cost, speed, intelligence) = self.standing();
        cost * u32::from(favor.cost)
            + speed * u32::from(favor.speed)
            + intelligence * u32::from(favor.intelligence)
    }

    /// The name the turn's ending message gives the door.
    pub fn name(self) -> &'static str {
        match self {
            Lane::Gateway => "the free OpenAgents allowance",
            Lane::Local => "this computer's model logins",
            Lane::OpenCode => "OpenCode",
            Lane::OpenRouter => "the OpenRouter key",
            Lane::GeminiKey => "your Gemini API key",
            Lane::FreeTier => "Coder's free answers",
        }
    }
}

/// The doors `auto` walks this turn, in try order, and why doors it did
/// not try were out, for the message the ending shows when none answered.
pub struct Plan {
    pub lanes: Vec<Lane>,
    /// Plain-words clauses for connected doors the plan skipped, such as
    /// "the OpenAgents account can't answer right now".
    pub notes: Vec<String>,
    /// The door the chat is held to, tried first (#11645): when another
    /// door answers instead, the turn says so in one grey line.
    pub held: Option<Lane>,
    /// The OpenAgents account answers here, so the ending's fix is credit
    /// rather than signing in.
    pub signed_in: bool,
}

/// The doors an auto turn may take, in the order `favor` puts them.
/// `has_key` says an OpenRouter key is connected; `execution` says whether
/// the OpenAgents account is signed in and an enabled OpenCode agent is
/// installed.
///
/// A turn never ends on one door (#11645): a door the chat is held to (the
/// one its earlier answers came through, or a gateway model the person
/// picked) goes first, and every other door that can run follows it, so a
/// failure falls back instead of ending the turn. Signed out, Coder's own
/// free answers ([`Lane::FreeTier`]) are always last.
#[must_use]
pub fn plan(execution: &ExecutionSettings, has_key: bool, favor: Favor) -> Plan {
    let mut plan = doors(execution, has_key, favor);
    let first = if execution.no_harness.is_some() {
        None
    } else if execution.gateway_model.is_some()
        && execution
            .connections
            .as_ref()
            .is_some_and(crate::provider::gateway_open)
    {
        // A gateway model the person picked (#11526) runs there first.
        Some(Lane::Gateway)
    } else {
        // A chat goes on through the door its first answer came through
        // (#11563), first.
        execution.door
    };
    let signed_in = execution
        .connections
        .as_ref()
        .is_some_and(crate::provider::gateway_open);
    // The free answers are for a signed-out chat only.
    let first =
        first.filter(|first| *first != Lane::FreeTier || (execution.free_tier && !signed_in));
    if let Some(first) = first {
        plan.lanes.retain(|lane| *lane != first);
        plan.lanes.insert(0, first);
        plan.held = Some(first);
    }
    plan.signed_in = signed_in;
    if execution.free_tier && !signed_in && !plan.lanes.contains(&Lane::FreeTier) {
        plan.lanes.push(Lane::FreeTier);
    }
    plan
}

/// The doors [`plan`] walks before a held door is moved first and the free
/// answers are added.
fn doors(execution: &ExecutionSettings, has_key: bool, favor: Favor) -> Plan {
    let mut lanes = Vec::new();
    let mut notes = Vec::new();
    // No harness here (#11548): the one model, through the gateway while
    // signed in, then the person's own Gemini key. Never this computer's
    // logins, OpenCode, or an OpenRouter key.
    if let Some(no_harness) = &execution.no_harness {
        let signed_in = match &execution.connections {
            Some(account) if crate::provider::gateway_open(account) => true,
            Some(account) => {
                notes.push(closed_gateway(account));
                false
            }
            None => false,
        };
        let lanes = no_harness
            .policy
            .doors(signed_in, no_harness.gemini_key.is_some())
            .into_iter()
            .map(|door| match door {
                coder_sdk::auto_policy::Door::Gateway => Lane::Gateway,
                coder_sdk::auto_policy::Door::GeminiKey => Lane::GeminiKey,
            })
            .collect::<Vec<_>>();
        // The person's own OpenRouter key answers too, rather than no one
        // (#11645).
        let mut lanes = lanes;
        if has_key {
            lanes.push(Lane::OpenRouter);
        }
        if lanes.is_empty() && !execution.free_tier {
            notes.push(
                "you're not signed in (run coder login) and no GEMINI_API_KEY is set".to_owned(),
            );
        }
        return Plan {
            lanes,
            notes,
            held: None,
            signed_in,
        };
    }
    // A cheap answer tries the doors cheapest first, whatever the favor
    // says: the gateway's free allowance, the logins' cheapest models,
    // OpenCode's free model, OpenRouter's free router.
    let cheap = execution.cheap.is_some();
    match &execution.connections {
        Some(account) if crate::provider::gateway_open(account) => lanes.push(Lane::Gateway),
        Some(account) => notes.push(closed_gateway(account)),
        None => {}
    }
    lanes.push(Lane::Local);
    let opencode = execution
        .agents
        .iter()
        .any(|agent| agent.id == "opencode" && agent.enabled);
    if opencode {
        match opencode_blocked() {
            Some(refusal) => notes.push(format!(
                "OpenCode is rate limited until {}",
                crate::long_session::clock(refusal.until * 1000)
            )),
            None => lanes.push(Lane::OpenCode),
        }
    }
    if has_key {
        lanes.push(Lane::OpenRouter);
    }
    // Standing order is the push order; sort_by_key is stable, so equal
    // scores keep it.
    if !cheap {
        lanes.sort_by_key(|lane| std::cmp::Reverse(lane.score(favor)));
    }
    Plan {
        lanes,
        notes,
        held: None,
        signed_in: false,
    }
}

/// Why the plan skipped `account`'s gateway: the 402's own "needs credit"
/// sentence while a used-up allowance is held (#11640), else that the
/// account can't answer.
fn closed_gateway(account: &openagents_login::Saved) -> String {
    crate::provider::gateway_needs_credit(account)
        .unwrap_or_else(|| "the OpenAgents account can't answer right now".to_owned())
}

/// The variable that sets how long a fallback door may take to show its
/// first sign of life, in seconds; `0` turns the limit off.
pub const FIRST_EVENT_VAR: &str = "CODER_DOOR_FIRST_EVENT_SECS";

/// The default for [`FIRST_EVENT_VAR`].
pub const FIRST_EVENT_DEFAULT: u64 = 30;

/// How long a fallback door (any door after the first) may take before
/// its first sign of life — streamed text, thinking, a tool call, or
/// usage — before `auto` stops it and tries the next one (#11640). A turn
/// that can't run answers at once rather than waiting out a door that
/// never starts. `None` when [`FIRST_EVENT_VAR`] is `0`.
#[must_use]
pub fn first_event_limit() -> Option<std::time::Duration> {
    #[cfg(test)]
    if let Some(limit) = TEST_FIRST_EVENT.with(std::cell::Cell::get) {
        return limit;
    }
    first_event_limit_from(std::env::var(FIRST_EVENT_VAR).ok().as_deref())
}

#[cfg(test)]
thread_local! {
    /// A test's own first-event limit, on the thread that runs its turn,
    /// so tests never set the process's environment.
    pub(crate) static TEST_FIRST_EVENT: std::cell::Cell<Option<Option<std::time::Duration>>> =
        const { std::cell::Cell::new(None) };
}

fn first_event_limit_from(value: Option<&str>) -> Option<std::time::Duration> {
    let seconds = value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(FIRST_EVENT_DEFAULT);
    (seconds > 0).then(|| std::time::Duration::from_secs(seconds))
}

/// The door a Coder-loop answer paid by `payer` came through.
#[must_use]
pub fn door_of(payer: crate::harness_plugin::Payer) -> Option<Lane> {
    use crate::harness_plugin::Payer;
    match payer {
        Payer::GatewayCredits => Some(Lane::Gateway),
        Payer::ChatGptLogin | Payer::ClaudePlan => Some(Lane::Local),
        Payer::OpenCode => Some(Lane::OpenCode),
        Payer::OpenRouterKey => Some(Lane::OpenRouter),
        Payer::GeminiKey => Some(Lane::GeminiKey),
        Payer::FreeTier => Some(Lane::FreeTier),
        _ => None,
    }
}

/// The capacity book's live refusal for OpenCode, when another flow (a
/// delegate door, a repository run) already found it out — or `None`.
/// Tests never read the book: it lives in the real home.
#[cfg(not(test))]
pub(crate) fn opencode_blocked() -> Option<microcoder_loop::capacity::Refusal> {
    coder::delegate_door::capacity_dir()
        .map(|dir| microcoder_loop::capacity::Book::load(&dir))
        .and_then(|book| {
            book.blocking(
                microcoder_loop::capacity::Provider::OpenCode,
                microcoder_loop::failover::unix_now(),
            )
            .cloned()
        })
}

#[cfg(test)]
pub(crate) fn opencode_blocked() -> Option<microcoder_loop::capacity::Refusal> {
    None
}

/// The model the OpenRouter lane asks for: a strong model when
/// intelligence leads, a fast one when speed leads, else the free router
/// so the metered door is tried last and spends nothing.
#[must_use]
pub fn openrouter_model(favor: Favor, cheap: bool) -> &'static str {
    if cheap {
        crate::models::DEFAULT_MODEL
    } else if favor.intelligence >= 4 && favor.cost < 4 {
        "anthropic/claude-fable-5.1"
    } else if favor.speed >= 4 && favor.cost < 4 {
        crate::models::OPENROUTER
    } else {
        crate::models::DEFAULT_MODEL
    }
}

impl Plan {
    /// Why no door answered, in one sentence for the turn's ending notice.
    /// `reasons` holds what each door said — skipped doors' notes first,
    /// then each tried door's failure — so a provider's own refusal text
    /// never reaches the person unframed.
    /// When the OpenAgents gateway answered 402, its "needs credit"
    /// sentence (how to pay included) leads, and the other doors follow
    /// (#11640).
    #[must_use]
    pub fn explain(&self, reasons: &[String]) -> String {
        // The free answers ran out (#11645): that, with the command that
        // gets more, is the whole ending.
        if reasons
            .iter()
            .any(|reason| reason.contains(crate::free_tier::USED_UP))
        {
            return crate::free_tier::USED_UP.to_owned();
        }
        if let Some(credit) = reasons
            .iter()
            .position(|reason| reason.contains(crate::provider::NEEDS_CREDIT))
        {
            let others: Vec<String> = reasons
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != credit)
                .map(|(_, reason)| reason.trim().trim_end_matches('.').trim_end().to_owned())
                .filter(|reason| !reason.is_empty())
                .collect();
            let credit = reasons[credit].trim();
            return if others.is_empty() {
                credit.to_owned()
            } else {
                // Each door's reason may be a whole sentence of its own.
                format!(
                    "{credit} Nothing else here could answer either: {}.",
                    others.join("; ")
                )
            };
        }
        let reasons: Vec<String> = reasons
            .iter()
            .enumerate()
            .map(|(index, reason)| {
                // A door's reason is often a whole sentence; the list adds
                // its own full stop (no "in /models.. Connect").
                let reason = reason.trim().trim_end_matches('.').trim_end();
                if reason.is_empty() {
                    self.lanes.get(index).map_or_else(
                        || "a connected door".to_owned(),
                        |lane| lane.name().to_owned(),
                    )
                } else {
                    reason.to_owned()
                }
            })
            .collect();
        let fix = if self.signed_in {
            "Add credit at openagents.com, or connect another model in /plugins."
        } else {
            "Sign in with `coder login` to get answers, or connect another model in /plugins."
        };
        if reasons.is_empty() {
            return format!("Nothing here can answer yet. {fix}");
        }
        format!(
            "Coder couldn't find a model that can answer: {}. {fix}",
            join(&reasons)
        )
    }
}

/// `a`, `a and b`, or `a, b, and c`.
pub fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The refusals the local loop's providers hold, in its `result["refusals"]`
/// entries, rendered as `the Claude Code login is out until 11:10pm`
/// clauses for the ending message.
#[must_use]
pub fn refusal_clauses(refusals: &[serde_json::Value]) -> Vec<String> {
    refusals
        .iter()
        .filter_map(|value| {
            serde_json::from_value::<microcoder_loop::capacity::Refusal>(value.clone()).ok()
        })
        .map(|refusal| {
            let login = match refusal.provider {
                microcoder_loop::capacity::Provider::Codex => "The Codex login",
                microcoder_loop::capacity::Provider::Claude => "the Claude Code login",
                microcoder_loop::capacity::Provider::Vertex => "the OpenAgents cloud",
                microcoder_loop::capacity::Provider::OpenCode => "OpenCode",
                microcoder_loop::capacity::Provider::Devin => "the Devin login",
                microcoder_loop::capacity::Provider::Grok => "the Grok login",
            };
            let reset = crate::long_session::clock(refusal.until * 1000);
            format!("{login} is out until {reset}")
        })
        .collect()
}

#[cfg(test)]
mod plan_pick_tests {
    use super::*;
    use coder::local_chain::PlanLogin;

    /// #11550: a picked Claude model runs on the Claude plan when it is
    /// here, by Claude Code's own id; a Codex model on the Codex plan;
    /// anything else, or a plan that isn't here, stays on the key.
    #[test]
    fn a_picked_claude_or_codex_model_runs_on_its_plan() {
        let pick = plan_pick("anthropic/claude-fable-5.1:high", true, false).unwrap();
        assert_eq!(pick.login, PlanLogin::Claude);
        assert_eq!(pick.model, "claude-fable-5-1");
        assert_eq!(pick.effort.as_deref(), Some("high"));
        assert_eq!(
            plan_pick("anthropic/claude-opus-5.5", true, true)
                .unwrap()
                .model,
            "claude-opus-5-5"
        );
        assert!(plan_pick("anthropic/claude-fable-5.1", false, true).is_none());
        let codex = plan_pick(&format!("openai/{}", crate::codex::MODEL), false, true).unwrap();
        assert_eq!(codex.login, PlanLogin::Codex);
        assert!(plan_pick(&format!("openai/{}", crate::codex::MODEL), true, false).is_none());
        assert!(plan_pick("google/gemini-3.8-flash", true, true).is_none());
        assert!(plan_pick("anthropic/claude-unknown-9", true, true).is_none());
        assert!(plan_pick("claude-fable-5-1", true, true).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(agents: &[&str]) -> ExecutionSettings {
        let mut settings =
            crate::plugins::Plugins::default().execution_settings(std::env::temp_dir());
        settings.connections = Some(
            serde_json::from_value(serde_json::json!({
                "origin": "http://fixture", "account": "acct", "label": "Fixture",
                "expires_at": u64::MAX, "token": "fixture-session-token",
            }))
            .unwrap(),
        );
        settings.agents = agents
            .iter()
            .map(|id| crate::bundled_runtime::AcpAgent {
                id: (*id).into(),
                name: (*id).into(),
                program: id.into(),
                transport: Default::default(),
                arguments: vec![],
                mode: None,
                enabled: true,
            })
            .collect();
        settings
    }

    const BALANCED: Favor = Favor {
        cost: 3,
        speed: 3,
        intelligence: 3,
    };

    #[test]
    fn the_standing_order_puts_the_free_allowance_first_and_the_metered_key_last() {
        let plan = plan(&settings(&["opencode"]), true, BALANCED);
        assert_eq!(
            plan.lanes,
            [Lane::Gateway, Lane::Local, Lane::OpenCode, Lane::OpenRouter]
        );
    }

    /// #11563: a chat's later turns go first through the door its first
    /// answer came through; #11645: never only through it, so a door that
    /// can't answer falls back instead of ending the turn.
    #[test]
    fn a_chat_goes_on_through_the_door_it_answered_through_first() {
        use crate::harness_plugin::Payer;
        let mut settings = settings(&["opencode"]);
        settings.door = door_of(Payer::GatewayCredits);
        let held = plan(&settings, true, BALANCED);
        assert_eq!(
            held.lanes,
            [Lane::Gateway, Lane::Local, Lane::OpenCode, Lane::OpenRouter]
        );
        assert_eq!(held.held, Some(Lane::Gateway));
        settings.door = door_of(Payer::ChatGptLogin);
        assert_eq!(
            plan(&settings, true, BALANCED).lanes,
            [Lane::Local, Lane::Gateway, Lane::OpenCode, Lane::OpenRouter]
        );
        assert_eq!(door_of(Payer::ClaudePlan), Some(Lane::Local));
        assert_eq!(door_of(Payer::AgentLogin), None);
    }

    /// #11526: a gateway model picked in /models runs on the gateway
    /// first, whatever the favor, and the other doors follow it (#11645);
    /// signed out, the pick waits and `auto` walks its doors.
    #[test]
    fn a_picked_gateway_model_runs_on_the_gateway_first() {
        let mut settings = settings(&["opencode"]);
        settings.gateway_model = Some("zai/glm-5.3-flash".into());
        let favor = Favor {
            intelligence: 5,
            ..BALANCED
        };
        let picked = plan(&settings, true, favor);
        assert_eq!(picked.lanes[0], Lane::Gateway);
        assert_eq!(picked.held, Some(Lane::Gateway));
        assert!(picked.lanes.len() > 1);
        settings.connections = None;
        assert!(!plan(&settings, true, favor).lanes.contains(&Lane::Gateway));
    }

    /// #11537: a cheap answer walks the doors cheapest first whatever the
    /// favor says, the gateway's free allowance first while signed in, and a
    /// metered key stays on the free router.
    #[test]
    fn a_cheap_answer_tries_the_free_allowance_first_whatever_the_favor() {
        let mut settings = settings(&["opencode"]);
        settings.cheap = Some(Cheap {
            gateway_model: Some(crate::models::GATEWAY_FAST.into()),
        });
        let favor = Favor {
            intelligence: 5,
            ..BALANCED
        };
        assert_eq!(
            plan(&settings, true, favor).lanes,
            [Lane::Gateway, Lane::Local, Lane::OpenCode, Lane::OpenRouter]
        );
        assert_eq!(openrouter_model(favor, true), crate::models::DEFAULT_MODEL);
        // Signed out, the logins' cheapest models answer first.
        settings.connections = None;
        assert_eq!(plan(&settings, true, favor).lanes[0], Lane::Local);
        assert_eq!(CHEAP_CLAUDE_MODEL, "claude-haiku-5-5");
    }

    /// #11548: with no harness, the turn runs on its one model through
    /// the gateway while signed in, then the person's own Gemini key, never
    /// on this computer's logins or OpenCode; the person's own OpenRouter
    /// key is the last resort rather than no answer (#11645).
    #[test]
    fn a_no_harness_turn_takes_the_gateway_then_the_own_keys() {
        let mut settings = settings(&["opencode"]);
        settings.cheap = Some(Cheap {
            gateway_model: Some(crate::models::GATEWAY_FAST.into()),
        });
        settings.no_harness = Some(NoHarness {
            policy: coder_sdk::auto_policy::AutoPolicy::default(),
            gemini_key: Some(model_access::ApiKey::new("fixture-gemini-key")),
            gemini_base: coder_sdk::auto_policy::GEMINI_OPENAI_BASE.into(),
        });
        assert_eq!(
            plan(&settings, true, BALANCED).lanes,
            [Lane::Gateway, Lane::GeminiKey, Lane::OpenRouter]
        );
        assert_eq!(
            settings.no_harness.as_ref().unwrap().model(),
            "google/gemini-4-argon"
        );
        settings.connections = None;
        assert_eq!(
            plan(&settings, true, BALANCED).lanes,
            [Lane::GeminiKey, Lane::OpenRouter]
        );
        settings.no_harness.as_mut().unwrap().gemini_key = None;
        let none = plan(&settings, false, BALANCED);
        assert!(none.lanes.is_empty());
        assert!(none.explain(&none.notes).contains("GEMINI_API_KEY"));
    }

    /// #11640: when the gateway answered 402, its needs-credit sentence
    /// (with how to pay) leads the ending, and the other doors follow.
    #[test]
    fn a_402_leads_the_ending_with_how_to_pay() {
        let plan = Plan {
            lanes: vec![Lane::Gateway, Lane::OpenCode],
            notes: vec![],
            held: None,
            signed_in: true,
        };
        let credit = crate::provider::needs_credit(
            "Today's free allowance is used up. Add credit or use your own key.",
        );
        let said = plan.explain(&[
            credit.clone(),
            "OpenCode didn't start answering within 30 seconds".to_owned(),
        ]);
        assert!(said.starts_with(&credit), "{said}");
        assert!(said.contains("openagents.com"), "{said}");
        assert!(said.contains("OpenCode didn't start answering"), "{said}");
        assert!(!said.contains(".."), "{said}");
        assert_eq!(plan.explain(std::slice::from_ref(&credit)), credit);
    }

    /// #11640: fallback doors get 30 s to their first sign of life by
    /// default; the variable sets another limit, and `0` turns it off.
    #[test]
    fn the_first_event_limit_defaults_to_thirty_seconds() {
        use std::time::Duration;
        assert_eq!(first_event_limit_from(None), Some(Duration::from_secs(30)));
        assert_eq!(
            first_event_limit_from(Some("20")),
            Some(Duration::from_secs(20))
        );
        assert_eq!(first_event_limit_from(Some("0")), None);
        assert_eq!(
            first_event_limit_from(Some("soon")),
            Some(Duration::from_secs(30))
        );
    }

    /// A door's reason that is a whole sentence ends the list once, not
    /// with "..": the gateway's 402 reads "... in /models. Connect ...".
    #[test]
    fn a_sentence_reason_does_not_double_the_full_stop() {
        let plan = Plan {
            lanes: vec![Lane::Gateway],
            notes: vec![],
            held: None,
            signed_in: true,
        };
        let said = plan.explain(&["Today's free allowance on OpenAgents is used up (HTTP 402). Add credit at openagents.com, or choose a model in /models.".to_owned()]);
        assert!(!said.contains(".."), "{said}");
        assert!(said.contains("in /models. Add credit"), "{said}");
    }

    #[test]
    fn doors_that_are_not_connected_are_never_in_the_order() {
        let mut settings = settings(&[]);
        settings.connections = None;
        let plan = plan(&settings, false, BALANCED);
        assert_eq!(plan.lanes, [Lane::Local]);
    }

    #[test]
    fn speed_moves_the_fast_doors_ahead_of_the_subscriptions() {
        let favor = Favor {
            speed: 5,
            ..BALANCED
        };
        let plan = plan(&settings(&["opencode"]), true, favor);
        let position = |lane| plan.lanes.iter().position(|other| *other == lane).unwrap();
        assert!(position(Lane::Gateway) < position(Lane::Local));
        assert!(position(Lane::OpenCode) < position(Lane::Local));
        assert!(position(Lane::OpenRouter) < position(Lane::Local));
    }

    #[test]
    fn intelligence_puts_the_subscriptions_and_a_strong_keyed_model_first() {
        let favor = Favor {
            intelligence: 5,
            ..BALANCED
        };
        let plan = plan(&settings(&["opencode"]), true, favor);
        assert_eq!(plan.lanes[0], Lane::Local);
        assert_eq!(openrouter_model(favor, false), "anthropic/claude-fable-5.1");
    }

    #[test]
    fn cost_keeps_the_metered_key_on_the_free_router() {
        let favor = Favor {
            cost: 5,
            speed: 5,
            intelligence: 5,
        };
        assert_eq!(openrouter_model(favor, false), crate::models::DEFAULT_MODEL);
        let fast = Favor {
            speed: 5,
            ..BALANCED
        };
        assert_eq!(openrouter_model(fast, false), crate::models::OPENROUTER);
    }
}
