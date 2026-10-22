//! The Haunted House in the account's Game Lab (#12476): the `haunt_*`
//! tools on the account's own copy of the house, one change credit per
//! tester, and the house a page follows to show each change live.
//!
//! Every edit is one typed tool call the chat's model chose
//! ([`game_lab::haunt::edit`]), checked by the house's rules, and saved as
//! a version in the account's Labs store. A tester account gets one saved
//! change; a refused edit costs nothing. Site admins and the accounts in
//! `OPENAGENTS_GAME_LAB_UNLIMITED` (a comma list of account ids) have no
//! limit. The house page (`/games/the-haunted-house`) reads
//! `/labs/game/haunt/house` and asks again every second, so a change shows
//! in an open house without a reload.

use game_lab::haunt::{self, HAUNTED_HOUSE, Haunt, TITLE};
use lab_kit::Version;
use lab_store::{CheckedDraft, LabStore};
use serde_json::{Value, json};

use super::{Hosted, now};

/// How many saved changes a tester account may make.
pub(crate) const CREDITS: u64 = 1;
/// The sentence a tester sees after their change.
pub(crate) const SPENT: &str =
    "You've made your one change to the house for this test. We'll open more changes soon.";

/// Whether an account has no change limit.
pub(crate) fn unlimited(account: &str, admin: bool) -> bool {
    admin
        || std::env::var("OPENAGENTS_GAME_LAB_UNLIMITED")
            .ok()
            .is_some_and(|list| list.split(',').any(|id| id.trim() == account))
}

impl Hosted {
    fn haunt_original(&self) -> Version {
        let content = haunt::original();
        Version {
            document: HAUNTED_HOUSE.into(),
            number: 1,
            parent: None,
            author: self.account.clone(),
            summary: "The Haunted House as it stands today".into(),
            created_unix_ms: 0,
            schema: haunt_rules::KIND.into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: lab_kit::Report {
                warnings: Vec::new(),
                facts: Value::Null,
            },
            assets: vec![],
        }
    }

    /// The account's house at version `number`, or its latest (version 1
    /// until it makes a change).
    pub(crate) async fn haunt_version(&self, number: Option<u64>) -> lab_store::Result<Version> {
        match self.store.document(HAUNTED_HOUSE).await {
            Ok(meta) => {
                self.store
                    .version(HAUNTED_HOUSE, number.unwrap_or(meta.latest))
                    .await
            }
            Err(lab_store::Error::NotFound) if number.is_none_or(|n| n == 1) => {
                Ok(self.haunt_original())
            }
            Err(e) => Err(e),
        }
    }

    /// How many changes the account has saved to its house.
    async fn haunt_changes(&self) -> lab_store::Result<u64> {
        match self.store.document(HAUNTED_HOUSE).await {
            Ok(meta) => Ok(meta.latest.saturating_sub(1)),
            Err(lab_store::Error::NotFound) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// The versions of the house, for the card.
    pub(crate) async fn haunt_count(&self) -> u64 {
        self.store
            .document(HAUNTED_HOUSE)
            .await
            .map_or(1, |meta| meta.latest)
    }

    pub(crate) fn haunt_card(&self, version: &Version, changes: &Value) -> Value {
        json!({
            "card": super::super::CARD,
            "doc": HAUNTED_HOUSE,
            "title": version.content["title"].as_str().unwrap_or(TITLE),
            "version": version.number,
            "summary": version.summary,
            "changes": changes,
            "player": haunt::PLAY_PATH,
            "build": "ready",
            "_hosted": true,
            "_game_owner": self.owner,
        })
    }

    /// The game the account changed last: the Haunted House when its
    /// newest version is newer than The Ice Cream Man's.
    async fn last_game(&self) -> &'static str {
        let newest = |doc: &'static str| async move {
            let meta = self.store.document(doc).await.ok()?;
            self.store
                .version(doc, meta.latest)
                .await
                .ok()
                .map(|v| v.created_unix_ms)
        };
        let house = newest(HAUNTED_HOUSE).await.unwrap_or(0);
        let ice = newest(game_lab::doc::ICE_CREAM_MAN).await.unwrap_or(0);
        if house > ice {
            HAUNTED_HOUSE
        } else {
            game_lab::doc::ICE_CREAM_MAN
        }
    }

