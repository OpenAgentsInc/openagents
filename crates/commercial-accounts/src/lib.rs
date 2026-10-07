//! Current native product authorization under an explicit private operator policy.
//! Attribution grants no spending, host access, execution, or payout authority.
use pay_ledger::{
    Ledger,
    compute::{Need, credential_digest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
};
use tenancy::{
    Accounts, Registry, Role,
    accounts::commercial::{Product, Source, SourceAuthority, Sources},
};

#[cfg(feature = "merchant-commissions")]
pub mod commission;

pub const SCHEMA: &str = "openagents.commercial-policy.v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub policy: PathBuf,
    pub stores: Vec<NativeStore>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NativeStore {
    Tenancy { issuer: String, directory: PathBuf },
    Retail { issuer: String, ledger: PathBuf },
}
/// Each entry is a local operator decision about exact native identities. A
/// client cannot supply entries, credentials, policy booleans, or store paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub operator: String,
    pub entries: Vec<Entry>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub operator: String,
    pub source: Source,
    pub customer: String,
    pub workspace: String,
    pub canonical_owner: String,
    pub canonical_owner_epoch: u64,
    pub canonical_members_epoch: u64,
    pub principal: String,
    /// An exact current native credential, kept in a separate private file.
    pub credential_file: PathBuf,
    pub generation: u64,
    /// Tenancy sources also pin their current native owner and epochs.
    pub native_owner: Option<String>,
    pub native_owner_epoch: Option<u64>,
    pub native_members_epoch: Option<u64>,
    pub reviewed_at: u64,
    pub valid_until: u64,
    pub previous_authority: Option<String>,
}
impl Entry {
    pub fn digest(&self) -> String {
        // Struct field order is fixed by this versioned schema. The digest is
        // a local review identity, not a signature or remote attestation.
        format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(self).expect("entry serializes"))
        )
    }
}
struct Held {
    path: PathBuf,
    directory: File,
    file: File,
}
fn private(meta: &std::fs::Metadata, directory: bool) -> bool {
    // SAFETY: geteuid has no arguments and does not mutate process state.
    meta.uid() == unsafe { libc::geteuid() }
        && meta.mode() & 0o077 == 0
        && if directory {
            meta.is_dir()
        } else {
            meta.is_file() && meta.nlink() == 1
        }
}
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
impl Held {
    fn open(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("An explicit absolute private source path is required.".into());
        }
        let parent = path
            .parent()
            .ok_or("Explicit private source path required.")?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent)
            .map_err(|_| "Private source directory unavailable.")?;
        if !private(
            &directory
                .metadata()
                .map_err(|_| "Private directory metadata unavailable.")?,
            true,
        ) {
            return Err(
                "Source directory must already be private and owned by this operator.".into(),
            );
        }
        let name = std::ffi::CString::new(
            path.file_name()
                .ok_or("Explicit source file required.")?
                .as_encoded_bytes(),
        )
        .map_err(|_| "Invalid source filename.")?;
        // SAFETY: the held directory owns the live descriptor and name is terminated.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err("Private source file unavailable or linked.".into());
        }
        use std::os::fd::FromRawFd;
        // SAFETY: openat returned a new exclusively owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        let held = Self {
            path: path.into(),
            directory,
            file,
        };
        held.check()?;
        Ok(held)
    }
    fn check(&self) -> Result<(), String> {
        let dir = self
            .directory
            .metadata()
            .map_err(|_| "Private source custody unavailable.")?;
        let current_dir = std::fs::symlink_metadata(self.path.parent().unwrap())
            .map_err(|_| "Private source directory replaced.")?;
        let file = self
            .file
            .metadata()
            .map_err(|_| "Private source custody unavailable.")?;
        let current_file =
            std::fs::symlink_metadata(&self.path).map_err(|_| "Private source file replaced.")?;
        if !private(&dir, true)
            || !private(&current_dir, true)
            || !same(&dir, &current_dir)
            || !private(&file, false)
            || !private(&current_file, false)
            || !same(&file, &current_file)
        {
            return Err("Private native source custody changed.".into());
        }
        Ok(())
    }
    fn bytes(&self, max: usize) -> Result<Vec<u8>, String> {
        self.check()?;
        if self
            .file
            .metadata()
            .map_err(|_| "Private source metadata unavailable.")?
            .len()
            > max as u64
        {
            return Err("Private source exceeds its bound.".into());
        }
        // Reopen the original held descriptor through positional reads, so two
        // simultaneous selections never share a mutable seek offset.
        use std::os::unix::fs::FileExt;
        let mut bytes = vec![0; max + 1];
        let mut n = 0;
        while n < bytes.len() {
            let got = self
                .file
                .read_at(&mut bytes[n..], n as u64)
                .map_err(|_| "Private source read failed.")?;
            if got == 0 {
                break;
            }
            n += got;
        }
        if n > max {
            return Err("Private source grew beyond its bound.".into());
        }
        bytes.truncate(n);
        self.check()?;
        Ok(bytes)
    }
}
struct NativeHeld {
    store: NativeStore,
    files: Vec<Held>,
}
pub struct NativeSources {
    canonical: Accounts,
    canonical_path: PathBuf,
    canonical_directory: File,
    policy: Held,
    stores: Vec<NativeHeld>,
}
impl NativeSources {
    /// Establish original custody once for this configured operator. Atomic
    /// file replacement requires reopening the adapter; a changed mapping
    /// always requires an exact new binding review.
    pub fn open(canonical_directory: &Path, config: &Config) -> Result<Self, String> {
        if !canonical_directory.is_absolute() {
            return Err("An explicit absolute canonical directory is required.".into());
        }
        let canonical_file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(canonical_directory)
            .map_err(|_| "Canonical account directory unavailable.")?;
        if !private(
            &canonical_file
                .metadata()
                .map_err(|_| "Canonical directory metadata unavailable.")?,
            true,
        ) {
            return Err("Canonical account directory must already be private.".into());
        }
        let canonical =
            Accounts::open(canonical_directory).map_err(|_| "Canonical accounts unavailable.")?;
        if config.stores.is_empty() || config.stores.len() > 16 {
            return Err("Bounded native stores required.".into());
        }
        let policy = Held::open(&config.policy)?;
        let mut stores = Vec::new();
        let mut issuers = BTreeSet::new();
        let mut native_books = BTreeSet::new();
        for store in &config.stores {
            let (retail, issuer, paths) = match store {
                NativeStore::Tenancy { issuer, directory } => (
                    false,
                    issuer,
                    vec![
                        directory.join("accounts.json"),
                        directory.join("keys.json"),
                        directory.join("registry.json"),
                    ],
                ),
                NativeStore::Retail { issuer, ledger } => (true, issuer, vec![ledger.clone()]),
            };
            if issuer.is_empty() || issuer.len() > 128 || !issuers.insert((retail, issuer.clone()))
            {
                return Err("Native issuer selection is ambiguous.".into());
            }
            // Tenancy seals replace accounts.json atomically. Keep its private
            // directory custody; current native readers verify the sealed book.
            let paths = if retail {
                paths
            } else {
                vec![paths[1].clone()]
            };
            let files = paths
                .iter()
                .map(|p| Held::open(p))
                .collect::<Result<Vec<_>, _>>()?;
            let identity = if retail {
                files[0].file.metadata()
            } else {
                files[0].directory.metadata()
            }
            .map_err(|_| "Native book identity is unavailable.")?;
            if !native_books.insert((retail, identity.dev(), identity.ino())) {
                return Err("One native book cannot have conflicting configured issuers.".into());
            }
            stores.push(NativeHeld {
                store: store.clone(),
                files,
            });
        }
        Ok(Self {
            canonical,
            canonical_path: canonical_directory.into(),
            canonical_directory: canonical_file,
            policy,
            stores,
        })
    }
    fn check_canonical(&self) -> Result<(), String> {
        let original = self
            .canonical_directory
            .metadata()
            .map_err(|_| "Canonical directory custody unavailable.")?;
        let current = std::fs::symlink_metadata(&self.canonical_path)
            .map_err(|_| "Canonical directory replaced.")?;
        if !private(&original, true) || !private(&current, true) || !same(&original, &current) {
            return Err("Canonical directory custody changed.".into());
        }
        Ok(())
    }
    /// The caller constructs this exact source only after native account
    /// authentication. This projection grants no access to another source,
    /// account, balance, artifact, execution, or financial record.
    pub fn selection(
        &self,
        source: &Source,
    ) -> Result<Option<tenancy::accounts::commercial::Revision>, String> {
        self.check_canonical()?;
        let store = self
            .canonical
            .store()
            .map_err(|_| "Canonical commercial history unavailable.")?;
        let mut matching = store
            .commercial
            .bindings
            .values()
            .filter_map(|history| history.last())
            .filter(|revision| {
                revision.active && revision.sources.iter().any(|proof| &proof.source == source)
            });
        let Some(revision) = matching.next() else {
            return Ok(None);
        };
        if matching.next().is_some() {
            return Err("Native commercial selection is ambiguous.".into());
        }
        let member = self
            .canonical
            .authorize(&revision.workspace, &revision.customer)
            .map_err(|_| "Canonical commercial customer is no longer authorized.")?;
        self.canonical
            .commercial_selection(&member, source, self)
            .map_err(|_| {
                "Current commercial mapping is unavailable; review its native lineage.".to_owned()
            })
    }

