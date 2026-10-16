//! Finite source/completion subscriptions produce ordinary bounded child claims.
use crate::{Result, book::Coordinator, operations::Limits};
use nostr::{contracts, coordination, domain::Event, workspace};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn activate_background(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now, Some("openagents.background-plan.v1"))?;
        coordination::validate(&body).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        self.check(&scope, &actor, "background", now)?;
        if actor != scope.owner || body["owner"] != actor || body["lifetime"] == "session" {
            return Err("not_admitted: native background lifetime requires explicit owner and task or operation".into());
        }
        if body["destination"] != "evidence" || body["stale_policy"] != "retain_as_stale" {
            return Err(
                "unsupported_feature: native background results use retained evidence".into(),
            );
        }
        let reference = workspace_host::artifact::json_ref(&body)?;
        let digest = reference["digest"]
            .as_str()
            .ok_or("background digest")?
            .to_string();
        if let Some(previous) = scope.backgrounds.get(&digest) {
            return Ok(previous.clone());
        }
        if scope.backgrounds.len() >= 256 {
            return Err("limit_exceeded: background plans".into());
        }
        let inputs = self.store.json(&body["inputs"])?;
        crate::admin::closed(
            &inputs,
            &[
                "v",
                "requires",
                "template",
                "parent_claim",
                "sources",
                "watch",
                "max_occurrences",
            ],
        )?;
        if inputs["v"] != "openagents.coordination-native-background-inputs.v1" {
            return Err("unsupported_feature: native background inputs".into());
        }
        let max = inputs["max_occurrences"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 256)
            .ok_or("finite background occurrence limit")?;
        let template = &inputs["template"];
        coordination::validate(template).map_err(|e| e.to_string())?;
        let parent = scope
            .claims
            .values()
            .find(|l| l.reference == inputs["parent_claim"])
            .ok_or("independently reserved parent claim required")?;
        self.current_lease(&scope, parent, now)?;
        let parent_proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.body["proposal"])
            .ok_or("parent task")?;
        if self.operation(&parent_proposal.body["target"])?["binding_contract"]["operation"]
            != "coord-container"
            || template["parent"] != parent_proposal.body["task"]
            || template["coordinator"] != scope.reference
            || template["frame"] != body["task_frame"]
            || template["target"] != body["operation"]
            || template["lock"] != body["lock"]
            || body["operation"] != self.data_operation()?
            || template["reuse"] != "never"
        {
            return Err("not_admitted: exact reserved native data template required".into());
        }
        self.check_frame(&scope, template)?;
        let effects = contracts::parse_effects(&template["effects"]).map_err(|e| e.to_string())?;
        if !effects.writes.is_empty()
            || effects.spend
            || effects.process
            || effects.delegates
            || !effects.network.is_empty()
        {
            return Err("not_admitted: background operation must be read-only".into());
        }
        let aggregate = Limits::parse(&body["bounds"], &scope.policy)?;
        let per = Limits::parse(&template["bounds"], &scope.policy)?;
        if aggregate.calls < per.calls + 1
            || aggregate.calls > parent.reserved_calls
            || body["expires_at"].as_u64().is_none_or(|until| {
                until <= now || until > parent.body["expires_at"].as_u64().unwrap_or(0)
            })
        {
            return Err(
                "not_admitted: finite background aggregate and parent lifetime required".into(),
            );
        }
        let disclosure = self.store.json(&body["disclosure"])?;
        if disclosure
            != json!({"v":"openagents.coordination-native-disclosure.v1","requires":[],"recipients":scope.policy.recipients})
        {
            return Err("not_admitted: exact scope disclosure required".into());
        }
        let sources: Vec<String> =
            serde_json::from_value(inputs["sources"].clone()).map_err(|_| "background sources")?;
        if sources.len() > 64
            || sources.iter().any(|id| {
                !scope.resources.contains_key(id)
                    || !effects.reads.contains(id)
                    || !contracts::parse_effects(&scope.policy.effects)
                        .is_ok_and(|e| e.reads.contains(id))
            })
            || inputs["watch"]
                .as_str()
                .is_none_or(|id| !scope.proposals.contains_key(id))
            || body["trigger"] == "snapshot_changed" && sources.is_empty()
        {
            return Err("not_admitted: exact native source and watch identities required".into());
        }
        let watch = &scope.proposals[inputs["watch"].as_str().ok_or("watch task")?];
        if watch.body["frame"] != body["task_frame"] {
            return Err("not_admitted: background watch must use the admitted task frame".into());
        }
        self.closure(
            &scope,
            &actor,
            &[body["inputs"].clone(), body["disclosure"].clone()],
            &BTreeMap::new(),
        )?;
        self.retain(&mut state, event, &body)?;
        let entry = json!({"reference":reference,"body":body,"owner":actor,"max_occurrences":max,"occurrences":0,"charged_calls":0,"last_effective":null,"active":null,"pending":null,"closed":false,"outputs":[]});
        scope.backgrounds.insert(digest, entry.clone());
        scope.revision += 1;
        self.save_scope(&mut state, id, scope)?;
        Ok(entry)
    }
    /// Poll only admitted native sources; producer findings are never triggers.
    pub fn poll_background(&self, event: &Event, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.coordination-native-background-poll.v1"),
        )?;
        crate::admin::closed(&body, &["v", "requires", "coordinator", "plan"])?;
        let (id, plan_key, mut proposal, resources, parent_reservation, worker, until);
        {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            id = self.scope_key(&state, &body["coordinator"])?;
            let mut scope = state.scopes[&id].clone();
            self.check(&scope, &actor, "background", now)?;
            plan_key = body["plan"]["digest"]
                .as_str()
                .ok_or("background plan digest")?
                .to_string();
            let mut entry = scope
                .backgrounds
                .get(&plan_key)
                .filter(|e| e["reference"] == body["plan"])
                .ok_or("background plan not active")?
                .clone();
            if actor != scope.owner || entry["owner"] != actor {
                return Err("not_admitted: background owner required".into());
            }
            if !entry["pending"].is_null() {
                return Err(
                    "unknown: background admission was interrupted; no new occurrence".into(),
                );
            }
            let plan = entry["body"].clone();
            let inputs = self.store.json(&plan["inputs"])?;
            if entry["closed"] == true
                || now >= plan["expires_at"].as_u64().ok_or("plan expiry")?
                || entry["occurrences"].as_u64().unwrap_or(0)
                    >= entry["max_occurrences"].as_u64().unwrap_or(0)
            {
                return Err("stale: background lifetime ended".into());
            }
            if let Some(active) = entry["active"].as_str() {
                if scope.claims.get(active).is_some_and(|l| {
                    l.unknown || !matches!(l.phase.as_str(), "completed" | "released" | "cancelled")
                }) {
                    return Ok(json!({"active":scope.claims[active].reference}));
                }
            }
            let parent_key = scope
                .claims
                .iter()
                .find(|(_, l)| l.reference == inputs["parent_claim"])
                .map(|(key, _)| key.clone())
                .ok_or("background parent claim changed")?;
            let parent = scope.claims[&parent_key].clone();
            self.current_lease(&scope, &parent, now)?;
            if scope.claims.iter().any(|(key, l)| {
                key != &parent_key
                    && l.parent.as_deref() != Some(&parent_key)
                    && matches!(
                        l.phase.as_str(),
                        "admitted" | "running" | "integrating" | "unknown"
                    )
            }) {
                return Err("unavailable: foreground work has priority".into());
            }
            let template = &inputs["template"];
            if self.check_frame(&scope, template).is_err() {
                entry["closed"] = json!(true);
                scope.backgrounds.insert(plan_key.clone(), entry);
                self.save_scope(&mut state, id, scope)?;
                return Err("stale: background task frame changed".into());
            }
            let watch = scope
                .proposals
                .get(inputs["watch"].as_str().ok_or("watch")?)
                .ok_or("watch task")?
                .clone();
            let watch_closed = matches!(watch.status.as_str(), "cancelled" | "integrated")
                || scope
                    .claims
                    .values()
                    .any(|l| l.body["proposal"] == watch.reference && l.phase == "released");
            let trigger = plan["trigger"].as_str().ok_or("background trigger")?;
            if trigger != "task_closed" && watch_closed {
                return Err("stale: background task closed".into());
            }
            if trigger == "operation_completed"
                && !matches!(watch.status.as_str(), "completed" | "integrated")
                || trigger == "task_closed" && !watch_closed
            {
                return Ok(json!({"waiting":true}));
            }
            let aggregate = Limits::parse(&plan["bounds"], &scope.policy)?;
            let per = Limits::parse(&template["bounds"], &scope.policy)?;
            let charged = entry["charged_calls"].as_u64().ok_or("background calls")?;
            if charged
                .checked_add(1 + per.calls)
                .is_none_or(|n| n > aggregate.calls)
            {
                return Err("limit_exceeded: background aggregate allowance".into());
            }
            self.consume_call(&mut scope, &parent_key)?;
            entry["charged_calls"] = json!(charged + 1);
            scope.backgrounds.insert(plan_key.clone(), entry.clone());
            self.save_scope(&mut state, id.clone(), scope.clone())?;
            let mut observations = Vec::new();
            let mut effective = Vec::new();
            let mut claim_resources = Vec::new();
            for source_id in inputs["sources"].as_array().ok_or("background sources")? {
                let resource = &scope.resources[source_id.as_str().ok_or("source ID")?];
                self.source_current(resource, &Value::Null, false, now)?;
                let (read, files) = self.sources[&resource.source].coordination_capture(
                    &workspace::resource(&resource.reference).map_err(|e| e.to_string())?,
                    now,
                )?;
                self.attachments(&files)?;
                observations.push(json!({"id":source_id,"snapshot":read["snapshot"]}));
                effective.push(json!({"id":source_id,"version":read["observation"]["provider_revision"],"content":read["body"]["content"]}));
                claim_resources
                    .push(json!({"id":source_id,"mode":"read","snapshot":read["snapshot"]}));
            }
            let effective_now = now.saturating_add(started.elapsed().as_secs());
            self.current_lease(&scope, &parent, effective_now)?;
            if effective_now >= plan["expires_at"].as_u64().ok_or("plan expiry")?
                || started.elapsed().as_millis() > u128::from(aggregate.wall_ms)
            {
                return Err("stale: background observation exceeded its deadline".into());
            }
            let effective=contracts::digest_value(&json!({"sources":effective,"trigger":trigger,"watch":if trigger=="snapshot_changed"{Value::Null}else{json!({"reference":watch.reference,"status":watch.status})}})).map_err(|e|e.to_string())?;
            if entry["last_effective"] == effective {
                return Ok(json!({"coalesced":true}));
            }
            proposal = template.clone();
            proposal["task"] = json!(workspace_host::artifact::token());
            proposal["input"]=self.store.put_json(&json!({"v":"openagents.coordination-native-background-observation.v1","requires":[],"plan":body["plan"],"seed":template["input"],"sources":observations,"effective":effective}))?;
            entry["pending"] =
                json!({"task":proposal["task"],"proposal":proposal,"effective":effective});
            entry["charged_calls"] = json!(charged + 1 + per.calls);
            entry["occurrences"] = json!(
                entry["occurrences"]
                    .as_u64()
                    .ok_or("background occurrences")?
                    + 1
            );
            scope.backgrounds.insert(plan_key.clone(), entry);
            scope.revision += 1;
            parent_reservation = parent.body["reservation"].clone();
            worker = self.store.json(&proposal["context"])?["recipient"]
                .as_str()
                .ok_or("background worker")?
                .to_string();
            until = parent.body["expires_at"]
                .as_u64()
                .ok_or("parent expiry")?
                .min(now + per.wall_ms.div_ceil(1000));
            resources = Value::Array(claim_resources);
            self.save_scope(&mut state, id.clone(), scope)?;
        }
        let now = now.saturating_add(started.elapsed().as_secs());
        let request =
            workspace_host::host::request(&self.secret, self.authority(), proposal.clone(), now)?;
        let proposed = self.propose(&request, &BTreeMap::new(), now)?;
        let revision = self.scope(&body["coordinator"])?["revision"].clone();
        let request = workspace_host::host::request(
            &self.secret,
            self.authority(),
            json!({"v":"openagents.claim-request.v1","requires":[],"proposal":proposed["value"],"worker":worker,"resources":resources,"expected_revision":revision,"requested_until":until,"reservation":parent_reservation}),
            now,
        )?;
        let claimed = self.claim(&request, now)?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let mut scope = state.scopes[&id].clone();
        let claim_key = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == claimed["value"])
            .map(|(key, _)| key.clone())
            .ok_or("background claim")?;
        let entry = scope
            .backgrounds
            .get_mut(&plan_key)
            .ok_or("background plan")?;
        entry["last_effective"] = entry["pending"]["effective"].clone();
        entry["pending"] = Value::Null;
        entry["active"] = json!(claim_key);
        if entry["body"]["trigger"] != "snapshot_changed" {
            entry["closed"] = json!(true);
        }
        self.save_scope(&mut state, id, scope)?;
        Ok(claimed)
    }
}
