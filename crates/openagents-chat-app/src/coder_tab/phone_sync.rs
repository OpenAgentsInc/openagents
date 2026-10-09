//! This phone's own chats for the account sync (#11107): the phone app
//! uploads them to the person's openagents.com account when they chose
//! "Sync all my chats" (`openagents-mobile`'s `account_link`).

use super::*;
use openagents_chat::basic_coder::Turn;

impl CoderTab {
    /// This device's own chats: each one's id, title, and when it last
    /// changed, less archived ones and any still answering.
    #[must_use]
    pub fn phone_chats(&self) -> Vec<(String, String, u64)> {
        self.basic
            .list()
            .iter()
            .filter(|summary| !summary.archived && !self.basic.busy(&summary.id))
            .map(|summary| (summary.id.clone(), summary.title.clone(), summary.updated))
            .collect()
    }

    /// One of this device's chats, every turn.
    pub fn phone_turns(&mut self, id: &str) -> Vec<Turn> {
        self.basic.turns(id).to_vec()
    }
}
