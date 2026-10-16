//! The sovereign treasury: one durable accounting authority binding an
//! agent, its verified API account, its earnings beneficiary, its wallet,
//! and the X402 top-up resources it may buy from. It admits verified income,
//! reserves before every payment, and never pays twice for an uncertain one.
use crate::{
    Result,
    adapters::{Earnings, Pay, Redeem, TopUp, Wallet},
};
use nostr::{
    contracts::{self, digest_bytes},
    domain::{Event, RelaySigner},
    sovereign_treasury as treasury,
};
use openagents_x402::{
    PaymentPayload,
    policy::{Ceiling, Entry, Flags, Ledger as BuyerLedger, Policy as BuyerPolicy, Reservation},
    wire::encode_header,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use workspace_host::artifact::Artifacts;

/// Independently configured facts a request cannot nominate.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Agent pubkey to its governing authority.
    pub authorities: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Purchase {
    fingerprint: String,
    requester: String,
    hold_msat: u64,
    buyer_hold: Option<String>,
    payment_hash: Option<String>,
    preimage_known: bool,
    status: String,
    receipt: Value,
    period: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    policy: Option<Value>,
    period_start: u64,
    replenished_msat: u64,
    settled_msat: u64,
    income: BTreeMap<String, Value>,
    purchases: BTreeMap<String, Purchase>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    config: Config,
    agents: BTreeMap<String, Account>,
    signers: BTreeMap<String, BTreeSet<String>>,
}

/// A signed private request, its exact artifacts, and further declarations.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub event: Event,
    #[serde(default)]
    pub artifacts: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub declarations: Vec<Event>,
}

/// The owners one treasury drives.
pub struct Owners {
    pub wallet: Box<dyn Wallet>,
    pub earnings: Box<dyn Earnings>,
    pub topup: Box<dyn TopUp>,
}

pub struct Treasury {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
    owners: RefCell<Owners>,
    buyer: BuyerLedger,
}

fn refuse(code: &str, detail: &str) -> String {
    format!("{code}: {detail}")
}

/// The shared refusal code at the front of an error, or `unavailable`.
pub fn code(error: &str) -> &'static str {
    const CODES: &[&str] = &[
        "malformed",
        "unsupported_version",
        "unsupported_feature",
        "not_admitted",
        "unavailable",
        "content_unavailable",
        "identity_mismatch",
        "incompatible",
        "revoked",
        "stale",
        "cannot_enforce",
        "limit_exceeded",
        "idempotency_conflict",
        "conflict",
    ];
    let head = error.split(':').next().unwrap_or_default().trim();
    CODES
        .iter()
        .find(|c| **c == head)
        .copied()
        .unwrap_or("unavailable")
}

fn contract(e: contracts::ContractError) -> String {
    format!("{}: {}", e.code.as_str(), e)
}

fn digest_of(reference: &Value) -> &str {
    reference["digest"].as_str().unwrap_or_default()
}

fn same(a: &Value, b: &Value) -> bool {
    !digest_of(a).is_empty() && digest_of(a) == digest_of(b)
}

/// The x402 network identifier for a policy network. Regtest has none: it is
/// refused rather than aliased into another network.
pub fn network(policy: &Value) -> Option<&'static str> {
    match policy["network"].as_str() {
        Some("mainnet") => openagents_x402::network_id("bitcoin"),
        Some("testnet") => openagents_x402::network_id("testnet"),
        _ => None,
    }
}

impl Treasury {
    pub fn initialize(root: PathBuf, secret: SecretKey, config: Config) -> Result<()> {
        let store = Artifacts::new(root)?;
        if store.root().join("state.json").exists() {
            return Err("conflict: treasury state already exists".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        store.save(
            &State {
                config,
                ..State::default()
            },
            &signer,
        )
    }
    pub fn open(root: PathBuf, secret: SecretKey, owners: Owners) -> Result<Self> {
        let store = Artifacts::new(root.clone())?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let buyer = BuyerLedger::open(&root.join("buyer.ndjson"));
        let treasury = Self {
            store,
            signer,
            secret,
            owners: RefCell::new(owners),
            buyer,
        };
        treasury.state()?;
        Ok(treasury)
    }
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }
    pub fn store(&self) -> &Artifacts {
        &self.store
    }
    pub fn with_owners<T>(&self, f: impl FnOnce(&mut Owners) -> T) -> T {
        f(&mut self.owners.borrow_mut())
    }
    fn state(&self) -> Result<State> {
        self.store.load(&self.signer)
    }
    fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }

