//! Current fencing and explicit known-outcome release; expiry is not cleanup.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
impl Coordinator {
    /// Local owner inspection, including expired or unresolved reservations.
    pub fn scope(&self, reference: &Value) -> Result<Value> {
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let id = self.scope_key(&state, reference)?;
        let s = &state.scopes[&id];
        Ok(
            json!({"coordinator":s.reference,"transport":s.transport,"revision":s.revision,"allowance":s.allowance,"calls":s.calls,"charged_calls":crate::claims::allocated(s,None),"claims":s.claims,"proposals":s.proposals,"frames":s.frames,"backgrounds":s.backgrounds,"revoked":s.revoked,"frozen":s.frozen}),
        )
    }
    pub fn control(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.coordination-control.v1"))?;
        coordination::validate(&body).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.current_invocation(&state, event)?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let task = scope
            .proposals
            .iter()
            .find(|(_, p)| p.reference == body["proposal"])
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: proposal unavailable")?;
        let action = body["action"].as_str().ok_or("control action")?;
        self.check(
            &scope,
            &actor,
            if action == "status" {
                "read"
            } else {
                "control"
            },
            now,
        )?;
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!("{actor}:control:{fp}");
        if let Some(previous) = scope.requests.get(&key) {
            return Ok(previous.result.clone());
        }
        if scope.requests.len() >= 4096 {
            return Err("limit_exceeded: control retention".into());
        }
        if action != "status" && body["expected_revision"] != scope.revision {
            return Err("stale: scope revision changed".into());
        }
        let mut value = scope.proposals[&task].reference.clone();
        let lease_key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"] && l.body["proposal"] == body["proposal"])
            .map(|(id, _)| id.clone());
        let mut unknown = false;
        if action == "status" {
            let p = &scope.proposals[&task];
            if actor != scope.owner
                && actor != p.author
                && !lease_key
                    .as_ref()
                    .is_some_and(|k| scope.claims[k].actor == actor)
            {
                return Err("not_admitted: task status outside current ownership".into());
            }
            value=self.store.put_json(&json!({"v":"openagents.coordination-native-status.v1","requires":[],"proposal":p.reference,"phase":p.status,"claim":p.claim,"findings":p.findings,"reuse":p.reuse}))?;
        } else if let Some(lease_key) = lease_key {
            let mut lease = scope.claims[&lease_key].clone();
            if actor != scope.owner && actor != lease.actor {
                return Err("not_admitted: claim control belongs to named worker or owner".into());
            }
            match action {
                "renew" => {
                    self.current_lease(&scope, &lease, now)?;
                    if lease.dispatched || lease.execution.is_some() {
                        return Err(
                            "cannot_enforce: prepared execution requires its original deadline"
                                .into(),
                        );
                    }
                    let until = body["requested_until"].as_u64().ok_or("renewal expiry")?;
                    let worker = self.check(&scope, &lease.actor, "claim", now)?;
                    if until <= now
                        || until > now + scope.policy.max_claim_seconds
                        || until > scope.policy.until
                        || until > worker.until
                        || lease.parent.as_ref().is_some_and(|p| {
                            until > scope.claims[p].body["expires_at"].as_u64().unwrap_or(0)
                        })
                    {
                        return Err("not_admitted: renewal exceeds current authority".into());
                    }
                    self.resources_current(&scope, &lease.body["resources"], now)?;
                    scope.token = scope.token.checked_add(1).ok_or("claim token exhausted")?;
                    lease.body["token"] = json!(scope.token);
                    lease.body["expires_at"] = json!(until);
                    lease.body["revision"] = json!(scope.revision + 1);
                    lease.reference = self.declare(&mut state, &lease.body, &lease.actor, now)?;
                    scope.proposals.get_mut(&task).ok_or("task")?.claim =
                        Some(lease.reference.clone());
                    value = lease.reference.clone();
                }
                "release" => {
                    if scope.claims.values().any(|l| {
                        l.parent.as_ref() == Some(&lease_key)
                            && (l.phase != "released" || l.unknown)
                    }) {
                        return Err("unknown: descendant reservations remain held".into());
                    }
                    if lease.unknown || lease.dispatched && lease.result.is_none() {
                        return Err("unknown: dispatcher outcome requires reconciliation".into());
                    }
                    lease.phase = "released".into();
                    value = lease.reference.clone();
                }
                "cancel" => {
                    if lease.dispatched && lease.result.is_none() {
                        lease.phase = "cancel_requested".into();
                        lease.unknown = true;
                        unknown = true;
                    } else {
                        lease.phase = "cancelled".into();
                    }
                    scope.proposals.get_mut(&task).ok_or("task")?.status = "cancelled".into();
                    value = lease.reference.clone();
                }
                _ => return Err("unsupported control".into()),
            }
            scope.claims.insert(lease_key, lease);
        } else if action == "cancel"
            && body["claim"].is_null()
            && actor == scope.owner
            && scope.proposals[&task].claim.is_none()
        {
            scope.proposals.get_mut(&task).ok_or("task")?.status = "cancelled".into();
        } else {
            return Err("stale: exact current claim required".into());
        }
        if action != "status" {
            scope.revision += 1;
        }
        let input = self.retain(&mut state, event, &body)?;
        let record=self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":!unknown,"provenance":{"operation":action,"actor":actor,"unknown":unknown}}),now)?;
        let result = self.result(
            &mut state,
            &scope,
            &input,
            &record,
            if unknown { "unknown" } else { "accepted" },
            value,
            None,
            &actor,
            now,
        )?;
        scope.requests.insert(
            key,
            Request {
                fingerprint: fp,
                result: result.clone(),
            },
        );
        state.scopes.insert(id, scope);
        self.save(&state)?;
        Ok(result)
    }
}