    fn entry(&self, source: &Source, customer: &str, workspace: &str) -> Result<Entry, String> {
        self.check_canonical()?;
        let policy: Policy = serde_json::from_slice(&self.policy.bytes(128 * 1024)?)
            .map_err(|_| "Invalid private commercial policy.")?;
        if policy.schema != SCHEMA
            || policy.operator.is_empty()
            || policy.operator.len() > 128
            || policy.entries.is_empty()
            || policy.entries.len() > 128
        {
            return Err("Unsupported or unbounded commercial policy.".into());
        }
        let mut unique = BTreeSet::new();
        for e in &policy.entries {
            if !unique.insert(&e.source) {
                return Err("Conflicting commercial policy sources.".into());
            }
        }
        let entry = policy
            .entries
            .into_iter()
            .find(|e| &e.source == source && e.customer == customer && e.workspace == workspace)
            .ok_or("No current operator-reviewed native mapping.")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Clock unavailable.")?
            .as_secs();
        if entry.operator != policy.operator
            || entry.operator.chars().any(char::is_control)
            || entry.generation == 0
            || entry.reviewed_at > now
            || now >= entry.valid_until
            || entry.valid_until <= entry.reviewed_at
        {
            return Err("Native mapping review is expired or invalid.".into());
        }
        let owner = self
            .canonical
            .authorize(workspace, &entry.canonical_owner)
            .map_err(|_| "Canonical mapping owner is no longer authorized.")?;
        if owner.role != Role::Owner
            || owner.epoch != entry.canonical_owner_epoch
            || owner.members_epoch != entry.canonical_members_epoch
        {
            return Err("Canonical owner or membership changed; review mapping again.".into());
        }
        self.canonical
            .authorize(workspace, customer)
            .map_err(|_| "Canonical customer is no longer a workspace member.")?;
        Ok(entry)
    }
}
impl Sources for NativeSources {
    fn authorize(
        &self,
        source: &Source,
        customer: &str,
        workspace: &str,
    ) -> Result<SourceAuthority, String> {
        let entry = self.entry(source, customer, workspace)?;
        let native = self
            .stores
            .iter()
            .find(|n| match &n.store {
                NativeStore::Tenancy { issuer, .. } => {
                    source.product != Product::Retail && issuer == &source.issuer
                }
                NativeStore::Retail { issuer, .. } => {
                    source.product == Product::Retail && issuer == &source.issuer
                }
            })
            .ok_or("Native issuer is not configured.")?;
        for held in &native.files {
            held.check()?;
        }
        let credential = Held::open(&entry.credential_file)?;
        let bytes = credential.bytes(4096)?;
        let token = std::str::from_utf8(&bytes)
            .map_err(|_| "Invalid native credential file.")?
            .trim();
        if token.is_empty() {
            return Err("Native credential is missing.".into());
        }
        let native_identity = match &native.store {
            NativeStore::Tenancy { directory, .. } => {
                let registry =
                    Registry::open(directory).map_err(|_| "Native registry unavailable.")?;
                let source_workspace = source
                    .workspace
                    .as_deref()
                    .ok_or("Tenancy source requires an exact workspace.")?;
                let accounts =
                    Accounts::open(directory).map_err(|_| "Native accounts unavailable.")?;
                let member = accounts
                    .authenticate_key(registry.manifest(), source_workspace, token)
                    .map_err(|_| "Native account credential is no longer authorized.")?;
                let key = tenancy::keys::authenticate(directory, registry.manifest(), token)
                    .map_err(|_| "Native account credential is no longer authorized.")?;
                let source_member = accounts
                    .authorize(source_workspace, &source.account)
                    .map_err(|_| "Native source member is no longer authorized.")?;
                if source_member.account != source.account
                    || source_member.members_epoch != member.members_epoch
                    || Some(source_member.members_epoch) != entry.native_members_epoch
                    || member.role != Role::Owner
                    || entry.native_owner.as_deref() != Some(member.account.as_str())
                    || entry.native_owner_epoch != Some(member.epoch)
                    || entry.native_members_epoch != Some(member.members_epoch)
                    || entry.principal != format!("key:{}", key.key_id)
                {
                    return Err(
                        "Native source owner, member, principal, or membership changed.".into(),
                    );
                }
                native_identity("tenancy", "", &source.account, source.workspace.as_deref())
            }
            NativeStore::Retail { ledger, .. } => {
                if source.workspace.is_some()
                    || entry.native_owner.is_some()
                    || entry.native_owner_epoch.is_some()
                    || entry.native_members_epoch.is_some()
                {
                    return Err("Retail source does not establish tenancy ownership.".into());
                }
                let book = Ledger::open_read_only(ledger)
                    .map_err(|_| "Native retail ledger unavailable.")?;
                let current = book
                    .resolve_principal(&entry.principal, &credential_digest(token), Need::Read)
                    .map_err(|_| "Native retail credential is no longer authorized.")?;
                if current.account != source.account
                    || u64::try_from(current.generation).ok() != Some(entry.generation)
                {
                    return Err("Native retail account or credential generation changed.".into());
                }
                native_identity(
                    "retail",
                    &book
                        .origin()
                        .map_err(|_| "Native retail origin unavailable.")?,
                    &source.account,
                    None,
                )
            }
        };
        credential.check()?;
        for held in &native.files {
            held.check()?;
        }
        self.policy.check()?;
        // Native reads can wait for a writer. Recheck the exact reviewed policy,
        // current canonical membership, expiry, and credential after that wait.
        if self.entry(source, customer, workspace)?.digest() != entry.digest()
            || credential.bytes(4096)? != bytes
        {
            return Err(
                "Native mapping review or credential changed during authentication.".into(),
            );
        }
        Ok(SourceAuthority {
            source: source.clone(),
            native_identity,
            principal: entry.principal.clone(),
            generation: entry.generation,
            policy_digest: entry.digest(),
            previous_authority: entry.previous_authority.clone(),
        })
    }
}

fn native_identity(family: &str, origin: &str, account: &str, workspace: Option<&str>) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(
            serde_json::to_vec(&(family, origin, account, workspace))
                .expect("native identity serializes")
        )
    )
}

#[cfg(test)]
mod tests;
