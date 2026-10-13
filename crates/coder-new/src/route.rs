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
    /// An installed OpenCode agent's turn, on the free `big-pickle` model.
    OpenCode,
    /// The person's OpenRouter key, on the model the favor favors
    /// ([`openrouter_model`]).
    OpenRouter,
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
}

/// The doors an auto turn may take, in the order `favor` puts them.
/// `has_key` says an OpenRouter key is connected; `execution` says whether
/// the OpenAgents account is signed in and an enabled OpenCode agent is
/// installed.
#[must_use]
pub fn plan(execution: &ExecutionSettings, has_key: bool, favor: Favor) -> Plan {
    let mut lanes = Vec::new();
    let mut notes = Vec::new();
    match &execution.connections {
        Some(account) if crate::provider::gateway_open(account) => lanes.push(Lane::Gateway),
        Some(_) => notes.push("the OpenAgents account can't answer right now".to_owned()),
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
    lanes.sort_by_key(|lane| std::cmp::Reverse(lane.score(favor)));
    Plan { lanes, notes }
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
pub fn openrouter_model(favor: Favor) -> &'static str {
    if favor.intelligence >= 4 && favor.cost < 4 {
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
    #[must_use]
    pub fn explain(&self, reasons: &[String]) -> String {
        let reasons: Vec<String> = reasons
            .iter()
            .enumerate()
            .map(|(index, reason)| {
                let reason = reason.trim();
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
        format!(
            "Coder couldn't find a model that can answer: {}. Connect another one in /plugins or add credit at openagents.com.",
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
        assert_eq!(openrouter_model(favor), "anthropic/claude-fable-5.1");
    }

    #[test]
    fn cost_keeps_the_metered_key_on_the_free_router() {
        let favor = Favor {
            cost: 5,
            speed: 5,
            intelligence: 5,
        };
        assert_eq!(openrouter_model(favor), crate::models::DEFAULT_MODEL);
        let fast = Favor {
            speed: 5,
            ..BALANCED
        };
        assert_eq!(openrouter_model(fast), crate::models::OPENROUTER);
    }
}
