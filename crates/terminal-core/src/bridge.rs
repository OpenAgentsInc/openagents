//! Typed messages between a live-shell mount and the shared chat client adapter.

use crate::{context::Context, proposals::Binding};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub thread: String,
    pub request: String,
    pub new: bool,
    pub text: String,
    pub context: Context,
    pub binding: Binding,
}

impl Request {
    pub fn message(&self) -> Result<String, &'static str> {
        if self.binding.context_digest != self.context.identity()
            || self.text.trim().is_empty()
            || self
                .context
                .directory
                .as_ref()
                .is_some_and(|directory| directory != &self.binding.cwd)
        {
            return Err("request context or text changed");
        }
        // The question and its attachment only: how to answer, and the plan
        // format, travel in the system instructions (`basic_coder`), never
        // in the visible turn.
        let preview = self.context.preview();
        let text = if preview.trim().is_empty() {
            self.text.clone()
        } else {
            format!(
                "{}\n\nAttached from my terminal:\n{}",
                self.text,
                preview.trim_end()
            )
        };
        if text.len() > 32 * 1024 {
            return Err("attached context exceeds the request limit");
        }
        Ok(text)
    }
}

/// Only typed helper events cross back into application state.
pub enum Message {
    Attached(String),
    /// The reply's text, with any typed plan removed; the terminal shows it
    /// as plain ASCII.
    Answer(String),
    /// Where the request went: this process's own client, or the host.
    Door(String),
    Proposal(crate::proposals::Proposal, crate::proposals::Effect),
}
/// A mount owns the helper's process or network connection.
pub trait Process: Send {
    fn ended(&mut self) -> Option<bool>;
}
pub struct Connection {
    pub events: std::sync::mpsc::Receiver<Message>,
    pub process: Box<dyn Process>,
}
