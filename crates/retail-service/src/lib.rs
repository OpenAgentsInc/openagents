//! Authenticated customer transport and an independent durable retail worker.
//! Money, execution identities, counters, cleanup, and settlement remain in
//! the existing pay-ledger and retail-cloud records.

pub mod backup;
pub mod commercial;
mod custody;
pub mod http;
pub mod package;
mod store;
pub mod types;
mod worker;

pub use store::private_dir as store_private_dir;

use openagents_wallet::LightningWallet;
use pay_ledger::compute::{Need, Principal, credential_digest};
use retail_cloud::{
    authority::{Current, DisclosureConsent, ExecuteGrant, GrantSource, ObserveGrant, SpendRight},
    cancel, contract, dispatch, environment, material,
    offer::{self, Capacity, ConfirmedVia, FundedRequest},
    provision::Provider,
    retain::Artifacts,
    topup,
};
use retail_qualify::launch::{self, Gate};
use route_contract::digest_of;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};
use store::{Store, vault_read, vault_write};
use types::{Config, Confirmation, Request, RetailGrant};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("retail offer is unavailable")]
    Offer(offer::OfferRefusal),
    #[error("retail access denied")]
    Denied,
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("the compute balance cannot cover this offer")]
    Insufficient,
    #[error("retail lifecycle operation failed")]
    Lifecycle(#[from] retail_cloud::Error),
    #[error("retail ledger operation failed")]
    Ledger(#[from] pay_ledger::Error),
    #[error("private state operation failed")]
    Io(#[from] std::io::Error),
    #[error("private record operation failed")]
    Sql(#[from] rusqlite::Error),
    #[error("invalid retail document")]
    Json(#[from] serde_json::Error),
}

/// A single binding owns all five lifecycle seams for the same resource.
pub trait Backend:
    Provider
    + material::Sandbox
    + dispatch::TaskOwner
    + cancel::StopOwner
    + Artifacts
    + Send
    + Sync
    + 'static
{
}
impl<
    T: Provider
        + material::Sandbox
        + dispatch::TaskOwner
        + cancel::StopOwner
        + Artifacts
        + Send
        + Sync
        + 'static,
> Backend for T
{
}

pub struct Service<B: Backend, W: LightningWallet + Send + Sync + 'static> {
    pub(crate) config: Config,
    pub(crate) operations: Option<Arc<package::Operating>>,
    pub(crate) commercial: Option<commercial::Mapping>,
    pub(crate) store: Mutex<Store>,
    pub(crate) backend: Arc<custody::GuardBackend<B>>,
    pub(crate) wallet: Arc<custody::GuardWallet<W>>,
}

impl<B: Backend, W: LightningWallet + Send + Sync + 'static> Service<B, W> {
    /// Open only explicitly configured private records. This does not create
    /// an account, bind a principal, grant authority, or qualify paid capacity.
    pub fn open(config: Config, backend: Arc<B>, wallet: Arc<W>) -> Result<Self> {
        if config.schema != types::SCHEMA
            || config.template.is_empty()
            || config.template.len() > 128
            || config.grants.len() > 256
        {
            return Err(Error::Invalid("unsupported retail configuration"));
        }
        let date = config
            .template
            .strip_prefix("oa-coder-main-")
            .ok_or(Error::Invalid(
                "only the supported daily Coder template is admitted",
            ))?;
        // Native Boat daily templates use YYYYMMDD. Preserve earlier dated
        // fixture names without changing an already admitted execution.
        let compact = if date.len() == 10
            && date.as_bytes().get(4) == Some(&b'-')
            && date.as_bytes().get(7) == Some(&b'-')
        {
            date.replace('-', "")
        } else {
            date.to_owned()
        };
        if compact.len() != 8
            || !compact.bytes().all(|b| b.is_ascii_digit())
            || compact[4..6]
                .parse::<u8>()
                .map_or(true, |n| !(1..=12).contains(&n))
            || compact[6..8]
                .parse::<u8>()
                .map_or(true, |n| !(1..=31).contains(&n))
        {
            return Err(Error::Invalid(
                "a supported daily template date is required",
            ));
        }
        let mut principals = BTreeSet::new();
        if config.grants.iter().any(|g| {
            g.principal.is_empty()
                || g.account.is_empty()
                || g.generation < 1
                || !principals.insert(g.principal.clone())
        }) {
            return Err(Error::Invalid("invalid or duplicate retail grant"));
        }
        let store = Store::open(&config.state, &config.ledger)?;
        let node = wallet.node_id();
        if node.len() != 66
            || !(node.starts_with("02") || node.starts_with("03"))
            || !node
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid("a normalized receiver node is required"));
        }
        let node = store.receiver(&node)?;
        let backend = Arc::new(custody::GuardBackend {
            inner: backend,
            custody: Arc::clone(&store.custody),
        });
        let wallet = Arc::new(custody::GuardWallet {
            inner: wallet,
            custody: Arc::clone(&store.custody),
            node,
        });
        Ok(Self {
            config,
            operations: None,
            commercial: None,
            store: Mutex::new(store),
            backend,
            wallet,
        })
    }
    /// Enable the exact persistent shared binding; this does not grant native execution.
    pub fn with_shared_spend(self, config: pay_ledger::shared::ClientConfig) -> Result<Self> {
        let mut store = self.lock()?;
        store.ledger.configure_shared_client(config)?;
        let client = store
            .ledger
            .shared_adapter_call(pay_ledger::shared::Operation::Binding {})?;
        let binding: pay_ledger::shared::Binding =
            serde_json::from_value(client).map_err(|_| Error::Denied)?;
        if self.wallet.node_id() != binding.custodian_node {
            return Err(Error::Denied);
        }
        drop(store);
        Ok(self)
    }
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Store>> {
        let store = self
            .store
            .lock()
            .map_err(|_| Error::Unavailable("private retail state is unavailable"))?;
        store.check()?;
        Ok(store)
    }
    fn authenticate(
        &self,
        store: &Store,
        principal: &str,
        secret: &str,
        need: Need,
    ) -> Result<(Principal, RetailGrant)> {
        if principal.is_empty() || principal.len() > 128 || secret.is_empty() || secret.len() > 4096
        {
            return Err(Error::Denied);
        }
        let resolved = store
            .ledger
            .resolve_principal(principal, &credential_digest(secret), need)
            .map_err(|_| Error::Denied)?;
        let grant = self
            .config
            .grants
            .iter()
            .find(|g| {
                g.principal == resolved.id
                    && g.account == resolved.account
                    && g.generation == resolved.generation
            })
            .ok_or(Error::Denied)?;
        if !grant.observe {
            return Err(Error::Denied);
        }
        Ok((resolved, grant.clone()))
    }
    pub(crate) fn capacity(&self, store: &Store, exclude: Option<&str>) -> Result<Capacity> {
        let mut query = store
            .db
            .prepare("SELECT id,bytes FROM offer WHERE confirmation IS NOT NULL ORDER BY id")?;
        let rows = query
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let total = rows.len();
        let mut running = 0;
        for (id, bytes) in rows {
            if exclude == Some(id.as_str()) {
                continue;
            }
            let offer: offer::RetailOffer = serde_json::from_str(&bytes)?;
            let execution = &offer.admission.execution;
            let deleted = store
                .journal
                .retention_receipt(execution, 0)?
                .is_some_and(|r| r.deleted());
            let settled = store
                .ledger
                .hold_for_execution(execution)?
                .is_some_and(|h| h.state == pay_ledger::compute::HoldState::Settled);
            if !deleted && !settled {
                running += 1;
            }
        }
        Ok(Capacity {
            running,
            plan_starts_left: self.config.plan_starts_left.map(|n| {
                n.saturating_sub(
                    u32::try_from(total.saturating_sub(usize::from(exclude.is_some())))
                        .unwrap_or(u32::MAX),
                )
            }),
        })
    }
    fn advertisement(&self, store: &Store, exclude: Option<&str>) -> Result<launch::Advertisement> {
        let qualification = match &self.operations {
            Some(ops) => ops.gate(&*self.wallet).ok().flatten(),
            None => self.config.qualification.clone(),
        };
        let healthy = self.operations.is_none()
            || (launch::health(&store.journal, &store.ledger, http::now())?.is_empty()
                && self
                    .operations
                    .as_ref()
                    .is_some_and(|o| o.worker_healthy(http::now())));
        let shared_ready = store.ledger.shared_owner()?.is_none()
            || store
                .ledger
                .shared_adapter_call(pay_ledger::shared::Operation::Binding {})
                .ok()
                .and_then(|value| serde_json::from_value::<pay_ledger::shared::Binding>(value).ok())
                .is_some_and(|binding| {
                    binding.custodian_node == self.wallet.node_id()
                        && self
                            .config
                            .grants
                            .iter()
                            .any(|grant| grant.account == binding.source.account)
                });
        let mut advertisement = launch::advertise(&Gate {
            contract_confirmed: self.config.contract_confirmed,
            qualification,
            supported_plan: self.config.supported_plan.clone(),
            capacity: self.capacity(store, exclude)?,
        });
        if !healthy || !shared_ready {
            advertisement.paid_capacity = None;
            advertisement.closed = Some(launch::Closed::OperationalIncident);
        }
        Ok(advertisement)
    }
    /// The owner's environment launch, or closed.
    fn environments(&self) -> Result<&types::EnvironmentLaunch> {
        self.config
            .environments
            .as_ref()
            .ok_or(Error::Unavailable("saved environments are not available"))
    }
    fn require_open(&self, store: &Store, exclude: Option<&str>) -> Result<()> {
        if self.advertisement(store, exclude)?.paid_capacity.is_none() {
            return Err(Error::Unavailable("paid retail capacity is closed"));
        }
        Ok(())
    }
    fn funded_for(&self, store: &Store, account: &str, execution: &str) -> Result<FundedRequest> {
        let funded = store
            .journal
            .funded(execution)?
            .filter(|f| f.account == account)
            .ok_or(Error::Denied)?;
        Ok(funded)
    }
    fn read_current(funded: &FundedRequest) -> Current {
        Current {
            observe: Some(ObserveGrant {
                account: funded.account.clone(),
                execution: funded.execution.clone(),
                revoked: false,
            }),
            ..Current::default()
        }
    }
    pub(crate) fn current(
        &self,
        store: &Store,
        funded: &FundedRequest,
        confirmation: &Confirmation,
    ) -> Result<Current> {
        store.check()?;
        // Authentication is not replayed from a stored bearer: the ledger's
        // current principal, revocation epoch, and independently configured
        // retail grant are checked before each worker side effect.
        let principal = store
            .ledger
            .principals(&funded.account)?
            .into_iter()
            .find(|p| p.id == confirmation.principal);
        let grant = self.config.grants.iter().find(|g| {
            g.principal == confirmation.principal
                && g.account == funded.account
                && g.generation == confirmation.generation
        });
        let active = principal
            .as_ref()
            .is_some_and(|p| p.generation == confirmation.generation && p.revoked_at.is_none());
        let mapped =
            self.commercial_current(store, &funded.account, confirmation.commercial.as_ref());
        let execute = active && mapped && grant.is_some_and(|g| g.execute && g.observe);
        let disclose = active && mapped && grant.is_some_and(|g| g.disclose);
        Ok(Current {
            observe: Some(ObserveGrant {
                account: funded.account.clone(),
                execution: funded.execution.clone(),
                revoked: !active || !grant.is_some_and(|g| g.observe),
            }),
            execute: Some(ExecuteGrant {
                source: GrantSource::Retail,
                execution: funded.execution.clone(),
                generation: funded.admission.grant_generation,
                revoked: !execute,
            }),
            disclose: Some(DisclosureConsent {
                admission: confirmation.admission.clone(),
                withdrawn: !disclose,
            }),
            spend: principal
                .filter(|p| active && mapped && p.rights.spend)
                .map(|_| SpendRight {
                    account: funded.account.clone(),
                }),
            ..Current::default()
        })
    }
    /// One bounded HTTP operation. Call on a blocking thread; the live Boat
    /// adapter owns a blocking runtime. Responses contain no provider secrets.
    pub fn call(&self, principal: &str, secret: &str, request: Request, now: i64) -> Result<Value> {
        if now < 0 {
            return Err(Error::Invalid("invalid service time"));
        }
        let mut guard = self.lock()?;
        let store = &mut *guard;
        let need = match request {
            Request::TopUp { .. }
            | Request::Offer { .. }
            | Request::Confirm { .. }
            | Request::EnvironmentOffer { .. }
            | Request::EnvironmentConfirm { .. }
            | Request::EnvironmentRenew { .. } => Need::Spend,
            _ => Need::Read,
        };
        let (identity, grant) = self.authenticate(&store, principal, secret, need)?;
        let value = match request {
            Request::Account {} => {
                let b = store.ledger.compute_balance(&identity.account)?;
                let commercial = self.commercial_ref(store, &identity.account)?;
                let mut value = json!({"account":identity.account,"generation":identity.generation,"capabilities":{"observe":grant.observe,"spend":identity.rights.spend,"execute":grant.execute,"disclose":grant.disclose},"balance":b});
                if let Some(commercial) = commercial {
                    value["commercial"] = serde_json::to_value(commercial)?;
                }
                value
            }
            Request::Capacity {} => serde_json::to_value(self.advertisement(&store, None)?)?,
            Request::TopUp {
                idempotency,
                amount_sats,
            } => {
                let current = self.commercial_ref(store, &identity.account)?;
                self.require_open(&store, None)?;
                let purchase = opaque("rt", &identity.account, &idempotency)?;
                let amount_msat = msat(amount_sats)?;
                if amount_msat <= 0 || amount_msat > pay_ledger::compute::purchase::TOP_UP_MAX_MSAT
                {
                    return Err(Error::Invalid("top-up amount"));
                }
                let commercial = if let Some(original) = store.ledger.top_up(&purchase)? {
                    store.funding_commercial(
                        &purchase,
                        &identity.account,
                        original.top_up.amount_msat,
                    )?
                } else {
                    store.freeze_funding_commercial(
                        &purchase,
                        &identity.account,
                        amount_msat,
                        current.as_ref(),
                    )?
                };
                let record = topup::request_top_up(
                    &mut store.ledger,
                    &*self.wallet,
                    &topup::TopUpRequest {
                        principal: identity.id,
                        credential: credential_digest(secret),
                        purchase,
                        amount_sats,
                        now,
                    },
                )?;
                purchase_value(record, commercial.as_ref())
            }
            Request::TopUpStatus { purchase } => {
                if store
                    .ledger
                    .shared_retail_binding(&identity.account)?
                    .is_some()
                {
                    let value = store.ledger.shared_adapter_call(
                        pay_ledger::shared::Operation::FundingStatus {
                            purchase: purchase.clone(),
                        },
                    )?;
                    let amount = value["original_terms"]["amount_msat"]
                        .as_u64()
                        .ok_or(Error::Denied)?
                        / 1000;
                    let request = retail_cloud::topup::TopUpRequest {
                        principal: identity.id.clone(),
                        credential: String::new(),
                        purchase: purchase.clone(),
                        amount_sats: amount,
                        now,
                    };
                    let record =
                        retail_cloud::topup::shared_purchase(&identity.account, &request, value)?;
                    let commercial = store.funding_commercial(
                        &purchase,
                        &identity.account,
                        record.top_up.amount_msat,
                    )?;
                    purchase_value(record, commercial.as_ref())
                } else {
                    let record = store
                        .ledger
                        .top_up(&purchase)?
                        .filter(|p| p.top_up.account == identity.account)
                        .ok_or(Error::Denied)?;
                    let commercial = store.funding_commercial(
                        &purchase,
                        &identity.account,
                        record.top_up.amount_msat,
                    )?;
                    purchase_value(record, commercial.as_ref())
                }
            }
            Request::Offer { idempotency, task } => {
                if !grant.execute || !grant.disclose {
                    return Err(Error::Denied);
                }
                let commercial = self.commercial_ref(store, &identity.account)?;
                let id = opaque("ro", &identity.account, &idempotency)?;
                if let Some((made, owner, generation, _)) = store.offer(&id)? {
                    if made.request != task
                        || owner != identity.id
                        || generation != identity.generation
                    {
                        return Err(Error::Conflict(
                            "the offer retry changed its identity or terms",
                        ));
                    }
                    if store.commercial(&id)? != commercial {
                        return Err(Error::Conflict(
                            "the reviewed commercial attribution changed",
                        ));
                    }
                    offer_value(&made, commercial.as_ref())
                } else {
                    self.require_open(&store, None)?;
                    let made = offer::make_offer(
                        &contract::price_book(),
                        &identity.account,
                        &id,
                        &task,
                        self.capacity(&store, None)?,
                        now as u64,
                    )?;
                    store.insert_offer(
                        &made,
                        &identity.id,
                        identity.generation,
                        now,
                        commercial.as_ref(),
                    )?;
                    offer_value(&made, commercial.as_ref())
                }
            }
            Request::Confirm {
                offer: id,
                digest,
                admission,
                custody,
                credential,
            } => {
                if !grant.execute || !grant.disclose || !credential.service_custody {
                    return Err(Error::Denied);
                }
                let (made, owner, generation, existing) = store
                    .offer(&id)?
                    .filter(|(o, _, _, _)| o.admission.account == identity.account)
                    .ok_or(Error::Denied)?;
                if owner != identity.id || generation != identity.generation {
                    return Err(Error::Denied);
                }
                let commercial = store.commercial(&id)?;
                if !self.commercial_current(store, &identity.account, commercial.as_ref()) {
                    return Err(Error::Denied);
                }
                if digest != made.offer.digest || admission != made.admission.digest() {
                    return Err(Error::Conflict(
                        "confirmation differs from the displayed offer and admission",
                    ));
                }
                if credential.provider != "openai"
                    || credential.key.is_empty()
                    || credential.key.len() > types::KEY_MAX
                    || credential.key.bytes().any(|c| c.is_ascii_control())
                {
                    return Err(Error::Invalid("a bounded customer OpenAI key is required"));
                }
                if custody != custody_digest(&made, commercial.as_ref()) {
                    return Err(Error::Conflict(
                        "credential custody differs from the displayed terms",
                    ));
                }
                let key_digest = retail_cloud::sha256_hex(credential.key.as_bytes());
                if let Some(original) = &existing {
                    if original.commercial != commercial
                        || original.key_digest != key_digest
                        || original.admission != admission
                        || original.custody != custody
                    {
                        return Err(Error::Conflict(
                            "confirmed disclosure or credential cannot change",
                        ));
                    }
                } else {
                    self.require_open(&store, None)?;
                    made.offer
                        .confirm(&digest, now as u64, &made.offer.terms)
                        .map_err(|_| Error::Conflict("the offer expired or changed"))?;
                    let needed = crate::msat(made.quote.max_sats)?;
                    if store
                        .ledger
                        .compute_balance(&identity.account)?
                        .available_msat
                        < needed
                    {
                        return Err(Error::Insufficient);
                    }
                    let requoted = offer::make_offer(
                        &contract::price_book(),
                        &identity.account,
                        &id,
                        &made.request,
                        self.capacity(store, None)?,
                        now as u64,
                    )?;
                    if requoted.quote != made.quote || requoted.admission != made.admission {
                        return Err(Error::Conflict(
                            "the current supported terms differ from the offered admission",
                        ));
                    }
                    vault_write(&self.config.state, &id, &credential.key)?;
                    store.confirmation(
                        &id,
                        &Confirmation {
                            commercial: commercial.clone(),
                            principal: identity.id.clone(),
                            generation: identity.generation,
                            admission: admission.clone(),
                            key_digest,
                            custody,
                            at: now as u64,
                        },
                    )?;
                }
                // A durable confirmation is accepted before lifecycle side
                // effects. Restart replays it even if the client lost this reply.
                let original = store
                    .offer(&id)?
                    .and_then(|(_, _, _, c)| c)
                    .ok_or(Error::Conflict("confirmation was not retained"))?;
                let capacity = self.capacity(&store, Some(&id))?;
                let funded = offer::confirm(
                    &mut store.journal,
                    &made,
                    &digest,
                    ConfirmedVia::OfferControl,
                    &contract::price_book(),
                    capacity,
                    original.at,
                )?;
                let current = self.current(&store, &funded, &original)?;
                let hold =
                    retail_cloud::reserve::reserve(&mut store.ledger, &funded, &current, now)?;
                json!({"execution":funded.execution,"request":funded.request,"offer":funded.offer,"admission":funded.admission.digest(),"accepted":true,"state":hold.state.as_str()})
            }
            Request::EnvironmentOffer {
                idempotency,
                request,
            } => {
                if !grant.execute || !grant.disclose {
                    return Err(Error::Denied);
                }
                let launch = self.environments()?;
                let id = opaque("re", &identity.account, &idempotency)?;
                let p = environment::offer(
                    &mut store.journal,
                    &launch.book,
                    &launch.gate,
                    &identity.account,
                    &id,
                    &request,
                    now,
                )?;
                environment_value(&p, &identity.account, now)?
            }
            Request::EnvironmentConfirm { purchase, digest } => {
                if !grant.execute || !grant.disclose {
                    return Err(Error::Denied);
                }
                let launch = self.environments()?;
                let p = environment::confirm(
                    &mut store.journal,
                    &mut store.ledger,
                    &launch.book,
                    &launch.gate,
                    &identity.account,
                    &purchase,
                    &digest,
                    identity.rights.spend,
                    now,
                )?;
                environment_value(&p, &identity.account, now)?
            }
            Request::Environment { purchase } => {
                let p = environment::purchase(&store.journal, &identity.account, &purchase)?
                    .ok_or(Error::Denied)?;
                environment_value(&p, &identity.account, now)?
            }
            Request::EnvironmentRenew {
                purchase,
                idempotency,
                days,
            } => {
                let launch = self.environments()?;
                let renewal = opaque("rr", &identity.account, &idempotency)?;
                let p = environment::renew(
                    &mut store.journal,
                    &mut store.ledger,
                    &launch.book,
                    &launch.gate,
                    &identity.account,
                    &purchase,
                    &renewal,
                    days,
                    identity.rights.spend,
                    now,
                )?;
                environment_value(&p, &identity.account, now)?
            }
            Request::Executions { after } => {
                if let Some(cursor) = &after {
                    self.funded_for(store, &identity.account, cursor)?;
                }
                let mut records = store
                    .journal
                    .all_funded()?
                    .into_iter()
                    .filter(|f| {
                        f.account == identity.account
                            && after.as_ref().is_none_or(|cursor| f.execution > *cursor)
                    })
                    .collect::<Vec<_>>();
                records.sort_by(|a, b| a.execution.cmp(&b.execution));
                records.truncate(types::STEP_MAX);
                let next = records.last().map(|f| f.execution.clone());
                json!({"executions":records.into_iter().map(|f|json!({"execution":f.execution,"request":f.request,"offer":f.offer,"admission":f.admission.digest()})).collect::<Vec<_>>(),"next":next})
            }
            Request::Execution { execution } => {
                let funded = self.funded_for(&store, &identity.account, &execution)?;
                let current = Self::read_current(&funded);
                let snapshots = retail_cloud::recover::observe(&store.journal, &funded, &current)?;
                let hold = store.ledger.hold(&funded.request)?;
                let mut value = json!({"execution":execution,"admission":funded.admission,"quote":funded.quote,"snapshot":snapshots.last(),"hold":hold.map(|h|json!({"state":h.state.as_str(),"held_msat":if h.state==pay_ledger::compute::HoldState::Settled {0}else{h.request.amount_msat},"charge_msat":h.charge_msat}))});
                if let Some(commercial) = store.commercial(&funded.offer)? {
                    value["commercial"] = serde_json::to_value(commercial)?;
                }
                value
            }
            Request::Progress { execution, after } => {
                self.commercial_ref(store, &identity.account)?;
                let funded = self.funded_for(&store, &identity.account, &execution)?;
                let page = dispatch::observe(
                    &store.journal,
                    &*self.backend,
                    &funded,
                    &Self::read_current(&funded),
                    after,
                )?;
                let key = vault_read(&self.config.state, &funded.offer)?;
                if key.is_none() {
                    return Err(Error::Unavailable(
                        "live progress custody has ended; read the retained artifacts instead",
                    ));
                }
                let mut previous = after;
                let mut bytes = 0;
                let mut events = Vec::new();
                for mut event in page.events.into_iter().take(128) {
                    if event.cursor <= previous || event.text.len() > 8192 {
                        return Err(Error::Unavailable("invalid or oversized task progress"));
                    }
                    if let Some(key) = &key {
                        event.text = event.text.replace(key.as_str(), "[redacted]");
                    }
                    bytes += event.text.len();
                    if bytes > 64 * 1024 {
                        break;
                    }
                    previous = event.cursor;
                    events.push(event);
                }
                json!({"events":events,"next":previous,"status":page.status})
            }
            Request::Cancel { execution } => {
                let funded = self.funded_for(&store, &identity.account, &execution)?;
                let confirmation = store
                    .offer(&funded.offer)?
                    .and_then(|(_, _, _, c)| c)
                    .ok_or(Error::Denied)?;
                if confirmation.principal != identity.id
                    || confirmation.generation != identity.generation
                    || !grant.execute
                {
                    return Err(Error::Denied);
                }
                let current = self.current(&store, &funded, &confirmation)?;
                if retail_cloud::settle::observe(
                    &store.journal,
                    &funded,
                    &Self::read_current(&funded),
                )?
                .is_some()
                {
                    json!({"execution":execution,"cancellation_requested":false,"already_terminal":true,"stopped":store.journal.retention_receipt(&execution,now)?.is_some_and(|r|r.deleted())})
                } else {
                    cancel::request(&mut store.journal, &funded, &current, now)?;
                    json!({"execution":execution,"cancellation_requested":true,"stopped":false})
                }
            }
            Request::Artifact { execution, name } => {
                self.commercial_ref(store, &identity.account)?;
                if name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                {
                    return Err(Error::Invalid("invalid logical artifact name"));
                }
                let funded = self.funded_for(&store, &identity.account, &execution)?;
                let bytes = store
                    .journal
                    .retained_artifact(&funded, &Self::read_current(&funded), &name, now)?
                    .ok_or(Error::Unavailable(
                        "artifact is absent, incomplete, or expired",
                    ))?;
                let text = String::from_utf8(bytes)
                    .map_err(|_| Error::Unavailable("artifact is not UTF-8"))?;
                if let Some(key) = vault_read(&self.config.state, &funded.offer)?
                    && text.contains(key.as_str())
                {
                    return Err(Error::Unavailable(
                        "artifact contains private credential material",
                    ));
                }
                json!({"execution":execution,"name":name,"text":text})
            }
            Request::Receipt { execution } => {
                let funded = self.funded_for(&store, &identity.account, &execution)?;
                let current = Self::read_current(&funded);
                let cancellation = if store.journal.cancellation_requested(&execution)? {
                    Some(cancel::observe(
                        &store.journal,
                        &store.ledger,
                        &funded,
                        &current,
                        now,
                    )?)
                } else {
                    None
                };
                let mut value = json!({"execution":execution,"cancellation":cancellation,"retention":store.journal.retention_receipt(&execution,now)?,"settlement":retail_cloud::settle::observe(&store.journal,&funded,&current)?});
                if let Some(commercial) = store.commercial(&funded.offer)? {
                    value["commercial"] = serde_json::to_value(commercial)?;
                }
                value
            }
        };
        store.check()?;
        Ok(json!({"schema":types::SCHEMA,"result":value}))
    }
}
impl From<offer::OfferRefusal> for Error {
    fn from(value: offer::OfferRefusal) -> Self {
        Self::Offer(value)
    }
}
fn custody_terms(
    made: &offer::RetailOffer,
    commercial: Option<&receipts::purchase::CommercialRef>,
) -> Value {
    let mut terms = json!({"schema":"openagents.cloud.retail-credential-custody.v1","admission":made.admission.digest(),"recipient":"the authenticated retail service receiving this confirmation","material":"customer-owned OpenAI API key","uses":["delivery to the exact admitted sandbox and model payer","redaction","removal"],"maximum_seconds":made.quote.max_seconds.saturating_add(2*retail_cloud::provision::READY_DEADLINE_SECS as u64).saturating_add(900),"ends":"remove after acknowledged resource deletion or the maximum custody duration; incomplete cleanup and unknown costs remain recorded"});
    if let Some(commercial) = commercial {
        terms["commercial"] =
            serde_json::to_value(commercial).expect("commercial reference serializes");
    }
    terms
}
fn custody_digest(
    made: &offer::RetailOffer,
    commercial: Option<&receipts::purchase::CommercialRef>,
) -> route_contract::Digest {
    digest_of(&custody_terms(made, commercial))
}
fn offer_value(
    made: &offer::RetailOffer,
    commercial: Option<&receipts::purchase::CommercialRef>,
) -> Value {
    let mut value = serde_json::to_value(made).expect("retail offer serializes");
    value["custody"] =
        json!({"terms":custody_terms(made, commercial),"digest":custody_digest(made, commercial)});
    value
}
/// One saved-environment purchase as its own account reads it.
fn environment_value(p: &environment::Purchase, account: &str, now: i64) -> Result<Value> {
    Ok(json!({
        "purchase": p.id,
        "digest": p.digest,
        "request": p.request,
        "quote": p.quote,
        "admission": p.admission,
        "phase": p.phase,
        "retention": p.retention,
        "selectable": environment::may_select(p, account, now),
    }))
}

