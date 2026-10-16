//! Attestation-bound HPKE key admission and rotation.
use crate::{KeyPair, Result, hex, unb64};
use nostr::att::Level;
use std::collections::BTreeMap;

/// An endpoint HPKE key a client may seal to: taken from an endpoint record
/// whose evidence the caller verified (with `oa-att` or `att-platforms`)
/// and whose binding covers this exact key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted {
    /// `30203:<endpoint>:<instance>`.
    pub address: String,
    pub release: String,
    /// The endpoint's Nostr key, which signs the output chunks.
    pub endpoint: String,
    pub hpke: [u8; 32],
    pub binding: [u8; 32],
    pub level: Level,
    pub issued_at: u64,
    pub valid_until: u64,
}

impl Admitted {
    /// Admit the HPKE key of an endpoint already verified at `level`.
    /// `parsed` is what `oa_att::parse` returned for the same records the
    /// verifier accepted; `parse` has already recomputed the binding with
    /// this HPKE key.
    ///
    /// # Errors
    ///
    /// When the endpoint publishes no HPKE key, or the binding does not
    /// cover it.
    pub fn new(parsed: &oa_att::Parsed, level: Level) -> Result<Self> {
        let body = &parsed.endpoint.body;
        let encoded = body
            .hpke
            .as_deref()
            .ok_or("the endpoint publishes no HPKE key")?;
        let raw = unb64(encoded)?;
        let hpke: [u8; 32] = raw
            .as_slice()
            .try_into()
            .map_err(|_| "the HPKE key is not 32 bytes")?;
        let computed = nostr::att::binding(&body.endpoint, Some(&hpke), &parsed.release_id)?;
        if computed != body.binding {
            return Err("the binding does not cover this HPKE key".into());
        }
        Ok(Self {
            address: parsed.endpoint.address(),
            release: parsed.release_id.clone(),
            endpoint: body.endpoint.clone(),
            hpke,
            binding: crate::unhex32(&computed)?,
            level,
            issued_at: body.issued_at,
            valid_until: body.valid_until,
        })
    }

    /// A short key identifier: the hex of the public key.
    #[must_use]
    pub fn key_id(&self) -> String {
        hex(&self.hpke)
    }
}

/// The client's view of each endpoint's current HPKE key.
#[derive(Default, Debug)]
pub struct KeyBook {
    keys: BTreeMap<String, Admitted>,
}

impl KeyBook {
    /// Record a newly verified key. A newer record replaces the old key
    /// (rotation); an older one, or a different key at the same issue time,
    /// is refused.
    ///
    /// # Errors
    ///
    /// When the record has expired, or would roll the key back.
    pub fn admit(&mut self, key: Admitted, now: u64) -> Result<()> {
        if key.valid_until <= now {
            return Err("the endpoint record has expired".into());
        }
        if let Some(current) = self.keys.get(&key.address) {
            if key.issued_at < current.issued_at {
                return Err("an older endpoint record cannot replace a newer key".into());
            }
            if key.issued_at == current.issued_at && key.hpke != current.hpke {
                return Err("two keys claim the same issue time".into());
            }
        }
        self.keys.insert(key.address.clone(), key);
        Ok(())
    }

    /// The key to seal to for `address`, if it is still current.
    ///
    /// # Errors
    ///
    /// When there is no admitted key or it has expired.
    pub fn current(&self, address: &str, now: u64) -> Result<&Admitted> {
        let key = self
            .keys
            .get(address)
            .ok_or("no admitted key for this endpoint")?;
        if key.valid_until <= now {
            return Err("the admitted key has expired; verify a fresh endpoint record".into());
        }
        Ok(key)
    }
}

/// The worker's HPKE keys: the current key, and the previous key until the
/// record that published it expires, so requests sealed just before a
/// rotation still open.
#[derive(Debug)]
pub struct WorkerKeys {
    current: KeyPair,
    previous: Option<(KeyPair, u64)>,
}

impl WorkerKeys {
    #[must_use]
    pub fn new(current: KeyPair) -> Self {
        Self {
            current,
            previous: None,
        }
    }

    /// Rotate to `next`. The old key keeps opening requests until
    /// `old_valid_until`, the expiry of the record that published it.
    pub fn rotate(&mut self, next: KeyPair, old_valid_until: u64) {
        let old = std::mem::replace(&mut self.current, next);
        self.previous = Some((old, old_valid_until));
    }

    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        self.current.public()
    }

    /// The key pair for `public`, if it is current or still overlapping.
    ///
    /// # Errors
    ///
    /// When the request names a key this worker does not hold or retired.
    pub fn find(&self, public: &[u8; 32], now: u64) -> Result<&KeyPair> {
        if self.current.public() == *public {
            return Ok(&self.current);
        }
        match &self.previous {
            Some((key, until)) if key.public() == *public && now < *until => Ok(key),
            Some((key, _)) if key.public() == *public => {
                Err("the request is sealed to a retired key".into())
            }
            _ => Err("the request is sealed to a key this endpoint does not hold".into()),
        }
    }
}
