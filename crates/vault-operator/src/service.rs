//! The operator custody service: grants, use requests, revocation, and
//! audited key use through the operator KMS.
use crate::{MAX_GRANT_SECONDS, Result, audit::Audit};
use oa_vault::{
    Error as VaultError,
    object::{self, Tier, Wrap},
    operator::{OperatorKms, open_operator},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use vault_sealed::Signed;
use zeroize::Zeroizing;

/// Who may grant use and who may receive it, configured by the service.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub operators: BTreeSet<String>,
    pub recipients: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    body: Value,
    uses: u64,
    revoked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    fingerprint: String,
    status: String,
    object: String,
    grant: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    config: Config,
    grants: BTreeMap<String, Grant>,
    requests: BTreeMap<String, Request>,
}

/// The result of one use request. Plaintext goes only to the recipient.
pub struct Outcome {
    pub status: &'static str,
    pub reason: Option<String>,
    pub plaintext: Option<Zeroizing<Vec<u8>>>,
}

pub struct Service {
    state_path: PathBuf,
    kms: Box<dyn OperatorKms>,
    audit: Audit,
}

fn fingerprint(signed: &Signed) -> Result<String> {
    let bytes = serde_json::to_vec(signed).map_err(|e| e.to_string())?;
    Ok(oa_vault::hex(&Sha256::digest(&bytes)))
}

fn refused(reason: &str) -> Outcome {
    Outcome {
        status: "refused",
        reason: Some(reason.into()),
        plaintext: None,
    }
}

impl Service {
    pub fn initialize(dir: PathBuf, config: Config) -> Result<()> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("state.json");
        if path.exists() {
            return Err("conflict: service state already exists".into());
        }
        std::fs::write(
            &path,
            serde_json::to_vec(&State {
                config,
                ..State::default()
            })
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn open(dir: PathBuf, kms: Box<dyn OperatorKms>) -> Result<Self> {
        let audit = Audit::open(dir.join("audit.jsonl"))?;
        let service = Self {
            state_path: dir.join("state.json"),
            kms,
            audit,
        };
        service.state()?;
        Ok(service)
    }
    pub fn audit(&mut self) -> &mut Audit {
        &mut self.audit
    }
    fn state(&self) -> Result<State> {
        let bytes = std::fs::read(&self.state_path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|_| "conflict: service state is unreadable".into())
    }
    fn save(&self, state: &State) -> Result<()> {
        let tmp = self.state_path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(state).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.state_path).map_err(|e| e.to_string())
    }

    /// Admit an operator-signed grant for exact objects, one recipient, a
    /// purpose, at most a day, and a use count.
    pub fn grant(&mut self, signed: &Signed, now: u64) -> Result<String> {
        let mut state = self.state()?;
        let body = &signed.body;
        if body["v"] != "openagents.vault-operator-grant.v1" || body["requires"] != json!([]) {
            return Err("unsupported_version: grant".into());
        }
        if !state.config.operators.contains(&signed.signer) {
            return Err("not_admitted: only a configured operator grants use".into());
        }
        signed.verify(&signed.signer)?;
        let recipient = body["recipient"].as_str().unwrap_or_default();
        if !state.config.recipients.contains(recipient) {
            return Err("not_admitted: unknown recipient".into());
        }
        let issued = body["issued_at"].as_u64().unwrap_or(u64::MAX);
        let expires = body["expires_at"].as_u64().unwrap_or(0);
        if issued > now || expires <= now || expires - issued > MAX_GRANT_SECONDS {
            return Err("limit_exceeded: a grant lives at most a day".into());
        }
        if body["max_uses"].as_u64().unwrap_or(0) == 0
            || body["objects"].as_array().is_none_or(Vec::is_empty)
        {
            return Err("malformed: a grant names objects and uses".into());
        }
        let id = body["grant"].as_str().unwrap_or_default().to_string();
        if state.grants.contains_key(&id) {
            return Err("conflict: grant ID already used".into());
        }
        self.audit.append(
            "grant",
            json!({"grant":id,"recipient":recipient,"objects":body["objects"],
            "purpose":body["purpose"],"operator":signed.signer}),
            now,
        )?;
        state.grants.insert(
            id.clone(),
            Grant {
                body: body.clone(),
                uses: 0,
                revoked: false,
            },
        );
        self.save(&state)?;
        Ok(id)
    }

