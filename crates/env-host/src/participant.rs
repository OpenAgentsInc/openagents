//! Participant and worker roles. A participant validates only what it can
//! see and signs that; the primary worker additionally decrypts the signed CJ
//! execute and checks byte identity before any dispatch.
use crate::{Result, allocator::envelope};
use nostr::{
    contracts::{self, digest_bytes},
    domain::Event,
    execution::{self, Body, Execute, Window},
};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};

/// Everything one participant checks before admitting an attachment.
pub struct Admit<'a> {
    pub grant: &'a Value,
    pub grant_ref: &'a Value,
    pub attach: &'a Value,
    pub input: &'a Value,
    pub input_ref: &'a Value,
    pub materialization: &'a Value,
    pub materialization_ref: &'a Value,
    pub projection: &'a [u8],
    pub enforcement: &'a Value,
    /// The signed CJ execute and the caller's declared plaintext. Required
    /// for the primary executor only.
    pub execute: Option<(&'a Event, &'a Value)>,
}

pub struct Participant {
    secret: SecretKey,
    pubkey: String,
}

fn same(a: &Value, b: &Value) -> bool {
    a["digest"].is_string() && a["digest"] == b["digest"]
}

impl Participant {
    pub fn new(secret: SecretKey) -> Self {
        let pubkey = secret.x_only_public_key(&Secp256k1::new()).0.to_string();
        Self { secret, pubkey }
    }
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    fn check(&self, admit: &Admit<'_>, now: u64) -> Result<()> {
        if !same(&admit.input["attachment"], admit.grant_ref)
            || admit.input["participant"] != self.pubkey.as_str()
            || !same(&admit.grant["materialization"], admit.materialization_ref)
            || !same(&admit.attach["materialization"], admit.materialization_ref)
        {
            return Err("identity_mismatch: attachment identity".into());
        }
        let role = admit.materialization["participants"]
            .as_array()
            .and_then(|p| p.iter().find(|p| p["principal"] == self.pubkey.as_str()))
            .map(|p| p["role"].clone())
            .ok_or("not_admitted: not a listed participant")?;
        if admit.input["projection"]["digest"] != digest_bytes(admit.projection).as_str() {
            return Err("identity_mismatch: projection bytes".into());
        }
        if role == "executor" {
            let (event, declared) = admit
                .execute
                .ok_or("content_unavailable: the executor must verify the signed execute")?;
            let opened = execution::open_environment_request(
                event,
                &self.pubkey,
                &self.secret,
                now,
                Window::DEFAULT,
            )
            .map_err(|e| format!("not_admitted: {e:?}"))?;
            let Body::Execute(execute) = opened.body else {
                return Err("malformed: execute".into());
            };
            if execute.payload != *declared
                || admit.attach["input_digest"] != execute.input_digest.as_str()
            {
                return Err(
                    "identity_mismatch: signed execute differs from its declaration".into(),
                );
            }
        }
        Ok(())
    }

    /// Sign an admitted or refused result. A refusal never invents evidence.
    pub fn admit(&self, allocator: &str, admit: &Admit<'_>, now: u64) -> Result<Event> {
        let body = match self.check(admit, now) {
            Ok(()) => json!({"v":"openagents.environment-participant-admission.v1","requires":[],
                "input":admit.input_ref,"materialization":admit.materialization_ref,
                "participant":self.pubkey,"status":"admitted",
                "root_input_digest":admit.attach["input_digest"],
                "projection_digest":admit.input["projection"]["digest"],
                "enforcement":admit.enforcement,"reason":null}),
            Err(error) => {
                json!({"v":"openagents.environment-participant-admission.v1","requires":[],
                "input":admit.input_ref,"materialization":admit.materialization_ref,
                "participant":self.pubkey,"status":"refused","root_input_digest":null,
                "projection_digest":null,"enforcement":null,
                "reason":crate::allocator::code(&error)})
            }
        };
        envelope(&self.secret, allocator, &body, now)
    }

    /// Report that this dispatcher stopped (or cannot be reached about) an
    /// attachment's effects.
    pub fn fence(
        &self,
        allocator: &str,
        grant_ref: &Value,
        fenced: bool,
        now: u64,
    ) -> Result<Event> {
        let body = json!({"v":"openagents.environment-fencing.v1","requires":[],"attachment":grant_ref,
            "participant":self.pubkey,"status":if fenced {"fenced"} else {"unreachable"},"observed_at":now});
        envelope(&self.secret, allocator, &body, now)
    }
}

/// Worker intake for `openagents.environment-binding.v1`. The generic decoder
/// refuses the feature; this one decodes it and then requires the trusted
/// allocator check of a currently active exact attachment before dispatch.
pub fn intake(
    event: &Event,
    secret: &SecretKey,
    attachment: &str,
    now: u64,
    check: impl FnOnce(&str, &str) -> Result<()>,
) -> Result<Execute> {
    let worker = secret.x_only_public_key(&Secp256k1::new()).0.to_string();
    let opened = execution::open_environment_request(event, &worker, secret, now, Window::DEFAULT)
        .map_err(|e| format!("not_admitted: {e:?}"))?;
    let Body::Execute(execute) = opened.body else {
        return Err("malformed: execute".into());
    };
    if contracts::digest_value(&execute.payload).map_err(|e| e.to_string())? != execute.fingerprint
    {
        return Err("identity_mismatch: fingerprint".into());
    }
    check(attachment, &execute.fingerprint)?;
    Ok(*execute)
}
