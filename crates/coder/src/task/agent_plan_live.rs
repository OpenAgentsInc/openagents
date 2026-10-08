//! The live halves of a day plan: the agent's model as the plan's
//! `Writer` and Jev as its [`Judge`]. Each blocks on a runtime of its own,
//! because the host plans on a thread of its own. Unit tests use the
//! scripted fakes; nothing here runs in a test against a network.

use jev::Answer;

use super::super::agent::{LiveModel, Store};
use super::super::agent_reflect::{PoolWriter, Reply, Writer};
use super::{Judge, Reaction, SET, Services, react_request};

/// The agent's model through the capacity book, and Jev from the decision
/// profile.
///
/// # Errors
/// When no model is set up. Without Jev, every observed event continues.
pub fn services(store: &Store) -> Result<Services, String> {
    super::super::sales::privacy::model_available(store)?;
    let judge: Box<dyn Judge + Send> = match crate::decision::from_env() {
        Ok(Some(client)) => Box::new(JevJudge::new(client)?),
        _ => Box::new(Unanswered),
    };
    let home = pylon::home();
    let route = pylon::route::load(&home);
    let writer: Box<dyn Writer + Send> = if route.on {
        // Free pool jobs first (#10921); the agent's own model answers
        // when no pylon does.
        let fallback = LiveModel::new()
            .ok()
            .map(|model| Box::new(model) as Box<dyn Writer + Send>);
        Box::new(PoolWriter {
            pool: Box::new(move |system, prompt| {
                let answer = pylon::route::ask_blocking(&home, &route, system, prompt)?;
                Ok(Reply {
                    text: answer.text.unwrap_or_default(),
                    model: format!("pylon:{}", answer.model),
                    usd: Some(0.0),
                })
            }),
            fallback,
        })
    } else {
        Box::new(LiveModel::new()?)
    };
    Ok(Services { writer, judge })
}

/// No judge: every observed event continues, and the journal says why.
struct Unanswered;

impl Judge for Unanswered {
    fn react(&mut self, _: &serde_json::Value) -> Result<Reaction, String> {
        Err("Jev isn't set up".into())
    }
}

/// Jev answers `questions/react-or-continue.json`.
pub struct JevJudge {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevJudge {
    /// # Errors
    /// When the runtime doesn't start.
    pub fn new(client: jev::Client) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start a runtime: {e}"))?;
        Ok(Self { client, runtime })
    }
}

impl Judge for JevJudge {
    fn react(&mut self, state: &serde_json::Value) -> Result<Reaction, String> {
        let request = react_request(state.clone())?;
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        match response.answers.get(SET.gate.as_str()) {
            Some(Answer::Choice(answer)) => Reaction::parse(&answer.choice)
                .ok_or_else(|| format!("Jev chose `{}`, not an option", answer.choice)),
            _ => Err(format!("Jev didn't answer {}", SET.id)),
        }
    }
}
