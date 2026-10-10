//! The key index (NIP-VAULT "Key index"), tier `user`.
//!
//! The index lists the person's objects with their names and the wraps of
//! their data keys. A stored object carries no wrap of its own, so its data
//! key exists only here. The index is sealed as
//! `"OAVIDX01" ‖ be32(epoch) ‖ nonce (12 B) ‖ AES-256-GCM(index_key(epoch), JCS(index))`
//! with associated data `"openagents.vault.v1/index\0" ‖ vault id (32 B) ‖ be32(epoch)`.
//! Every change is written under the next epoch, and the service deletes
//! the previous one, so a removed file's key is gone with it.

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::keys::{Dek, Vmk, open, seal};
use crate::object::{self, Header, New, Tier, Wrap};
use crate::{Error, Result, hex, jcs, random, unhex, valid_id};

pub const INDEX_MAGIC: &[u8; 8] = b"OAVIDX01";
pub const INDEX_V: &str = "openagents.vault-index.v1";
pub const INDEX_AAD: &[u8] = b"openagents.vault.v1/index\0";
/// The most objects one index lists.
pub const MAX_ENTRIES: usize = 4096;
/// The longest file name kept, in characters.
pub const MAX_NAME: usize = 200;

/// What an object is to the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A file they added.
    File,
    /// An answer a model gave about their files, kept under the same key.
    Answer,
}

/// Where a model read the plaintext for an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    /// A model on the person's own device (a local Psionic server).
    Device,
    /// Google Gemini on Vertex AI, through the service.
    Fast,
    /// An attested model (NIP-ATT), not offered yet.
    Private,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub object: String,
    pub kind: Kind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// Plaintext bytes.
    pub size: u64,
    /// The project the file belongs to, when it was added in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// For an answer: the files it was about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub about: Vec<String>,
    /// For an answer: where the model read them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// The object's core digest, hex: the stored bytes must match it.
    pub core_digest: String,
    pub wraps: Vec<Wrap>,
    pub created_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub v: String,
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    pub vault: String,
    pub epoch: u32,
    pub entries: Vec<Entry>,
    /// Tier `sealed` system key certificates. Empty in tier `user`.
    #[serde(default)]
    pub system_keys: Vec<serde_json::Value>,
    /// The person's NIP-ATT approvals. Empty in tier `user`.
    #[serde(default)]
    pub approvals: Vec<serde_json::Value>,
}

impl Index {
    /// An empty index for a new vault, at epoch 1.
    pub fn new(vault: &str) -> Result<Self> {
        if !valid_id(vault) {
            return Err(Error::Format("The vault id is invalid."));
        }
        Ok(Self {
            v: INDEX_V.to_owned(),
            requires: Vec::new(),
            meta: None,
            vault: vault.to_owned(),
            epoch: 1,
            entries: Vec::new(),
            system_keys: Vec::new(),
            approvals: Vec::new(),
        })
    }

    fn aad(&self) -> Result<Vec<u8>> {
        let mut aad = INDEX_AAD.to_vec();
        aad.extend_from_slice(&unhex::<32>(&self.vault)?);
        aad.extend_from_slice(&self.epoch.to_be_bytes());
        Ok(aad)
    }

    /// Seal the index under its epoch's key.
    pub fn seal(&self, vmk: &Vmk) -> Result<Vec<u8>> {
        self.seal_with(vmk, random::<12>()?)
    }

    /// [`Index::seal`] with the nonce given (test vectors).
    pub fn seal_with(&self, vmk: &Vmk, nonce: [u8; 12]) -> Result<Vec<u8>> {
        self.check()?;
        let plain = Zeroizing::new(jcs::to_vec(self)?);
        let key = vmk.index_key(self.epoch);
        let mut out = INDEX_MAGIC.to_vec();
        out.extend_from_slice(&self.epoch.to_be_bytes());
        out.extend_from_slice(&nonce);
        out.extend(seal(&key, &nonce, &self.aad()?, &plain)?);
        Ok(out)
    }

    /// The epoch a sealed index claims, read without opening it.
    pub fn epoch_of(sealed: &[u8]) -> Result<u32> {
        if sealed.len() < 12 + 12 + 16 || &sealed[..8] != INDEX_MAGIC {
            return Err(Error::Format("This is not a vault key index."));
        }
        Ok(u32::from_be_bytes([
            sealed[8], sealed[9], sealed[10], sealed[11],
        ]))
    }

