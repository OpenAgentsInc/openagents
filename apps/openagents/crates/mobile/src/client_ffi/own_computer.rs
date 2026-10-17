//! The "Run this on your own computer" card (#11982) over
//! `openagents_account::own_computer`: Rust decides when the card shows in a
//! cloud chat and what it says; the app paints it.
use super::CoreAccount;
use openagents_account::{
    Controller, Phase,
    cloud_chat::Snapshot,
    own_computer::{Card, Offer, Scope},
};
use std::{path::PathBuf, sync::Arc};

/// What the card says. Every instruction has its command or link.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct OwnComputerCard {
    pub title: String,
    pub body: String,
    /// The install command, copied on a tap.
    pub command: String,
    pub download_label: String,
    pub download_url: String,
    pub connect: String,
    /// The page `coder login` codes are approved on; linked in `connect`.
    pub approve_url: String,
}

impl From<Card> for OwnComputerCard {
    fn from(card: Card) -> Self {
        Self {
            title: card.title,
            body: card.body,
            command: card.command,
            download_label: card.download_label,
            download_url: card.download_url,
            connect: card.connect,
            approve_url: card.approve_url,
        }
    }
}

#[derive(uniffi::Object)]
pub struct CoreOwnComputerOffer {
    offer: Offer,
    /// Signed in, the card's record is the account's; otherwise the install's.
    controller: Option<Arc<Controller>>,
}

impl CoreOwnComputerOffer {
    fn scope_id(&self) -> Option<String> {
        match self.controller.as_ref()?.snapshot().phase {
            Phase::SignedIn { account } => Some(account.id),
            _ => None,
        }
    }

    /// Whether the card shows under `snapshot`'s chat.
    pub(crate) fn shows_for(&self, snapshot: &Snapshot, computers: u32) -> bool {
        let id = self.scope_id();
        let scope = id.as_deref().map_or(Scope::Install, Scope::Account);
        self.offer.shows(scope, snapshot, computers as usize)
    }
}

#[uniffi::export]
impl CoreAccount {
    /// The own-computer card's record for whoever is signed in, or this
    /// install while signed out.
    pub fn own_computer_offer(&self) -> Arc<CoreOwnComputerOffer> {
        Arc::new(CoreOwnComputerOffer {
            offer: Offer::open(&self.data_dir().join("account")),
            controller: Some(self.controller().clone()),
        })
    }
}

#[uniffi::export]
impl CoreOwnComputerOffer {
    /// The offline demo's record, kept apart from any account's.
    #[uniffi::constructor]
    pub fn demo(data_dir: String) -> Arc<Self> {
        Arc::new(Self {
            offer: Offer::open(&PathBuf::from(data_dir).join("demo")),
            controller: None,
        })
    }

    pub fn card(&self) -> OwnComputerCard {
        Card::new().into()
    }

    /// The person closed the card; it doesn't come back.
    pub fn close(&self) {
        let id = self.scope_id();
        self.offer
            .close(id.as_deref().map_or(Scope::Install, Scope::Account));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_account::cloud_chat::{Line, Role};

    #[test]
    fn the_demo_offers_the_card_once_after_an_answer() {
        let dir = tempfile::tempdir().unwrap();
        let offer = CoreOwnComputerOffer::demo(dir.path().display().to_string());
        let mut snapshot = Snapshot {
            chat: Some("demo-1".into()),
            lines: vec![Line {
                role: Role::Assistant,
                text: "Here's how.".into(),
            }],
            ..Snapshot::default()
        };
        assert!(offer.shows_for(&snapshot, 0));
        assert_eq!(
            offer.card().command,
            openagents_account::own_computer::INSTALL_COMMAND
        );
        offer.close();
        assert!(!offer.shows_for(&snapshot, 0));
        snapshot.chat = Some("demo-2".into());
        assert!(!offer.shows_for(&snapshot, 0));
    }
}
