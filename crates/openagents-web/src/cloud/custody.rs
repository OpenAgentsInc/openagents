//! Server-side custody of a customer's own provider API key.
//!
//! This is the custody class [Bring your own Claude](../../../../docs/cloud/claude-code-byo.md)
//! rule 8 describes: a user's own API key, stored for that user's own scope,
//! revocable, released only to one exact admitted flow, and never exported.
//! It is deliberately not a store for Claude.ai logins: an OAuth or
//! `claude setup-token` value is refused for every material.
//!
//! Each entry binds the account, workspace, membership epoch, subject (the
//! flow that may receive it), and material. A changed epoch or subject finds
//! no entry. Status shows only a digest; no API returns the key except
//! [`Vault::release`], which requires the exact digest the flow reviewed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

pub const SCHEMA: &str = "openagents.cloud.provider-key-custody.v1";
const ENTRY_MAX: u64 = 16 * 1024;
const KEY_MAX: usize = 8192;

/// The kind of customer-owned credential. Claude.ai plan logins are not a
/// material and can never be stored here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    /// The customer's own OpenAI API key, billed to the customer.
    OpenAiApiKey,
    /// The customer's own Anthropic API key, billed to the customer.
    AnthropicApiKey,
    /// The customer's own Amazon Bedrock credential (BYO-04).
    BedrockCredential,
    /// The customer's own Google Vertex AI service account (BYO-04).
    VertexCredential,
    /// The customer's own Microsoft Foundry credential (BYO-04).
    FoundryCredential,
}

impl Material {
    pub fn label(self) -> &'static str {
        match self.claude() {
            Some(class) => class.label(),
            None => "your own OpenAI API key",
        }
    }

    /// The Claude Code credential class this material carries, if any.
    pub fn claude(self) -> Option<coder_cloud::claude::OwnCredential> {
        use coder_cloud::claude::OwnCredential;
        match self {
            Self::OpenAiApiKey => None,
            Self::AnthropicApiKey => Some(OwnCredential::AnthropicApiKey),
            Self::BedrockCredential => Some(OwnCredential::Bedrock),
            Self::VertexCredential => Some(OwnCredential::Vertex),
            Self::FoundryCredential => Some(OwnCredential::Foundry),
        }
    }
}

/// Who may hold the key and which exact flow may receive it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
    /// The admitted consumer, for example `retail:<delegation>`.
    pub subject: String,
    pub material: Material,
}

impl Scope {
    fn file(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("scope serializes");
        format!("{}.json", hex(&Sha256::digest(bytes)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustodyError {
    /// The key is empty, oversized, has control characters, or is a Claude.ai login.
    Invalid,
    /// Explicit custody consent was not given.
    Consent,
    /// No current entry for this exact scope, or it expired or was revoked.
    Absent,
    /// The stored key is not the one the flow reviewed.
    Changed,
    /// The private directory or entry changed, is shared, or is unreadable.
    Unavailable,
}

impl std::fmt::Display for CustodyError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Invalid => {
                "Enter one API key without spaces or control characters. Claude.ai logins and setup tokens are never accepted."
            }
            Self::Consent => "Custody needs your explicit consent.",
            Self::Absent => "No current key is in custody for this workspace and flow.",
            Self::Changed => "The key in custody changed. Review the request again.",
            Self::Unavailable => "Key custody is unavailable.",
        })
    }
}

/// A key in memory. It never prints or serializes; its bytes are zeroed on drop.
pub struct Key(String);

impl Key {
    pub fn new(value: String) -> Result<Self, CustodyError> {
        let key = Self(value);
        let text = key.0.as_str();
        if text.is_empty()
            || text.len() > KEY_MAX
            || text
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            // Claude.ai OAuth and `claude setup-token` values: never collected.
            || text.starts_with("sk-ant-oat")
        {
            return Err(CustodyError::Invalid);
        }
        Ok(key)
    }
    /// Validate `value` for `material`. A Claude Code class is stored in its
    /// canonical form (a key, or a compact JSON document whose private-key
    /// text may hold spaces); every class refuses Claude.ai logins.
    pub fn for_material(material: Material, value: String) -> Result<Self, CustodyError> {
        let Some(class) = material.claude() else {
            return Self::new(value);
        };
        // The submitted bytes are zeroed when `submitted` drops.
        let submitted = Self(value);
        let key = Self(
            class
                .canonical(&submitted.0)
                .map_err(|_| CustodyError::Invalid)?,
        );
        if key.0.is_empty()
            || key.0.len() > KEY_MAX
            || key.0.chars().any(char::is_control)
            || key.0.starts_with("sk-ant-oat")
        {
            return Err(CustodyError::Invalid);
        }
        Ok(key)
    }
    /// SHA-256 hex of the key, which reviews and status bind.
    pub fn digest(&self) -> String {
        hex(&Sha256::digest(self.0.as_bytes()))
    }
    /// Move the bytes into the admitted delivery; the caller owns zeroing.
    pub fn into_delivery(mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Key(redacted)")
    }
}

