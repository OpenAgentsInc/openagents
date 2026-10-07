//! Exact plugin approvals and uncertain charges in the existing customer book.
use super::*;
use nostr::x402::{SupportedProfiles, binding_hash, http_binding, validate_challenge};
use openagents_x402::{PaymentRequired, SettlementResponse, execution, front};
use serde_json::json;

/// Only an explicitly selected resident node can pay this purchase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payer {
    pub home: PathBuf,
    pub node: String,
    pub network: String,
}
/// Digests and bounds verified from the actual signed executable packet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub module: String,
    pub input: String,
    pub operation: String,
    pub profile: String,
    pub limits: Value,
}
/// A fresh native read identity from the selected credential and service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReader {
    pub origin: String,
    pub credential_alias: String,
    pub identity: receipts::purchase::PluginReadIdentity,
}
/// The front quote, exact invoice, and selected wallet are approved together.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub url: String,
    pub relay: String,
    pub blossom: Option<String>,
    pub quote: front::Quote,
    pub payment: PaymentRequired,
    pub packet: Packet,
    pub payer: Payer,
    pub max_msat: u64,
    pub max_fee_msat: u64,
    pub request_hash: String,
    pub expires_at_ms: u64,
    /// Commitment to the original private purchase authorization, never a payer identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_authorization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<receipts::shared_spend::Reference>,
}
impl Offer {
    pub fn body(&self, request: &str) -> Vec<u8> {
        let mut body =
            json!({"quote_digest":execution::quote_digest(&self.quote),"request":request});
        if let Some(authorization) = &self.recovery_authorization {
            body["recovery_authorization"] = json!(authorization);
        }
        serde_json::to_vec(&body).expect("plugin request serializes")
    }
    pub fn digest(&self) -> String {
        digest_request(&serde_json::to_value(self).expect("plugin offer serializes"))
    }
    pub fn invoice(&self) -> &str {
        self.payment.accepts[0].extra["invoice"]
            .as_str()
            .unwrap_or_default()
    }
    fn validate(&self, request: &str, selected: &Selection, now: u64) -> Result<()> {
        let url = reqwest::Url::parse(&self.url).map_err(|_| "Invalid plugin resource.")?;
        let sum = self
            .quote
            .parts
            .iter()
            .try_fold(0u64, |n, p| n.checked_add(p.msat));
        let fee = self.quote.fee_msat.ok_or("Missing signed author fee.")?;
        if url.origin().ascii_serialization() != selected.origin
            || self.shared.as_ref().is_some_and(|s| {
                self.commercial.as_ref() != Some(&s.mode.commercial)
                    || s.mode.source.product != receipts::purchase::CommercialProduct::Plugin
                    || s.mode.source.account != selected.context.account
                    || s.mode.source.workspace.as_deref() != Some(&selected.context.workspace)
                    || s.mode.node != self.payer.node
                    || s.digest.len() != 64
                    || !s.intent.starts_with("shared:")
                    || s.intent.len() != 71
            })
            || self.commercial.as_ref().is_some_and(|r| {
                r.validate().is_err()
                    || !r.matches_native(
                        receipts::purchase::CommercialProduct::Plugin,
                        &selected.context.account,
                        Some(&selected.context.workspace),
                    )
            })
            || selected.context.commercial.as_ref().is_some_and(|gateway| {
                self.commercial.as_ref().is_none_or(|r| {
                    gateway.binding != r.binding
                        || gateway.revision != r.revision
                        || gateway.digest != r.digest
                        || gateway.customer != r.customer
                        || gateway.workspace != r.workspace
                })
            })
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || self.quote.plugin.as_deref().is_none_or(str::is_empty)
            || self.quote.release.as_deref().is_none_or(|s| s.len() != 64)
            || self.quote.author.as_deref().is_none_or(|s| s.len() != 64)
            || self.quote.parts.len() != 2
            || self.quote.parts[0].name != "endpoint"
            || self.quote.parts[1].name != "author_fee"
            || self.quote.parts[1].msat != fee
            || sum != Some(self.quote.price_msat)
            || self.quote.price_msat == 0
            || self.quote.price_msat > self.max_msat
            || self.payment.x402_version != 2
            || self.payment.accepts.len() != 1
            || self.payment.resource.url != self.url
            || request.len() > 32 * 1024
            || !self.payer.home.is_absolute()
            || self.payer.node.len() != 66
            || self.packet.operation.is_empty()
            || !matches!(self.packet.profile.as_str(), "pure" | "snapshot-read")
            || self.packet.module.len() != 71
            || self.packet.input.len() != 71
            || now >= self.expires_at_ms
            || self
                .recovery_authorization
                .as_ref()
                .is_some_and(|s| !openagents_x402::outcome::token(s))
        {
            return Err("Plugin offer changes its exact resource, release, packet, total, payer, or expiry.".into());
        }
        let hash = binding_hash(
            &http_binding("POST", &self.url, &self.body(request), &[])
                .map_err(|_| "Invalid plugin request binding.")?,
        )
        .map_err(|_| "Invalid plugin request hash.")?;
        if hash != self.request_hash {
            return Err("Plugin input differs from its exact invoice binding.".into());
        }
        let terms = &self.payment.accepts[0];
        let invoice = validate_challenge(
            terms,
            &hash,
            now / 1000,
            0,
            SupportedProfiles {
                http: true,
                mcp: false,
                native: false,
            },
        )
        .map_err(|_| "Plugin invoice is expired or differs from the approved request.")?;
        if terms.network != self.payer.network
            || invoice.amount_msat() != self.quote.price_msat
            || self.expires_at_ms
                > invoice
                    .created_at()
                    .saturating_add(invoice.expiry_seconds())
                    .saturating_mul(1000)
        {
            return Err("Plugin invoice changes amount, network, or expiry.".into());
        }
        Ok(())
    }
}
/// A protected native approval permits only its original payment attempt.
#[derive(Clone)]
pub struct SharedApproval {
    pub purchase: String,
    pub offer: Offer,
    pub selection: Selection,
    pub approval_digest: String,
}
impl Store {
    /// Read an already committed approval without taking the buyer's writer lock.
    /// This never changes an uncertain purchase or issues invocation authority.
    #[cfg(unix)]
    pub fn shared_plugin_approval(root: &Path, id: &str, now: u64) -> Result<SharedApproval> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        if !root.is_absolute() || !alias(id) {
            return Err("An exact private buyer directory and purchase are required.".into());
        }
        let dir = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(root)
            .map_err(|_| "Private buyer directory is unavailable.")?;
        let original = dir
            .metadata()
            .map_err(|_| "Private buyer directory is unavailable.")?;
        if !original.is_dir()
            || original.mode() & 0o077 != 0
            || original.uid() != unsafe { libc::geteuid() }
        {
            return Err("Private buyer directory is required.".into());
        }
        let path = root.join("state.json");
        let file = task::private_open(&path, false, false)
            .map_err(|_| "Private buyer approval is unavailable.")?;
        let meta = file
            .metadata()
            .map_err(|_| "Private buyer approval is unavailable.")?;
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err("Private buyer approval is unavailable.".into());
        }
        let mut bytes = Vec::new();
        (&file)
            .take(MAX_STATE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Private buyer approval is unavailable.")?;
        if bytes.len() > MAX_STATE {
            return Err("Private buyer state exceeds its bound.".into());
        }
        let book: Book =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid private buyer state.")?;
        super::check(&book)?;
        let p = book
            .plugin_purchases
            .get(id)
            .ok_or("Original plugin purchase is absent.")?;
        if p.phase != Phase::Paying || p.offer.shared.is_none() || p.charge.is_some() {
            return Err("Original paying approval is required for shared handoff.".into());
        }
        let approval = p
            .approval
            .as_ref()
            .ok_or("Original plugin approval is absent.")?;
        validate_approval(approval, &p.offer, &p.selection, now)?;
        p.offer.validate(&p.request, &p.selection, now)?;
        task::verify_same_file(&path, &file)
            .map_err(|_| "Buyer approval changed during admission.")?;
        let current = std::fs::symlink_metadata(root).map_err(|_| "Buyer directory changed.")?;
        if !current.is_dir() || current.dev() != original.dev() || current.ino() != original.ino() {
            return Err("Buyer directory changed during admission.".into());
        }
        Ok(SharedApproval {
            purchase: id.into(),
            offer: p.offer.clone(),
            selection: p.selection.clone(),
            approval_digest: approval.digest(),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Quoted,
    Approved,
    Cancelled,
    Paying,
    Paid,
    Completed,
    Failed,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charge {
    pub payment_hash: String,
    pub amount_msat: u64,
    pub fee_msat: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Purchase {
    selection: Selection,
    offer: Offer,
    request: String,
    created_at_ms: u64,
    approval: Option<Approval>,
    phase: Phase,
    charge: Option<Charge>,
    settlement: Option<SettlementResponse>,
    result: Option<Value>,
    delivery_status: Option<u16>,
    #[serde(default)]
    recovery_secret: Option<String>,
    #[serde(default)]
    recovery: Option<openagents_x402::outcome::View>,
}
impl Purchase {
    pub(super) fn recover(&mut self) -> bool {
        if matches!(self.phase, Phase::Paying | Phase::Paid) {
            self.phase = Phase::Unknown;
            true
        } else {
            false
        }
    }
    fn unresolved(&self) -> bool {
        matches!(self.phase, Phase::Paying | Phase::Paid | Phase::Unknown)
    }
}
/// Attribution and actual receipt; private input and preimage are omitted.
#[derive(Clone, Serialize)]
pub struct View {
    pub id: String,
    pub customer: Selection,
    pub offer: Offer,
    pub approval_digest: String,
    pub phase: Phase,
    pub charge: Option<Charge>,
    pub settlement: Option<SettlementResponse>,
    pub result: Option<Value>,
    pub delivery_status: Option<u16>,
    pub unresolved_maximum_msat: Option<u64>,
    pub recovery: Option<openagents_x402::outcome::View>,
}
fn quote(id: &str, p: &Purchase) -> Quote {
    Quote {
        id: id.into(),
        context: p.selection.context.clone(),
        request_digest: p.offer.digest(),
        created_at_ms: p.created_at_ms,
        expires_at_ms: p.offer.expires_at_ms,
    }
}
fn validate_approval(
    approval: &Approval,
    offer: &Offer,
    current: &Selection,
    now: u64,
) -> Result<()> {
    if offer.commercial.is_none() {
        return approval
            .validate_current(&current.context, &offer.digest(), now)
            .map_err(str::to_owned);
    }
    // A mapped plugin's native membership and signed wallet offer are its
    // admission. Decision availability remains an independent, honest field.
    current.context.validate().map_err(str::to_owned)?;
    let quote = &approval.quote;
    if quote.context != current.context
        || quote.request_digest != offer.digest()
        || quote.expires_at_ms <= quote.created_at_ms
        || quote.expires_at_ms - quote.created_at_ms > MAX_QUOTE_MS
        || approval.approved_at_ms < quote.created_at_ms
        || approval.approved_at_ms > now
        || now >= quote.expires_at_ms
    {
        return Err(
            "Plugin approval is expired or changes its native customer, mapping, or signed offer."
                .into(),
        );
    }
    Ok(())
}
pub(super) fn check(book: &Book) -> Result<()> {
    if book.plugin_purchases.len() > MAX_PURCHASES {
        return Err("Too many retained plugin purchases.".into());
    }
    for (id, p) in &book.plugin_purchases {
        selection_valid(&p.selection)?;
        if !alias(id)
            || p.created_at_ms >= p.offer.expires_at_ms
            || p.offer.expires_at_ms - p.created_at_ms > MAX_QUOTE_MS
            || p.offer
                .validate(
                    &p.request,
                    &p.selection,
                    p.offer.expires_at_ms.saturating_sub(1),
                )
                .is_err()
            || matches!(p.phase, Phase::Quoted | Phase::Cancelled) != p.approval.is_none()
            || matches!(p.phase, Phase::Paid | Phase::Completed | Phase::Failed)
                && p.charge.is_none()
            || p.offer.recovery_authorization
                != p.recovery_secret
                    .as_ref()
                    .map(|s| openagents_x402::outcome::commitment(s))
            || p.recovery_secret
                .as_ref()
                .is_some_and(|s| !openagents_x402::outcome::token(s))
        {
            return Err("Retained plugin purchase attribution changed.".into());
        }
        if let Some(a) = &p.approval {
            if a.quote != quote(id, p)
                || a.approved_at_ms < a.quote.created_at_ms
                || a.approved_at_ms >= a.quote.expires_at_ms
            {
                return Err("Retained plugin approval changed.".into());
            }
        }
        if let Some(charge) = &p.charge {
            let invoice = nostr::x402::decode_invoice(p.offer.invoice())
                .map_err(|_| "Retained plugin invoice changed.")?;
            let hash = invoice
                .payment_hash()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            if charge.payment_hash != hash
                || charge.amount_msat != p.offer.quote.price_msat
                || charge.fee_msat > p.offer.max_fee_msat
            {
                return Err("Retained plugin charge changed its invoice, amount, or fee.".into());
            }
        }
        if matches!(p.phase, Phase::Completed | Phase::Failed) {
            let charge = p
                .charge
                .as_ref()
                .ok_or("Terminal plugin purchase has no charge.")?;
            let settled = p
                .settlement
                .as_ref()
                .ok_or("Terminal plugin purchase has no settlement.")?;
            if !settled.success
                || settled.transaction != charge.payment_hash
                || settled.network != p.offer.payer.network
                || settled.amount.as_deref() != Some(charge.amount_msat.to_string().as_str())
                || p.phase == Phase::Completed
                    && (p.result.is_none() || p.delivery_status != Some(200))
                || p.phase == Phase::Failed && p.delivery_status.is_none_or(|s| s < 400)
            {
                return Err("Retained plugin settlement or delivery position changed.".into());
            }
            if p.phase == Phase::Completed {
                check_result(
                    &p.offer,
                    charge,
                    p.result.as_ref().ok_or("Missing retained plugin result.")?,
                )?;
            }
        }
        if let Some(recovery) = &p.recovery {
            check_recovery(p, recovery)?;
        }
    }
    Ok(())
}
fn check_result(offer: &Offer, charge: &Charge, result: &Value) -> Result<()> {
    if result["plugin"] != json!(offer.quote.plugin)
        || result["release"] != json!(offer.quote.release)
        || result["verification"] != "not_run"
        || serde_json::to_vec(result)
            .map_err(|_| "Invalid plugin result.")?
            .len()
            > 128 * 1024
    {
        return Err("Plugin result changes its release, bound, or verification class.".into());
    }
    let receipt = plugin::InvocationReceipt::from_json(
        &serde_json::to_vec(&result["receipt"]).map_err(|_| "Invalid actual plugin receipt.")?,
    )
    .map_err(|_| "Invalid actual plugin receipt.")?;
    if receipt.module != offer.packet.module
        || receipt.input != offer.packet.input
        || receipt.operation != offer.packet.operation
        || receipt.invocation != charge.payment_hash
        || receipt.engine != plugin::ENGINE
        || !receipt.required
        || receipt.to_json()["profile"] != offer.packet.profile
        || receipt.to_json()["limits"] != offer.packet.limits
        || receipt.snapshot
            != plugin::digest(plugin::canonical(&json!({"entries":[],"handles":{}})).as_bytes())
        || receipt.outcome
            != (plugin::Outcome::Value {
                status: result["status"].as_str().unwrap_or_default().into(),
                output: plugin::digest(plugin::canonical(&result["value"]).as_bytes()),
            })
    {
        return Err(
            "Plugin execution receipt changes the approved packet, input, invocation, or output."
                .into(),
        );
    }
    Ok(())
}
impl Store {
    fn plugin(&self, id: &str) -> Result<&Purchase> {
        let p = self
            .book
            .plugin_purchases
            .get(id)
            .ok_or("Plugin purchase is unavailable.")?;
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select a customer first.")?;
        if !same_customer(selected, &p.selection) {
            return Err("Plugin purchase belongs to another selected customer.".into());
        }
        Ok(p)
    }
    pub fn plugin_view(&self, id: &str) -> Result<View> {
        let p = self.plugin(id)?;
        Ok(View {
            id: id.into(),
            customer: p.selection.clone(),
            offer: p.offer.clone(),
            approval_digest: quote(id, p).digest(),
            phase: p.phase,
            charge: p.charge.clone(),
            settlement: p.settlement.clone(),
            result: p.result.clone(),
            delivery_status: p.delivery_status,
            unresolved_maximum_msat: p.unresolved().then(|| {
                p.offer
                    .quote
                    .price_msat
                    .saturating_add(p.offer.max_fee_msat)
            }),
            recovery: p.recovery.clone(),
        })
    }
    pub fn plugin_liability(&self) -> bool {
        self.book.selected.as_ref().is_some_and(|s| {
            self.book
                .plugin_purchases
                .values()
                .any(|p| same_payer(s, &p.selection) && p.unresolved())
        })
    }
    fn plugin_wallet_liability(&self, payer: &Payer) -> bool {
        self.book.plugin_purchases.values().any(|p| {
            p.unresolved()
                && p.offer.payer.node == payer.node
                && p.offer.payer.network == payer.network
        })
    }
    pub fn quote_plugin(
        &mut self,
        id: &str,
        offer: Offer,
        request: String,
        current: Selection,
        now: u64,
    ) -> Result<View> {
        self.quote_plugin_with_recovery(id, offer, request, current, now, None)
    }
    /// Keep the original secret in private custody before approving its commitment.
    pub fn quote_plugin_with_recovery(
        &mut self,
        id: &str,
        offer: Offer,
        request: String,
        current: Selection,
        now: u64,
        secret: Option<String>,
    ) -> Result<View> {
        if current.context.team_policy.is_some() {
            return Err("This workspace has an active team policy; paid plugin execution is not a qualified route. Original financial recovery remains available.".into());
        }
        if offer.recovery_authorization
            != secret
                .as_ref()
                .map(|s| openagents_x402::outcome::commitment(s))
            || secret
                .as_ref()
                .is_some_and(|s| !openagents_x402::outcome::token(s))
        {
            return Err("Recovery must bind the original private purchase authorization.".into());
        }
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select a customer first.")?;
        if !alias(id)
            || self.book.plugin_purchases.contains_key(id)
            || self.book.plugin_purchases.len() >= MAX_PURCHASES
            || self.plugin_liability()
            || self.plugin_wallet_liability(&offer.payer)
            || selected != &current
            || !current.context.can_invoke && offer.commercial.is_none()
            || self.book.purchases.values().any(|p| {
                same_payer(selected, &p.selection)
                    && matches!(p.status, Status::Running | Status::Unknown)
            })
            || offer.expires_at_ms <= now
            || offer.expires_at_ms - now > MAX_QUOTE_MS
        {
            return Err("Plugin quote needs current customer rights, a fresh identity, and resolved payer liability.".into());
        }
        offer.validate(&request, &current, now)?;
        let mut next = self.book.clone();
        next.plugin_purchases.insert(
            id.into(),
            Purchase {
                selection: current,
                offer,
                request,
                created_at_ms: now,
                approval: None,
                phase: Phase::Quoted,
                charge: None,
                settlement: None,
                result: None,
                delivery_status: None,
                recovery_secret: secret,
                recovery: None,
            },
        );
        self.persist(next)?;
        self.plugin_view(id)
    }
    pub fn approve_plugin(
        &mut self,
        id: &str,
        digest: &str,
        current: &Selection,
        payer: &Payer,
        now: u64,
    ) -> Result<View> {
        self.approve_plugin_reviewed(id, digest, current, payer, None, now)
    }
    pub fn approve_plugin_reviewed(
        &mut self,
        id: &str,
        digest: &str,
        current: &Selection,
        payer: &Payer,
        commercial: Option<&receipts::purchase::CommercialRef>,
        now: u64,
    ) -> Result<View> {
        if current.context.team_policy.is_some() {
            return Err(
                "Paid plugin approval is unavailable under this workspace team policy.".into(),
            );
        }
        let p = self.plugin(id)?;
        let q = quote(id, p);
        if p.phase != Phase::Quoted
            || p.offer.commercial.as_ref() != commercial
            || q.digest() != digest
            || current != &p.selection
            || payer != &p.offer.payer
            || self.book.selected.as_ref() != Some(current)
        {
            return Err("Approve the exact reviewed plugin quote with its original customer and payer, once.".into());
        }
        p.offer.validate(&p.request, current, now)?;
        let a = Approval {
            quote: q,
            approved_at_ms: now,
        };
        validate_approval(&a, &p.offer, current, now)?;
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        p.approval = Some(a);
        p.phase = Phase::Approved;
        self.persist(next)?;
        self.plugin_view(id)
    }
    pub fn cancel_plugin(&mut self, id: &str) -> Result<View> {
        if !matches!(self.plugin(id)?.phase, Phase::Quoted | Phase::Approved) {
            return Err("A started payment cannot be cancelled or erased.".into());
        }
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        p.phase = Phase::Cancelled;
        p.approval = None;
        self.persist(next)?;
        self.plugin_view(id)
    }
    /// Persist the once-only dispatch fence before any wallet payment.
    pub fn begin_plugin(
        &mut self,
        id: &str,
        current: &Selection,
        payer: &Payer,
        packet: &Packet,
        now: u64,
    ) -> Result<(Offer, Vec<u8>)> {
        self.begin_plugin_reviewed(id, current, payer, packet, None, now)
    }
    pub fn begin_plugin_reviewed(
        &mut self,
        id: &str,
        current: &Selection,
        payer: &Payer,
        packet: &Packet,
        commercial: Option<&receipts::purchase::CommercialRef>,
        now: u64,
    ) -> Result<(Offer, Vec<u8>)> {
        if current.context.team_policy.is_some() {
            return Err(
                "Paid plugin dispatch is unavailable under this workspace team policy.".into(),
            );
        }
        let p = self.plugin(id)?;
        if p.phase != Phase::Approved
            || p.offer.commercial.as_ref() != commercial
            || current != &p.selection
            || payer != &p.offer.payer
            || packet != &p.offer.packet
            || self.book.selected.as_ref() != Some(current)
            || self.plugin_liability()
            || self.plugin_wallet_liability(payer)
            || self.book.purchases.values().any(|p| {
                same_payer(current, &p.selection)
                    && matches!(p.status, Status::Running | Status::Unknown)
            })
        {
            return Err("Plugin purchase is unapproved, changed, or already attempted; it cannot pay again.".into());
        }
        p.offer.validate(&p.request, current, now)?;
        validate_approval(
            p.approval.as_ref().ok_or("Missing plugin approval.")?,
            &p.offer,
            current,
            now,
        )?;
        let result = (p.offer.clone(), p.offer.body(&p.request));
        let mut next = self.book.clone();
        next.plugin_purchases.get_mut(id).unwrap().phase = Phase::Paying;
        self.persist(next)?;
        Ok(result)
    }
    pub fn plugin_request(&self, id: &str) -> Result<&str> {
        Ok(&self.plugin(id)?.request)
    }
    /// The caller authenticates the original customer before beginning payment.
    pub fn plugin_invocation_authorization(
        &self,
        id: &str,
        current: &Selection,
    ) -> Result<Option<String>> {
        self.plugin_invocation_authorization_reviewed(id, current, None)
    }
    pub fn plugin_invocation_authorization_reviewed(
        &self,
        id: &str,
        current: &Selection,
        commercial: Option<&receipts::purchase::CommercialRef>,
    ) -> Result<Option<String>> {
        let p = self.plugin(id)?;
        if p.phase != Phase::Approved
            || &p.selection != current
            || self.book.selected.as_ref() != Some(current)
            || p.offer.commercial.as_ref() != commercial
            || !current.context.can_invoke && p.offer.commercial.is_none()
        {
            return Err("Original current customer approval is required before exposing invocation authority.".into());
        }
        Ok(p.recovery_secret.clone())
    }
    pub fn plugin_paid(
        &mut self,
        id: &str,
        preimage: &str,
        charge: Charge,
        now: u64,
    ) -> Result<View> {
        let p = self.plugin(id)?;
        if p.phase != Phase::Paying {
            return Err("Plugin payment was not dispatched once.".into());
        }
        let terms = &p.offer.payment.accepts[0];
        let checked = nostr::x402::validate_paid_proof(
            terms,
            terms,
            preimage,
            now / 1000,
            nostr::x402::DEFAULT_CLOCK_SKEW,
            SupportedProfiles {
                http: true,
                mcp: false,
                native: false,
            },
        )
        .map_err(|_| "Wallet proof differs from the original invoice.")?;
        if charge.payment_hash != checked.payment_hash
            || charge.amount_msat != p.offer.quote.price_msat
            || charge.fee_msat > p.offer.max_fee_msat
        {
            return Err("Wallet disposition changes approved hash, amount, or fee ceiling.".into());
        }
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        p.charge = Some(charge);
        p.phase = Phase::Paid;
        self.persist(next)?;
        self.plugin_view(id)
    }
    pub fn plugin_unknown(&mut self, id: &str) -> Result<View> {
        if !self.plugin(id)?.unresolved() {
            return Err("Only a dispatched plugin purchase can become uncertain.".into());
        }
        let mut next = self.book.clone();
        next.plugin_purchases.get_mut(id).unwrap().phase = Phase::Unknown;
        self.persist(next)?;
        self.plugin_view(id)
    }
    /// Retain matching settlement and execution claims without attestation.
    pub fn plugin_delivered(
        &mut self,
        id: &str,
        settlement: SettlementResponse,
        result: Value,
    ) -> Result<View> {
        let p = self.plugin(id)?;
        let charge = p.charge.as_ref().ok_or("No confirmed plugin charge.")?;
        if !matches!(p.phase, Phase::Paid | Phase::Unknown | Phase::Completed)
            || !settlement.success
            || settlement.transaction != charge.payment_hash
            || settlement.network != p.offer.payer.network
            || settlement.amount.as_deref() != Some(p.offer.quote.price_msat.to_string().as_str())
        {
            return Err(
                "Plugin settlement or result changes the original paid identities and amount."
                    .into(),
            );
        }
        check_result(&p.offer, charge, &result)?;
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        p.phase = Phase::Completed;
        p.settlement = Some(settlement);
        p.result = Some(result);
        p.delivery_status = Some(200);
        self.persist(next)?;
        self.plugin_view(id)
    }
    /// A verified settled charge can still have a failed service delivery.
    pub fn plugin_failed_delivery(
        &mut self,
        id: &str,
        settlement: SettlementResponse,
        status: u16,
    ) -> Result<View> {
        let p = self.plugin(id)?;
        let charge = p.charge.as_ref().ok_or("No confirmed plugin charge.")?;
        if !matches!(p.phase, Phase::Paid | Phase::Unknown | Phase::Failed)
            || status < 400
            || !settlement.success
            || settlement.transaction != charge.payment_hash
            || settlement.network != p.offer.payer.network
            || settlement.amount.as_deref() != Some(p.offer.quote.price_msat.to_string().as_str())
        {
            return Err("Failed delivery has no matching original settlement.".into());
        }
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        p.phase = Phase::Failed;
        p.settlement = Some(settlement);
        p.delivery_status = Some(status);
        self.persist(next)?;
        self.plugin_view(id)
    }

    /// Recheck the selected authenticated principal before exposing recovery authority.
    pub fn plugin_recovery(
        &self,
        id: &str,
        current: &Selection,
        payer: &Payer,
    ) -> Result<(Offer, Vec<u8>, String)> {
        self.plugin_recovery_reviewed(id, current, payer, None)
    }
    pub fn plugin_recovery_reviewed(
        &self,
        id: &str,
        current: &Selection,
        payer: &Payer,
        commercial: Option<&receipts::purchase::CommercialRef>,
    ) -> Result<(Offer, Vec<u8>, String)> {
        let p = self.plugin(id)?;
        let mapped = p.offer.commercial.is_some();
        let current_source = match (&p.offer.commercial, commercial) {
            (None, None) => current.context.can_invoke,
            (Some(original), Some(current)) => {
                current.validate().is_ok() && original.source == current.source
            }
            _ => false,
        };
        if !matches!(
            p.phase,
            Phase::Unknown | Phase::Paid | Phase::Completed | Phase::Failed
        ) || !same_customer(current, &p.selection)
            || mapped && current.context.validate().is_err()
            || !mapped
                && (!same_identity(&current.context, &p.selection.context)
                    || current.credential_alias != p.selection.credential_alias)
            || !current_source
            || self.book.selected.as_ref().is_none_or(|s| {
                !same_customer(s, current) || s.credential_alias != current.credential_alias
            })
            || payer != &p.offer.payer
        {
            return Err("Recovery needs the original authenticated customer, current rights, and exact resident binding.".into());
        }
        // A reviewed rotation may read the original result. It cannot change
        // the historical principal, alias, offer, or once-only payment fence.
        Ok((p.offer.clone(), p.offer.body(&p.request), p.recovery_secret.clone().ok_or("This older purchase has no private recovery authorization; retain its receipt for support.")?))
    }
    /// Authenticate native reads even after canonical linkage is retired or revoked.
    pub async fn plugin_native_reader(&self, id: &str) -> Result<NativeReader> {
        let p = self.plugin(id)?;
        let source = &p
            .offer
            .commercial
            .as_ref()
            .ok_or("Legacy recovery needs its original customer rights.")?
            .source;
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select the original native customer first.")?;
        let identity = self.client(&selected.origin, &selected.credential_alias)?.account()
            .plugin_reader(source).await
            .map_err(|_| "Native Plugin read authentication is unavailable; retain the original liability.")?;
        Ok(NativeReader {
            origin: selected.origin.clone(),
            credential_alias: selected.credential_alias.clone(),
            identity,
        })
    }
    /// Expose only the original outcome authorization; this cannot approve or pay.
    pub fn plugin_recovery_native(
        &self,
        id: &str,
        reader: &NativeReader,
        payer: &Payer,
    ) -> Result<(Offer, Vec<u8>, String)> {
        let p = self.plugin(id)?;
        let original = p
            .offer
            .commercial
            .as_ref()
            .ok_or("Legacy recovery needs its original customer rights.")?;
        let selected = self
            .book
            .selected
            .as_ref()
            .ok_or("Select the original native customer first.")?;
        if !matches!(
            p.phase,
            Phase::Unknown | Phase::Paid | Phase::Completed | Phase::Failed
        ) || reader.identity.validate().is_err()
            || reader.identity.source != original.source
            || reader.identity.tenant != p.selection.context.tenant
            || reader.origin != p.selection.origin
            || reader.credential_alias != selected.credential_alias
            || !same_customer(selected, &p.selection)
            || payer != &p.offer.payer
        {
            return Err("Recovery needs current native read authentication, the original source, and exact resident binding.".into());
        }
        Ok((p.offer.clone(), p.offer.body(&p.request), p.recovery_secret.clone().ok_or("This older purchase has no private recovery authorization; retain its receipt for support.")?))
    }
    /// An exact resident lookup can recover a lost payment acknowledgment. It never pays.
    pub fn plugin_recovered_charge(
        &mut self,
        id: &str,
        charge: Charge,
        preimage: &str,
    ) -> Result<View> {
        use sha2::{Digest, Sha256};
        let p = self.plugin(id)?;
        let invoice = nostr::x402::decode_invoice(p.offer.invoice())
            .map_err(|_| "Retained invoice changed.")?;
        let hash = invoice
            .payment_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let bytes = (0..preimage.len())
            .step_by(2)
            .map(|i| {
                preimage
                    .get(i..i + 2)
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
            })
            .collect::<Option<Vec<_>>>();
        if !matches!(
            p.phase,
            Phase::Unknown | Phase::Paid | Phase::Completed | Phase::Failed
        ) || !openagents_x402::outcome::token(preimage)
            || bytes
                .as_ref()
                .is_none_or(|b| format!("{:x}", Sha256::digest(b)) != hash)
            || charge.payment_hash != hash
            || charge.amount_msat != p.offer.quote.price_msat
            || charge.fee_msat > p.offer.max_fee_msat
            || p.charge.as_ref().is_some_and(|old| old != &charge)
        {
            return Err(
                "Recovery lookup changes the exact paid invoice, amount, fee, or proof.".into(),
            );
        }
        let mut next = self.book.clone();
        next.plugin_purchases.get_mut(id).unwrap().charge = Some(charge);
        self.persist(next)?;
        self.plugin_view(id)
    }
    pub fn plugin_recovered(
        &mut self,
        id: &str,
        recovery: openagents_x402::outcome::View,
    ) -> Result<View> {
        let p = self.plugin(id)?;
        check_recovery(p, &recovery)?;
        let mut next = self.book.clone();
        let p = next.plugin_purchases.get_mut(id).unwrap();
        if let Some(response) = &recovery.response {
            let settlement = recovery
                .settlement
                .clone()
                .ok_or("Recovery result has no settlement.")?;
            if response.status == 200 {
                let value = serde_json::from_slice(&response.body)
                    .map_err(|_| "Recovery result is invalid.")?;
                check_result(
                    &p.offer,
                    p.charge.as_ref().ok_or("Missing recovered charge.")?,
                    &value,
                )?;
                if p.result.as_ref().is_some_and(|old| old != &value) {
                    return Err("A recovered result changes the original retained delivery.".into());
                }
                p.result = Some(value);
                p.phase = Phase::Completed;
            } else {
                p.phase = Phase::Failed;
            }
            p.settlement = Some(settlement);
            p.delivery_status = Some(response.status);
        }
        p.recovery = Some(recovery);
        self.persist(next)?;
        self.plugin_view(id)
    }
}

fn check_recovery(p: &Purchase, r: &openagents_x402::outcome::View) -> Result<()> {
    use openagents_x402::outcome::{SCHEMA, Stage};
    if r.schema != SCHEMA
        || r.identity.network != p.offer.payer.network
        || r.identity.invoice != p.offer.invoice()
        || r.identity.request_hash != p.offer.request_hash
        || r.identity.authorization != p.offer.recovery_authorization.clone().unwrap_or_default()
        || r.identity.quote != p.offer.quote
        || r.receipt_reference != r.identity.receipt_reference()
        || r.guidance.len() > 1024
        || p.charge
            .as_ref()
            .is_none_or(|c| c.payment_hash != r.identity.payment_hash)
        || r.response.is_some() != matches!(r.stage, Stage::Completed | Stage::Failed)
        || matches!(
            r.stage,
            Stage::Completed | Stage::Failed | Stage::Invoking | Stage::Settled
        ) != r.settlement.is_some()
        || r.settlement.as_ref().is_some_and(|s| {
            !s.success
                || s.transaction != r.identity.payment_hash
                || s.network != p.offer.payer.network
                || s.amount.as_deref() != Some(p.offer.quote.price_msat.to_string().as_str())
        })
        || p.phase == Phase::Completed && r.stage != Stage::Completed
        || p.phase == Phase::Failed && r.stage != Stage::Failed
    {
        return Err(
            "Recovery evidence changes the original protected purchase, payment, or disposition."
                .into(),
        );
    }
    if let Some(response) = &r.response {
        if response.body.len() > 128 * 1024
            || response.headers.len() > 16
            || response
                .headers
                .iter()
                .any(|(n, v)| n.len() > 128 || v.len() > 4096)
            || (r.stage == Stage::Completed) != (response.status == 200)
            || r.stage == Stage::Failed && response.status < 400
            || p.delivery_status.is_some_and(|old| old != response.status)
            || p.recovery
                .as_ref()
                .and_then(|old| old.response.as_ref())
                .is_some_and(|old| {
                    old.status != response.status
                        || old.headers != response.headers
                        || old.body != response.body
                })
        {
            return Err("Recovery changes the bounded delivery disposition.".into());
        }
    }
    Ok(())
}

/// An original private buyer record held under the existing customer lock.
/// No deserializer or caller-supplied labels can construct this authority.
pub struct CommissionSource<'a> {
    owner: &'a Store,
    state: std::fs::File,
    directory: std::fs::File,
    credential: std::fs::File,
    state_digest: String,
    token: jev::ApiKey,
    current_selection: Selection,
    view: View,
    secret: String,
}
impl CommissionSource<'_> {
    pub fn view(&self) -> &View {
        &self.view
    }
    pub fn current_selection(&self) -> &Selection {
        &self.current_selection
    }
    pub fn authorization(&self) -> &str {
        &self.secret
    }
    /// Native authentication consumes this privately; callers must not log it.
    pub fn credential(&self) -> &jev::ApiKey {
        &self.token
    }
    pub fn current(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        crate::task::verify_same_file(&self.owner.dir.join("customer.lock"), &self.owner.lock)
            .map_err(|_| "Customer custody lock changed.")?;
        let held = self
            .directory
            .metadata()
            .map_err(|_| "Customer custody unavailable.")?;
        let visible =
            std::fs::symlink_metadata(&self.owner.dir).map_err(|_| "Customer custody replaced.")?;
        if held.dev() != visible.dev()
            || held.ino() != visible.ino()
            || !visible.is_dir()
            || visible.mode() & 0o077 != 0
        {
            return Err("Customer custody replaced or disclosed.".into());
        }
        crate::task::verify_same_file(&self.owner.dir.join("state.json"), &self.state)
            .map_err(|_| "Original customer record changed.")?;
        crate::task::verify_same_file(
            &self
                .owner
                .dir
                .join("credentials")
                .join(&self.current_selection.credential_alias),
            &self.credential,
        )
        .map_err(|_| "Customer credential custody changed.")?;
        let mut bytes = Vec::new();
        use sha2::Digest;
        use std::io::{Read, Seek};
        let mut file = &self.state;
        file.rewind()
            .and_then(|_| {
                file.take(super::MAX_STATE as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|_| "Original customer record unavailable.")?;
        if bytes.len() > super::MAX_STATE
            || format!("{:x}", sha2::Sha256::digest(&bytes)) != self.state_digest
        {
            return Err("Original customer record changed.".into());
        }
        if self
            .owner
            .credential(&self.current_selection.credential_alias)?
            .expose()
            != self.token.expose()
        {
            return Err("Customer credential changed.".into());
        }
        Ok(())
    }
}
impl Store {
    /// Admission is permitted only before the approved original payment.
    /// Reads after payment retain the same offer and recovery authorization.
    pub fn plugin_commission_source(
        &self,
        id: &str,
        admission: bool,
    ) -> Result<CommissionSource<'_>> {
        use std::os::unix::fs::OpenOptionsExt;
        let p = self.plugin(id)?;
        if self.poisoned
            || admission && p.phase != Phase::Approved
            || !admission
                && !matches!(
                    p.phase,
                    Phase::Approved
                        | Phase::Unknown
                        | Phase::Paid
                        | Phase::Completed
                        | Phase::Failed
                )
        {
            return Err("Original approved plugin purchase is required.".into());
        }
        if self
            .book
            .selected
            .as_ref()
            .is_none_or(|s| !same_customer(s, &p.selection) || admission && s != &p.selection)
            || p.approval.is_none()
        {
            return Err("Original selected buyer and approval are required.".into());
        }
        let state = crate::task::private_open(&self.dir.join("state.json"), false, false)
            .map_err(|_| "Customer record unavailable.")?;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&self.dir)
            .map_err(|_| "Customer directory unavailable.")?;
        let current_selection = self.book.selected.as_ref().unwrap().clone();
        let credential = crate::task::private_open(
            &self
                .dir
                .join("credentials")
                .join(&current_selection.credential_alias),
            false,
            false,
        )
        .map_err(|_| "Private buyer credential unavailable.")?;
        let mut bytes = Vec::new();
        let file = &state;
        use sha2::Digest;
        use std::io::Read;
        file.take(super::MAX_STATE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Customer record unavailable.")?;
        let result = CommissionSource {
            owner: self,
            state,
            directory,
            credential,
            state_digest: format!("{:x}", sha2::Sha256::digest(&bytes)),
            token: self.credential(&current_selection.credential_alias)?,
            current_selection,
            view: self.plugin_view(id)?,
            secret: p
                .recovery_secret
                .clone()
                .ok_or("Original private recovery authorization is required.")?,
        };
        result.current()?;
        Ok(result)
    }
}
