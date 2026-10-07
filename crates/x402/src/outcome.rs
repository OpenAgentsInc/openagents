//! Private paid-plugin custody. Recovery reads never admit another execution.
use crate::{
    front::{Quote, Settlement},
    replay::ReplayEntry,
    server::Response,
    wire::SettlementResponse,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const AUTHORIZATION: &str = "OpenAgents-Recovery-Authorization";
pub const PAYMENT: &str = "OpenAgents-Recovery-Payment";
pub const SCHEMA: &str = "openagents.plugin-outcome.v1";
const MAX_RECORD: usize = 256 * 1024;

pub fn token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(all(test, unix))]
#[path = "outcome/tests.rs"]
mod tests;
pub fn commitment(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub network: String,
    pub payment_hash: String,
    pub invoice: String,
    pub request_hash: String,
    pub authorization: String,
    pub quote: Quote,
}
impl Identity {
    pub fn receipt_reference(&self) -> String {
        format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                serde_json::to_vec(self).expect("identity serializes")
            ))
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    Prepared,
    SettlementPending,
    Settled,
    Invoking,
    Completed,
    Failed,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    schema: String,
    pub identity: Identity,
    pub replay: ReplayEntry,
    pub settlement: Settlement,
    pub payment: SettlementResponse,
    pub stage: Stage,
    #[serde(with = "response_codec")]
    pub response: Option<Response>,
}
/// A private provider claim with stable purchase identity and bounded result.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub schema: String,
    pub identity: Identity,
    pub stage: Stage,
    pub settlement: Option<SettlementResponse>,
    #[serde(with = "response_codec")]
    pub response: Option<Response>,
    pub receipt_reference: String,
    pub guidance: String,
}
mod response_codec {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Encoded {
        status: u16,
        headers: Vec<(String, String)>,
        body_base64: String,
    }
    pub fn serialize<S: serde::Serializer>(
        value: &Option<Response>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .as_ref()
            .map(|r| Encoded {
                status: r.status,
                headers: r.headers.clone(),
                body_base64: STANDARD.encode(&r.body),
            })
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Response>, D::Error> {
        Option::<Encoded>::deserialize(deserializer)?
            .map(|r| {
                if r.body_base64.len() > (128 * 1024usize).div_ceil(3) * 4
                    || r.headers.len() > 16
                    || r.headers
                        .iter()
                        .any(|(n, v)| n.len() > 128 || v.len() > 4096)
                {
                    return Err(serde::de::Error::custom(
                        "Retained response exceeds its bound.",
                    ));
                }
                let body = STANDARD
                    .decode(r.body_base64)
                    .map_err(|_| serde::de::Error::custom("Invalid retained response encoding."))?;
                if body.len() > 128 * 1024 {
                    return Err(serde::de::Error::custom(
                        "Retained response exceeds its bound.",
                    ));
                }
                Ok(Response {
                    status: r.status,
                    headers: r.headers,
                    body,
                })
            })
            .transpose()
    }
}
impl Record {
    pub fn view(&self) -> View {
        View {
            schema: SCHEMA.into(), identity: self.identity.clone(), stage: self.stage,
            settlement: matches!(self.stage, Stage::Settled | Stage::Invoking | Stage::Completed | Stage::Failed).then(|| self.payment.clone()),
            response: self.response.clone(),
            receipt_reference: self.identity.receipt_reference(),
            guidance: match self.stage {
                Stage::Completed => "Payment and retained delivery are known. This is a provider execution claim, not independent quality verification.",
                Stage::Failed => "Payment settled but delivery failed. Keep this receipt for support; any reversal or new purchase needs separate authorization.",
                Stage::Settled => "Payment settled and no invocation was admitted. Keep this receipt for support; recovery does not authorize execution, refund, or a new purchase.",
                _ => "The original payment or invocation remains unresolved. Do not pay again, reexecute, release liability, or infer zero cost or a refund.",
            }.into(),
        }
    }
    fn check(&self) -> Result<(), String> {
        let i = &self.identity;
        let invoice = nostr::x402::decode_invoice(&i.invoice)
            .map_err(|_| "Retained outcome invoice is invalid.")?;
        if self.schema != SCHEMA
            || i.invoice.len() > 16 * 1024
            || hex::encode(invoice.payment_hash()) != i.payment_hash
            || hex::encode(invoice.description_hash()) != i.request_hash
            || invoice.amount_msat() != i.quote.price_msat
            || invoice.currency()
                != match i.network.as_str() {
                    nostr::x402::MAINNET => "bc",
                    nostr::x402::TESTNET => "tb",
                    _ => "invalid",
                }
            || !token(&i.payment_hash)
            || !token(&i.request_hash)
            || !token(&i.authorization)
            || self.replay.key != format!("{}:{}", i.network, i.payment_hash)
            || self.replay.network != i.network
            || self.replay.payment_hash != i.payment_hash
            || self.replay.amount_msat != i.quote.price_msat
            || self.replay.purchase != format!("{}:{}", self.settlement.route, i.request_hash)
            || self.settlement.network != i.network
            || self.settlement.payment_hash != i.payment_hash
            || self.settlement.request_hash != i.request_hash
            || self.settlement.price_msat != i.quote.price_msat
            || self.settlement.release != i.quote.release
            || self.settlement.author != i.quote.author
            || self.settlement.plugin != i.quote.plugin
            || self.settlement.fee_msat != i.quote.fee_msat
            || !self.payment.success
            || self.payment.transaction != i.payment_hash
            || self.payment.network != i.network
            || self.payment.amount.as_deref() != Some(i.quote.price_msat.to_string().as_str())
            || self.response.is_some() != matches!(self.stage, Stage::Completed | Stage::Failed)
            || self.response.as_ref().is_some_and(|r| {
                r.body.len() > 128 * 1024
                    || r.headers.len() > 16
                    || r.headers
                        .iter()
                        .any(|(n, v)| n.len() > 128 || v.len() > 4096)
                    || (self.stage == Stage::Completed) != (r.status == 200)
            })
        {
            return Err(
                "Paid-plugin outcome custody changed its pinned identity or result.".into(),
            );
        }
        Ok(())
    }
}
#[cfg(unix)]
pub use custody::Store;
#[cfg(unix)]
pub(crate) use custody::Transaction;
#[cfg(unix)]
mod custody {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
        path::{Path, PathBuf},
        sync::Arc,
    };
    /// One explicit private directory shared by every front for this receiver.
    #[derive(Clone)]
    pub struct Store {
        dir: Arc<File>,
        root: PathBuf,
    }
    struct Sealed {
        file: File,
        digest: [u8; 32],
    }
    #[derive(Serialize, Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct PurchaseBinding {
        request_hash: String,
        payment_hash: String,
        quote_reference: String,
    }
    impl Store {
        fn current(&self) -> Result<(), String> {
            let held = self
                .dir
                .metadata()
                .map_err(|_| "Outcome directory is unavailable.")?;
            let visible =
                std::fs::symlink_metadata(&self.root).map_err(|_| "Outcome directory changed.")?;
            if !visible.is_dir()
                || visible.dev() != held.dev()
                || visible.ino() != held.ino()
                || visible.uid() != unsafe { libc::geteuid() }
                || visible.mode() & 0o077 != 0
            {
                return Err("Outcome directory changed.".into());
            }
            Ok(())
        }
        fn same_file(&self, name: &str, held: &File) -> Result<(), String> {
            self.current()?;
            let current = self.file(name, libc::O_RDONLY)?;
            let visible = current
                .metadata()
                .map_err(|_| "Outcome source is unavailable.")?;
            let prior = held
                .metadata()
                .map_err(|_| "Outcome source is unavailable.")?;
            if visible.dev() != prior.dev() || visible.ino() != prior.ino() {
                return Err("Outcome source identity changed.".into());
            }
            Ok(())
        }
        fn sealed(&self, name: &str) -> Result<(Vec<u8>, Sealed), String> {
            self.current()?;
            let mut file = self.file(name, libc::O_RDONLY)?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(MAX_RECORD as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Outcome source is unavailable.")?;
            if bytes.len() > MAX_RECORD {
                return Err("Outcome source exceeds its bound.".into());
            }
            let digest = Sha256::digest(&bytes).into();
            self.same_file(name, &file)?;
            Ok((bytes, Sealed { file, digest }))
        }
        fn check_sealed(&self, name: &str, sealed: &Sealed) -> Result<(), String> {
            self.same_file(name, &sealed.file)?;
            if self.sealed(name)?.1.digest != sealed.digest {
                return Err("Outcome source bytes changed.".into());
            }
            Ok(())
        }
        fn source(identity: &Identity) -> String {
            format!(
                "purchase-{}",
                hex::encode(Sha256::digest(format!(
                    "{}:{}",
                    identity.network, identity.authorization
                )))
            )
        }
        fn binding(&self, source: &str, identity: &Identity) -> Result<Sealed, String> {
            let (bytes, sealed) = self.sealed(&format!("{source}.binding"))?;
            let old: PurchaseBinding = serde_json::from_slice(&bytes)
                .map_err(|_| "Original purchase binding is invalid.")?;
            if old
                != (PurchaseBinding {
                    request_hash: identity.request_hash.clone(),
                    payment_hash: identity.payment_hash.clone(),
                    quote_reference: crate::execution::quote_digest(&identity.quote),
                })
            {
                return Err("Original purchase binding changed.".into());
            }
            Ok(sealed)
        }
        pub(crate) fn bound(&self, network: &str, authorization: &str) -> Result<bool, String> {
            self.current()?;
            if !token(authorization) {
                return Err("Invalid purchase authorization commitment.".into());
            }
            let name = format!(
                "purchase-{}.binding",
                hex::encode(Sha256::digest(format!("{network}:{authorization}")))
            );
            let name = std::ffi::CString::new(name).unwrap();
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    self.dir.as_raw_fd(),
                    name.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
            {
                return Ok(true);
            }
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                Ok(false)
            } else {
                Err("Original purchase binding is unavailable.".into())
            }
        }
        pub fn open(root: &Path) -> Result<Self, String> {
            if !root.is_absolute() {
                return Err("Outcome custody requires an absolute private directory.".into());
            }
            if !root.exists() {
                let mut builder = std::fs::DirBuilder::new();
                use std::os::unix::fs::DirBuilderExt;
                builder
                    .mode(0o700)
                    .create(root)
                    .map_err(|_| "Outcome directory is unavailable.")?;
            }
            let dir = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
                .open(root)
                .map_err(|_| "Outcome directory is unavailable.")?;
            let metadata = dir
                .metadata()
                .map_err(|_| "Outcome directory is unavailable.")?;
            if metadata.permissions().mode() & 0o077 != 0
                || metadata.uid() != unsafe { libc::geteuid() }
            {
                return Err("Outcome custody must be private.".into());
            }
            Ok(Self {
                dir: Arc::new(dir),
                root: root.to_path_buf(),
            })
        }
        fn file(&self, name: &str, flags: i32) -> Result<File, String> {
            let name = std::ffi::CString::new(name).map_err(|_| "Invalid outcome path.")?;
            let fd = unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    name.as_ptr(),
                    flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err("Outcome custody is unavailable.".into());
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let m = file
                .metadata()
                .map_err(|_| "Outcome custody is unavailable.")?;
            if !m.is_file()
                || m.uid() != unsafe { libc::geteuid() }
                || m.nlink() != 1
                || m.permissions().mode() & 0o077 != 0
                || m.len() > MAX_RECORD as u64
            {
                return Err("Outcome custody is not a bounded private file.".into());
            }
            Ok(file)
        }
        fn name(network: &str, hash: &str) -> Result<String, String> {
            if !token(hash) || !matches!(network, nostr::x402::MAINNET | nostr::x402::TESTNET) {
                return Err("Invalid outcome identity.".into());
            }
            Ok(hex::encode(Sha256::digest(format!("{network}:{hash}"))))
        }
        fn lock(&self, name: &str) -> Result<File, String> {
            self.current()?;
            let lock = self.file(&format!("{name}.lock"), libc::O_RDWR | libc::O_CREAT)?;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(
                    "This purchase is being processed; recover its original identity later.".into(),
                );
            }
            self.same_file(&format!("{name}.lock"), &lock)?;
            Ok(lock)
        }
        fn read(&self, name: &str) -> Result<(Record, Sealed), String> {
            let (bytes, sealed) = self.sealed(&format!("{name}.json"))?;
            let record: Record =
                serde_json::from_slice(&bytes).map_err(|_| "Outcome custody is invalid.")?;
            record.check()?;
            Ok((record, sealed))
        }
        pub(crate) fn prepare(
            &self,
            identity: Identity,
            replay: ReplayEntry,
            settlement: Settlement,
            payment: SettlementResponse,
        ) -> Result<Transaction, String> {
            let name = Self::name(&identity.network, &identity.payment_hash)?;
            let source = Self::source(&identity);
            let source_lock = self.lock(&source)?;
            let binding = PurchaseBinding {
                request_hash: identity.request_hash.clone(),
                payment_hash: identity.payment_hash.clone(),
                quote_reference: crate::execution::quote_digest(&identity.quote),
            };
            let source_file = format!("{source}.binding");
            let c_source = std::ffi::CString::new(source_file.clone()).unwrap();
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    self.dir.as_raw_fd(),
                    c_source.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
            {
                let mut bytes = Vec::new();
                self.file(&source_file, libc::O_RDONLY)?
                    .take(1025)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Original purchase binding is unavailable.")?;
                let old: PurchaseBinding = serde_json::from_slice(&bytes)
                    .map_err(|_| "Original purchase binding is invalid.")?;
                if old != binding {
                    return Err("Original purchase authorization is already bound to another invoice or request; no execution was admitted.".into());
                }
            } else {
                if std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
                    return Err("Original purchase binding is unavailable.".into());
                }
                let mut file =
                    self.file(&source_file, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
                file.write_all(
                    &serde_json::to_vec(&binding).map_err(|_| "Invalid purchase binding.")?,
                )
                .and_then(|()| file.sync_all())
                .map_err(|_| "Original purchase binding could not be persisted.")?;
                self.dir
                    .sync_all()
                    .map_err(|_| "Original purchase binding directory could not be synced.")?;
            }
            let lock = self.lock(&name)?;
            let binding = self.binding(&source, &identity)?;
            // A retained claim is never permission to invoke again, even with the original paid proof.
            let path = std::ffi::CString::new(format!("{name}.json")).unwrap();
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe {
                libc::fstatat(
                    self.dir.as_raw_fd(),
                    path.as_ptr(),
                    &mut stat,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
            {
                return Err("This purchase already has outcome custody; use authorized recovery without repayment or reexecution.".into());
            }
            if std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
                return Err("Outcome custody is unavailable.".into());
            }
            let mut tx = Transaction {
                store: self.clone(),
                name,
                _lock: lock,
                _source_lock: source_lock,
                source,
                binding,
                sealed: None,
                poisoned: false,
                record: Record {
                    schema: SCHEMA.into(),
                    identity,
                    replay,
                    settlement,
                    payment,
                    stage: Stage::Prepared,
                    response: None,
                },
            };
            tx.save()?;
            Ok(tx)
        }
        pub(crate) fn recover(
            &self,
            network: &str,
            hash: &str,
            request_hash: &str,
            secret: &str,
        ) -> Result<Transaction, String> {
            if !token(secret) {
                return Err("Original purchase authorization is required.".into());
            }
            let name = Self::name(network, hash)?;
            // Authenticate an existing sealed record before creating or locking a file.
            // Unauthenticated lookups cannot fill custody with attacker-chosen keys.
            let prior = self.read(&name)?.0;
            if prior.identity.network != network
                || prior.identity.payment_hash != hash
                || prior.identity.request_hash != request_hash
                || prior.identity.authorization != commitment(secret)
            {
                return Err("Original purchase authorization is required.".into());
            }
            let source = Self::source(&prior.identity);
            let source_lock = self.lock(&source)?;
            let lock = self.lock(&name)?;
            let (record, sealed) = self.read(&name)?;
            if record.identity.network != network
                || record.identity.payment_hash != hash
                || record.identity.request_hash != request_hash
                || record.identity.authorization != commitment(secret)
            {
                return Err("Original purchase authorization is required.".into());
            }
            let binding = self.binding(&source, &record.identity)?;
            let tx = Transaction {
                store: self.clone(),
                name,
                _lock: lock,
                _source_lock: source_lock,
                source,
                binding,
                sealed: Some(sealed),
                poisoned: false,
                record,
            };
            tx.current()?;
            Ok(tx)
        }
    }
    pub(crate) struct Transaction {
        store: Store,
        name: String,
        _lock: File,
        _source_lock: File,
        source: String,
        binding: Sealed,
        sealed: Option<Sealed>,
        poisoned: bool,
        pub record: Record,
    }
    impl Transaction {
        pub fn current(&self) -> Result<(), String> {
            if self.poisoned {
                return Err("Outcome custody requires fresh recovery.".into());
            }
            self.store
                .same_file(&format!("{}.lock", self.name), &self._lock)?;
            self.store
                .same_file(&format!("{}.lock", self.source), &self._source_lock)?;
            self.store
                .check_sealed(&format!("{}.binding", self.source), &self.binding)?;
            if let Some(sealed) = &self.sealed {
                self.store
                    .check_sealed(&format!("{}.json", self.name), sealed)?;
            } else {
                let name = std::ffi::CString::new(format!("{}.json", self.name)).unwrap();
                let mut stat: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe {
                    libc::fstatat(
                        self.store.dir.as_raw_fd(),
                        name.as_ptr(),
                        &mut stat,
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                    || std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT)
                {
                    return Err("Outcome record appeared during admission.".into());
                }
            }
            Ok(())
        }
        pub fn save(&mut self) -> Result<(), String> {
            let result = self.save_inner();
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }
        fn save_inner(&mut self) -> Result<(), String> {
            self.current()?;
            self.record.check()?;
            let bytes = serde_json::to_vec(&self.record).map_err(|_| "Invalid outcome record.")?;
            if bytes.len() > MAX_RECORD {
                return Err("Outcome record exceeds its bound.".into());
            }
            let pending = format!("{}.pending", self.name);
            let c_pending = std::ffi::CString::new(pending.clone()).unwrap();
            // Only the holder of this purchase's lock can remove an interrupted write.
            unsafe {
                libc::unlinkat(self.store.dir.as_raw_fd(), c_pending.as_ptr(), 0);
            }
            let mut file = self
                .store
                .file(&pending, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| "Outcome record could not be persisted.")?;
            self.current()?;
            let target = std::ffi::CString::new(format!("{}.json", self.name)).unwrap();
            if unsafe {
                libc::renameat(
                    self.store.dir.as_raw_fd(),
                    c_pending.as_ptr(),
                    self.store.dir.as_raw_fd(),
                    target.as_ptr(),
                )
            } != 0
            {
                return Err("Outcome record could not be sealed.".into());
            }
            self.sealed = Some(Sealed {
                file,
                digest: Sha256::digest(&bytes).into(),
            });
            self.store
                .dir
                .sync_all()
                .map_err(|_| "Outcome directory could not be synced.")?;
            self.current()
        }
        pub fn stage(&mut self, stage: Stage) -> Result<(), String> {
            let old = self.record.stage;
            self.record.stage = stage;
            if let Err(e) = self.save() {
                self.record.stage = old;
                return Err(e);
            }
            Ok(())
        }
        pub fn finish(&mut self, response: Response) -> Result<(), String> {
            let old = self.record.clone();
            self.record.stage = if response.status == 200 {
                Stage::Completed
            } else {
                Stage::Failed
            };
            self.record.response = Some(response);
            if let Err(e) = self.save() {
                self.record = old;
                return Err(e);
            }
            Ok(())
        }
    }
}