/// Masked standing: never the key or a fragment of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    pub material: Material,
    pub digest: String,
    pub stored_at: u64,
    pub expires_at: u64,
    pub terms: String,
}

impl Status {
    pub fn masked(&self) -> String {
        format!("SHA-256 {}…", &self.digest[..12])
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema: String,
    scope: Scope,
    digest: String,
    stored_at: u64,
    expires_at: u64,
    /// Digest of the custody terms the person accepted.
    terms: String,
    key: String,
}

impl Drop for Entry {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.key).into_bytes();
        bytes.fill(0);
    }
}

/// A private, operator-provisioned directory (owned by this user, mode 0700,
/// no symbolic links). Entry files are mode 0600 and never shared.
pub struct Vault {
    root: PathBuf,
    pin: (u64, u64),
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self, String> {
        let metadata = checked_directory(root).map_err(|_| "Key custody is unavailable.")?;
        Ok(Self {
            root: root.into(),
            pin: (metadata.dev(), metadata.ino()),
        })
    }

    fn check(&self) -> Result<(), CustodyError> {
        let metadata = checked_directory(&self.root)?;
        if (metadata.dev(), metadata.ino()) != self.pin {
            return Err(CustodyError::Unavailable);
        }
        Ok(())
    }

    /// Store `key` for `scope` until `expires_at`, replacing an earlier key
    /// for the same scope. `terms` is the digest of the displayed custody
    /// terms the person accepted with `consent`.
    pub fn store(
        &self,
        scope: &Scope,
        key: Key,
        consent: bool,
        terms: &str,
        now: u64,
        expires_at: u64,
    ) -> Result<Status, CustodyError> {
        if !consent {
            return Err(CustodyError::Consent);
        }
        if expires_at <= now || terms.is_empty() || terms.len() > 128 {
            return Err(CustodyError::Invalid);
        }
        self.check()?;
        let entry = Entry {
            schema: SCHEMA.into(),
            scope: scope.clone(),
            digest: key.digest(),
            stored_at: now,
            expires_at,
            terms: terms.into(),
            key: key.into_delivery(),
        };
        let mut bytes = serde_json::to_vec(&entry).map_err(|_| CustodyError::Unavailable)?;
        self.check()?;
        let result = write_private(&self.root, &scope.file(), &bytes);
        bytes.fill(0);
        result?;
        Ok(status(&entry))
    }

    fn load(&self, scope: &Scope, now: u64) -> Result<Option<Entry>, CustodyError> {
        self.check()?;
        let path = self.root.join(scope.file());
        let Some(mut bytes) = read_private(&path, ENTRY_MAX)? else {
            return Ok(None);
        };
        let parsed = serde_json::from_slice::<Entry>(&bytes).ok();
        bytes.fill(0);
        let entry = parsed.ok_or(CustodyError::Unavailable)?;
        if entry.schema != SCHEMA || entry.scope != *scope {
            return Err(CustodyError::Unavailable);
        }
        if entry.expires_at <= now {
            erase(&path)?;
            return Ok(None);
        }
        Ok(Some(entry))
    }

    pub fn status(&self, scope: &Scope, now: u64) -> Result<Option<Status>, CustodyError> {
        Ok(self.load(scope, now)?.as_ref().map(status))
    }

    /// Release the key only when it is still the exact reviewed key.
    pub fn release(&self, scope: &Scope, digest: &str, now: u64) -> Result<Key, CustodyError> {
        let mut entry = self.load(scope, now)?.ok_or(CustodyError::Absent)?;
        if entry.digest != digest {
            return Err(CustodyError::Changed);
        }
        Key::new(std::mem::take(&mut entry.key)).map_err(|_| CustodyError::Unavailable)
    }

    /// Overwrite and remove the entry. Returns whether one existed.
    pub fn revoke(&self, scope: &Scope) -> Result<bool, CustodyError> {
        self.check()?;
        let path = self.root.join(scope.file());
        if fs::symlink_metadata(&path).is_err() {
            return Ok(false);
        }
        erase(&path)?;
        Ok(true)
    }
}

/// Atomically replace `root/name` with a new private (0600) file. The caller
/// has checked `root`; a leftover partial file is erased first.
pub(super) fn write_private(root: &Path, name: &str, bytes: &[u8]) -> Result<(), CustodyError> {
    let next = root.join(format!("{name}.next"));
    if fs::symlink_metadata(&next).is_ok() {
        erase(&next)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&next)
        .map_err(|_| CustodyError::Unavailable)?;
    if file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        let _ = erase(&next);
        return Err(CustodyError::Unavailable);
    }
    fs::rename(&next, root.join(name)).map_err(|_| CustodyError::Unavailable)?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| CustodyError::Unavailable)
}

/// Read a private (0600, owned, unshared) bounded file, or `None` if absent.
pub(super) fn read_private(path: &Path, maximum: u64) -> Result<Option<Vec<u8>>, CustodyError> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CustodyError::Unavailable),
    };
    let metadata = file.metadata().map_err(|_| CustodyError::Unavailable)?;
    if !private_file(&metadata) || metadata.len() > maximum {
        return Err(CustodyError::Unavailable);
    }
    let mut bytes = Vec::new();
    if Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > maximum
    {
        bytes.fill(0);
        return Err(CustodyError::Unavailable);
    }
    Ok(Some(bytes))
}