    fn sign(&self, state: &mut State, body: &Value, recipient: &str, now: u64) -> Result<Value> {
        treasury::validate(body).map_err(contract)?;
        let reference = self.store.put_json(body)?;
        let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
        self.store.put(
            &serde_json::to_vec(&event).map_err(|e| e.to_string())?,
            "application/json",
            None,
        )?;
        state
            .signers
            .entry(digest_of(&reference).into())
            .or_default()
            .insert(self.pubkey().into());
        Ok(reference)
    }

    fn authenticate(&self, state: &mut State, event: &Event) -> Result<(String, Value, Value)> {
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let bytes = opened
            .inline_bytes()
            .ok_or("malformed: requests carry their artifact inline")?
            .to_vec();
        let body = contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)?;
        let schema = body["v"].as_str().unwrap_or_default().to_string();
        let reference = self.store.put(
            &bytes,
            "application/json",
            Some(schema.as_str()).filter(|s| !s.is_empty()),
        )?;
        state
            .signers
            .entry(digest_of(&reference).into())
            .or_default()
            .insert(opened.signer().into());
        Ok((opened.signer().into(), body, reference))
    }

    fn json(&self, reference: &Value) -> Result<Value> {
        let bytes = self
            .store
            .get(reference)
            .map_err(|_| refuse("content_unavailable", "referenced bytes are not retained"))?;
        contracts::parse_strict_bounded(&bytes, 1_048_576).map_err(contract)
    }

    /// Admit one signed operation.
    pub fn submit(&self, bundle: &Bundle, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        for (digest, bytes) in &bundle.artifacts {
            if digest_bytes(bytes) != *digest {
                return Err(refuse("identity_mismatch", "attached artifact digest"));
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let mut declarations = Vec::new();
        for event in &bundle.declarations {
            declarations.push(self.authenticate(&mut state, event)?);
        }
        let (signer, body, input) = self.authenticate(&mut state, &bundle.event)?;
        let outcome = match treasury::validate(&body) {
            Err(e) => Err(format!("{}: {}", e.code.as_str(), e)),
            Ok(()) => match body["v"].as_str() {
                Some("openagents.sovereign-treasury-policy.v1") => {
                    self.admit_policy(&mut state, &signer, &body, &input)
                }
                Some("openagents.sovereign-income.v1") => {
                    self.income(&mut state, &signer, &body, &input, now)
                }
                Some("openagents.sovereign-purchase-request.v1") => {
                    self.purchase(&mut state, &signer, &body, &input, &declarations, now)
                }
                _ => Err(refuse("unsupported_feature", "operation")),
            },
        };
        let value = match outcome {
            Ok(value) => value,
            Err(error) => {
                let mut fresh = self.state()?;
                fresh.signers.extend(state.signers);
                state = fresh;
                json!({"status":if matches!(code(&error),"conflict"|"idempotency_conflict"){"conflict"}else{"refused"},"reason":code(&error)})
            }
        };
        self.save(&state)?;
        Ok(value)
    }

    fn admit_policy(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        if body["authority"] != signer
            || state.config.authorities.get(&agent).map(String::as_str) != Some(signer)
        {
            return Err(refuse(
                "not_admitted",
                "the policy is not signed by the agent's authority",
            ));
        }
        if network(body).is_none() {
            return Err(refuse(
                "incompatible",
                "the X402 profile supports only mainnet or testnet",
            ));
        }
        {
            let owners = self.owners.borrow();
            if body["wallet"]["fee_limit"] != true
                || body["wallet"]["lookup"] != true
                || !owners.wallet.fee_limit()
                || !owners.wallet.lookup()
            {
                return Err(refuse(
                    "cannot_enforce",
                    "the wallet adapter lacks fee-limit or recovery support",
                ));
            }
        }
        // Verified API-account control evidence must be retained exactly.
        self.store
            .get(&body["account"]["control"])
            .map_err(|_| refuse("content_unavailable", "account control evidence"))?;
        let allowance = body["allowance_msat"].as_u64().unwrap_or(0);
        if body["spenders"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|s| s["cap_msat"].as_u64().unwrap_or(0) > allowance)
        {
            return Err(refuse(
                "limit_exceeded",
                "a child cap cannot exceed the allowance",
            ));
        }
        let account = state.agents.entry(agent).or_default();
        match (&account.policy, body["previous"].is_null()) {
            (None, true) => account.period_start = body["starts_at"].as_u64().unwrap_or(0),
            (Some(current), false) if same(current, &body["previous"]) => {}
            _ => return Err(refuse("conflict", "policy predecessor")),
        }
        account.policy = Some(input.clone());
        Ok(json!({"status":"accepted","value":input}))
    }

    /// Roll the accounting period forward. Unknown liabilities carry over.
    fn roll(account: &mut Account, policy: &Value, now: u64) {
        let period = policy["period_seconds"].as_u64().unwrap_or(u64::MAX).max(1);
        if now >= account.period_start.saturating_add(period) {
            let periods = (now - account.period_start) / period;
            account.period_start += periods * period;
            account.settled_msat = 0;
            account.replenished_msat = 0;
        }
    }

    fn income(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        let account = state
            .agents
            .get(&agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "null treasury"))?;
        let policy_ref = account
            .policy
            .clone()
            .ok_or_else(|| refuse("not_admitted", "null treasury"))?;
        if !same(&policy_ref, &body["policy"]) {
            return Err(refuse("stale", "policy revision"));
        }
        let policy = self.json(&policy_ref)?;
        if signer != policy["reconciliation"] && signer != agent {
            return Err(refuse(
                "not_admitted",
                "income is reported by the agent or its reconciliation authority",
            ));
        }
        let payout = body["payout"].as_str().unwrap_or_default().to_string();
        if account.income.contains_key(&payout) {
            // The same payout never credits twice.
            return Ok(json!({"status":"duplicate","value":account.income[&payout]}));
        }
        let amount = body["amount_msat"].as_u64().unwrap_or(0);
        let reference = body["wallet_reference"].as_str().unwrap_or_default();
        let beneficiary = policy["beneficiary"].as_str().unwrap_or_default();
        let owners = self.owners.borrow();
        // The authoritative ledger must have sent this payout to this
        // beneficiary, and the agent's wallet must have received it.
        match owners.earnings.sent_payout(beneficiary, &payout)? {
            Some((sent, wallet_reference)) if sent == amount && wallet_reference == reference => {}
            _ => {
                return Err(refuse(
                    "not_admitted",
                    "no matching sent payout to this beneficiary",
                ));
            }
        }
        if owners.wallet.received(reference) != Some(amount) {
            return Err(refuse(
                "not_admitted",
                "the wallet has not received that payment",
            ));
        }
        drop(owners);
        let account = state.agents.get_mut(&agent).ok_or("account")?;
        Self::roll(account, &policy, now);
        let replenish = if policy["income"]["mode"] == "replenish" {
            let max = policy["income"]["max_msat"].as_u64().unwrap_or(0);
            amount.min(max.saturating_sub(account.replenished_msat))
        } else {
            0
        };
        account.replenished_msat += replenish;
        account.income.insert(payout, input.clone());
        Ok(json!({"status":"accepted","value":input,"replenished_msat":replenish}))
    }

    fn holds(account: &Account) -> (u64, u64) {
        let mut held = 0;
        let mut unknown = 0;
        for purchase in account.purchases.values() {
            match purchase.status.as_str() {
                "held" => held += purchase.hold_msat,
                "unknown" => unknown += purchase.hold_msat,
                _ => {}
            }
        }
        (held, unknown)
    }

    fn purchase(
        &self,
        state: &mut State,
        signer: &str,
        body: &Value,
        input: &Value,
        declarations: &[(String, Value, Value)],
        now: u64,
    ) -> Result<Value> {
        let agent = body["agent"].as_str().unwrap_or_default().to_string();
        let id = body["purchase"].as_str().unwrap_or_default().to_string();
        let mut account = state
            .agents
            .get(&agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "null treasury: no spending"))?;
        if let Some(prior) = account.purchases.get(&id) {
            if prior.fingerprint != digest_of(input) {
                return Err(refuse(
                    "idempotency_conflict",
                    "purchase ID reused with other bytes",
                ));
            }
            return Ok(json!({"status":prior.status,"receipt":prior.receipt}));
        }
        let policy_ref = account
            .policy
            .clone()
            .ok_or_else(|| refuse("not_admitted", "null treasury: no spending"))?;
        if !same(&policy_ref, &body["policy"]) {
            return Err(refuse("stale", "policy revision"));
        }
        let policy = self.json(&policy_ref)?;
        if now >= policy["expires_at"].as_u64().unwrap_or(0)
            || body["deadline"].as_u64().unwrap_or(0) <= now
        {
            return Err(refuse("stale", "policy or request expired"));
        }
        if body["requester"] != signer {
            return Err(refuse("identity_mismatch", "requester is the signer"));
        }
        let spender = policy["spenders"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["principal"] == signer))
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "not an admitted spender"))?;
        if !policy["destinations"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|d| *d == body["destination"])
        {
            return Err(refuse(
                "not_admitted",
                "destination is not an approved top-up resource",
            ));
        }
        let approved = declarations.iter().any(|(approver, approval, _)| {
            approval["v"] == "openagents.sovereign-purchase-approval.v1"
                && approver == policy["authorizer"].as_str().unwrap_or_default()
                && approval["authorizer"] == *approver
                && same(&approval["request"], input)
                && approval["expires_at"].as_u64().is_some_and(|e| e > now)
        });
        if !approved {
            return Err(refuse(
                "not_admitted",
                "the authorizer has not approved this exact purchase",
            ));
        }
        let amount = body["amount_msat"].as_u64().unwrap_or(0);
        let fee_ceiling = policy["fee_ceiling_msat"].as_u64().unwrap_or(0);
        if amount > policy["per_operation_msat"].as_u64().unwrap_or(0) {
            return Err(refuse("limit_exceeded", "per-operation ceiling"));
        }
        Self::roll(&mut account, &policy, now);
        let hold = amount + fee_ceiling;
        let (held, unknown) = Self::holds(&account);
        let available = (policy["allowance_msat"].as_u64().unwrap_or(0) + account.replenished_msat)
            .saturating_sub(account.settled_msat + held + unknown);
        if hold > available {
            return Err(refuse(
                "limit_exceeded",
                "the shared allowance cannot cover this purchase",
            ));
        }
        let child_use: u64 = account
            .purchases
            .values()
            .filter(|p| {
                p.requester == signer
                    && (p.period == account.period_start || p.status != "completed")
            })
            .map(|p| p.hold_msat)
            .sum();
        if child_use + hold > spender["cap_msat"].as_u64().unwrap_or(0) {
            return Err(refuse(
                "limit_exceeded",
                "the child's cap draws on the same allowance",
            ));
        }
        // Validate the seller's challenge before any wallet effect.
        let url = body["destination"]["resource"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let request_body = serde_json::to_vec(&json!({"account":policy["account"]["id"]}))
            .map_err(|e| e.to_string())?;
        let required =
            self.owners
                .borrow_mut()
                .topup
                .challenge(&url, &request_body, amount, now)?;
        let accepted = required
            .accepts
            .first()
            .cloned()
            .ok_or("malformed: no payment terms")?;
        let expected_network = network(&policy).ok_or("network")?;
        let request_hash = nostr::x402::binding_hash(
            &nostr::x402::http_binding("POST", &url, &request_body, &[])
                .map_err(|e| format!("malformed: {e:?}"))?,
        )
        .map_err(|e| format!("malformed: {e:?}"))?;
        let invoice =
            nostr::x402::validate_challenge(&accepted, &request_hash, now, 60, Default::default())
                .map_err(|e| refuse("identity_mismatch", &format!("challenge: {e:?}")))?;
        if accepted.network != expected_network
            || accepted.amount != amount.to_string()
            || accepted.pay_to != body["destination"]["provider"].as_str().unwrap_or_default()
        {
            return Err(refuse(
                "identity_mismatch",
                "the challenge does not match the approved purchase",
            ));
        }
        // The existing buyer ledger enforces ceilings and the provider allowlist.
        let buyer_policy = BuyerPolicy {
            default: Ceiling {
                max_msat: Some(policy["per_operation_msat"].as_u64().unwrap_or(0)),
                max_fee_msat: Some(fee_ceiling),
            },
            daily_cap_msat: None,
            providers: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            allow: policy["destinations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["provider"].as_str().map(str::to_string))
                .collect(),
        };
        let limits = BuyerPolicy::limits(
            Some(&buyer_policy),
            Flags::default(),
            Some(&accepted.pay_to),
            None,
        )
        .map_err(|e| refuse("limit_exceeded", &e.to_string()))?;
        let reservation = self
            .buyer
            .reserve(Some(&buyer_policy), limits, &accepted.pay_to, amount, now)
            .map_err(|e| refuse("limit_exceeded", &e.to_string()))?;
        let payment_hash = hex::encode(invoice.payment_hash());
        // Durable hold before paying: a crash leaves it held, never repeated.
        account.purchases.insert(
            id.clone(),
            Purchase {
                fingerprint: digest_of(input).into(),
                requester: signer.into(),
                hold_msat: hold,
                buyer_hold: Some(reservation.id.clone()),
                payment_hash: Some(payment_hash.clone()),
                preimage_known: false,
                status: "held".into(),
                receipt: Value::Null,
                period: account.period_start,
            },
        );
        state.agents.insert(agent.clone(), account.clone());
        self.save(state)?;
        let bolt11 = accepted
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let paid = self.owners.borrow_mut().wallet.pay(&bolt11, fee_ceiling);
        let (status, payment, credit, fee, reason) = match paid {
            Pay::Failed => {
                let _ = self.buyer.release(&Reservation {
                    id: reservation.id.clone(),
                    hold_msat: hold,
                });
                ("refused", None, None, None, Some("unavailable"))
            }
            Pay::Unknown => ("unknown", None, None, None, None),
            Pay::Paid { preimage, fee_msat } => {
                let mut payload = Map::new();
                payload.insert("preimage".into(), json!(preimage));
                let signature = encode_header(&PaymentPayload {
                    x402_version: 2,
                    resource: None,
                    accepted: accepted.clone(),
                    payload,
                    extensions: None,
                })
                .map_err(|e| e.to_string())?;
                let payment = Some(self.store.put(&serde_json::to_vec(&json!({"payment_hash":payment_hash,"amount_msat":amount,"fee_msat":fee_msat,"network":accepted.network})).map_err(|e| e.to_string())?, "application/json", None)?);
                match self.owners.borrow_mut().topup.redeem(
                    &url,
                    &request_body,
                    &signature,
                    amount,
                    now,
                ) {
                    Redeem::Credited(receipt) => {
                        let entry = Entry {
                            paid_at: now,
                            binding: "http:1".into(),
                            network: accepted.network.clone(),
                            provider: accepted.pay_to.clone(),
                            capability: None,
                            resource: url.clone(),
                            amount_msat: amount,
                            fee_msat,
                            payment_hash: payment_hash.clone(),
                            phase: "completed".into(),
                        };
                        self.buyer
                            .settle(
                                &Reservation {
                                    id: reservation.id.clone(),
                                    hold_msat: hold,
                                },
                                &entry,
                            )
                            .map_err(|e| e.to_string())?;
                        (
                            "completed",
                            payment,
                            Some(self.store.put(&receipt, "application/json", None)?),
                            Some(fee_msat),
                            None,
                        )
                    }
                    // Paid but no credit evidence: retain the liability.
                    Redeem::Refused(_) | Redeem::Unknown => {
                        ("unknown", payment, None, Some(fee_msat), None)
                    }
                }
            }
        };
        let receipt = json!({"v":"openagents.sovereign-purchase-receipt.v1","requires":[],"request":input,
            "status":status,"payment":payment,"credit":credit,"fee_msat":fee,"reason":reason});
        let receipt = self.sign(state, &receipt, signer, now)?;
        let account = state.agents.get_mut(&agent).ok_or("account")?;
        let purchase = account.purchases.get_mut(&id).ok_or("purchase")?;
        purchase.status = status.into();
        purchase.receipt = receipt.clone();
        purchase.preimage_known = payment.is_some();
        if status == "completed" {
            purchase.hold_msat = amount + fee.unwrap_or(0);
            account.settled_msat += purchase.hold_msat;
        } else if status == "refused" {
            purchase.hold_msat = 0;
        }
        Ok(json!({"status":status,"receipt":receipt}))
    }

    /// Reconcile an unknown purchase by its exact payment hash. It never pays
    /// again; it only converts proven outcomes.
    pub fn reconcile(&self, agent: &str, purchase: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut account = state
            .agents
            .get(agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "null treasury"))?;
        let policy = self.json(account.policy.as_ref().ok_or("policy")?)?;
        let mut item = account
            .purchases
            .get(purchase)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "unknown purchase"))?;
        if item.status != "unknown" {
            return Ok(json!({"status":item.status,"receipt":item.receipt}));
        }
        let hash = item.payment_hash.clone().ok_or("payment hash")?;
        let account_id = policy["account"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let credited = self.owners.borrow_mut().topup.credited(&account_id, &hash);
        let request = self.json(&item.receipt)?["request"].clone();
        let (status, credit) = if let Some(receipt) = credited {
            (
                "completed",
                Some(self.store.put(&receipt, "application/json", None)?),
            )
        } else {
            match self.owners.borrow_mut().wallet.status(&hash) {
                Pay::Failed => ("refused", None),
                _ => ("unknown", None),
            }
        };
        if status == "unknown" {
            return Ok(json!({"status":"unknown","receipt":item.receipt}));
        }
        let previous = self.json(&item.receipt)?;
        let body = json!({"v":"openagents.sovereign-purchase-receipt.v1","requires":[],"request":request,
            "status":status,"payment":previous["payment"],"credit":credit,"fee_msat":previous["fee_msat"],
            "reason":if status == "refused" {json!("unavailable")} else {Value::Null}});
        if status == "completed" && body["payment"].is_null() {
            return Ok(json!({"status":"unknown","receipt":item.receipt}));
        }
        let receipt = self.sign(&mut state, &body, agent, now)?;
        if let Some(hold) = &item.buyer_hold {
            let reservation = Reservation {
                id: hold.clone(),
                hold_msat: item.hold_msat,
            };
            if status == "completed" {
                let payment = self.json(&body["payment"])?;
                let fee = payment["fee_msat"].as_u64().unwrap_or(0);
                let amount = payment["amount_msat"].as_u64().unwrap_or(0);
                let entry = Entry {
                    paid_at: now,
                    binding: "http:1".into(),
                    network: payment["network"].as_str().unwrap_or_default().into(),
                    provider: String::new(),
                    capability: None,
                    resource: String::new(),
                    amount_msat: amount,
                    fee_msat: fee,
                    payment_hash: hash.clone(),
                    phase: "completed".into(),
                };
                let _ = self.buyer.settle(&reservation, &entry);
                item.hold_msat = amount + fee;
                if item.period == account.period_start {
                    account.settled_msat += item.hold_msat;
                }
            } else {
                let _ = self.buyer.release(&reservation);
                item.hold_msat = 0;
            }
        }
        item.status = status.into();
        item.receipt = receipt.clone();
        account.purchases.insert(purchase.into(), item);
        state.agents.insert(agent.into(), account);
        self.save(&state)?;
        Ok(json!({"status":status,"receipt":receipt}))
    }

    /// A private statement of the current period: source references, holds,
    /// and unknown liabilities. It is a view, not another ledger.
    pub fn statement(&self, agent: &str, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let mut account = state
            .agents
            .get(agent)
            .cloned()
            .ok_or_else(|| refuse("not_admitted", "null treasury"))?;
        let policy_ref = account.policy.clone().ok_or("policy")?;
        let policy = self.json(&policy_ref)?;
        Self::roll(&mut account, &policy, now);
        let (held, unknown) = Self::holds(&account);
        let body = json!({"v":"openagents.sovereign-treasury-statement.v1","requires":[],"policy":policy_ref,
            "period_start":account.period_start,"allowance_msat":policy["allowance_msat"],
            "replenished_msat":account.replenished_msat,"settled_msat":account.settled_msat,"held_msat":held,
            "unknown_msat":unknown,"income":account.income.values().collect::<Vec<_>>(),
            "purchases":account.purchases.values().filter(|p| !p.receipt.is_null()).map(|p| p.receipt.clone()).collect::<Vec<_>>()});
        let reference = self.sign(&mut state, &body, agent, now)?;
        state.agents.insert(agent.into(), account);
        self.save(&state)?;
        Ok(json!({"statement":reference,"body":body}))
    }
}

/// Build a signed private request envelope for a treasury.
pub fn envelope(secret: &SecretKey, treasury_key: &str, body: &Value, now: u64) -> Result<Event> {
    treasury::validate(body).map_err(contract)?;
    workspace_host::host::request(secret, treasury_key, body.clone(), now)
}
