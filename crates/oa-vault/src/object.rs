//! The vault object (NIP-VAULT "Vault object" and "Wraps"):
//! `"OAVAULT1" ‖ be32(header_len) ‖ JCS(header) ‖ chunks`.

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::keys::{Dek, Vmk, open, seal};
use crate::{Error, Result, b64, jcs, random, sha256, unb64, unb64_n, valid_id};

pub const MAGIC: &[u8; 8] = b"OAVAULT1";
pub const OBJECT_V: &str = "openagents.vault-object.v1";
pub const CONTENT_ALG: &str = "aes-256-gcm-chunked-v1";
pub const CHUNK_BYTES: u32 = 65_536;
const TAG: usize = 16;
pub const CHUNK_AAD: &[u8] = b"openagents.vault.v1/chunk\0";
pub const WRAP_USER_AAD: &[u8] = b"openagents.vault.v1/wrap-user\0";
/// The longest header a reader accepts.
pub const MAX_HEADER: u32 = 64 * 1024;
/// The smallest and largest chunk sizes a reader accepts.
const MIN_CHUNK: u32 = 1024;
const MAX_CHUNK: u32 = 1024 * 1024;

/// Who can open an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    User,
    Sealed,
    Operator,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub alg: String,
    pub chunk_bytes: u32,
    /// 7 bytes, base64.
    pub nonce_prefix: String,
    pub chunks: u32,
}

/// The part of the header that the content and every wrap are bound to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Core {
    pub object: String,
    pub vault: String,
    pub tier: Tier,
    pub content: Content,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    pub created_at: u64,
}

/// One way to open an object's data key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum Wrap {
    /// AES-256-GCM under `K_user_wrap`.
    User {
        key: String,
        /// 12 bytes, base64.
        nonce: String,
        /// 48 bytes (ciphertext and tag), base64.
        dek: String,
    },
    /// HPKE `mode_psk` to the person's system key (tier `sealed`). Parsed and
    /// kept, never opened by a client.
    Sealed {
        system_key: String,
        user_key: String,
        suite: String,
        enc: String,
        dek: String,
    },
    /// A KMS-wrapped key (tier `operator`). Parsed and kept, never opened by
    /// a client.
    Operator { kms_key: String, dek: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    pub core: Core,
    pub wraps: Vec<Wrap>,
}

impl Header {
    /// `core_digest = SHA-256(JCS(core))`, the raw 32 bytes. JSON carries it
    /// as lowercase hex.
    pub fn core_digest(&self) -> Result<[u8; 32]> {
        Ok(sha256(&jcs::to_vec(&self.core)?))
    }

