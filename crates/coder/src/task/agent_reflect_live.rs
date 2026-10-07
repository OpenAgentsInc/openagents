//! The live halves of a reflection: the agent's model as a [`Writer`] and
//! Jev as a [`Verify`]. Each blocks on a runtime of its own, because a
//! reflection runs on a thread of its own. Unit tests use the scripted
//! fakes; nothing here runs in a test against a network.

use jev::Answer;

use super::super::agent::{LiveModel, Model};
use super::{PREFERENCE, Record, Reply, SET, SUPPORTED, Support, Verify, Writer, verify_request};

impl Writer for LiveModel {
    fn write(&mut self, system: &str, prompt: &str) -> Result<Reply, String> {
        let action = self.next(system, prompt)?;
        let text = if action.reply.trim().is_empty() {
            action.rationale
        } else {
            action.reply
        };
        Ok(Reply {
            text,
            model: self.model.clone().unwrap_or_default(),
            usd: self.usd,
        })
    }
}

/// Jev answers `questions/insight-support.json`.
pub struct JevVerify {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevVerify {
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

impl Verify for JevVerify {
    fn verify(&mut self, agent: &str, insight: &str, cited: &[&Record]) -> Result<Support, String> {
        let request = verify_request(agent, insight, cited)?;
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        let noul = |id: &str| match response.answers.get(id) {
            Some(Answer::Noul(answer)) => Ok(answer.noul),
            _ => Err(format!("Jev didn't answer `{id}` in {}", SET.id)),
        };
        Ok(Support {
            supported: noul(SUPPORTED)?,
            preference: noul(PREFERENCE)?,
            model: response.model.clone(),
        })
    }
}
