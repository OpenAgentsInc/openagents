//! The repository Kiln proposes changes to (#12451): its NIP-34
//! announcement (`30617`), its state (`30618`), and who it trusts.
//!
//! The announcement for this repository is `d` = `openagents`, signed by a
//! repository key the owner holds (`kiln-git announce`). Besides NIP-34's
//! tags it carries `kiln-runner`, the keys whose check comments readers
//! count. Maintainers are the announcement's author and its `maintainers`
//! tag; only their status events count, and only they open patches for
//! the GitHub bridge.

use std::collections::BTreeSet;

use nostr::domain::{Event, Tag};

/// The identifier of this repository's announcement.
pub const REPO_ID: &str = "openagents";
/// The repository state kind.
pub const STATE_KIND: u16 = 30_618;
/// The tag that lists runner keys.
pub const RUNNER_TAG: &str = "kiln-runner";
/// GitHub's address for this repository.
pub const GITHUB: &str = "https://github.com/OpenAgentsInc/openagents";

/// What an announcement says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announce {
    pub id: String,
    pub name: String,
    pub description: String,
    pub relay: String,
    /// The earliest unique commit, 40 hex characters.
    pub euc: Option<String>,
    pub maintainers: Vec<String>,
    pub runners: Vec<String>,
}

/// The tags of a `30617` announcement.
#[must_use]
pub fn announce_tags(a: &Announce) -> Vec<Tag> {
    let mut tags = vec![
        Tag::new(vec!["d".into(), a.id.clone()]),
        Tag::new(vec!["name".into(), a.name.clone()]),
        Tag::new(vec!["description".into(), a.description.clone()]),
        Tag::new(vec!["web".into(), GITHUB.into()]),
        Tag::new(vec!["clone".into(), format!("{GITHUB}.git")]),
        Tag::new(vec!["relays".into(), a.relay.clone()]),
    ];
    if let Some(euc) = &a.euc {
        tags.push(Tag::new(vec!["r".into(), euc.clone(), "euc".into()]));
    }
    tags.push(Tag::new(vec!["t".into(), "rust".into()]));
    let mut list = vec!["maintainers".to_string()];
    list.extend(a.maintainers.iter().cloned());
    tags.push(Tag::new(list));
    if !a.runners.is_empty() {
        let mut list = vec![RUNNER_TAG.to_string()];
        list.extend(a.runners.iter().cloned());
        tags.push(Tag::new(list));
    }
    tags
}

/// The tags of a `30618` state with `main` at `commit`.
#[must_use]
pub fn state_tags(id: &str, commit: &str) -> Vec<Tag> {
    vec![
        Tag::new(vec!["d".into(), id.into()]),
        Tag::new(vec!["refs/heads/main".into(), commit.into()]),
        Tag::new(vec!["HEAD".into(), "ref: refs/heads/main".into()]),
    ]
}

/// A repository as a reader trusts it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trust {
    /// `30617:<owner>:<id>`.
    pub address: String,
    pub name: String,
    /// The announcement's author and its `maintainers`.
    pub maintainers: BTreeSet<String>,
    /// Keys whose check comments count.
    pub runners: BTreeSet<String>,
    pub euc: Option<String>,
}

/// Read an announcement; `None` for anything else.
#[must_use]
pub fn read(event: &Event) -> Option<Trust> {
    let repo = nostr::domain::open_repository(event).ok()?;
    let mut maintainers: BTreeSet<String> = repo.maintainers.into_iter().collect();
    maintainers.insert(event.pubkey.clone());
    let runners = event
        .tags
        .iter()
        .filter(|t| t.name() == Some(RUNNER_TAG))
        .flat_map(|t| t.as_slice().iter().skip(1).cloned())
        .filter(|k| k.len() == 64 && k.bytes().all(|b| b.is_ascii_hexdigit()))
        .collect();
    Some(Trust {
        address: format!("30617:{}:{}", event.pubkey, repo.identifier),
        name: repo.name.unwrap_or(repo.identifier),
        maintainers,
        runners,
        euc: repo.earliest_commit,
    })
}

impl Trust {
    /// A trust list for an address whose announcement isn't known yet: its
    /// owner only.
    #[must_use]
    pub fn of_address(address: &str) -> Self {
        let owner = address.split(':').nth(1).unwrap_or_default().to_string();
        Self {
            address: address.to_string(),
            name: address.split(':').nth(2).unwrap_or_default().to_string(),
            maintainers: BTreeSet::from([owner]),
            ..Self::default()
        }
    }

    /// Whether a check comment from `pubkey` counts.
    #[must_use]
    pub fn counts_check(&self, pubkey: &str) -> bool {
        self.runners.contains(pubkey) || self.maintainers.contains(pubkey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cj::Key;

    #[test]
    fn an_announcement_reads_back_its_maintainers_and_runners() {
        let repo_key = Key::from_bytes([5; 32]).unwrap();
        let runner = Key::from_bytes([6; 32]).unwrap();
        let maintainer = "a".repeat(64);
        let tags = announce_tags(&Announce {
            id: REPO_ID.into(),
            name: "OpenAgents".into(),
            description: "d".into(),
            relay: "wss://relay.openagents.com".into(),
            euc: Some("b".repeat(40)),
            maintainers: vec![maintainer.clone()],
            runners: vec![runner.pubkey().to_string()],
        });
        let event = repo_key.sign(crate::patch::REPO_KIND, tags, String::new(), 1);
        let trust = read(&event).unwrap();
        assert_eq!(
            trust.address,
            format!("30617:{}:openagents", repo_key.pubkey())
        );
        assert!(trust.maintainers.contains(&maintainer));
        assert!(trust.maintainers.contains(repo_key.pubkey()));
        assert!(trust.counts_check(runner.pubkey()));
        assert!(!trust.counts_check(&"c".repeat(64)));
        assert_eq!(trust.euc, Some("b".repeat(40)));
        let state = repo_key.sign(
            STATE_KIND,
            state_tags(REPO_ID, &"d".repeat(40)),
            String::new(),
            1,
        );
        let read_state = nostr::domain::open_repository_state(&state).unwrap();
        assert_eq!(
            read_state.refs,
            vec![("refs/heads/main".to_string(), "d".repeat(40))]
        );
    }
}