    fn check(&self) -> Result<()> {
        if self.v != OBJECT_V {
            return Err(Error::Format(
                "This is not a vault object of a known version.",
            ));
        }
        if !self.requires.is_empty() {
            return Err(Error::Refused(
                "The object needs a feature this client lacks.",
            ));
        }
        let core = &self.core;
        if !valid_id(&core.object) || !valid_id(&core.vault) {
            return Err(Error::Format("The object or vault id is invalid."));
        }
        let content = &core.content;
        if content.alg != CONTENT_ALG
            || !(MIN_CHUNK..=MAX_CHUNK).contains(&content.chunk_bytes)
            || content.chunks == 0
        {
            return Err(Error::Format("The object's content scheme is unknown."));
        }
        unb64_n::<7>(&content.nonce_prefix)?;
        if core
            .media
            .as_ref()
            .is_some_and(|m| m.is_empty() || m.len() > 127)
        {
            return Err(Error::Format("The object's media type is invalid."));
        }
        for wrap in &self.wraps {
            match (wrap, core.tier) {
                (Wrap::Operator { .. }, Tier::Operator) => {}
                (Wrap::Operator { .. }, _) => {
                    return Err(Error::Refused(
                        "Only an operator-tier object may carry an operator wrap.",
                    ));
                }
                (Wrap::Sealed { .. }, Tier::User) => {
                    return Err(Error::Refused(
                        "A user-tier object can't carry a sealed wrap.",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// What a new object is.
#[derive(Clone, Debug)]
pub struct New<'a> {
    pub object: &'a str,
    pub vault: &'a str,
    pub tier: Tier,
    pub media: Option<&'a str>,
    pub created_at: u64,
}

/// A sealed object: its bytes, its header, and the data key that opens it.
pub struct Sealed {
    pub bytes: Vec<u8>,
    pub header: Header,
    pub dek: Dek,
}

/// Seal `plain` as a new object under a fresh data key and nonce prefix.
/// The stored object carries `wraps` (often none: then the data key lives
/// only in the key index, so deleting it there shreds the object).
pub fn seal_new(new: &New<'_>, plain: &[u8], wraps: Vec<Wrap>) -> Result<Sealed> {
    seal_with(
        new,
        plain,
        wraps,
        Dek::generate()?,
        random::<7>()?,
        CHUNK_BYTES,
    )
}

/// [`seal_new`] with every random value given (test vectors).
pub fn seal_with(
    new: &New<'_>,
    plain: &[u8],
    wraps: Vec<Wrap>,
    dek: Dek,
    nonce_prefix: [u8; 7],
    chunk_bytes: u32,
) -> Result<Sealed> {
    let size = chunk_bytes as usize;
    let chunks = if plain.is_empty() {
        1
    } else {
        plain.len().div_ceil(size)
    };
    let header = Header {
        v: OBJECT_V.to_owned(),
        requires: Vec::new(),
        meta: None,
        core: Core {
            object: new.object.to_owned(),
            vault: new.vault.to_owned(),
            tier: new.tier,
            content: Content {
                alg: CONTENT_ALG.to_owned(),
                chunk_bytes,
                nonce_prefix: b64(&nonce_prefix),
                chunks: u32::try_from(chunks)
                    .map_err(|_| Error::Refused("The file is too large."))?,
            },
            media: new.media.map(str::to_owned),
            created_at: new.created_at,
        },
        wraps,
    };
    header.check()?;
    let digest = header.core_digest()?;
    let head = jcs::to_vec(&header)?;
    let head_len = u32::try_from(head.len())
        .ok()
        .filter(|len| *len <= MAX_HEADER)
        .ok_or(Error::Refused("The object header is too large."))?;
    let mut bytes = Vec::with_capacity(12 + head.len() + plain.len() + chunks * TAG);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&head_len.to_be_bytes());
    bytes.extend_from_slice(&head);
    let aad = chunk_aad(&digest);
    for index in 0..chunks {
        let start = index * size;
        let end = (start + size).min(plain.len());
        let nonce = chunk_nonce(&nonce_prefix, index as u32, index + 1 == chunks);
        bytes.extend(seal(dek.bytes(), &nonce, &aad, &plain[start..end])?);
    }
    Ok(Sealed { bytes, header, dek })
}

fn chunk_aad(digest: &[u8; 32]) -> Vec<u8> {
    let mut aad = CHUNK_AAD.to_vec();
    aad.extend_from_slice(digest);
    aad
}

/// `nonce_prefix (7 B) ‖ be32(i) ‖ last (0x01 for the final chunk, else 0x00)`.
fn chunk_nonce(prefix: &[u8; 7], index: u32, last: bool) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..7].copy_from_slice(prefix);
    nonce[7..11].copy_from_slice(&index.to_be_bytes());
    nonce[11] = u8::from(last);
    nonce
}

/// The header of an object and where its chunks start. Checks the magic,
/// the header, and the header's own rules; not the content.
pub fn parse(bytes: &[u8]) -> Result<(Header, usize)> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(Error::Format("This is not a vault object."));
    }
    let len = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if len > MAX_HEADER || bytes.len() < 12 + len as usize {
        return Err(Error::Format("The vault object's header is cut short."));
    }
    let raw = &bytes[12..12 + len as usize];
    let header: Header = serde_json::from_slice(raw)
        .map_err(|_| Error::Format("The vault object's header is invalid."))?;
    if jcs::to_vec(&header)? != raw {
        return Err(Error::Format("The vault object's header is not canonical."));
    }
    header.check()?;
    Ok((header, 12 + len as usize))
}

/// Open an object with its data key. Refuses a stream that is truncated,
/// extended, reordered, or changed.
pub fn open_object(bytes: &[u8], dek: &Dek) -> Result<Zeroizing<Vec<u8>>> {
    let (header, start) = parse(bytes)?;
    let digest = header.core_digest()?;
    let content = &header.core.content;
    let prefix = unb64_n::<7>(&content.nonce_prefix)?;
    let size = content.chunk_bytes as usize + TAG;
    let body = &bytes[start..];
    let pieces: Vec<&[u8]> = if body.is_empty() {
        Vec::new()
    } else {
        body.chunks(size).collect()
    };
    if pieces.len() != content.chunks as usize {
        return Err(Error::Decrypt("The file is incomplete or has extra data."));
    }
    let aad = chunk_aad(&digest);
    let mut plain = Zeroizing::new(Vec::with_capacity(body.len()));
    for (index, piece) in pieces.iter().enumerate() {
        let last = index + 1 == pieces.len();
        let nonce = chunk_nonce(&prefix, index as u32, last);
        let chunk = open(
            dek.bytes(),
            &nonce,
            &aad,
            piece,
            "The file can't be opened with this key.",
        )?;
        if chunk.is_empty() && !(last && pieces.len() == 1) {
            return Err(Error::Decrypt("The file has an empty chunk."));
        }
        plain.extend_from_slice(&chunk);
    }
    Ok(plain)
}

fn wrap_user_aad(digest: &[u8; 32]) -> Vec<u8> {
    let mut aad = WRAP_USER_AAD.to_vec();
    aad.extend_from_slice(digest);
    aad
}

/// The `user` wrap of `dek` for the object whose core digest is `digest`.
pub fn wrap_user(vmk: &Vmk, digest: &[u8; 32], dek: &Dek) -> Result<Wrap> {
    wrap_user_with(vmk, digest, dek, random::<12>()?)
}

/// [`wrap_user`] with the nonce given (test vectors).
pub fn wrap_user_with(vmk: &Vmk, digest: &[u8; 32], dek: &Dek, nonce: [u8; 12]) -> Result<Wrap> {
    let key = vmk.user_wrap_key();
    let sealed = seal(&key, &nonce, &wrap_user_aad(digest), dek.bytes())?;
    Ok(Wrap::User {
        key: vmk.user_key_id(),
        nonce: b64(&nonce),
        dek: b64(&sealed),
    })
}

/// Open a `user` wrap for the object whose core digest is `digest`.
pub fn unwrap_user(vmk: &Vmk, digest: &[u8; 32], wrap: &Wrap) -> Result<Dek> {
    let Wrap::User { key, nonce, dek } = wrap else {
        return Err(Error::Refused("Only a user wrap opens on this device."));
    };
    if *key != vmk.user_key_id() {
        return Err(Error::Decrypt("This wrap is for another vault key."));
    }
    let opened = open(
        &vmk.user_wrap_key(),
        &unb64_n::<12>(nonce)?,
        &wrap_user_aad(digest),
        &unb64(dek)?,
        "This wrap doesn't belong to this file.",
    )?;
    let bytes: [u8; 32] = opened
        .as_slice()
        .try_into()
        .map_err(|_| Error::Format("A wrapped data key has the wrong length."))?;
    Ok(Dek::from_bytes(bytes))
}
