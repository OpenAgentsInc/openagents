//! Exact supplied bytes and explicitly shared frame data; no cache by hash grant.
use crate::{
    Result,
    book::{Coordinator, Scope},
};
use nostr::contracts;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
impl Coordinator {
    /// Local administration by the holder of this store's actual private key.
    /// Remote proposals cannot invoke this method or enlarge a read closure.
    pub fn put_local_json(&self, value: &Value) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.state()?;
        self.store.put_json(value)
    }
    pub fn put_local_bytes(
        &self,
        bytes: &[u8],
        media: &str,
        schema: Option<&str>,
    ) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.state()?;
        self.store.put(bytes, media, schema)
    }
    /// Local owner inspection; never exposed as a remote digest lookup.
    pub fn get_local_bytes(&self, reference: &Value) -> Result<Vec<u8>> {
        let _guard = self.store.lock()?;
        self.state()?;
        self.store.get(reference)
    }
    pub(crate) fn attachments(&self, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
        if files.len() > 256 || files.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576 {
            return Err("coordinator input closure limit exceeded".into());
        }
        for (digest, bytes) in files {
            if bytes.len() > 1_048_576 || contracts::digest_bytes(bytes) != *digest {
                return Err("coordinator input bytes differ from their digest".into());
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        Ok(())
    }
    pub(crate) fn closure(
        &self,
        scope: &Scope,
        actor: &str,
        roots: &[Value],
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Result<BTreeSet<String>> {
        let mut available = BTreeSet::new();
        for frame in scope.frames.values() {
            if let Some(refs) = frame["closure"].as_array() {
                for reference in refs {
                    if let Some(digest) = reference.as_str() {
                        available.insert(digest.to_string());
                    }
                }
            }
        }
        for proposal in scope.proposals.values() {
            if proposal.author == actor
                || proposal.claim.as_ref().is_some_and(|reference| {
                    scope
                        .claims
                        .values()
                        .any(|claim| claim.reference == *reference && claim.body["worker"] == actor)
                })
            {
                available.extend(proposal.closure.iter().cloned());
            }
        }
        for definition in scope.targets.values() {
            available.insert(
                definition["artifact"]["digest"]
                    .as_str()
                    .ok_or("operation digest")?
                    .to_string(),
            );
            let operation = self.operation(definition)?;
            for field in ["input", "output"] {
                available.insert(
                    operation[field]["digest"]
                        .as_str()
                        .ok_or("schema digest")?
                        .to_string(),
                );
            }
        }
        let mut queue = roots
            .iter()
            .filter(|r| !r.is_null())
            .cloned()
            .map(|r| (r, 0usize))
            .collect::<Vec<_>>();
        let mut checked = BTreeSet::new();
        let mut bytes_total = 0usize;
        while let Some((reference, depth)) = queue.pop() {
            let parsed = contracts::parse_artifact(&reference).map_err(|e| e.to_string())?;
            if parsed.schema.as_deref().is_some_and(private) {
                continue;
            }
            if checked.contains(&parsed.digest) {
                continue;
            }
            if depth > 32 || checked.len() >= 256 {
                return Err("coordinator artifact graph limit exceeded".into());
            }
            if actor != scope.owner
                && !files.contains_key(&parsed.digest)
                && !available.contains(&parsed.digest)
            {
                return Err("not_admitted: private referenced bytes were not supplied or shared with this participant".into());
            }
            let bytes = files
                .get(&parsed.digest)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| self.store.get(&reference))?;
            contracts::check_artifact_bytes(&parsed, &bytes).map_err(|e| e.to_string())?;
            bytes_total += bytes.len();
            if bytes_total > 8 * 1_048_576 {
                return Err("coordinator artifact byte limit exceeded".into());
            }
            if matches!(
                parsed.media_type.as_str(),
                "application/json" | "application/schema+json"
            ) {
                let body = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
                if body["v"].as_str().is_some_and(private) {
                    continue;
                }
                if body["v"] == "openagents.evidence.v1" {
                    let evidence = contracts::parse_evidence(&body).map_err(|e| e.to_string())?;
                    if scope
                        .policy
                        .recipients
                        .iter()
                        .any(|key| !contracts::may_read(&evidence, key))
                    {
                        return Err(
                            "not_admitted: frame evidence excludes a scope recipient".into()
                        );
                    }
                }
                let mut nodes = vec![&body];
                let mut count = 0usize;
                while let Some(value) = nodes.pop() {
                    count += 1;
                    if count > 8192 {
                        return Err("coordinator artifact structure limit exceeded".into());
                    }
                    if contracts::parse_artifact(value).is_ok() {
                        queue.push((value.clone(), depth + 1));
                        continue;
                    }
                    match value {
                        Value::Object(fields) => nodes.extend(
                            fields
                                .iter()
                                .filter(|(key, _)| key.as_str() != "meta")
                                .map(|(_, value)| value),
                        ),
                        Value::Array(items) => nodes.extend(items),
                        _ => {}
                    }
                }
            }
            checked.insert(parsed.digest);
        }
        Ok(checked)
    }
}
fn private(schema: &str) -> bool {
    matches!(
        schema,
        "openagents.automation-native-policy.v1"
            | "openagents.workspace-binding.v1"
            | "openagents.coordinator.v1"
            | "openagents.coordination-native-policy.v1"
            | "openagents.coordination-native-grant.v1"
            | "openagents.coordination-native-resources.v1"
            | "openagents.coordination-native-operations.v1"
    )
}
