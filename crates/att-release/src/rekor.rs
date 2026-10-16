//! Rekor transparency evidence: RFC 9162 Merkle inclusion and consistency
//! proofs, the log's signed checkpoint (a signed note), and the signed
//! entry timestamp, all under a pinned log key. A [`Witness`] remembers the
//! largest checkpoint seen per log and refuses forks and stale views.
use crate::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// The public Sigstore Rekor log key (rekor.sigstore.dev), as served by
/// `/api/v1/log/publicKey`.
pub const SIGSTORE_REKOR_KEY: &str = include_str!("../vectors/rekor.sigstore.dev.pub");
pub const SIGSTORE_REKOR_NAME: &str = "rekor.sigstore.dev";

pub type Hash = [u8; 32];

fn h(parts: &[&[u8]]) -> Hash {
    let mut d = Sha256::new();
    for p in parts {
        d.update(p);
    }
    d.finalize().into()
}

/// RFC 9162 leaf hash.
#[must_use]
pub fn leaf_hash(leaf: &[u8]) -> Hash {
    h(&[&[0], leaf])
}

/// RFC 9162 interior node hash.
#[must_use]
pub fn node_hash(left: &Hash, right: &Hash) -> Hash {
    h(&[&[1], left, right])
}

/// RFC 9162 §2.1.3.2: verify an inclusion proof.
///
/// # Errors
///
/// When the proof does not lead from the leaf to the root.
pub fn verify_inclusion(
    index: u64,
    size: u64,
    leaf: &Hash,
    proof: &[Hash],
    root: &Hash,
) -> Result<()> {
    if index >= size {
        return Err("the entry index is outside the tree".into());
    }
    let (mut f, mut s) = (index, size - 1);
    let mut r = *leaf;
    for p in proof {
        if s == 0 {
            return Err("the inclusion proof is too long".into());
        }
        if f & 1 == 1 || f == s {
            r = node_hash(p, &r);
            if f & 1 == 0 {
                while f & 1 == 0 && f != 0 {
                    f >>= 1;
                    s >>= 1;
                }
            }
        } else {
            r = node_hash(&r, p);
        }
        f >>= 1;
        s >>= 1;
    }
    if s != 0 || r != *root {
        return Err("the inclusion proof does not lead to the checkpoint root".into());
    }
    Ok(())
}

/// RFC 9162 §2.1.4.2: verify that the tree of `size2` extends the tree of
/// `size1`.
///
/// # Errors
///
/// When the proof does not show the larger tree contains the smaller one.
pub fn verify_consistency(
    size1: u64,
    size2: u64,
    root1: &Hash,
    root2: &Hash,
    proof: &[Hash],
) -> Result<()> {
    let fail = || Err("the log is not consistent with the checkpoint seen before".into());
    if size1 > size2 || size1 == 0 {
        return fail();
    }
    if size1 == size2 {
        return if proof.is_empty() && root1 == root2 {
            Ok(())
        } else {
            fail()
        };
    }
    let mut path = Vec::with_capacity(proof.len() + 1);
    if size1.is_power_of_two() {
        path.push(*root1);
    }
    path.extend_from_slice(proof);
    let Some((first, rest)) = path.split_first() else {
        return fail();
    };
    let (mut f, mut s) = (size1 - 1, size2 - 1);
    while f & 1 == 1 {
        f >>= 1;
        s >>= 1;
    }
    let (mut fr, mut sr) = (*first, *first);
    for c in rest {
        if s == 0 {
            return fail();
        }
        if f & 1 == 1 || f == s {
            fr = node_hash(c, &fr);
            sr = node_hash(c, &sr);
            while f & 1 == 0 && f != 0 {
                f >>= 1;
                s >>= 1;
            }
        } else {
            sr = node_hash(&sr, c);
        }
        f >>= 1;
        s >>= 1;
    }
    if fr == *root1 && sr == *root2 && s == 0 {
        Ok(())
    } else {
        fail()
    }
}

/// A pinned transparency log key.
#[derive(Clone, Debug)]
pub struct LogKey {
    pub name: String,
    key: VerifyingKey,
    /// SHA-256 of the key's DER, Rekor's log ID.
    pub log_id: Hash,
}

impl LogKey {
    /// From a PEM `PUBLIC KEY` (P-256) and the note signer name.
    ///
    /// # Errors
    ///
    /// When the key is not a P-256 public key.
    pub fn from_pem(name: &str, pem: &str) -> Result<Self> {
        let key = VerifyingKey::from_public_key_pem(pem)
            .map_err(|_| "the log key is not a P-256 public key")?;
        let der = key
            .to_public_key_der()
            .map_err(|_| "the log key does not encode")?;
        Ok(Self {
            name: name.into(),
            key,
            log_id: h(&[der.as_bytes()]),
        })
    }

    /// The public Sigstore Rekor key.
    ///
    /// # Errors
    ///
    /// Never, for the shipped key.
    pub fn sigstore() -> Result<Self> {
        Self::from_pem(SIGSTORE_REKOR_NAME, SIGSTORE_REKOR_KEY)
    }

    fn verify(&self, message: &[u8], der: &[u8]) -> Result<()> {
        let sig = Signature::from_der(der).map_err(|_| "a log signature is malformed")?;
        self.key
            .verify(message, &sig)
            .map_err(|_| "a log signature does not verify".into())
    }
}

/// A verified checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub origin: String,
    pub size: u64,
    pub root: String,
}

impl Checkpoint {
    fn root(&self) -> Result<Hash> {
        let raw = STANDARD
            .decode(&self.root)
            .map_err(|_| "a checkpoint root is not base64")?;
        raw.as_slice()
            .try_into()
            .map_err(|_| "a checkpoint root is not 32 bytes".into())
    }
}

