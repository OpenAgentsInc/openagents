//! The source owner applies exact checked output under its own conditional write.
use crate::{Result, book::Coordinator};
use nostr::{contracts, domain::Event, workspace};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn integrate(
        &self,
        event: &Event,
        now: u64,
        approvals: Option<&dyn workspace_host::host::ApprovalVerifier>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-integration.v1"),
        )?;
        crate::admin::closed(&body, &["v", "requires", "coordinator", "claim", "recover"])?;
        let recover = body["recover"].as_bool().ok_or("recovery selection")?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .map(|(id, _)| id.clone())
            .ok_or("current claim")?;
        let lease = scope.claims[&key].clone();
        let grant = self.check(
            &scope,
            &actor,
            if recover { "read" } else { "integrate" },
            now,
        )?;
        if recover && actor != scope.owner {
            return Err("not_admitted: source reconciliation belongs to owner".into());
        }
        if let Some(previous) = &lease.integration {
            if !previous["outcome"].is_null() {
                return Ok(previous["outcome"].clone());
            }
            if !recover {
                return Err("unknown: integration needs explicit source reconciliation".into());
            }
            let resource = scope
                .resources
                .get(
                    previous["resource"]
                        .as_str()
                        .ok_or("integration resource")?,
                )
                .ok_or("source")?;
            self.source_current(resource, &Value::Null, false, now)?;
            let source = self
                .sources
                .get(&resource.source)
                .ok_or("source not mounted")?;
            let result = source.coordination_recover_command(
                previous["command"].as_str().ok_or("source command")?,
                previous["fingerprint"]
                    .as_str()
                    .ok_or("source fingerprint")?,
                now,
            )?;
            let result=result.unwrap_or_else(||json!({"v":"openagents.coordination-native-source-absence.v1","requires":[],"outcome":"refused","dispatched":false,"command":previous["command"],"fingerprint":previous["fingerprint"],"observed_at":now}));
            return self.finish_integration(&mut state, id, scope, &key, result, now);
        }
        if recover {
            return Err("not_admitted: no retained integration intent".into());
        }
        self.current_lease(&scope, &lease, now)?;
        self.resources_current(&scope, &lease.body["resources"], now)?;
        let task = scope
            .proposals
            .iter()
            .find(|(_, p)| p.reference == lease.body["proposal"])
            .map(|(id, _)| id.clone())
            .ok_or("proposal")?;
        let proposal = scope.proposals[&task].clone();
        let target = proposal.body["target"]["id"].as_str().ok_or("target")?;
        if !grant.all && !grant.targets.contains(target) {
            return Err("not_admitted: integration target outside grant".into());
        }
        let report = self.store.json(
            lease
                .check
                .as_ref()
                .ok_or("not_admitted: independent check required")?,
        )?;
        if report["input"]["digest"] != report["output"]["digest"]
            || report["passed"] != true
            || report["claim"] != lease.reference
            || report["producer"] == report["checker"]
            || !scope
                .policy
                .checkers
                .contains(report["checker"].as_str().ok_or("checker")?)
        {
            return Err("not_admitted: exact independent passing check required".into());
        }
        self.check(
            &scope,
            report["checker"].as_str().ok_or("checker")?,
            "finding",
            now,
        )?;
        let operation = self.operation(&proposal.body["target"])?;
        if operation["binding_contract"]["operation"] != "openagents.coord-native-git-change.v1" {
            return Err("unsupported_feature: data results have no source integration".into());
        }
        let output = self.store.json(&report["output"])?;
        let resource_id = output["resource"]
            .as_str()
            .ok_or("output resource")?
            .to_string();
        if operation["binding_contract"]["resource"] != resource_id
            || !grant.all && !grant.resources.contains(&resource_id)
        {
            return Err("not_admitted: integration resource outside exact operation grant".into());
        }
        let claim_resource = lease.body["resources"]
            .as_array()
            .ok_or("claim resources")?
            .iter()
            .find(|r| {
                r["id"] == resource_id
                    && r["mode"] == "write"
                    && r["snapshot"] == output["snapshot"]
            })
            .ok_or("not_admitted: exact current write claim and source snapshot required")?;
        let resource = scope.resources.get(&resource_id).ok_or("source resource")?;
        self.source_current(resource, &claim_resource["snapshot"], true, now)?;
        let source = self
            .sources
            .get(&resource.source)
            .ok_or("source not mounted")?;
        let resource_ref = workspace::resource(&resource.reference).map_err(|e| e.to_string())?;
        let snapshot = self.store.json(&output["snapshot"])?;
        let content = self.store.get(&output["content"])?;
        if content.len() > 1_048_576 || std::str::from_utf8(&content).is_err() {
            return Err("limit_exceeded: UTF-8 document replacement required".into());
        }
        let read = workspace_host::host::request(
            &self.secret,
            self.authority(),
            json!({"v":"openagents.ws-resource-read.v1","requires":[],"resource":resource_ref}),
            now,
        )?;
        let current = source.read_document(&read, now)?;
        let command=contracts::digest_value(&json!({"claim":lease.reference,"output":report["output"],"operation":"conditional-integration"})).map_err(|e|e.to_string())?.trim_start_matches("sha256:").to_string();
        let replacement = json!({"v":"openagents.document-replacement.v1","requires":[],"snapshot":output["snapshot"],"content":output["content"],"encoding":"openagents.git-blob-utf8.v1"});
        let replacement_ref = self.store.put_json(&replacement)?;
        let change = json!({"v":"openagents.resource-change.v1","requires":[],"command":command,"actor":self.authority(),"resource":resource_ref,"binding":resource.binding,"expected":snapshot["observation"],"operation":current["operation"],"input":replacement_ref,"authorization":current["authorization"],"issued_at":now,"expires_at":lease.body["expires_at"]});
        let change_ref = self.store.put_json(&change)?;
        let admission = json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":change_ref,"actor":self.authority(),"approval":null});
        let source_event =
            workspace_host::host::request(&self.secret, self.authority(), admission.clone(), now)?;
        let fingerprint = contracts::digest_value(&admission).map_err(|e| e.to_string())?;
        let mut files = BTreeMap::from([(
            output["content"]["digest"]
                .as_str()
                .ok_or("content digest")?
                .into(),
            content,
        )]);
        for value in [&replacement, &change] {
            let bytes = contracts::jcs(value).map_err(|e| e.to_string())?;
            files.insert(contracts::digest_bytes(&bytes), bytes);
        }
        let effective_now = now.saturating_add(started.elapsed().as_secs());
        self.current_lease(&scope, &lease, effective_now)?;
        self.check(&scope, &actor, "integrate", effective_now)?;
        self.consume_call(&mut scope, &key)?;
        scope.claims.get_mut(&key).ok_or("claim")?.integration = Some(
            json!({"v":"openagents.coordination-native-integration-intent.v1","requires":[],"resource":resource_id,"command":command,"fingerprint":fingerprint,"request":source_event,"outcome":null}),
        );
        scope.claims.get_mut(&key).ok_or("claim")?.unknown = true;
        scope.claims.get_mut(&key).ok_or("claim")?.phase = "integrating".into();
        scope.revision += 1;
        self.journal(&mut scope,"dispatched",Some(&format!("integration:{command}")),json!({"binding":current["operation"],"attempt":1,"input":change_ref,"context":proposal.body["context"],"effect":"conditional-source-write","generation":0}),now)?;
        self.save_scope(&mut state, id.clone(), scope.clone())?;
        let result = source.change(
            &source_event,
            &files,
            now.saturating_add(started.elapsed().as_secs()),
            approvals,
        )?;
        self.finish_integration(&mut state, id, scope, &key, result, now)
    }
    fn finish_integration(
        &self,
        state: &mut crate::book::State,
        id: String,
        mut scope: crate::book::Scope,
        key: &str,
        result: Value,
        now: u64,
    ) -> Result<Value> {
        let unknown = !matches!(
            result["outcome"].as_str(),
            Some("completed" | "refused" | "failed" | "cancelled")
        );
        let lease = scope.claims.get_mut(key).ok_or("claim")?;
        lease.unknown = unknown;
        lease.phase = if unknown { "unknown" } else { "completed" }.into();
        let intent = lease.integration.as_mut().ok_or("integration intent")?;
        intent["outcome"] = result.clone();
        let command = intent["command"]
            .as_str()
            .ok_or("source command")?
            .to_string();
        let proposal = lease.body["proposal"].clone();
        if result["outcome"] == "completed" {
            scope
                .proposals
                .values_mut()
                .find(|p| p.reference == proposal)
                .ok_or("proposal")?
                .status = "integrated".into();
        }
        scope.revision += 1;
        let receipt = self.store.put_json(&result)?;
        self.journal(&mut scope,"observed",Some(&format!("integration:{command}")),json!({"evidence":[receipt],"sources":[],"complete":!unknown,"provenance":{"operation":"source-owner-integration","outcome":result["outcome"]}}),now)?;
        self.save_scope(state, id, scope)?;
        Ok(result)
    }
}
