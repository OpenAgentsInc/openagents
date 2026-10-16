//! An admitted checker reads one counted snapshot of actual child outcomes.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{automation, contracts, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn automation_evaluation_authorization(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value)> {
        self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-auxiliary.v1"),
        )
    }
    pub fn automation_outcome_authorization(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value)> {
        self.authenticate(event, now, Some("openagents.automation-outcome.v1"))
    }
    pub fn automation_evaluation_material(
        &self,
        event: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Event> {
        let started = std::time::Instant::now();
        let (checker, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-evaluation-material.v1"),
        )?;
        crate::admin::closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "claim",
                "request",
                "input",
                "rule",
                "admission",
                "outcome",
            ],
        )?;
        coordination_id(&body["request"])?;
        let admission: Event = serde_json::from_value(body["admission"].clone())
            .map_err(|_| "original controller admission")?;
        let (controller, aux) = self.automation_evaluation_authorization(&admission, now)?;
        crate::admin::closed(
            &aux,
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
        coordination_id(&aux["command"])?;
        let outcome_event: Event = serde_json::from_value(body["outcome"].clone())
            .map_err(|_| "original outcome declaration")?;
        let (outcome_controller, outcome) = self.authenticate(
            &outcome_event,
            now,
            Some("openagents.automation-outcome.v1"),
        )?;
        automation::validate(&outcome).map_err(|e| e.to_string())?;
        let input_artifact =
            contracts::parse_artifact(&body["input"]).map_err(|e| e.to_string())?;
        let input_bytes = files
            .get(&input_artifact.digest)
            .ok_or("exact evaluation input bytes")?;
        contracts::check_artifact_bytes(&input_artifact, input_bytes).map_err(|e| e.to_string())?;
        let input = automation::parse(input_bytes).map_err(|e| e.to_string())?;
        if input["v"] != "openagents.automation-evaluation-input.v1"
            || outcome_controller != controller
            || aux["coordinator"] != body["coordinator"]
            || aux["claim"] != body["claim"]
            || aux["request"] != body["request"]
            || aux["purpose"] != "check"
            || aux["fingerprint"] != contracts::digest_value(&input).map_err(|e| e.to_string())?
            || input["outcome"]["digest"]
                != contracts::digest_value(&outcome).map_err(|e| e.to_string())?
            || input["occurrence"] != outcome["occurrence"]
        {
            return Err(
                "identity_mismatch: exact signed evaluation admission/outcome required".into(),
            );
        }
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(&input["outcome"]).map_err(|e| e.to_string())?,
            &contracts::jcs(&outcome).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let key = format!(
            "{checker}:auto-evaluation:{}",
            body["request"].as_str().ok_or("request")?
        );
        let fingerprint = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fingerprint {
                return Err("idempotency_conflict: evaluation material request changed".into());
            }
            if previous.result["status"] == "pending" {
                return Err("unknown: interrupted evaluation snapshot was not repeated".into());
            }
            return serde_json::from_value(previous.result["event"].clone())
                .map_err(|_| "retained evaluation material".into());
        }
        if scope.requests.len() >= 4094 {
            return Err("evaluation admission retention bound".into());
        }
        self.check(&scope, &checker, "finding", now)?;
        self.check(&scope, &controller, "control", now)?;
        if !scope.policy.checkers.contains(&checker) || checker == controller {
            return Err("not_admitted: independently selected checker required".into());
        }
        let (parent_key, parent) = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .ok_or("parent claim")?;
        self.current_lease(&scope, parent, now)?;
        if parent.actor != controller {
            return Err("not_admitted: exact parent controller required".into());
        }
        let parent_key = parent_key.clone();
        let parent_proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.body["proposal"])
            .ok_or("parent proposal")?;
        if self.operation(&parent_proposal.body["target"])?["binding_contract"]["operation"]
            != "coord-container"
        {
            return Err("not_admitted: nondispatchable evaluation parent required".into());
        }
        let frame = self.store.json(&input["frame"])?;
        if parent_proposal.body["frame"] != input["frame"]
            || !frame["acceptance"]
                .as_array()
                .is_some_and(|a| a.contains(&body["rule"]) && a.contains(&input["acceptance"]))
        {
            return Err("not_admitted: protected current completion rule required".into());
        }
        let rule = self.store.json(&body["rule"])?;
        crate::admin::closed(
            &rule,
            &["v", "requires", "check", "minimum_successes", "acceptance"],
        )?;
        if rule["v"] != "openagents.automation-native-completion-rule.v1"
            || rule["check"] != "verified-occurrence-count"
            || rule["acceptance"] != input["acceptance"]
            || rule["minimum_successes"]
                .as_u64()
                .is_none_or(|n| !(1..=16).contains(&n))
        {
            return Err("cannot_enforce: native completion rule differs".into());
        }
        self.attachments(files)?;
        let plan = self.store.json(&input["plan"])?;
        automation::validate(&plan).map_err(|e| e.to_string())?;
        if plan["owner"] != scope.owner
            || plan["controller"] != controller
            || plan["coordinator"] != scope.reference
            || plan["frame"] != input["frame"]
            || plan["acceptance"] != input["acceptance"]
            || plan["task"] != parent_proposal.body["task"]
            || plan["reservation"] != scope.claims[&parent_key].body["reservation"]
        {
            return Err("identity_mismatch: evaluation plan differs from actual parent".into());
        }
        let occurrence = self.store.json(&input["occurrence"])?;
        automation::validate(&occurrence).map_err(|e| e.to_string())?;
        if occurrence["plan"] != input["plan"] {
            return Err("identity_mismatch: evaluated occurrence plan differs".into());
        }
        let current = scope
            .claims
            .values()
            .find(|l| l.reference == outcome["claim"] && l.body["proposal"] == outcome["proposal"])
            .ok_or("actual evaluated child claim")?;
        let current_result = current
            .result
            .as_ref()
            .ok_or("actual evaluated child result")?;
        if current
            .input
            .as_ref()
            .is_none_or(|p| p["request"] != occurrence["request"] || p["run"] != occurrence["run"])
            || outcome["result"]["digest"]
                != contracts::digest_value(current_result).map_err(|e| e.to_string())?
            || !current_result["receipts"]
                .as_array()
                .is_some_and(|r| r.contains(&outcome["settled"]))
        {
            return Err("identity_mismatch: evaluated result or terminal differs".into());
        }
        let aux_key = format!(
            "{controller}:aux:{}",
            body["request"].as_str().ok_or("request")?
        );
        let aux_fp = contracts::digest_value(&aux).map_err(|e| e.to_string())?;
        if let Some(previous) = scope.requests.get(&aux_key) {
            if previous.fingerprint != aux_fp {
                return Err("idempotency_conflict: evaluation allowance request changed".into());
            }
        } else {
            self.consume_call(&mut scope, &parent_key)?;
            let input_ref = self.retain(&mut state, &admission, &aux)?;
            let receipt=self.store.put_json(&json!({"v":"openagents.coordination-native-charge.v1","requires":[],"coordinator":scope.reference,"reservation":scope.claims[&parent_key].body["reservation"],"request":aux["request"],"fingerprint":aux["fingerprint"],"calls":1,"spend_microunits":0,"evidence":input_ref}))?;
            scope.requests.insert(
                aux_key,
                Request {
                    fingerprint: aux_fp,
                    result: receipt,
                },
            );
        }
        self.retain(&mut state, &outcome_event, &outcome)?;
        let original = self.retain(&mut state, event, &body)?;
        scope.requests.insert(
            key.clone(),
            Request {
                fingerprint: fingerprint.clone(),
                result: json!({"status":"pending"}),
            },
        );
        scope.revision += 1;
        self.journal(&mut scope,"observed",None,json!({"evidence":[original],"sources":[],"complete":false,"provenance":{"operation":"automation-evaluation-intent","request":body["request"],"calls":1}}),now)?;
        self.save_scope(&mut state, id.clone(), scope.clone())?;
        let mut children = vec![];
        let mut unknowns = vec![];
        for (lease_key, lease) in &scope.claims {
            if lease_key == &parent_key {
                continue;
            }
            let mut ancestor = lease.parent.as_deref();
            let mut belongs = false;
            for _ in 0..64 {
                let Some(key) = ancestor else {
                    break;
                };
                if key == parent_key {
                    belongs = true;
                    break;
                }
                ancestor = scope.claims.get(key).ok_or("ancestor")?.parent.as_deref();
            }
            if !belongs {
                continue;
            }
            if lease.phase != "released"
                || lease.unknown
                || lease.dispatched && lease.result.is_none()
            {
                unknowns.push(lease.reference.clone());
            }
            let proposal = scope
                .proposals
                .values()
                .find(|p| p.reference == lease.body["proposal"])
                .ok_or("child proposal")?;
            let frozen = self.store.json(&proposal.body["input"])?;
            if frozen["v"] != "openagents.automation-input.v1" || frozen["plan"] != input["plan"] {
                continue;
            }
            let occurrence_body = self.store.json(&frozen["occurrence"])?;
            let linked = occurrence_body["plan"] == input["plan"]
                && occurrence_body["task"] == proposal.body["task"]
                && lease.input.as_ref().is_some_and(|e| {
                    e["request"] == occurrence_body["request"] && e["run"] == occurrence_body["run"]
                });
            let check = lease
                .check
                .as_ref()
                .map(|r| self.store.json(r))
                .transpose()?;
            let result_ref = lease
                .result
                .as_ref()
                .map(|r| self.store.put_json(r))
                .transpose()?;
            let terminal = lease
                .result
                .as_ref()
                .and_then(|r| r["receipts"].as_array())
                .and_then(|r| r.iter().find(|r| r["schema"] == nostr::run::RECORD_SCHEMA))
                .cloned();
            children.push(json!({"claim":lease.reference,"proposal":proposal.reference,"plan":frozen["plan"],"occurrence":frozen["occurrence"],"ordinal":occurrence_body["ordinal"],"linked":linked,"frame":proposal.body["frame"],"acceptance":proposal.body["acceptance"],"target":proposal.body["target"],"phase":lease.phase,"unknown":lease.unknown,"producer":lease.actor,"outcome":lease.result.as_ref().map(|r|r["outcome"].clone()),"result":result_ref,"terminal":terminal,"check":lease.check,"passed":check.as_ref().is_some_and(|c|c["passed"]==true&&c["checker"]==checker&&c["producer"]==lease.actor&&c["input"]==proposal.body["input"]&&lease.result.as_ref().is_some_and(|r|c["output"]==r["output"]))}));
        }
        if children.len() > 16 || unknowns.len() > 64 {
            return Err("limit_exceeded: native evaluation material bound".into());
        }
        let effective_now = now.saturating_add(started.elapsed().as_secs());
        self.check(&scope, &checker, "finding", effective_now)?;
        self.current_lease(&scope, &scope.claims[&parent_key], effective_now)?;
        if effective_now >= plan["expires_at"].as_u64().ok_or("plan expiry")? {
            return Err("stale: evaluation plan expired".into());
        }
        let material = json!({"v":"openagents.coordination-native-evaluation-data.v1","requires":[],"input":body["input"],"plan":input["plan"],"frame":input["frame"],"acceptance":input["acceptance"],"rule":body["rule"],"rule_body":rule,"target":plan["target"],"producer":current_producer(&scope,&outcome["claim"])? ,"children":children,"unknowns":unknowns,"captured_at":now});
        let response =
            workspace_host::host::request(&self.secret, &checker, material.clone(), now)?;
        let reference = self.retain(&mut state, &response, &material)?;
        scope.requests.insert(
            key,
            Request {
                fingerprint,
                result: json!({"event":response,"material":reference}),
            },
        );
        scope.revision += 1;
        self.save_scope(&mut state, id, scope)?;
        Ok(response)
    }
}
fn coordination_id(value: &Value) -> Result<()> {
    nostr::coordination::id(value)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
fn current_producer(scope: &crate::book::Scope, claim: &Value) -> Result<String> {
    scope
        .claims
        .values()
        .find(|l| l.reference == *claim)
        .map(|l| l.actor.clone())
        .ok_or("current producer".into())
}
