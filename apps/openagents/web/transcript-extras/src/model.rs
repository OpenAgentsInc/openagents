//! Account replies may finish after an account switch.
use openagents_proto::{AgentAccountsSnapshot, Connectivity};

#[derive(Default)]
pub struct Accounts {
    pub snapshot: AgentAccountsSnapshot,
    pub error: Option<String>,
    pub busy: bool,
    pub loaded: bool,
    pub revision: u64,
}

impl Accounts {
    pub fn begin(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.busy = true;
        self.revision
    }

    /// False for a stale poll, including a failure from the previous account.
    pub fn read(&mut self, revision: u64, result: Result<AgentAccountsSnapshot, String>) -> bool {
        if revision != self.revision || self.busy {
            return false;
        }
        self.loaded = true;
        match result {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }

    pub fn switched(&mut self, revision: u64, result: Result<AgentAccountsSnapshot, String>) {
        if revision == self.revision {
            self.busy = false;
            self.read(revision, result);
        }
    }
}

pub fn pending(connectivity: &Connectivity, chat: &str) -> bool {
    connectivity
        .chats
        .iter()
        .find(|c| c.chat_id == chat)
        .is_some_and(|c| c.pending_pushes > 0 && !c.delivery_live)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_poll_cannot_replace_a_switch_or_stop_the_next_poll() {
        let mut accounts = Accounts::default();
        let old = accounts.revision;
        let current = accounts.begin();
        assert!(!accounts.read(old, Err("old account failed".into())));
        assert!(!accounts.loaded);
        accounts.switched(current, Ok(AgentAccountsSnapshot::default()));
        assert!(!accounts.read(old, Err("late failure".into())));
        assert!(accounts.error.is_none());
        assert!(accounts.read(current, Ok(AgentAccountsSnapshot::default())));
    }

    #[test]
    fn only_the_selected_chats_undelivered_batches_offer_retry() {
        let state: Connectivity = serde_json::from_value(serde_json::json!({
            "state":"connected", "chats":[
                {"chatId":"a","connected":true,"pendingPushes":1,"deliveryLive":false},
                {"chatId":"b","connected":true,"pendingPushes":1,"deliveryLive":true}
            ]
        }))
        .unwrap();
        assert!(pending(&state, "a"));
        assert!(!pending(&state, "b"));
        assert!(!pending(&state, "missing"));
    }
}
