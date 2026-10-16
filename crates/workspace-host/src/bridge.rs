//! Existing workbench references point to exact WS artifacts, never host paths.
use crate::{Result, artifact};
use serde_json::Value;
pub fn artifact_view(authority: &str, body: &Value) -> Result<workbench::ResourceRef> {
    let reference = artifact::json_ref(body)?;
    let digest = reference["digest"].as_str().ok_or("artifact digest")?;
    let value = workbench::ResourceRef::new(
        workbench::Kind::Artifact,
        workbench::Host::Paired {
            key: authority.into(),
        },
        digest.strip_prefix("sha256:").ok_or("artifact digest")?,
    );
    value.check().map_err(|e| e.to_string())?;
    Ok(value)
}

impl crate::host::Host {
    /// Local owner administration for an explicitly mounted coordinator.
    /// A remote participant cannot choose a path or assert an alias domain.
    pub fn coordination_resource(
        &self,
        reference: &nostr::workspace::ResourceRef,
        now: u64,
    ) -> crate::Result<serde_json::Value> {
        let _guard = self.artifacts.lock()?;
        let state = self.state()?;
        self.check(
            &state,
            self.signer.pubkey(),
            Some(&reference.resource),
            false,
            now,
        )?;
        let resource = state
            .resources
            .get(&reference.resource)
            .filter(|resource| resource.reference == *reference)
            .ok_or("workspace source unavailable")?;
        resource.source.capture()?;
        Ok(
            serde_json::json!({"authority":reference.authority,"resource":reference,"binding":state.binding_ref,"domain":resource.source.conflict_domain()?}),
        )
    }
    /// Check the actual source, rather than trusting a caller's cached digest.
    pub fn coordination_snapshot(
        &self,
        reference: &nostr::workspace::ResourceRef,
        snapshot_ref: &serde_json::Value,
        write: bool,
        now: u64,
    ) -> crate::Result<String> {
        let _guard = self.artifacts.lock()?;
        let state = self.state()?;
        self.check(
            &state,
            self.signer.pubkey(),
            Some(&reference.resource),
            write,
            now,
        )?;
        let resource = state
            .resources
            .get(&reference.resource)
            .filter(|resource| resource.reference == *reference)
            .ok_or("workspace source unavailable")?;
        let snapshot = self.artifacts.json(snapshot_ref)?;
        let observation = self.artifacts.json(&snapshot["observation"])?;
        let capture = resource.source.capture()?;
        nostr::workspace::verify_document(&snapshot, &observation, Some(&capture.bytes))
            .map_err(|e| e.to_string())?;
        if snapshot["resource"] != serde_json::to_value(reference).map_err(|e| e.to_string())?
            || snapshot["binding"] != state.binding_ref
            || observation["adapter"] != resource.adapter
            || observation["provider_revision"] != capture.position
        {
            return Err("stale: workspace source changed".into());
        }
        Ok(capture.position)
    }
}

impl crate::host::Host {
    /// Observe or reconcile a previously admitted local-owner command.
    /// An absent command never starts an action, even if its lease expired.
    pub fn coordination_recover_command(
        &self,
        command: &str,
        fingerprint: &str,
        now: u64,
    ) -> crate::Result<Option<serde_json::Value>> {
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        let key = format!("{}:{command}", self.signer.pubkey());
        let Some(previous) = state.commands.get(&key) else {
            return Ok(None);
        };
        if previous.fingerprint != fingerprint {
            return Err("idempotency_conflict: source command differs".into());
        }
        if let Some(result) = &previous.result {
            return Ok(Some(result.clone()));
        }
        self.recover_locked(&mut state, &key, now).map(Some)
    }
}

impl crate::host::Host {
    /// Local-owner capture for a mounted coordinator; exports only this document.
    pub fn coordination_capture(
        &self,
        reference: &nostr::workspace::ResourceRef,
        now: u64,
    ) -> crate::Result<(
        serde_json::Value,
        std::collections::BTreeMap<String, Vec<u8>>,
    )> {
        let event = crate::host::request(
            &self.secret,
            self.signer.pubkey(),
            serde_json::json!({"v":"openagents.ws-resource-read.v1","requires":[],"resource":reference}),
            now,
        )?;
        let read = self.read_document(&event, now)?;
        let mut files = std::collections::BTreeMap::new();
        for reference in [
            &read["snapshot"],
            &read["body"]["observation"],
            &read["body"]["content"],
            &read["observation"]["adapter"]["artifact"],
        ] {
            let bytes = self.artifacts.get(reference)?;
            files.insert(nostr::contracts::digest_bytes(&bytes), bytes);
        }
        Ok((read, files))
    }
}
