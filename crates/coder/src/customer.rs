//! Private purchase-client custody. Selection never grants gateway authority.

use crate::task;
use receipts::{
    execution::digest_request,
    purchase::{Approval, Context, MAX_QUOTE_MS, Quote},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const SCHEMA: &str = "openagents.customer-client.v1";
const MAX_STATE: usize = 2 * 1024 * 1024;
const MAX_PURCHASES: usize = 128;
const MAX_REQUEST: usize = 64 * 1024;
pub type Result<T> = std::result::Result<T, String>;

/// A selected authenticated origin and private credential alias.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub origin: String,
    pub credential_alias: String,
    pub context: Context,
}

/// An interruption leaves liability unresolved; it does not authorize a retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Quoted,
    Approved,
    Running,
    Answered,
    Refused,
    Unknown,
}

/// The response's references, without its private decision input or answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptReference {
    pub digest: String,
    pub outcome: String,
    pub settlement: String,
}

/// A scoped client view. The context is frozen attribution, not current rights.
#[derive(Clone, Debug, Serialize)]
pub struct PurchaseView {
    pub id: String,
    pub origin: String,
    pub quote: Quote,
    pub quote_digest: String,
    pub approval_digest: Option<String>,
    pub status: Status,
    pub receipt: Option<ReceiptReference>,
    pub unresolved_ceiling: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Purchase {
    selection: Selection,
    quote: Quote,
    request: Value,
    approval: Option<Approval>,
    status: Status,
    receipt: Option<ReceiptReference>,
}
impl Purchase {
    fn view(&self) -> PurchaseView {
        PurchaseView {
            id: self.quote.id.clone(),
            origin: self.selection.origin.clone(),
            quote: self.quote.clone(),
            quote_digest: self.quote.digest(),
            approval_digest: self.approval.as_ref().map(Approval::digest),
            status: self.status,
            receipt: self.receipt.clone(),
            unresolved_ceiling: matches!(self.status, Status::Running | Status::Unknown)
                .then_some(self.quote.context.price.maximum_charge),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    schema: String,
    revision: u64,
    selected: Option<Selection>,
    purchases: BTreeMap<String, Purchase>,
    #[serde(default)]
    credential_operations: BTreeMap<String, credentials::Operation>,
    #[serde(default)]
    funding: BTreeMap<String, funding::Entry>,
    #[serde(default)]
    plugin_purchases: BTreeMap<String, plugins::Purchase>,
    #[serde(default)]
    team: BTreeMap<String, team::Operation>,
}
impl Default for Book {
    fn default() -> Self {
        Self {
            schema: SCHEMA.into(),
            revision: 0,
            selected: None,
            purchases: BTreeMap::new(),
            credential_operations: BTreeMap::new(),
            funding: BTreeMap::new(),
            plugin_purchases: BTreeMap::new(),
            team: BTreeMap::new(),
        }
    }
}

fn alias(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
fn origin(value: &str) -> Result<String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid customer origin.")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || !(url.scheme() == "https"
            || url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1")))
    {
        return Err("Customer origin requires HTTPS or an explicit loopback fixture without credentials or a path.".into());
    }
    Ok(url.origin().ascii_serialization())
}
fn same_customer(a: &Selection, b: &Selection) -> bool {
    a.origin == b.origin
        && a.context.account == b.context.account
        && a.context.workspace == b.context.workspace
        && a.context.payer_workspace == b.context.payer_workspace
        && a.context.tenant == b.context.tenant
}
fn same_payer(a: &Selection, b: &Selection) -> bool {
    a.origin == b.origin
        && a.context.payer_workspace == b.context.payer_workspace
        && a.context.tenant == b.context.tenant
}
fn same_identity(a: &Context, b: &Context) -> bool {
    a.account == b.account
        && a.workspace == b.workspace
        && a.payer_workspace == b.payer_workspace
        && a.tenant == b.tenant
        && a.credential_reference == b.credential_reference
}
fn receipt_status(receipt: &ReceiptReference) -> Result<Status> {
    let hex = receipt
        .digest
        .strip_prefix("sha256:")
        .ok_or("Invalid purchase receipt reference.")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !matches!(
            receipt.outcome.as_str(),
            "answered" | "refused" | "unavailable" | "unattempted" | "unknown"
        )
        || !matches!(
            receipt.settlement.as_str(),
            "settled" | "released" | "outstanding"
        )
    {
        return Err("Invalid purchase receipt reference.".into());
    }
    Ok(
        if receipt.settlement == "outstanding" || receipt.outcome == "unknown" {
            Status::Unknown
        } else if receipt.outcome == "answered" {
            Status::Answered
        } else {
            Status::Refused
        },
    )
}
fn credential_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && (value.starts_with("oak_") || value.starts_with("sess_"))
        && !value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
}
fn selection_valid(value: &Selection) -> Result<()> {
    if origin(&value.origin)? != value.origin || !alias(&value.credential_alias) {
        return Err("Invalid customer selection.".into());
    }
    value.context.validate().map_err(str::to_owned)
}
fn request(value: &Value) -> Result<jev::SystemOneRequest> {
    if serde_json::to_vec(value)
        .map_err(|_| "Invalid private purchase request.")?
        .len()
        > MAX_REQUEST
        || value.as_object().is_none_or(|body| {
            body.len() != 3
                || ["state", "model", "questions"]
                    .iter()
                    .any(|key| !body.contains_key(*key))
        })
    {
        return Err("Purchase requests require bounded model, state, and questions fields.".into());
    }
    let decision = jev::DecisionRequest::from_value(value.clone())
        .map_err(|_| "Invalid private purchase request.")?;
    let request = jev::SystemOneRequest::from_decision(decision);
    let model = value["model"]
        .as_str()
        .ok_or("Missing purchase resource.")?;
    if Value::Object(
        request
            .body(model)
            .map_err(|_| "Invalid private purchase questions.")?,
    ) != *value
    {
        return Err("Purchase request cannot be reproduced exactly.".into());
    }
    Ok(request)
}
fn check(book: &Book) -> Result<()> {
    credentials::check_operations(&book.credential_operations)?;
    funding::check(&book.funding)?;
    plugins::check(book)?;
    team::check(&book.team)?;
    if book.schema != SCHEMA || book.purchases.len() > MAX_PURCHASES {
        return Err("Invalid customer state.".into());
    }
    if let Some(selected) = &book.selected {
        selection_valid(selected)?;
    }
    for (id, purchase) in &book.purchases {
        selection_valid(&purchase.selection)?;
        purchase.quote.validate().map_err(str::to_owned)?;
        request(&purchase.request)?;
        if id != &purchase.quote.id
            || purchase.selection.context != purchase.quote.context
            || digest_request(&purchase.request) != purchase.quote.request_digest
            || matches!(purchase.status, Status::Quoted) != purchase.approval.is_none()
        {
            return Err("Customer purchase attribution changed.".into());
        }
        if let Some(receipt) = &purchase.receipt {
            if receipt_status(receipt)? != purchase.status {
                return Err("Retained receipt and purchase status differ.".into());
            }
        } else if matches!(purchase.status, Status::Answered | Status::Refused) {
            return Err("Terminal purchase has no retained receipt.".into());
        }
        if let Some(approval) = &purchase.approval {
            if approval.quote != purchase.quote
                || approval.approved_at_ms < purchase.quote.created_at_ms
                || approval.approved_at_ms >= purchase.quote.expires_at_ms
            {
                return Err("Invalid retained purchase approval.".into());
            }
        }
    }
    Ok(())
}

/// Holds the existing private stable-file lock through each client operation.
pub struct Store {
    dir: PathBuf,
    lock: File,
    book: Book,
    poisoned: bool,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err("Customer state needs an explicit absolute directory.".into());
        }
        task::prepare_directory(root).map_err(|_| "Customer state directory is not private.")?;
        let dir = root
            .canonicalize()
            .map_err(|_| "Customer state directory is unavailable.")?;
        let keys = dir.join("credentials");
        task::prepare_directory(&keys)
            .map_err(|_| "Customer credentials directory is not private.")?;
        let lock = task::open_lock(&dir.join("customer.lock"))
            .map_err(|_| "Customer state lock is unavailable.")?;
        task::take_lock(&lock, Duration::from_secs(5))
            .map_err(|_| "Another customer operation holds this state.")?;
        let path = dir.join("state.json");
        let exists = task::regular_or_absent(&path).map_err(|_| "Unsafe customer state file.")?;
        let mut book = if exists {
            let mut bytes = Vec::new();
            task::private_open(&path, false, false)
                .map_err(|_| "Unsafe customer state file.")?
                .take(MAX_STATE as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Customer state read failed.")?;
            if bytes.len() > MAX_STATE {
                return Err("Customer state exceeds its bound.".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "Invalid customer state document.")?
        } else {
            Book::default()
        };
        check(&book)?;
        // A temporary write is never authority. Dispatch starts only after a
        // committed Running record, which recovery conservatively preserves.
        let temporary = dir.join(".state.json.tmp");
        if task::regular_or_absent(&temporary).map_err(|_| "Unsafe interrupted customer write.")? {
            let mut bytes = Vec::new();
            task::private_open(&temporary, false, false)
                .map_err(|_| "Unsafe interrupted customer write.")?
                .take(MAX_STATE as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Interrupted customer write is unreadable.")?;
            if bytes.len() > MAX_STATE {
                return Err("Interrupted customer state exceeds its bound.".into());
            }
            let pending: Book = serde_json::from_slice(&bytes)
                .map_err(|_| "Interrupted customer state needs inspection.")?;
            check(&pending)?;
            if !exists
                && (!pending.purchases.is_empty()
                    || pending.selected.is_some()
                    || !pending.team.is_empty())
            {
                return Err("Uncommitted customer history needs inspection.".into());
            }
            std::fs::remove_file(temporary)
                .map_err(|_| "Interrupted customer write cannot be cleared.")?;
        }
        let mut recovered = false;
        for purchase in book.purchases.values_mut() {
            if purchase.status == Status::Running {
                purchase.status = Status::Unknown;
                recovered = true;
            }
        }
        for operation in book.credential_operations.values_mut() {
            if operation.status == credentials::CredentialStatus::Pending {
                operation.status = credentials::CredentialStatus::Unknown;
                recovered = true;
            }
        }
        for purchase in book.plugin_purchases.values_mut() {
            if purchase.recover() {
                recovered = true;
            }
        }
        for operation in book.team.values_mut() {
            if operation.status == team::TeamStatus::Pending {
                operation.status = team::TeamStatus::Unknown;
                recovered = true;
            }
        }
        let mut store = Self {
            dir,
            lock,
            book: Book::default(),
            poisoned: false,
        };
        if !exists || recovered {
            store.persist(book)?;
        } else {
            store.book = book;
        }
        Ok(store)
    }
    fn persist(&mut self, mut book: Book) -> Result<()> {
        if self.poisoned {
            return Err("Customer state requires recovery before another operation.".into());
        }
        task::verify_same_file(&self.dir.join("customer.lock"), &self.lock)
            .map_err(|_| "Customer state lock changed.")?;
        book.revision = self
            .book
            .revision
            .max(book.revision)
            .checked_add(1)
            .ok_or("Customer revision exhausted.")?;
        check(&book)?;
        let bytes = serde_json::to_vec(&book).map_err(|_| "Customer state encoding failed.")?;
        if bytes.len() > MAX_STATE {
            return Err("Customer state exceeds its bound.".into());
        }
        if task::replace_file(&self.dir, "state.json", &bytes).is_err() {
            self.poisoned = true;
            return Err("Customer state write is uncertain; recover before continuing.".into());
        }
        self.book = book;
        Ok(())
    }
    pub fn revision(&self) -> u64 {
        self.book.revision
    }
    pub fn selected(&self) -> Option<&Selection> {
        self.book.selected.as_ref()
    }
    pub fn import_credential(&mut self, name: &str, key: &jev::ApiKey) -> Result<()> {
        if self.poisoned || !alias(name) || !credential_valid(key.expose()) {
            return Err("Invalid private account credential or alias.".into());
        }
        let path = self.dir.join("credentials").join(name);
        if task::regular_or_absent(&path).map_err(|_| "Unsafe customer credential file.")? {
            let existing = self.credential(name)?;
            if existing.expose() != key.expose() {
                return Err("Credential aliases are immutable; import a new alias after rotation or recovery.".into());
            }
            return Ok(());
        }
        let mut file = task::private_open(&path, true, true)
            .map_err(|_| "Customer credential creation failed.")?;
        file.write_all(key.expose().as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|_| "Customer credential write failed.")?;
        task::sync_directory(&self.dir.join("credentials"))
            .map_err(|_| "Customer credential directory sync failed.")?;
        Ok(())
    }
    fn credential(&self, name: &str) -> Result<jev::ApiKey> {
        if !alias(name) {
            return Err("Invalid credential alias.".into());
        }
        let mut bytes = Vec::new();
        task::private_open(&self.dir.join("credentials").join(name), false, false)
            .map_err(|_| "Customer credential is unavailable or unsafe.")?
            .take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| "Customer credential read failed.")?;
        if bytes.is_empty() || bytes.len() > 4096 {
            return Err("Invalid private customer credential.".into());
        }
        let text = String::from_utf8(bytes).map_err(|_| "Invalid private customer credential.")?;
        if !credential_valid(&text) {
            return Err("Invalid private customer credential.".into());
        }
        Ok(jev::ApiKey::new(text))
    }
    pub fn client(&self, origin_url: &str, name: &str) -> Result<jev::Client> {
        self.client_with_key(origin_url, self.credential(name)?)
    }
    fn client_with_key(&self, origin_url: &str, key: jev::ApiKey) -> Result<jev::Client> {
        let origin_url = origin(origin_url)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Customer transport is unavailable.")?;
        jev::Client::new(
            jev::Config::new()
                .base_url(origin_url)
                .api_key(key)
                .http_client(http),
        )
        .map_err(|_| "Customer transport configuration failed.".into())
    }
    pub fn bind(&mut self, selection: Selection) -> Result<()> {
        selection_valid(&selection)?;
        self.credential(&selection.credential_alias)?;
        let mut next = self.book.clone();
        next.selected = Some(selection);
        self.persist(next)
    }
    fn visible(&self, id: &str) -> Result<&Purchase> {
        let purchase = self
            .book
            .purchases
            .get(id)
            .ok_or("Purchase is unavailable.")?;
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select a customer and workspace first.")?;
        if !same_customer(selected, &purchase.selection) {
            return Err("Purchase belongs to another selected customer or workspace.".into());
        }
        Ok(purchase)
    }
    pub fn show(&self, id: &str) -> Result<PurchaseView> {
        Ok(self.visible(id)?.view())
    }
    pub fn history(&self) -> Vec<PurchaseView> {
        self.book
            .purchases
            .values()
            .filter(|purchase| {
                self.book
                    .selected
                    .as_ref()
                    .is_some_and(|selected| same_customer(selected, &purchase.selection))
            })
            .map(Purchase::view)
            .collect()
    }
    pub fn quote(
        &mut self,
        id: &str,
        body: Value,
        current: Context,
        now: u64,
    ) -> Result<PurchaseView> {
        request(&body)?;
        if self.plugin_liability() {
            return Err("An unresolved plugin payment retains this payer's liability.".into());
        }
        let selected = self
            .book
            .selected
            .clone()
            .ok_or("Select a customer and workspace first.")?;
        if !alias(id)
            || self.book.purchases.len() >= MAX_PURCHASES
            || self.book.purchases.contains_key(id)
        {
            return Err("Purchase ID is invalid, retained, or its record bound is reached.".into());
        }
        if !same_identity(&selected.context, &current)
            || selected.context.door != current.door
            || body["model"].as_str() != Some(current.door.as_str())
        {
            return Err("Current purchase identity differs from the explicit selection.".into());
        }
        if self.book.purchases.values().any(|purchase| {
            same_payer(&selected, &purchase.selection)
                && matches!(purchase.status, Status::Running | Status::Unknown)
        }) {
            return Err(
                "This payer has unresolved purchase liability; reconcile before another purchase."
                    .into(),
            );
        }
        let quote = Quote {
            id: id.into(),
            context: current.clone(),
            request_digest: digest_request(&body),
            created_at_ms: now,
            expires_at_ms: now.checked_add(MAX_QUOTE_MS).ok_or("Invalid quote time.")?,
        };
        quote.validate().map_err(str::to_owned)?;
        let purchase = Purchase {
            selection: Selection {
                context: current,
                ..selected
            },
            quote,
            request: body,
            approval: None,
            status: Status::Quoted,
            receipt: None,
        };
        let view = purchase.view();
        let mut next = self.book.clone();
        next.purchases.insert(id.into(), purchase);
        self.persist(next)?;
        Ok(view)
    }
    pub fn approve(
        &mut self,
        id: &str,
        expected_quote_digest: &str,
        current: &Context,
        now: u64,
    ) -> Result<PurchaseView> {
        let purchase = self.visible(id)?;
        if purchase.quote.digest() != expected_quote_digest || purchase.status != Status::Quoted {
            return Err("Approval needs the reviewed exact quote, once.".into());
        }
        let approval = Approval {
            quote: purchase.quote.clone(),
            approved_at_ms: now,
        };
        approval
            .validate_current(current, &purchase.quote.request_digest, now)
            .map_err(str::to_owned)?;
        let mut next = self.book.clone();
        let purchase = next.purchases.get_mut(id).unwrap();
        purchase.approval = Some(approval);
        purchase.status = Status::Approved;
        self.persist(next)?;
        self.show(id)
    }
    /// Persist Running before returning the exact frozen dispatch inputs.
    pub fn begin(
        &mut self,
        id: &str,
        current: &Context,
        now: u64,
    ) -> Result<(Selection, Approval, jev::SystemOneRequest)> {
        if self.plugin_liability() {
            return Err("An unresolved plugin payment retains this payer's liability.".into());
        }
        let purchase = self.visible(id)?;
        if purchase.status != Status::Approved {
            return Err(
                "Purchase is not approved or was already attempted; it cannot replay.".into(),
            );
        }
        let approval = purchase
            .approval
            .clone()
            .ok_or("Purchase approval is absent.")?;
        approval
            .validate_current(current, &purchase.quote.request_digest, now)
            .map_err(str::to_owned)?;
        let selected = self.book.selected.as_ref().unwrap();
        if selected.credential_alias != purchase.selection.credential_alias
            || !same_identity(&selected.context, current)
        {
            return Err("Selected credential changed; historical approval cannot move to another credential.".into());
        }
        let result = (
            purchase.selection.clone(),
            approval,
            request(&purchase.request)?,
        );
        let mut next = self.book.clone();
        next.purchases.get_mut(id).unwrap().status = Status::Running;
        self.persist(next)?;
        Ok(result)
    }
    pub fn uncertain(&mut self, id: &str) -> Result<PurchaseView> {
        if self.visible(id)?.status != Status::Running {
            return Err("Only an attempted purchase can become uncertain.".into());
        }
        let mut next = self.book.clone();
        next.purchases.get_mut(id).unwrap().status = Status::Unknown;
        self.persist(next)?;
        self.show(id)
    }
    fn complete(&mut self, id: &str, receipt: ReceiptReference) -> Result<PurchaseView> {
        if !matches!(self.visible(id)?.status, Status::Running | Status::Unknown) {
            return Err("Only an attempted purchase can receive its result.".into());
        }
        let status = receipt_status(&receipt)?;
        let mut next = self.book.clone();
        let purchase = next.purchases.get_mut(id).unwrap();
        purchase.receipt = Some(receipt);
        purchase.status = status;
        self.persist(next)?;
        self.show(id)
    }
    fn record_proof(&mut self, id: &str, proof: jev::PurchaseReceipt) -> Result<PurchaseView> {
        let purchase = self.visible(id)?;
        let context = &purchase.quote.context;
        let receipt = &proof.receipt;
        if receipt.verify().is_err()
            || receipt.request != purchase.quote.id
            || receipt.attempt != 1
            || receipt.transport != "http"
            || receipt.workspace.as_deref() != Some(&context.workspace)
            || receipt.tenant.as_deref() != Some(&context.credential_reference)
            || receipt.request_digest != purchase.quote.request_digest
            || digest_request(
                &serde_json::to_value(&receipt.requested)
                    .map_err(|_| "Receipt artifact identity cannot be represented.")?,
            ) != context.artifact_digest
            || receipt
                .registry
                .as_ref()
                .map(|registry| registry.digest.as_str())
                != Some(context.registry_digest.as_str())
        {
            return Err("Receipt does not identify the original admitted purchase.".into());
        }
        let cost = proof
            .cost
            .ok_or("Receipt has no verified settlement position; liability remains unresolved.")?;
        if cost.price_version != context.price.version
            || cost.reserved != context.price.maximum_charge
            || cost.retail.is_some_and(|amount| amount > cost.reserved)
            || cost.phase == "settled" && cost.retail.is_none()
        {
            return Err("Settlement does not match the original price and reservation.".into());
        }
        let settlement = match cost.phase.as_str() {
            "settled" => "settled",
            "released" => "released",
            "held" | "unknown" => "outstanding",
            _ => return Err("Unknown settlement position.".into()),
        };
        let outcome = match receipt.outcome {
            receipts::execution::Outcome::Answered => "answered",
            receipts::execution::Outcome::Refused => "refused",
            receipts::execution::Outcome::Unavailable => "unavailable",
            receipts::execution::Outcome::Unattempted => "unattempted",
            receipts::execution::Outcome::Unknown => "unknown",
        };
        self.complete(
            id,
            ReceiptReference {
                digest: receipt.digest.clone(),
                outcome: outcome.into(),
                settlement: settlement.into(),
            },
        )
    }
    /// Reconcile by exact receipt or a bounded scan, under current membership.
    /// A missing, truncated, or inaccessible history never releases liability.
    pub async fn reconcile(
        &mut self,
        id: &str,
        receipt_digest: Option<&str>,
    ) -> Result<PurchaseView> {
        let purchase = self.visible(id)?.clone();
        if !matches!(purchase.status, Status::Running | Status::Unknown) {
            return Err("Purchase has no unresolved attempt to reconcile.".into());
        }
        let selected = self.book.selected.as_ref().unwrap();
        let client = self.client(&selected.origin, &selected.credential_alias)?;
        let mut digest = receipt_digest.map(str::to_owned);
        if digest.is_none() {
            let mut cursor = None;
            for _ in 0..10 {
                let page = client.account().purchase_activity(&purchase.quote.context.workspace,
                    &purchase.quote.context.credential_reference, cursor.as_deref()).await
                    .map_err(|_| "Purchase activity is unavailable; original liability remains unresolved.")?;
                let matches: Vec<_> = page
                    .items
                    .iter()
                    .filter(|item| item.request == id && item.attempt == 1)
                    .collect();
                if matches.len() > 1 {
                    return Err("Purchase attempt has conflicting receipt references.".into());
                }
                if let Some(item) = matches.first() {
                    digest = Some(item.digest.clone());
                    break;
                }
                match page.cursor {
                    Some(next) if cursor.as_ref() != Some(&next) => cursor = Some(next),
                    _ => break,
                }
            }
        }
        let digest = digest.ok_or("Original receipt was not found within the bounded history; liability remains unresolved.")?;
        let proof = client.account().purchase_receipt(&purchase.quote.context.workspace, &digest).await
            .map_err(|_| "Original receipt is unavailable or unverifiable; liability remains unresolved.")?;
        self.record_proof(id, proof)
    }
    /// Dispatch once after persisting the original approval. Any uncertainty
    /// retains the reservation ceiling until an authenticated receipt resolves it.
    pub async fn invoke(
        &mut self,
        id: &str,
        now: u64,
    ) -> Result<(PurchaseView, jev::SystemOneResponse)> {
        let current = self.current_context().await?;
        let selected = self.book.selected.as_ref().unwrap();
        let client = self.client(&selected.origin, &selected.credential_alias)?;
        let (_, approval, request) = self.begin(id, &current, now)?;
        let result = client.approved_system_one(&approval, request).await;
        let response = match result {
            Ok(response) => response,
            Err(_) => {
                self.uncertain(id)?;
                return Err("Purchase response is unavailable; the attempt cannot replay and its liability needs reconciliation.".into());
            }
        };
        let digest = response
            .raw()
            .headers
            .get("x-receipt")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        // The headers locate evidence; only the authenticated receipt and
        // existing money ledger can establish the final retained status.
        self.uncertain(id)?;
        let view = match digest {
            Some(digest) => self
                .reconcile(id, Some(&digest))
                .await
                .unwrap_or(self.show(id)?),
            None => self.show(id)?,
        };
        Ok((view, response))
    }
    /// Bind only the server's authenticated selection, never a local assertion.
    pub async fn select(
        &mut self,
        origin_url: &str,
        credential_alias: &str,
        workspace: &str,
        door: &str,
    ) -> Result<Selection> {
        self.select_expected(origin_url, credential_alias, workspace, door, None)
            .await
    }
    pub async fn select_account(
        &mut self,
        origin_url: &str,
        credential_alias: &str,
        account: &str,
        workspace: &str,
        door: &str,
    ) -> Result<Selection> {
        self.select_expected(origin_url, credential_alias, workspace, door, Some(account))
            .await
    }
    async fn select_expected(
        &mut self,
        origin_url: &str,
        credential_alias: &str,
        workspace: &str,
        door: &str,
        expected: Option<&str>,
    ) -> Result<Selection> {
        let canonical_origin = origin(origin_url)?;
        let client = self.client(&canonical_origin, credential_alias)?;
        let context = client.account().purchase_context(workspace, door).await
            .map_err(|_| "Customer selection is unavailable; authentication, membership, or the selected resource must be restored.")?;
        if expected.is_some_and(|account| account != context.account) {
            return Err("Authenticated customer differs from the requested account; selection is unchanged.".into());
        }
        let selection = Selection {
            origin: canonical_origin,
            credential_alias: credential_alias.into(),
            context,
        };
        for operation in self.book.credential_operations.values() {
            if operation.command_output_alias() == Some(credential_alias)
                && (operation.command_origin() != selection.origin
                    || operation.command_account() != selection.context.account)
            {
                return Err(
                    "Issued credential differs from its original account or origin.".into(),
                );
            }
        }
        self.bind(selection.clone())?;
        Ok(selection)
    }
    pub async fn current_selection(&self) -> Result<Selection> {
        let context = self.current_context().await?;
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select a customer first.")?;
        let current = Selection {
            context,
            ..selected.clone()
        };
        if !same_customer(selected, &current) {
            return Err("Current customer differs from the retained selection.".into());
        }
        Ok(current)
    }
    async fn current_context(&self) -> Result<Context> {
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select a customer and workspace first.")?;
        self.client(&selected.origin, &selected.credential_alias)?.account()
            .purchase_context(&selected.context.workspace, &selected.context.door).await
            .map_err(|_| "Current customer rights or price are unavailable; the selection and retained liabilities are unchanged.".into())
    }
    pub async fn create_quote(&mut self, id: &str, body: Value, now: u64) -> Result<PurchaseView> {
        let current = self.current_context().await?;
        self.quote(id, body, current, now)
    }
    pub async fn approve_quote(
        &mut self,
        id: &str,
        reviewed_digest: &str,
        now: u64,
    ) -> Result<PurchaseView> {
        let current = self.current_context().await?;
        self.approve(id, reviewed_digest, &current, now)
    }
}

mod credentials;
mod funding;
pub mod plugins;
pub use credentials::{CredentialAction, CredentialCommand, CredentialStatus, CredentialView};
pub mod team;
pub use team::{TeamAction, TeamCommand, TeamStatus, TeamView};

#[cfg(test)]
mod tests;