    /// The card for `doc` (by default the game the account changed last)
    /// at version `v`, compared with `a` for The Ice Cream Man. The Grid's
    /// Game Lab panel asks for the card a chat turn made without naming
    /// the game, so the last-changed game answers.
    pub(crate) async fn card_of(
        &self,
        doc: Option<&str>,
        v: Option<u64>,
        a: Option<u64>,
    ) -> lab_store::Result<Value> {
        let doc = match doc {
            Some(doc) => doc,
            None => self.last_game().await,
        };
        if doc == HAUNTED_HOUSE {
            let version = self.haunt_version(v).await?;
            return Ok(self.haunt_card(&version, &version.report.facts["changes"]));
        }
        self.card(v, a).await
    }

    /// One `haunt_*` tool.
    pub(crate) async fn haunt(&self, name: &str, args: &Value) -> lab_store::Result<Value> {
        let base = self.haunt_version(None).await?;
        if name == "haunt_describe" {
            let library = args["library"].as_bool().unwrap_or(false);
            let text =
                haunt::describe(&base.content, library).map_err(lab_store::Error::Refused)?;
            return Ok(json!({"version": base.number, "text": text,
                "card": self.haunt_card(&base, &Value::Null)}));
        }
        if !haunt::EDITS.contains(&name) {
            return Err(lab_store::Error::NotFound);
        }
        // Check the edit before spending anything: a refusal costs nothing.
        let edited = haunt::edit(&base.content, name, args).map_err(lab_store::Error::Refused)?;
        if !unlimited(&self.account, self.admin) && self.haunt_changes().await? >= CREDITS {
            return Err(lab_store::Error::Refused(lab_kit::Refusal::new(
                "credit_spent",
                SPENT,
            )));
        }
        let summary = args["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 2048)
            .map_or_else(|| edited.changes.join("; "), str::to_owned);
        let change = haunt::intent(name, args);
        if self.store.document(HAUNTED_HOUSE).await.is_err() {
            let original = self.haunt_original();
            let initial = CheckedDraft::initial::<Haunt>(
                haunt_rules::KIND,
                original.content,
                &original.summary,
                &Value::Null,
            )?;
            match self
                .store
                .create(HAUNTED_HOUSE, TITLE, &self.account, initial, now())
                .await
            {
                Ok(_) | Err(lab_store::Error::Conflict) => {}
                Err(e) => return Err(e),
            }
        }
        let saved_base = self.store.version(HAUNTED_HOUSE, base.number).await?;
        if saved_base.content_sha256 != base.content_sha256 {
            return Err(lab_store::Error::Conflict);
        }
        let draft = CheckedDraft::change::<Haunt>(&saved_base, &change, &summary, &Value::Null)?;
        let version = self
            .store
            .save(HAUNTED_HOUSE, &self.account, draft, now())
            .await?;
        let changes = json!(edited.changes);
        let left = if unlimited(&self.account, self.admin) {
            json!(null)
        } else {
            json!(CREDITS.saturating_sub(self.haunt_changes().await.unwrap_or(CREDITS)))
        };
        Ok(json!({
            "saved": true,
            "version": version.number,
            "changes": changes,
            "changes_left": left,
            "text": format!("Saved version {}: {}. An open house shows it within a second or two.",
                version.number, edited.changes.join("; ")),
            "card": self.haunt_card(&version, &changes),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spent_sentence_is_plain_and_says_we() {
        assert!(oa_copy::violations(SPENT, &[]).is_empty());
        assert!(SPENT.contains("We'll"));
        assert!(unlimited("anyone", true));
        assert!(!unlimited("tester-account-that-is-not-listed", false));
    }
}