pub(super) fn checked_root(root: &Path) -> Result<(), CustodyError> {
    checked_directory(root).map(|_| ())
}

fn status(entry: &Entry) -> Status {
    Status {
        material: entry.scope.material,
        digest: entry.digest.clone(),
        stored_at: entry.stored_at,
        expires_at: entry.expires_at,
        terms: entry.terms.clone(),
    }
}

/// Zero the bytes in place before unlinking, so a later reader of the
/// directory or a backup of the inode cannot recover the key.
fn erase(path: &Path) -> Result<(), CustodyError> {
    let file = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| CustodyError::Unavailable)?;
    let metadata = file.metadata().map_err(|_| CustodyError::Unavailable)?;
    if !private_file(&metadata) || metadata.len() > ENTRY_MAX + KEY_MAX as u64 {
        return Err(CustodyError::Unavailable);
    }
    let mut file = file;
    let zeros = vec![0; metadata.len() as usize];
    file.write_all(&zeros)
        .and_then(|()| file.sync_all())
        .map_err(|_| CustodyError::Unavailable)?;
    fs::remove_file(path).map_err(|_| CustodyError::Unavailable)
}

fn private_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o077 == 0
        && metadata.nlink() == 1
}

fn checked_directory(root: &Path) -> Result<fs::Metadata, CustodyError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(CustodyError::Unavailable);
    }
    let mut prefix = PathBuf::new();
    for part in root.components() {
        prefix.push(part);
        let metadata = fs::symlink_metadata(&prefix).map_err(|_| CustodyError::Unavailable)?;
        if metadata.file_type().is_symlink() {
            return Err(CustodyError::Unavailable);
        }
    }
    let metadata = fs::symlink_metadata(root).map_err(|_| CustodyError::Unavailable)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(CustodyError::Unavailable);
    }
    Ok(metadata)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const KEY: &str = "synthetic-openai-key-for-custody-tests";

    fn vault() -> (tempfile::TempDir, PathBuf, Vault) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("vault");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let vault = Vault::open(&root).unwrap();
        (temp, root, vault)
    }

    fn scope(epoch: u64) -> Scope {
        Scope {
            account: "alice".into(),
            workspace: "alice-personal".into(),
            members_epoch: epoch,
            subject: "retail:alice".into(),
            material: Material::OpenAiApiKey,
        }
    }

    #[test]
    fn stores_masked_releases_exactly_and_revokes() {
        let (_temp, root, vault) = vault();
        assert_eq!(
            vault
                .store(&scope(3), Key::new(KEY.into()).unwrap(), false, "t", 10, 20)
                .unwrap_err(),
            CustodyError::Consent
        );
        let status = vault
            .store(&scope(3), Key::new(KEY.into()).unwrap(), true, "t", 10, 20)
            .unwrap();
        assert!(!status.masked().contains(KEY));
        assert!(!format!("{status:?}").contains(KEY));
        let entry = fs::read_dir(&root).unwrap().next().unwrap().unwrap();
        assert_eq!(entry.metadata().unwrap().mode() & 0o777, 0o600);
        // Another epoch, subject, or material is another scope.
        assert_eq!(vault.status(&scope(4), 11).unwrap(), None);
        let mut other = scope(3);
        other.subject = "byo:computers".into();
        assert!(vault.release(&other, &status.digest, 11).is_err());
        assert_eq!(
            vault
                .release(&scope(3), "0".repeat(64).as_str(), 11)
                .unwrap_err(),
            CustodyError::Changed
        );
        let key = vault.release(&scope(3), &status.digest, 11).unwrap();
        assert_eq!(format!("{key:?}"), "Key(redacted)");
        assert_eq!(key.into_delivery(), KEY);
        // Expiry removes the entry.
        assert_eq!(vault.status(&scope(3), 20).unwrap(), None);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        vault
            .store(&scope(3), Key::new(KEY.into()).unwrap(), true, "t", 10, 20)
            .unwrap();
        assert!(vault.revoke(&scope(3)).unwrap());
        assert!(!vault.revoke(&scope(3)).unwrap());
        assert_eq!(
            vault.release(&scope(3), &status.digest, 11).unwrap_err(),
            CustodyError::Absent
        );
    }

    #[test]
    fn refuses_claude_logins_shared_directories_and_replaced_roots() {
        for value in ["", "has space", "line\nbreak", "sk-ant-oat01-plan-login"] {
            assert_eq!(Key::new(value.into()).unwrap_err(), CustodyError::Invalid);
        }
        assert!(Key::new("x".repeat(KEY_MAX + 1)).is_err());
        let (_temp, root, vault) = vault();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            vault
                .store(&scope(3), Key::new(KEY.into()).unwrap(), true, "t", 1, 9)
                .is_err()
        );
        assert!(Vault::open(&root).is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let moved = root.with_file_name("moved");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            vault.status(&scope(3), 1).unwrap_err(),
            CustodyError::Unavailable
        );
    }
}
