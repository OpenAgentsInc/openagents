//! Native merchant commissions over protected buyer, account, outcome, and
//! receiver custody. Reconciliation performs no payment or guest invocation.
use coder::customer::plugins::CommissionSource;
use openagents_wallet::{LightningWallet, PaymentDirection, PaymentStatus};
use pay_ledger::{
    Ledger,
    commission::{Admission, Cost, Report},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use tenancy::{Accounts, Registry, accounts::referrals::commission as contract};

fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(v).map_err(|_| "Commission source encoding failed.".into())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub registry: PathBuf,
    pub outcomes: PathBuf,
    pub receiver_node: String,
    pub operator_account: String,
    pub operator_workspace: String,
    pub operator_credential: PathBuf,
    pub canonical_directory: Option<PathBuf>,
    pub commercial: Option<crate::Config>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostEntry {
    pub category: String,
    /// An absent amount stays unknown. Zero is an explicit declared cost.
    pub amount: Option<u64>,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostPolicy {
    pub schema: String,
    pub operator: String,
    pub offer_digest: String,
    pub unit: tenancy::money::funding::Unit,
    pub entries: Vec<CostEntry>,
    pub digest: String,
}
impl CostPolicy {
    pub fn seal(mut self) -> Result<Self, String> {
        self.digest.clear();
        self.digest = hash(encode(&self)?);
        Ok(self)
    }
    fn costs(
        &self,
        terms: &contract::Terms,
        offer: &str,
        operator: &str,
    ) -> Result<Vec<Cost>, String> {
        if self.schema != "openagents.commission-cost-policy.v1"
            || self.operator != operator
            || self.offer_digest != offer
            || self.unit != terms.unit
            || self.clone().seal()?.digest != self.digest
            || self.entries.len() != 6
        {
            return Err("Exact native denomination and reviewed cost policy are required.".into());
        }
        self.entries
            .iter()
            .map(|v| {
                Ok(Cost {
                    category: v.category.clone(),
                    amount_msat: v
                        .amount
                        .map(|n| {
                            terms
                                .amount_msat(n)
                                .map_err(|_| "Cost policy scale overflow.".to_string())
                        })
                        .transpose()?,
                    provenance: "operator-declared".into(),
                    evidence: v.evidence.clone(),
                })
            })
            .collect()
    }
}
pub struct Native {
    config: Config,
    root: File,
    accounts: Accounts,
    operator: crate::Held,
    config_source: Option<(crate::Held, String)>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbuseInput {
    pub rules: pay_ledger::commission_abuse::Rules,
    pub review: pay_ledger::commission_abuse::Review,
}
impl Native {
    /// Review an original liability under current native buyer and merchant
    /// grants. The owner's approval names the exact private rules digest.
    pub fn review_abuse(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        id: &str,
        input_path: &Path,
        approved: &str,
        now: u64,
    ) -> Result<pay_ledger::commission_abuse::Record, String> {
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let a = ledger
            .commission_admission(id)
            .map_err(|e| e.to_string())?
            .ok_or("Original abuse admission absent.")?;
        self.matches(&a, source, ledger)?;
        let held = crate::Held::open(input_path)?;
        let bytes = held.bytes(65536)?;
        let input: AbuseInput =
            serde_json::from_slice(&bytes).map_err(|_| "Private reviewed abuse input invalid.")?;
        let expected = hash(&bytes);
        let mut result = None;
        self.accounts
            .with_commission_agreement(
                &a.buyer_account,
                &a.customer,
                Some(&a.agreement),
                || self.buyer(source).is_ok() && self.operator().is_ok(),
                |v| {
                    let old: contract::View = serde_json::from_str(&a.contract)
                        .map_err(|_| tenancy::accounts::referrals::Error::Invalid)?;
                    if old.agreement != v.agreement || old.terms != v.terms {
                        return Err(tenancy::accounts::referrals::Error::Conflict);
                    }
                    result = Some((|| {
                        if input.review.action == pay_ledger::commission_abuse::Action::Release
                            && v.current_referrer_owner.as_ref().is_none_or(|owner| {
                                owner == &a.buyer_account || owner == &a.operator_account
                            })
                        {
                            return Err(
                                "Current native identity overlap cannot release a commission."
                                    .into(),
                            );
                        }
                        self.buyer(source)?;
                        self.operator()?;
                        ledger.require_native_custody().map_err(|e| e.to_string())?;
                        if hash(held.bytes(65536)?) != expected {
                            return Err("Reviewed abuse input changed.".into());
                        }
                        ledger
                            .review_commission_abuse(
                                id,
                                &input.rules,
                                approved,
                                &self.config.operator_account,
                                &input.review,
                                now,
                            )
                            .map_err(|e| e.to_string())
                    })());
                    Ok(())
                },
            )
            .map_err(|_| "Original abuse review custody or current authority refused.")?;
        result.ok_or("Original abuse review absent.")?
    }
    pub fn open_private(path: &Path) -> Result<Self, String> {
        let held = crate::Held::open(path)?;
        let bytes = held.bytes(64 * 1024)?;
        let config = serde_json::from_slice(&bytes)
            .map_err(|_| "Private native commission config invalid.")?;
        let mut native = Self::open(config)?;
        native.config_source = Some((held, hash(bytes)));
        native.current()?;
        Ok(native)
    }
    pub fn open(config: Config) -> Result<Self, String> {
        if let Some(directory) = &config.canonical_directory {
            if directory.canonicalize().ok().as_ref() != Some(&config.registry) {
                return Err("Cross-registry commission authority is not qualified.".into());
            }
        }
        if !config.registry.is_absolute()
            || !config.outcomes.is_absolute()
            || config
                .registry
                .canonicalize()
                .map_err(|_| "Registry unavailable.")?
                != config.registry
        {
            return Err("Explicit canonical native source paths are required.".into());
        }
        let root = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&config.registry)
            .map_err(|_| "Native registry custody unavailable.")?;
        if !crate::private(
            &root
                .metadata()
                .map_err(|_| "Registry metadata unavailable.")?,
            true,
        ) {
            return Err("Native registry must already be private.".into());
        }
        let accounts =
            Accounts::open(&config.registry).map_err(|_| "Native accounts unavailable.")?;
        let operator = crate::Held::open(&config.operator_credential)?;
        let native = Self {
            config,
            root,
            accounts,
            operator,
            config_source: None,
        };
        native.current()?;
        native.operator()?;
        Ok(native)
    }
    fn current(&self) -> Result<(), String> {
        if let Some((held, digest)) = &self.config_source {
            if hash(held.bytes(64 * 1024)?) != *digest {
                return Err("Native commission config changed.".into());
            }
        }
        let visible = std::fs::symlink_metadata(&self.config.registry)
            .map_err(|_| "Native registry custody replaced.")?;
        let held = self
            .root
            .metadata()
            .map_err(|_| "Native registry custody unavailable.")?;
        if !crate::private(&visible, true) || !crate::same(&held, &visible) {
            return Err("Native registry custody replaced or disclosed.".into());
        }
        self.operator.check()
    }
    fn operator(&self) -> Result<(), String> {
        self.current()?;
        let bytes = self.operator.bytes(4096)?;
        let token =
            std::str::from_utf8(&bytes).map_err(|_| "Native operator credential is invalid.")?;
        let registry =
            Registry::open(&self.config.registry).map_err(|_| "Native registry unavailable.")?;
        let key = tenancy::keys::authenticate(&self.config.registry, registry.manifest(), token)
            .map_err(|_| "Current native merchant credential required.")?;
        let member = self
            .accounts
            .authenticate_key(registry.manifest(), &self.config.operator_workspace, token)
            .map_err(|_| "Current native merchant operator authority required.")?;
        if member.account != self.config.operator_account
            || member.role != tenancy::Role::Owner
            || key
                .scopes
                .as_ref()
                .is_some_and(|s| !s.permits_action("accounts"))
        {
            return Err("Current native merchant owner authority required.".into());
        }
        Ok(())
    }
    fn buyer(&self, source: &CommissionSource<'_>) -> Result<(), String> {
        self.current()?;
        source.current()?;
        let selected = source.current_selection();
        let registry =
            Registry::open(&self.config.registry).map_err(|_| "Native registry unavailable.")?;
        let key = tenancy::keys::authenticate(
            &self.config.registry,
            registry.manifest(),
            source.credential().expose(),
        )
        .map_err(|_| "Current native buyer credential required.")?;
        let member = self
            .accounts
            .authenticate_key(
                registry.manifest(),
                &selected.context.workspace,
                source.credential().expose(),
            )
            .map_err(|_| "Current native buyer membership required.")?;
        if member.account != source.view().customer.context.account
            || member.workspace != source.view().customer.context.workspace
            || key.tenant != selected.context.tenant
            || key
                .scopes
                .as_ref()
                .is_some_and(|s| !s.permits_action("accounts"))
        {
            return Err(
                "Original buyer and current native account-read authority required.".into(),
            );
        }
        Ok(())
    }
    fn customer(&self, source: &CommissionSource<'_>) -> Result<String, String> {
        let offer = &source.view().offer;
        if let Some(reference) = &offer.commercial {
            let config = self
                .config
                .commercial
                .as_ref()
                .ok_or("Native commercial source qualification is required.")?;
            let directory = self
                .config
                .canonical_directory
                .as_ref()
                .ok_or("Canonical native account directory required.")?;
            let sources = crate::NativeSources::open(directory, config)?;
            if reference.source.product != receipts::purchase::CommercialProduct::Plugin {
                return Err("Original native plugin commercial source required.".into());
            }
            let native_source = tenancy::accounts::commercial::Source {
                product: tenancy::accounts::commercial::Product::Plugin,
                issuer: reference.source.issuer.clone(),
                account: reference.source.account.clone(),
                workspace: reference.source.workspace.clone(),
            };
            let current = sources
                .selection(&native_source)?
                .ok_or("Current commercial attribution unavailable.")?;
            if current.binding != reference.binding
                || current.revision != reference.revision
                || current.digest != reference.digest
                || current.customer != reference.customer
                || current.workspace != reference.workspace
            {
                return Err("Original native commercial mapping changed before admission.".into());
            }
            Ok(reference.customer.clone())
        } else {
            Ok(source.view().customer.context.account.clone())
        }
    }
    pub fn admit(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        policy_path: &Path,
        receiver_wallet: &dyn LightningWallet,
        now: u64,
    ) -> Result<Admission, String> {
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let offer = &source.view().offer;
        if source.view().phase != coder::customer::plugins::Phase::Approved
            || offer.expires_at_ms <= now.checked_mul(1000).ok_or("Clock overflow.")?
        {
            return Err(
                "The original approved plugin purchase must be admitted before payment and expiry."
                    .into(),
            );
        }
        let invoice = nostr::x402::decode_invoice(offer.invoice())
            .map_err(|_| "Signed original merchant invoice required.")?;
        if hash_key(invoice.payee()) != self.config.receiver_node
            || offer.payer.node == self.config.receiver_node
            || source.view().customer.context.account == self.config.operator_account
            || invoice.created_at() > now
            || invoice
                .created_at()
                .checked_add(invoice.expiry_seconds())
                .is_none_or(|end| end <= now)
            || invoice.amount_msat() != offer.quote.price_msat
            || hash_key(invoice.description_hash()) != offer.request_hash
            || offer.recovery_authorization.as_deref()
                != Some(openagents_x402::outcome::commitment(source.authorization()).as_str())
        {
            return Err("Original signed invoice and private purchase binding disagree.".into());
        }
        let policy = crate::Held::open(policy_path)?;
        let costs: CostPolicy = serde_json::from_slice(&policy.bytes(64 * 1024)?)
            .map_err(|_| "Private reviewed cost policy is invalid.")?;
        let source_id = hash(encode(&(
            ledger.origin().map_err(|e| e.to_string())?,
            offer.request_hash.clone(),
            invoice.payment_hash(),
            source.authorization(),
        ))?);
        if let Some(old) = ledger
            .commission_admission(&source_id)
            .map_err(|e| e.to_string())?
        {
            self.matches(&old, source, ledger)?;
            let retained: contract::View = serde_json::from_str(&old.contract)
                .map_err(|_| "Original admitted contract invalid.")?;
            if costs.digest != old.cost_policy
                || costs.costs(
                    &retained.terms.terms,
                    &old.offer_digest,
                    &self.config.operator_account,
                )? != old.costs
            {
                return Err("Original admission cost policy changed; use explicit cost qualification for unknown costs.".into());
            }
            return self.original(ledger, source, &old, |_| {
                check_cost_policy(&policy, &costs)?;
                Ok(old.clone())
            });
        }
        if receiver_wallet
            .lookup_from_node(&self.config.receiver_node, invoice.payment_hash())
            .map_err(|_| "Native original receiver lookup unavailable before admission.")?
            .is_some()
        {
            return Err(
                "Already received or pending payment cannot receive new commission terms.".into(),
            );
        }
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let customer = self.customer(source)?;
        let actor = &source.view().customer.context.account;
        let mut result = None;
        self.accounts.with_commission_agreement(actor,&customer,None,||self.buyer(source).is_ok()&&self.operator().is_ok(),|v|{
            result=Some((||{
                if !v.active_for_new_transactions||!v.terms_qualified||!v.terms.terms.products.contains(&contract::Product::PluginCall)||v.agreement.binding.referrer.source_only||v.current_referrer_owner.as_ref().is_none_or(|owner|owner==actor||owner==&self.config.operator_account){return Err("Current independent attribution and bilateral plugin commission terms required.".into());}
                // The native contract also excludes self referral. Both native
                // acceptances and original source identity stay frozen here.
                let t=&v.terms.terms;
                let a=Admission{schema:pay_ledger::commission::SCHEMA.into(),id:hash(encode(&(ledger.origin().map_err(|_|"Native ledger origin unavailable.")?,offer.request_hash.clone(),invoice.payment_hash(),source.authorization()))?),ledger_origin:ledger.origin().map_err(|_|"Native ledger origin unavailable.")?,payment_hash:hash_key(invoice.payment_hash()),request_hash:offer.request_hash.clone(),authorization:openagents_x402::outcome::commitment(source.authorization()),buyer_account:actor.clone(),buyer_workspace:source.view().customer.context.workspace.clone(),operator_account:self.config.operator_account.clone(),operator_workspace:self.config.operator_workspace.clone(),customer:customer.clone(),referrer:v.agreement.binding.referrer.id.clone(),party:format!("referrer:{}",v.agreement.binding.referrer.id),agreement:v.agreement.id.clone(),terms:t.digest.clone(),contract:String::from_utf8(encode(v)?).map_err(|_|"Contract encoding failed.")?,offer_digest:offer.digest(),invoice:offer.invoice().into(),receiver:self.config.receiver_node.clone(),payer:offer.payer.node.clone(),plugin:offer.quote.plugin.clone().ok_or("Native plugin identity required.")?,release:offer.quote.release.clone().ok_or("Native plugin release required.")?,author:offer.quote.author.clone().ok_or("Native author identity required.")?,author_fee_msat:offer.quote.fee_msat.ok_or("Native signed author fee required.")?,price_msat:offer.quote.price_msat,numerator:t.share.numerator,denominator:t.share.denominator,exact_rounding:t.rounding==tenancy::money::funding::Rounding::Exact,hold_secs:t.hold_secs,minimum_msat:t.amount_msat(t.minimum).map_err(|_|"Native payout precision required.")?,destinations:t.destinations.iter().map(|d|match d{contract::Destination::QualifiedSpark=>"spark".into(),contract::Destination::QualifiedLightningAddress=>"lud16".into()}).collect(),costs:costs.costs(t,&offer.digest(),&self.config.operator_account)?,cost_policy:costs.digest.clone(),admitted_at:now};
                check_cost_policy(&policy,&costs)?;self.buyer(source)?;self.operator()?;ledger.require_native_custody().map_err(|e|e.to_string())?;
                ledger.admit_commission(&a).map_err(|e|e.to_string())
            })());Ok(())
        }).map_err(|_|"Native commission agreement custody or authorization refused.")?;
        result.ok_or("Native commission admission absent.")?
    }
    pub fn reconcile(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        id: &str,
        wallet: &dyn LightningWallet,
        now: u64,
    ) -> Result<Report, String> {
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let a = ledger
            .commission_admission(id)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission unavailable.")?;
        self.matches(&a, source, ledger)?;
        let store = openagents_x402::outcome::Store::open(&self.config.outcomes)?;
        let inspection = store.inspect(
            &source.view().offer.payer.network,
            &a.payment_hash,
            &a.request_hash,
            source.authorization(),
        )?;
        let view = inspection.view()?;
        let original = inspection.merchant_settlement()?;
        if view.identity.invoice != a.invoice
            || view.identity.quote != source.view().offer.quote
            || view.identity.authorization != a.authorization
            || original.role != "plugin_call"
            || original.plugin.as_deref() != Some(&a.plugin)
            || original.release.as_deref() != Some(&a.release)
            || original.author.as_deref() != Some(&a.author)
            || original.fee_msat != Some(a.author_fee_msat)
        {
            return Err("Original native merchant outcome changed.".into());
        }
        let invoice = nostr::x402::decode_invoice(&a.invoice)
            .map_err(|_| "Retained merchant invoice invalid.")?;
        let payment = wallet
            .lookup_from_node(&a.receiver, invoice.payment_hash())
            .map_err(|_| "Native receiver lookup unavailable; liability stays unresolved.")?;
        if payment
            .as_ref()
            .is_none_or(|p| p.status != PaymentStatus::Succeeded)
        {
            inspection.current()?;
            return self.report(ledger, source, id);
        }
        let payment = payment.unwrap();
        verify_payment(
            &payment,
            &invoice,
            &a.invoice,
            PaymentDirection::Inbound,
            now,
        )?;
        if original.received_msat != a.price_msat || !original.received_from_wallet {
            return Err("Native full inbound collection required.".into());
        }
        let completed = match view.stage {
            openagents_x402::outcome::Stage::Completed => {
                verify_result(&view, &source.view().offer)?;
                Some(true)
            }
            openagents_x402::outcome::Stage::Failed => Some(false),
            _ => None,
        };
        let evidence = hash(encode(&view)?);
        let actor = &a.buyer_account;
        let mut result = None;
        self.accounts
            .with_commission_agreement(
                actor,
                &a.customer,
                Some(&a.agreement),
                || self.buyer(source).is_ok() && self.operator().is_ok(),
                |v| {
                    result = Some((|| {
                        let retained: contract::View = serde_json::from_str(&a.contract)
                            .map_err(|_| "Retained original agreement invalid.")?;
                        if v.agreement != retained.agreement || v.terms != retained.terms {
                            return Err("Original admitted bilateral agreement changed.".into());
                        }
                        // Current eligibility is for new transactions. It cannot erase
                        // the terms and attribution already admitted for this purchase.
                        inspection.current()?;
                        self.buyer(source)?;
                        self.operator()?;
                        ledger.require_native_custody().map_err(|e| e.to_string())?;
                        if v.current_referrer_owner.as_ref().is_none_or(|owner| {
                            owner == actor || owner == &self.config.operator_account
                        }) {
                            let proof = format!(
                                "sha256:{}",
                                hash(encode(&(id, &v.current_referrer_owner))?)
                            );
                            ledger
                                .hold_commission_signal(
                                    id,
                                    "native-identity-overlap",
                                    pay_ledger::commission_abuse::Reason::IdentityOverlap,
                                    &proof,
                                    now,
                                )
                                .map_err(|e| e.to_string())?;
                            if ledger
                                .commission_report(id)
                                .map_err(|e| e.to_string())?
                                .state
                                == "earned"
                            {
                                return ledger.commission_report(id).map_err(|e| e.to_string());
                            }
                            return ledger
                                .observe_commission(id, &evidence, Some(false), now)
                                .map_err(|e| e.to_string());
                        }
                        ledger
                            .observe_commission(id, &evidence, completed, now)
                            .map_err(|e| e.to_string())
                    })());
                    Ok(())
                },
            )
            .map_err(|_| "Original native agreement custody refused.")?;
        result.ok_or("Native reconciliation absent.")?
    }
    fn matches(
        &self,
        a: &Admission,
        source: &CommissionSource<'_>,
        ledger: &Ledger,
    ) -> Result<(), String> {
        if a.ledger_origin != ledger.origin().map_err(|e| e.to_string())?
            || a.buyer_account != source.view().customer.context.account
            || a.buyer_workspace != source.view().customer.context.workspace
            || a.offer_digest != source.view().offer.digest()
            || a.authorization != openagents_x402::outcome::commitment(source.authorization())
            || a.receiver != self.config.receiver_node
            || a.operator_account != self.config.operator_account
            || a.operator_workspace != self.config.operator_workspace
        {
            return Err("Original protected buyer purchase and ledger custody required.".into());
        }
        Ok(())
    }
}
fn hash_key<const N: usize>(bytes: [u8; N]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn verify_payment(
    payment: &openagents_wallet::PaymentRecord,
    invoice: &nostr::x402::Invoice,
    bolt11: &str,
    direction: PaymentDirection,
    now: u64,
) -> Result<(), String> {
    let end = invoice
        .created_at()
        .checked_add(invoice.expiry_seconds())
        .and_then(|v| v.checked_add(nostr::x402::DEFAULT_CLOCK_SKEW))
        .ok_or("Invoice deadline overflow.")?;
    if payment.payment_hash != hash_key(invoice.payment_hash())
        || payment.direction != direction
        || payment.status != PaymentStatus::Succeeded
        || payment.amount_msat != Some(invoice.amount_msat())
        || payment.bolt11.as_ref().is_some_and(|v| v != bolt11)
        || payment.updated_at < invoice.created_at()
        || payment.updated_at > end
        || payment.updated_at > now
        || payment.preimage.as_ref().is_some_and(|p| {
            openagents_wallet::parse_hash32(p)
                .map(|v| hash(v) != hash_key(invoice.payment_hash()))
                .unwrap_or(true)
        })
    {
        return Err(
            "Original exact native payment record does not match retained signed invoice.".into(),
        );
    }
    Ok(())
}
fn verify_result(
    view: &openagents_x402::outcome::View,
    offer: &coder::customer::plugins::Offer,
) -> Result<(), String> {
    let response = view
        .response
        .as_ref()
        .ok_or("Completed native response required.")?;
    let value: serde_json::Value =
        serde_json::from_slice(&response.body).map_err(|_| "Native plugin result is malformed.")?;
    if value["plugin"].as_str() != offer.quote.plugin.as_deref()
        || value["release"].as_str() != offer.quote.release.as_deref()
    {
        return Err("Original plugin result release changed.".into());
    }
    let receipt = plugin::InvocationReceipt::from_json(&encode(&value["receipt"])?)?;
    if receipt.module != offer.packet.module
        || receipt.input != offer.packet.input
        || receipt.operation != offer.packet.operation
        || serde_json::to_value(receipt.to_json()["limits"].clone())
            .map_err(|_| "Receipt limits invalid.")?
            != offer.packet.limits
        || receipt.to_json()["profile"].as_str() != Some(&offer.packet.profile)
        || !receipt.required
        || receipt.engine != plugin::ENGINE
    {
        return Err("Native invocation receipt does not match original packet.".into());
    }
    match receipt.outcome {
        plugin::Outcome::Value { status, output }
            if value["status"].as_str() == Some(&status)
                && plugin::digest(plugin::canonical(&value["value"]).as_bytes()) == output =>
        {
            Ok(())
        }
        _ => Err("Native successful delivery receipt required.".into()),
    }
}

impl Native {
    /// Issue a refund invoice on the original buyer resident after authenticating
    /// both original buyer and current merchant owner. This does not send funds.
    #[allow(clippy::too_many_arguments)]
    fn prepare_refund_record(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        admission: &str,
        request: &str,
        amount: u64,
        now: u64,
        expiry: u32,
    ) -> Result<(pay_ledger::commission::Refund, bool), String> {
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        if request.is_empty()
            || request.len() > 128
            || request.chars().any(char::is_control)
            || expiry < 60
            || expiry > 3600
        {
            return Err("Bounded explicit refund request and expiry required.".into());
        }
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let a = ledger
            .commission_admission(admission)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission required.")?;
        self.matches(&a, source, ledger)?;
        let report = ledger
            .commission_report(admission)
            .map_err(|e| e.to_string())?;
        if report.state == "admitted-unsettled" {
            return Err(
                "Original verified merchant collection required before refund preparation.".into(),
            );
        }
        let id = hash(encode(&(
            "native-refund",
            &a.ledger_origin,
            admission,
            request,
        ))?);
        if let Some(old) = ledger.commission_refund(&id).map_err(|e| e.to_string())? {
            if old.amount_msat != amount
                || old.buyer != a.buyer_account
                || old.operator != self.config.operator_account
            {
                return Err("Original refund request changed.".into());
            }
            return Ok((old, false));
        }
        let description_hash = hash(encode(&(
            "openagents.commission-refund.v1",
            &id,
            admission,
            &a.payment_hash,
            &a.request_hash,
            amount,
            &a.buyer_account,
            &self.config.operator_account,
        ))?);
        let r = pay_ledger::commission::Refund {
            schema: "openagents.commission-refund.v1".into(),
            id,
            admission: admission.into(),
            amount_msat: amount,
            description_hash,
            buyer: a.buyer_account.clone(),
            operator: self.config.operator_account.clone(),
            beneficiary_node: a.payer.clone(),
            merchant_node: a.receiver.clone(),
            created_at: now,
            invoice: None,
            state: "preparing".into(),
        };
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        ledger
            .begin_commission_refund_once(&r)
            .map_err(|e| e.to_string())
    }
    /// Resolve only the recorded outbound transfer from the original merchant.
    /// No recipient declaration, client receipt, or amount-only label proves it.
    fn reconcile_refund_observation(
        &self,
        ledger: &mut Ledger,
        r: &pay_ledger::commission::Refund,
        payment: Option<openagents_wallet::PaymentRecord>,
        now: u64,
    ) -> Result<Report, String> {
        if ledger
            .commission_refund(&r.id)
            .map_err(|e| e.to_string())?
            .as_ref()
            != Some(r)
        {
            return Err(
                "Original refund changed during native lookup; reread its retained history.".into(),
            );
        }
        let bolt11 = r
            .invoice
            .as_deref()
            .ok_or("Refund issuance remains unknown; do not remint or infer payment.")?;
        let invoice = nostr::x402::decode_invoice(bolt11)
            .map_err(|_| "Retained native refund invoice invalid.")?;
        if hash_key(invoice.payee()) != r.beneficiary_node
            || hash_key(invoice.description_hash()) != r.description_hash
            || invoice.amount_msat() != r.amount_msat
            || invoice.created_at() < r.created_at
        {
            return Err("Retained signed refund binding changed.".into());
        }
        let Some(payment) = payment else {
            ledger
                .set_commission_refund_outcome(&r.id, "unknown")
                .map_err(|e| e.to_string())?;
            return ledger
                .commission_report(&r.admission)
                .map_err(|e| e.to_string());
        };
        match payment.status {
            PaymentStatus::Succeeded => {
                if r.state == "failed" {
                    return Err("A terminal failed refund cannot become successful; preserve the original journal for reconciliation.".into());
                }
                verify_payment(&payment, &invoice, bolt11, PaymentDirection::Outbound, now)?;
                let proof = hash(encode(&(
                    &r.id,
                    &r.admission,
                    r.amount_msat,
                    &r.description_hash,
                    &r.buyer,
                    &r.operator,
                    &r.beneficiary_node,
                    &r.merchant_node,
                    r.created_at,
                    bolt11,
                    &payment.payment_hash,
                    payment.direction,
                    payment.amount_msat,
                ))?);
                ledger
                    .reverse_commission_with_payment(
                        &r.admission,
                        &r.id,
                        &proof,
                        r.amount_msat as i64,
                        r.created_at as i64,
                        &String::from_utf8(encode(&serde_json::json!({"receiver":r.merchant_node,"invoice":bolt11,"payment":payment}))?).map_err(|_|"Native refund evidence encoding failed.")?,
                    )
                    .map_err(|e| e.to_string())?;
                ledger
                    .set_commission_refund_outcome(&r.id, "reversed")
                    .map_err(|e| e.to_string())?;
                ledger
                    .commission_report(&r.admission)
                    .map_err(|e| e.to_string())
            }
            PaymentStatus::Failed => {
                if payment.payment_hash != hash_key(invoice.payment_hash())
                    || payment.direction != PaymentDirection::Outbound
                    || payment.amount_msat != Some(r.amount_msat)
                    || payment.updated_at < r.created_at
                    || payment.updated_at > now
                    || payment.bolt11.as_ref().is_some_and(|v| v != bolt11)
                {
                    return Err("Native failed refund identity disagrees.".into());
                }
                ledger
                    .set_commission_refund_outcome(&r.id, "failed")
                    .map_err(|e| e.to_string())?;
                ledger
                    .commission_report(&r.admission)
                    .map_err(|e| e.to_string())
            }
            _ => {
                ledger
                    .set_commission_refund_outcome(&r.id, "unknown")
                    .map_err(|e| e.to_string())?;
                ledger
                    .commission_report(&r.admission)
                    .map_err(|e| e.to_string())
            }
        }
    }
    pub fn report(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        id: &str,
    ) -> Result<Report, String> {
        self.buyer(source)?;
        self.operator()?;
        ledger.require_native_custody().map_err(|e| e.to_string())?;
        let a = ledger
            .commission_admission(id)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission unavailable.")?;
        self.matches(&a, source, ledger)?;
        self.original(ledger, source, &a, |ledger| {
            ledger.commission_report(id).map_err(|e| e.to_string())
        })
    }
}

impl Native {
    fn original<R>(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        a: &Admission,
        use_original: impl FnOnce(&mut Ledger) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut result = None;
        self.accounts
            .with_commission_agreement(
                &a.buyer_account,
                &a.customer,
                Some(&a.agreement),
                || self.buyer(source).is_ok() && self.operator().is_ok(),
                |v| {
                    let old: contract::View = serde_json::from_str(&a.contract)
                        .map_err(|_| tenancy::accounts::referrals::Error::Invalid)?;
                    if old.agreement != v.agreement || old.terms != v.terms {
                        return Err(tenancy::accounts::referrals::Error::Conflict);
                    }
                    result = Some((|| {
                        self.buyer(source)?;
                        self.operator()?;
                        ledger.require_native_custody().map_err(|e| e.to_string())?;
                        use_original(ledger)
                    })());
                    Ok(())
                },
            )
            .map_err(|_| "Original native commission custody or current authority refused.")?;
        result.ok_or("Native original commission operation absent.")?
    }
    pub fn qualify_costs(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        id: &str,
        policy_path: &Path,
    ) -> Result<(), String> {
        let a = ledger
            .commission_admission(id)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission unavailable.")?;
        self.matches(&a, source, ledger)?;
        let held = crate::Held::open(policy_path)?;
        let policy: CostPolicy = serde_json::from_slice(&held.bytes(64 * 1024)?)
            .map_err(|_| "Native cost policy invalid.")?;
        let original: contract::View =
            serde_json::from_str(&a.contract).map_err(|_| "Original contract invalid.")?;
        let costs = policy.costs(
            &original.terms.terms,
            &a.offer_digest,
            &self.config.operator_account,
        )?;
        self.original(ledger, source, &a, |ledger| {
            check_cost_policy(&held, &policy)?;
            ledger
                .qualify_commission_costs(id, &policy.digest, &costs)
                .map_err(|e| e.to_string())
        })
    }
}

impl Native {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_refund(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        admission: &str,
        request: &str,
        amount: u64,
        buyer_wallet: &dyn LightningWallet,
        now: u64,
        expiry: u32,
    ) -> Result<pay_ledger::commission::Refund, String> {
        let a = ledger
            .commission_admission(admission)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission unavailable.")?;
        self.matches(&a, source, ledger)?;
        let (r, created) = self.original(ledger, source, &a, |ledger| {
            self.prepare_refund_record(ledger, source, admission, request, amount, now, expiry)
        })?;
        if !created {
            return Ok(r);
        }
        // The once-only preparation is durable before IPC. Native account
        // mutations remain available while the resident answers; a changed
        // grant leaves the original preparation unknown, never reminted.
        self.original(ledger, source, &a, |_| Ok(()))?;
        let issued = buyer_wallet.receive_exact_from_node(&r.beneficiary_node,amount,
            openagents_wallet::parse_hash32(&r.description_hash).map_err(|_|"Refund binding invalid.")?,expiry)
            .map_err(|_|"Refund invoice issuance is unknown; retain its original preparation and do not remint.")?;
        let invoice = nostr::x402::decode_invoice(&issued.bolt11)
            .map_err(|_| "Native buyer refund invoice invalid; preparation remains held.")?;
        if hash_key(invoice.payee()) != a.payer
            || invoice.amount_msat() != amount
            || hash_key(invoice.description_hash()) != r.description_hash
            || hash_key(invoice.payment_hash()) != issued.payment_hash
            || invoice.created_at() < now
            || invoice.expiry_seconds() > u64::from(expiry)
        {
            return Err("Native refund invoice changed its exact original terms.".into());
        }
        self.original(ledger, source, &a, |ledger| {
            if ledger
                .commission_refund(&r.id)
                .map_err(|e| e.to_string())?
                .as_ref()
                != Some(&r)
            {
                return Err(
                    "Original refund changed during issuance; retain its unknown preparation."
                        .into(),
                );
            }
            ledger
                .seal_commission_refund(&r.id, &issued.bolt11)
                .map_err(|e| e.to_string())
        })
    }
    pub fn reconcile_refund(
        &self,
        ledger: &mut Ledger,
        source: &CommissionSource<'_>,
        refund: &str,
        merchant_wallet: &dyn LightningWallet,
        now: u64,
    ) -> Result<Report, String> {
        let r = ledger
            .commission_refund(refund)
            .map_err(|e| e.to_string())?
            .ok_or("Original native refund unavailable.")?;
        let a = ledger
            .commission_admission(&r.admission)
            .map_err(|e| e.to_string())?
            .ok_or("Original commission admission unavailable.")?;
        self.matches(&a, source, ledger)?;
        self.original(ledger, source, &a, |ledger| {
            if r.operator != a.operator_account
                || r.buyer != a.buyer_account
                || r.merchant_node != a.receiver
                || r.beneficiary_node != a.payer
                || ledger
                    .commission_refund(refund)
                    .map_err(|e| e.to_string())?
                    .as_ref()
                    != Some(&r)
            {
                return Err("Original native refund authority changed.".into());
            }
            Ok(())
        })?;
        let bolt11 = r
            .invoice
            .as_deref()
            .ok_or("Refund issuance remains unknown; do not remint or infer payment.")?;
        let invoice = nostr::x402::decode_invoice(bolt11)
            .map_err(|_| "Retained native refund invoice invalid.")?;
        if hash_key(invoice.payee()) != r.beneficiary_node
            || hash_key(invoice.description_hash()) != r.description_hash
            || invoice.amount_msat() != r.amount_msat
            || invoice.created_at() < r.created_at
        {
            return Err("Retained signed refund binding changed.".into());
        }
        let payment = merchant_wallet
            .lookup_from_node(&r.merchant_node, invoice.payment_hash())
            .map_err(
                |_| "Native outbound refund lookup unavailable; original liability stays held.",
            )?;
        self.original(ledger, source, &a, |ledger| {
            self.reconcile_refund_observation(ledger, &r, payment, now)
        })
    }
}

fn check_cost_policy(held: &crate::Held, expected: &CostPolicy) -> Result<(), String> {
    let current: CostPolicy = serde_json::from_slice(&held.bytes(64 * 1024)?)
        .map_err(|_| "Native cost policy changed.")?;
    if current.digest != expected.digest || current.clone().seal()?.digest != current.digest {
        return Err("Exact native cost policy changed.".into());
    }
    Ok(())
}