    /// Open a sealed index of `vault`.
    pub fn open(vmk: &Vmk, vault: &str, sealed: &[u8]) -> Result<Self> {
        let epoch = Self::epoch_of(sealed)?;
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&sealed[12..24]);
        let probe = Self {
            epoch,
            ..Self::new(vault)?
        };
        let plain = open(
            &vmk.index_key(epoch),
            &nonce,
            &probe.aad()?,
            &sealed[24..],
            "This vault's file list can't be opened with this key.",
        )?;
        let index: Self = serde_json::from_slice(&plain)
            .map_err(|_| Error::Format("The vault's file list is invalid."))?;
        if index.vault != vault || index.epoch != epoch {
            return Err(Error::Decrypt(
                "The vault's file list belongs to another vault.",
            ));
        }
        index.check()?;
        Ok(index)
    }

    fn check(&self) -> Result<()> {
        if self.v != INDEX_V {
            return Err(Error::Format("This is not a key index of a known version."));
        }
        if !self.requires.is_empty() {
            return Err(Error::Refused(
                "The index needs a feature this client lacks.",
            ));
        }
        if self.epoch == 0 || self.entries.len() > MAX_ENTRIES {
            return Err(Error::Format("The key index is invalid."));
        }
        for entry in &self.entries {
            if !valid_id(&entry.object) || entry.name.chars().count() > MAX_NAME {
                return Err(Error::Format("A key index entry is invalid."));
            }
            unhex::<32>(&entry.core_digest)?;
        }
        Ok(())
    }

    pub fn find(&self, object: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.object == object)
    }

    /// The next epoch's index: the same entries, `epoch + 1`.
    pub fn next(&self) -> Result<Self> {
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or(Error::Refused("This vault has changed too many times."))?;
        Ok(Self {
            epoch,
            ..self.clone()
        })
    }

    /// Seal `plain` as a new `user`-tier object and list it in the next
    /// epoch's index. Returns the object's bytes and the new index. The
    /// stored object carries no wrap: its data key is only in the index.
    pub fn add(&self, vmk: &Vmk, add: Add<'_>, plain: &[u8]) -> Result<(Vec<u8>, Self)> {
        if self.entries.len() >= MAX_ENTRIES {
            return Err(Error::Refused("This vault holds as many files as it can."));
        }
        let object = crate::new_id()?;
        let sealed = object::seal_new(
            &New {
                object: &object,
                vault: &self.vault,
                tier: Tier::User,
                // The media type stays in the index, not in what the
                // service stores.
                media: None,
                created_at: add.created_at,
            },
            plain,
            Vec::new(),
        )?;
        let digest = sealed.header.core_digest()?;
        let wrap = object::wrap_user(vmk, &digest, &sealed.dek)?;
        let mut next = self.next()?;
        next.entries.push(Entry {
            object,
            kind: add.kind,
            name: clean_name(add.name),
            media: add.media.map(str::to_owned),
            size: plain.len() as u64,
            project: add.project.map(str::to_owned),
            about: add.about,
            route: add.route,
            core_digest: hex(&digest),
            wraps: vec![wrap],
            created_at: add.created_at,
        });
        Ok((sealed.bytes, next))
    }

    /// Open object `bytes`, listed here as `object`.
    pub fn open_object(&self, vmk: &Vmk, object: &str, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let entry = self
            .find(object)
            .ok_or(Error::Refused("This file isn't in your vault any more."))?;
        let (header, _) = object::parse(bytes)?;
        let digest = header.core_digest()?;
        if hex(&digest) != entry.core_digest
            || header.core.object != object
            || header.core.vault != self.vault
        {
            return Err(Error::Decrypt(
                "The stored file isn't the one in your vault.",
            ));
        }
        let dek = self.dek(vmk, entry, &header)?;
        object::open_object(bytes, &dek)
    }

    fn dek(&self, vmk: &Vmk, entry: &Entry, header: &Header) -> Result<Dek> {
        let digest = header.core_digest()?;
        let key = vmk.user_key_id();
        let wrap = entry
            .wraps
            .iter()
            .chain(header.wraps.iter())
            .find(|wrap| matches!(wrap, Wrap::User { key: k, .. } if *k == key))
            .ok_or(Error::Decrypt("No key in your vault opens this file."))?;
        object::unwrap_user(vmk, &digest, wrap)
    }

    /// The next epoch's index without `object`. Once the service deletes
    /// the current epoch, the object's key exists nowhere.
    pub fn remove(&self, object: &str) -> Result<Self> {
        if self.find(object).is_none() {
            return Err(Error::Refused("This file isn't in your vault any more."));
        }
        let mut next = self.next()?;
        next.entries.retain(|entry| entry.object != object);
        Ok(next)
    }

    /// Every entry's wrap rewrapped under `new` (a new master key after a
    /// device was removed), at the next epoch. Object content is untouched.
    pub fn rewrap(&self, old: &Vmk, new: &Vmk) -> Result<Self> {
        let mut next = self.next()?;
        let old_key = old.user_key_id();
        for entry in &mut next.entries {
            let digest = unhex::<32>(&entry.core_digest)?;
            let wrap = entry
                .wraps
                .iter()
                .find(|wrap| matches!(wrap, Wrap::User { key, .. } if *key == old_key))
                .ok_or(Error::Decrypt("No key in your vault opens a file."))?;
            let dek = object::unwrap_user(old, &digest, wrap)?;
            entry.wraps = vec![object::wrap_user(new, &digest, &dek)?];
        }
        Ok(next)
    }
}

/// What a new object is, for [`Index::add`].
#[derive(Clone, Debug)]
pub struct Add<'a> {
    pub kind: Kind,
    pub name: &'a str,
    pub media: Option<&'a str>,
    pub project: Option<&'a str>,
    pub about: Vec<String>,
    pub route: Option<Route>,
    pub created_at: u64,
}

/// A file name with control characters removed, at most [`MAX_NAME`]
/// characters, never empty.
pub fn clean_name(raw: &str) -> String {
    let name: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect::<String>()
        .trim()
        .to_owned();
    if name.is_empty() {
        "File".to_owned()
    } else {
        name
    }
}
