//! Verify authority-signed histories without choosing a fork by arrival time.
use crate::{
    Result,
    authority::{Authority, closed},
};
use nostr::{
    domain::Event,
    work::{self, WorkRef},
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Reader {
    pub scopes: BTreeMap<String, Value>,
    pub histories: BTreeMap<String, Vec<Value>>,
    pub forks: BTreeMap<String, Vec<Value>>,
}
impl Reader {
    pub fn admit_scope(
        &mut self,
        event: &Event,
        secret: &SecretKey,
        expected_owner: &str,
    ) -> Result<()> {
        let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
        let body = work::parse(opened.inline_bytes().ok_or("inline scope required")?)
            .map_err(|e| e.to_string())?;
        if body["v"] != "openagents.work-scope.v1"
            || body["owner"] != expected_owner
            || opened.signer() != expected_owner
        {
            return Err("work scope differs from the selected owner".into());
        }
        if self.scopes.len() >= 64 {
            return Err("reader scope limit exceeded".into());
        }
        let id = body["scope"].as_str().ok_or("scope ID")?.to_string();
        if self.scopes.get(&id).is_some_and(|known| *known != body) {
            return Err("conflict: work scope descriptor changed".into());
        }
        self.scopes.insert(id, body);
        Ok(())
    }
    pub fn ingest(&mut self, event: &Event, secret: &SecretKey) -> Result<()> {
        let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
        let revision = work::parse(opened.inline_bytes().ok_or("inline revision required")?)
            .map_err(|e| e.to_string())?;
        if revision["v"] != "openagents.work-revision.v1" || revision["issuer"] != opened.signer() {
            return Err("work revision authority differs".into());
        }
        let w = work::work_ref(&revision["work"]).map_err(|e| e.to_string())?;
        let scope = self
            .scopes
            .get(&w.scope)
            .ok_or("not_admitted: work scope has not been selected")?;
        if scope["authority"] != w.authority
            || revision["scope"] != workspace_host::artifact::json_ref(scope)?
        {
            return Err("work revision differs from the admitted scope and authority".into());
        }
        if self.histories.len() >= 256 && !self.histories.contains_key(&w.key()) {
            return Err("reader work item limit exceeded".into());
        }
        let key = w.key();
        let reference = json!({"work":w,"revision":revision["revision"],"artifact":workspace_host::artifact::json_ref(&revision)?});
        let history = self.histories.entry(key.clone()).or_default();
        if history.contains(&reference)
            || self.forks.get(&key).is_some_and(|f| f.contains(&reference))
        {
            return Ok(());
        }
        if history.len() >= 128 {
            return Err("work reader history limit exceeded".into());
        }
        if history
            .iter()
            .any(|r| r["revision"] == revision["revision"])
        {
            let forks = self.forks.entry(key).or_default();
            if forks.len() >= 128 {
                return Err("work reader fork limit exceeded".into());
            }
            forks.push(reference);
            return Err("conflict: signed work history has forked".into());
        }
        if self.forks.contains_key(&key) {
            return Err("conflict: work history remains frozen".into());
        }
        if history.last() != revision.get("previous").filter(|p| !p.is_null()) {
            return Err("partial: work predecessor is unavailable".into());
        }
        history.push(reference);
        Ok(())
    }
    pub fn head(&self, work: &WorkRef) -> Result<Option<&Value>> {
        if self.forks.contains_key(&work.key()) {
            return Err("conflict: work history remains frozen".into());
        }
        Ok(self.histories.get(&work.key()).and_then(|h| h.last()))
    }
}
impl Authority {
    pub fn read_history(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-history-read.v1"))?;
        closed(&body, &["v", "requires", "issuer", "work"])?;
        let _lock = self.store.lock()?;
        let state = self.state()?;
        let w = work::work_ref(&body["work"]).map_err(|e| e.to_string())?;
        let scope = state
            .scopes
            .get(&w.scope)
            .filter(|s| s.descriptor["authority"] == w.authority)
            .ok_or("not_admitted: work scope unavailable")?;
        self.check(scope, &actor, Some(&w.item), "read", now)?;
        let item = scope
            .items
            .get(&w.item)
            .ok_or("not_admitted: work unavailable")?;
        let mut records = Vec::new();
        for reference in &item.history {
            let revision = self.store.json(&reference["artifact"])?;
            work::verify_revision(reference, &revision).map_err(|e| e.to_string())?;
            let event = workspace_host::host::request(&self.secret, &actor, revision.clone(), now)?;
            let snapshot = self.snapshot(&revision["snapshot"])?;
            let available = self.disclosure(&state, &actor, &snapshot, now).is_ok();
            let (issuer, command) = self.original(&state, &revision["command"])?;
            let original = if issuer == self.signer.pubkey() {
                Some(workspace_host::host::request(
                    &self.secret,
                    &actor,
                    command.clone(),
                    now,
                )?)
            } else {
                state
                    .declarations
                    .get(
                        revision["command"]["digest"]
                            .as_str()
                            .ok_or("command digest")?,
                    )
                    .and_then(|events| {
                        events.iter().find(|event| {
                            event.pubkey == actor || event.tag_values("p").any(|p| p == actor)
                        })
                    })
                    .cloned()
            };
            records.push(json!({"revision":reference,"event":event,"snapshot":if available{Some(snapshot)}else{None},"disclosure":if available{"available"}else{"redacted"},"command":{"body":command,"original":original,"authorship":if issuer==actor||issuer==self.signer.pubkey()||actor==self.signer.pubkey(){"original_available"}else{"authority_observed"}}}));
        }
        Ok(
            json!({"work":w,"coverage":if item.frozen{"conflict"}else{"complete"},"records":records,"forks":item.forks}),
        )
    }
    /// Retain a signed conflicting authority successor, then stop mutations.
    pub fn observe_fork(&self, event: &Event, now: u64) -> Result<()> {
        let (actor, body) = self.authenticate(event, now, Some("openagents.work-revision.v1"))?;
        work::validate(&body).map_err(|e| e.to_string())?;
        let w = work::work_ref(&body["work"]).map_err(|e| e.to_string())?;
        if actor != w.authority {
            return Err("fork must be signed by the work authority".into());
        }
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope = state
            .scopes
            .get_mut(&w.scope)
            .filter(|s| s.descriptor["authority"] == w.authority && s.reference == body["scope"])
            .ok_or("fork scope unavailable")?;
        let item = scope
            .items
            .get_mut(&w.item)
            .ok_or("fork item unavailable")?;
        let reference = json!({"work":w,"revision":body["revision"],"artifact":workspace_host::artifact::json_ref(&body)?});
        if item.history.contains(&reference) || item.forks.contains(&reference) {
            return Ok(());
        }
        let prior = item
            .history
            .iter()
            .find(|r| r["revision"] == body["revision"])
            .ok_or("fork predecessor must be retained")?;
        let known = self.store.json(&prior["artifact"])?;
        if known["previous"] != body["previous"] {
            return Err("signed record is not a successor of the retained predecessor".into());
        }
        if item.forks.len() >= 128 {
            return Err("fork retention limit exceeded".into());
        }
        item.frozen = true;
        item.forks.push(reference);
        self.retain_declaration(&mut state, event, &body)?;
        self.save(&state)
    }
}
