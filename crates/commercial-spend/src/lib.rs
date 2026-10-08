//! One private owner serializes commercial reservations and actual wallet effects.
pub mod authority;
mod private;
mod protocol;
mod refunds;
mod statements;
pub use refunds::FundingReversalReview;
pub use statements::StatementGrant;
pub mod wallet;
use commercial_accounts::NativeSources;
use openagents_wallet::{custody::Manifest, resident::RemoteWallet};
use pay_ledger::{
    Ledger,
    shared::{Binding, Owner, SCHEMA},
};
use private::Held;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
};
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Shared commercial authority is unavailable or changed; no native fallback is allowed."
    )]
    Denied,
    #[error("Shared funds cannot cover the exact admitted liability.")]
    Funds,
    #[error("The original shared operation requires read-only reconciliation.")]
    Unknown,
    #[error("Shared state could not be retained.")]
    Io(#[from] std::io::Error),
    #[error("Shared ledger refused the operation.")]
    Ledger(#[from] pay_ledger::Error),
    #[error("Invalid bounded shared document.")]
    Json(#[from] serde_json::Error),
    #[error("Native account custody changed.")]
    Accounts(#[from] tenancy::accounts::Trouble),
    #[error("Custodian did not confirm the original operation.")]
    Wallet(#[from] openagents_wallet::WalletError),
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |v| v.as_secs())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub ledger: PathBuf,
    pub origin: String,
    pub wallet_home: PathBuf,
    pub socket: PathBuf,
    pub writer_file: PathBuf,
    pub canonical_directory: PathBuf,
    pub commercial: commercial_accounts::Config,
    pub grants: Vec<Grant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refunds: Vec<pay_ledger::shared::RefundReview>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding_reversals: Vec<FundingReversalReview>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statements: Vec<StatementGrant>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub binding: Binding,
    pub credential_digest: String,
    pub native_credential_file: PathBuf,
    pub native: Native,
    pub reviewed_at: u64,
    pub valid_until: u64,
    #[serde(default)]
    pub previous_binding: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Native {
    Retail {
        principal: String,
        generation: i64,
    },
    Tenancy {
        directory: PathBuf,
        principal: String,
        tenant: String,
        member_epoch: u64,
        members_epoch: u64,
        #[serde(default)]
        money_ledger: Option<PathBuf>,
        #[serde(default)]
        buyer_root: Option<PathBuf>,
    },
}
/// The original SQLite writer, private review, and native credentials remain held.
pub struct Controller {
    pub config: Config,
    pub(crate) policy: Held,
    pub(crate) ledger_custody: Held,
    pub(crate) ledger: Mutex<Ledger>,
    pub(crate) writer: Held,
    pub(crate) commercial: NativeSources,
    pub(crate) native_credentials: BTreeMap<String, Held>,
    pub(crate) native_books: BTreeMap<String, Held>,
    pub(crate) buyer_directories: BTreeMap<String, File>,
    pub(crate) wallet: RemoteWallet,
    _lock: File,
}
impl Controller {
    pub fn open(path: &Path) -> Result<Self> {
        let policy = Held::open(path, true)?;
        let config: Config = serde_json::from_slice(&policy.bytes(256 * 1024)?)?;
        if config.schema != SCHEMA
            || config.grants.is_empty()
            || config.grants.len() > 128
            || !config.socket.is_absolute()
            || config.refunds.len() > 128
            || config.funding_reversals.len() > 128
            || config.statements.len() > 128
        {
            return Err(Error::Denied);
        }
        let mut refund_ids = BTreeSet::new();
        let mut refund_evidence = BTreeSet::new();
        for review in &config.refunds {
            if !refund_ids.insert(&review.id)
                || !refund_evidence.insert(&review.evidence)
                || review.units == 0
                || review.valid_until <= review.reviewed_at
                || review.id.is_empty()
                || review.id.len() > 200
                || review.evidence.is_empty()
                || review.evidence.len() > 200
                || review.intent_digest.len() != 64
            {
                return Err(Error::Denied);
            }
        }
        let mut reversal_ids = BTreeSet::new();
        let mut reversal_evidence = BTreeSet::new();
        for r in &config.funding_reversals {
            if !reversal_ids.insert(&r.id)
                || !reversal_evidence.insert(&r.evidence)
                || r.id.is_empty()
                || r.id.len() > 200
                || r.funding.is_empty()
                || r.funding.len() > 200
                || r.evidence.is_empty()
                || r.evidence.len() > 200
                || r.funding_digest.len() != 64
                || !r.funding_digest.bytes().all(|b| b.is_ascii_hexdigit())
                || r.amount_msat <= 0
                || r.amount_msat > 1_000_000_000
                || r.valid_until <= r.reviewed_at
            {
                return Err(Error::Denied);
            }
        }
        let ledger_custody = Held::open(&config.ledger, false)?;
        let lock_path = config.ledger.with_extension("shared-owner.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)?;
        let m = lock.metadata()?;
        if !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o077 != 0
            || m.uid() != unsafe { libc::geteuid() }
            || unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
        {
            return Err(Error::Denied);
        }
        let writer = Held::open(&config.writer_file, true)?;
        let secret = writer.token()?;
        if secret.len() != 64 || hex::decode(&secret).is_err() {
            return Err(Error::Denied);
        }
        let mut ledger = Ledger::open(&config.ledger)?;
        if ledger.origin()? != config.origin {
            return Err(Error::Denied);
        }
        let wallet = RemoteWallet::probe(&config.wallet_home).ok_or(Error::Denied)?;
        let node = wallet.bound_payment_identity()?;
        let physical = ledger_custody.file.metadata()?;
        let owner = Owner {
            schema: SCHEMA.into(),
            origin: config.origin.clone(),
            node: node.clone(),
            socket: config.socket.clone(),
            file_device: physical.dev(),
            file_inode: physical.ino(),
            writer: digest(secret.as_bytes()),
        };
        ledger.install_shared_owner(&owner)?;
        ledger.admit_shared_writer(&secret)?;
        let commercial = NativeSources::open(&config.canonical_directory, &config.commercial)
            .map_err(|_| Error::Denied)?;
        let mut native_credentials = BTreeMap::new();
        let mut native_books = BTreeMap::new();
        let mut buyer_directories = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for g in &config.grants {
            g.binding.validate()?;
            if !ids.insert(&g.binding.id)
                || g.binding.ledger_origin != owner.origin
                || g.binding.custodian_node != owner.node
                || g.credential_digest.len() != 64
                || g.valid_until <= g.reviewed_at
            {
                return Err(Error::Denied);
            }
            native_credentials.insert(
                g.binding.id.clone(),
                Held::open(&g.native_credential_file, true)?,
            );
            if g.binding.source.product == receipts::purchase::CommercialProduct::Plugin {
                let Native::Tenancy {
                    buyer_root: Some(root),
                    ..
                } = &g.native
                else {
                    return Err(Error::Denied);
                };
                let dir = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
                    .open(root)?;
                let meta = dir.metadata()?;
                if !root.is_absolute()
                    || !meta.is_dir()
                    || meta.mode() & 0o077 != 0
                    || meta.uid() != unsafe { libc::geteuid() }
                {
                    return Err(Error::Denied);
                }
                buyer_directories.insert(g.binding.id.clone(), dir);
            }
            if let Native::Tenancy { directory, .. } = &g.native {
                native_books.insert(
                    g.binding.id.clone(),
                    Held::open(&directory.join("keys.json"), false)?,
                );
            }
        }
        let controller = Self {
            config,
            policy,
            ledger_custody,
            ledger: Mutex::new(ledger),
            writer,
            commercial,
            native_credentials,
            native_books,
            buyer_directories,
            wallet,
            _lock: lock,
        };
        // Activation cannot import legacy balances. Every original native grant is
        // checked before custody is installed; partial activation remains disabled.
        for g in &controller.config.grants {
            let existing = controller
                .ledger
                .lock()
                .map_err(|_| Error::Denied)?
                .shared_binding(&g.binding.id)?;
            if existing.as_ref().is_some_and(|b| b != &g.binding) {
                return Err(Error::Denied);
            }
            controller.current(g, existing.is_none())?;
        }
        controller.wallet.activate_shared_custody(Manifest {
            schema: openagents_wallet::custody::SCHEMA.into(),
            origin: owner.origin,
            node: owner.node,
            controller: owner.socket,
            writer_digest: owner.writer,
        })?;
        {
            let mut ledger = controller.ledger.lock().map_err(|_| Error::Denied)?;
            for g in &controller.config.grants {
                controller.activate_native(g)?;
                if ledger.shared_binding(&g.binding.id)?.is_none() {
                    controller.current(g, true)?;
                    if let Some(previous) = &g.previous_binding {
                        ledger.migrate_shared(&g.binding, previous)?;
                    } else {
                        ledger.activate_shared(&g.binding)?;
                    }
                }
                if ledger.shared_head(&g.binding)?.as_ref() != Some(&g.binding) {
                    return Err(Error::Denied);
                }
            }
        }
        controller.ledger_custody.check()?;
        Ok(controller)
    }
    fn activate_native(&self, grant: &Grant) -> Result<()> {
        if grant.binding.source.product == receipts::purchase::CommercialProduct::Gateway {
            let Native::Tenancy {
                directory,
                money_ledger: Some(path),
                ..
            } = &grant.native
            else {
                return Err(Error::Denied);
            };
            let workspace = grant
                .binding
                .source
                .workspace
                .as_deref()
                .ok_or(Error::Denied)?;
            let mode = grant.binding.mode();
            let marker = tenancy::money::shared::required(directory, workspace)
                .map_err(|_| Error::Denied)?;
            let mut native = tenancy::money::Ledger::open(path).map_err(|_| Error::Denied)?;
            let original = native.shared_mode(workspace).cloned();
            if let Some(old) = original {
                if old != mode {
                    if !old.same_native(&mode)
                        || grant.previous_binding.as_deref() != Some(&old.binding_digest)
                    {
                        return Err(Error::Denied);
                    }
                    if marker.as_ref().is_some_and(|m| m != &old && m != &mode) {
                        return Err(Error::Denied);
                    }
                    if marker.is_none() {
                        tenancy::money::shared::install_marker(directory, &old)
                            .map_err(|_| Error::Denied)?;
                    }
                    native
                        .migrate_shared(workspace, &old.binding_digest, mode.clone())
                        .map_err(|_| Error::Denied)?;
                }
                if let Some(old_marker) = tenancy::money::shared::required(directory, workspace)
                    .map_err(|_| Error::Denied)?
                {
                    if old_marker != mode {
                        if !old_marker.same_native(&mode)
                            || grant.previous_binding.as_deref() != Some(&old_marker.binding_digest)
                        {
                            return Err(Error::Denied);
                        }
                        tenancy::money::shared::migrate_marker(directory, &old_marker, &mode)
                            .map_err(|_| Error::Denied)?;
                    }
                } else {
                    tenancy::money::shared::install_marker(directory, &mode)
                        .map_err(|_| Error::Denied)?;
                }
            } else {
                if marker.as_ref().is_some_and(|m| m != &mode) {
                    return Err(Error::Denied);
                }
                native
                    .shared_activation_empty(workspace)
                    .map_err(|_| Error::Denied)?;
                tenancy::money::shared::install_marker(directory, &mode)
                    .map_err(|_| Error::Denied)?;
                native
                    .install_shared(workspace, mode)
                    .map_err(|_| Error::Denied)?;
            }
        }
        Ok(())
    }
    pub(crate) fn grant(&self, id: &str) -> Result<&Grant> {
        self.config
            .grants
            .iter()
            .find(|g| g.binding.id == id)
            .ok_or(Error::Denied)
    }
    pub(crate) fn original_grant(&self, binding: &Binding) -> Result<&Grant> {
        self.config
            .grants
            .iter()
            .find(|g| g.binding.mode().same_native(&binding.mode()))
            .ok_or(Error::Denied)
    }
}