fn opaque(prefix: &str, account: &str, idempotency: &str) -> Result<String> {
    if idempotency.is_empty()
        || idempotency.len() > 64
        || !idempotency
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(Error::Invalid("invalid bounded idempotency key"));
    }
    Ok(format!(
        "{prefix}_{}",
        digest_of(&(account, idempotency))
            .as_str()
            .trim_start_matches("sha256:")
    ))
}
fn purchase_value(
    p: pay_ledger::compute::Purchase,
    commercial: Option<&receipts::purchase::CommercialRef>,
) -> Value {
    let mut value = json!({"purchase":p.top_up.id,"account":p.top_up.account,"amount_msat":p.top_up.amount_msat,"invoice":p.top_up.invoice,"payment_hash":p.top_up.payment_hash,"state":p.state.as_str(),"expires_at":p.top_up.expires_at,"observed_at":p.observed_at});
    if let Some(reference) = commercial {
        value["commercial"] =
            serde_json::to_value(reference).expect("commercial reference serializes");
    }
    value
}
fn msat(sats: u64) -> Result<i64> {
    sats.checked_mul(1000)
        .and_then(|v| i64::try_from(v).ok())
        .ok_or(Error::Invalid("quote amount overflows millisatoshis"))
}

#[cfg(test)]
mod tests;
