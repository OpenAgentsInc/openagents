//! Independently authenticated owner controls; proposals cannot call this path.
use crate::{
    Result,
    book::{Coordinator, Scope, State},
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeSet;
impl Coordinator {
    pub(crate) fn scope_key(&self, state: &State, reference: &Value) -> Result<String> {
        state
            .scopes
            .iter()
            .find(|(_, scope)| scope.reference == *reference)
            .map(|(key, _)| key.clone())
            .ok_or_else(|| "not_admitted: coordination scope unavailable".into())
    }
    pub(crate) fn journal(
        &self,
        scope: &mut Scope,
        kind: &str,
        subject: Option<&str>,
        data: Value,
        now: u64,
    ) -> Result<Value> {
        if scope.journal.len() >= 8192 {
            return Err("coordinator journal retention limit exceeded".into());
        }
        if scope.journal.is_empty() {
            let record = json!({"run":scope.descriptor["run"],"seq":0,"previous":null,"controller":self.authority(),"generation":scope.descriptor["epoch"],"type":"created","subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":{"owner":scope.owner,"request":scope.reference,"base":scope.namespace,"program":scope.descriptor["capability"],"lock":scope.namespace["digest"],"context":scope.reference,"policy":scope.policy_ref,"parent":scope.policy.parent,"recipients":[scope.owner]}});
            let digest = nostr::run::logical_digest(&record).map_err(|e| e.to_string())?;
            let body = json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
            self.store.put_json(&body)?;
            scope.journal.push(body);
        }
        let previous = scope.journal.last().ok_or("coordinator root")?["digest"].clone();
        let record = json!({"run":scope.descriptor["run"],"seq":scope.journal.len(),"previous":previous,"controller":self.authority(),"generation":scope.descriptor["epoch"],"type":kind,"subject":{"step":subject,"iteration":null,"attempt":null},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(|e| e.to_string())?;
        let body =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for body in scope.journal.iter().chain(std::iter::once(&body)) {
            let record = nostr::run::parse_envelope(body).map_err(|e| e.to_string())?;
            if nostr::run::ingest(&mut journal, record, self.authority())
                .map_err(|e| e.to_string())?
                != nostr::run::Ingest::Applied
            {
                return Err("coordinator RUN did not extend its exact history".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        scope.journal.push(body);
        Ok(reference)
    }
    pub(crate) fn result(
        &self,
        state: &mut State,
        scope: &Scope,
        input: &Value,
        record: &Value,
        status: &str,
        value: Value,
        reason: Option<&str>,
        recipient: &str,
        now: u64,
    ) -> Result<Value> {
        let result = json!({"v":"openagents.coordination-result.v1","requires":[],"request":input,"coordinator":scope.reference,"revision":scope.revision,"status":status,"record":record,"value":value,"reason":reason});
        coordination::validate(&result).map_err(|e| e.to_string())?;
        self.declare(state, &result, recipient, now)?;
        Ok(result)
    }
    pub fn control_grants(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-control.v1"),
        )?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "command",
                "expected_revision",
                "actor",
                "roles",
                "resources",
                "targets",
                "all",
                "until",
                "revoke",
                "scope_revoke",
            ],
        )?;
        coordination::id(&body["command"]).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        if scope.owner != actor {
            return Err("not_admitted: independent scope owner control required".into());
        }
        let key = format!(
            "{actor}:admin:{}",
            body["command"].as_str().ok_or("command ID")?
        );
        let fingerprint = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fingerprint {
                return Err("idempotency_conflict: changed owner control".into());
            }
            return Ok(previous.result.clone());
        }
        if body["expected_revision"] != scope.revision {
            return Err("stale: coordinator revision changed".into());
        }
        if scope.requests.len() >= 4096 {
            return Err("coordinator request retention limit exceeded".into());
        }
        let until = body["until"].as_u64().ok_or("grant expiry")?;
        let target = body["actor"].as_str().ok_or("grant actor")?;
        let roles: BTreeSet<String> =
            serde_json::from_value(body["roles"].clone()).map_err(|_| "grant roles")?;
        let resources: BTreeSet<String> =
            serde_json::from_value(body["resources"].clone()).map_err(|_| "resource selection")?;
        let targets: BTreeSet<String> =
            serde_json::from_value(body["targets"].clone()).map_err(|_| "target selection")?;
        if !scope.policy.recipients.contains(target) {
            return Err(
                "not_admitted: the immutable policy does not include this recipient".into(),
            );
        }
        if resources.iter().any(|id| !scope.resources.contains_key(id))
            || targets.iter().any(|id| !scope.targets.contains_key(id))
            || until > scope.policy.until
            || body["revoke"].as_bool().is_none()
            || body["scope_revoke"].as_bool().is_none()
        {
            return Err("unsupported coordinator grant scope".into());
        }
        let all = body["all"].as_bool().ok_or("grant selection")?;
        let epoch = scope
            .grants
            .get(target)
            .map(|g| g.epoch.checked_add(1).ok_or("grant epoch exhausted"))
            .transpose()?
            .unwrap_or(0);
        let mut grant = self.grant(target, roles, resources, targets, all, until, epoch)?;
        grant.revoked = body["revoke"] == true;
        let reference = grant.reference.clone();
        scope.grants.insert(target.into(), grant);
        if body["scope_revoke"] == true {
            scope.revoked = true;
        }
        for lease in scope.claims.values_mut() {
            if (scope.revoked || lease.body["worker"] == target)
                && matches!(lease.phase.as_str(), "admitted" | "running" | "unknown")
            {
                if !lease.dispatched {
                    lease.phase = "cancelled".into();
                } else {
                    lease.phase = "cancel_requested".into();
                    lease.unknown = true;
                }
            }
        }
        let input = self.retain(&mut state, event, &body)?;
        scope.revision = scope
            .revision
            .checked_add(1)
            .ok_or("scope revision exhausted")?;
        let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":true,"provenance":{"operation":"owner_grant_control","actor":actor}}),now)?;
        let result = self.result(
            &mut state, &scope, &input, &record, "accepted", reference, None, &actor, now,
        )?;
        scope.requests.insert(
            key,
            crate::book::Request {
                fingerprint,
                result: result.clone(),
            },
        );
        state.scopes.insert(id, scope);
        self.save(&state)?;
        Ok(result)
    }
    pub fn frame(&self, event: &Event, frame_bytes: &[u8], now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.coordination-native-frame.v1"))?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "command",
                "expected_revision",
                "frame",
            ],
        )?;
        coordination::id(&body["command"]).map_err(|e| e.to_string())?;
        let reference = contracts::parse_artifact(&body["frame"]).map_err(|e| e.to_string())?;
        contracts::check_artifact_bytes(&reference, frame_bytes).map_err(|e| e.to_string())?;
        let frame = nostr::work::parse(frame_bytes).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        if actor != scope.owner
            || frame["owner"] != scope.owner
            || frame["controller"] != self.authority()
            || frame["v"] != "openagents.task-frame.v1"
            || reference.schema.as_deref() != Some("openagents.task-frame.v1")
        {
            return Err("not_admitted: exact owner/controller task frame required".into());
        }
        let key = format!(
            "{actor}:frame:{}",
            body["command"].as_str().ok_or("command ID")?
        );
        let fingerprint = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fingerprint {
                return Err("idempotency_conflict: changed frame command".into());
            }
            return Ok(previous.result.clone());
        }
        if scope.requests.len() >= 4096 {
            return Err("coordinator request retention limit exceeded".into());
        }
        if body["expected_revision"] != scope.revision {
            return Err("stale: coordinator revision changed".into());
        }
        let task = frame["task"].as_str().ok_or("task identity")?.to_string();
        if let Some(previous) = scope.frames.get(&task) {
            if previous["reference"] == body["frame"] {
                return Err("conflict: task frame is already registered".into());
            }
            let old = &previous["body"];
            if frame["previous"] != previous["reference"]
                || old["revision"].as_u64().and_then(|v| v.checked_add(1))
                    != frame["revision"].as_u64()
            {
                return Err("stale: frame must extend its retained predecessor".into());
            }
        } else if frame["revision"] != 0 || scope.frames.len() >= 256 {
            return Err("initial task frame or frame retention limit differs".into());
        }
        self.store.put(
            frame_bytes,
            "application/json",
            Some("openagents.task-frame.v1"),
        )?;
        let closure = self.closure(
            &scope,
            &actor,
            &[body["frame"].clone()],
            &Default::default(),
        )?;
        scope.frames.insert(
            task,
            json!({"reference":body["frame"],"body":frame,"event":event.id,"closure":closure}),
        );
        scope.revision += 1;
        let input = self.retain(&mut state, event, &body)?;
        let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":true,"provenance":{"operation":"owner_frame_control","actor":actor}}),now)?;
        let result = self.result(
            &mut state,
            &scope,
            &input,
            &record,
            "accepted",
            body["frame"].clone(),
            None,
            &actor,
            now,
        )?;
        scope.requests.insert(
            key,
            crate::book::Request {
                fingerprint,
                result: result.clone(),
            },
        );
        state.scopes.insert(id, scope);
        self.save(&state)?;
        Ok(result)
    }
}
pub(crate) fn closed(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("closed coordinator object")?;
    if object.len() != fields.len()
        || object.keys().any(|key| !fields.contains(&key.as_str()))
        || value["requires"] != json!([])
    {
        return Err("unsupported coordinator fields".into());
    }
    Ok(())
}
