//! Operator-owned publisher enrollment, distribution review, and revocation.
use super::{Diagnostic, Result, release, workspace};
use secp256k1::{Keypair, Secp256k1, XOnlyPublicKey, schnorr::Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};
const LIMIT: usize = 2 * 1024 * 1024;
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::at("publication.json", "$", message)
}
fn digest_valid(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
    pub authority: [u8; 32],
    pub publisher: [u8; 32],
    pub artifact: String,
    pub signature: Vec<u8>,
}
impl Submission {
    pub fn signing_digest(&self) -> Result<[u8; 32]> {
        if self.authority == [0; 32] || !digest_valid(&self.artifact) {
            return Err(error("Invalid publication authority or artifact"));
        }
        let mut hash = Sha256::new();
        hash.update(b"verse.public.submission.v1\0");
        hash.update(self.authority);
        hash.update(self.publisher);
        hash.update(self.artifact.as_bytes());
        Ok(hash.finalize().into())
    }
    /// Caller-supplied identity key; this module stores no signing secrets.
    pub fn sign(authority: [u8; 32], artifact: &str, key: &Keypair) -> Result<Self> {
        let mut value = Self {
            authority,
            publisher: key.x_only_public_key().0.serialize(),
            artifact: artifact.into(),
            signature: vec![],
        };
        value.signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&value.signing_digest()?, key)
            .to_byte_array()
            .to_vec();
        Ok(value)
    }
    fn verify(&self) -> Result<()> {
        let public = XOnlyPublicKey::from_byte_array(self.publisher)
            .map_err(|_| error("Invalid publisher identity"))?;
        let bytes: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| error("Invalid publisher signature length"))?;
        Secp256k1::verification_only()
            .verify_schnorr(
                &Signature::from_byte_array(bytes),
                &self.signing_digest()?,
                &public,
            )
            .map_err(|_| error("Publisher signature does not bind this artifact and authority"))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Approved,
    Rejected,
    Revoked,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub submission: Submission,
    pub status: Status,
    pub revision: u64,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Publisher {
    public: [u8; 32],
    enabled: bool,
    reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: String,
    authority: [u8; 32],
    revision: u64,
    publishers: BTreeMap<String, Publisher>,
    entries: BTreeMap<String, Entry>,
}
/// Trusted local operator capability. Never pass its mutation methods to a scene or client.
pub struct Book {
    root: PathBuf,
    _lock: File,
    state: State,
    poisoned: bool,
}
fn publisher_id(public: [u8; 32]) -> String {
    public.iter().map(|b| format!("{b:02x}")).collect()
}
fn entry_id(public: [u8; 32], artifact: &str) -> String {
    workspace::hash(
        &[
            b"verse.public.entry.v1\0".as_slice(),
            &public,
            artifact.as_bytes(),
        ]
        .concat(),
    )
}
impl Book {
    pub fn open(root: &Path, authority: [u8; 32]) -> Result<Self> {
        workspace::ancestors(root)?;
        if authority == [0; 32] {
            return Err(error("Publication authority must be nonzero"));
        }
        std::fs::create_dir_all(root).map_err(|e| error(e.to_string()))?;
        let lock_path = root.join("publication.lock");
        if lock_path.exists()
            && !std::fs::symlink_metadata(&lock_path)
                .map_err(|e| error(e.to_string()))?
                .is_file()
        {
            return Err(error("Publication lock must be a regular file"));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|e| error(e.to_string()))?;
        lock.try_lock()
            .map_err(|_| error("Another operator holds this publication book"))?;
        let path = root.join("publication.json");
        let state = if path.exists() {
            super::parse::<State>("publication.json", &workspace::read(&path, LIMIT)?, LIMIT)?
        } else {
            let state = State {
                schema: "verse.public.book.v1".into(),
                authority,
                revision: 1,
                publishers: BTreeMap::new(),
                entries: BTreeMap::new(),
            };
            workspace::create(
                &path,
                &serde_json::to_vec(&state).map_err(|e| error(e.to_string()))?,
            )?;
            workspace::sync_dir(root)?;
            state
        };
        if state.schema != "verse.public.book.v1"
            || state.authority != authority
            || state.revision == 0
            || state.publishers.len() > 256
            || state.entries.len() > 512
        {
            return Err(error("Publication book identity or budget differs"));
        }
        for (id, publisher) in &state.publishers {
            XOnlyPublicKey::from_byte_array(publisher.public)
                .map_err(|_| error("Invalid enrolled publisher"))?;
            if *id != publisher_id(publisher.public) || !text(&publisher.reason) {
                return Err(error("Invalid publisher record"));
            }
        }
        for (id, entry) in &state.entries {
            entry.submission.verify()?;
            if entry.submission.authority != authority
                || *id != entry_id(entry.submission.publisher, &entry.submission.artifact)
                || entry.revision == 0
                || !text(&entry.reason)
                || !state
                    .publishers
                    .contains_key(&publisher_id(entry.submission.publisher))
            {
                return Err(error("Invalid publication record"));
            }
        }
        Ok(Self {
            root: root.into(),
            _lock: lock,
            state,
            poisoned: false,
        })
    }
    fn healthy(&self) -> Result<()> {
        if self.poisoned {
            Err(error("Publication outcome is uncertain; reopen the book"))
        } else {
            Ok(())
        }
    }
    fn commit(&mut self, mut next: State) -> Result<()> {
        self.healthy()?;
        next.revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or_else(|| error("Publication revision exhausted"))?;
        let bytes = serde_json::to_vec(&next).map_err(|e| error(e.to_string()))?;
        if bytes.len() > LIMIT {
            return Err(error("Publication book byte budget exceeded"));
        }
        if let Err(error) = workspace::atomic(&self.root.join("publication.json"), &bytes) {
            self.poisoned = true;
            return Err(error);
        }
        self.state = next;
        Ok(())
    }
    /// Operator attests enrollment separately from the publisher's proof of possession.
    pub fn publisher(&mut self, public: [u8; 32], enabled: bool, reason: &str) -> Result<()> {
        self.healthy()?;
        XOnlyPublicKey::from_byte_array(public).map_err(|_| error("Invalid publisher key"))?;
        if !text(reason) {
            return Err(error("Supply a bounded printable enrollment reason"));
        }
        let id = publisher_id(public);
        if !self.state.publishers.contains_key(&id) && self.state.publishers.len() >= 256 {
            return Err(error("Publisher budget exceeded"));
        }
        let mut next = self.state.clone();
        next.publishers.insert(
            id,
            Publisher {
                public,
                enabled,
                reason: reason.into(),
            },
        );
        self.commit(next)
    }
    /// Signed submissions become pending; local builds and signatures cannot approve them.
    pub fn submit(
        &mut self,
        submission: &Submission,
        release: &release::Verified,
    ) -> Result<Entry> {
        self.healthy()?;
        submission.verify()?;
        if submission.authority != self.state.authority || submission.artifact != release.id() {
            return Err(error("Submission names another authority or artifact"));
        }
        if !self
            .state
            .publishers
            .get(&publisher_id(submission.publisher))
            .is_some_and(|p| p.enabled)
        {
            return Err(error("Publisher is not enrolled or is suspended"));
        }
        let id = entry_id(submission.publisher, &submission.artifact);
        if let Some(entry) = self.state.entries.get(&id) {
            return Ok(entry.clone());
        }
        if self.state.entries.len() >= 512 {
            return Err(error("Publication entry budget exceeded"));
        }
        let entry = Entry {
            submission: submission.clone(),
            status: Status::Pending,
            revision: 1,
            reason: "Awaiting operator distribution review".into(),
        };
        let mut next = self.state.clone();
        next.entries.insert(id, entry.clone());
        self.commit(next)?;
        Ok(entry)
    }
    /// Trusted operator review; revoked artifact identities cannot be resurrected.
    pub fn review(
        &mut self,
        publisher: [u8; 32],
        release: &release::Verified,
        expected: u64,
        status: Status,
        reason: &str,
    ) -> Result<Entry> {
        self.review_entry(publisher, release.id(), expected, status, reason)
    }
    /// Withdraw an enrolled artifact even when its files are damaged or absent.
    pub fn revoke(
        &mut self,
        publisher: [u8; 32],
        artifact: &str,
        expected: u64,
        reason: &str,
    ) -> Result<Entry> {
        self.review_entry(publisher, artifact, expected, Status::Revoked, reason)
    }
    fn review_entry(
        &mut self,
        publisher: [u8; 32],
        artifact: &str,
        expected: u64,
        status: Status,
        reason: &str,
    ) -> Result<Entry> {
        self.healthy()?;
        if !digest_valid(artifact) {
            return Err(error("Invalid artifact identity"));
        }
        if status == Status::Pending || !text(reason) {
            return Err(error("Supply an explicit bounded moderation decision"));
        }
        let id = entry_id(publisher, artifact);
        let mut entry = self
            .state
            .entries
            .get(&id)
            .cloned()
            .ok_or_else(|| error("Publication has not been submitted"))?;
        if entry.revision != expected || entry.status == Status::Revoked {
            return Err(error(
                "Publication revision is stale or permanently revoked",
            ));
        }
        if status == Status::Approved
            && !self
                .state
                .publishers
                .get(&publisher_id(publisher))
                .is_some_and(|p| p.enabled)
        {
            return Err(error("Publisher is suspended"));
        }
        entry.status = status;
        entry.revision = entry
            .revision
            .checked_add(1)
            .ok_or_else(|| error("Publication revision exhausted"))?;
        entry.reason = reason.into();
        let mut next = self.state.clone();
        next.entries.insert(id, entry.clone());
        self.commit(next)?;
        Ok(entry)
    }
    /// Recheck live approval, enrollment, compatibility, and all bytes on each resolve.
    pub fn resolve(
        &self,
        publisher: [u8; 32],
        artifact: &str,
        directory: &Path,
    ) -> Result<release::Verified> {
        self.healthy()?;
        if !self
            .state
            .publishers
            .get(&publisher_id(publisher))
            .is_some_and(|p| p.enabled)
            || !self
                .state
                .entries
                .get(&entry_id(publisher, artifact))
                .is_some_and(|e| e.status == Status::Approved)
        {
            return Err(error("Publication is unapproved, revoked, or suspended"));
        }
        let release = release::verify(directory)?;
        if release.id() != artifact {
            return Err(error("Publication bytes differ from the approved artifact"));
        }
        Ok(release)
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Revoke {
        publisher: [u8; 32],
        artifact: String,
        expected_revision: u64,
        reason: String,
    },
    Publisher {
        public: [u8; 32],
        enabled: bool,
        reason: String,
    },
    Submit {
        directory: PathBuf,
        submission: Submission,
    },
    Review {
        publisher: [u8; 32],
        directory: PathBuf,
        expected_revision: u64,
        status: Status,
        reason: String,
    },
    Resolve {
        publisher: [u8; 32],
        artifact: String,
        directory: PathBuf,
    },
}
/// Local operator command. Book paths are deployment authority, never world input.
pub fn run(args: &[std::ffi::OsString]) -> Result<()> {
    if args.len() != 3 {
        return Err(error(
            "Usage: verse-content publication BOOK AUTHORITY_HEX COMMAND_JSON",
        ));
    }
    let authority = args[1]
        .to_str()
        .ok_or_else(|| error("Authority must be hexadecimal"))?;
    if !digest_valid(authority) {
        return Err(error("Authority must be 32 bytes of lowercase hexadecimal"));
    }
    let mut id = [0; 32];
    for (i, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&authority[2 * i..2 * i + 2], 16)
            .map_err(|e| error(e.to_string()))?;
    }
    let command: Command = super::parse(
        "command.json",
        &workspace::read(Path::new(&args[2]), 64 * 1024)?,
        64 * 1024,
    )?;
    let mut book = Book::open(Path::new(&args[0]), id)?;
    let response = match command {
        Command::Revoke {
            publisher,
            artifact,
            expected_revision,
            reason,
        } => serde_json::to_value(book.revoke(publisher, &artifact, expected_revision, &reason)?)
            .map_err(|e| error(e.to_string()))?,
        Command::Publisher {
            public,
            enabled,
            reason,
        } => {
            book.publisher(public, enabled, &reason)?;
            serde_json::json!({"publisher":public,"enabled":enabled})
        }
        Command::Submit {
            directory,
            submission,
        } => serde_json::to_value(book.submit(&submission, &release::verify(&directory)?)?)
            .map_err(|e| error(e.to_string()))?,
        Command::Review {
            publisher,
            directory,
            expected_revision,
            status,
            reason,
        } => serde_json::to_value(book.review(
            publisher,
            &release::verify(&directory)?,
            expected_revision,
            status,
            &reason,
        )?)
        .map_err(|e| error(e.to_string()))?,
        Command::Resolve {
            publisher,
            artifact,
            directory,
        } => {
            let release = book.resolve(publisher, &artifact, &directory)?;
            serde_json::json!({"artifact":release.id(),"manifest":release.manifest()})
        }
    };
    println!("{}", response);
    Ok(())
}

#[cfg(test)]
mod tests;
