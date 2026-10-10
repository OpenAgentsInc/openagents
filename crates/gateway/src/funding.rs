//! Exact Lightning funding of one decision workspace through its money writer.
//! Wallet lookup establishes collection; client claims and invoices create no credit.
use fs2::FileExt;
use openagents_wallet::{
    IssuedInvoice, LightningWallet, PaymentDirection, PaymentStatus, parse_hash32,
};
use receipts::{execution::digest_request, purchase::Context};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tenancy::money::{
    Ledger, Mutation, Operation,
    funding::{self, Finality, Policy, Unit},
};

const SCHEMA: &str = "openagents.decision-funding.v1";
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_RECORDS: usize = 128;
const INVOICE_SECONDS: u32 = 900;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub state: PathBuf,
    pub wallet_home: PathBuf,
    pub receiver_node: String,
    pub network: String,
    pub policy: Policy,
    pub conversion: String,
    pub maximum_msat: u64,
}
impl Config {
    pub fn check(&self) -> Result<(), String> {
        self.policy.validate()?;
        let conversion = self
            .policy
            .conversions
            .iter()
            .find(|c| c.version == self.conversion)
            .ok_or("The selected funding conversion is unavailable.")?;
        let node = secp256k1::PublicKey::from_slice(&decode(&self.receiver_node)?)
            .map_err(|_| "Invalid funding receiver.")?;
        if !self.state.is_absolute()
            || !self.wallet_home.is_absolute()
            || self.receiver_node
                != node
                    .serialize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            || !matches!(self.network.as_str(), "bitcoin" | "testnet" | "regtest")
            || self.maximum_msat == 0
            || self.maximum_msat > 1_000_000_000_000
            || self.policy.unit
                != (Unit::CurrencyMillionths {
                    currency: "BTC".into(),
                })
            || conversion.source != Unit::Millisatoshis
            || conversion.target != self.policy.unit
            || conversion.rounding != funding::Rounding::Exact
            || conversion.max_fee_units != 0
            || self.policy.purchases.required_finality != Finality::Final
        {
            return Err("Funding requires a private explicit receiver, exact BTC conversion, and final collection.".into());
        }
        Ok(())
    }
}
fn decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("Invalid hex identity.".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|s| {
            std::str::from_utf8(s)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or("Invalid hex identity.".into())
        })
        .collect()
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub id: String,
    pub context: Context,
    pub policy: Policy,
    pub conversion: String,
    pub amount_msat: u64,
    pub credit_millionths_btc: u64,
    pub receiver_node: String,
    pub network: String,
    pub created_at: u64,
    pub expires_at: u64,
}
impl Quote {
    pub fn digest(&self) -> String {
        digest_request(&serde_json::to_value(self).expect("Funding quote serializes."))
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Quoted,
    Issuing,
    Unknown,
    Invoice,
    Funded,
    Failed,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub quote: Quote,
    pub phase: Phase,
    pub invoice: Option<IssuedInvoice>,
    pub observation: Option<openagents_wallet::PaymentRecord>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Book {
    schema: String,
    records: BTreeMap<String, Record>,
}
impl Default for Book {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            records: BTreeMap::new(),
        }
    }
}
pub struct Store {
    root: PathBuf,
    _lock: File,
    root_identity: (u64, u64),
    book: Book,
    poisoned: bool,
}
fn private(path: &Path, create: bool) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Private funding file is unavailable.")?;
    let meta = file
        .metadata()
        .map_err(|_| "Funding metadata is unavailable.")?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.nlink() != 1
        || meta.permissions().mode() & 0o077 != 0
        || meta.len() > MAX_BYTES
    {
        return Err("Funding files must be private, bounded, and ordinary.".into());
    }
    Ok(file)
}
impl Store {
    pub fn open(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Funding state needs an absolute private directory.".into());
        }
        if !root.exists() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|_| "Funding directory is unavailable.")?;
        }
        let metadata =
            std::fs::symlink_metadata(root).map_err(|_| "Funding directory is unavailable.")?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err("Funding directory must be private and ordinary.".into());
        }
        let lock = private(&root.join("lock"), true)?;
        lock.try_lock_exclusive()
            .map_err(|_| "Another funding writer holds this directory.")?;
        let mut bytes = Vec::new();
        let book = if root.join("book.json").exists() {
            private(&root.join("book.json"), false)?
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Funding history is unreadable.")?;
            serde_json::from_slice::<Book>(&bytes).map_err(|_| "Funding history is invalid.")?
        } else {
            Book::default()
        };
        if book.schema != SCHEMA || book.records.len() > MAX_RECORDS {
            return Err("Funding history exceeds its bound.".into());
        }
        let mut store = Self {
            root: root.into(),
            _lock: lock,
            root_identity: (metadata.dev(), metadata.ino()),
            book,
            poisoned: false,
        };
        for (id, record) in &mut store.book.records {
            if *id != record_key(&record.quote.context, &record.quote.id)
                || record.quote.context.validate().is_err()
            {
                return Err("Funding history has changed identity.".into());
            }
            if record.phase == Phase::Issuing {
                record.phase = Phase::Unknown;
            }
        }
        store.save()?;
        Ok(store)
    }
    fn ensure(&mut self) -> Result<(), String> {
        let root =
            std::fs::symlink_metadata(&self.root).map_err(|_| "Funding directory changed.")?;
        let retained = self
            ._lock
            .metadata()
            .map_err(|_| "Funding lock is unavailable.")?;
        let current = std::fs::symlink_metadata(self.root.join("lock"))
            .map_err(|_| "Funding lock changed.")?;
        if !root.is_dir()
            || (root.dev(), root.ino()) != self.root_identity
            || root.uid() != unsafe { libc::geteuid() }
            || root.permissions().mode() & 0o077 != 0
            || !current.is_file()
            || (current.dev(), current.ino()) != (retained.dev(), retained.ino())
            || current.nlink() != 1
            || current.permissions().mode() & 0o077 != 0
        {
            self.poisoned = true;
        }
        if self.poisoned {
            return Err("Funding writer requires recovery.".into());
        }
        Ok(())
    }
    fn save(&mut self) -> Result<(), String> {
        self.ensure()?;
        let bytes =
            serde_json::to_vec(&self.book).map_err(|_| "Funding history is not serializable.")?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Funding history exceeds its bound.".into());
        }
        let temporary = self.root.join("next.json");
        let result = (|| {
            // A retained temporary belongs to the held writer and never supersedes book.json.
            if temporary.exists() {
                private(&temporary, false).map_err(|_| ())?;
                std::fs::remove_file(&temporary).map_err(|_| ())?;
            }
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&temporary)
                .map_err(|_| ())?;
            f.write_all(&bytes)
                .and_then(|()| f.sync_all())
                .map_err(|_| ())?;
            std::fs::rename(&temporary, self.root.join("book.json")).map_err(|_| ())?;
            File::open(&self.root)
                .and_then(|f| f.sync_all())
                .map_err(|_| ())
        })();
        if result.is_err() {
            self.poisoned = true;
            return Err("Funding history write is uncertain; restart before continuing.".into());
        }
        Ok(())
    }
    pub fn read(&mut self, id: &str, context: &Context) -> Result<Record, String> {
        self.ensure()?;
        let record = self
            .book
            .records
            .get(&record_key(context, id))
            .ok_or("Funding identity is unavailable.")?;
        if !same_customer(&record.quote.context, context) {
            return Err("Funding belongs to another customer or workspace.".into());
        }
        let mut view = record.clone();
        if !matches!(view.phase, Phase::Invoice | Phase::Funded) {
            view.invoice = None;
        }
        Ok(view)
    }
    pub fn quote(
        &mut self,
        config: &Config,
        ledger: &Ledger,
        context: &Context,
        id: &str,
        amount_msat: u64,
        now: u64,
    ) -> Result<Record, String> {
        self.ensure()?;
        config.check()?;
        context.validate()?;
        if !context.can_invoke
            || !matches!(context.role.as_str(), "owner" | "admin")
            || context.price.currency != "BTC"
            || amount_msat == 0
            || amount_msat > config.maximum_msat
            || id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(
                "Funding requires current spending authority and bounded exact terms.".into(),
            );
        }
        if let Some(prior) = self.book.records.get(&record_key(context, id)) {
            if prior.quote.context != *context || prior.quote.amount_msat != amount_msat {
                return Err("Funding identity has conflicting terms.".into());
            }
            return Ok(prior.clone());
        }
        let statement = ledger.statement(&context.workspace)?;
        if statement.unit != config.policy.unit
            || ledger.funding_policy(&context.workspace) != Some(&config.policy)
        {
            return Err("The workspace has no matching current funding policy.".into());
        }
        let converted = config
            .policy
            .conversions
            .iter()
            .find(|c| c.version == config.conversion)
            .ok_or("Funding conversion is unavailable.")?
            .quote(amount_msat, 0, now)?;
        if self.book.records.len() >= MAX_RECORDS {
            return Err("Funding history has reached its bound.".into());
        }
        let record = Record {
            quote: Quote {
                id: id.into(),
                context: context.clone(),
                policy: config.policy.clone(),
                conversion: config.conversion.clone(),
                amount_msat,
                credit_millionths_btc: converted.credited_units,
                receiver_node: config.receiver_node.clone(),
                network: config.network.clone(),
                created_at: now,
                expires_at: now.checked_add(300).ok_or("Funding deadline overflow.")?,
            },
            phase: Phase::Quoted,
            invoice: None,
            observation: None,
        };
        self.book
            .records
            .insert(record_key(context, id), record.clone());
        self.save()?;
        Ok(record)
    }
    pub fn issue(
        &mut self,
        ledger: &mut Ledger,
        wallet: &dyn LightningWallet,
        context: &Context,
        id: &str,
        approved: &str,
        now: u64,
    ) -> Result<Record, String> {
        self.ensure()?;
        let record = self.read(id, context)?;
        if ledger.funding_policy(&context.workspace) != Some(&record.quote.policy) {
            return Err("Funding policy changed before invoice approval.".into());
        }
        if record.quote.context != *context
            || !context.can_invoke
            || !matches!(context.role.as_str(), "owner" | "admin")
            || approved != record.quote.digest()
        {
            return Err("Funding approval or current rights have changed.".into());
        }
        if record.phase != Phase::Quoted {
            return Ok(record);
        }
        if now < record.quote.created_at
            || now >= record.quote.expires_at
            || wallet.node_id() != record.quote.receiver_node
        {
            return Err("Funding quote expired or its receiver changed.".into());
        }
        self.book
            .records
            .get_mut(&record_key(context, id))
            .unwrap()
            .phase = Phase::Issuing;
        self.save()?;
        let began = std::time::Instant::now();
        let invoice = match wallet.receive_exact_from_node(
            &record.quote.receiver_node,
            record.quote.amount_msat,
            receive_hash(&record.quote)?,
            INVOICE_SECONDS,
        ) {
            Ok(value) => value,
            Err(_) => {
                self.book
                    .records
                    .get_mut(&record_key(context, id))
                    .unwrap()
                    .phase = Phase::Unknown;
                self.save()?;
                return Err("Invoice creation is uncertain; it cannot be replayed.".into());
            }
        };
        let completed = now.saturating_add(began.elapsed().as_secs().saturating_add(1));
        if check_invoice(&record.quote, &invoice, completed).is_err() {
            self.book
                .records
                .get_mut(&record_key(context, id))
                .unwrap()
                .phase = Phase::Unknown;
            self.save()?;
            return Err("The receiver returned conflicting invoice terms.".into());
        }
        self.book
            .records
            .get_mut(&record_key(context, id))
            .unwrap()
            .invoice = Some(invoice.clone());
        self.save()?;
        begin(ledger, &record.quote, &invoice)?;
        self.book
            .records
            .get_mut(&record_key(context, id))
            .unwrap()
            .phase = Phase::Invoice;
        self.save()?;
        self.read(id, context)
    }
    /// Reconcile original collection even after the original credential retires.
    /// Callers must authorize any projection separately; no caller supplies payment truth.
    pub fn reconcile(
        &mut self,
        ledger: &mut Ledger,
        wallet: &dyn LightningWallet,
        context: &Context,
        id: &str,
        now: u64,
    ) -> Result<Record, String> {
        self.read(id, context)?;
        self.reconcile_key(ledger, wallet, &record_key(context, id), now)
    }
    fn reconcile_key(
        &mut self,
        ledger: &mut Ledger,
        wallet: &dyn LightningWallet,
        id: &str,
        now: u64,
    ) -> Result<Record, String> {
        self.ensure()?;
        let record = self
            .book
            .records
            .get(id)
            .ok_or("Funding identity is unavailable.")?
            .clone();
        let invoice = record.invoice.as_ref().ok_or(
            "No retained invoice exists; unknown issuance requires operator reconciliation.",
        )?;
        check_invoice(&record.quote, invoice, now)?;
        if wallet.node_id() != record.quote.receiver_node {
            return Err("Funding receiver changed.".into());
        }
        let began = std::time::Instant::now();
        let payment = wallet
            .lookup_from_node(
                &record.quote.receiver_node,
                parse_hash32(&invoice.payment_hash).map_err(|_| "Invalid payment hash.")?,
            )
            .map_err(|_| "Funding lookup is unavailable; no credit was posted.")?;
        self.ensure()?;
        let observed_at = now.saturating_add(began.elapsed().as_secs().saturating_add(1));
        let Some(mut payment) = payment else {
            return Ok(record);
        };
        if payment.payment_hash != invoice.payment_hash
            || payment.direction != PaymentDirection::Inbound
            || payment.updated_at > observed_at
        {
            return Err("Funding observation changes collection identity.".into());
        }
        if payment.status == PaymentStatus::Pending {
            return Ok(record);
        }
        if payment.status == PaymentStatus::Failed {
            if record.phase == Phase::Funded {
                return Err("Confirmed collection cannot become failed.".into());
            }
            self.book.records.get_mut(id).unwrap().phase = Phase::Failed;
            self.save()?;
            return Ok(self.book.records[id].clone());
        }
        if record.phase == Phase::Failed {
            return Err("Failed collection cannot become confirmed.".into());
        }
        let decoded = nostr::x402::decode_invoice(&invoice.bolt11)
            .map_err(|_| "Invalid retained invoice.")?;
        if payment.amount_msat != Some(record.quote.amount_msat)
            || payment
                .bolt11
                .as_ref()
                .is_some_and(|observed| observed != &invoice.bolt11)
            || payment.updated_at < decoded.created_at()
            || payment.updated_at
                >= decoded
                    .created_at()
                    .checked_add(decoded.expiry_seconds())
                    .ok_or("Invoice deadline overflow.")?
        {
            return Err(
                "Funding collection changes its original amount, invoice, or deadline.".into(),
            );
        }
        if let Some(preimage) = &payment.preimage {
            use sha2::{Digest, Sha256};
            if <[u8; 32]>::from(Sha256::digest(
                parse_hash32(preimage).map_err(|_| "Invalid funding proof.")?,
            )) != decoded.payment_hash()
            {
                return Err("Funding proof conflicts with the invoice.".into());
            }
        }
        payment.preimage = None;
        if record
            .observation
            .as_ref()
            .is_some_and(|prior| prior != &payment)
        {
            return Err("Confirmed funding observation cannot change.".into());
        }
        begin(ledger, &record.quote, invoice)?;
        ledger.apply(Mutation {
            workspace: record.quote.context.workspace.clone(),
            source: format!("decision-funding:{}:final", record.quote.digest()),
            audit: record.quote.digest(),
            operation: Operation::FundingFinality {
                funding: record.quote.digest(),
                finality: Finality::Final,
                evidence: digest_request(
                    &serde_json::to_value(&payment)
                        .map_err(|_| "Funding evidence is unavailable.")?,
                ),
            },
        })?;
        let entry = self.book.records.get_mut(id).unwrap();
        entry.phase = Phase::Funded;
        entry.observation = Some(payment);
        self.save()?;
        Ok(self.book.records[id].clone())
    }
}
fn record_key(context: &Context, id: &str) -> String {
    digest_request(
        &serde_json::json!({"account":context.account,"workspace":context.workspace,"payer":context.payer_workspace,"tenant":context.tenant,"id":id}),
    )
}
fn same_customer(a: &Context, b: &Context) -> bool {
    a.account == b.account
        && a.workspace == b.workspace
        && a.payer_workspace == b.payer_workspace
        && a.tenant == b.tenant
}
fn receive_hash(quote: &Quote) -> Result<[u8; 32], String> {
    parse_hash32(
        quote
            .digest()
            .strip_prefix("sha256:")
            .ok_or("Invalid funding digest.")?,
    )
    .map_err(|_| "Invalid funding quote digest.".into())
}
fn begin(ledger: &mut Ledger, quote: &Quote, invoice: &IssuedInvoice) -> Result<(), String> {
    ledger.apply(Mutation {
        workspace: quote.context.workspace.clone(),
        source: format!("decision-funding:{}:begin", quote.digest()),
        audit: quote.digest(),
        operation: Operation::BeginFunding {
            funding: funding::Funding {
                id: quote.digest(),
                origin: quote.receiver_node.clone(),
                payment: format!("lightning:{}:{}", quote.network, invoice.payment_hash),
                policy: quote.policy.version.clone(),
                conversion: quote.conversion.clone(),
                gross_units: quote.amount_msat,
                fee_units: 0,
            },
        },
    })?;
    Ok(())
}
fn check_invoice(quote: &Quote, invoice: &IssuedInvoice, now: u64) -> Result<(), String> {
    let parsed = nostr::x402::decode_invoice(&invoice.bolt11)
        .map_err(|_| "Invalid signed funding invoice.")?;
    let network = match quote.network.as_str() {
        "bitcoin" => "bc",
        "testnet" => "tb",
        "regtest" => "bcrt",
        _ => return Err("Unsupported funding network.".into()),
    };
    if parsed.currency() != network
        || parsed.amount_msat() != quote.amount_msat
        || parsed.description_hash() != receive_hash(quote)?
        || parsed
            .payee()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            != quote.receiver_node
        || invoice.pay_to != quote.receiver_node
        || invoice.amount_msat != quote.amount_msat
        || invoice.description_hash != quote.digest().strip_prefix("sha256:").unwrap_or("")
        || parse_hash32(&invoice.payment_hash).map_err(|_| "Invalid funding payment identity.")?
            != parsed.payment_hash()
        || parsed.created_at() > now
        || parsed.created_at() < quote.created_at
        || parsed.expiry_seconds() != u64::from(INVOICE_SECONDS)
        || invoice.expiry_secs != INVOICE_SECONDS
    {
        return Err("Funding invoice differs from its exact approved terms.".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "funding_tests.rs"]
mod tests;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Quote { id: String, amount_msat: u64 },
    Issue { id: String, approved: String },
    Read { id: String },
    Reconcile { id: String },
}
pub(crate) fn routes() -> Vec<(
    &'static str,
    axum::routing::MethodRouter<std::sync::Arc<crate::serve::ServeState>>,
)> {
    let route =
        || axum::routing::post(handle).layer(axum::extract::DefaultBodyLimit::max(32 * 1024));
    vec![
        // A Lightning top-up (#11161): the product word for it.
        ("/v1/workspaces/{workspace}/topups/{door}", route()),
        // Its older name, until the two newest clients stop calling it.
        (
            "/v1/workspaces/{workspace}/decision-funding/{door}",
            crate::envelope::deprecated(route(), "/decision-funding/", "/topups/"),
        ),
    ]
}
fn current(
    state: &crate::serve::ServeState,
    headers: &axum::http::HeaderMap,
    door: &str,
) -> Result<Context, axum::response::Response> {
    let (_registry, caller) = crate::serve::authenticate(state, headers)
        .map_err(|(status, code, message)| crate::accounts::refused(status, code, &message))?;
    if caller
        .scopes
        .as_ref()
        .is_some_and(|s| !s.permits_action("balance"))
    {
        return Err(crate::accounts::refused(
            axum::http::StatusCode::FORBIDDEN,
            "out_of_scope",
            "Funding requires balance visibility.",
        ));
    }
    crate::purchase::current(state, headers, door)
}
async fn handle(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::serve::ServeState>>,
    axum::extract::Path((workspace, door)): axum::extract::Path<(String, String)>,
    mut headers: axum::http::HeaderMap,
    axum::Json(request): axum::Json<Request>,
) -> axum::response::Response {
    use axum::{http::StatusCode, response::IntoResponse};
    let Ok(permit) = state.funding_slots.clone().try_acquire_owned() else {
        return crate::accounts::refused(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "The funding queue is full.",
        );
    };
    if headers.get_all("x-workspace-id").iter().count() > 1
        || headers
            .get("x-workspace-id")
            .is_some_and(|v| v.as_bytes() != workspace.as_bytes())
    {
        return crate::accounts::refused(
            StatusCode::CONFLICT,
            "funding_changed",
            "The workspace selection is ambiguous.",
        );
    }
    let Ok(value) = workspace.parse() else {
        return crate::accounts::refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid funding workspace.",
        );
    };
    headers.insert("x-workspace-id", value);
    if let Err(response) = current(&state, &headers, &door) {
        return response;
    }
    let result=tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // Current membership is reopened after acquiring the durable writers.
        let mut store=state.funding.as_ref().ok_or("Funding is unavailable.")?.lock().map_err(|_| "Funding writer is unavailable.")?;
        let mut ledger=state.money_blocking_lock().ok_or("Monetary admission is unavailable.")?;
        let context=current(&state,&headers,&door).map_err(|_| "Current funding membership is unavailable.")?;
        let config=state.config.funding.as_ref().ok_or("Funding is unavailable.")?;
        let now=unix_now();
        let id=match &request {Request::Quote{id,..}|Request::Issue{id,..}|Request::Read{id}|Request::Reconcile{id}=>id};
        match &request {
            Request::Quote{amount_msat,..}=>{store.quote(config,&ledger,&context,id,*amount_msat,now)?;}
            Request::Read{..}=>{store.read(id,&context)?;}
            Request::Issue{approved,..}=>{
                let wallet=receiver(config).ok_or("The admitted receiver is unavailable.")?;
                let fresh=current(&state,&headers,&door).map_err(|_| "Current funding membership is unavailable.")?;
                store.issue(&mut ledger,&wallet,&fresh,id,approved,unix_now())?;
            }
            Request::Reconcile{..}=>{
                store.read(id,&context)?;
                let wallet=receiver(config).ok_or("The admitted receiver is unavailable.")?;
                store.reconcile(&mut ledger,&wallet,&context,id,unix_now())?;
            }
        }
        let fresh=current(&state,&headers,&door).map_err(|_| "Current funding membership is unavailable; original collection remains retained.")?;
        let record=store.read(id,&fresh)?;
        Ok::<_,String>(serde_json::json!({"schema":SCHEMA,"quote_digest":record.quote.digest(),"record":record,"balance":ledger.balance(&fresh.workspace)?,"wallet_liquidity":"unknown","earned_usage":false,"production_qualification":"owner_required_O5_O8"}))
    }).await;
    let mut response = match result {
        Ok(Ok(value)) => axum::Json(value).into_response(),
        Ok(Err(message)) => {
            crate::accounts::refused(StatusCode::CONFLICT, "funding_unavailable", &message)
        }
        Err(_) => crate::accounts::refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "funding_unavailable",
            "Funding reconciliation is unavailable; its original state remains retained.",
        ),
    };
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
fn receiver(config: &Config) -> Option<openagents_wallet::resident::RemoteWallet> {
    let metadata = std::fs::symlink_metadata(&config.wallet_home).ok()?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return None;
    }
    let wallet = openagents_wallet::resident::RemoteWallet::probe(&config.wallet_home)?;
    (wallet.node_id() == config.receiver_node).then_some(wallet)
}
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
/// A bounded resident pass reconciles original invoices without needing the retired caller.
pub(crate) fn resume(state: &std::sync::Arc<crate::serve::ServeState>) {
    if state.config.funding.is_none() {
        return;
    }
    let weak = std::sync::Arc::downgrade(state);
    tokio::spawn(async move {
        let mut cursor = 0usize;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            let Some(state) = weak.upgrade() else {
                break;
            };
            let result = tokio::task::spawn_blocking(move || {
                let Some(config) = state.config.funding.as_ref() else {
                    return cursor;
                };
                let Some(service) = state.funding.as_ref() else {
                    return cursor;
                };
                let Ok(mut store) = service.lock() else {
                    return cursor;
                };
                let Some(mut ledger) = state.money_blocking_lock() else {
                    return cursor;
                };
                let Some(wallet) = receiver(config) else {
                    return cursor;
                };
                let ids: Vec<_> = store
                    .book
                    .records
                    .iter()
                    .filter(|(_, r)| {
                        r.invoice.is_some()
                            && matches!(r.phase, Phase::Invoice | Phase::Unknown | Phase::Issuing)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                if ids.is_empty() {
                    return 0;
                }
                for offset in 0..ids.len().min(2) {
                    let id = &ids[(cursor + offset) % ids.len()];
                    let _ = store.reconcile_key(&mut ledger, &wallet, id, unix_now());
                }
                (cursor + 2) % ids.len()
            })
            .await;
            if let Ok(next) = result {
                cursor = next;
            }
        }
    });
}
