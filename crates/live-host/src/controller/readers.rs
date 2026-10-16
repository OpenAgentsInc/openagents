//! Artifact access follows supplied plaintext and explicit host outputs.
use super::*;

pub(super) struct ReadScope {
    pub(super) principal: String,
    pub(super) known: BTreeSet<String>,
}
struct ReadGuard<'a> {
    reader: &'a RefCell<Option<ReadScope>>,
    previous: Option<ReadScope>,
}
impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        self.reader.replace(self.previous.take());
    }
}
impl Controller {
    pub(super) fn read_scope(&self, state: &State, principal: &str) -> Result<ReadScope> {
        let mut known = state.known.get(principal).cloned().unwrap_or_default();
        // Original signers already supplied these exact root bytes. Nested
        // references never inherit that knowledge.
        known.extend(
            state
                .signers
                .iter()
                .filter(|(_, s)| s.contains(principal))
                .map(|(d, _)| d.clone()),
        );
        // Restore selected host-produced outputs from older state files. Read
        // these roots through trusted session ownership, never caller links.
        for session in state.sessions.values().filter(|s| s.owner == principal) {
            for reference in session.observations.values() {
                known.insert(digest_of(reference).into());
                let body = self.store.json(reference)?;
                for field in ["content", "evidence", "format", "freshness"] {
                    if !digest_of(&body[field]).is_empty() {
                        known.insert(digest_of(&body[field]).into());
                    }
                }
                if !digest_of(&body["geometry"]["transform"]).is_empty() {
                    known.insert(digest_of(&body["geometry"]["transform"]).into());
                }
            }
            for reference in session.transcripts.values().flatten() {
                known.insert(digest_of(reference).into());
                let body = self.store.json(reference)?;
                for segment in body["segments"].as_array().into_iter().flatten() {
                    known.insert(digest_of(&segment["text"]).into());
                }
            }
        }
        Ok(ReadScope {
            principal: principal.into(),
            known,
        })
    }
    pub(super) fn with_read_scope<T>(
        &self,
        scope: ReadScope,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let previous = self.reader.replace(Some(scope));
        let _guard = ReadGuard {
            reader: &self.reader,
            previous,
        };
        f()
    }
    pub(super) fn remember(
        &self,
        state: &mut State,
        principal: &str,
        reference: &Value,
    ) -> Result<()> {
        let digest = digest_of(reference);
        if digest.is_empty() {
            return Ok(());
        }
        let known = state.known.entry(principal.into()).or_default();
        if known.len() >= 65_536 && !known.contains(digest) {
            return Err(refuse("limit_exceeded", "shared artifact retention"));
        }
        known.insert(digest.into());
        if let Some(scope) = self.reader.borrow_mut().as_mut()
            && scope.principal == principal
        {
            scope.known.insert(digest.into());
        }
        Ok(())
    }
    pub(super) fn share_output(
        &self,
        state: &mut State,
        body: &Value,
        recipient: &str,
        reference: &Value,
    ) -> Result<()> {
        self.remember(state, recipient, reference)?;
        // Only known output fields are shared. Refusals and caller-authored
        // links cannot turn a guessed digest into permission to read it.
        let fields: &[&str] = match body["v"].as_str() {
            Some("openagents.live-result.v1") => &["value", "record"],
            Some("openagents.live-observation.v1") => {
                &["content", "evidence", "format", "freshness"]
            }
            Some("openagents.live-input-outcome.v1") if body["outcome"] == "applied" => {
                &["observed"]
            }
            _ => &[],
        };
        for field in fields {
            self.remember(state, recipient, &body[*field])?;
        }
        if body["v"] == "openagents.live-observation.v1" {
            self.remember(state, recipient, &body["geometry"]["transform"])?;
        }
        if body["v"] == "openagents.live-transcript.v1" {
            for segment in body["segments"].as_array().into_iter().flatten() {
                self.remember(state, recipient, &segment["text"])?;
            }
        }
        Ok(())
    }
    pub(super) fn may_request(
        &self,
        state: &State,
        id: Option<&str>,
        signer: &str,
        body: &Value,
        now: u64,
    ) -> bool {
        let session = id.and_then(|id| state.sessions.get(id));
        let Some(session) = session else {
            return body["v"] == "openagents.live-plan.v1" && state.policy.owners.contains(signer);
        };
        if session.owner == signer && state.policy.owners.contains(signer) {
            return true;
        }
        if body["v"] == "openagents.live-plan.v1" {
            return false;
        }
        let Some(member) = session.members.get(signer).filter(|m| m.expires_at > now) else {
            return false;
        };
        let _ = member;
        match body["v"].as_str() {
            Some("openagents.live-participant.v1") => {
                body["action"] == "revoke" && body["participant"] == signer
            }
            Some("openagents.live-input-floor-request.v1" | "openagents.live-floor-request.v1") => {
                body["participant"] == signer
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_output_has_separate_recipient_envelopes_without_nested_disclosure() {
        use crate::media::{LocalBroker, SyntheticDevice, SyntheticSource};
        let temp = tempfile::tempdir().unwrap();
        let secret = SecretKey::from_byte_array([11; 32]).unwrap();
        Controller::initialize(temp.path().join("state"), secret, Policy::default()).unwrap();
        let c = Controller::open(
            temp.path().join("state"),
            secret,
            Adapters {
                broker: Box::new(LocalBroker::default()),
                source: Box::new(SyntheticSource::default()),
                device: Box::new(SyntheticDevice::default()),
            },
        )
        .unwrap();
        let key_a = SecretKey::from_byte_array([12; 32]).unwrap();
        let key_b = SecretKey::from_byte_array([13; 32]).unwrap();
        let pk = |key: &SecretKey| {
            key.x_only_public_key(&secp256k1::Secp256k1::new())
                .0
                .to_string()
        };
        let private = c
            .store
            .put(b"private", "application/octet-stream", None)
            .unwrap();
        let body = json!({"v":"private.output.v1","private_link":private});
        let mut state = c.state().unwrap();
        let reference = c.sign(&mut state, &body, &pk(&key_a), 1).unwrap();
        assert_eq!(
            c.sign(&mut state, &body, &pk(&key_b), 1).unwrap(),
            reference
        );
        for (key, other) in [(key_a, key_b), (key_b, key_a)] {
            let declaration =
                &state.declarations[&format!("{}:to:{}", digest_of(&reference), pk(&key))];
            let event: Event = serde_json::from_value(c.store.json(declaration).unwrap()).unwrap();
            assert!(nostr::private_artifact::open(&event, &key).is_ok());
            assert!(nostr::private_artifact::open(&event, &other).is_err());
            c.with_read_scope(c.read_scope(&state, &pk(&key)).unwrap(), || {
                assert!(c.bytes(&reference).is_ok());
                assert!(c.bytes(&private).unwrap_err().starts_with("not_admitted"));
                Ok(())
            })
            .unwrap();
        }
    }
}
