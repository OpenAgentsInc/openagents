//! Local owner cleanup can fence work after authority expires, but cannot run it.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{automation, contracts, coordination, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeSet;
impl Coordinator {
    /// Local owner capability, deliberately absent from the remote CAP endpoint.
    /// Cancel exact native work, retain unknown effects, and sign the actual facts.
    pub fn automation_cleanup(
        &self,
        plan: &Value,
        occurrence: &Value,
        proposal: &Value,
        terminal: bool,
        now: u64,
    ) -> Result<Event> {
        automation::validate(plan).map_err(|e| e.to_string())?;
        automation::validate(occurrence).map_err(|e| e.to_string())?;
        coordination::validate(proposal).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &plan["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let controller = plan["controller"].as_str().ok_or("controller")?;
        let task = occurrence["task"].as_str().ok_or("occurrence task")?;
        let plan_ref = self.store.put_json(plan)?;
        let occurrence_ref = self.store.put_json(occurrence)?;
        if plan["owner"] != scope.owner
            || occurrence["plan"] != plan_ref
            || plan["target"] != self.automation_operation()?
            || proposal["task"] != occurrence["task"]
            || proposal["parent"] != plan["task"]
            || proposal["target"] != plan["target"]
            || proposal["coordinator"] != plan["coordinator"]
            || proposal["frame"] != plan["frame"]
        {
            return Err("not_admitted: exact local native automation required".into());
        }
        let parent = scope
            .claims
            .iter()
            .find(|(_, l)| l.body["reservation"] == plan["reservation"])
            .map(|(k, l)| (k.clone(), l.clone()))
            .ok_or("admitted parent")?;
        let parent_proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.1.body["proposal"])
            .ok_or("parent proposal")?;
        if parent.1.actor != controller
            || parent_proposal.body["task"] != plan["task"]
            || self.operation(&parent_proposal.body["target"])?["binding_contract"]["operation"]
                != "coord-container"
        {
            return Err("not_admitted: cleanup parent differs".into());
        }
        let proposal_ref = self.store.put_json(proposal)?;
        if let Some(p) = scope.proposals.get(task) {
            if p.reference != proposal_ref || p.author != controller {
                return Err("identity_mismatch: cleanup proposal differs".into());
            }
        }
        let fence_key = format!("automation:fenced-task:{task}");
        if !scope.requests.contains_key(&fence_key) && scope.requests.len() >= 4096 {
            return Err("cleanup retention bound".into());
        }
        // Fencing a not-yet-admitted identity is atomic with proposal admission.
        scope.requests.entry(fence_key).or_insert(Request {
            fingerprint: contracts::digest_value(proposal).map_err(|e| e.to_string())?,
            result: proposal_ref.clone(),
        });
        let root = scope
            .claims
            .iter()
            .find(|(_, l)| l.body["proposal"] == proposal_ref)
            .map(|(k, _)| k.clone());
        let mut managed = BTreeSet::new();
        for key in scope.claims.keys() {
            let mut cursor = Some(key.as_str());
            for _ in 0..64 {
                let Some(k) = cursor else { break };
                if (terminal && k == parent.0) || root.as_deref() == Some(k) {
                    managed.insert(key.clone());
                    break;
                }
                cursor = scope.claims.get(k).and_then(|l| l.parent.as_deref());
            }
        }
        // Cancel all descendants before releasing any of their reservations.
        for key in &managed {
            let lease = scope.claims.get_mut(key).ok_or("managed claim")?;
            if lease.dispatched && lease.result.is_none() {
                lease.unknown = true;
                lease.phase = "cancel_requested".into();
            } else if lease.phase != "released" {
                lease.phase = "cancelled".into();
            }
            if let Some(p) = scope
                .proposals
                .values_mut()
                .find(|p| p.reference == lease.body["proposal"])
            {
                p.status = "cancelled".into();
            }
        }
        if let Some(p) = scope.proposals.get_mut(task) {
            p.status = "cancelled".into();
        }
        for _ in 0..64 {
            let releasable: Vec<_> = managed
                .iter()
                .filter(|key| {
                    let l = &scope.claims[*key];
                    !l.unknown
                        && (!l.dispatched || l.result.is_some())
                        && l.phase != "released"
                        && !scope.claims.values().any(|c| {
                            c.parent.as_ref() == Some(*key) && (c.phase != "released" || c.unknown)
                        })
                })
                .cloned()
                .collect();
            if releasable.is_empty() {
                break;
            }
            for key in releasable {
                scope.claims.get_mut(&key).ok_or("claim")?.phase = "released".into();
            }
        }
        let unknown = managed.iter().any(|k| {
            let l = &scope.claims[k];
            l.unknown || l.phase != "released"
        });
        let lease = root.as_ref().and_then(|k| scope.claims.get(k));
        let result = lease.and_then(|l| l.result.clone());
        let claim = lease.map(|l| l.reference.clone());
        let dispatched = lease.is_some_and(|l| l.dispatched);
        let cache_key = format!(
            "automation:cleanup:{task}:{terminal}:{}",
            if unknown { "unknown" } else { "known" }
        );
        if let Some(previous) = scope.requests.get(&cache_key).filter(|_| !unknown) {
            let d = previous.result["digest"].as_str().ok_or("cleanup proof")?;
            return state
                .originals
                .get(d)
                .and_then(|e| e.iter().find(|e| e.pubkey == self.authority()))
                .cloned()
                .ok_or("original cleanup proof".into());
        }
        if scope.requests.len() >= 4096 {
            return Err("cleanup proof retention bound".into());
        }
        let record = if !unknown && !dispatched {
            Some(self.journal(&mut scope,"resolved",Some(occurrence["request"].as_str().ok_or("request")?),json!({"outcome":"cancelled","dispatched":false,"output":null,"receipts":[],"usage":{"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now)?)
        } else {
            None
        };
        scope.revision += 1;
        let proof = json!({"v":"openagents.coordination-native-automation-cleanup.v1","requires":[],"coordinator":scope.reference,"plan":plan_ref,"occurrence":occurrence_ref,"proposal":proposal_ref,"claim":claim,"result":result,"dispatched":dispatched,"known":!unknown,"record":record,"terminal":terminal,"revision":scope.revision});
        let event = workspace_host::host::request(&self.secret, controller, proof.clone(), now)?;
        let reference = self.retain(&mut state, &event, &proof)?;
        scope.requests.insert(
            cache_key,
            Request {
                fingerprint: contracts::digest_value(&proof).map_err(|e| e.to_string())?,
                result: reference,
            },
        );
        self.save_scope(&mut state, id, scope)?;
        Ok(event)
    }
}
