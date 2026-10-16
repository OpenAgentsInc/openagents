//! Signed private host reads and administration for a resident controller.
use super::*;
use crate::resident::Reply;
impl Controller {
    pub(super) fn reply(
        &self,
        recipient: &str,
        body: Value,
        artifacts: BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Reply> {
        Ok(Reply {
            event: workspace_host::host::request(&self.secret, recipient, body, now)?,
            artifacts,
        })
    }
    /// Preserve the original signer through the untrusted transport.
    pub fn serve(&self, bundle: &Bundle, now: u64) -> Result<Reply> {
        let result = self.submit(bundle, now)?;
        self.reply(&bundle.event.pubkey, result, BTreeMap::new(), now)
    }
    /// Private reads, one-shot credential delivery, and host-key administration.
    pub fn inspect(&self, event: &Event, now: u64) -> Result<Reply> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.expire_media_in(&mut state, now)?;
        let (signer, body, input) = self.authenticate(&mut state, event, now)?;
        let method = body["v"].as_str().unwrap_or_default();
        let fields: &[&str] = match method {
            "openagents.livehost-read.v1" => &["v", "requires", "artifacts"],
            "openagents.livehost-media.v1" => &["v", "requires"],
            "openagents.livehost-frame-status.v1" => &["v", "requires", "session", "frame"],
            "openagents.livehost-operation.v1" => &["v", "requires"],
            "openagents.livehost-credential.v1" => &["v", "requires", "session"],
            "openagents.livehost-input-policy.v1" => &["v", "requires", "request", "grants"],
            _ => return Err("unsupported_feature: host operation".into()),
        };
        if body
            .as_object()
            .is_none_or(|m| m.keys().any(|k| !fields.contains(&k.as_str())))
            || body
                .get("requires")
                .is_some_and(|r| r.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err("malformed: host operation fields".into());
        }
        let owner = state.policy.owners.contains(&signer);
        let mut artifacts = BTreeMap::new();
        let mut result = match method {
            "openagents.livehost-read.v1"
                if owner
                    || state
                        .sessions
                        .values()
                        .any(|s| s.members.get(&signer).is_some_and(|m| m.expires_at > now)) =>
            {
                let refs = body["artifacts"]
                    .as_array()
                    .filter(|a| a.len() <= 128)
                    .ok_or("limit_exceeded: artifact reads")?;
                self.with_read_scope(self.read_scope(&state, &signer)?, || {
                    let mut size = 0;
                    for reference in refs {
                        let bytes = self.bytes(reference)?;
                        size += bytes.len();
                        if size > 4 * 1_048_576 {
                            return Err("limit_exceeded: artifact reads".into());
                        }
                        artifacts.insert(digest_bytes(&bytes), bytes);
                    }
                    Ok(())
                })?;
                json!({"v":"openagents.livehost-read-result.v1","artifacts":refs})
            }
            "openagents.livehost-frame-status.v1" => {
                let id = body["session"].as_str().ok_or("malformed: frame session")?;
                let session = state
                    .sessions
                    .get(id)
                    .ok_or("not_admitted: frame session")?;
                let digest = digest_of(&body["frame"]);
                let prefix = format!("frame:{id}:");
                let owner_reader = session.owner == signer && owner;
                let visible: Vec<_> = state
                    .priors
                    .iter()
                    .filter(|(key, p)| {
                        key.starts_with(&prefix)
                            && (owner_reader || p.result["participant"] == signer)
                    })
                    .map(|(_, p)| p)
                    .collect();
                if !owner_reader && visible.is_empty() {
                    return Err("not_admitted: frame receipt reader".into());
                }
                let prior = visible
                    .into_iter()
                    .find(|p| p.fingerprint == digest)
                    .ok_or("content_unavailable: frame receipt")?;
                json!({"v":"openagents.livehost-frame-status-result.v1","frame":body["frame"],"outcome":prior.result})
            }
            "openagents.livehost-media.v1" if owner => {
                let installed = self.media_operation()?;
                let definition = contracts::parse_strict_bounded(
                    &self.store.get(&installed["operation"]["artifact"])?,
                    1_048_576,
                )
                .map_err(contract)?;
                let refs = vec![
                    installed["operation"]["artifact"].clone(),
                    installed["binding"].clone(),
                    installed["destination"].clone(),
                    definition["input"].clone(),
                    definition["output"].clone(),
                ];
                for reference in &refs {
                    self.remember(&mut state, &signer, reference)?;
                    artifacts.insert(digest_of(reference).into(), self.store.get(reference)?);
                }
                json!({"v":"openagents.livehost-media-result.v1","media":installed,"artifacts":refs})
            }
            "openagents.livehost-operation.v1" if owner => {
                let operation = self.device_operation()?;
                let definition = contracts::parse_strict_bounded(
                    &self.store.get(&operation["artifact"])?,
                    1_048_576,
                )
                .map_err(contract)?;
                let mut refs = vec![operation["artifact"].clone()];
                for field in ["input", "output"] {
                    refs.push(definition[field].clone());
                }
                for field in [
                    "binding",
                    "implementation",
                    "observation_schema",
                    "preconditions_schema",
                ] {
                    refs.push(definition["meta"][field].clone());
                }
                for reference in &refs {
                    self.remember(&mut state, &signer, &reference)?;
                    artifacts.insert(digest_of(&reference).into(), self.store.get(&reference)?);
                }
                json!({"v":"openagents.livehost-operation-result.v1","operation":operation,"artifacts":refs})
            }
            "openagents.livehost-input-policy.v1" if signer == self.pubkey() => {
                let request = body["request"]
                    .as_str()
                    .filter(|r| {
                        r.len() == 64
                            && r.bytes()
                                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                    })
                    .ok_or("malformed: policy request")?;
                let key = format!("admin:input-policy:{request}");
                if let Some(prior) = state.priors.get(&key) {
                    if prior.fingerprint != digest_of(&input) {
                        return Err("idempotency_conflict: input policy request".into());
                    }
                    prior.result.clone()
                } else {
                    let grants = serde_json::from_value(body["grants"].clone())
                        .map_err(|_| "malformed: input grants")?;
                    let rules = self.replace_input_grants(&mut state, grants, now)?;
                    let result =
                        json!({"v":"openagents.livehost-input-policy-result.v1","rules":rules});
                    state.priors.insert(
                        key,
                        Prior {
                            fingerprint: digest_of(&input).into(),
                            result: result.clone(),
                        },
                    );
                    result
                }
            }
            "openagents.livehost-credential.v1" => {
                let id = body["session"]
                    .as_str()
                    .ok_or("malformed: credential session")?;
                let session = state
                    .sessions
                    .get(id)
                    .ok_or("not_admitted: credential session")?;
                if !state.policy.owners.contains(&session.owner)
                    || session.body["expires_at"].as_u64().is_none_or(|v| v <= now)
                    || session
                        .members
                        .get(&signer)
                        .is_none_or(|m| m.expires_at <= now)
                    || session.state == "closed"
                    || session.state == "unknown"
                {
                    return Err("not_admitted: credential recipient".into());
                }
                let token = self
                    .take_credential(id, &signer)
                    .ok_or("unavailable: credential was already delivered")?;
                // Tokens are encrypted directly. They never enter artifacts,
                // the state file, retry receipts, or logs.
                let result = json!({"v":"openagents.livehost-credential-result.v1","session":id,"epoch":session.epoch,"token":token,"request":input});
                self.save(&state)?;
                return self.reply(&signer, result, artifacts, now);
            }
            _ => return Err("not_admitted: host operation".into()),
        };
        result["request"] = input;
        self.save(&state)?;
        self.reply(&signer, result, artifacts, now)
    }
}
