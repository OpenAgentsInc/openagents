//! WS pages over a recipient-filtered native history; boards own no lifecycle.
use crate::{
    Result,
    authority::{Authority, Scope, closed},
};
use nostr::{
    contracts,
    domain::Event,
    work::{self, WorkRef},
    workspace,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use workspace_host::{artifact, projection::Projection};
impl Authority {
    pub fn define_view(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-native-view.v1"))?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "issuer",
                "scope",
                "all",
                "items",
                "retain_until",
            ],
        )?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope_id = state
            .scopes
            .iter()
            .find(|(_, s)| s.reference == body["scope"])
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: scope unavailable")?;
        let mut scope = state.scopes[&scope_id].clone();
        let grant = self.check(&scope, &actor, None, "read", now)?.clone();
        let all = body["all"].as_bool().ok_or("view selection")?;
        let items: BTreeSet<String> =
            serde_json::from_value(body["items"].clone()).map_err(|_| "view item selection")?;
        if items.len() > 256 || all && !grant.all || all && !items.is_empty() {
            return Err("not_admitted: unsupported view selection".into());
        }
        for item in &items {
            self.check(&scope, &actor, Some(item), "read", now)?;
            if !scope.items.contains_key(item) {
                return Err("not_admitted: selected work unavailable".into());
            }
        }
        let until = body["retain_until"]
            .as_u64()
            .filter(|v| *v > now && *v <= now + 7 * 86_400)
            .ok_or("view retention")?;
        if scope.views.len() >= 64 {
            return Err("work view limit exceeded".into());
        }
        let selection = json!({"v":"openagents.work-native-selection.v1","requires":[],"scope":scope.reference,"all":all,"items":items,"recipient":actor,"grant_epoch":grant.epoch});
        let selection = self.store.put_json(&selection)?;
        let parameters=self.store.put_json(&json!({"v":"openagents.work-native-view-parameters.v1","requires":[],"withheld":[{"artifact":scope.policy_ref,"state":"redacted"},{"artifact":grant.reference,"state":"redacted"}],"scope":"one_native_work_scope"}))?;
        let definition_data=self.store.put_json(&json!({"v":"openagents.work-native-view-transform.v1","requires":[],"semantics":"accepted_revisions_only","source_order":"authorized_feed_chain","unconfirmed_proposals":false}))?;
        let transform = json!({"id":format!("{}:work/native-view",self.signer.pubkey()),"artifact":definition_data});
        let wire: Value = serde_json::from_slice(work::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let mut defs = serde_json::Map::new();
        for key in ["artifact", "event", "work", "revision_ref"] {
            defs.insert(key.into(), wire["$defs"][key].clone());
        }
        let schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":defs,"type":"object","additionalProperties":false,"required":["v","requires","work","revision"],"properties":{"v":{"const":"openagents.work-view-row.v1"},"requires":{"type":"array","maxItems":0},"work":{"$ref":"#/$defs/work"},"revision":{"$ref":"#/$defs/revision_ref"},"meta":{"type":"object"}}});
        let schema = self.store.put(
            &contracts::jcs(&schema).map_err(|e| e.to_string())?,
            "application/schema+json",
            None,
        )?;
        let source=self.store.put_json(&json!({"v":"openagents.work-authorized-history-source.v1","requires":[],"selection":selection,"recipient":actor,"grant_epoch":grant.epoch,"ordering":"previous_digest_and_sequence","authority":self.signer.pubkey()}))?;
        let definition = json!({"v":"openagents.projection-definition.v1","requires":[],"projection":artifact::token(),"authority":self.signer.pubkey(),"recipient":actor,"scope":selection,"policy":scope.policy_ref,"authorization":grant.reference,"transform":transform,"parameters":parameters,"row_schema":schema,"sources":[source],"retention":{"retain_until":until,"max_items":256,"max_bytes":8*1_048_576}});
        let mut view = Projection::new(
            definition,
            scope.reference.clone(),
            grant.epoch,
            BTreeSet::new(),
            &self.store,
        )?;
        for reference in [
            view.reference().clone(),
            selection,
            parameters,
            schema,
            source,
            transform["artifact"].clone(),
        ] {
            view.permit(&reference)?;
        }
        self.populate_view(&scope, &mut view, None, now)?;
        let reference = view.reference().clone();
        scope.views.insert(
            reference["digest"].as_str().ok_or("view digest")?.into(),
            view,
        );
        state.scopes.insert(scope_id, scope);
        self.save(&state)?;
        Ok(reference)
    }
    fn view_access(&self, scope: &Scope, view: &Projection, actor: &str, now: u64) -> Result<()> {
        let grant = self.check(scope, actor, None, "read", now)?;
        if scope
            .stale_views
            .contains(view.reference()["digest"].as_str().ok_or("view digest")?)
        {
            return Err("stale: create a new work view".into());
        }

        if view.recipient() != actor
            || view.authorization_epoch() != grant.epoch
            || view.definition()["authorization"] != grant.reference
            || now
                > view.definition()["retention"]["retain_until"]
                    .as_u64()
                    .ok_or("view retention")?
        {
            return Err("not_admitted: work view unavailable".into());
        }
        let selection = self.store.json(&view.definition()["scope"])?;
        if selection["scope"] != scope.reference
            || selection["recipient"] != actor
            || selection["all"] == true && !grant.all
        {
            return Err("not_admitted: work selection unavailable".into());
        }
        for item in selection["items"].as_array().ok_or("view items")? {
            self.check(scope, actor, item.as_str(), "read", now)?;
        }
        Ok(())
    }
    fn selected(&self, scope: &Scope, view: &Projection, item: &str) -> Result<bool> {
        let selection = self.store.json(&view.definition()["scope"])?;
        Ok((selection["all"] == true
            || selection["items"]
                .as_array()
                .is_some_and(|a| a.iter().any(|id| *id == item)))
            && scope.items.contains_key(item))
    }
    fn populate_view(
        &self,
        scope: &Scope,
        view: &mut Projection,
        transition: Option<&Value>,
        now: u64,
    ) -> Result<()> {
        self.view_access(scope, view, view.recipient(), now)?;
        let mut rows = BTreeMap::new();
        let mut complete = true;
        for (id, item) in &scope.items {
            if !self.selected(scope, view, id)? {
                continue;
            }
            self.check(scope, view.recipient(), Some(id), "read", now)?;
            let (reference, revision, snapshot) =
                self.head(scope, id)?.ok_or("work unavailable")?;
            let w = work::work_ref(&reference["work"]).map_err(|e| e.to_string())?;
            let value = json!({"v":"openagents.work-view-row.v1","requires":[],"work":w,"revision":reference});
            work::validate(&value).map_err(|e| e.to_string())?;
            let value = self.store.put_json(&value)?;
            let hidden_relation = snapshot["relations"]
                .as_array()
                .ok_or("work relations")?
                .iter()
                .any(|relation| {
                    work::work_ref(&relation["target"]).is_ok_and(|target| {
                        target.scope != w.scope
                            || self
                                .check(scope, view.recipient(), Some(&target.item), "read", now)
                                .is_err()
                    })
                });
            let disclosure=self.store.put_json(&json!({"v":"openagents.work-view-disclosure.v1","requires":[],"work":w,"revision":reference,"snapshot":{"artifact":revision["snapshot"],"state":if hidden_relation{"redacted"}else{"available"}},"history":if item.frozen{"conflict"}else{"retained"}}))?;
            if hidden_relation || item.frozen {
                complete = false;
            }
            view.permit(&reference["artifact"])?;
            if !hidden_relation {
                view.permit(&revision["snapshot"])?;
            }
            view.permit(&disclosure)?;
            rows.insert(
                w.key(),
                json!({"key":w.key(),"value":value,"sources":[disclosure]}),
            );
        }
        let current = view.cut().ok().cloned();
        let previous = current
            .as_ref()
            .map(|cut| cut["frontiers"][0]["position"].clone());
        let sequence = current
            .as_ref()
            .and_then(|cut| cut["sequence"].as_u64())
            .map(|n| n + 1)
            .unwrap_or(0);
        let feed=self.store.put_json(&json!({"v":"openagents.work-authorized-history.v1","requires":[],"source":view.definition()["sources"][0],"sequence":sequence,"previous":previous,"transition":transition,"coverage":if complete{"complete"}else{"partial"}}))?;
        view.permit(&feed)?;
        let frontiers =
            vec![json!({"source":view.definition()["sources"][0],"position":feed["digest"]})];
        view.append(
            &self.store,
            rows,
            frontiers,
            if complete { "complete" } else { "partial" },
            now,
        )
    }
    pub(crate) fn advance_views(
        &self,
        scope: &mut Scope,
        changed: &WorkRef,
        revision: &Value,
        now: u64,
    ) -> Result<()> {
        let keys = scope.views.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let mut view = scope.views[&key].clone();
            if self
                .view_access(scope, &view, view.recipient(), now)
                .is_err()
            {
                continue;
            }
            if !self.selected(scope, &view, &changed.item)? {
                continue;
            }
            match self.populate_view(scope, &mut view, Some(revision), now) {
                Ok(()) => {
                    scope.views.insert(key, view);
                }
                Err(_) => {
                    scope.stale_views.insert(key);
                }
            }
        }
        Ok(())
    }
    pub fn projection_read(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, request) =
            self.authenticate(event, now, Some("openagents.projection-read.v1"))?;
        workspace::validate(&request).map_err(|e| e.to_string())?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let request_ref = self.store.put_json(&request)?;
        let Some(scope_id) = state
            .scopes
            .iter()
            .find(|(_, s)| {
                s.views
                    .contains_key(request["projection"]["digest"].as_str().unwrap_or(""))
            })
            .map(|(key, _)| key.clone())
        else {
            return self.view_refusal(&request_ref, "not_admitted");
        };
        let mut scope = state.scopes[&scope_id].clone();
        let key = request["projection"]["digest"]
            .as_str()
            .ok_or("view digest")?
            .to_string();
        let mut view = scope.views[&key].clone();
        if view.reference() != &request["projection"] {
            return self.view_refusal(&request_ref, "not_admitted");
        }
        if let Err(error) = self.view_access(&scope, &view, &actor, now) {
            return self.view_refusal(
                &request_ref,
                if error.starts_with("stale:") {
                    "stale"
                } else {
                    "not_admitted"
                },
            );
        }

        let id = request["request"].as_str().ok_or("read ID")?;
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&request).map_err(|e| e.to_string())?);
        match view.known_read(id, &fingerprint) {
            Ok(Some(result)) => return Ok(result),
            Err(_) => return self.view_refusal(&request_ref, "idempotency_conflict"),
            Ok(None) => {}
        }
        let result = match view.page(&self.store, &actor, &request, &request_ref) {
            Ok(page) => {
                let reference = self.store.put_json(&page)?;
                view.permit(&reference)?;
                json!({"v":"openagents.projection-result.v1","requires":[],"request":request_ref,"status":"completed","page":reference,"marker":null,"reason":null})
            }
            Err(error) => self.view_refusal(
                &request_ref,
                if error.starts_with("limit_exceeded") {
                    "limit_exceeded"
                } else {
                    "stale"
                },
            )?,
        };
        workspace::validate(&result).map_err(|e| e.to_string())?;
        view.remember_read(id.into(), fingerprint, result.clone())?;
        scope.views.insert(key, view);
        state.scopes.insert(scope_id, scope);
        self.save(&state)?;
        Ok(result)
    }
    fn view_refusal(&self, reference: &Value, reason: &str) -> Result<Value> {
        Ok(
            json!({"v":"openagents.projection-result.v1","requires":[],"request":reference,"status":"refused","page":null,"marker":null,"reason":reason}),
        )
    }
    pub fn projection_fetch(&self, event: &Event, now: u64) -> Result<Vec<u8>> {
        let (actor, request) =
            self.authenticate(event, now, Some("openagents.work-view-fetch.v1"))?;
        closed(
            &request,
            &[
                "v",
                "requires",
                "issuer",
                "projection",
                "artifact",
                "max_bytes",
            ],
        )?;
        let _lock = self.store.lock()?;
        let state = self.state()?;
        let scope = state
            .scopes
            .values()
            .find(|s| {
                s.views
                    .contains_key(request["projection"]["digest"].as_str().unwrap_or(""))
            })
            .ok_or("not_admitted: work view unavailable")?;
        let view = scope
            .views
            .get(
                request["projection"]["digest"]
                    .as_str()
                    .ok_or("view digest")?,
            )
            .ok_or("not_admitted: view unavailable")?;
        self.view_access(scope, view, &actor, now)?;
        let reference =
            contracts::parse_artifact(&request["artifact"]).map_err(|e| e.to_string())?;
        let limit = request["max_bytes"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 1_048_576)
            .ok_or("artifact fetch limit")?;
        if view.reference() != &request["projection"]
            || !view.permitted(&reference.digest)
            || reference.size > limit
        {
            return Err("not_admitted: work artifact unavailable in this view".into());
        }
        let bytes = self.store.get(&request["artifact"])?;
        if reference.media_type == "application/json" {
            self.disclosure(
                &state,
                &actor,
                &contracts::parse_strict(&bytes).map_err(|e| e.to_string())?,
                now,
            )?;
        }
        Ok(bytes)
    }
    pub(crate) fn update_attention(
        &self,
        scope: &mut Scope,
        w: &WorkRef,
        prior: Option<&Value>,
        item: &Value,
        basis: &Value,
        cause: &Value,
        now: u64,
    ) -> Result<()> {
        let reason = if item["state"] == "blocked" {
            Some("blocked")
        } else if prior.is_none_or(|p| p["accountable"] != item["accountable"]) {
            Some("assigned")
        } else if prior.is_some_and(|p| p["evidence"] != item["evidence"]) {
            Some("disposition_required")
        } else {
            None
        };
        let Some(reason) = reason else {
            return Ok(());
        };
        let recipients = scope.grants.keys().cloned().collect::<Vec<_>>();
        for actor in recipients {
            if self
                .check(scope, &actor, Some(&w.item), "read", now)
                .is_err()
            {
                continue;
            }
            let attention = json!({"v":"openagents.work-attention.v1","requires":[],"issuer":self.signer.pubkey(),"recipient":actor,"work":w,"basis":basis,"cause":cause,"reason":reason,"expires_at":now+7*86_400});
            work::validate(&attention).map_err(|e| e.to_string())?;
            let key = contracts::digest_bytes(
                &contracts::jcs(&json!({"work":w,"recipient":actor,"cause":cause,"reason":reason}))
                    .map_err(|e| e.to_string())?,
            );
            if scope.attention.len() >= 4096 && !scope.attention.contains_key(&key) {
                scope.attention_overflow = true;
                continue;
            }
            scope.attention.entry(key).or_insert(attention);
        }
        Ok(())
    }
    pub fn attention(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, request) =
            self.authenticate(event, now, Some("openagents.work-attention-read.v1"))?;
        closed(&request, &["v", "requires", "issuer", "scope", "dismiss"])?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope = state
            .scopes
            .values_mut()
            .find(|s| s.reference == request["scope"])
            .ok_or("not_admitted: scope unavailable")?;
        self.check(scope, &actor, None, "read", now)?;
        if !request["dismiss"].is_null() {
            let id = request["dismiss"].as_str().ok_or("attention identity")?;
            let attention = scope
                .attention
                .get(id)
                .filter(|a| a["recipient"] == actor)
                .ok_or("not_admitted: attention unavailable")?;
            let w = work::work_ref(&attention["work"]).map_err(|e| e.to_string())?;
            self.check(scope, &actor, Some(&w.item), "read", now)?;
            scope.dismissed.insert(format!("{actor}:{id}"));
        }
        let mut result = Vec::new();
        for (id, attention) in &scope.attention {
            if attention["recipient"] != actor
                || scope.dismissed.contains(&format!("{actor}:{id}"))
                || attention["expires_at"].as_u64().is_some_and(|n| now >= n)
            {
                continue;
            }
            let w = work::work_ref(&attention["work"]).map_err(|e| e.to_string())?;
            if self
                .check(scope, &actor, Some(&w.item), "read", now)
                .is_ok()
            {
                result.push(json!({"id":id,"body":attention}));
                if result.len() == 65 {
                    break;
                }
            }
        }
        let coverage = if scope.attention_overflow || result.len() > 64 {
            "partial"
        } else {
            "complete"
        };
        result.truncate(64);
        self.save(&state)?;
        Ok(json!({"records":result,"coverage":coverage}))
    }
}
