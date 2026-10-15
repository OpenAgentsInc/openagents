//! A cached digest is an identity, never permission to disclose its bytes.
use crate::{
    Result,
    authority::{Authority, Scope, State},
};
use nostr::{contracts, work::WorkRef};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
impl Authority {
    pub(crate) fn admit_artifacts(
        &self,
        state: &State,
        scope: &Scope,
        _work: &WorkRef,
        actor: &str,
        command: &Value,
        attachments: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<BTreeSet<String>> {
        let owner = scope.descriptor["owner"] == actor;
        let mut allowed = BTreeSet::new();
        for other in state.scopes.values() {
            for (id, item) in &other.items {
                if self.check(other, actor, Some(id), "read", now).is_ok()
                    && (!item.commercial
                        || self
                            .market
                            .as_ref()
                            .is_some_and(|port| port.disclosure(actor, now).is_ok()))
                {
                    allowed.extend(item.artifacts.iter().cloned());
                }
            }
        }
        // An author's exact original declaration is already their supplied data.
        for (digest, events) in &state.declarations {
            if events.iter().any(|event| event.pubkey == actor) {
                allowed.insert(digest.clone());
            }
        }
        let mut queue = Vec::new();
        for field in ["snapshot", "source", "reason"] {
            if !command[field].is_null() {
                queue.push((command[field].clone(), 0usize));
            }
        }
        let mut visited = BTreeSet::new();
        let mut disclosed = BTreeSet::new();
        let mut bytes_total = 0usize;
        while let Some((reference, depth)) = queue.pop() {
            let parsed = contracts::parse_artifact(&reference).map_err(|e| e.to_string())?;
            if parsed.schema.as_deref().is_some_and(private_schema) {
                continue;
            }
            if visited.contains(&parsed.digest) {
                continue;
            }
            if depth > 32 || visited.len() >= 256 {
                return Err("work artifact reference limit exceeded".into());
            }
            if !owner
                && !attachments.contains_key(&parsed.digest)
                && !allowed.contains(&parsed.digest)
            {
                return Err(
                    "not_admitted: referenced work bytes were not supplied or readable".into(),
                );
            }
            let bytes = attachments
                .get(&parsed.digest)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| self.store.get(&reference))
                .map_err(|error| {
                    format!(
                        "content_unavailable: {} [{}] ({error})",
                        parsed.digest,
                        parsed.schema.as_deref().unwrap_or(&parsed.media_type)
                    )
                })?;
            contracts::check_artifact_bytes(&parsed, &bytes).map_err(|e| e.to_string())?;
            bytes_total = bytes_total
                .checked_add(bytes.len())
                .ok_or("work artifact byte limit")?;
            if bytes_total > 8 * 1_048_576 {
                return Err("work artifact byte limit exceeded".into());
            }
            visited.insert(parsed.digest.clone());
            if matches!(
                parsed.media_type.as_str(),
                "application/json" | "application/schema+json"
            ) {
                let value = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
                if value["v"].as_str().is_some_and(private_schema) {
                    continue;
                }
                disclosed.insert(parsed.digest.clone());
                let mut nodes = vec![&value];
                let mut count = 0;
                while let Some(value) = nodes.pop() {
                    count += 1;
                    if count > 8192 {
                        return Err("work artifact structure limit exceeded".into());
                    }
                    if contracts::parse_artifact(value).is_ok() {
                        queue.push((value.clone(), depth + 1));
                        continue;
                    }
                    match value {
                        Value::Object(map) => {
                            if value["v"] == "openagents.work-delegation.v1" {
                                nodes.extend(
                                    map.iter()
                                        .filter(|(key, _)| key.as_str() != "grant")
                                        .map(|(_, value)| value),
                                );
                            } else {
                                nodes.extend(map.values());
                            }
                        }
                        Value::Array(items) => nodes.extend(items),
                        _ => {}
                    }
                }
            } else {
                disclosed.insert(parsed.digest);
            }
        }
        Ok(disclosed)
    }
}

impl Authority {
    pub(crate) fn disclosure(
        &self,
        state: &State,
        actor: &str,
        body: &Value,
        now: u64,
    ) -> Result<()> {
        let mut queue = vec![body];
        let mut count = 0;
        while let Some(value) = queue.pop() {
            count += 1;
            if count > 8192 {
                return Err("work disclosure structure limit exceeded".into());
            }
            if value.as_object().is_some_and(|o| {
                o.len() == 3
                    && o.contains_key("authority")
                    && o.contains_key("scope")
                    && o.contains_key("item")
            }) {
                let work = nostr::work::work_ref(value).map_err(|e| e.to_string())?;
                let scope = state
                    .scopes
                    .get(&work.scope)
                    .filter(|s| s.descriptor["authority"] == work.authority)
                    .ok_or("not_admitted: linked work unavailable")?;
                self.check(scope, actor, Some(&work.item), "read", now)?;
            }
            match value {
                Value::Object(map) => queue.extend(map.values()),
                Value::Array(items) => queue.extend(items),
                _ => {}
            }
        }
        Ok(())
    }
    pub fn fetch_artifact(&self, event: &nostr::domain::Event, now: u64) -> Result<Vec<u8>> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-artifact-read.v1"))?;
        crate::authority::closed(
            &body,
            &["v", "requires", "issuer", "work", "artifact", "max_bytes"],
        )?;
        let _lock = self.store.lock()?;
        let state = self.state()?;
        let work = nostr::work::work_ref(&body["work"]).map_err(|e| e.to_string())?;
        let scope = state
            .scopes
            .get(&work.scope)
            .filter(|s| s.descriptor["authority"] == work.authority)
            .ok_or("not_admitted: work unavailable")?;
        self.check(scope, &actor, Some(&work.item), "read", now)?;
        let item = scope
            .items
            .get(&work.item)
            .ok_or("not_admitted: work unavailable")?;
        if item.commercial {
            self.market
                .as_ref()
                .ok_or("commercial history bytes require the owning LAB book")?
                .disclosure(&actor, now)?;
        }
        let reference = contracts::parse_artifact(&body["artifact"]).map_err(|e| e.to_string())?;
        let limit = body["max_bytes"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 1_048_576)
            .ok_or("work fetch byte limit")?;
        if !item.artifacts.contains(&reference.digest) || reference.size > limit {
            return Err("not_admitted: artifact is outside this work history".into());
        }
        let bytes = self.store.get(&body["artifact"])?;
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
}

fn private_schema(schema: &str) -> bool {
    matches!(
        schema,
        "openagents.work-scope.v1"
            | "openagents.work-native-policy.v1"
            | "openagents.work-native-grant.v1"
            | "openagents.work-execution-grant.v1"
            | "openagents.work-native-control.v1"
    )
}
