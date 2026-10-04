//! The browser build's relay link: offline. It has the native link's
//! interface so the session, chat, and feeds compile unchanged, accepts no
//! command, and delivers nothing.
use super::{In, Out};
use std::time::Duration;

/// A relay link that never connects.
pub struct Link {
    /// The relay URL the caller asked for.
    pub url: String,
}
impl Link {
    #[cfg(test)]
    pub(crate) fn idle() -> Self {
        Self::start("ws://127.0.0.1:1")
    }
    /// Returns an offline link; nothing is opened.
    #[must_use]
    pub fn start(url: &str) -> Self {
        Self { url: url.into() }
    }
    /// Refuses `out`: an offline link queues nothing.
    pub fn send(&self, out: Out) -> bool {
        self.send_batch(vec![out])
    }
    /// Refuses `commands`: an offline link queues nothing.
    pub fn send_batch(&self, _commands: Vec<Out>) -> bool {
        false
    }
    /// Returns no messages.
    #[must_use]
    pub fn drain(&self) -> Vec<In> {
        Vec::new()
    }
    /// Returns true: there is no worker to stop.
    pub fn shutdown(&mut self, _wait: Duration) -> bool {
        true
    }
}
