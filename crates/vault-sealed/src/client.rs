//! The person's client for tier `sealed`.
use crate::{MAX_LEASE_SECONDS, Result, Signed, WORKLOAD, id_bytes};
use nostr::att::Level;
use oa_vault::{
    Vmk, b64,
    object::{self, New, Tier, Wrap},
    sealed::{self, KeyPair, OsRandom},
    unb64, unb64_n,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// An attested `vault` endpoint the client verified. Construct it only with
/// [`verify_endpoint`] (or, for in-process fixtures, [`VerifiedEndpoint::assume_for_fixture`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedEndpoint {
    /// The endpoint's signing key (hex x-only).
    pub endpoint: String,
    /// The endpoint's HPKE key: the session key.
    pub hpke: [u8; 32],
    /// The raw id of the verified endpoint event.
    pub event: [u8; 32],
    /// The release event id the endpoint runs.
    pub release: String,
    pub level: Level,
    /// When the endpoint's evidence stops being current.
    pub valid_until: u64,
}

impl VerifiedEndpoint {
    /// An endpoint taken as verified without ATT evidence. For in-process
    /// fixtures only: production code verifies with [`verify_endpoint`].
    #[doc(hidden)]
    pub fn assume_for_fixture(
        endpoint: &str,
        hpke: [u8; 32],
        event: [u8; 32],
        release: &str,
        valid_until: u64,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            hpke,
            event,
            release: release.into(),
            level: Level::TeeCloud,
            valid_until,
        }
    }
}

/// Verify a `vault` endpoint end to end through `oa-att` (records, token
/// chain, measurement, key binding) and require level `tee-cloud` and an
/// HPKE session key.
pub fn verify_endpoint(
    records: &oa_att::Records,
    publisher: &str,
    now: u64,
) -> Result<VerifiedEndpoint> {
    let policy = oa_att::Policy {
        publisher: publisher.into(),
        workload: WORKLOAD.into(),
        required: Level::TeeCloud,
        seen_generation: None,
    };
    let parsed = oa_att::parse(records, &policy, now).map_err(|e| format!("not_admitted: {e}"))?;
    let claims = oa_att::chain(&parsed, now).map_err(|e| format!("not_admitted: {e}"))?;
    oa_att::measure(&parsed, &claims, now, oa_att::Tamper::None)
        .map_err(|e| format!("not_admitted: {e}"))?;
    let bound =
        oa_att::bind(&parsed, &claims, &policy, None).map_err(|e| format!("not_admitted: {e}"))?;
    if bound.level != Level::TeeCloud {
        return Err("not_admitted: the vault needs level tee-cloud".into());
    }
    let hpke = parsed
        .endpoint
        .body
        .hpke
        .as_deref()
        .ok_or("not_admitted: the endpoint has no session key")?;
    Ok(VerifiedEndpoint {
        endpoint: parsed.endpoint.body.endpoint.clone(),
        hpke: unb64_n::<32>(hpke).map_err(|e| e.to_string())?,
        event: id_bytes(&records.endpoint.id)?,
        release: parsed.release_id.clone(),
        level: bound.level,
        valid_until: parsed.endpoint.body.valid_until,
    })
}

/// The person's client: the vault master key and one enrolled slot key that
/// signs leases and revocations.
pub struct Client {
    vmk: Vmk,
    slot: String,
    slot_key: secp256k1::SecretKey,
    certs: BTreeMap<String, Value>,
    highest_epoch: BTreeMap<String, u64>,
}

/// What a request left with the client: its reply key and lease id.
pub struct Pending {
    pub reply: KeyPair,
    pub lease: String,
}

impl Client {
    pub fn new(vmk: Vmk, slot: &str, slot_key: secp256k1::SecretKey) -> Self {
        Self {
            vmk,
            slot: slot.into(),
            slot_key,
            certs: BTreeMap::new(),
            highest_epoch: BTreeMap::new(),
        }
    }
    pub fn slot_pubkey(&self) -> String {
        self.slot_key
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string()
    }
    pub fn user_key_id(&self) -> String {
        self.vmk.user_key_id()
    }

