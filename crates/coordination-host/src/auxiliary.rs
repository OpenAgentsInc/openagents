//! Auxiliary admission consumes the same parent allowance and grants no effects.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
impl Coordinator {
    pub fn charge_auxiliary(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) = self.auxiliary_request(event, now)?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.charge_auxiliary_locked(&mut state, event, &actor, &body, now)
    }
    pub(crate) fn auxiliary_request(&self, event: &Event, now: u64) -> Result<(String, Value)> {
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-auxiliary.v1"),
        )?;
        crate::admin::closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "claim",
                "command",
                "purpose",
                "request",
                "fingerprint",
            ],
        )?;
        coordination::id(&body["command"]).map_err(|e| e.to_string())?;
        coordination::id(&body["request"]).map_err(|e| e.to_string())?;
        let digest = body["fingerprint"]
            .as_str()
            .and_then(|s| s.strip_prefix("sha256:"))
            .ok_or("exact auxiliary request fingerprint required")?;
        coordination::id(&json!(digest)).map_err(|e| e.to_string())?;
        if !matches!(
            body["purpose"].as_str(),
            Some("prepare" | "observe" | "revalidate" | "check" | "recovery")
        ) {
            return Err("unsupported auxiliary purpose".into());
        }
        Ok((actor, body))
    }
    pub(crate) fn charge_auxiliary_locked(
        &self,
        state: &mut crate::book::State,
        event: &Event,
        actor: &str,
        body: &Value,
        now: u64,
    ) -> Result<Value> {
        let id = self.scope_key(state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "control", now)?;
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!(
            "{actor}:aux:{}",
            body["request"].as_str().ok_or("auxiliary request")?
        );
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fp {
                return Err("idempotency_conflict: auxiliary request changed".into());
            }
            return Ok(previous.result.clone());
        }
        if scope.requests.len() >= 4096 {
            return Err("limit_exceeded: auxiliary admission retention".into());
        }
        let parent_key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .map(|(id, _)| id.clone())
            .ok_or("current parent claim required")?;
        let parent = &scope.claims[&parent_key];
        self.current_lease(&scope, parent, now)?;
        if actor != scope.owner && actor != parent.actor {
            return Err("not_admitted: parent controller required".into());
        }
        let proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.body["proposal"])
            .ok_or("parent proposal")?;
        if self.operation(&proposal.body["target"])?["binding_contract"]["operation"]
            != "coord-container"
        {
            return Err("not_admitted: separately reserved container required".into());
        }
        let reservation = parent.body["reservation"].clone();
        self.consume_call(&mut scope, &parent_key)?;
        scope.revision += 1;
        let input = self.retain(state, event, &body)?;
        let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":true,"provenance":{"operation":"auxiliary-call-charge","calls":1,"spend_microunits":0,"request":body["request"]}}),now)?;
        let receipt = json!({"v":"openagents.coordination-native-charge.v1","requires":[],"coordinator":scope.reference,"reservation":reservation,"request":body["request"],"fingerprint":body["fingerprint"],"calls":1,"spend_microunits":0,"record":record});
        self.declare(state, &receipt, &actor, now)?;
        scope.requests.insert(
            key,
            Request {
                fingerprint: fp,
                result: receipt.clone(),
            },
        );
        self.save_scope(state, id, scope)?;
        Ok(receipt)
    }
}