    /// Revoke a grant; further use refuses.
    pub fn revoke(&mut self, signed: &Signed, now: u64) -> Result<()> {
        let mut state = self.state()?;
        if signed.body["v"] != "openagents.vault-operator-revoke.v1"
            || !state.config.operators.contains(&signed.signer)
        {
            return Err("not_admitted: revocation".into());
        }
        signed.verify(&signed.signer)?;
        let id = signed.body["grant"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let grant = state
            .grants
            .get_mut(&id)
            .ok_or("not_admitted: unknown grant")?;
        self.audit
            .append("revoked", json!({"grant":id,"operator":signed.signer}), now)?;
        grant.revoked = true;
        self.save(&state)
    }

    /// One recipient-signed use of one object under a grant.
    pub fn use_object(&mut self, signed: &Signed, bytes: &[u8], now: u64) -> Result<Outcome> {
        let mut state = self.state()?;
        let body = &signed.body;
        if body["v"] != "openagents.vault-operator-use.v1" || body["requires"] != json!([]) {
            return Ok(refused("unsupported_version"));
        }
        let request_id = body["request"].as_str().unwrap_or_default().to_string();
        let print = fingerprint(signed)?;
        let retry = match state.requests.get(&request_id) {
            Some(prior) if prior.fingerprint != print => {
                return Ok(refused("idempotency_conflict"));
            }
            Some(prior) => Some(prior.status.clone()),
            None => None,
        };
        let grant_id = body["grant"].as_str().unwrap_or_default().to_string();
        let Some(grant) = state.grants.get(&grant_id).cloned() else {
            return Ok(refused("not_admitted"));
        };
        // The signer must be the grant's one recipient.
        let recipient = grant.body["recipient"].as_str().unwrap_or_default();
        if signed.signer != recipient || signed.verify(recipient).is_err() {
            return Ok(refused("not_admitted"));
        }
        if grant.revoked {
            return Ok(refused("revoked"));
        }
        if grant.body["expires_at"].as_u64().unwrap_or(0) <= now {
            return Ok(refused("stale"));
        }
        let object = body["object"].as_str().unwrap_or_default().to_string();
        if body["purpose"] != grant.body["purpose"]
            || !grant.body["objects"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|o| *o == object)
        {
            return Ok(refused("limit_exceeded"));
        }
        let (header, _) = object::parse(bytes).map_err(|e| e.to_string())?;
        if header.core.object != object
            || header.core.tier != Tier::Operator
            || header.core.vault != grant.body["vault"]
        {
            return Ok(refused("identity_mismatch"));
        }
        if retry.is_none() && grant.uses >= grant.body["max_uses"].as_u64().unwrap_or(0) {
            return Ok(refused("conflict"));
        }
        let wrap = header
            .wraps
            .iter()
            .find(|w| matches!(w, Wrap::Operator { .. }))
            .cloned()
            .ok_or("not_admitted: the object has no operator wrap")?;
        let Wrap::Operator { kms_key, .. } = &wrap else {
            unreachable!()
        };
        let kind = match retry.as_deref() {
            Some("completed") => "redelivered",
            Some(_) => "retry",
            None => "intent",
        };
        // Audit first: a use whose record cannot be written does not happen.
        if let Err(why) = self.audit.append(
            kind,
            json!({"request":request_id,"grant":grant_id,"object":object,
            "recipient":recipient,"key_version":kms_key}),
            now,
        ) {
            return Ok(Outcome {
                status: "refused",
                reason: Some(why),
                plaintext: None,
            });
        }
        if retry.is_none() {
            state.grants.get_mut(&grant_id).ok_or("grant")?.uses += 1;
        }
        state.requests.insert(
            request_id.clone(),
            Request {
                fingerprint: print.clone(),
                status: "unknown".into(),
                object: object.clone(),
                grant: grant_id.clone(),
            },
        );
        self.save(&state)?;
        let digest = header.core_digest().map_err(|e| e.to_string())?;
        let (status, reason, plaintext) = match open_operator(self.kms.as_mut(), &digest, &wrap) {
            Ok(dek) => match object::open_object(bytes, &dek) {
                Ok(plain) => ("completed", None, Some(plain)),
                Err(_) => ("failed", Some("identity_mismatch".to_string()), None),
            },
            Err(VaultError::Decrypt(_)) | Err(VaultError::Format(_)) => {
                ("failed", Some("identity_mismatch".to_string()), None)
            }
            // A key-service error after the call may have used the key.
            Err(_) => ("unknown", None, None),
        };
        self.audit.append(
            if status == "completed" {
                "used"
            } else {
                status
            },
            json!({"request":request_id,"grant":grant_id,"object":object}),
            now,
        )?;
        state.requests.insert(
            request_id,
            Request {
                fingerprint: print,
                status: status.into(),
                object,
                grant: grant_id,
            },
        );
        self.save(&state)?;
        Ok(Outcome {
            status,
            reason,
            plaintext,
        })
    }
}
