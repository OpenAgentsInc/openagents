//! The attested workload's side of tier `sealed`. It holds the endpoint's
//! signing and session keys, keeps each person's system key only as KMS
//! ciphertext, opens objects only while a request carries the person's
//! share under a current, exact, unused, unrevoked lease, and records what
//! it did without content or key bytes.
use crate::{MAX_LEASE_SECONDS, Result, Signed, id_bytes, kms::Kms};
use oa_vault::{
    b64,
    object::{self, Wrap},
    sealed::{self, KeyPair, OsRandom},
    unb64_n,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

#[derive(Default, Clone)]
struct LeaseState {
    uses: u64,
    revoked: bool,
}

/// What the workload may do with plaintext for a route.
pub trait Work {
    /// Produce the derived answer for the `private` route inside the TEE.
    fn answer(&mut self, purpose: &str, plaintexts: &[Zeroizing<Vec<u8>>]) -> Vec<u8>;
}

/// A deterministic in-process worker for fixtures: it reports sizes only.
pub struct SizeWork;
impl Work for SizeWork {
    fn answer(&mut self, purpose: &str, plaintexts: &[Zeroizing<Vec<u8>>]) -> Vec<u8> {
        serde_json::to_vec(&json!({"purpose":purpose,"bytes":plaintexts.iter().map(|p| p.len()).collect::<Vec<_>>()}))
            .unwrap_or_default()
    }
}

pub struct Workload {
    endpoint: secp256k1::SecretKey,
    session: KeyPair,
    event: [u8; 32],
    release: String,
    kms: Box<dyn Kms>,
    work: Box<dyn Work>,
    /// Whether the release declares `fast` egress.
    egress: bool,
    systems: BTreeMap<String, (u64, [u8; 32], Vec<u8>)>,
    leases: BTreeMap<String, LeaseState>,
    log: Vec<Value>,
}

/// One reply sealed to the client's reply key.
pub type Reply = ([u8; 32], Vec<u8>);

impl Workload {
    pub fn new(
        endpoint: secp256k1::SecretKey,
        session: KeyPair,
        event: [u8; 32],
        release: &str,
        kms: Box<dyn Kms>,
        work: Box<dyn Work>,
        egress: bool,
    ) -> Self {
        Self {
            endpoint,
            session,
            event,
            release: release.into(),
            kms,
            work,
            egress,
            systems: BTreeMap::new(),
            leases: BTreeMap::new(),
            log: Vec::new(),
        }
    }
    pub fn endpoint_pubkey(&self) -> String {
        self.endpoint
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string()
    }
    pub fn session_public(&self) -> [u8; 32] {
        self.session.public()
    }

    /// The person's system key certificate for the current epoch, signed by
    /// the endpoint key. The private half exists only as KMS ciphertext.
    pub fn system_key(&mut self, vault: &str) -> Result<Signed> {
        if !self.systems.contains_key(vault) {
            self.rotate(vault)?;
        }
        let (epoch, public, _) = self.systems.get(vault).cloned().ok_or("system key")?;
        Signed::sign(
            json!({"v":"openagents.vault-system-key.v1","requires":[],"vault":vault,
                "system_key":sealed::system_key_id(&public),"public":b64(&public),"epoch":epoch,"release":self.release}),
            &self.endpoint,
        )
    }

    /// A new epoch: a new system key; the old one is gone.
    pub fn rotate(&mut self, vault: &str) -> Result<u64> {
        let pair = KeyPair::generate().map_err(|e| e.to_string())?;
        let sealed = self.kms.seal(&self.release, pair.private())?;
        let epoch = self.systems.get(vault).map_or(1, |s| s.0 + 1);
        self.systems
            .insert(vault.into(), (epoch, pair.public(), sealed));
        Ok(epoch)
    }

    /// The disclosure log: lease, run, object IDs, route, and result only.
    pub fn log(&self) -> &[Value] {
        &self.log
    }

    fn record(&mut self, lease: &Value, result: &str) {
        self.log.push(
            json!({"lease":lease["lease"],"run":lease["run"],"objects":lease["objects"],
            "route":lease["route"],"result":result}),
        );
    }

    /// Serve one sealed session request for `objects`. `enrolled` maps the
    /// person's slot ids to the keys allowed to sign leases.
    pub fn serve(
        &mut self,
        enc: &[u8; 32],
        request: &[u8],
        objects: &[(String, Vec<u8>)],
        enrolled: &BTreeMap<String, String>,
        now: u64,
    ) -> Result<Vec<Reply>> {
        let plain = sealed::open_request(&self.session, &self.event, enc, request)
            .map_err(|_| "identity_mismatch: the request is not for this endpoint")?;
        let body: Value = serde_json::from_slice(&plain).map_err(|_| "malformed: request")?;
        if body["v"] != "openagents.vault-request.v1" || body["requires"] != json!([]) {
            return Err("unsupported_version: request".into());
        }
        let signed: Signed =
            serde_json::from_value(body["lease"].clone()).map_err(|_| "malformed: lease")?;
        let lease = signed.body.clone();
        let outcome = self.admit(&signed, &body, objects, enrolled, now);
        if let Err(reason) = &outcome {
            self.record(
                &lease,
                &format!("refused:{}", reason.split(':').next().unwrap_or("refused")),
            );
            return Err(reason.clone());
        }
        let share = Zeroizing::new(
            unb64_n::<32>(body["user_share"].as_str().unwrap_or_default())
                .map_err(|e| e.to_string())?,
        );
        let vault = body["vault"].as_str().unwrap_or_default().to_string();
        let (_, public, kms_sealed) = self
            .systems
            .get(&vault)
            .cloned()
            .ok_or("not_admitted: no system key for this vault")?;
        let private = self.kms.unseal(&self.release, &kms_sealed)?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&private);
        let system = KeyPair::from_private(key).map_err(|e| e.to_string())?;
        key.fill(0);
        if system.public() != public {
            return Err("identity_mismatch: system key".into());
        }
        let mut plaintexts = Vec::new();
        for (_, bytes) in objects {
            let (header, _) = object::parse(bytes).map_err(|e| e.to_string())?;
            let digest = header.core_digest().map_err(|e| e.to_string())?;
            let wrap = header
                .wraps
                .iter()
                .find(|w| matches!(w, Wrap::Sealed { .. }))
                .ok_or("not_admitted: the object has no sealed wrap")?;
            let opened = sealed::open_sealed(wrap, &digest, &system, &share)
                .and_then(|dek| object::open_object(bytes, &dek));
            match opened {
                Ok(plain) => plaintexts.push(plain),
                Err(_) => {
                    self.record(&lease, "refused:decrypt");
                    return Err("identity_mismatch: wrong share or system key".into());
                }
            }
        }
        let reply_to = unb64_n::<32>(body["reply_to"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let lease_id = id_bytes(lease["lease"].as_str().unwrap_or_default())?;
        let route = lease["route"].as_str().unwrap_or_default();
        let outputs: Vec<Vec<u8>> = match route {
            "private" => vec![
                self.work
                    .answer(lease["purpose"].as_str().unwrap_or_default(), &plaintexts),
            ],
            "device" => plaintexts.iter().map(|p| p.to_vec()).collect(),
            _ => return Err("not_admitted: route".into()),
        };
        let mut replies = Vec::new();
        for output in outputs {
            replies.push(
                sealed::seal_reply(&reply_to, &lease_id, &output, &mut OsRandom)
                    .map_err(|e| e.to_string())?,
            );
        }
        let state = self
            .leases
            .entry(lease["lease"].as_str().unwrap_or_default().into())
            .or_default();
        state.uses += 1;
        self.record(&lease, "granted");
        // The share, the system key, and every data key are dropped here.
        drop(system);
        drop(plaintexts);
        Ok(replies)
    }

    fn admit(
        &mut self,
        signed: &Signed,
        body: &Value,
        objects: &[(String, Vec<u8>)],
        enrolled: &BTreeMap<String, String>,
        now: u64,
    ) -> Result<()> {
        let lease = &signed.body;
        if lease["v"] != "openagents.vault-lease.v1" || lease["requires"] != json!([]) {
            return Err("unsupported_version: lease".into());
        }
        let slot = lease["issued_by"].as_str().unwrap_or_default();
        let key = enrolled.get(slot).ok_or("not_admitted: unknown slot key")?;
        signed.verify(key)?;
        let id = lease["lease"].as_str().unwrap_or_default().to_string();
        let state = self.leases.get(&id).cloned().unwrap_or_default();
        if state.revoked {
            return Err("revoked: the lease was revoked".into());
        }
        let issued = lease["issued_at"].as_u64().unwrap_or(u64::MAX);
        let expires = lease["expires_at"].as_u64().unwrap_or(0);
        if expires <= now || issued > now || expires.saturating_sub(issued) > MAX_LEASE_SECONDS {
            return Err("stale: the lease is expired or too long".into());
        }
        if state.uses >= lease["max_uses"].as_u64().unwrap_or(0) {
            return Err("conflict: the lease was already used".into());
        }
        let allowed: BTreeSet<&str> = lease["objects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if objects.is_empty() || objects.iter().any(|(id, _)| !allowed.contains(id.as_str())) {
            return Err("limit_exceeded: the request is wider than the lease".into());
        }
        for (id, bytes) in objects {
            let (header, _) = object::parse(bytes).map_err(|e| e.to_string())?;
            if header.core.object != *id || header.core.vault != body["vault"] {
                return Err("identity_mismatch: object identity".into());
            }
        }
        match lease["route"].as_str() {
            Some("private" | "device") => {}
            Some("fast") if self.egress => {
                return Err("unsupported_feature: fast egress is not built here".into());
            }
            Some("fast") => return Err("not_admitted: the release declares no fast egress".into()),
            _ => return Err("malformed: route".into()),
        }
        if lease["effects"]["read"] != true || lease["effects"]["write_derived"] == true {
            return Err("unsupported_feature: effects".into());
        }
        if lease["route"] == "device" && lease["effects"]["export"] != true {
            return Err("not_admitted: returning plaintext needs export".into());
        }
        Ok(())
    }

    /// A revocation signed like the lease. Further use refuses.
    pub fn revoke(&mut self, signed: &Signed, enrolled: &BTreeMap<String, String>) -> Result<()> {
        let body = &signed.body;
        if body["v"] != "openagents.vault-revoke.v1" {
            return Err("unsupported_version: revoke".into());
        }
        let key = enrolled
            .get(body["issued_by"].as_str().unwrap_or_default())
            .ok_or("not_admitted: unknown slot key")?;
        signed.verify(key)?;
        let id = body["lease"].as_str().unwrap_or_default().to_string();
        self.leases.entry(id.clone()).or_default().revoked = true;
        self.log.push(json!({"lease":id,"result":"released"}));
        Ok(())
    }
}
