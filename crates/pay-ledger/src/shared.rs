//! One authoritative pool for admitted product reservations and known debits.
//! Native products keep their original grants, units, and economic evidence.
pub(crate) mod accounting;
use crate::compute::{ComputeBalance, Hold, HoldRequest};
use crate::{Error, Ledger, Rail, Result, SettlementInput, Split, record_settlement_in};
pub use accounting::{FundingReversal, Refund, RefundPlan, RefundReview, SourceSettlement};
pub(crate) use accounting::{claim_inbound_in, protected_loss_in};
use receipts::funding_units::{Conversion, Rounding, Unit};
use receipts::purchase::{CommercialProduct, CommercialRef, CommercialSource};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

pub const SCHEMA: &str = "openagents.shared-spend.v1";
pub const BODY_MAX: usize = 128 * 1024;
pub(crate) const TABLES: &str = "
CREATE TABLE IF NOT EXISTS shared_owner(singleton INTEGER PRIMARY KEY CHECK(singleton=1),bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_binding(id TEXT PRIMARY KEY,native_identity TEXT NOT NULL UNIQUE,retail_account TEXT UNIQUE,pool TEXT NOT NULL,bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_binding_version(id TEXT PRIMARY KEY,native_identity TEXT NOT NULL,previous TEXT NOT NULL,bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_binding_head(native_identity TEXT PRIMARY KEY,head TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_intent(id TEXT PRIMARY KEY,binding TEXT NOT NULL,native_attempt TEXT NOT NULL,digest TEXT NOT NULL UNIQUE,bytes TEXT NOT NULL,UNIQUE(binding,native_attempt));
CREATE TABLE IF NOT EXISTS shared_outcome(id TEXT PRIMARY KEY,state TEXT NOT NULL CHECK(state IN ('held','sending','unknown','settled')),charge INTEGER,expense TEXT,evidence TEXT,at INTEGER);
CREATE TABLE IF NOT EXISTS shared_funding_intent(id TEXT PRIMARY KEY,pool TEXT NOT NULL,bytes TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_wallet_handoff(intent TEXT PRIMARY KEY,permit TEXT NOT NULL,authority TEXT NOT NULL,at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS shared_funding_record(id TEXT PRIMARY KEY,digest TEXT NOT NULL UNIQUE,binding TEXT NOT NULL,terms TEXT NOT NULL,invoice TEXT);
CREATE TABLE IF NOT EXISTS shared_funding_reversal(id TEXT PRIMARY KEY,pool TEXT NOT NULL,funding TEXT NOT NULL,amount INTEGER NOT NULL,recovered INTEGER NOT NULL,loss INTEGER NOT NULL,evidence TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS shared_native_actor(intent TEXT PRIMARY KEY,bytes TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS shared_actor_no_update BEFORE UPDATE ON shared_native_actor BEGIN SELECT RAISE(ABORT,'Original native actor is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_actor_no_delete BEFORE DELETE ON shared_native_actor BEGIN SELECT RAISE(ABORT,'Original native actor is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_owner_no_update BEFORE UPDATE ON shared_owner BEGIN SELECT RAISE(ABORT,'Shared custody is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_owner_no_delete BEFORE DELETE ON shared_owner BEGIN SELECT RAISE(ABORT,'Shared custody is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_binding_no_update BEFORE UPDATE ON shared_binding BEGIN SELECT RAISE(ABORT,'Shared binding is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_binding_no_delete BEFORE DELETE ON shared_binding BEGIN SELECT RAISE(ABORT,'Shared binding is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_binding_version_no_update BEFORE UPDATE ON shared_binding_version BEGIN SELECT RAISE(ABORT,'Shared review is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_binding_version_no_delete BEFORE DELETE ON shared_binding_version BEGIN SELECT RAISE(ABORT,'Shared review is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_intent_no_update BEFORE UPDATE ON shared_intent BEGIN SELECT RAISE(ABORT,'Shared intent is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_intent_no_delete BEFORE DELETE ON shared_intent BEGIN SELECT RAISE(ABORT,'Shared intent is retained'); END;
CREATE TRIGGER IF NOT EXISTS shared_funding_intent_no_update BEFORE UPDATE ON shared_funding_intent BEGIN SELECT RAISE(ABORT,'Shared funding is immutable'); END;
CREATE TRIGGER IF NOT EXISTS shared_funding_intent_no_delete BEFORE DELETE ON shared_funding_intent BEGIN SELECT RAISE(ABORT,'Shared funding is retained'); END;
";
fn invalid(_: impl std::fmt::Display) -> Error {
    Error::Invalid("invalid shared spend document")
}
fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(invalid)
}
fn parse<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(invalid)
}
pub fn digest_secret(value: &str) -> String {
    crate::digest(value)
}
fn digest(value: &impl Serialize) -> String {
    crate::digest(&serde_json::to_string(value).expect("shared value serializes"))
}
fn hash(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn table(c: &Connection, name: &str) -> Result<bool> {
    Ok(c.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
        [name],
        |r| r.get(0),
    )?)
}
fn identifier(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn source_head_in(c: &Connection, binding: &Binding) -> Result<u64> {
    Ok(c.query_row("SELECT COALESCE(MAX(i.rowid),0) FROM shared_intent i JOIN (SELECT id,native_identity FROM shared_binding UNION ALL SELECT id,native_identity FROM shared_binding_version) b ON i.binding=b.id WHERE b.native_identity=?", [binding.native_identity()], |r|r.get(0))?)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub schema: String,
    pub origin: String,
    pub node: String,
    pub socket: PathBuf,
    pub file_device: u64,
    pub file_inode: u64,
    pub writer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub source: CommercialSource,
    pub native_origin: String,
    pub commercial: CommercialRef,
    pub pool: String,
    pub ledger_origin: String,
    pub custodian_node: String,
    pub controller: PathBuf,
    pub conversion: Conversion,
    pub operator_review: String,
}
impl Binding {
    pub fn mode(&self) -> receipts::shared_spend::Mode {
        receipts::shared_spend::Mode {
            binding: self.id.clone(),
            binding_digest: self.digest(),
            origin: self.ledger_origin.clone(),
            native_origin: self.native_origin.clone(),
            node: self.custodian_node.clone(),
            pool: self.pool.clone(),
            socket: self.controller.clone(),
            conversion: self.conversion.clone(),
            commercial: self.commercial.clone(),
            source: self.source.clone(),
        }
    }

    pub fn digest(&self) -> String {
        digest(self)
    }
    pub fn native_identity(&self) -> String {
        digest(&(
            &self.source.product,
            &self.native_origin,
            &self.source.account,
            &self.source.workspace,
        ))
    }
    pub fn validate(&self) -> Result<()> {
        self.commercial.validate().map_err(invalid)?;
        self.conversion.validate().map_err(invalid)?;
        if self.source != self.commercial.source
            || self.conversion.target != Unit::Millisatoshis
            || !identifier(&self.id)
            || !identifier(&self.pool)
            || !identifier(&self.operator_review)
            || !hash(&self.ledger_origin, 64)
            || !hash(&self.native_origin, 64)
            || !hash(&self.custodian_node, 66)
            || !self.controller.is_absolute()
        {
            return Err(Error::Invalid(
                "shared native identity, conversion, or review",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Liability {
    /// The provider's original receiver ledger owns its author shares and payout.
    ExternalInvoice {
        merchant: String,
        plugin: String,
        release: String,
        author: String,
        author_fee_msat: u64,
    },
    NativeService {
        resource: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Invoice {
    pub bolt11: String,
    pub payment_hash: String,
    pub request_hash: String,
    pub receiver: String,
    pub network: String,
    pub amount_msat: u64,
    pub valid_until: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub id: String,
    pub binding: Binding,
    pub native_attempt: String,
    pub quote: String,
    pub execution: String,
    pub terms: String,
    pub maximum_units: u64,
    pub fee_cap_msat: u64,
    pub invoice: Option<Invoice>,
    pub liability: Liability,
    pub admitted_at: u64,
}
impl Intent {
    pub fn stable_id(binding: &Binding, attempt: &str) -> String {
        format!(
            "shared:{}",
            digest(&(SCHEMA, binding.native_identity(), attempt))
        )
    }
    pub fn digest(&self) -> String {
        digest(self)
    }
    /// Spend liabilities round upward; precision never manufactures free work.
    pub fn convert(&self, units: u64) -> Result<u64> {
        let c = &self.binding.conversion;
        c.validate().map_err(invalid)?;
        if units == 0 {
            return Ok(0);
        }
        let n = u128::from(units) * u128::from(c.numerator);
        let d = u128::from(c.denominator);
        if c.rounding == Rounding::Exact && n % d != 0 {
            return Err(Error::Invalid("spend conversion precision"));
        }
        u64::try_from(n / d + u128::from(n % d != 0)).map_err(invalid)
    }
    pub fn reserved_msat(&self) -> Result<i64> {
        self.binding.validate()?;
        if self.id != Self::stable_id(&self.binding, &self.native_attempt)
            || !identifier(&self.native_attempt)
            || !identifier(&self.quote)
            || !identifier(&self.execution)
            || !identifier(&self.terms)
            || self.maximum_units == 0
            || self.admitted_at < self.binding.conversion.valid_from
            || self.admitted_at >= self.binding.conversion.valid_until
        {
            return Err(Error::Invalid("shared intent scope or validity"));
        }
        if let Some(invoice) = &self.invoice {
            if self.binding.source.product != CommercialProduct::Plugin
                || invoice.amount_msat != self.convert(self.maximum_units)?
                || invoice.amount_msat == 0
                || invoice.payment_hash.len() != 64
                || invoice.request_hash.len() != 64
                || invoice.receiver.len() != 66
                || invoice.bolt11.len() > 32 * 1024
                || self.admitted_at >= invoice.valid_until
                || !matches!(self.liability, Liability::ExternalInvoice { .. })
            {
                return Err(Error::Invalid("shared exact invoice"));
            }
        } else if self.fee_cap_msat != 0
            || !matches!(self.liability, Liability::NativeService { .. })
        {
            return Err(Error::Invalid("shared fee or native liability"));
        }
        i64::try_from(
            self.convert(self.maximum_units)?
                .checked_add(self.fee_cap_msat)
                .ok_or(Error::Invalid("shared amount overflow"))?,
        )
        .map_err(invalid)
    }
    pub fn hold_request(&self) -> Result<HoldRequest> {
        Ok(HoldRequest {
            id: self.id.clone(),
            account: self.binding.pool.clone(),
            quote: self.quote.clone(),
            execution: format!(
                "shared-execution:{}",
                digest(&(&self.binding.id, &self.execution))
            ),
            terms: self.digest(),
            amount_msat: self.reserved_msat()?,
            at: i64::try_from(self.admitted_at).map_err(invalid)?,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub intent: Intent,
    pub hold: Hold,
    pub state: String,
    pub expense: Option<Value>,
    pub evidence: Option<String>,
}
/// A compact original-attempt page never includes private invoice or source bytes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    pub intent: String,
    pub native_attempt: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionPage {
    pub entries: Vec<Projection>,
    pub through: u64,
    pub next: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    pub socket: PathBuf,
    pub binding: String,
    pub token_file: PathBuf,
    pub origin: String,
}
#[derive(Clone, Debug)]
pub struct Client {
    pub config: ClientConfig,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub binding: String,
    pub authorization: String,
    pub origin: String,
    pub operation: Operation,
}
/// An in-memory credential travels only over the protected local adapter socket.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayActor {
    pub credential: String,
    pub door: String,
}
impl std::fmt::Debug for GatewayActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayActor")
            .field("door", &self.door)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    Binding {},
    Identity {},
    Handoff {
        id: String,
        actor: GatewayActor,
    },
    Reserve {
        intent: Intent,
        projection_head: u64,
        actor: GatewayActor,
    },
    Observe {
        id: String,
    },
    DispatchPlugin {
        intent: Intent,
        wait_secs: u64,
    },
    ReconcilePlugin {
        id: String,
    },
    Settle {
        id: String,
        units: u64,
        evidence: String,
    },
    ReleaseUndispatched {
        id: String,
        evidence: String,
    },
    Unknown {
        id: String,
    },
    Balance {},
    SourceOutcomes {
        after: u64,
        through: Option<u64>,
    },
    RetailReserve {
        request: HoldRequest,
    },
    RetailSettle {
        id: String,
        charge_msat: i64,
        at: i64,
    },
    RetailUnknown {
        id: String,
    },
    RetailHandoff {
        id: String,
    },
    Funding {
        purchase: String,
        amount_sats: u64,
    },
    FundingStatus {
        purchase: String,
    },
    Refund {
        review: String,
    },
    RefundStatus {
        review: String,
    },
    ReverseFunding {
        review: String,
    },
}
impl Client {
    pub fn call(&self, operation: Operation) -> Result<Value> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
        let secret = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.config.token_file)
            .map_err(|_| Error::Denied("shared adapter credential unavailable"))?;
        let meta = secret.metadata().map_err(invalid)?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.mode() & 0o077 != 0
            || meta.len() > 1024
            || meta.uid() != unsafe { libc::geteuid() }
        {
            return Err(Error::Denied("private shared adapter credential required"));
        }
        use std::io::Read;
        let mut authorization = String::new();
        (&secret)
            .take(1025)
            .read_to_string(&mut authorization)
            .map_err(invalid)?;
        let end = std::fs::symlink_metadata(&self.config.token_file).map_err(invalid)?;
        if end.dev() != meta.dev() || end.ino() != meta.ino() || end.mode() & 0o077 != 0 {
            return Err(Error::Denied("shared adapter credential changed"));
        }
        let socket_metadata = std::fs::symlink_metadata(&self.config.socket).map_err(invalid)?;
        let parent = std::fs::symlink_metadata(
            self.config
                .socket
                .parent()
                .ok_or(Error::Invalid("shared socket parent"))?,
        )
        .map_err(invalid)?;
        if !socket_metadata.file_type().is_socket()
            || socket_metadata.mode() & 0o077 != 0
            || socket_metadata.uid() != unsafe { libc::geteuid() }
            || !parent.is_dir()
            || parent.mode() & 0o077 != 0
            || parent.uid() != unsafe { libc::geteuid() }
        {
            return Err(Error::Denied("private canonical socket required"));
        }
        let envelope = Envelope {
            binding: self.config.binding.clone(),
            authorization: authorization.trim().into(),
            origin: self.config.origin.clone(),
            operation,
        };
        let mut socket = UnixStream::connect(&self.config.socket)
            .map_err(|_| Error::Denied("shared controller unavailable; no native fallback"))?;
        socket
            .set_read_timeout(Some(Duration::from_secs(75)))
            .map_err(invalid)?;
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(invalid)?;
        let body = json(&envelope)?;
        if body.len() > BODY_MAX {
            return Err(Error::Invalid("shared request bound"));
        }
        socket.write_all(body.as_bytes()).map_err(invalid)?;
        socket.write_all(b"\n").map_err(invalid)?;
        let mut line = String::new();
        BufReader::new(socket)
            .take((BODY_MAX + 1) as u64)
            .read_line(&mut line)
            .map_err(invalid)?;
        if line.len() > BODY_MAX {
            return Err(Error::Invalid("shared reply bound"));
        }
        let reply: Value = serde_json::from_str(&line).map_err(invalid)?;
        let end = std::fs::symlink_metadata(&self.config.socket).map_err(invalid)?;
        if end.dev() != socket_metadata.dev() || end.ino() != socket_metadata.ino() {
            return Err(Error::Denied("canonical socket changed during operation"));
        }
        if reply["origin"] != self.config.origin {
            return Err(Error::Denied("shared reply origin changed"));
        }
        if let Some(result) = reply.get("result") {
            Ok(result.clone())
        } else {
            Err(Error::Denied(match reply["error"].as_str() {
                Some("custodian_refused") => "original custodian refused the operation",
                Some("canonical_ledger_refused") => "canonical ledger refused the operation",
                Some("native_account_custody_changed") => "native account custody changed",
                Some("invalid_shared_document") => "invalid bounded shared document",
                Some("shared_state_unavailable") => "shared state unavailable",
                Some("shared_funds_insufficient") => "shared funds insufficient",
                Some("original_reconciliation_required") => {
                    "original read-only reconciliation required"
                }
                _ => "shared controller refused the original operation",
            }))
        }
    }
}
fn binding_in(c: &Connection, id: &str) -> Result<Option<Binding>> {
    c.query_row("SELECT bytes FROM shared_binding WHERE id=? UNION ALL SELECT bytes FROM shared_binding_version WHERE id=?1 LIMIT 1", [id], |r| {
        r.get::<_, String>(0)
    })
    .optional()?
    .map(|s| parse(&s))
    .transpose()
}
fn intent_in(c: &Connection, id: &str) -> Result<Option<Intent>> {
    c.query_row("SELECT bytes FROM shared_intent WHERE id=?", [id], |r| {
        r.get::<_, String>(0)
    })
    .optional()?
    .map(|s| parse(&s))
    .transpose()
}
impl Ledger {
    /// The private writer secret is retained by the controller, never by adapters.
    pub fn admit_shared_writer(&mut self, secret: &str) -> Result<()> {
        let owner = self
            .shared_owner()?
            .ok_or(Error::Denied("shared owner required"))?;
        if !hash(secret, 64) || digest_secret(secret) != owner.writer {
            return Err(Error::Denied("private canonical writer required"));
        }
        self.shared_writer = true;
        Ok(())
    }
    fn require_shared_writer(&self) -> Result<()> {
        if !self.shared_writer {
            return Err(Error::Denied("canonical controller owns shared mutations"));
        }
        Ok(())
    }
    pub fn shared_pool(&self, account: &str) -> Result<bool> {
        if !table(&self.connection, "shared_binding")? {
            return Ok(false);
        }
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM shared_binding WHERE pool=?)",
            [account],
            |r| r.get(0),
        )?)
    }
    pub fn shared_owner(&self) -> Result<Option<Owner>> {
        self.connection
            .query_row(
                "SELECT bytes FROM shared_owner WHERE singleton=1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| parse(&s))
            .transpose()
    }
    /// Activation fixes this physical writer; a copied origin is not another pool.
    pub fn install_shared_owner(&mut self, owner: &Owner) -> Result<()> {
        if owner.schema != SCHEMA
            || owner.origin != self.origin()?
            || !owner.socket.is_absolute()
            || !hash(&owner.writer, 64)
            || !hash(&owner.node, 66)
        {
            return Err(Error::Invalid("shared owner identity"));
        }
        if let Some(old) = self.shared_owner()? {
            if &old == owner {
                return Ok(());
            }
            return Err(Error::Conflict("shared owner is immutable"));
        }
        self.connection.execute(
            "INSERT INTO shared_owner(singleton,bytes) VALUES(1,?)",
            [json(owner)?],
        )?;
        Ok(())
    }
    pub fn activate_shared(&mut self, binding: &Binding) -> Result<()> {
        self.require_shared_writer()?;
        binding.validate()?;
        let owner = self
            .shared_owner()?
            .ok_or(Error::Denied("shared custody has not been installed"))?;
        if binding.ledger_origin != owner.origin
            || binding.custodian_node != owner.node
            || binding.controller != owner.socket
        {
            return Err(Error::Conflict("another shared custodian"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = binding_in(&tx, &binding.id)? {
            if &old == binding {
                return Ok(());
            }
            return Err(Error::Conflict("shared binding is immutable"));
        }
        if binding.source.product == CommercialProduct::Retail {
            if binding.native_origin != owner.origin {
                return Err(Error::Denied(
                    "retail requires this actual canonical native ledger",
                ));
            }
            let account = &binding.source.account;
            let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM compute_credit WHERE account=?1) OR EXISTS(SELECT 1 FROM compute_hold WHERE account=?1) OR EXISTS(SELECT 1 FROM compute_purchase WHERE account=?1)",[account],|r|r.get(0))?;
            if occupied {
                return Err(Error::Denied(
                    "legacy retail activation requires an empty unencumbered account",
                ));
            }
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM compute_account WHERE id=?)",
                [account],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(Error::Denied("native retail account is required"));
            }
        }
        let pools: Vec<String> = {
            let mut q = tx.prepare("SELECT bytes FROM shared_binding WHERE pool=?")?;
            q.query_map([&binding.pool], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        if pools.is_empty() {
            let occupied: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM compute_credit WHERE account=?1) OR EXISTS(SELECT 1 FROM compute_hold WHERE account=?1) OR EXISTS(SELECT 1 FROM compute_purchase WHERE account=?1) OR EXISTS(SELECT 1 FROM compute_principal WHERE account=?1)", [&binding.pool], |r| r.get(0))?;
            if occupied {
                return Err(Error::Denied(
                    "canonical pool activation requires an empty unencumbered native account",
                ));
            }
        }
        for bytes in pools {
            let old: Binding = parse(&bytes)?;
            if old.commercial.customer != binding.commercial.customer
                || old.commercial.workspace != binding.commercial.workspace
            {
                return Err(Error::Conflict(
                    "a pool cannot merge distinct canonical customers",
                ));
            }
        }
        tx.execute(
            "INSERT INTO compute_account(id,created_at) VALUES(?,0) ON CONFLICT(id) DO NOTHING",
            [&binding.pool],
        )?;
        tx.execute("INSERT INTO shared_binding(id,native_identity,retail_account,pool,bytes) VALUES(?,?,?,?,?)",params![binding.id,binding.native_identity(),(binding.source.product==CommercialProduct::Retail).then_some(&binding.source.account),binding.pool,json(binding)?])?;
        tx.execute(
            "INSERT INTO shared_binding_head(native_identity,head) VALUES(?,?)",
            params![binding.native_identity(), binding.id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn shared_adapter_call(&self, operation: Operation) -> Result<Value> {
        self.shared_client
            .as_ref()
            .ok_or(Error::Denied(
                "shared controller required; no native fallback",
            ))?
            .call(operation)
    }
    pub fn shared_binding(&self, id: &str) -> Result<Option<Binding>> {
        binding_in(&self.connection, id)
    }
    pub fn shared_head(&self, binding: &Binding) -> Result<Option<Binding>> {
        let head: Option<String> = self
            .connection
            .query_row(
                "SELECT head FROM shared_binding_head WHERE native_identity=?",
                [binding.native_identity()],
                |r| r.get(0),
            )
            .optional()?;
        head.map(|id| binding_in(&self.connection, &id))
            .transpose()
            .map(Option::flatten)
    }
    /// An explicit reviewed transition preserves the physical alias and pool.
    pub fn migrate_shared(&mut self, binding: &Binding, previous: &str) -> Result<()> {
        self.require_shared_writer()?;
        binding.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let head: String = tx.query_row(
            "SELECT head FROM shared_binding_head WHERE native_identity=?",
            [binding.native_identity()],
            |r| r.get(0),
        )?;
        if let Some(old) = binding_in(&tx, &binding.id)? {
            return if old == *binding && head == binding.id {
                Ok(())
            } else {
                Err(Error::Conflict(
                    "shared historical review cannot become a fresh head",
                ))
            };
        }
        let old = binding_in(&tx, &head)?.ok_or(Error::Invalid("original shared head missing"))?;
        if old.digest() != previous
            || !old.mode().same_native(&binding.mode())
            || old.id == binding.id
            || binding.commercial.revision <= old.commercial.revision
        {
            return Err(Error::Conflict(
                "explicit same-native commercial migration required",
            ));
        }
        tx.execute(
            "INSERT INTO shared_binding_version(id,native_identity,previous,bytes) VALUES(?,?,?,?)",
            params![
                binding.id,
                binding.native_identity(),
                previous,
                json(binding)?
            ],
        )?;
        tx.execute(
            "UPDATE shared_binding_head SET head=? WHERE native_identity=?",
            params![binding.id, binding.native_identity()],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn shared_retail_binding(&self, account: &str) -> Result<Option<Binding>> {
        if !table(&self.connection, "shared_binding")? {
            return Ok(None);
        }
        let baseline = self
            .connection
            .query_row(
                "SELECT bytes FROM shared_binding WHERE retail_account=?",
                [account],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| parse(&s))
            .transpose()?;
        baseline
            .map(|b| self.shared_head(&b))
            .transpose()
            .map(Option::flatten)
    }
    pub fn configure_shared_client(&mut self, config: ClientConfig) -> Result<()> {
        if config.origin != self.origin()?
            || !config.socket.is_absolute()
            || self
                .shared_owner()?
                .is_none_or(|o| o.socket != config.socket)
        {
            return Err(Error::Invalid("shared client origin"));
        }
        let b = self
            .shared_binding(&config.binding)?
            .ok_or(Error::Denied("native shared binding required"))?;
        if b.source.product != CommercialProduct::Retail {
            return Err(Error::Denied("retail adapter required"));
        }
        self.shared_client = Some(Client { config });
        Ok(())
    }
    pub fn shared_reserve(&mut self, intent: &Intent) -> Result<Outcome> {
        self.shared_reserve_checked(intent, None)
    }
    /// The native journal inspects this exact source head before a new allocation.
    pub fn shared_reserve_projected(&mut self, intent: &Intent, head: u64) -> Result<Outcome> {
        self.shared_reserve_checked(intent, Some(head))
    }
    fn shared_reserve_checked(
        &mut self,
        intent: &Intent,
        projection: Option<u64>,
    ) -> Result<Outcome> {
        self.require_shared_writer()?;
        let request = intent.hold_request()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if binding_in(&tx, &intent.binding.id)?.as_ref() != Some(&intent.binding) {
            return Err(Error::Conflict("shared source binding changed"));
        }
        if let Some(old) = intent_in(&tx, &intent.id)? {
            if &old != intent {
                return Err(Error::Conflict("shared original intent changed"));
            }
            drop(tx);
            return self
                .shared_outcome(&intent.id)?
                .ok_or(Error::Invalid("missing shared hold"));
        }
        if let Some(head) = projection {
            if source_head_in(&tx, &intent.binding)? != head {
                return Err(Error::Conflict("original native projection head changed"));
            }
        }
        let head: String = tx.query_row(
            "SELECT head FROM shared_binding_head WHERE native_identity=?",
            [intent.binding.native_identity()],
            |r| r.get(0),
        )?;
        if head != intent.binding.id {
            return Err(Error::Denied(
                "new spending requires the current reviewed shared head",
            ));
        }
        let balance = crate::compute::hold::balance_in(&tx, &request.account)?;
        if protected_loss_in(&tx, &request.account)? > 0 {
            return Err(Error::Denied(
                "original protected loss requires reconciliation before new spending",
            ));
        }
        if balance.available_msat < request.amount_msat {
            return Err(Error::Insufficient {
                available_msat: balance.available_msat,
            });
        }
        tx.execute(
            "INSERT INTO shared_intent(id,binding,native_attempt,digest,bytes) VALUES(?,?,?,?,?)",
            params![
                intent.id,
                intent.binding.id,
                intent.native_attempt,
                intent.digest(),
                json(intent)?
            ],
        )?;
        tx.execute("INSERT INTO compute_hold(id,account,quote,execution,terms,amount_msat,state,created_at) VALUES(?,?,?,?,?,?,'held',?)",params![request.id,request.account,request.quote,request.execution,request.terms,request.amount_msat,request.at])?;
        tx.execute(
            "INSERT INTO shared_outcome(id,state) VALUES(?,'held')",
            [&intent.id],
        )?;
        tx.commit()?;
        self.shared_outcome(&intent.id)?
            .ok_or(Error::Invalid("missing shared hold"))
    }
    pub fn shared_outcome(&self, id: &str) -> Result<Option<Outcome>> {
        let Some(intent) = intent_in(&self.connection, id)? else {
            return Ok(None);
        };
        let (state, expense, evidence): (String, Option<String>, Option<String>) =
            self.connection.query_row(
                "SELECT state,expense,evidence FROM shared_outcome WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
        let hold = crate::compute::hold::read_hold(&self.connection, "id", id)?
            .ok_or(Error::Invalid("missing shared hold"))?;
        Ok(Some(Outcome {
            intent,
            hold,
            state,
            expense: expense.map(|s| parse(&s)).transpose()?,
            evidence,
        }))
    }
    /// Unknown is committed before IPC. A retry observes; it never redispatches.
    pub fn shared_handoff(&mut self, id: &str) -> Result<Outcome> {
        self.require_shared_writer()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state: String =
            tx.query_row("SELECT state FROM shared_outcome WHERE id=?", [id], |r| {
                r.get(0)
            })?;
        if state != "held" {
            return Err(Error::Denied(
                "the original handoff is already sealed; reconcile it",
            ));
        }
        tx.execute("UPDATE shared_outcome SET state='sending' WHERE id=?", [id])?;
        tx.execute(
            "UPDATE compute_hold SET state='unknown' WHERE id=? AND state='held'",
            [id],
        )?;
        tx.commit()?;
        self.shared_outcome(id)?
            .ok_or(Error::Invalid("missing shared intent"))
    }
    pub fn shared_unknown(&mut self, id: &str) -> Result<Outcome> {
        self.require_shared_writer()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE shared_outcome SET state='unknown' WHERE id=? AND state!='settled'",
            [id],
        )?;
        tx.execute(
            "UPDATE compute_hold SET state='unknown' WHERE id=? AND state='held'",
            [id],
        )?;
        tx.commit()?;
        self.shared_outcome(id)?
            .ok_or(Error::Invalid("missing shared intent"))
    }
    /// Known native expense, or a native service debit, and release are atomic.
    pub fn shared_settle(
        &mut self,
        id: &str,
        units: u64,
        fee: u64,
        evidence: &str,
        expense: Option<&Value>,
        at: i64,
    ) -> Result<Outcome> {
        self.require_shared_writer()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let intent =
            intent_in(&tx, id)?.ok_or(Error::Invalid("original shared intent required"))?;
        if !identifier(evidence) || units > intent.maximum_units || fee > intent.fee_cap_msat {
            return Err(Error::Invalid("known shared settlement bounds"));
        }
        let charge = i64::try_from(
            intent
                .convert(units)?
                .checked_add(fee)
                .ok_or(Error::Invalid("shared charge overflow"))?,
        )
        .map_err(invalid)?;
        let (state, old_charge, old_expense): (String, Option<i64>, Option<String>) = tx
            .query_row(
                "SELECT state,charge,expense FROM shared_outcome WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
        let bytes = expense.map(json).transpose()?;
        accounting::seal_settlement_in(&tx, &intent, units, fee, evidence, expense, at)?;
        if state == "settled" {
            if old_charge != Some(charge) || old_expense != bytes {
                return Err(Error::Conflict("original shared settlement changed"));
            }
            drop(tx);
            return self
                .shared_outcome(id)?
                .ok_or(Error::Invalid("missing shared settlement"));
        }
        match &intent.liability {
            Liability::ExternalInvoice { .. }
                if !(state == "held" && units == 0 && fee == 0 && expense.is_none())
                    && (expense.is_none() || units != intent.maximum_units) =>
            {
                return Err(Error::Denied("original paid expense proof required"));
            }
            Liability::NativeService { resource } if charge > 0 => {
                record_settlement_in(
                    &tx,
                    SettlementInput {
                        key: format!("debit:{id}"),
                        resource: resource.clone(),
                        plugin_id: None,
                        release_id: None,
                        price_msat: charge,
                        received_msat: charge,
                        rail: Rail::Balance,
                        payer_alias: None,
                        settled_at: at,
                        split: Split::OpenAgents,
                    },
                )?;
            }
            _ => {}
        }
        tx.execute(
            "UPDATE compute_hold SET state='settled',charge_msat=?,settled_at=? WHERE id=?",
            params![charge, at, id],
        )?;
        tx.execute("UPDATE shared_outcome SET state='settled',charge=?,expense=?,evidence=?,at=? WHERE id=?",params![charge,bytes,evidence,at,id])?;
        tx.commit()?;
        self.shared_outcome(id)?
            .ok_or(Error::Invalid("missing shared settlement"))
    }
    pub fn shared_release_undispatched(
        &mut self,
        id: &str,
        evidence: &str,
        at: i64,
    ) -> Result<Outcome> {
        let outcome = self
            .shared_outcome(id)?
            .ok_or(Error::Invalid("original shared intent required"))?;
        if outcome.state != "held" {
            return Err(Error::Denied(
                "sending or unknown work cannot be released as undispatched",
            ));
        }
        self.shared_settle(id, 0, 0, evidence, None, at)
    }
    pub fn shared_retail_outcome(&self, native_attempt: &str) -> Result<Option<Outcome>> {
        if !table(&self.connection, "shared_intent")? {
            return Ok(None);
        }
        let rows: Vec<String> = {
            let mut q = self
                .connection
                .prepare("SELECT i.id FROM shared_intent i JOIN (SELECT id,bytes FROM shared_binding UNION ALL SELECT id,bytes FROM shared_binding_version) b ON i.binding=b.id WHERE i.native_attempt=? AND json_extract(b.bytes,'$.source.product')='retail' LIMIT 2")?;
            q.query_map([native_attempt], |r| r.get(0))?
                .collect::<std::result::Result<_, _>>()?
        };
        if rows.len() > 1 {
            return Err(Error::Conflict("native retail attempt is ambiguous"));
        }
        for id in rows {
            let out = self
                .shared_outcome(&id)?
                .ok_or(Error::Invalid("shared retail intent disappeared"))?;
            if out.intent.binding.source.product == CommercialProduct::Retail {
                return Ok(Some(out));
            }
        }
        Ok(None)
    }
    pub(crate) fn shared_retail_client(&self, out: &Outcome) -> Result<&Client> {
        let client = self
            .shared_client
            .as_ref()
            .ok_or(Error::Denied("original shared controller required"))?;
        let current = self
            .shared_binding(&client.config.binding)?
            .ok_or(Error::Denied("retained native binding required"))?;
        if !current.mode().same_native(&out.intent.binding.mode()) {
            return Err(Error::Denied("original native retail source required"));
        }
        Ok(client)
    }
    pub fn shared_retail_hold(out: &Outcome) -> Result<Hold> {
        let mut hold = out.hold.clone();
        hold.request.id = out.intent.native_attempt.clone();
        hold.request.account = out.intent.binding.source.account.clone();
        hold.request.quote = out.intent.quote.clone();
        hold.request.execution = out.intent.execution.clone();
        hold.request.terms = out.intent.terms.clone();
        if out.state == "sending" {
            hold.state = crate::compute::HoldState::Held;
        }
        Ok(hold)
    }
    pub fn shared_balance(&self, binding: &str) -> Result<ComputeBalance> {
        let b = self
            .shared_binding(binding)?
            .ok_or(Error::Denied("native shared binding required"))?;
        crate::compute::hold::balance_in(&self.connection, &b.pool)
    }
    /// Inspect compact pages at one original source head, then pin that head in reserve.
    pub fn shared_source_outcomes(
        &self,
        binding: &Binding,
        after: u64,
        through: Option<u64>,
    ) -> Result<ProjectionPage> {
        let current = source_head_in(&self.connection, binding)?;
        let through = through.unwrap_or(current);
        if after > through || through > current {
            return Err(Error::Invalid("native projection cursor"));
        }
        let mut q = self.connection.prepare("SELECT i.rowid,i.id,i.native_attempt FROM shared_intent i JOIN (SELECT id,native_identity FROM shared_binding UNION ALL SELECT id,native_identity FROM shared_binding_version) b ON i.binding=b.id WHERE b.native_identity=? AND i.rowid>? AND i.rowid<=? ORDER BY i.rowid LIMIT 129")?;
        let mut rows: Vec<(u64, Projection)> = q
            .query_map(params![binding.native_identity(), after, through], |r| {
                Ok((
                    r.get(0)?,
                    Projection {
                        intent: r.get(1)?,
                        native_attempt: r.get(2)?,
                    },
                ))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let next = (rows.len() > 128).then(|| rows[127].0);
        rows.truncate(128);
        Ok(ProjectionPage {
            entries: rows.into_iter().map(|r| r.1).collect(),
            through,
            next,
        })
    }
    pub fn shared_intent_by_digest(&self, value: &str) -> Result<Option<Intent>> {
        self.connection
            .query_row(
                "SELECT bytes FROM shared_intent WHERE digest=?",
                [value],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| parse(&s))
            .transpose()
    }
    /// A resident callback seals the exact current native admission once, before its IPC effect.
    pub fn shared_wallet_handoff(
        &mut self,
        intent: &str,
        permit: &Value,
        authority: &Value,
        at: u64,
    ) -> Result<()> {
        self.require_shared_writer()?;
        if !hash(intent, 64) {
            return Err(Error::Invalid("custody intent digest"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sending: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM shared_intent i JOIN shared_outcome o ON i.id=o.id WHERE i.digest=?1 AND o.state='sending') OR EXISTS(SELECT 1 FROM shared_funding_record WHERE digest=?1 AND invoice IS NULL) OR EXISTS(SELECT 1 FROM shared_refund_plan WHERE digest=?1 AND invoice IS NULL)", [intent], |r| r.get(0))?;
        if !sending {
            return Err(Error::Denied("original sending intent required"));
        }
        tx.execute(
            "INSERT INTO shared_wallet_handoff(intent,permit,authority,at) VALUES(?,?,?,?)",
            params![intent, json(permit)?, json(authority)?, at],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn shared_begin_funding(
        &mut self,
        id: &str,
        binding: &Binding,
        terms: &Value,
    ) -> Result<bool> {
        self.require_shared_writer()?;
        if self.shared_binding(&binding.id)?.as_ref() != Some(binding) {
            return Err(Error::Conflict("original funding binding required"));
        }
        let bytes = json(terms)?;
        if !identifier(id) || bytes.len() > BODY_MAX {
            return Err(Error::Invalid("bounded funding intent required"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(String, String)> = tx
            .query_row(
                "SELECT binding,terms FROM shared_funding_record WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((old_binding, old)) = previous {
            if old_binding != binding.id || old != bytes {
                return Err(Error::Conflict("original funding terms changed"));
            }
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO shared_funding_record(id,digest,binding,terms) VALUES(?,?,?,?)",
            params![id, digest(terms), binding.id, bytes],
        )?;
        tx.commit()?;
        Ok(true)
    }
    pub fn shared_funding_record(
        &self,
        id: &str,
    ) -> Result<Option<(String, Value, Option<Value>)>> {
        self.connection
            .query_row(
                "SELECT binding,terms,invoice FROM shared_funding_record WHERE id=?",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
            .map(|(b, t, i)| Ok((b, parse(&t)?, i.map(|s| parse(&s)).transpose()?)))
            .transpose()
    }
    pub fn shared_funding_by_digest(&self, value: &str) -> Result<Option<(String, Value)>> {
        self.connection
            .query_row(
                "SELECT binding,terms FROM shared_funding_record WHERE digest=?",
                [value],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
            .map(|(b, t)| Ok((b, parse(&t)?)))
            .transpose()
    }
    pub fn shared_funding_invoice(&mut self, id: &str, invoice: &Value) -> Result<()> {
        self.require_shared_writer()?;
        let bytes = json(invoice)?;
        let changed = self.connection.execute(
            "UPDATE shared_funding_record SET invoice=? WHERE id=? AND invoice IS NULL",
            params![bytes, id],
        )?;
        if changed != 1
            && self.shared_funding_record(id)?.and_then(|r| r.2).as_ref() != Some(invoice)
        {
            return Err(Error::Conflict("original funding invoice changed"));
        }
        Ok(())
    }
    pub fn shared_freeze_funding(&mut self, id: &str, pool: &str, terms: &Value) -> Result<bool> {
        self.require_shared_writer()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !identifier(id) || !identifier(pool) {
            return Err(Error::Invalid("shared funding identity"));
        }
        let bytes = json(terms)?;
        if let Some((old_pool, old_bytes)) = tx
            .query_row(
                "SELECT pool,bytes FROM shared_funding_intent WHERE id=?",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if old_pool != pool || old_bytes != bytes {
                return Err(Error::Conflict("original shared funding changed"));
            }
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO shared_funding_intent(id,pool,bytes) VALUES(?,?,?)",
            params![id, pool, bytes],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::{Receipt, TopUp};
    use receipts::funding_units::FeePayer;
    #[test]
    fn original_units_and_cumulative_refunds_keep_rounding_after_migration() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        fund(&mut ledger, &b);
        let mut fx = b.clone();
        fx.id = "fx-gateway".into();
        fx.source.product = CommercialProduct::Gateway;
        fx.source.account = "native-gateway".into();
        fx.source.workspace = Some("native-workspace".into());
        fx.commercial.source = fx.source.clone();
        fx.conversion.source = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        fx.conversion.numerator = 1;
        fx.conversion.denominator = 3;
        fx.conversion.rounding = Rounding::Down;
        ledger.activate_shared(&fx).unwrap();
        let admitted = intent(&fx, "rounding", 6);
        ledger.shared_reserve(&admitted).unwrap();
        ledger
            .shared_settle(&admitted.id, 5, 0, "original-native-cost", None, 6)
            .unwrap();
        assert!(
            ledger
                .shared_settle(&admitted.id, 4, 0, "original-native-cost", None, 7)
                .is_err()
        );
        assert!(
            ledger
                .shared_settle(&admitted.id, 5, 0, "changed-cost", None, 7)
                .is_err()
        );
        let original = ledger
            .shared_source_settlement(&admitted.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                original.units,
                original.converted_msat,
                original.remainder,
                original.denominator
            ),
            (5, 2, 2, 3)
        );
        let review = |id: &str, units| RefundReview {
            id: id.into(),
            intent: admitted.id.clone(),
            intent_digest: admitted.digest(),
            units,
            evidence: format!("native-return:{id}"),
            reviewed_at: 0,
            valid_until: 1000,
        };
        let first = ledger
            .shared_return_expense(&review("first", 1), None, 8)
            .unwrap();
        assert_eq!(first.returned_msat, 0);
        let second = ledger
            .shared_return_expense(&review("second", 1), None, 9)
            .unwrap();
        assert_eq!(second.returned_msat, 1);
        let mut later = fx.clone();
        later.id = "fx-next".into();
        later.commercial.revision += 1;
        later.commercial.digest = format!("sha256:{}", "c".repeat(64));
        later.conversion.numerator = 100;
        ledger.migrate_shared(&later, &fx.digest()).unwrap();
        let full = ledger
            .shared_return_expense(&review("full", 3), None, 10)
            .unwrap();
        assert_eq!(full.returned_msat, 1);
        assert_eq!(full.original, original);
        assert_eq!(
            ledger
                .shared_return_expense(&review("first", 1), None, 20)
                .unwrap(),
            first
        );
        assert!(
            ledger
                .shared_return_expense(&review("too-much", 1), None, 11)
                .is_err()
        );
        let balance = ledger.shared_balance(&later.id).unwrap();
        assert_eq!(
            (
                balance.credited_msat,
                balance.refunded_msat,
                balance.available_msat,
                balance.settled_msat
            ),
            (1000, 2, 1000, 2)
        );
        assert_eq!(
            ledger
                .settlement(&format!("debit:{}", admitted.id))
                .unwrap()
                .unwrap()
                .price_msat,
            2
        );
    }
    #[test]
    fn protected_original_claim_loss_blocks_new_spend_without_erasing_history() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        fund(&mut ledger, &b);
        let admitted = intent(&b, "protected", 1000);
        ledger.shared_reserve(&admitted).unwrap();
        ledger
            .shared_settle(&admitted.id, 1000, 0, "original", None, 6)
            .unwrap();
        ledger
            .register_payee(crate::Payee {
                party: crate::OPENAGENTS.into(),
                destination_kind: "spark".into(),
                destination_value: "isolated".into(),
                source: "protected-owner-review".into(),
                verified_at: 7,
            })
            .unwrap();
        let shares = ledger.available_shares(crate::OPENAGENTS).unwrap();
        ledger
            .reserve_payout("original-unknown-payout", crate::OPENAGENTS, &shares, 8)
            .unwrap();
        let review = RefundReview {
            id: "accepted-original-refund".into(),
            intent: admitted.id.clone(),
            intent_digest: admitted.digest(),
            units: 1000,
            evidence: "protected-native-refund".into(),
            reviewed_at: 0,
            valid_until: 1000,
        };
        let returned = ledger.shared_return_expense(&review, None, 9).unwrap();
        assert_eq!(
            (
                returned.returned_msat,
                returned.reduced_msat,
                returned.loss_msat
            ),
            (1000, 0, 1000)
        );
        let balance = ledger.shared_balance(&b.id).unwrap();
        assert_eq!(
            (
                balance.available_msat,
                balance.restricted_msat,
                balance.protected_loss_msat,
                balance.settled_msat
            ),
            (0, 1000, 1000, 1000)
        );
        assert!(
            ledger
                .shared_reserve(&intent(&b, "cannot-recycle", 1))
                .is_err()
        );
        assert_eq!(
            ledger.shared_return_expense(&review, None, 10).unwrap(),
            returned
        );
        assert_eq!(ledger.shared_balance(&b.id).unwrap(), balance);
        assert_eq!(
            ledger
                .settlement(&format!("debit:{}", admitted.id))
                .unwrap()
                .unwrap()
                .price_msat,
            1000
        );
    }
    #[test]
    fn funding_reversal_keeps_unknown_holds_and_blocks_fresh_funding() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        let terms = serde_json::json!({"original":"fixture"});
        ledger
            .shared_begin_funding("fixture-funding", &b, &terms)
            .unwrap();
        ledger
            .shared_funding_invoice(
                "fixture-funding",
                &serde_json::json!({"original":"invoice"}),
            )
            .unwrap();
        fund(&mut ledger, &b);
        let original = intent(&b, "original-unknown", 800);
        ledger.shared_reserve(&original).unwrap();
        ledger.shared_handoff(&original.id).unwrap();
        ledger.shared_unknown(&original.id).unwrap();
        let row = ledger
            .shared_reverse_funding(
                "original-reversal",
                "fixture-funding",
                600,
                "original-confirmed-source-loss",
            )
            .unwrap();
        assert_eq!(
            (row.amount_msat, row.recovered_msat, row.loss_msat),
            (600, 200, 400)
        );
        let balance = ledger.shared_balance(&b.id).unwrap();
        assert_eq!(
            (
                balance.available_msat,
                balance.held_msat,
                balance.protected_loss_msat
            ),
            (0, 800, 400)
        );
        assert_eq!(
            ledger.shared_outcome(&original.id).unwrap().unwrap().state,
            "unknown"
        );
        assert_eq!(
            ledger
                .shared_reverse_funding(
                    "original-reversal",
                    "fixture-funding",
                    600,
                    "original-confirmed-source-loss"
                )
                .unwrap(),
            row
        );
        assert!(
            ledger
                .shared_reverse_funding(
                    "changed-original",
                    "fixture-funding",
                    401,
                    "oversized-loss"
                )
                .is_err()
        );
        ledger
            .shared_freeze_funding(
                "fresh-funding",
                &b.pool,
                &serde_json::json!({"custodian":b.custodian_node,"original_source":b.source}),
            )
            .unwrap();
        ledger
            .open_top_up(&TopUp {
                id: "fresh-funding".into(),
                account: b.pool.clone(),
                amount_msat: 1000,
                payment_hash: "d".repeat(64),
                invoice: "isolated-fresh-invoice".into(),
                created_at: 10,
                expires_at: 900,
            })
            .unwrap();
        ledger
            .observe_top_up(
                &"d".repeat(64),
                &Receipt::Paid {
                    received_msat: 1000,
                    at: 11,
                },
            )
            .unwrap();
        let balance = ledger.shared_balance(&b.id).unwrap();
        assert_eq!(
            (
                balance.available_msat,
                balance.restricted_msat,
                balance.held_msat,
                balance.protected_loss_msat
            ),
            (0, 1000, 800, 400)
        );
        assert!(
            ledger
                .shared_reserve(&intent(&b, "fresh-cannot-bypass-loss", 1))
                .is_err()
        );
        assert!(
            ledger
                .shared_release_undispatched(&original.id, "not-evidence", 12)
                .is_err()
        );
        assert_eq!(ledger.shared_balance(&b.id).unwrap(), balance);
    }
    #[test]
    fn delayed_original_refund_confirmation_preserves_reserved_units_and_rounding() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        fund(&mut ledger, &b);
        let mut fx = b.clone();
        fx.id = "fractional-native-gateway".into();
        fx.source.product = CommercialProduct::Gateway;
        fx.source.account = "gateway".into();
        fx.source.workspace = Some("workspace".into());
        fx.commercial.source = fx.source.clone();
        fx.conversion.source = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        fx.conversion.numerator = 1;
        fx.conversion.denominator = 3;
        fx.conversion.rounding = Rounding::Down;
        ledger.activate_shared(&fx).unwrap();
        let purchase = intent(&fx, "delayed-original", 6);
        ledger.shared_reserve(&purchase).unwrap();
        ledger
            .shared_settle(&purchase.id, 5, 0, "original-cost", None, 6)
            .unwrap();
        let review = |id: &str, units| RefundReview {
            id: id.into(),
            intent: purchase.id.clone(),
            intent_digest: purchase.digest(),
            units,
            evidence: format!("original:{id}"),
            reviewed_at: 0,
            valid_until: 1000,
        };
        let first = review("prepared-original", 1);
        let plan = ledger.shared_prepare_return(&first, 7).unwrap();
        assert_eq!(plan.returned_msat, 0);
        let second = ledger
            .shared_return_expense(&review("confirmed-second", 1), None, 8)
            .unwrap();
        assert_eq!(second.returned_msat, 1);
        let recovered = ledger.shared_return_expense(&first, None, 9).unwrap();
        assert_eq!(recovered.returned_msat, 0);
        assert_eq!(
            ledger.shared_return_expense(&first, None, 10).unwrap(),
            recovered
        );
        let last = ledger
            .shared_return_expense(&review("confirmed-last", 3), None, 11)
            .unwrap();
        assert_eq!(last.returned_msat, 1);
        assert!(
            ledger
                .shared_prepare_return(&review("over-original", 1), 12)
                .is_err()
        );
        let balance = ledger.shared_balance(&fx.id).unwrap();
        assert_eq!(
            (
                balance.refunded_msat,
                balance.available_msat,
                balance.settled_msat
            ),
            (2, 1000, 2)
        );
    }

    fn setup(ledger: &mut Ledger) -> Binding {
        let rule = crate::V1
            .replace("version = 1", "version = 2")
            .replace("2026-10-02T00:00:00Z", "1970-01-01T00:00:00Z");
        ledger.load_rule(&rule, &crate::digest(&rule)).unwrap();
        let origin = ledger.origin().unwrap();
        ledger
            .install_shared_owner(&Owner {
                schema: SCHEMA.into(),
                origin: origin.clone(),
                node: "02".repeat(33),
                socket: "/isolated/controller.sock".into(),
                file_device: 0,
                file_inode: 0,
                writer: digest_secret(&"aa".repeat(32)),
            })
            .unwrap();
        ledger.admit_shared_writer(&"aa".repeat(32)).unwrap();
        ledger.create_compute_account("native-retail", 0).unwrap();
        let source = CommercialSource {
            product: CommercialProduct::Retail,
            issuer: "retail-native".into(),
            account: "native-retail".into(),
            workspace: None,
        };
        let binding = Binding {
            id: "reviewed-retail".into(),
            source: source.clone(),
            native_origin: origin.clone(),
            commercial: CommercialRef {
                binding: "canonical-binding".into(),
                revision: 1,
                digest: format!("sha256:{}", "a".repeat(64)),
                customer: "canonical-customer".into(),
                workspace: "canonical-workspace".into(),
                source,
            },
            pool: "canonical-pool".into(),
            ledger_origin: origin,
            custodian_node: "02".repeat(33),
            controller: "/isolated/controller.sock".into(),
            conversion: Conversion {
                version: "reviewed-msat-v1".into(),
                source: Unit::Millisatoshis,
                target: Unit::Millisatoshis,
                numerator: 1,
                denominator: 1,
                source_ref: "fixture-native-unit-review".into(),
                valid_from: 0,
                valid_until: 1000,
                rounding: Rounding::Exact,
                fee_payer: FeePayer::Operator,
                max_fee_units: 0,
            },
            operator_review: "isolated-explicit-review".into(),
        };
        ledger.activate_shared(&binding).unwrap();
        binding
    }
    fn fund(ledger: &mut Ledger, b: &Binding) {
        ledger
            .shared_freeze_funding(
                "fixture-funding",
                &b.pool,
                &serde_json::json!({"custodian":b.custodian_node,"original_source":b.source}),
            )
            .unwrap();
        ledger
            .open_top_up(&TopUp {
                id: "fixture-funding".into(),
                account: b.pool.clone(),
                amount_msat: 1000,
                payment_hash: "b".repeat(64),
                invoice: "isolated-synthetic-invoice".into(),
                created_at: 0,
                expires_at: 900,
            })
            .unwrap();
        ledger
            .observe_top_up(
                &"b".repeat(64),
                &Receipt::Paid {
                    received_msat: 1000,
                    at: 1,
                },
            )
            .unwrap();
        ledger
            .observe_top_up(
                &"b".repeat(64),
                &Receipt::Paid {
                    received_msat: 1000,
                    at: 2,
                },
            )
            .unwrap();
    }
    fn intent(b: &Binding, id: &str, units: u64) -> Intent {
        Intent {
            id: Intent::stable_id(b, id),
            binding: b.clone(),
            native_attempt: id.into(),
            quote: "original-native-quote".into(),
            execution: format!("native-execution:{id}"),
            terms: "original-native-terms".into(),
            maximum_units: units,
            fee_cap_msat: 0,
            invoice: None,
            liability: Liability::NativeService {
                resource: crate::compute::hold::RETAIL_RESOURCE.into(),
            },
            admitted_at: 5,
        }
    }
    #[test]
    fn one_pool_preserves_unknown_original_terms_unused_holds_and_once_only_native_liability() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        fund(&mut ledger, &b);
        let original = intent(&b, "original", 900);
        ledger.shared_reserve(&original).unwrap();
        ledger.shared_handoff(&original.id).unwrap();
        ledger.shared_unknown(&original.id).unwrap();
        assert!(
            ledger
                .shared_release_undispatched(&original.id, "not-proof", 6)
                .is_err()
        );
        assert!(ledger.shared_handoff(&original.id).is_err());
        assert_eq!(ledger.shared_reserve(&original).unwrap().state, "unknown");
        let mut changed = original.clone();
        changed.terms = "changed".into();
        assert!(ledger.shared_reserve(&changed).is_err());
        assert!(ledger.shared_reserve(&intent(&b, "parallel", 101)).is_err());
        let known = ledger
            .shared_settle(
                &original.id,
                100,
                0,
                "original-native-receipt",
                None,
                1_791_000_000,
            )
            .unwrap();
        assert_eq!(known.hold.released_msat(), Some(800));
        ledger
            .shared_settle(
                &original.id,
                100,
                0,
                "original-native-receipt",
                None,
                1_791_000_001,
            )
            .unwrap();
        assert_eq!(
            ledger
                .settlement(&format!("debit:{}", original.id))
                .unwrap()
                .unwrap()
                .shares
                .iter()
                .map(|s| s.amount_msat)
                .sum::<i64>(),
            100
        );
        let balance = ledger.compute_balance("native-retail").unwrap();
        assert_eq!(
            (
                balance.credited_msat,
                balance.available_msat,
                balance.held_msat,
                balance.settled_msat
            ),
            (1000, 900, 0, 100)
        );
        assert!(
            ledger
                .reserve(&HoldRequest {
                    id: "legacy-bypass".into(),
                    account: "native-retail".into(),
                    quote: "q".into(),
                    execution: "e".into(),
                    terms: "t".into(),
                    amount_msat: 1,
                    at: 8
                })
                .is_err()
        );
        assert!(
            ledger
                .open_top_up(&TopUp {
                    id: "legacy-topup".into(),
                    account: "native-retail".into(),
                    amount_msat: 1000,
                    payment_hash: "c".repeat(64),
                    invoice: "legacy".into(),
                    created_at: 8,
                    expires_at: 900
                })
                .is_err()
        );
    }
    #[test]
    fn native_alias_or_units_cannot_create_another_pool_or_import_legacy_funds() {
        let mut ledger = Ledger::in_memory().unwrap();
        let b = setup(&mut ledger);
        fund(&mut ledger, &b);
        let mut alias = b.clone();
        alias.id = "another-issuer".into();
        alias.source.issuer = "renamed".into();
        alias.commercial.source = alias.source.clone();
        assert!(ledger.activate_shared(&alias).is_err());
        let mut wrong = b.clone();
        wrong.conversion.target = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        assert!(wrong.validate().is_err());
        ledger.create_compute_account("occupied", 0).unwrap();
        ledger
            .open_top_up(&TopUp {
                id: "occupied-funding".into(),
                account: "occupied".into(),
                amount_msat: 1000,
                payment_hash: "d".repeat(64),
                invoice: "isolated".into(),
                created_at: 0,
                expires_at: 900,
            })
            .unwrap();
        let mut occupied = b.clone();
        occupied.id = "occupied".into();
        occupied.source.account = "occupied".into();
        occupied.commercial.source = occupied.source.clone();
        assert!(ledger.activate_shared(&occupied).is_err());
    }
    #[test]
    fn external_invoice_is_an_expense_without_another_merchant_gross_credit() {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut b = setup(&mut ledger);
        fund(&mut ledger, &b);
        b.id = "reviewed-plugin".into();
        b.source.product = CommercialProduct::Plugin;
        b.source.account = "plugin-buyer".into();
        b.source.workspace = Some("native-workspace".into());
        b.commercial.source = b.source.clone();
        b.native_origin = "e".repeat(64);
        ledger.activate_shared(&b).unwrap();
        let mut purchase = intent(&b, "paid-plugin", 800);
        purchase.fee_cap_msat = 100;
        purchase.invoice = Some(Invoice {
            bolt11: "isolated".into(),
            payment_hash: "f".repeat(64),
            request_hash: "a".repeat(64),
            receiver: "03".repeat(33),
            network: "testnet".into(),
            amount_msat: 800,
            valid_until: 900,
        });
        purchase.liability = Liability::ExternalInvoice {
            merchant: "03".repeat(33),
            plugin: "plugin".into(),
            release: "release".into(),
            author: "author".into(),
            author_fee_msat: 40,
        };
        ledger.shared_reserve(&purchase).unwrap();
        ledger.shared_handoff(&purchase.id).unwrap();
        let proof = serde_json::json!({"native_wallet_receipt":"isolated-proof","merchant_settlement":"original","original_author_fee_msat":40});
        ledger
            .shared_settle(&purchase.id, 800, 25, "wallet-proof", Some(&proof), 9)
            .unwrap();
        ledger
            .shared_settle(&purchase.id, 800, 25, "wallet-proof", Some(&proof), 10)
            .unwrap();
        assert!(
            ledger
                .settlement(&format!("debit:{}", purchase.id))
                .unwrap()
                .is_none()
        );
        let balance = ledger.shared_balance(&b.id).unwrap();
        assert_eq!(
            (
                balance.available_msat,
                balance.settled_msat,
                balance.released_msat
            ),
            (175, 825, 75)
        );
    }
    #[test]
    fn explicit_migration_retains_original_liabilities_and_refuses_historical_reactivation() {
        let mut ledger = Ledger::in_memory().unwrap();
        let old = setup(&mut ledger);
        fund(&mut ledger, &old);
        let admitted = intent(&old, "original-compute", 600);
        ledger.shared_reserve(&admitted).unwrap();
        ledger.shared_handoff(&admitted.id).unwrap();
        let mut next = old.clone();
        next.id = "reviewed-retail-v2".into();
        next.commercial.revision = 2;
        next.commercial.digest = format!("sha256:{}", "b".repeat(64));
        next.commercial.workspace = "canonical-team".into();
        ledger.migrate_shared(&next, &old.digest()).unwrap();
        assert_eq!(
            ledger.shared_retail_binding("native-retail").unwrap(),
            Some(next.clone())
        );
        assert_eq!(
            ledger
                .shared_retail_outcome("original-compute")
                .unwrap()
                .unwrap()
                .intent,
            admitted
        );
        assert!(
            ledger
                .shared_reserve(&intent(&old, "old-new-effect", 1))
                .is_err()
        );
        assert!(ledger.migrate_shared(&old, &next.digest()).is_err());
        let mut foreign = next.clone();
        foreign.id = "other-customer".into();
        foreign.commercial.revision = 3;
        foreign.commercial.customer = "foreign".into();
        assert!(ledger.migrate_shared(&foreign, &next.digest()).is_err());
        let later = intent(&next, "new-compute", 400);
        ledger.shared_reserve(&later).unwrap();
        assert_eq!(
            ledger
                .shared_source_outcomes(&next, 0, None)
                .unwrap()
                .entries
                .len(),
            2
        );
        assert_eq!(ledger.shared_balance(&next.id).unwrap().available_msat, 0);
        ledger
            .shared_settle(
                &admitted.id,
                600,
                0,
                "original-native-settlement",
                None,
                1_791_000_000,
            )
            .unwrap();
        assert_eq!(
            ledger
                .shared_outcome(&admitted.id)
                .unwrap()
                .unwrap()
                .intent
                .binding,
            old
        );
        assert!(
            ledger
                .shared_release_undispatched(&admitted.id, "late-release", 10)
                .is_err()
        );
    }
    #[test]
    fn incoming_and_outgoing_callbacks_seal_once_and_never_use_an_unadmitted_intent() {
        let mut ledger = Ledger::in_memory().unwrap();
        let binding = setup(&mut ledger);
        fund(&mut ledger, &binding);
        let permit = serde_json::json!({"fixture":"exact-original-permit"});
        let authority = serde_json::json!({"native_epoch":1});
        assert!(
            ledger
                .shared_wallet_handoff(&"d".repeat(64), &permit, &authority, 4)
                .is_err()
        );
        let outgoing = intent(&binding, "outgoing", 1);
        ledger.shared_reserve(&outgoing).unwrap();
        assert!(
            ledger
                .shared_wallet_handoff(&outgoing.digest(), &permit, &authority, 4)
                .is_err()
        );
        ledger.shared_handoff(&outgoing.id).unwrap();
        ledger
            .shared_wallet_handoff(&outgoing.digest(), &permit, &authority, 5)
            .unwrap();
        assert!(
            ledger
                .shared_wallet_handoff(&outgoing.digest(), &permit, &authority, 5)
                .is_err()
        );
        ledger.shared_unknown(&outgoing.id).unwrap();
        assert!(
            ledger
                .shared_wallet_handoff(&outgoing.digest(), &permit, &authority, 6)
                .is_err()
        );
        let terms =
            serde_json::json!({"direction":"inbound","amount_msat":1000,"request":"original"});
        ledger
            .shared_begin_funding("new-original-inbound", &binding, &terms)
            .unwrap();
        ledger
            .shared_wallet_handoff(&digest(&terms), &permit, &authority, 7)
            .unwrap();
        assert!(
            ledger
                .shared_wallet_handoff(&digest(&terms), &permit, &authority, 8)
                .is_err()
        );
        let seals: i64 = ledger
            .connection
            .query_row("SELECT COUNT(*) FROM shared_wallet_handoff", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(seals, 2);
        assert_eq!(ledger.shared_balance(&binding.id).unwrap().held_msat, 1);
    }
    #[test]
    fn compact_projection_pages_preserve_one_snapshot_and_fence_a_new_allocation() {
        let mut ledger = Ledger::in_memory().unwrap();
        let binding = setup(&mut ledger);
        fund(&mut ledger, &binding);
        for index in 0..600 {
            let native = intent(&binding, &format!("native-{index}"), 1);
            ledger.shared_reserve(&native).unwrap();
            ledger
                .shared_release_undispatched(&native.id, "never-dispatched", 6)
                .unwrap();
        }
        let first = ledger.shared_source_outcomes(&binding, 0, None).unwrap();
        assert_eq!(first.entries.len(), 128);
        assert!(serde_json::to_vec(&first).unwrap().len() < BODY_MAX);
        let late = intent(&binding, "after-snapshot", 1);
        ledger.shared_reserve(&late).unwrap();
        let mut count = first.entries.len();
        let mut next = first.next;
        while let Some(after) = next {
            let page = ledger
                .shared_source_outcomes(&binding, after, Some(first.through))
                .unwrap();
            assert_eq!(page.through, first.through);
            assert!(page.entries.iter().all(|p| p.intent != late.id));
            count += page.entries.len();
            next = page.next;
        }
        assert_eq!(count, 600);
        let candidate = intent(&binding, "stale-inspected-head", 1);
        assert!(
            ledger
                .shared_reserve_projected(&candidate, first.through)
                .is_err()
        );
        assert!(ledger.shared_outcome(&candidate.id).unwrap().is_none());
        assert_eq!(
            ledger
                .shared_reserve_projected(&late, first.through)
                .unwrap()
                .intent,
            late
        );
        let fresh = ledger.shared_source_outcomes(&binding, 0, None).unwrap();
        ledger
            .shared_reserve_projected(&candidate, fresh.through)
            .unwrap();
    }
}
