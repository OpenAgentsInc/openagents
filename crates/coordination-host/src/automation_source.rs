//! A counted source call reads only an explicitly mounted native workspace.
use crate::{
    Result,
    book::{Coordinator, Request},
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    pub fn automation_source_operation(&self, resource: &str) -> Result<Value> {
        if resource.is_empty() || resource.len() > 1024 {
            return Err("source resource ID".into());
        }
        let defs: Value =
            serde_json::from_slice(nostr::automation::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let input = json!({"$defs":defs["$defs"],"type":"object","additionalProperties":false,"required":["v","requires","coordinator","claim","source","resource","request","phase","poll","plan","executor"],"properties":{"v":{"const":"openagents.automation-native-source-input.v1"},"requires":{"type":"array","maxItems":0},"coordinator":{"$ref":"#/$defs/artifact"},"claim":{"$ref":"#/$defs/artifact"},"source":{"$ref":"#/$defs/definition"},"resource":{"const":resource},"request":{"type":"string","minLength":64,"maxLength":64},"executor":{"type":"string","minLength":64,"maxLength":64},"phase":{"enum":["observe","revalidate"]},"poll":{"$ref":"#/$defs/artifact"},"plan":{"$ref":"#/$defs/artifact"}}});
        let output = json!({"$defs":defs["$defs"],"$ref":"#/$defs/automation-observation"});
        let schema = |v: Value| {
            self.store.put(
                &contracts::jcs(&v).map_err(|e| e.to_string())?,
                "application/schema+json",
                None,
            )
        };
        let id = format!(
            "{}:coord/{}",
            self.authority(),
            contracts::digest_value(&json!({"operation":"automation-source","resource":resource}))
                .map_err(|e| e.to_string())?
                .trim_start_matches("sha256:")
        );
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Read one owner-selected workspace source version.","input":schema(input)?,"output":schema(output)?,"effects":{"reads":["artifact",resource],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"coord-automation-source","interface":"openagents.automation-native-source.v1"}});
        nostr::cap::parse_definition(&card).map_err(|e| e.to_string())?;
        let artifact = self.store.put(
            &contracts::jcs(&card).map_err(|e| e.to_string())?,
            "application/json",
            Some("openagents.cap.v1"),
        )?;
        Ok(json!({"id":id,"artifact":artifact}))
    }
    /// Verify the original controller declaration without disclosing source bytes.
    pub fn automation_source_authorization(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value)> {
        self.authenticate(
            event,
            now,
            Some("openagents.automation-native-source-input.v1"),
        )
    }
    /// Read once under the current parent fence, after durably charging its call.
    /// An interrupted read remains unknown and is never recaptured on retry.
    pub fn automation_observe(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            event,
            now,
            Some("openagents.automation-native-source-input.v1"),
        )?;
        crate::admin::closed(
            &body,
            &[
                "v",
                "requires",
                "coordinator",
                "claim",
                "source",
                "resource",
                "request",
                "phase",
                "poll",
                "plan",
                "executor",
            ],
        )?;
        coordination::id(&body["request"]).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let mut scope = state.scopes[&id].clone();
        let fingerprint = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!(
            "{actor}:auto-source:{}",
            body["request"].as_str().ok_or("source request")?
        );
        if let Some(previous) = scope.requests.get(&key) {
            if previous.fingerprint != fingerprint {
                return Err("idempotency_conflict: source request changed".into());
            }
            if previous.result["status"] == "pending" {
                return Err(
                    "unknown: interrupted source read requires reconciliation; no replay".into(),
                );
            }
            let event: Event = serde_json::from_value(previous.result["event"].clone())
                .map_err(|_| "source response")?;
            let files = previous.result["files"]
                .as_array()
                .ok_or("source files")?
                .iter()
                .map(|r| {
                    let bytes = self.store.get(r)?;
                    Ok((contracts::digest_bytes(&bytes), bytes))
                })
                .collect::<Result<_>>()?;
            return Ok((event, files));
        }
        if scope.requests.len() >= 4096 {
            return Err("source call retention bound".into());
        }
        let resource_id = body["resource"].as_str().ok_or("resource")?;
        let source = self.automation_source_operation(resource_id)?;
        if body["source"] != source
            || scope.targets.get(source["id"].as_str().ok_or("source ID")?) != Some(&source)
        {
            return Err("not_admitted: exact registered source adapter required".into());
        }
        let operation = self.operation(&source)?;
        self.schema_input(&operation["input"], &body)?;
        self.check(&scope, &actor, "control", now)?;
        let grant = self.check(&scope, &actor, "read", now)?;
        if !grant.all
            && (!grant.resources.contains(resource_id)
                || !grant
                    .targets
                    .contains(source["id"].as_str().ok_or("source ID")?))
        {
            return Err("not_admitted: source outside current read grant".into());
        }
        let executor = body["executor"].as_str().ok_or("source executor")?;
        let executor_grant = self.check(&scope, executor, "read", now)?;
        if !scope.policy.recipients.contains(executor)
            || !executor_grant.all
                && (!executor_grant.resources.contains(resource_id)
                    || !executor_grant
                        .targets
                        .contains(source["id"].as_str().ok_or("source ID")?))
        {
            return Err("not_admitted: source executor outside disclosure grant".into());
        }
        let (parent_key, parent) = scope
            .claims
            .iter()
            .find(|(_, l)| l.reference == body["claim"])
            .ok_or("current source parent claim")?;
        self.current_lease(&scope, parent, now)?;
        if parent.actor != actor {
            return Err("not_admitted: owning parent controller required".into());
        }
        let parent_proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.body["proposal"])
            .ok_or("parent proposal")?;
        if self.operation(&parent_proposal.body["target"])?["binding_contract"]["operation"]
            != "coord-container"
        {
            return Err("not_admitted: nondispatchable source parent required".into());
        }
        self.effects_subset(&operation["effects"], &parent_proposal.body["effects"])?;
        let resource = scope
            .resources
            .get(resource_id)
            .ok_or("admitted source resource")?
            .clone();
        let parent_key = parent_key.clone();
        self.consume_call(&mut scope, &parent_key)?;
        scope.revision += 1;
        let input = self.retain(&mut state, event, &body)?;
        self.journal(&mut scope,"observed",None,json!({"evidence":[input],"sources":[],"complete":false,"provenance":{"operation":"automation-source-intent","request":body["request"],"phase":body["phase"],"calls":1}}),now)?;
        scope.requests.insert(
            key.clone(),
            Request {
                fingerprint: fingerprint.clone(),
                result: json!({"status":"pending"}),
            },
        );
        self.save_scope(&mut state, id.clone(), scope.clone())?;
        let capture = (|| {
            self.source_current(&resource, &Value::Null, false, now)?;
            self.sources
                .get(&resource.source)
                .ok_or("source mount")?
                .coordination_capture(
                    &nostr::workspace::resource(&resource.reference).map_err(|e| e.to_string())?,
                    now,
                )
        })();
        let (observation, mut files) = match capture {
            Ok((read, files)) => {
                let revision = read["observation"]["provider_revision"]
                    .as_str()
                    .ok_or("native provider revision")?;
                let observed_at = read["observation"]["captured_at"]
                    .as_u64()
                    .ok_or("actual source capture time")?;
                (
                    json!({"v":"openagents.automation-observation.v1","requires":[],"source":source,"resource":resource_id,"revision":revision,"observed_at":observed_at,"available":true,"evidence":read["snapshot"],"reason":null}),
                    files,
                )
            }
            Err(_) => (
                json!({"v":"openagents.automation-observation.v1","requires":[],"source":source,"resource":resource_id,"revision":null,"observed_at":now,"available":false,"evidence":null,"reason":"unavailable"}),
                BTreeMap::new(),
            ),
        };
        self.current_lease(
            &scope,
            &scope.claims[&parent_key],
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        nostr::automation::validate(&observation).map_err(|e| e.to_string())?;
        self.attachments(&files)?;
        let response =
            workspace_host::host::request(&self.secret, &actor, observation.clone(), now)?;
        let observation_ref = self.retain(&mut state, &response, &observation)?;
        files.insert(
            observation_ref["digest"]
                .as_str()
                .ok_or("observation digest")?
                .into(),
            contracts::jcs(&observation).map_err(|e| e.to_string())?,
        );
        let refs = files
            .values()
            .map(|b| self.store.put(b, "application/octet-stream", None))
            .collect::<Result<Vec<_>>>()?;
        scope.requests.insert(
            key,
            Request {
                fingerprint,
                result: json!({"event":response,"observation":observation_ref,"files":refs}),
            },
        );
        scope.revision += 1;
        self.journal(&mut scope,"observed",None,json!({"evidence":[observation_ref],"sources":[],"complete":true,"provenance":{"operation":"automation-source-observation","request":body["request"]}}),now)?;
        self.save_scope(&mut state, id, scope)?;
        Ok((response, files))
    }
}