/// Verify a signed note checkpoint under `key`.
///
/// # Errors
///
/// When it is malformed, from another log, or not signed by the key.
pub fn checkpoint(note: &str, key: &LogKey) -> Result<Checkpoint> {
    let (body, signatures) = note
        .split_once("\n\n")
        .ok_or("the checkpoint has no signature block")?;
    let body = format!("{body}\n");
    let mut lines = body.lines();
    let origin = lines.next().unwrap_or_default().to_string();
    let size = lines
        .next()
        .and_then(|l| l.parse::<u64>().ok())
        .ok_or("the checkpoint size is malformed")?;
    let root = lines.next().unwrap_or_default().to_string();
    if !origin.starts_with(&key.name) {
        return Err("the checkpoint is from another log".into());
    }
    let prefix = format!("\u{2014} {} ", key.name);
    let mut verified = false;
    for line in signatures.lines().filter(|l| !l.is_empty()) {
        let Some(sig) = line.strip_prefix(&prefix) else {
            continue;
        };
        let raw = STANDARD
            .decode(sig)
            .map_err(|_| "a checkpoint signature is not base64")?;
        if raw.len() > 4 && raw[..4] == key.log_id[..4] {
            key.verify(body.as_bytes(), &raw[4..])?;
            verified = true;
        }
    }
    if !verified {
        return Err("the checkpoint is not signed by the pinned log key".into());
    }
    let checkpoint = Checkpoint { origin, size, root };
    checkpoint.root()?;
    Ok(checkpoint)
}

/// The largest verified checkpoint seen per log, kept by the client.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Witness {
    pub logs: BTreeMap<String, Checkpoint>,
}

impl Witness {
    /// Accept `next` if it is the same view, or a larger tree with a
    /// consistency proof from the one seen before.
    ///
    /// # Errors
    ///
    /// On a fork (same size, other root), a stale view (smaller tree), or
    /// growth without a valid consistency proof.
    pub fn advance(&mut self, next: &Checkpoint, consistency: Option<&[Hash]>) -> Result<()> {
        if let Some(seen) = self.logs.get(&next.origin) {
            if next.size < seen.size {
                return Err(
                    "the transparency checkpoint is stale: an older tree than one already seen"
                        .into(),
                );
            }
            if next.size == seen.size {
                if next.root != seen.root {
                    return Err("the transparency log forked: two roots for one size".into());
                }
                return Ok(());
            }
            let proof =
                consistency.ok_or("the log grew, but no consistency proof shows it only grew")?;
            verify_consistency(seen.size, next.size, &seen.root()?, &next.root()?, proof)?;
        }
        self.logs.insert(next.origin.clone(), next.clone());
        Ok(())
    }
}

/// A Rekor log entry as returned by `/api/v1/log/entries`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub body: String,
    pub integrated_time: u64,
    #[serde(rename = "logID")]
    pub log_id: String,
    pub log_index: u64,
    pub verification: Verification,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub inclusion_proof: InclusionProof,
    pub signed_entry_timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InclusionProof {
    pub checkpoint: String,
    pub hashes: Vec<String>,
    pub log_index: u64,
    pub root_hash: String,
    pub tree_size: u64,
}

fn unhex32(text: &str) -> Result<Hash> {
    if text.len() != 64 {
        return Err("a hash is not 64 hex characters".into());
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| "a hash is not hex")?;
    }
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What a verified entry establishes.
#[derive(Clone, Debug)]
pub struct Included {
    pub body: Value,
    pub integrated_time: u64,
    pub log_index: u64,
    pub checkpoint: Checkpoint,
}

/// Verify an entry: the signed entry timestamp, inclusion under a signed
/// checkpoint, and the checkpoint against the witness.
///
/// # Errors
///
/// When any signature, proof, or the witness check fails.
pub fn verify_entry(
    entry: &Entry,
    key: &LogKey,
    witness: &mut Witness,
    consistency: Option<&[Hash]>,
) -> Result<Included> {
    if entry.log_id != hex(&key.log_id) {
        return Err("the entry is from another log".into());
    }
    // The signed entry timestamp covers the body, time, log and index.
    let set = serde_json::json!({
        "body": entry.body, "integratedTime": entry.integrated_time,
        "logID": entry.log_id, "logIndex": entry.log_index,
    });
    let set_bytes = serde_json::to_vec(&set).map_err(|_| "the entry does not encode")?;
    let set_sig = STANDARD
        .decode(&entry.verification.signed_entry_timestamp)
        .map_err(|_| "the entry timestamp is not base64")?;
    key.verify(&set_bytes, &set_sig)
        .map_err(|_| "the signed entry timestamp does not verify")?;
    let body = STANDARD
        .decode(&entry.body)
        .map_err(|_| "the entry body is not base64")?;
    let proof = &entry.verification.inclusion_proof;
    let checkpoint = checkpoint(&proof.checkpoint, key)?;
    let root = unhex32(&proof.root_hash)?;
    if checkpoint.size != proof.tree_size || checkpoint.root()? != root {
        return Err("the inclusion proof names another tree than its checkpoint".into());
    }
    let hashes: Vec<Hash> = proof
        .hashes
        .iter()
        .map(|h| unhex32(h))
        .collect::<Result<_>>()?;
    verify_inclusion(
        proof.log_index,
        proof.tree_size,
        &leaf_hash(&body),
        &hashes,
        &root,
    )?;
    witness.advance(&checkpoint, consistency)?;
    Ok(Included {
        body: serde_json::from_slice(&body).map_err(|_| "the entry body is not JSON")?,
        integrated_time: entry.integrated_time,
        log_index: entry.log_index,
        checkpoint,
    })
}
