//! An independently admitted checker signs the exact result it actually compared.
use crate::{
    Result,
    book::{Coordinator, Scope},
};
use nostr::{contracts, domain::Event};
use serde_json::{Value, json};
impl Coordinator {
    pub(crate) fn consume_call(&self, scope: &mut Scope, key: &str) -> Result<()> {
        let lease = scope.claims.get(key).ok_or("claim reservation")?;
        if lease
            .own_calls
            .checked_add(crate::claims::allocated(scope, Some(key)))
            .is_none_or(|n| n >= lease.reserved_calls)
        {
            return Err("limit_exceeded: task allowance is spent or held".into());
        }
        let lease = scope.claims.get_mut(key).ok_or("claim")?;
        lease.own_calls += 1;
        lease.spent_calls += 1;
        let mut parent = lease.parent.clone();
        while let Some(key) = parent {
            let ancestor = scope.claims.get_mut(&key).ok_or("ancestor reservation")?;
            ancestor.spent_calls += 1;
            parent = ancestor.parent.clone();
        }
        scope.calls += 1;
        Ok(())
    }
    pub(crate) fn check_material(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(Value, Vec<u8>, Vec<u8>)> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-check-request.v1"),
        )?;
        crate::admin::closed(&body, &["v", "requires", "coordinator", "claim"])?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "finding", now)?;
        if !scope.policy.checkers.contains(&actor) {
            return Err("not_admitted: independently selected checker required".into());
        }
        let key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .map(|(id, _)| id.clone())
            .ok_or("current claim")?;
        let lease = scope.claims[&key].clone();
        self.current_lease(&scope, &lease, now)?;
        if actor == lease.actor {
            return Err("not_admitted: worker cannot check its own output".into());
        }
        self.resources_current(&scope, &lease.body["resources"], now)?;
        let result = lease
            .result
            .as_ref()
            .filter(|r| r["outcome"] == "completed")
            .ok_or("known completed worker result required")?;
        let p = scope
            .proposals
            .values()
            .find(|p| p.reference == lease.body["proposal"])
            .ok_or("proposal")?;
        self.current_lease(
            &scope,
            &lease,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let material = json!({"v":"openagents.coordination-native-check.v1","requires":[],"coordinator":scope.reference,"proposal":p.reference,"claim":lease.reference,"producer":lease.actor,"checker":actor,"input":p.body["input"],"output":result["output"],"frame":p.body["frame"],"context":p.body["context"],"lock":p.body["lock"],"acceptance":p.body["acceptance"],"resources":lease.body["resources"],"mechanism":"exact-admitted-output","passed":false});
        if let Some(previous) = &lease.check_admission {
            if *previous != material {
                return Err("conflict: check admission changed".into());
            }
        } else {
            self.consume_call(&mut scope, &key)?;
            scope.claims.get_mut(&key).ok_or("claim")?.check_admission = Some(material.clone());
            scope.revision += 1;
            self.save_scope(&mut state, id, scope)?;
        }
        Ok((
            material.clone(),
            self.store.get(&material["input"])?,
            self.store.get(&material["output"])?,
        ))
    }
    pub fn record_check(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.coordination-native-check.v1"))?;
        crate::admin::closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "proposal",
                "claim",
                "producer",
                "checker",
                "input",
                "output",
                "frame",
                "context",
                "lock",
                "acceptance",
                "resources",
                "mechanism",
                "passed",
            ],
        )?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "finding", now)?;
        let key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .map(|(id, _)| id.clone())
            .ok_or("current claim")?;
        let lease = scope.claims[&key].clone();
        self.current_lease(&scope, &lease, now)?;
        self.resources_current(&scope, &lease.body["resources"], now)?;
        let mut expected = lease
            .check_admission
            .clone()
            .ok_or("not_admitted: checker did not admit this exact comparison")?;
        if body["passed"].as_bool().is_none()
            || body["checker"] != actor
            || actor == lease.actor
            || !scope.policy.checkers.contains(&actor)
        {
            return Err("not_admitted: current independent checker required".into());
        }
        expected["passed"] = body["passed"].clone();
        if body != expected {
            return Err("identity_mismatch: check pins differ".into());
        }
        let reference = self.retain(&mut state, event, &body)?;
        if let Some(previous) = &lease.check {
            if *previous != reference {
                return Err("conflict: checker report fork".into());
            }
            return Ok(reference);
        }
        scope.claims.get_mut(&key).ok_or("claim")?.check = Some(reference.clone());
        scope.revision += 1;
        self.journal(&mut scope,"observed",None,json!({"evidence":[reference],"sources":[],"complete":true,"provenance":{"operation":"independent_output_check","checker":actor}}),now)?;
        self.save_scope(&mut state, id, scope)?;
        Ok(reference)
    }
}
pub struct Checker {
    secret: secp256k1::SecretKey,
}
impl Checker {
    pub fn new(secret: secp256k1::SecretKey) -> Self {
        Self { secret }
    }
    pub fn check(
        &self,
        coordinator: &Coordinator,
        scope: &Value,
        claim: &Value,
        now: u64,
    ) -> Result<Event> {
        let request = workspace_host::host::request(
            &self.secret,
            coordinator.authority(),
            json!({"v":"openagents.coordination-native-check-request.v1","requires":[],"coordinator":scope,"claim":claim}),
            now,
        )?;
        let (mut report, input, output) = coordinator.check_material(&request, now)?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(&report["input"]).map_err(|e| e.to_string())?,
            &input,
        )
        .map_err(|e| e.to_string())?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(&report["output"]).map_err(|e| e.to_string())?,
            &output,
        )
        .map_err(|e| e.to_string())?;
        report["passed"] = json!(input == output);
        workspace_host::host::request(&self.secret, coordinator.authority(), report, now)
    }
}