    /// Accept a system key only from the verified, current endpoint, for
    /// the same release, never at a lower epoch than one already seen.
    pub fn accept_system_key(
        &mut self,
        endpoint: &VerifiedEndpoint,
        signed: &Signed,
        now: u64,
    ) -> Result<()> {
        if endpoint.valid_until <= now {
            return Err("stale: the endpoint's attestation is no longer current".into());
        }
        signed.verify(&endpoint.endpoint)?;
        let cert = &signed.body;
        if cert["v"] != "openagents.vault-system-key.v1"
            || cert["release"] != endpoint.release.as_str()
        {
            return Err("identity_mismatch: the certificate is for another release".into());
        }
        let public = unb64_n::<32>(cert["public"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        if cert["system_key"] != sealed::system_key_id(&public).as_str() {
            return Err("identity_mismatch: system key id".into());
        }
        let vault = cert["vault"].as_str().unwrap_or_default().to_string();
        let epoch = cert["epoch"].as_u64().unwrap_or(0);
        if self.highest_epoch.get(&vault).is_some_and(|e| epoch < *e) {
            return Err("stale: an older system key epoch is a rollback".into());
        }
        self.highest_epoch.insert(vault.clone(), epoch);
        self.certs.insert(vault, cert.clone());
        Ok(())
    }

    /// Seal an object for tier `sealed`, wrapped to the accepted system key
    /// and, unless turned off, with a device copy.
    pub fn seal_object(
        &self,
        vault: &str,
        object: &str,
        plain: &[u8],
        device_copy: bool,
        now: u64,
    ) -> Result<Vec<u8>> {
        let cert = self
            .certs
            .get(vault)
            .ok_or("not_admitted: no verified system key for this vault")?;
        let public = unb64_n::<32>(cert["public"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        let new = New {
            object,
            vault,
            tier: Tier::Sealed,
            media: None,
            created_at: now,
        };
        let draft = object::seal_new(&new, plain, Vec::new()).map_err(|e| e.to_string())?;
        let digest = draft.header.core_digest().map_err(|e| e.to_string())?;
        let mut wraps = vec![
            sealed::wrap_sealed(&self.vmk, &digest, &draft.dek, &public)
                .map_err(|e| e.to_string())?,
        ];
        if device_copy {
            wraps.push(
                object::wrap_user(&self.vmk, &digest, &draft.dek).map_err(|e| e.to_string())?,
            );
        }
        let nonce =
            unb64_n::<7>(&draft.header.core.content.nonce_prefix).map_err(|e| e.to_string())?;
        let made = object::seal_with(
            &new,
            plain,
            wraps,
            draft.dek,
            nonce,
            draft.header.core.content.chunk_bytes,
        )
        .map_err(|e| e.to_string())?;
        Ok(made.bytes)
    }

    /// A lease for exactly these objects, purpose, route, and effects.
    #[allow(clippy::too_many_arguments)]
    pub fn lease(
        &self,
        run: &str,
        objects: &[&str],
        purpose: &str,
        route: &str,
        effects: Value,
        ttl: u64,
        max_uses: u64,
        now: u64,
    ) -> Result<Signed> {
        if ttl == 0 || ttl > MAX_LEASE_SECONDS {
            return Err("limit_exceeded: a lease lives at most a day".into());
        }
        let body = json!({"v":"openagents.vault-lease.v1","requires":[],"lease":oa_vault::new_id().map_err(|e| e.to_string())?,
            "run":run,"objects":objects,"purpose":purpose,"route":route,"effects":effects,"expires_at":now+ttl,
            "max_uses":max_uses,"issued_by":self.slot,"issued_at":now});
        Signed::sign(body, &self.slot_key)
    }

    /// Revoke a lease; signed like the lease.
    pub fn revoke(&self, lease: &str) -> Result<Signed> {
        Signed::sign(
            json!({"v":"openagents.vault-revoke.v1","requires":[],"lease":lease,"issued_by":self.slot}),
            &self.slot_key,
        )
    }

    /// Seal one session request carrying the person's share and the lease.
    pub fn request(
        &self,
        endpoint: &VerifiedEndpoint,
        vault: &str,
        lease: &Signed,
        now: u64,
    ) -> Result<([u8; 32], Vec<u8>, Pending)> {
        if endpoint.valid_until <= now {
            return Err("stale: the endpoint's attestation is no longer current".into());
        }
        if endpoint.level != Level::TeeCloud {
            return Err("not_admitted: the vault needs level tee-cloud".into());
        }
        let reply = KeyPair::generate().map_err(|e| e.to_string())?;
        let share = self.vmk.user_share();
        let body = Zeroizing::new(
            serde_json::to_vec(
                &json!({"v":"openagents.vault-request.v1","requires":[],"vault":vault,
                "user_share":b64(share.as_ref()),"lease":lease,"reply_to":b64(&reply.public()),
                "nonce":b64(&oa_vault::random::<16>().map_err(|e| e.to_string())?)}),
            )
            .map_err(|e| e.to_string())?,
        );
        let (enc, sealed) =
            sealed::seal_request(&endpoint.hpke, &endpoint.event, &body, &mut OsRandom)
                .map_err(|e| e.to_string())?;
        let lease_id = lease.body["lease"].as_str().unwrap_or_default().to_string();
        Ok((
            enc,
            sealed,
            Pending {
                reply,
                lease: lease_id,
            },
        ))
    }

    /// Open the replies sealed to this request's reply key.
    pub fn open_replies(
        &self,
        pending: &Pending,
        replies: &[([u8; 32], Vec<u8>)],
    ) -> Result<Vec<Zeroizing<Vec<u8>>>> {
        let lease = id_bytes(&pending.lease)?;
        replies
            .iter()
            .map(|(enc, sealed)| {
                sealed::open_reply(&pending.reply, &lease, enc, sealed).map_err(|e| e.to_string())
            })
            .collect()
    }

    /// Open an object on this device through its `user` wrap, if it has one.
    pub fn open_on_device(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let (header, _) = object::parse(bytes).map_err(|e| e.to_string())?;
        let digest = header.core_digest().map_err(|e| e.to_string())?;
        let wrap = header
            .wraps
            .iter()
            .find(|w| matches!(w, Wrap::User { .. }))
            .ok_or("not_admitted: this object has no device copy")?;
        let dek = object::unwrap_user(&self.vmk, &digest, wrap).map_err(|e| e.to_string())?;
        object::open_object(bytes, &dek).map_err(|e| e.to_string())
    }
}

/// Decode a reply payload list (base64 pairs) a transport carried.
pub fn decode_replies(value: &Value) -> Result<Vec<([u8; 32], Vec<u8>)>> {
    value
        .as_array()
        .ok_or("malformed: replies")?
        .iter()
        .map(|r| {
            Ok((
                unb64_n::<32>(r["enc"].as_str().unwrap_or_default()).map_err(|e| e.to_string())?,
                unb64(r["sealed"].as_str().unwrap_or_default()).map_err(|e| e.to_string())?,
            ))
        })
        .collect()
}
