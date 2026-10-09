//! Selected native retail controls over the authenticated durable service.
//! Explicit confirmation grants only the exact reviewed offer. Account reads,
//! ordinary task text, shell approvals, and reconnects never authorize spending.
mod private;
use crate::Balance;
use retail_cloud::{
    contract::{self, TaskRequest},
    dispatch::Page,
    offer::{Capacity, RetailOffer},
};
use route_contract::{Digest, digest_of};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SCHEMA: &str = "openagents.compute-retail-client.v1";
const SERVICE_SCHEMA: &str = "openagents.cloud.retail-customer.v1";
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Private(&'static str),
    #[error("{0}")]
    Refused(&'static str),
    #[error("retail service refused the operation: {0}")]
    Service(String),
    #[error("retail transport is unavailable; retry the same operation identity")]
    Transport,
    #[error("invalid bounded retail document")]
    Json(#[from] serde_json::Error),
    #[error("private retail file operation failed")]
    Io(#[from] std::io::Error),
}
/// Explicit configuration. Credentials and state never fall back to HOME,
/// environment variables, a wallet, or a paired host.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub endpoint: String,
    pub principal: String,
    pub bearer_file: PathBuf,
    pub state: PathBuf,
    pub read_only: bool,
    /// Only numeric loopback HTTP is admitted for isolated development.
    pub development_loopback: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub observe: bool,
    pub spend: bool,
    pub execute: bool,
    pub disclose: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    pub account: String,
    pub generation: i64,
    pub capabilities: Capabilities,
    pub balance: Balance,
}
impl Account {
    pub fn lines(&self) -> String {
        format!(
            "{}Account {} / generation {}\nAvailable {}\nHeld {}\nCharged {}\nUnused hold released {} (already available; not a payment refund)\nRights: observe={}, spend={}, execute={}, disclose={}",
            commercial_line(self.commercial.as_ref()),
            self.account,
            self.generation,
            crate::credits(self.balance.available_msat),
            crate::credits(self.balance.held_msat),
            crate::credits(self.balance.settled_msat),
            crate::credits(self.balance.released_msat),
            self.capabilities.observe,
            self.capabilities.spend,
            self.capabilities.execute,
            self.capabilities.disclose
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Custody {
    pub terms: Value,
    pub digest: Digest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub endpoint: String,
    pub principal: String,
    pub account: String,
    pub generation: i64,
    pub idempotency: String,
    pub offer: RetailOffer,
    pub custody: Custody,
    pub provider_key_digest: String,
    pub digest: Digest,
}
impl Review {
    fn compute_digest(&self) -> Digest {
        digest_of(&(
            SCHEMA,
            &self.endpoint,
            &self.principal,
            &self.account,
            self.generation,
            &self.idempotency,
            &self.offer,
            &self.custody,
            &self.provider_key_digest,
        ))
    }
    /// All disclosure and payment lines precede the separate confirm control.
    pub fn lines(&self) -> String {
        let o = &self.offer;
        let commercial: Option<receipts::purchase::CommercialRef> = self
            .custody
            .terms
            .get("commercial")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let mut lines = vec![
            format!("Review {}", self.digest),
            format!(
                "Authenticated service {} / principal {} / account {} / generation {}",
                self.endpoint, self.principal, self.account, self.generation
            ),
            format!(
                "Public GitHub source {} @ {}",
                o.request.source.repository, o.request.source.commit
            ),
            format!("Task {}", visible(&o.request.task)),
            format!(
                "Declared checks: {}",
                o.request
                    .checks
                    .iter()
                    .map(|s| visible(s))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            format!(
                "Computer {} / one sandbox / wall limit {} seconds",
                o.admission.computer_class, o.quote.max_seconds
            ),
            format!(
                "Effects: {}",
                serde_json::to_string(&o.admission.effects).unwrap_or_default()
            ),
            format!(
                "Material recipients: {}",
                serde_json::to_string(&o.admission.recipients).unwrap_or_default()
            ),
            format!(
                "Disclosed material: {}",
                serde_json::to_string(&o.admission.disclosed).unwrap_or_default()
            ),
            format!(
                "Model payer: {} / customer OpenAI key; customer model expense unknown and separate",
                serde_json::to_string(&o.admission.model_payer).unwrap_or_default()
            ),
            format!(
                "Customer OpenAI key SHA-256 {} (contents withheld)",
                self.provider_key_digest
            ),
            "Sponsored hosted inference: off".into(),
            format!(
                "Price book {} / {} / maximum {} credits / expires at {}",
                o.quote.version, o.quote.book, o.quote.max_sats, o.offer.expires_at
            ),
        ];
        if commercial.is_some() {
            lines.insert(2, commercial_line(commercial.as_ref()).trim_end().into());
        }
        for line in &o.quote.lines {
            lines.push(format!(
                "Resource {:?}: payer {:?}, recipient {}, basis {:?}, OpenAgents maximum {} sats",
                line.resource,
                line.payer,
                line.recipient
                    .as_deref()
                    .unwrap_or("customer provider; OpenAgents does not bill this line"),
                line.basis,
                line.max_sats
            ));
        }
        lines.extend([
            format!("Service custody {}: {}",self.custody.digest,self.custody.terms),
            "Invoice payment buys non-withdrawable compute credits. A balance grants no execution authority. Client disconnection does not stop billing.".into(),
            format!("To confirm this exact review: confirm --review {} --provider-key ABSOLUTE_PRIVATE_FILE --service-custody",self.digest),
            "Ordinary task input and shell-command approval cannot confirm this offer.".into(),
        ]);
        lines.join("\n")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Purchase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    pub purchase: String,
    pub account: String,
    pub amount_msat: i64,
    pub invoice: String,
    pub payment_hash: String,
    pub state: String,
    pub expires_at: i64,
    pub observed_at: Option<i64>,
}
impl Purchase {
    pub fn lines(&self) -> String {
        let attribution = if self.commercial.is_some() {
            format!(
                "{}Native retail account {}\n",
                commercial_line(self.commercial.as_ref()),
                self.account
            )
        } else {
            String::new()
        };
        format!(
            "{attribution}Purchase {}: {}\nAmount {}\nPay the exact invoice in your separately selected wallet; this client does not pay it.\nInvoice {}\nPayment hash {}\nExpires at {}\nPayment creates compute credits, not execution authority. Credits cannot be withdrawn as Lightning.",
            self.purchase,
            self.state,
            crate::credits(self.amount_msat),
            self.invoice,
            self.payment_hash,
            self.expires_at
        )
    }
    fn validate(self) -> Result<Self> {
        if let Some(reference) = &self.commercial {
            reference.validate().map_err(Error::Refused)?;
            if !reference.matches_native(
                receipts::purchase::CommercialProduct::Retail,
                &self.account,
                None,
            ) {
                return Err(Error::Refused(
                    "Funding attribution differs from its native account.",
                ));
            }
        }
        Ok(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub execution: String,
    pub request: String,
    pub offer: String,
    pub admission: Digest,
    pub accepted: bool,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    pub execution: String,
    pub admission: retail_cloud::authority::RetailAdmission,
    pub quote: route_contract::price_book::Quote,
    pub snapshot: Option<retail_cloud::recover::Snapshot>,
    pub hold: Option<Hold>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub state: String,
    pub held_msat: i64,
    pub charge_msat: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    pub execution: String,
    pub cancellation: Option<retail_cloud::cancel::Receipt>,
    pub retention: Option<retail_cloud::retain::Receipt>,
    pub settlement: Option<retail_cloud::settle::Receipt>,
}
impl Receipt {
    pub fn lines(&self) -> String {
        let cancel = self.cancellation.as_ref();
        let deleted = self.retention.as_ref().is_some_and(|r| r.deleted());
        let settlement = self.settlement.as_ref();
        format!(
            "{}Execution {}\nStop requested: {}\nExecutor acknowledged: {}\nSandbox deleted: {}\nUsage: {}\nCharged: {}\nUnused hold released: {} (not a payment refund)\nStill held: {}\nChecks: {}\nPayment refund: no refund record supplied\nCustomer OpenAI model expense: unknown; paid separately by the customer\nSponsored inference: off",
            commercial_line(self.commercial.as_ref()),
            self.execution,
            cancel.is_some(),
            cancel.is_some_and(|c| c.executor.is_some()),
            deleted,
            settlement
                .and_then(|s| s.usage_digest.as_deref())
                .unwrap_or("unknown; funds remain held"),
            settlement
                .and_then(|s| s.charge_msat)
                .map(crate::credits)
                .unwrap_or_else(|| "unknown; funds remain held".into()),
            settlement
                .map(|s| crate::credits(s.released_msat))
                .unwrap_or_else(|| "unknown".into()),
            settlement
                .map(|s| crate::credits(s.held_msat))
                .unwrap_or_else(|| "unknown".into()),
            settlement
                .and_then(|s| s.checks)
                .map(|v| format!("{v:?}; measured compute charges still apply"))
                .unwrap_or_else(|| "unknown".into())
        )
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: String,
    binding: Digest,
    reviews: BTreeMap<String, Pending>,
    cursors: BTreeMap<String, u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    review: Review,
    confirm_intent: bool,
    accepted: Option<Accepted>,
}
struct Secret(String);
impl Secret {
    fn read(path: &Path, max: u64) -> Result<Self> {
        let bytes = private::read(path, max)?;
        let s = String::from_utf8(bytes).map_err(|_| Error::Private("credential must be UTF-8"))?;
        let secret = Self(s);
        if secret.0.is_empty() || secret.0.bytes().any(|b| b.is_ascii_control()) {
            return Err(Error::Private(
                "credential must be nonempty without control characters",
            ));
        }
        Ok(secret)
    }
}
/// A customer's own provider key held in memory by another custody owner,
/// such as the web adapter's vault. It never prints or serializes, and its
/// bytes are zeroed on drop.
pub struct ProviderKey(Secret);
impl ProviderKey {
    /// Admit nonempty UTF-8 of at most 8 KiB without control characters.
    pub fn new(key: String) -> Result<Self> {
        let secret = Secret(key);
        if secret.0.is_empty()
            || secret.0.len() > 8192
            || secret.0.bytes().any(|b| b.is_ascii_control())
        {
            return Err(Error::Private(
                "credential must be nonempty and bounded without control characters",
            ));
        }
        Ok(Self(secret))
    }
    /// The SHA-256 a review binds; never the key itself.
    pub fn digest(&self) -> String {
        retail_cloud::sha256_hex(self.0.0.as_bytes())
    }
}
impl std::fmt::Debug for ProviderKey {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("ProviderKey(redacted)")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        let mut b = std::mem::take(&mut self.0).into_bytes();
        b.fill(0);
    }
}
pub struct Client {
    config: Config,
    http: reqwest::blocking::Client,
    bearer: Secret,
    files: private::StateFile,
    state: State,
}
impl Client {
    pub fn from_file(path: &Path) -> Result<Self> {
        Self::open(serde_json::from_slice(&private::read(path, 32 * 1024)?)?)
    }
    pub fn open(config: Config) -> Result<Self> {
        if config.schema != SCHEMA
            || config.principal.is_empty()
            || config.principal.len() > 128
            || config.principal.bytes().any(|b| b.is_ascii_control())
        {
            return Err(Error::Refused("unsupported retail client configuration"));
        }
        let url = reqwest::Url::parse(&config.endpoint)
            .map_err(|_| Error::Refused("invalid retail endpoint"))?;
        let loopback = url
            .host_str()
            .and_then(|h| h.trim_matches(['[', ']']).parse::<std::net::IpAddr>().ok())
            .is_some_and(|a| a.is_loopback());
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/v1/retail"
            || url.host_str().is_none()
            || !(url.scheme() == "https"
                || (config.development_loopback && loopback && url.scheme() == "http"))
        {
            return Err(Error::Refused(
                "use an explicit HTTPS retail endpoint; development HTTP requires numeric loopback",
            ));
        }
        let bearer = Secret::read(&config.bearer_file, 4096)?;
        let binding = digest_of(&(
            SCHEMA,
            &config.endpoint,
            &config.principal,
            &config.bearer_file,
            &config.state,
            config.read_only,
            config.development_loopback,
            Digest::of_bytes(bearer.0.as_bytes()),
        ));
        let mut files = private::StateFile::open(&config.state)?;
        let state = files.load()?.unwrap_or(State {
            schema: SCHEMA.into(),
            binding: binding.clone(),
            reviews: BTreeMap::new(),
            cursors: BTreeMap::new(),
        });
        if state.schema != SCHEMA
            || state.binding != binding
            || state.reviews.len() > 64
            || state.cursors.len() > 64
        {
            return Err(Error::Private(
                "client references belong to another credential, endpoint, or policy",
            ));
        }
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| Error::Transport)?;
        files.save(&state)?;
        Ok(Self {
            config,
            http,
            bearer,
            files,
            state,
        })
    }
    fn call<T: DeserializeOwned>(&self, request: Value) -> Result<T> {
        self.files.check()?;
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() > 32 * 1024 {
            return Err(Error::Refused("retail request exceeds its bound"));
        }
        let response = self
            .http
            .post(&self.config.endpoint)
            .bearer_auth(&self.bearer.0)
            .header("x-retail-principal", &self.config.principal)
            .header("content-type", "application/json")
            .body(bytes)
            .send()
            .map_err(|_| Error::Transport)?;
        let status = response.status();
        let mut bytes = vec![];
        response
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Transport)?;
        self.files.check()?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::Refused("retail response exceeds its bound"));
        }
        let mut value: Value = serde_json::from_slice(&bytes)?;
        if value["schema"] != SERVICE_SCHEMA {
            return Err(Error::Refused("unsupported retail service schema"));
        }
        if !status.is_success() {
            let code = value["error"]
                .as_str()
                .filter(|v| {
                    matches!(
                        *v,
                        "busy"
                            | "access_denied"
                            | "invalid_request"
                            | "terms_conflict"
                            | "insufficient_balance"
                            | "unavailable"
                    )
                })
                .unwrap_or("unavailable");
            return Err(Error::Service(code.into()));
        }
        Ok(serde_json::from_value(value["result"].take())?)
    }
    fn mutable(&self) -> Result<()> {
        if self.config.read_only {
            Err(Error::Refused("this client is observation-only"))
        } else {
            Ok(())
        }
    }
    fn authority(&self, disclosure: bool) -> Result<Account> {
        self.mutable()?;
        let account = self.account()?;
        if !account.capabilities.observe
            || !account.capabilities.spend
            || (disclosure && (!account.capabilities.execute || !account.capabilities.disclose))
        {
            return Err(Error::Refused(
                "current spend, execution, or disclosure rights are absent",
            ));
        }
        Ok(account)
    }
    pub fn account(&self) -> Result<Account> {
        let account: Account = self.call(json!({"op":"account"}))?;
        if let Some(reference) = &account.commercial {
            reference.validate().map_err(Error::Refused)?;
            if !reference.matches_native(
                receipts::purchase::CommercialProduct::Retail,
                &account.account,
                None,
            ) {
                return Err(Error::Refused(
                    "Commercial account attribution differs from the native account.",
                ));
            }
        }
        Ok(account)
    }
    pub fn capacity(&self) -> Result<Value> {
        self.call(json!({"op":"capacity"}))
    }
    pub fn top_up(&self, idempotency: &str, amount_sats: u64) -> Result<Purchase> {
        let account = self.authority(false)?;
        let purchase: Purchase =
            self.call(json!({"op":"top_up","idempotency":idempotency,"amount_sats":amount_sats}))?;
        let expected = amount_sats
            .checked_mul(1000)
            .and_then(|v| i64::try_from(v).ok());
        if purchase.account != account.account || Some(purchase.amount_msat) != expected {
            return Err(Error::Refused(
                "Funding returned another native account or amount.",
            ));
        }
        purchase.validate()
    }
    pub fn top_up_status(&self, purchase: &str) -> Result<Purchase> {
        let record: Purchase = self.call(json!({"op":"top_up_status","purchase":purchase}))?;
        if record.purchase != purchase {
            return Err(Error::Refused("Funding history returned another identity."));
        }
        record.validate()
    }
    pub fn quote(
        &mut self,
        idempotency: &str,
        task: TaskRequest,
        provider_key: &Path,
    ) -> Result<Review> {
        self.mutable()?;
        let key = ProviderKey(Secret::read(provider_key, 8192)?);
        self.quote_key(idempotency, task, &key)
    }
    /// [`Client::quote`] with a key another custody owner holds in memory.
    pub fn quote_key(
        &mut self,
        idempotency: &str,
        task: TaskRequest,
        key: &ProviderKey,
    ) -> Result<Review> {
        let account = self.authority(true)?;
        let key = &key.0;
        let key_digest = retail_cloud::sha256_hex(key.0.as_bytes());
        if let Some(pending) = self.state.reviews.get(idempotency) {
            if pending.review.offer.request != task
                || pending.review.provider_key_digest != key_digest
            {
                return Err(Error::Refused(
                    "the retry changed reviewed terms; request a new offer identity",
                ));
            }
        } else if self.state.reviews.len() >= 64 {
            return Err(Error::Refused("the private review cache is full"));
        }
        let (offer, custody) = self.fetch_offer(idempotency, &task)?;
        if offer.admission.account != account.account {
            return Err(Error::Refused("offer belongs to another service account"));
        }
        let offer_commercial: Option<receipts::purchase::CommercialRef> = custody
            .terms
            .get("commercial")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?;
        if offer_commercial != account.commercial {
            return Err(Error::Refused(
                "The current commercial account differs from the offered attribution.",
            ));
        }
        let mut review = Review {
            endpoint: self.config.endpoint.clone(),
            principal: self.config.principal.clone(),
            account: account.account,
            generation: account.generation,
            idempotency: idempotency.into(),
            offer,
            custody,
            provider_key_digest: key_digest,
            digest: Digest::of_bytes(&[]),
        };
        review.digest = review.compute_digest();
        if let Some(old) = self.state.reviews.get(idempotency) {
            if old.review != review {
                return Err(Error::Refused(
                    "terms or identity changed; a new review and offer are required",
                ));
            }
        } else {
            self.state.reviews.insert(
                idempotency.into(),
                Pending {
                    review: review.clone(),
                    confirm_intent: false,
                    accepted: None,
                },
            );
            self.files.save(&self.state)?;
        }
        Ok(review)
    }
    fn fetch_offer(&self, id: &str, task: &TaskRequest) -> Result<(RetailOffer, Custody)> {
        let mut value: Value = self.call(json!({"op":"offer","idempotency":id,"task":task}))?;
        let custody: Custody = serde_json::from_value(
            value
                .as_object_mut()
                .ok_or(Error::Refused("invalid retail offer"))?
                .remove("custody")
                .ok_or(Error::Refused("credential custody terms are absent"))?,
        )?;
        let offer: RetailOffer = serde_json::from_value(value)?;
        let expected = retail_cloud::offer::make_offer(
            &contract::price_book(),
            &offer.admission.account,
            &offer.offer.id,
            task,
            Capacity {
                running: 0,
                plan_starts_left: None,
            },
            offer.offer.created_at,
        )
        .map_err(|_| Error::Refused("unsupported retail terms"))?;
        if offer != expected
            || custody.digest != digest_of(&custody.terms)
            || custody.terms["maximum_seconds"]
                != json!(
                    offer
                        .quote
                        .max_seconds
                        .saturating_add(2 * retail_cloud::provision::READY_DEADLINE_SECS as u64)
                        .saturating_add(900)
                )
            || custody.terms["material"] != "customer-owned OpenAI API key"
            || custody.terms["schema"] != "openagents.cloud.retail-credential-custody.v1"
            || custody.terms["admission"] != serde_json::to_value(offer.admission.digest())?
        {
            return Err(Error::Refused(
                "offer, price, source, disclosure, or custody binding is invalid",
            ));
        }
        if let Some(value) = custody.terms.get("commercial") {
            let reference: receipts::purchase::CommercialRef =
                serde_json::from_value(value.clone())?;
            reference.validate().map_err(Error::Refused)?;
            if !reference.matches_native(
                receipts::purchase::CommercialProduct::Retail,
                &offer.admission.account,
                None,
            ) {
                return Err(Error::Refused(
                    "Commercial offer attribution differs from the native account.",
                ));
            }
        }
        Ok((offer, custody))
    }
    /// The exact review digest and separate custody switch are mandatory.
    /// Persist the intent before transport so a lost reply can recover only
    /// the same offer, key digest, payer, and recipient.
    pub fn confirm(
        &mut self,
        review_digest: &Digest,
        provider_key: &Path,
        service_custody: bool,
    ) -> Result<Accepted> {
        self.mutable()?;
        let key = ProviderKey(Secret::read(provider_key, 8192)?);
        self.confirm_key(review_digest, &key, service_custody)
    }
    /// [`Client::confirm`] with a key another custody owner holds in memory.
    /// The same retained review digest and explicit custody consent apply.
    pub fn confirm_key(
        &mut self,
        review_digest: &Digest,
        key: &ProviderKey,
        service_custody: bool,
    ) -> Result<Accepted> {
        let account = self.authority(true)?;
        if !service_custody {
            return Err(Error::Refused(
                "explicit service custody consent is required",
            ));
        }
        let id = self
            .state
            .reviews
            .iter()
            .find(|(_, p)| &p.review.digest == review_digest)
            .map(|(id, _)| id.clone())
            .ok_or(Error::Refused(
                "confirm requires an exact retained review digest",
            ))?;
        let p = self.state.reviews.get(&id).unwrap();
        let review = p.review.clone();
        if review.compute_digest() != *review_digest
            || account.account != review.account
            || account.generation != review.generation
        {
            return Err(Error::Refused(
                "review identity or authority generation changed",
            ));
        }
        let reviewed_commercial: Option<receipts::purchase::CommercialRef> = review
            .custody
            .terms
            .get("commercial")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?;
        if reviewed_commercial != account.commercial {
            return Err(Error::Refused(
                "The reviewed commercial attribution changed.",
            ));
        }
        let key = &key.0;
        if retail_cloud::sha256_hex(key.0.as_bytes()) != review.provider_key_digest {
            return Err(Error::Refused("the reviewed customer key changed"));
        }
        let (offer, custody) = self.fetch_offer(&id, &review.offer.request)?;
        if offer != review.offer || custody != review.custody {
            return Err(Error::Refused(
                "current offer terms changed; review a new offer",
            ));
        }
        if !p.confirm_intent && now() >= offer.offer.expires_at {
            return Err(Error::Refused("the offer expired; review a new offer"));
        }
        self.state.reviews.get_mut(&id).unwrap().confirm_intent = true;
        self.files.save(&self.state)?;
        let request = json!({"op":"confirm","offer":offer.offer.id,"digest":offer.offer.digest,"admission":offer.admission.digest(),"custody":custody.digest,"credential":{"provider":"openai","key":key.0,"service_custody":true}});
        let accepted: Accepted = self.call(request)?;
        if !accepted.accepted
            || accepted.offer != review.offer.offer.id
            || accepted.execution != review.offer.admission.execution
            || accepted.admission != review.offer.admission.digest()
        {
            return Err(Error::Refused(
                "confirmation returned another funded identity",
            ));
        }
        let p = self.state.reviews.get_mut(&id).unwrap();
        if p.accepted.as_ref().is_some_and(|old| {
            old.execution != accepted.execution || old.request != accepted.request
        }) {
            return Err(Error::Refused(
                "confirmation retry changed its funded request",
            ));
        }
        p.accepted = Some(accepted.clone());
        self.files.save(&self.state)?;
        Ok(accepted)
    }
    pub fn executions(&self, after: Option<&str>) -> Result<Value> {
        self.call(json!({"op":"executions","after":after}))
    }
    pub fn execution(&self, execution: &str) -> Result<Execution> {
        let record: Execution = self.call(json!({"op":"execution","execution":execution}))?;
        if record.execution != execution {
            return Err(Error::Refused(
                "Execution history returned another identity.",
            ));
        }
        if let Some(reference) = &record.commercial {
            reference.validate().map_err(Error::Refused)?;
            if !reference.matches_native(
                receipts::purchase::CommercialProduct::Retail,
                &record.admission.account,
                None,
            ) {
                return Err(Error::Refused(
                    "Historical commercial attribution differs from its native account.",
                ));
            }
        }
        Ok(record)
    }
    pub fn reconnect(&mut self, execution: &str) -> Result<Execution> {
        let record = self.execution(execution)?;
        if record.execution != execution || record.admission.account != self.account()?.account {
            return Err(Error::Refused(
                "reconnect returned another account or execution",
            ));
        }
        if let Some(pending) = self.state.reviews.values().find(|p| {
            p.accepted
                .as_ref()
                .is_some_and(|a| a.execution == execution)
        }) {
            if record.admission != pending.review.offer.admission
                || record.quote != pending.review.offer.quote
            {
                return Err(Error::Refused(
                    "reconnect changed the funded source, payer, or terms",
                ));
            }
        }
        if !self.state.cursors.contains_key(execution) {
            if self.state.cursors.len() >= 64 {
                return Err(Error::Refused("the private execution cursor cache is full"));
            }
            self.state.cursors.insert(execution.into(), 0);
            self.files.save(&self.state)?;
        }
        Ok(record)
    }
    pub fn progress(&mut self, execution: &str) -> Result<Page> {
        self.reconnect(execution)?;
        let after = self.state.cursors[execution];
        let page = self.progress_after(execution, after)?;
        self.state.cursors.insert(execution.into(), page.next);
        self.files.save(&self.state)?;
        Ok(page)
    }
    /// One validated progress page after a cursor the caller retains, for a
    /// custody owner (such as the web adapter) that keeps its own durable
    /// event log. The client's own cursor is unchanged.
    pub fn progress_after(&mut self, execution: &str, after: u64) -> Result<Page> {
        self.reconnect(execution)?;
        let page: Page = self.call(json!({"op":"progress","execution":execution,"after":after}))?;
        let mut previous = after;
        if page.events.len() > 128 {
            return Err(Error::Refused("oversized progress page"));
        }
        for e in &page.events {
            if e.cursor <= previous || e.text.len() > 8192 {
                return Err(Error::Refused("invalid source-bound progress cursor"));
            }
            previous = e.cursor;
        }
        if page.next != previous {
            return Err(Error::Refused("invalid progress continuation"));
        }
        Ok(page)
    }
    pub fn cancel(&self, execution: &str) -> Result<Value> {
        self.mutable()?;
        let a = self.account()?;
        if !a.capabilities.observe || !a.capabilities.execute {
            return Err(Error::Refused("current execution control is absent"));
        }
        self.call(json!({"op":"cancel","execution":execution}))
    }
    /// Offer one saved environment (`docs/cloud/retail-environment-contract.md`).
    /// The service refuses until the owner opens environments.
    pub fn environment_offer(
        &self,
        idempotency: &str,
        request: &retail_cloud::environment::EnvironmentRequest,
    ) -> Result<Value> {
        self.authority(true)?;
        self.call(json!({"op":"environment_offer","idempotency":idempotency,"request":request}))
    }
    /// Confirm the displayed environment offer: the setup runs on the
    /// month's included hours.
    pub fn environment_confirm(&self, purchase: &str, digest: &Digest) -> Result<Value> {
        self.authority(true)?;
        self.call(json!({"op":"environment_confirm","purchase":purchase,"digest":digest}))
    }
    pub fn environment(&self, purchase: &str) -> Result<Value> {
        self.call(json!({"op":"environment","purchase":purchase}))
    }
    /// Delete a saved environment version to free its storage.
    pub fn environment_delete(&self, purchase: &str) -> Result<Value> {
        self.mutable()?;
        self.call(json!({"op":"environment_delete","purchase":purchase}))
    }
    pub fn artifact(&self, execution: &str, name: &str) -> Result<Value> {
        self.call(json!({"op":"artifact","execution":execution,"name":name}))
    }
    pub fn receipt(&self, execution: &str) -> Result<Receipt> {
        let record: Receipt = self.call(json!({"op":"receipt","execution":execution}))?;
        if record.execution != execution {
            return Err(Error::Refused("Receipt history returned another identity."));
        }
        if let Some(reference) = &record.commercial {
            reference.validate().map_err(Error::Refused)?;
            if reference.source.product != receipts::purchase::CommercialProduct::Retail {
                return Err(Error::Refused(
                    "Receipt history has another product source.",
                ));
            }
            if let Some(review) = self.state.reviews.values().find(|p| {
                p.accepted
                    .as_ref()
                    .is_some_and(|a| a.execution == execution)
            }) {
                if reference.source.account != review.review.account {
                    return Err(Error::Refused("Receipt history has another native payer."));
                }
            }
        }
        Ok(record)
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
pub fn read_task(path: &Path) -> Result<TaskRequest> {
    Ok(serde_json::from_slice(&private::read(path, 32 * 1024)?)?)
}

fn visible(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn commercial_line(reference: Option<&receipts::purchase::CommercialRef>) -> String {
    reference
        .map(|r| {
            format!(
                "Commercial customer {} / workspace {} / binding {} revision {} ({})\n",
                r.customer, r.workspace, r.binding, r.revision, r.digest
            )
        })
        .unwrap_or_default()
}
