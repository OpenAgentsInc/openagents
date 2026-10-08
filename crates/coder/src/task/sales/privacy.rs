//! Contact admission and copy retention in the canonical private pipeline.
//! Suppression is independent of a member key, policy, and surviving lead.
use super::*;
use std::collections::BTreeSet;
use std::fs;

pub const SCHEMA: &str = "openagents.sales-contact-privacy.v1";
pub const COMMAND_SCHEMA: &str = "openagents.sales-contact-command.v1";
pub const DEFAULT_INACTIVITY: u64 = 90 * 86_400;
const MAX_COPIES: usize = 1024;
const MAX_IDENTIFIERS: usize = 8192;
const MAX_COPY: usize = 2 * 1024 * 1024;
const MAX_IDENTIFIER: usize = 2048;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u64,
    pub inactivity_seconds: u64,
    pub owner_reference: String,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            version: 0,
            inactivity_seconds: DEFAULT_INACTIVITY,
            owner_reference: String::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    RequestedContact,
    AcceptedIntroduction,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    GivenBusinessRole,
    PublishedBusinessRole,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub lead: String,
    pub expected_lead_revision: u64,
    /// The existing account ID, never a name inferred by an agent.
    pub customer: String,
    pub jurisdiction: String,
    pub source_kind: SourceKind,
    pub permission_kind: PermissionKind,
    pub source_sha256: String,
    pub permission_reference_sha256: String,
    pub owner_reference: String,
    /// Owner-confirmed addresses of this same business contact. No alias
    /// grants that channel permission; each lead still needs its own consent.
    pub aliases: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Grant {
    customer: String,
    aliases: Vec<String>,
    scope_sha256: String,
    admission_sha256: String,
    recorded_by: String,
    at: u64,
    engaged_at: u64,
    engagement_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub schema: String,
    pub id: String,
    pub expected_revision: u64,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Policy {
        policy: Policy,
    },
    Admit {
        admission: Admission,
    },
    /// An attributed customer reply or accepted introduction, not an edit,
    /// generated preference, or operator task. Its reference remains a digest.
    Engagement {
        lead: String,
        at: u64,
        evidence_reference: String,
        event_kind: EngagementKind,
    },
    OptOut {
        contact: String,
        customer: Option<String>,
        reference: String,
        ambiguous: bool,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementKind {
    CustomerReply,
    AcceptedIntroduction,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CopyState {
    Planned,
    Present,
    Removed,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Copy {
    pub reference: String,
    pub leads: Vec<String>,
    pub recipient: String,
    pub sha256: String,
    pub state: CopyState,
    pub retain_until: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    parent_identity: Identity,
    file_identity: Option<Identity>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obligation {
    pub kind: String,
    pub original_sha256: String,
    pub origin_sha256: String,
    pub currency: Option<String>,
    pub currency_scale: Option<u64>,
    pub amount_minor: Option<u64>,
    pub paid_minor: Option<u64>,
    pub refunded_minor: Option<u64>,
    pub unknown: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Tombstone {
    at: u64,
    reference_sha256: String,
    obligations: Vec<Obligation>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Fingerprint {
    length: usize,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PolicyRevision {
    policy: Policy,
    recorded_by: String,
    at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    enabled: bool,
    pub(super) revision: u64,
    policy: Policy,
    #[serde(default)]
    policy_history: BTreeMap<u64, PolicyRevision>,
    grants: BTreeMap<String, Grant>,
    suppressed_customers: BTreeMap<String, Suppression>,
    // Normalized alias hashes survive removal of their original source.
    aliases: BTreeMap<String, BTreeSet<String>>,
    identifiers: BTreeMap<String, Fingerprint>,
    copies: BTreeMap<String, Copy>,
    pub(super) deleted: BTreeMap<String, Tombstone>,
    #[serde(default)]
    retired_obligations: BTreeMap<String, Obligation>,
    #[serde(default)]
    credential_digests: BTreeSet<String>,
    #[serde(default)]
    credential_fingerprints: BTreeMap<String, Fingerprint>,
    commands: BTreeMap<String, (String, u64)>,
    agent_names: BTreeSet<String>,
    #[serde(default)]
    pub(super) agent_cleanup: BTreeMap<String, bool>,
    #[serde(default)]
    pub(super) native_cleanup_truncated: bool,
}
impl Default for Book {
    fn default() -> Self {
        Self {
            enabled: true,
            revision: 0,
            policy: Policy::default(),
            policy_history: BTreeMap::new(),
            grants: BTreeMap::new(),
            suppressed_customers: BTreeMap::new(),
            aliases: BTreeMap::new(),
            identifiers: BTreeMap::new(),
            copies: BTreeMap::new(),
            deleted: BTreeMap::new(),
            retired_obligations: BTreeMap::new(),
            credential_digests: BTreeSet::new(),
            credential_fingerprints: BTreeMap::new(),
            commands: BTreeMap::new(),
            agent_names: BTreeSet::new(),
            agent_cleanup: BTreeMap::new(),
            native_cleanup_truncated: false,
        }
    }
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.grants.len() > MAX_LEADS
            || self.aliases.len() > MAX_RECEIPTS
            || self.suppressed_customers.len() > MAX_RECEIPTS
            || self.identifiers.len() > MAX_IDENTIFIERS
            || self.copies.len() > MAX_COPIES
            || self.deleted.len() > MAX_RECEIPTS
            || self.retired_obligations.len() > MAX_RECEIPTS
            || self.credential_digests.len() > MAX_RECEIPTS
            || self.credential_fingerprints.len() > MAX_RECEIPTS
            || self.commands.len() > MAX_RECEIPTS
            || self.policy_history.len() > MAX_RECEIPTS
            || self.agent_names.len() > 64
            || !(1..=RETENTION_MAX).contains(&self.policy.inactivity_seconds)
        {
            return Err("sales privacy state exceeds its bound".into());
        }
        for value in self.identifiers.values() {
            token(&value.sha256)?;
            if !(4..=MAX_IDENTIFIER).contains(&value.length) {
                return Err("invalid customer identifier fingerprint".into());
            }
        }
        for value in self.credential_fingerprints.values() {
            token(&value.sha256)?;
            if !(1..=MAX_IDENTIFIER).contains(&value.length) {
                return Err("invalid mailbox credential fingerprint".into());
            }
        }
        Ok(())
    }
}
fn salted(state: &State, kind: &str, value: &str) -> String {
    digest(format!("{}:{kind}:{value}", state.salt).as_bytes())
}
fn customer(state: &State, value: &str) -> Result<String> {
    if value.trim().is_empty() || value.len() > 2048 {
        return Err("original customer identity is unavailable".into());
    }
    Ok(salted(state, "customer", value))
}
/// Only these explicit addresses can enter the selected contact gate. Unicode,
/// display names, URI parameters, whitespace, and ambiguous address forms refuse.
pub fn normalize(value: &str) -> Result<String> {
    if !value.is_ascii() || value.len() > 256 || value.trim() != value {
        return Err("sales contact address is malformed or ambiguous".into());
    }
    let normalized = contact(value)?;
    let (channel, address) = normalized
        .split_once(':')
        .ok_or("contact channel is missing")?;
    match channel {
        "email" => {
            let (local, domain) = address
                .split_once('@')
                .ok_or("business email address is missing")?;
            if local.is_empty()
                || local.len() > 64
                || local.starts_with('.')
                || local.ends_with('.')
                || local.contains("..")
                || domain.len() > 190
                || !domain.contains('.')
                || domain.starts_with('.')
                || domain.ends_with('.')
                || domain.contains("..")
                || domain.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                })
                || !local
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._%+-".contains(&b))
                || !domain
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
            {
                return Err("business email address is malformed or ambiguous".into());
            }
        }
        "nostr" if address.len() == 64 && address.bytes().all(|b| b.is_ascii_hexdigit()) => {}
        _ => return Err("sales contact channel has no qualified address adapter".into()),
    }
    Ok(normalized)
}
fn alias(state: &State, address: &str) -> Result<String> {
    Ok(salted(state, "contact", &contact(address)?))
}
pub(super) fn check_identity(state: &State, address: &str, account: &str) -> Result<()> {
    if state.privacy.enabled {
        normalize(address)?;
    }
    check_suppression(state, address, account)
}
fn check_suppression(state: &State, address: &str, account: &str) -> Result<()> {
    if !state.privacy.enabled {
        return Ok(());
    }
    let a = alias(state, address)?;
    let c = customer(state, account)?;
    if state.privacy.suppressed_customers.contains_key(&c)
        || state.privacy.aliases.get(&a).is_some_and(|cs| {
            cs.iter()
                .any(|c| state.privacy.suppressed_customers.contains_key(c))
        })
        || state
            .suppressions
            .contains_key(&Store::suppression(state, address)?)
    {
        return Err("sales contact is permanently suppressed".into());
    }
    Ok(())
}
pub(super) fn address_suppressed(state: &State, address: &str) -> Result<bool> {
    let a = alias(state, address)?;
    Ok(state.privacy.aliases.get(&a).is_some_and(|cs| {
        cs.iter()
            .any(|c| state.privacy.suppressed_customers.contains_key(c))
    }))
}
pub(super) fn suppressed(state: &State, lead: &Lead) -> Result<bool> {
    Ok(check_suppression(state, &lead.contact, &lead.details.account).is_err())
}
pub(super) fn inactive(state: &State, lead: &Lead, now: u64) -> bool {
    if !state.privacy.enabled {
        return false;
    }
    let last = state
        .privacy
        .grants
        .get(&lead.id)
        .map_or(lead.created_at, |g| g.engaged_at);
    last.checked_add(state.privacy.policy.inactivity_seconds)
        .is_none_or(|until| until <= now)
}
fn scope(lead: &Lead) -> Result<String> {
    super::agents::scope(lead)
}
pub(super) fn remember_identifier(state: &mut State, value: &str) -> Result<()> {
    let value = value.to_ascii_lowercase();
    if (4..=MAX_IDENTIFIER).contains(&value.len()) {
        let sha256 = salted(state, "identifier", &value);
        state
            .privacy
            .identifiers
            .entry(sha256.clone())
            .or_insert(Fingerprint {
                length: value.len(),
                sha256,
            });
    }
    state.privacy.check()
}
pub(super) fn remember(state: &mut State, lead: &Lead) -> Result<()> {
    let mut values = vec![
        &lead.details.account,
        &lead.contact,
        &lead.source,
        &lead.details.workflow,
        &lead.details.baseline_reference,
        &lead.details.permission.reference,
    ];
    if let Some(next) = &lead.details.next {
        values.push(&next.description);
    }
    if let Some(decision) = &lead.details.customer_decision {
        values.push(&decision.decision);
        values.push(&decision.reference);
    }
    for sale in lead.service_sales.values() {
        values.push(&sale.facts.customer_decision_maker);
    }
    for value in values {
        let value = value.to_ascii_lowercase();
        if (4..=MAX_IDENTIFIER).contains(&value.len()) {
            let sha256 = salted(state, "identifier", &value);
            state
                .privacy
                .identifiers
                .entry(sha256.clone())
                .or_insert(Fingerprint {
                    length: value.len(),
                    sha256,
                });
        }
    }
    // The bare address is sensitive too; channel prefixes do not protect it.
    if let Some((_, value)) = lead.contact.split_once(':') {
        let value = value.to_ascii_lowercase();
        let sha256 = salted(state, "identifier", &value);
        state
            .privacy
            .identifiers
            .entry(sha256.clone())
            .or_insert(Fingerprint {
                length: value.len(),
                sha256,
            });
    }
    state.privacy.check()
}
// Older pipeline records predate fingerprints. Rebuild their bounded view from
// the canonical retained fields before any generic-agent copy is screened.
pub(super) fn remember_retained(state: &mut State) -> Result<()> {
    if state.leads.len() > MAX_LEADS {
        return Err("sales privacy source exceeds its lead bound".into());
    }
    let leads: Vec<_> = state.leads.values().cloned().collect();
    for lead in &leads {
        remember(state, lead)?;
    }
    Ok(())
}
pub(super) fn service_obligations(sale: &receipts::service_sale::Sale) -> Result<Vec<Obligation>> {
    let mut obligations = vec![];
    let summary = sale.summary()?;
    obligations.push(Obligation {
        kind: "service_invoice".into(),
        original_sha256: digest(
            &serde_json::to_vec(&sale.admission).map_err(|_| "obligation serialization failed")?,
        ),
        origin_sha256: sale.admission_command_digest.clone(),
        currency: Some(sale.admission.invoice.currency.clone()),
        currency_scale: Some(sale.admission.invoice.currency_scale),
        amount_minor: Some(sale.admission.invoice.amount_minor),
        paid_minor: Some(summary.paid_minor),
        refunded_minor: Some(summary.refunded_minor),
        unknown: summary.unresolved,
    });
    if let Some(f) = sale.effective_fulfillment()? {
        obligations.push(Obligation {
            kind: "fulfillment".into(),
            original_sha256: digest(
                &serde_json::to_vec(&f).map_err(|_| "obligation serialization failed")?,
            ),
            origin_sha256: sale.admission_command_digest.clone(),
            currency: Some(f.currency),
            currency_scale: Some(f.currency_scale),
            amount_minor: Some(f.amount_minor),
            paid_minor: None,
            refunded_minor: None,
            unknown: true,
        });
    }
    Ok(obligations)
}
pub(super) fn retire_service(state: &mut State, sale: &receipts::service_sale::Sale) -> Result<()> {
    for mut obligation in service_obligations(sale)? {
        obligation.unknown = true; // No current provider/payment source remains in this projection.
        state
            .privacy
            .retired_obligations
            .entry(obligation.original_sha256.clone())
            .or_insert(obligation);
    }
    state.privacy.check()
}
pub(super) fn retain_credentials(state: &mut State) -> Result<()> {
    state
        .privacy
        .credential_digests
        .extend(state.principals.values().map(|p| p.token_digest.clone()));
    state.privacy.credential_digests.extend(
        state
            .leads
            .values()
            .flat_map(|l| l.agent_records.assignments.values())
            .map(|a| a.credential_sha256.clone()),
    );
    state.privacy.check()
}
pub(super) fn remember_credential(state: &mut State, sha256: &str) -> Result<()> {
    token(sha256)?;
    state.privacy.credential_digests.insert(sha256.into());
    state.privacy.check()
}
pub(super) fn remember_mailbox_credential(state: &mut State, secret: &str) -> Result<()> {
    if secret.is_empty() || secret.len() > MAX_IDENTIFIER {
        return Err("mailbox credential fingerprint exceeds its bound".into());
    }
    let key = salted(state, "identifier", &secret.to_ascii_lowercase());
    state.privacy.credential_fingerprints.insert(
        key.clone(),
        Fingerprint {
            length: secret.len(),
            sha256: key,
        },
    );
    state.privacy.check()
}
pub(super) fn check_credentials(state: &State, text: &str) -> Result<()> {
    secret_screen::Screen::shapes()
        .check(text)
        .map_err(|_| "sales text refuses credential material")?;
    for candidate in text
        .as_bytes()
        .split(|b| !b.is_ascii_hexdigit())
        .filter(|part| part.len() == 64)
    {
        let sha = digest(candidate);
        if state.privacy.credential_digests.contains(&sha)
            || state.principals.values().any(|p| p.token_digest == sha)
        {
            return Err("sales text refuses credential material".into());
        }
    }
    if !state.privacy.credential_fingerprints.is_empty() {
        let mut restricted = state.clone();
        restricted.privacy.identifiers = state.privacy.credential_fingerprints.clone();
        if contains_customer(&restricted, text)? {
            return Err("sales text refuses credential material".into());
        }
    }
    Ok(())
}
/// Native approved outbound material may name its original lead, never another customer.
pub(super) fn check_outbound_copy(state: &State, lead: &Lead, text: &str) -> Result<()> {
    check_credentials(state, text)?;
    let mut own = state.clone();
    own.privacy.identifiers.clear();
    remember(&mut own, lead)?;
    let mut restricted = state.clone();
    restricted
        .privacy
        .identifiers
        .retain(|key, _| !own.privacy.identifiers.contains_key(key));
    if contains_customer(&restricted, text)? {
        return Err(
            "outbound material contains a customer outside the original approved lead".into(),
        );
    }
    Ok(())
}
pub(super) fn remove(state: &mut State, lead: &str, now: u64, reference: &str) -> Result<()> {
    let found = state.leads.get(lead).ok_or("lead is unavailable")?.clone();
    // Legacy unadmitted records still retain their original contact suppression.
    if state.privacy.enabled {
        remember(state, &found)?;
        let c = customer(state, &found.details.account)?;
        let a = alias(state, &found.contact)?;
        state
            .privacy
            .aliases
            .entry(a)
            .or_default()
            .insert(c.clone());
        let mut connected = BTreeSet::from([c]);
        loop {
            let before = connected.len();
            for cs in state.privacy.aliases.values() {
                if cs.iter().any(|c| connected.contains(c)) {
                    connected.extend(cs.iter().cloned());
                }
            }
            if before == connected.len() {
                break;
            }
        }
        for c in connected {
            state
                .privacy
                .suppressed_customers
                .entry(c)
                .or_insert(Suppression {
                    at: now,
                    reference_digest: digest(reference.as_bytes()),
                });
        }
    }
    let mut obligations = state.outbox.obligations(lead);
    state.outbox.redact(lead, now);
    for sale in found.service_sales.values() {
        obligations.extend(service_obligations(sale)?);
    }
    for assignment in found.partner_assignments.values() {
        obligations.push(Obligation {
            kind: "partner_assignment".into(),
            original_sha256: digest(
                &serde_json::to_vec(assignment).map_err(|_| "obligation serialization failed")?,
            ),
            origin_sha256: digest(assignment.proposal.id.as_bytes()),
            currency: None,
            currency_scale: None,
            amount_minor: None,
            paid_minor: None,
            refunded_minor: None,
            unknown: true,
        });
    }
    state
        .privacy
        .deleted
        .entry(lead.into())
        .or_insert(Tombstone {
            at: now,
            reference_sha256: digest(reference.as_bytes()),
            obligations,
        });
    for assignment in found.agent_records.assignments.values() {
        state
            .privacy
            .agent_names
            .insert(assignment.anchor.name.clone());
        state
            .privacy
            .agent_cleanup
            .insert(assignment.anchor.name.clone(), false);
    }
    state.privacy.grants.remove(lead);
    for copy in state
        .privacy
        .copies
        .values_mut()
        .filter(|c| c.leads.iter().any(|l| l == lead))
    {
        copy.retain_until = copy.retain_until.min(now);
    }
    state.privacy.check()
}

impl Store {
    pub fn sales_contact_check(
        &mut self,
        access: &Access,
        lead: &str,
        channel: &str,
    ) -> Result<serde_json::Value> {
        self.refresh()?;
        let record = self.state.leads.get(lead).ok_or("lead is unavailable")?;
        self.readable(access, record)?;
        self.contact_admitted(record, channel)?;
        Ok(
            serde_json::json!({"lead":lead,"revision":record.revision,"channel":channel,"contact_admitted":true,"send_authority":false,"model_disclosure_authority":false}),
        )
    }
    pub fn sales_privacy_view(&mut self, access: &Access) -> Result<serde_json::Value> {
        self.refresh()?;
        self.admin(access)?;
        Ok(
            serde_json::json!({"schema":SCHEMA,"revision":self.state.privacy.revision,"enabled":self.state.privacy.enabled,"policy":self.state.privacy.policy,"admitted_leads":self.state.privacy.grants.keys().collect::<Vec<_>>(),"suppressed_customers":self.state.privacy.suppressed_customers.len(),"copies":self.state.privacy.copies.values().map(|c|serde_json::json!({"reference":c.reference,"leads":c.leads,"recipient":c.recipient,"sha256":c.sha256,"state":c.state,"retain_until":c.retain_until})).collect::<Vec<_>>(),"deleted":self.state.privacy.deleted,"retired_obligations":self.state.privacy.retired_obligations,"agent_cleanup":self.state.privacy.agent_cleanup,"native_cleanup_truncated":self.state.privacy.native_cleanup_truncated,"policy_history":self.state.privacy.policy_history,"model_disclosure_available":false,"sender_available":false,"relay_disclosure_available":false,"historical_remote_erasure_verified":false}),
        )
    }
    pub fn apply_sales_privacy(&mut self, access: &Access, bytes: &[u8]) -> Result<u64> {
        self.refresh()?;
        // Opt-outs are safe reductions by a current human writer, even after
        // permission expired; all grants and engagement require the owner.
        let command: Command = super::agents::parse(bytes)?;
        if command.schema != COMMAND_SCHEMA {
            return Err("unsupported sales privacy command".into());
        }
        id(&command.id)?;
        let role = self.check(access)?;
        if matches!(command.operation, Operation::OptOut { .. }) {
            if role == Role::Reader {
                return Err("sales opt-out requires a current human writer".into());
            }
        } else {
            self.admin(access)?;
        }
        let key = digest(format!("{}:{}", access.principal(), command.id).as_bytes());
        let input = digest(bytes);
        if let Some((previous, revision)) = self.state.privacy.commands.get(&key) {
            return if previous == &input {
                Ok(*revision)
            } else {
                Err("sales privacy retry changed bytes".into())
            };
        }
        check_credentials(
            &self.state,
            std::str::from_utf8(bytes).map_err(|_| "privacy command is not UTF-8")?,
        )?;
        let reduction = matches!(command.operation, Operation::OptOut { .. });
        if command.expected_revision > self.state.privacy.revision
            || (!reduction && self.state.privacy.revision != command.expected_revision)
        {
            return Err("sales privacy revision conflict".into());
        }
        let now = (self.clock)();
        let mut next = self.state.clone();
        next.privacy.enabled = true;
        match command.operation {
            Operation::Policy { policy } => {
                text(&policy.owner_reference, 256)?;
                if policy.version
                    != next
                        .privacy
                        .policy
                        .version
                        .checked_add(1)
                        .ok_or("privacy policy version overflow")?
                    || !(1..=RETENTION_MAX).contains(&policy.inactivity_seconds)
                {
                    return Err("sales inactivity policy is invalid or not the next version".into());
                }
                let root = self.dir.parent().ok_or("host root is unavailable")?;
                check_copy(root, &policy.owner_reference)?;
                next.privacy.policy_history.insert(
                    policy.version,
                    PolicyRevision {
                        policy: policy.clone(),
                        recorded_by: access.principal.clone(),
                        at: now,
                    },
                );
                next.privacy.policy = policy;
            }
            Operation::Admit { admission } => {
                let lead = next
                    .leads
                    .get(&admission.lead)
                    .ok_or("lead is unavailable")?
                    .clone();
                self.readable(access, &lead)?;
                text(&admission.owner_reference, 256)?;
                if lead.revision != admission.expected_lead_revision
                    || lead.details.account != admission.customer
                    || lead.details.jurisdiction != "US"
                    || admission.jurisdiction != "US"
                    || digest(lead.source.as_bytes()) != admission.source_sha256
                    || digest(lead.details.permission.reference.as_bytes())
                        != admission.permission_reference_sha256
                    || lead.details.permission.state != PermissionState::Granted
                    || lead.details.permission.expires_at <= now
                    || lead.details.permission.recorded_at > now
                    || lead
                        .details
                        .permission
                        .recorded_at
                        .checked_add(next.privacy.policy.inactivity_seconds)
                        .is_none_or(|until| until <= now)
                    || admission.aliases.is_empty()
                    || admission.aliases.len() > 8
                    || inactive(&next, &lead, now)
                {
                    return Err("business contact admission does not match current source, permission, or retention".into());
                }
                check_identity(&next, &lead.contact, &lead.details.account)?;
                let own = normalize(&lead.contact)?;
                let normalized = admission
                    .aliases
                    .iter()
                    .map(|a| normalize(a))
                    .collect::<Result<Vec<_>>>()?;
                if !normalized.contains(&own)
                    || normalized.iter().collect::<BTreeSet<_>>().len() != normalized.len()
                {
                    return Err(
                        "business contact aliases are missing or collide after normalization"
                            .into(),
                    );
                }
                let c = customer(&next, &admission.customer)?;
                let mut hashes = vec![];
                for a in &normalized {
                    check_identity(&next, a, &admission.customer)?;
                    let hash = alias(&next, a)?;
                    // An ambiguous alias cannot be used to split one contact into
                    // another customer's authority. Suppression still connects it.
                    if next
                        .privacy
                        .aliases
                        .get(&hash)
                        .is_some_and(|cs| cs.iter().any(|old| old != &c))
                    {
                        return Err(
                            "business contact alias has a conflicting original customer".into()
                        );
                    }
                    next.privacy
                        .aliases
                        .entry(hash.clone())
                        .or_default()
                        .insert(c.clone());
                    hashes.push(hash);
                }
                remember(&mut next, &lead)?;
                for alias in &normalized {
                    remember_identifier(&mut next, alias)?;
                    if let Some((_, identity)) = alias.split_once(':') {
                        remember_identifier(&mut next, identity)?;
                    }
                }
                let at = lead.details.permission.recorded_at;
                let grant = Grant {
                    customer: c,
                    aliases: hashes,
                    scope_sha256: scope(&lead)?,
                    admission_sha256: digest(
                        &serde_json::to_vec(&admission)
                            .map_err(|_| "contact admission serialization failed")?,
                    ),
                    recorded_by: access.principal().into(),
                    at: now,
                    engaged_at: at,
                    engagement_sha256: admission.permission_reference_sha256,
                };
                next.privacy.grants.insert(lead.id, grant);
            }
            Operation::Engagement {
                lead,
                at,
                evidence_reference,
                event_kind: _,
            } => {
                text(&evidence_reference, 256)?;
                let record = next.leads.get(&lead).ok_or("lead is unavailable")?;
                self.readable(access, record)?;
                self.contact_admitted(record, "email")?;
                let grant = next
                    .privacy
                    .grants
                    .get_mut(&lead)
                    .ok_or("contact has no owner admission")?;
                if at > now || at <= grant.engaged_at {
                    return Err("customer engagement must advance its attributed event time".into());
                }
                grant.engaged_at = at;
                grant.engagement_sha256 = digest(evidence_reference.as_bytes());
            }
            Operation::OptOut {
                contact,
                customer: original,
                reference,
                ambiguous: _,
            } => {
                text(&reference, 256)?;
                let address = super::contact(&contact)?;
                let a = alias(&next, &address)?;
                let mut customers = next.privacy.aliases.get(&a).cloned().unwrap_or_default();
                for lead in next.leads.values() {
                    if super::contact(&lead.contact).ok().as_deref() == Some(&address) {
                        customers.insert(customer(&next, &lead.details.account)?);
                    }
                }
                if let Some(original) = original {
                    customers.insert(customer(&next, &original)?);
                }
                // Unknown-contact opt-out still survives a future import.
                next.suppressions
                    .entry(Self::suppression(&next, &address)?)
                    .or_insert(Suppression {
                        at: now,
                        reference_digest: digest(reference.as_bytes()),
                    });
                next.privacy
                    .aliases
                    .entry(a)
                    .or_default()
                    .extend(customers.iter().cloned());
                for c in customers {
                    next.privacy
                        .suppressed_customers
                        .entry(c)
                        .or_insert(Suppression {
                            at: now,
                            reference_digest: digest(reference.as_bytes()),
                        });
                }
                let deleted = next
                    .leads
                    .values()
                    .filter(|lead| suppressed(&next, lead).unwrap_or(true))
                    .map(|l| l.id.clone())
                    .collect::<Vec<_>>();
                for lead in deleted {
                    if next.leads.contains_key(&lead) {
                        Self::remove(&mut next, &lead, now, &reference)?;
                    }
                }
            }
        }
        next.privacy.revision = next
            .privacy
            .revision
            .checked_add(1)
            .ok_or("sales privacy revision overflow")?;
        let revision = next.privacy.revision;
        next.privacy.commands.insert(key, (input, revision));
        next.privacy.check()?;
        self.persist(next)?;
        self.refresh()?;
        Ok(revision)
    }
    pub(super) fn contact_admitted(&self, lead: &Lead, channel: &str) -> Result<()> {
        check_identity(&self.state, &lead.contact, &lead.details.account)?;
        let grant = self
            .state
            .privacy
            .grants
            .get(&lead.id)
            .ok_or("sales contact needs explicit owner business and permission admission")?;
        if grant.scope_sha256 != scope(lead)?
            || inactive(&self.state, lead, (self.clock)())
            || lead.details.jurisdiction != "US"
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= (self.clock)()
            || !lead
                .details
                .permission
                .channels
                .iter()
                .any(|c| c == channel)
            || !normalize(&lead.contact)?.starts_with(&format!("{channel}:"))
        {
            return Err("sales contact admission is stale or outside its channel".into());
        }
        if channel != "email" {
            return Err("proactive contact channel has no qualified adapter".into());
        }
        Ok(())
    }
}

fn identity(file: &File, directory: bool) -> Result<Identity> {
    let meta = file
        .metadata()
        .map_err(|_| "private copy metadata is unavailable")?;
    if meta.is_dir() != directory || (!directory && !meta.is_file()) {
        return Err("private copy type changed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o077 != 0
            || (!directory && meta.nlink() != 1)
        {
            return Err("private copy must be owned, private, and unshared".into());
        }
        Ok(Identity {
            device: meta.dev(),
            inode: meta.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        if !private_fs::is_private(file).map_err(|_| "private copy protection is unavailable")? {
            return Err("private copy must be private".into());
        }
        Ok(Identity::default())
    }
}
pub(super) fn private_bytes(path: &Path, maximum: usize) -> Result<(File, Vec<u8>)> {
    let mut file = super::super::private_open(path, false, false)
        .map_err(|_| "private copy is unavailable")?;
    identity(&file, false)?;
    let mut bytes = vec![];
    std::io::Read::by_ref(&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private copy read failed")?;
    if bytes.len() > maximum {
        return Err("private copy exceeds its read bound".into());
    }
    super::super::verify_same_file(path, &file).map_err(|_| "private copy changed during read")?;
    identity(&file, false)?;
    Ok((file, bytes))
}
impl Store {
    /// Current native callers use this for the single private export producer.
    /// File provenance is sealed before content is written; a crash is retained
    /// as unavailable cleanup, never as proof that a copy was erased.
    pub(super) fn write_sales_copy(
        &mut self,
        access: &Access,
        leads: &[String],
        path: &Path,
        bytes: &[u8],
    ) -> Result<String> {
        self.write_scoped_sales_copy(access, leads, path, bytes, None)
    }
    pub(super) fn write_scoped_sales_copy(
        &mut self,
        access: &Access,
        leads: &[String],
        path: &Path,
        bytes: &[u8],
        partner: Option<&str>,
    ) -> Result<String> {
        self.check(access)?;
        if leads.is_empty() || leads.len() > MAX_LEADS || bytes.len() > MAX_COPY {
            return Err("private export scope exceeds its bound".into());
        }
        let mut until = u64::MAX;
        for id in leads {
            let lead = self
                .state
                .leads
                .get(id)
                .ok_or("private export lead is unavailable")?;
            if let Some(assignment) = partner {
                self.partner_copy_readable(access, lead, assignment)?;
                until = until.min(lead.partner_assignments[assignment].data.retain_until);
            } else {
                self.readable(access, lead)?;
            }
            until = until.min(lead.details.data.retain_until);
            // An extended lead consent cannot extend the immutable recipient
            // and retention boundaries of a service or funnel export.
            for sale in lead.service_sales.values() {
                until = until.min(sale.retain_until);
            }
            for journey in lead.funnel_journeys.values() {
                until = until.min(journey.retain_until);
            }
            if self.state.privacy.enabled {
                if inactive(&self.state, lead, (self.clock)()) {
                    return Err("private export contact retention expired".into());
                }
                let last = self
                    .state
                    .privacy
                    .grants
                    .get(id)
                    .map_or(lead.created_at, |g| g.engaged_at);
                until = until.min(
                    last.checked_add(self.state.privacy.policy.inactivity_seconds)
                        .ok_or("contact retention overflow")?,
                );
            }
        }
        self.external_file(path)?;
        let parent = path.parent().ok_or("private export parent is missing")?;
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        }
        .canonicalize()
        .map_err(|_| "private export parent is unavailable")?;
        let directory = agents::native::directory(&parent)?;
        let parent_identity = identity(&directory, true)?;
        let path = parent.join(
            path.file_name()
                .ok_or("private export file name is missing")?,
        );
        if path.exists() || fs::symlink_metadata(&path).is_ok() {
            return Err("private export already exists".into());
        }
        let reference = random_token();
        let hash = digest(bytes);
        let mut next = self.state.clone();
        next.privacy.copies.insert(
            reference.clone(),
            Copy {
                reference: reference.clone(),
                leads: leads.to_vec(),
                recipient: access.principal().into(),
                sha256: hash.clone(),
                state: CopyState::Planned,
                retain_until: until,
                path: Some(path.clone()),
                parent_identity,
                file_identity: None,
            },
        );
        next.privacy.check()?;
        self.persist(next)?;
        self.sales_custody()?;
        agents::native::same_directory(&parent, &directory)?;
        self.copy_contact_fence(access, leads, until, partner)?;
        let mut file = super::super::private_open(&path, true, true)
            .map_err(|_| "private export create refused")?;
        let file_identity = identity(&file, false)?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "private export write failed")?;
        agents::native::same_directory(&parent, &directory)?;
        super::super::verify_same_file(&path, &file)
            .map_err(|_| "private export custody changed")?;
        super::super::sync_directory(&parent)
            .map_err(|_| "private export directory sync failed")?;
        self.sales_custody()?;
        self.check(access)?;
        let mut next = self.state.clone();
        let copy = next
            .privacy
            .copies
            .get_mut(&reference)
            .ok_or("private copy provenance disappeared")?;
        copy.file_identity = Some(file_identity);
        copy.state = CopyState::Present;
        // Seal exact file provenance even when its admitted time boundary
        // elapsed while the write/sync blocked. Cleanup then checks this file,
        // never a guessed path or a subsequent unrelated replacement.
        self.persist(next)?;
        if let Err(why) = self.copy_contact_fence(access, leads, until, partner) {
            self.cleanup_sales_copies()?;
            return Err(why);
        }
        Ok(hash)
    }
    fn copy_contact_fence(
        &self,
        access: &Access,
        leads: &[String],
        until: u64,
        partner: Option<&str>,
    ) -> Result<()> {
        self.sales_custody()?;
        self.check(access)?;
        let now = (self.clock)();
        if now >= until {
            return Err("native private copy retention expired before completion".into());
        }
        for id in leads {
            let record = self
                .state
                .leads
                .get(id)
                .ok_or("private export lead is unavailable")?;
            if let Some(assignment) = partner {
                self.partner_copy_readable(access, record, assignment)?;
            } else {
                self.readable(access, record)?;
            }
            if record.details.data.retain_until <= now
                || inactive(&self.state, record, now)
                || suppressed(&self.state, record)?
            {
                return Err("private export contact scope changed before completion".into());
            }
        }
        Ok(())
    }
    fn partner_copy_readable(&self, access: &Access, lead: &Lead, id: &str) -> Result<()> {
        self.check(access)?;
        let assignment = lead
            .partner_assignments
            .get(id)
            .ok_or("private partner copy assignment is unavailable")?;
        let actor = access.principal();
        if !Self::recipient(&lead.details, actor)
            || !assignment
                .data
                .recipients
                .contains(&format!("human:{actor}"))
            || (self.clock)() >= assignment.data.retain_until
            || !(actor == assignment.owner_human
                || actor == assignment.proposal.recipient_human
                || assignment
                    .handoff
                    .as_ref()
                    .is_some_and(|h| h.target == actor)
                || assignment
                    .events
                    .iter()
                    .any(|e| e.actor == actor && e.outcome == "handoff_accepted"))
        {
            return Err("private partner copy exceeds its current recipient boundary".into());
        }
        Ok(())
    }
    pub(super) fn cleanup_sales_copies(&mut self) -> Result<()> {
        let now = (self.clock)();
        let pending = self
            .state
            .privacy
            .copies
            .values()
            .filter(|c| c.retain_until <= now && c.state != CopyState::Removed)
            .cloned()
            .collect::<Vec<_>>();
        for copy in pending {
            self.sales_custody()?;
            let removed = (|| -> Result<()> {
                let path = copy
                    .path
                    .as_ref()
                    .ok_or("private copy path is unavailable")?;
                let parent = path.parent().ok_or("private copy parent is unavailable")?;
                let held = agents::native::directory(parent)?;
                if identity(&held, true)? != copy.parent_identity {
                    return Err("private copy parent was replaced".into());
                }
                if fs::symlink_metadata(path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                {
                    return Ok(());
                }
                let (file, bytes) = private_bytes(path, MAX_COPY)?;
                if copy.file_identity.as_ref() != Some(&identity(&file, false)?)
                    || digest(&bytes) != copy.sha256
                {
                    return Err("private copy was replaced or was never sealed".into());
                }
                agents::native::same_directory(parent, &held)?;
                super::super::verify_same_file(path, &file)
                    .map_err(|_| "private copy custody changed")?;
                self.sales_custody()?;
                fs::remove_file(path).map_err(|_| "private copy removal failed")?;
                super::super::sync_directory(parent)
                    .map_err(|_| "private copy removal sync failed")?;
                Ok(())
            })()
            .is_ok();
            let mut next = self.state.clone();
            let row = next
                .privacy
                .copies
                .get_mut(&copy.reference)
                .ok_or("copy provenance disappeared")?;
            if removed {
                row.state = CopyState::Removed;
                row.path = None;
            } else {
                row.state = CopyState::Unavailable;
            }
            self.persist(next)?;
        }
        self.scrub_sales_agent_copies()?;
        Ok(())
    }
    fn scrub_sales_agent_copies(&mut self) -> Result<()> {
        let names = self
            .state
            .privacy
            .agent_cleanup
            .iter()
            .filter(|(_, done)| !**done)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in names {
            self.sales_custody()?;
            let root = self.dir.parent().ok_or("host root is unavailable")?;
            let scrubbed = (|| -> Result<()> {
                let store = super::super::agent::Store::with_keys(root, &name, self.native_keys.clone())?;
                let record = store.load()?.ok_or("cleanup member is unavailable")?;
                store.custody(&record)?;
                if contains_customer(&self.state, &serde_json::to_string(&record).map_err(|_| "agent definition serialization failed")?)? {
                    return Err("customer content in an agent definition needs owner repair; cleanup remains unavailable".into());
                }
                let directory = agents::native::directory(store.dir())?;
                for file_name in ["memory.jsonl", "scores.jsonl", "journal.jsonl"] {
                    let path = store.dir().join(file_name);
                    if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) { continue; }
                    let (file, bytes) = private_bytes(&path, MAX_COPY)?;
                    let mut rows = vec![];
                    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
                        let text = std::str::from_utf8(line).map_err(|_| "agent copy is not UTF-8")?;
                        if (record.job_role.is_some() && text.contains('@')) || contains_customer(&self.state, text)? {
                            if file_name == "journal.jsonl" {
                                let mut entry: super::super::agent::Entry = serde_json::from_slice(line).map_err(|_| "agent journal copy is malformed")?;
                                entry.text = "Private customer content removed; canonical opaque audit remains.".into();
                                entry.from = None;
                                rows.extend(serde_json::to_vec(&entry).map_err(|_| "journal cleanup serialization failed")?);
                                rows.push(b'\n');
                            }
                        } else { rows.extend_from_slice(line); rows.push(b'\n'); }
                    }
                    if rows != bytes {
                        agents::native::same_directory(store.dir(), &directory)?;
                        super::super::verify_same_file(&path, &file).map_err(|_| "agent copy custody changed")?;
                        store.custody(&record)?;
                        self.sales_custody()?;
                        super::super::replace_file(store.dir(), file_name, &rows).map_err(|_| "agent copy cleanup failed")?;
                    }
                }
                let drafts = super::super::agent_share::drafts_dir(&store);
                if fs::symlink_metadata(&drafts).is_ok() {
                    let drafts_directory = agents::native::directory(&drafts)?;
                    let mut seen = 0usize;
                    let mut total = 0usize;
                    for item in fs::read_dir(&drafts).map_err(|_| "agent draft cleanup is unavailable")? {
                        seen += 1;
                        if seen > 128 { return Err("agent draft cleanup exceeds its file bound".into()); }
                        let path = item.map_err(|_| "agent draft cleanup source is unavailable")?.path();
                        let (file, bytes) = private_bytes(&path, MAX_COPY)?;
                        total = total.checked_add(bytes.len()).ok_or("agent draft cleanup byte count overflow")?;
                        if total > MAX_COPY { return Err("agent draft cleanup exceeds its byte bound".into()); }
                        let text = std::str::from_utf8(&bytes).map_err(|_| "agent draft cleanup source is not UTF-8")?;
                        if (record.job_role.is_some() && text.contains('@')) || contains_draft_customer(&self.state, text)? {
                            agents::native::same_directory(&drafts, &drafts_directory)?;
                            super::super::verify_same_file(&path, &file).map_err(|_| "agent draft cleanup custody changed")?;
                            store.custody(&record)?;
                            self.sales_custody()?;
                            fs::remove_file(&path).map_err(|_| "agent draft removal failed")?;
                            super::super::sync_directory(&drafts).map_err(|_| "agent draft removal did not persist")?;
                        }
                    }
                    agents::native::same_directory(&drafts, &drafts_directory)?;
                }
                super::super::agent_engrams::scrub_customer(&store, (self.clock)(), |text| {
                    if record.job_role.is_some() && text.contains('@') { Ok(true) } else { contains_customer(&self.state, text) }
                })?;
                // Signed historic verdicts remain original audit records. If
                // they contain customer content, refuse projection and retain
                // unavailable cleanup rather than rewriting an approval.
                store.crew_verdicts()?;
                agents::native::same_directory(store.dir(), &directory)?;
                store.custody(&record)?;
                self.sales_custody()?;
                Ok(())
            })().is_ok();
            if scrubbed {
                let mut next = self.state.clone();
                next.privacy.agent_cleanup.insert(name, true);
                self.persist(next)?;
            }
        }
        Ok(())
    }
}
/// Exhaustion is unavailable evidence, not a match and not permission to erase.
pub(super) fn contains_customer(state: &State, text: &str) -> Result<bool> {
    const CHECKS: usize = 131_072;
    const NODES: usize = 4096;
    if text.len() > MAX_COPY {
        return Err("customer copy screening exceeds its byte bound".into());
    }
    let lengths = state
        .privacy
        .identifiers
        .values()
        .chain(state.privacy.credential_fingerprints.values())
        .map(|f| f.length)
        .collect::<BTreeSet<_>>();
    let mut checks = CHECKS;
    let mut nodes = NODES;
    fn literal(
        state: &State,
        lengths: &BTreeSet<usize>,
        text: &str,
        checks: &mut usize,
    ) -> Result<bool> {
        let lower = text.to_ascii_lowercase();
        for &length in lengths {
            if length > lower.len() {
                continue;
            }
            for start in 0..=lower.len() - length {
                if !lower.is_char_boundary(start) || !lower.is_char_boundary(start + length) {
                    continue;
                }
                *checks = checks
                    .checked_sub(1)
                    .ok_or("customer copy screening work is unavailable")?;
                let key = salted(state, "identifier", &lower[start..start + length]);
                if state.privacy.identifiers.contains_key(&key)
                    || state.privacy.credential_fingerprints.contains_key(&key)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn value(
        state: &State,
        lengths: &BTreeSet<usize>,
        v: &serde_json::Value,
        checks: &mut usize,
        nodes: &mut usize,
        depth: u8,
    ) -> Result<bool> {
        *nodes = nodes
            .checked_sub(1)
            .ok_or("customer JSON screening exceeds its node bound")?;
        if depth > 16 {
            return Err("customer JSON screening exceeds its depth bound".into());
        }
        match v {
            serde_json::Value::String(s) => {
                if literal(state, lengths, s, checks)? {
                    return Ok(true);
                }
                // Engram memory values can themselves contain a JSON entry.
                if matches!(s.trim_start().chars().next(), Some('{' | '[' | '"')) {
                    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(s) {
                        if value(state, lengths, &inner, checks, nodes, depth + 1)? {
                            return Ok(true);
                        }
                    }
                }
            }
            serde_json::Value::Array(rows) => {
                for row in rows {
                    if value(state, lengths, row, checks, nodes, depth + 1)? {
                        return Ok(true);
                    }
                }
            }
            serde_json::Value::Object(rows) => {
                for (key, row) in rows {
                    if literal(state, lengths, key, checks)?
                        || value(state, lengths, row, checks, nodes, depth + 1)?
                    {
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }
    if literal(state, &lengths, text, &mut checks)? {
        return Ok(true);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return value(state, &lengths, &v, &mut checks, &mut nodes, 0);
    }
    // Historic memory and journal files are bounded JSONL, not one JSON value.
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if value(state, &lengths, &v, &mut checks, &mut nodes, 0)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn raw_state(root: &Path) -> Result<Option<State>> {
    let path = root.join("sales/state.json");
    if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        if fs::symlink_metadata(root.join("sales"))
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            return Ok(None);
        }
        return Err("the configured private sales source is unavailable".into());
    }
    let directory = agents::native::directory(&root.join("sales"))?;
    let (_, bytes) = private_bytes(&path, MAX_STATE)?;
    agents::native::same_directory(&root.join("sales"), &directory)?;
    let mut state: State =
        serde_json::from_slice(&bytes).map_err(|_| "sales privacy source is malformed")?;
    if state.schema != super::SCHEMA {
        return Err("sales privacy source schema is unsupported".into());
    }
    state.privacy.check()?;
    remember_retained(&mut state)?;
    Ok(Some(state))
}
/// Private CLI records have an authorized human recipient. Generic agents
/// cannot turn known customer material into shared memory or model permission.
pub(crate) fn check_agent_copy(store: &super::super::agent::Store, text: &str) -> Result<()> {
    check_agent_directory(store.dir(), text)
}
pub(crate) fn check_agent_directory(directory: &Path, text: &str) -> Result<()> {
    let root = directory
        .parent()
        .and_then(Path::parent)
        .ok_or("agent host root is unavailable")?;
    if raw_state(root)?.is_none() {
        return Ok(());
    }
    let path = directory.join("agent.json");
    if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return check_known_copy(root, text);
    }
    let (_, bytes) = private_bytes(&path, 64 * 1024)?;
    let record: super::super::agent::Record =
        serde_json::from_slice(&bytes).map_err(|_| "native agent privacy record is malformed")?;
    check_record_copy(root, record.job_role.is_some(), text)
}
pub(crate) fn check_record_copy(root: &Path, sales_role: bool, text: &str) -> Result<()> {
    if sales_role {
        check_copy(root, text)
    } else {
        check_known_copy(root, text)
    }
}
fn check_known_copy(root: &Path, text: &str) -> Result<()> {
    if let Some(state) = raw_state(root)? {
        check_credentials(&state, text)?;
        if contains_customer(&state, text)? {
            return Err(
                "identifiable customer material stays in the canonical private sales pipeline"
                    .into(),
            );
        }
    }
    Ok(())
}
pub(crate) fn check_copy(root: &Path, text: &str) -> Result<()> {
    if text.len() > MAX_COPY {
        return Err("sales memory copy exceeds its bound".into());
    }
    secret_screen::Screen::shapes()
        .check(text)
        .map_err(|_| "sales memory refuses secret material")?;
    // Email shapes and exact retained identifier fingerprints are stricter
    // than the credential-only screen used by general-purpose agents.
    check_known_copy(root, text)?;
    if text.contains('@') {
        return Err(
            "identifiable customer material stays in the canonical private sales pipeline".into(),
        );
    }
    Ok(())
}
pub(crate) fn read_agent_text(
    store: &super::super::agent::Store,
    path: &Path,
) -> Result<Option<String>> {
    read_agent_directory_text(store.dir(), path)
}
pub(crate) fn read_agent_directory_text(directory: &Path, path: &Path) -> Result<Option<String>> {
    if fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return Ok(None);
    }
    let root = directory
        .parent()
        .and_then(Path::parent)
        .ok_or("agent host root is unavailable")?;
    let text = if raw_state(root)?.is_some() {
        let (_, bytes) = private_bytes(path, MAX_COPY)?;
        String::from_utf8(bytes).map_err(|_| "private agent text is not UTF-8")?
    } else {
        fs::read_to_string(path).map_err(|_| "agent text read failed")?
    };
    check_agent_directory(directory, &text)?;
    Ok(Some(text))
}
pub(crate) fn check_agent_draft(store: &super::super::agent::Store, text: &str) -> Result<()> {
    check_agent_copy(store, text)?;
    let entry = knowledge::Entry::parse(text).map_err(|_| "agent draft fields are unavailable")?;
    check_agent_copy(
        store,
        &serde_json::to_string(&entry).map_err(|_| "agent draft serialization failed")?,
    )
}
fn contains_draft_customer(state: &State, text: &str) -> Result<bool> {
    if contains_customer(state, text)? {
        return Ok(true);
    }
    let entry =
        knowledge::Entry::parse(text).map_err(|_| "agent draft cleanup fields are unavailable")?;
    contains_customer(
        state,
        &serde_json::to_string(&entry).map_err(|_| "agent draft serialization failed")?,
    )
}
pub(crate) fn read_agent_drafts(store: &super::super::agent::Store) -> Result<Vec<String>> {
    let path = super::super::agent_share::drafts_dir(store);
    if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return Ok(vec![]);
    }
    let held = agents::native::directory(&path)?;
    let mut entries = vec![];
    let mut bytes = 0usize;
    for item in fs::read_dir(&path).map_err(|_| "agent drafts are unavailable")? {
        let item = item.map_err(|_| "agent draft source is unavailable")?;
        if entries.len() >= 128 {
            return Err("agent drafts exceed their file bound".into());
        }
        let (_, body) = private_bytes(&item.path(), MAX_COPY)?;
        bytes = bytes
            .checked_add(body.len())
            .ok_or("agent draft byte count overflow")?;
        if bytes > MAX_COPY {
            return Err("agent drafts exceed their byte bound".into());
        }
        let text = String::from_utf8(body).map_err(|_| "agent draft is not UTF-8")?;
        check_agent_draft(store, &text)?;
        entries.push(text);
    }
    agents::native::same_directory(&path, &held)?;
    Ok(entries)
}
pub(crate) fn append_agent_directory_text(directory: &Path, path: &Path, text: &str) -> Result<()> {
    if path.parent() != Some(directory) || text.len() > MAX_COPY {
        return Err("private agent append is outside its bound".into());
    }
    let root = directory
        .parent()
        .and_then(Path::parent)
        .ok_or("agent host root is unavailable")?;
    if raw_state(root)?.is_none() {
        let mut options = fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        private_fs::nofollow(&mut options);
        let mut file = options
            .open(path)
            .map_err(|_| "agent append source is unavailable")?;
        file.write_all(text.as_bytes())
            .map_err(|_| "agent append failed")?;
        return file
            .sync_all()
            .map_err(|_| "agent append did not persist".into());
    }
    let held = agents::native::directory(directory)?;
    check_agent_directory(directory, text)?;
    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    private_fs::nofollow(&mut options);
    let mut file = options
        .open(path)
        .map_err(|_| "private agent append source is unavailable")?;
    identity(&file, false)?;
    super::super::verify_same_file(path, &file)
        .map_err(|_| "private agent append custody changed")?;
    agents::native::same_directory(directory, &held)?;
    if file
        .metadata()
        .map_err(|_| "private agent append metadata failed")?
        .len()
        .checked_add(text.len() as u64)
        .is_none_or(|len| len > MAX_COPY as u64)
    {
        return Err("private agent append exceeds its file bound".into());
    }
    check_agent_directory(directory, text)?;
    file.write_all(text.as_bytes())
        .map_err(|_| "private agent append failed")?;
    file.sync_all()
        .map_err(|_| "private agent append did not persist")?;
    super::super::verify_same_file(path, &file)
        .map_err(|_| "private agent append custody changed")?;
    identity(&file, false)?;
    agents::native::same_directory(directory, &held)?;
    super::super::sync_directory(directory)
        .map_err(|_| "private agent append directory did not persist")?;
    Ok(())
}
pub(crate) fn model_available(store: &super::super::agent::Store) -> Result<()> {
    if store.load()?.is_some_and(|r| r.job_role.is_some()) {
        return Err("Sales model execution requires a current bounded canonical floor admission; the general model path is unavailable.".into());
    }
    Ok(())
}
pub(crate) fn sync_available(store: &super::super::agent::Store) -> Result<()> {
    if store.load()?.is_some_and(|r| r.job_role.is_some()) {
        return Err("sales crew relay disclosure is unavailable; sync stays off".into());
    }
    Ok(())
}
/// Screens an explicitly selected local memory before a private CLI projection.
/// This check grants no model, relay, or contact authority.
///
/// # Errors
/// When the canonical privacy source or current local subject is unavailable,
/// or the projection contains known customer or secret material.
pub fn check_memory_projection(store: &super::super::agent::Store, text: &str) -> Result<()> {
    check_agent_copy(store, text)
}
/// Checks a relay read before constructing a connector. With an active private
/// pipeline, only an exact current local non-sales subject may use the general
/// memory reader; sales and unknown subjects have no recipient adapter.
///
/// # Errors
/// When the selected subject is unqualified or its local privacy source changed.
pub fn check_relay_read(store: &super::super::agent::Store, selected_key: &str) -> Result<()> {
    sync_available(store)?;
    let root = store
        .dir()
        .parent()
        .and_then(Path::parent)
        .ok_or("agent host root is unavailable")?;
    if raw_state(root)?.is_some() {
        let record = store
            .load()?
            .ok_or("sales profile relay subject is unavailable")?;
        if record.pubkey.as_deref() != Some(selected_key) || record.job_role.is_some() {
            return Err("sales profile relay recipient is unqualified; read is unavailable".into());
        }
        check_agent_copy(
            store,
            &serde_json::to_string(&record).map_err(|_| "native subject serialization failed")?,
        )?;
    }
    Ok(())
}
pub fn read_command(path: &Path) -> Result<Vec<u8>> {
    private_bytes(path, MAX_COMMAND).map(|(_, bytes)| bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{
        agent,
        agent_engrams::{EngramStore, Opened},
        agent_key::FileKeys,
        agent_memory::{Author, Memory, MemoryKind},
    };
    fn now() -> u64 {
        1_000_000
    }
    fn day20() -> u64 {
        now() + 20 * 86_400
    }
    fn day90() -> u64 {
        now() + DEFAULT_INACTIVITY
    }
    fn after_permission() -> u64 {
        now() + 201 * 86_400
    }
    struct Fixture {
        dir: tempfile::TempDir,
        store: Store,
        owner: Access,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
            }
            let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
            let credential = dir.path().join("owner");
            store.initialize("operator", &credential).unwrap();
            let owner = store
                .authenticate(&Store::read_credential(&credential).unwrap())
                .unwrap();
            Self { dir, store, owner }
        }
        fn create(&mut self, name: &str, address: &str, account: &str) -> String {
            let command = super::super::Command {
                schema: super::super::COMMAND_SCHEMA.into(),
                id: name.into(),
                lead: None,
                expected_revision: 0,
                operation: super::super::Operation::Create {
                    ownership_acceptance: "operator accepted private responsibility".into(),
                    input: Input {
                        contact: address.into(),
                        source: "Seeded original business introduction".into(),
                        source_at: now(),
                        details: Details {
                            account: account.into(),
                            jurisdiction: "US".into(),
                            permission: Permission {
                                state: PermissionState::Granted,
                                reference: "Owner verified customer business request".into(),
                                recorded_at: now(),
                                expires_at: now() + 200 * 86_400,
                                channels: vec![address.split_once(':').unwrap().0.into()],
                            },
                            workflow: "Seeded private workflow text".into(),
                            baseline_reference: "Seeded protected baseline".into(),
                            data: DataBoundary {
                                recipients: vec!["human:operator".into()],
                                permitted_use: "one private workflow".into(),
                                retain_until: now() + 300 * 86_400,
                            },
                            stage: Stage::Qualified,
                            next: Some(NextAction {
                                description: "review permissioned private scope".into(),
                                due_at: now() + 60,
                            }),
                            customer_decision: None,
                            readers: vec![],
                        },
                    },
                },
            };
            self.store
                .apply(&self.owner, &serde_json::to_vec(&command).unwrap())
                .unwrap()
                .lead
        }
        fn command(&mut self, id: &str, operation: Operation) -> Result<u64> {
            let command = Command {
                schema: COMMAND_SCHEMA.into(),
                id: id.into(),
                expected_revision: self.store.state.privacy.revision,
                operation,
            };
            self.store
                .apply_sales_privacy(&self.owner, &serde_json::to_vec(&command).unwrap())
        }
        fn admission(&self, lead: &str, aliases: Vec<String>) -> Admission {
            let lead = &self.store.state.leads[lead];
            Admission {
                lead: lead.id.clone(),
                expected_lead_revision: lead.revision,
                customer: lead.details.account.clone(),
                jurisdiction: "US".into(),
                source_kind: SourceKind::GivenBusinessRole,
                permission_kind: PermissionKind::RequestedContact,
                source_sha256: digest(lead.source.as_bytes()),
                permission_reference_sha256: digest(lead.details.permission.reference.as_bytes()),
                owner_reference: "operator checked actual requested business contact".into(),
                aliases,
            }
        }
        fn admit(&mut self, lead: &str) {
            let a = self.admission(lead, vec![self.store.state.leads[lead].contact.clone()]);
            self.command(
                &format!("admit-{}", &digest(lead.as_bytes())[..24]),
                Operation::Admit { admission: a },
            )
            .unwrap();
        }
        fn member(&self) -> agent::Store {
            let root = self.dir.path().join("host");
            let member =
                agent::Store::with_keys(&root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
            let record = member
                .open_as(self.dir.path(), now(), agent::preset("paul"))
                .unwrap();
            let record = member.ensure_key(record, now()).unwrap();
            member
                .attest(
                    record,
                    &secp256k1::SecretKey::from_byte_array([31; 32]).unwrap(),
                    now() + 300 * 86_400,
                    now(),
                )
                .unwrap();
            member
        }
    }
    #[test]
    fn actual_contact_gate_requires_fresh_business_evidence_and_rejects_ambiguous_aliases() {
        let mut f = Fixture::new();
        let lead = f.create("buyer", "email:Role@BUSINESS.invalid", "original-customer");
        assert!(
            f.store
                .sales_contact_check(&f.owner, &lead, "email")
                .is_err()
        );
        let mut admission = f.admission(
            &lead,
            vec![
                "email:role@business.invalid".into(),
                "EMAIL:ROLE@BUSINESS.INVALID".into(),
            ],
        );
        assert!(
            f.command(
                "collision",
                Operation::Admit {
                    admission: admission.clone()
                }
            )
            .is_err()
        );
        admission.aliases.pop();
        admission.jurisdiction = "unknown".into();
        assert!(
            f.command(
                "unknown",
                Operation::Admit {
                    admission: admission.clone()
                }
            )
            .is_err()
        );
        admission.jurisdiction = "US".into();
        admission.source_sha256 = "0".repeat(64);
        assert!(
            f.command(
                "changed-source",
                Operation::Admit {
                    admission: admission.clone()
                }
            )
            .is_err()
        );
        f.admit(&lead);
        assert_eq!(
            f.store
                .sales_contact_check(&f.owner, &lead, "email")
                .unwrap()["contact_admitted"],
            true
        );
        assert_eq!(
            f.store
                .sales_contact_check(&f.owner, &lead, "email")
                .unwrap()["send_authority"],
            false
        );
        assert!(
            f.store
                .sales_contact_check(&f.owner, &lead, "nostr")
                .is_err()
        );
        for address in [
            "email:Name <role@business.invalid>",
            "email:role@business.invalid?x=1",
            "email:role@@business.invalid",
            "email:rôle@business.invalid",
            "community:person",
        ] {
            assert!(normalize(address).is_err(), "{address}");
        }
        let mut raw =
            serde_json::to_value(f.admission(&lead, vec!["email:role@business.invalid".into()]))
                .unwrap();
        raw["source_kind"] = "personal_private_source".into();
        assert!(serde_json::from_value::<Admission>(raw).is_err());
    }
    #[test]
    fn ambiguous_opt_out_crosses_channels_hires_restart_and_changed_source() {
        let mut f = Fixture::new();
        let email = f.create("email", "email:role@business.invalid", "canonical-customer");
        let other = f.create(
            "nostr",
            &format!("nostr:{}", "a".repeat(64)),
            "canonical-customer",
        );
        let aliases = vec![
            "email:role@business.invalid".into(),
            format!("nostr:{}", "a".repeat(64)),
        ];
        f.command(
            "admit",
            Operation::Admit {
                admission: f.admission(&email, aliases.clone()),
            },
        )
        .unwrap();
        f.store
            .export(
                &f.owner,
                &email,
                &f.dir.path().join("authorized-export.json"),
            )
            .unwrap();
        f.command(
            "uncertain-stop",
            Operation::OptOut {
                contact: aliases[1].clone(),
                customer: None,
                reference: "ambiguous stop reply".into(),
                ambiguous: true,
            },
        )
        .unwrap();
        assert!(f.store.show(&f.owner, &email).is_err());
        assert!(f.store.show(&f.owner, &other).is_err());
        assert!(!f.dir.path().join("authorized-export.json").exists());
        assert!(
            f.store
                .is_suppressed(&f.owner, "EMAIL:ROLE@BUSINESS.INVALID")
                .unwrap()
        );
        let root = f.dir.path().join("host");
        drop(f.store);
        let mut store = Store::open_with_clock(&root, now).unwrap();
        let access = store
            .authenticate(&Store::read_credential(&f.dir.path().join("owner")).unwrap())
            .unwrap();
        assert!(store.is_suppressed(&access, &aliases[1]).unwrap());
        assert!(
            check_identity(
                &store.state,
                "email:new-role@business.invalid",
                "canonical-customer"
            )
            .is_err()
        );
        assert!(
            check_identity(
                &store.state,
                "email:role@business.invalid",
                "new-display-name"
            )
            .is_err()
        );
        let bytes = fs::read_to_string(root.join("sales/state.json")).unwrap();
        for private in [
            "role@business.invalid",
            "Seeded original business introduction",
            "Seeded private workflow text",
            "canonical-customer",
        ] {
            assert!(!bytes.contains(private), "retained raw {private}");
        }
        assert!(store.state.privacy.deleted.contains_key(&email));
    }
    #[test]
    fn expired_permission_does_not_prevent_immediate_negative_contact_admission() {
        let mut f = Fixture::new();
        let lead = f.create(
            "expired",
            "email:expired@business.invalid",
            "expired-customer",
        );
        f.admit(&lead);
        f.store.clock = after_permission;
        // Refresh expires the lead, but the original normalized identity can
        // still be reduced. No surviving grant or unexpired permission needed.
        f.command(
            "late-stop",
            Operation::OptOut {
                contact: "email:expired@business.invalid".into(),
                customer: None,
                reference: "actual late opt-out".into(),
                ambiguous: false,
            },
        )
        .unwrap();
        assert!(
            f.store
                .is_suppressed(&f.owner, "email:expired@business.invalid")
                .unwrap()
        );
    }
    #[test]
    fn default_inactivity_uses_customer_engagement_not_operator_edits() {
        let mut f = Fixture::new();
        let stale = f.create("stale", "email:stale@business.invalid", "stale-customer");
        let engaged = f.create(
            "engaged",
            "email:engaged@business.invalid",
            "engaged-customer",
        );
        f.admit(&stale);
        f.admit(&engaged);
        f.store.clock = day20;
        let mut details = f.store.show(&f.owner, &stale).unwrap().details;
        details.next.as_mut().unwrap().description = "operator changed a task".into();
        let command = super::super::Command {
            schema: super::super::COMMAND_SCHEMA.into(),
            id: "operator-edit".into(),
            lead: Some(stale.clone()),
            expected_revision: 1,
            operation: super::super::Operation::Update { details },
        };
        f.store
            .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
            .unwrap();
        f.command(
            "actual-reply",
            Operation::Engagement {
                lead: engaged.clone(),
                at: day20(),
                evidence_reference: "owner verified original customer reply".into(),
                event_kind: EngagementKind::CustomerReply,
            },
        )
        .unwrap();
        f.store.clock = day90;
        assert!(f.store.show(&f.owner, &stale).is_err());
        assert!(f.store.show(&f.owner, &engaged).is_ok());
        assert!(
            f.command(
                "operator-cannot-restore",
                Operation::Policy {
                    policy: Policy {
                        version: 1,
                        inactivity_seconds: RETENTION_MAX,
                        owner_reference: "reviewed extended period".into()
                    }
                }
            )
            .is_ok()
        );
        assert!(
            f.store
                .is_suppressed(&f.owner, "email:stale@business.invalid")
                .unwrap()
        );
    }
    #[test]
    fn changed_native_export_remains_unknown_and_does_not_delete_replacement() {
        let mut f = Fixture::new();
        let lead = f.create("copy", "email:copy@business.invalid", "copy-customer");
        let path = f.dir.path().join("native-export.json");
        f.store.export(&f.owner, &lead, &path).unwrap();
        fs::remove_file(&path).unwrap();
        agent::write_private(&path, b"unrelated replacement").unwrap();
        f.command(
            "stop",
            Operation::OptOut {
                contact: "email:copy@business.invalid".into(),
                customer: None,
                reference: "actual stop".into(),
                ambiguous: true,
            },
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"unrelated replacement");
        assert_eq!(
            f.store.sales_privacy_view(&f.owner).unwrap()["copies"][0]["state"],
            "unavailable"
        );
        assert!(
            f.store
                .is_suppressed(&f.owner, "email:copy@business.invalid")
                .unwrap()
        );
    }
    thread_local! { static COPY_CLOCK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
    fn expires_before_create() -> u64 {
        COPY_CLOCK.with(|count| {
            let n = count.get() + 1;
            count.set(n);
            if n >= 2 { day90() } else { now() }
        })
    }
    fn expires_after_write() -> u64 {
        COPY_CLOCK.with(|count| {
            let n = count.get() + 1;
            count.set(n);
            if n >= 3 { day90() } else { now() }
        })
    }
    #[test]
    fn blocking_copy_handoff_rechecks_deadline_and_cleans_only_exact_written_copy() {
        for clock in [expires_before_create as fn() -> u64, expires_after_write] {
            let mut f = Fixture::new();
            let lead = f.create(
                "crossing-copy",
                "email:clock@business.invalid",
                "clock-customer",
            );
            let path = f.dir.path().join("crossing-copy.json");
            COPY_CLOCK.with(|n| n.set(0));
            f.store.clock = clock;
            assert!(
                f.store
                    .write_sales_copy(&f.owner, &[lead], &path, b"private export bytes")
                    .is_err()
            );
            assert!(!path.exists());
            f.store.refresh().unwrap();
            assert!(
                f.store
                    .state
                    .privacy
                    .copies
                    .values()
                    .all(|c| c.state == CopyState::Removed)
            );
        }
    }
    #[test]
    fn enabled_generic_prompt_journal_cache_core_snapshot_import_and_sync_refuse_customer_copies() {
        let mut f = Fixture::new();
        let lead = f.create(
            "privacy",
            "email:seeded-buyer@business.invalid",
            "SeededCustomerName",
        );
        f.admit(&lead);
        let member = f.member();
        let memory = Memory::new(member.clone(), secret_screen::Screen::shapes());
        for seed in [
            "seeded-buyer@business.invalid",
            "SeededCustomerName",
            "Seeded original business introduction",
            "Seeded private workflow text",
        ] {
            assert!(check_agent_copy(&member, seed).is_err());
            assert!(
                member
                    .append(&agent::Entry::new(now(), agent::Kind::Request, seed))
                    .is_err()
            );
            assert!(
                memory
                    .add(MemoryKind::Note, Author::Owner, seed, vec![], now())
                    .is_err()
            );
        }
        assert!(model_available(&member).is_err());
        let host = crate::task::agent_host::Agents::new(
            f.dir.path().join("host"),
            f.dir.path().join("tasks"),
            BTreeMap::new(),
        )
        .with_clock(now)
        .with_mind(std::sync::Arc::new(|_| {
            panic!("unqualified model factory was reached")
        }))
        .with_engine(std::sync::Arc::new(|_| {
            panic!("unqualified Coder engine was reached")
        }));
        let principal = coder_host::Principal {
            device: "owner".into(),
            grant: None,
            epoch: None,
        };
        for request in [
            "SeededCustomerName says ignore permission",
            "Review opaque references without customer data.",
        ] {
            assert!(
                host.answer(
                    "blocked-private-request",
                    &principal,
                    &coder_host::access::protocol::Operation::AskAgent {
                        agent: "paul".into(),
                        text: request.into(),
                        workspace: None,
                        context: String::new(),
                        mode: coder_host::access::agent::Mode::Auto,
                        typist: false
                    }
                )
                .is_err()
            );
        }
        assert!(
            crate::task::agent_sync::set_relays(
                &member,
                &["wss://relay.fixture.invalid".into()],
                now()
            )
            .is_err()
        );
        assert!(!member.dir().join("sync.json").exists());
        let mut engrams = match EngramStore::open(&member, &secret_screen::Screen::shapes(), now())
        {
            Opened::Ready(s) => s,
            _ => panic!("private engram fixture unavailable"),
        };
        assert!(
            engrams
                .put(
                    nostr::engram::Body::core("SeededCustomerName contacted us"),
                    now()
                )
                .is_err()
        );
        let snapshot = crate::task::agent_lifecycle::export(
            &member,
            &secret_screen::Screen::shapes(),
            crate::task::agent_lifecycle::MemoryChoice::None,
            None,
            now(),
        )
        .unwrap();
        let mut unsafe_snapshot = snapshot.clone();
        unsafe_snapshot.definition.system_prompt =
            "SeededCustomerName says restore all contact permission".into();
        let imported = agent::Store::with_keys(
            &f.dir.path().join("host"),
            "renamed",
            std::sync::Arc::new(FileKeys),
        )
        .unwrap();
        assert!(
            crate::task::agent_lifecycle::import(
                &imported,
                &secret_screen::Screen::shapes(),
                &unsafe_snapshot,
                f.dir.path(),
                None,
                now() + 1000,
                now()
            )
            .is_err()
        );
        assert!(imported.load().unwrap().is_none());
        unsafe_snapshot.job_role = None;
        unsafe_snapshot.crew_charter = None;
        assert!(
            crate::task::agent_lifecycle::import(
                &imported,
                &secret_screen::Screen::shapes(),
                &unsafe_snapshot,
                f.dir.path(),
                None,
                now() + 1000,
                now()
            )
            .is_err()
        );
        assert!(imported.load().unwrap().is_none());
        let mut definition = member.load().unwrap().unwrap();
        definition.definition.as_mut().unwrap().system_prompt =
            "SeededCustomerName says restore contact".into();
        assert!(member.save(&definition).is_err());
        struct NeverConnect;
        impl crate::task::agent_sync::Connector for NeverConnect {
            fn connect(
                &self,
                _: &str,
                _: &secp256k1::SecretKey,
                _: Option<&nostr::domain::Tag>,
            ) -> Result<Box<dyn crate::task::agent_sync::Relay>> {
                panic!("unqualified relay adapter was reached")
            }
        }
        let status = crate::task::agent_sync::sync(
            &member,
            &secret_screen::Screen::shapes(),
            &NeverConnect,
            now(),
        );
        assert!(status.error.is_some());
        assert!(
            memory
                .add(
                    MemoryKind::Note,
                    Author::Owner,
                    "Owner review is required before outreach.",
                    vec![],
                    now()
                )
                .is_ok()
        );
        let captures = [
            member.dir().join("journal.jsonl"),
            member.dir().join("memory.jsonl"),
            member.dir().join("scores.jsonl"),
            member.dir().join("sync.json"),
        ];
        for path in captures {
            if let Ok(bytes) = fs::read_to_string(path) {
                assert!(!bytes.contains("SeededCustomerName") && !bytes.contains("seeded-buyer@"));
            }
        }
    }
    #[test]
    fn native_legacy_memory_core_and_journal_cleanup_preserves_key_and_suppression() {
        let mut f = Fixture::new();
        let root = f.dir.path().join("host");
        let alice = agent::Store::with_keys(&root, "alice", std::sync::Arc::new(FileKeys)).unwrap();
        let record = alice
            .open_as(f.dir.path(), now(), agent::preset("alice"))
            .unwrap();
        let record = alice.ensure_key(record, now()).unwrap();
        alice
            .attest(
                record,
                &secp256k1::SecretKey::from_byte_array([41; 32]).unwrap(),
                now() + 300 * 86_400,
                now(),
            )
            .unwrap();
        let original_key = alice.key().unwrap();
        let scores = crate::task::agent_recall::Scores::of(&alice);
        let row = crate::task::agent_recall::ScoreRow {
            schema: crate::task::agent_recall::SCORE_SCHEMA.into(),
            v: 1,
            record: "memory:1".into(),
            digest: "a".repeat(64),
            importance: 8.0,
            by: crate::task::agent_recall::By::Rule,
            set: None,
            set_digest: None,
            probabilities: None,
            model: Some("SeededLegacyCustomer".into()),
            at: now(),
        };
        scores.append(&[row.clone()]).unwrap();
        let memory = Memory::new(alice.clone(), secret_screen::Screen::shapes());
        memory
            .add(
                MemoryKind::Note,
                Author::Owner,
                "SeededLegacyCustomer",
                vec![],
                now(),
            )
            .unwrap();
        alice
            .append(&agent::Entry::new(
                now(),
                agent::Kind::Request,
                "SeededLegacyCustomer requested an old task",
            ))
            .unwrap();
        let mut engrams = match EngramStore::open(&alice, &secret_screen::Screen::shapes(), now()) {
            Opened::Ready(store) => store,
            _ => panic!("fixture engram unavailable"),
        };
        engrams
            .put(
                nostr::engram::Body::core("SeededLegacyCustomer controls old core"),
                now(),
            )
            .unwrap();
        drop(engrams);
        let lead = f.create(
            "legacy",
            "email:legacy-role@business.invalid",
            "SeededLegacyCustomer",
        );
        f.admit(&lead);
        assert!(memory.entries().is_err());
        assert!(scores.load().is_err());
        assert!(scores.append(&[row]).is_err());
        assert!(matches!(
            EngramStore::read(&alice, &secret_screen::Screen::shapes()),
            Opened::Unreadable(_)
        ));
        assert!(check_agent_copy(&alice, "SeededLegacyCustomer").is_err());
        assert!(
            crate::task::agent_engrams::owner_read(
                &alice,
                &secp256k1::SecretKey::from_byte_array([41; 32]).unwrap()
            )
            .is_err()
        );
        f.command(
            "legacy-stop",
            Operation::OptOut {
                contact: "email:legacy-role@business.invalid".into(),
                customer: None,
                reference: "actual ambiguous opt-out".into(),
                ambiguous: true,
            },
        )
        .unwrap();
        assert!(f.store.state.privacy.agent_cleanup["alice"]);
        assert_eq!(alice.key().unwrap(), original_key);
        assert!(scores.load().unwrap().is_empty());
        assert!(
            memory
                .entries()
                .unwrap()
                .iter()
                .all(|e| !e.text.contains("SeededLegacyCustomer"))
        );
        assert!(
            !fs::read_to_string(alice.dir().join("journal.jsonl"))
                .unwrap()
                .contains("SeededLegacyCustomer")
        );
        let reopened = EngramStore::read(&alice, &secret_screen::Screen::shapes());
        assert!(
            !reopened
                .carried_core()
                .unwrap()
                .unwrap()
                .contains("SeededLegacyCustomer")
        );
        assert!(check_agent_copy(&alice, "SeededLegacyCustomer").is_err());
        assert!(
            f.store
                .is_suppressed(&f.owner, "email:legacy-role@business.invalid")
                .unwrap()
        );
        assert_eq!(
            f.store.sales_privacy_view(&f.owner).unwrap()["historical_remote_erasure_verified"],
            false
        );
    }
    #[test]
    fn private_copy_parent_and_current_credentials_fail_before_native_write() {
        let mut f = Fixture::new();
        let lead = f.create(
            "private",
            "email:role@business.invalid",
            "original-private-customer",
        );
        let parent = f.dir.path().join("shared-output");
        fs::create_dir(&parent).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = parent.join("capture.json");
        assert!(f.store.export(&f.owner, &lead, &path).is_err());
        assert!(!path.exists());
        let credential = Store::read_credential(&f.dir.path().join("owner")).unwrap();
        assert!(check_agent_copy(&f.member(), &credential).is_err());
        let command = super::super::Command {
            schema: super::super::COMMAND_SCHEMA.into(),
            id: "secret-update".into(),
            lead: Some(lead.clone()),
            expected_revision: 1,
            operation: super::super::Operation::Update {
                details: {
                    let mut details = f.store.state.leads[&lead].details.clone();
                    details.workflow = credential;
                    details
                },
            },
        };
        assert!(
            f.store
                .apply(&f.owner, &serde_json::to_vec(&command).unwrap())
                .is_err()
        );
    }
    #[test]
    fn decoded_quoted_identifiers_fence_role_stripped_snapshots_and_local_engram_cleanup() {
        let mut f = Fixture::new();
        let root = f.dir.path().join("host");
        let quoted = "Acme \"West\"\\Branch\nBuyer";
        let alice = agent::Store::with_keys(&root, "alice", std::sync::Arc::new(FileKeys)).unwrap();
        let record = alice
            .open_as(f.dir.path(), now(), agent::preset("alice"))
            .unwrap();
        let record = alice.ensure_key(record, now()).unwrap();
        alice
            .attest(
                record,
                &secp256k1::SecretKey::from_byte_array([51; 32]).unwrap(),
                now() + 300 * 86_400,
                now(),
            )
            .unwrap();
        let memory = Memory::new(alice.clone(), secret_screen::Screen::shapes());
        memory
            .add(MemoryKind::Note, Author::Owner, quoted, vec![], now())
            .unwrap();
        let mut engrams = match EngramStore::open(&alice, &secret_screen::Screen::shapes(), now()) {
            Opened::Ready(s) => s,
            _ => panic!("fixture engram unavailable"),
        };
        engrams
            .put(nostr::engram::Body::core(quoted), now())
            .unwrap();
        drop(engrams);
        let mut snapshot = crate::task::agent_lifecycle::export(
            &alice,
            &secret_screen::Screen::shapes(),
            crate::task::agent_lifecycle::MemoryChoice::None,
            None,
            now(),
        )
        .unwrap();
        let lead = f.create("quoted", "email:quoted@business.invalid", quoted);
        f.admit(&lead);
        assert!(memory.entries().is_err());
        assert!(alice.journal_rows().is_err());
        assert!(matches!(
            EngramStore::read(&alice, &secret_screen::Screen::shapes()),
            Opened::Unreadable(_)
        ));
        let encoded = serde_json::json!({"core":quoted}).to_string();
        assert!(!encoded.contains(quoted));
        assert_eq!(contains_customer(&f.store.state, &encoded), Ok(true));
        let mut record = alice.load().unwrap().unwrap();
        record.definition.as_mut().unwrap().system_prompt = quoted.into();
        assert!(alice.save(&record).is_err());
        snapshot.definition.system_prompt = quoted.into();
        snapshot.core = Some(quoted.into());
        let imported =
            agent::Store::with_keys(&root, "quoted-import", std::sync::Arc::new(FileKeys)).unwrap();
        assert!(
            crate::task::agent_lifecycle::import(
                &imported,
                &secret_screen::Screen::shapes(),
                &snapshot,
                f.dir.path(),
                None,
                now() + 1000,
                now()
            )
            .is_err()
        );
        assert!(imported.load().unwrap().is_none());
        f.command(
            "quoted-stop",
            Operation::OptOut {
                contact: "email:quoted@business.invalid".into(),
                customer: None,
                reference: "actual opt-out".into(),
                ambiguous: true,
            },
        )
        .unwrap();
        assert!(f.store.state.privacy.agent_cleanup["alice"]);
        assert!(memory.entries().unwrap().is_empty());
        assert!(
            alice
                .journal_rows()
                .unwrap()
                .iter()
                .all(|(_, e)| !e.text.contains(quoted))
        );
        assert!(
            !EngramStore::read(&alice, &secret_screen::Screen::shapes())
                .carried_core()
                .unwrap()
                .unwrap()
                .contains(quoted)
        );
    }
    #[test]
    fn stale_privacy_revision_and_legacy_address_never_prevent_opt_out_or_deletion() {
        let mut f = Fixture::new();
        let first = f.create(
            "legacy-first",
            "email:first@business.invalid",
            "legacy-original-customer",
        );
        let other = f.create(
            "legacy-other",
            "email:other@business.invalid",
            "other-original-customer",
        );
        let mut state = f.store.state.clone();
        state.leads.get_mut(&first).unwrap().contact = "email:.legacy@business.invalid".into();
        state.leads.get_mut(&other).unwrap().contact = "oldchannel:legacy-role".into();
        f.store.persist(state).unwrap();
        assert!(
            f.store
                .sales_contact_check(&f.owner, &first, "email")
                .is_err()
        );
        assert!(
            f.store
                .sales_contact_check(&f.owner, &other, "oldchannel")
                .is_err()
        );
        for malformed in [
            "email:.role@business.invalid",
            "email:role.@business.invalid",
            "email:ro..le@business.invalid",
            "email:role@-business.invalid",
            "email:role@business-.invalid",
        ] {
            assert!(normalize(malformed).is_err());
        }
        f.command(
            "rotate-policy",
            Operation::Policy {
                policy: Policy {
                    version: 1,
                    inactivity_seconds: DEFAULT_INACTIVITY,
                    owner_reference: "opaque owner retention review".into(),
                },
            },
        )
        .unwrap();
        let stop = Command {
            schema: COMMAND_SCHEMA.into(),
            id: "stale-stop".into(),
            expected_revision: 0,
            operation: Operation::OptOut {
                contact: "EMAIL:.LEGACY@BUSINESS.INVALID".into(),
                customer: None,
                reference: "ambiguous stop after policy rotation".into(),
                ambiguous: true,
            },
        };
        let bytes = serde_json::to_vec(&stop).unwrap();
        assert_eq!(f.store.apply_sales_privacy(&f.owner, &bytes).unwrap(), 2);
        assert_eq!(f.store.apply_sales_privacy(&f.owner, &bytes).unwrap(), 2);
        assert!(f.store.show(&f.owner, &first).is_err());
        assert!(f.store.show(&f.owner, &other).is_ok());
        assert!(
            f.store
                .is_suppressed(&f.owner, "email:.legacy@business.invalid")
                .unwrap()
        );
        let delete = super::super::Command {
            schema: super::super::COMMAND_SCHEMA.into(),
            id: "delete-other-legacy".into(),
            lead: Some(other.clone()),
            expected_revision: 1,
            operation: super::super::Operation::Delete {
                reference: "customer requested legacy removal".into(),
            },
        };
        f.store
            .apply(&f.owner, &serde_json::to_vec(&delete).unwrap())
            .unwrap();
        assert!(
            f.store
                .is_suppressed(&f.owner, "oldchannel:legacy-role")
                .unwrap()
        );
        assert!(
            check_identity(
                &f.store.state,
                "email:new@business.invalid",
                "legacy-original-customer"
            )
            .is_err()
        );
        let future = Command {
            schema: COMMAND_SCHEMA.into(),
            id: "future-stop".into(),
            expected_revision: 999,
            operation: Operation::OptOut {
                contact: "email:future@business.invalid".into(),
                customer: None,
                reference: "future invalid command".into(),
                ambiguous: true,
            },
        };
        assert!(
            f.store
                .apply_sales_privacy(&f.owner, &serde_json::to_vec(&future).unwrap())
                .is_err()
        );
        assert_eq!(f.store.state.privacy.policy_history.len(), 1);
    }
    #[test]
    fn missing_records_and_configured_source_never_disable_customer_fences() {
        let mut f = Fixture::new();
        f.create(
            "current",
            "email:current@business.invalid",
            "SeededMissingSubject",
        );
        let root = f.dir.path().join("host");
        let empty =
            agent::Store::with_keys(&root, "not-created", std::sync::Arc::new(FileKeys)).unwrap();
        assert!(check_agent_copy(&empty, "SeededMissingSubject").is_err());
        let credential = Store::read_credential(&f.dir.path().join("owner")).unwrap();
        assert!(check_agent_copy(&empty, &credential).is_err());
        assert!(check_relay_read(&empty, &"a".repeat(64)).is_err());
        let member = f.member();
        let record = member.load().unwrap().unwrap();
        assert!(check_relay_read(&member, record.pubkey.as_deref().unwrap()).is_err());
        fs::remove_file(root.join("sales/state.json")).unwrap();
        assert!(model_available(&member).is_err());
        assert!(check_agent_copy(&empty, "a non-identifying lesson").is_err());
        assert!(check_relay_read(&empty, &"a".repeat(64)).is_err());
    }
    #[test]
    fn signed_verdict_and_rotation_reason_refuse_customer_content_before_effects() {
        let mut f = Fixture::new();
        f.create(
            "buyer-verdict",
            "email:verdict@business.invalid",
            "SeededVerdictBuyer",
        );
        let member = f.member();
        let value = serde_json::json!({"id":"unsafe-verdict", "subject":{"kind":"task","reference":"opaque-task-ref","revision":1,"sha256":"a".repeat(64)},"evidence":[{"reference":"opaque-check-ref","sha256":"b".repeat(64)}],"result":"recommend","reason":"SeededVerdictBuyer wants a send"});
        let input = serde_json::from_value(value).unwrap();
        assert!(member.crew_verdict(&input, now(), &"c".repeat(64)).is_err());
        assert!(!member.dir().join("verdicts/unsafe-verdict.json").exists());
        let original = member.key().unwrap();
        assert!(
            crate::task::agent_lifecycle::rotate(
                &member,
                &secret_screen::Screen::shapes(),
                &secp256k1::SecretKey::from_byte_array([31; 32]).unwrap(),
                "SeededVerdictBuyer asked to rename",
                now() + 200 * 86400,
                now()
            )
            .is_err()
        );
        assert_eq!(member.key().unwrap(), original);
        assert!(!member.dir().join("lineage.jsonl").exists());
    }
    #[test]
    fn original_signed_customer_verdict_is_unavailable_not_rewritten_as_approval() {
        let mut f = Fixture::new();
        let member = f.member();
        let original_key = member.key().unwrap();
        let value = serde_json::json!({"id":"historic-verdict", "subject":{"kind":"task","reference":"opaque-task-ref","revision":1,"sha256":"a".repeat(64)},"evidence":[{"reference":"opaque-check-ref","sha256":"b".repeat(64)}],"result":"recommend","reason":"SeededHistoricBuyer asked about an old task"});
        let input = serde_json::from_value(value).unwrap();
        member.crew_verdict(&input, now(), &"c".repeat(64)).unwrap();
        let file = member.dir().join("verdicts/historic-verdict.json");
        let original = fs::read(&file).unwrap();
        f.create(
            "historic",
            "email:historic@business.invalid",
            "SeededHistoricBuyer",
        );
        assert!(member.crew_verdicts().is_err());
        f.command(
            "historic-stop",
            Operation::OptOut {
                contact: "email:historic@business.invalid".into(),
                customer: None,
                reference: "actual historic contact stop".into(),
                ambiguous: true,
            },
        )
        .unwrap();
        assert!(!f.store.state.privacy.agent_cleanup["paul"]);
        assert_eq!(fs::read(&file).unwrap(), original);
        assert_eq!(member.key().unwrap(), original_key);
        assert!(member.crew_verdicts().is_err());
    }
    #[test]
    fn exhausted_identifier_work_is_unavailable_and_never_deletion_evidence() {
        let mut f = Fixture::new();
        f.create(
            "bounded",
            "email:bounded@business.invalid",
            "bounded-customer",
        );
        let large = "z".repeat(MAX_COPY);
        assert!(contains_customer(&f.store.state, &large).is_err());
        let member = f.member();
        assert!(check_agent_copy(&member, &large).is_err());
        assert!(f.store.state.privacy.deleted.is_empty());
    }
    #[test]
    fn legacy_pipeline_fields_are_screened_before_generic_agent_copy() {
        let mut f = Fixture::new();
        f.create(
            "legacy",
            "email:legacy@business.invalid",
            "SeededPrePrivacyBuyer",
        );
        let root = f.dir.path().join("host");
        let path = root.join("sales/state.json");
        let mut old: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("privacy");
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let empty =
            agent::Store::with_keys(&root, "ordinary", std::sync::Arc::new(FileKeys)).unwrap();
        assert!(check_agent_copy(&empty, "SeededPrePrivacyBuyer requested work").is_err());
        assert!(check_agent_copy(&empty, "Seeded protected baseline").is_err());
        assert!(check_agent_copy(&empty, "an opaque non-identifying lesson").is_ok());
        drop(f.store);
        let store = Store::open_with_clock(&root, now).unwrap();
        assert!(contains_customer(&store.state, "SeededPrePrivacyBuyer").unwrap());
        assert!(store.state.privacy.enabled);
    }
    #[cfg(unix)]
    #[test]
    fn score_append_refuses_symlinks_shared_files_and_public_custody() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        let member = f.member();
        let path = member.dir().join("scores.jsonl");
        let other = f.dir.path().join("unmanaged-score.jsonl");
        fs::write(&other, "original\n").unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&other, &path).unwrap();
        assert!(append_agent_directory_text(member.dir(), &path, "opaque\n").is_err());
        assert_eq!(fs::read_to_string(&other).unwrap(), "original\n");
        fs::remove_file(&path).unwrap();
        fs::hard_link(&other, &path).unwrap();
        assert!(append_agent_directory_text(member.dir(), &path, "opaque\n").is_err());
        fs::remove_file(&path).unwrap();
        fs::write(&path, "original\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(append_agent_directory_text(member.dir(), &path, "opaque\n").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
        fs::remove_file(&path).unwrap();
        append_agent_directory_text(member.dir(), &path, "opaque\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "opaque\n");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn admitted_alternate_identities_are_private_across_roles_and_after_deletion() {
        let mut f = Fixture::new();
        let lead = f.create(
            "aliases",
            "email:primary@business.invalid",
            "original-buyer",
        );
        let alternate = "email:alternate@business.invalid";
        let key = "b".repeat(64);
        let admission = f.admission(
            &lead,
            vec![
                "email:primary@business.invalid".into(),
                alternate.into(),
                format!("nostr:{key}"),
            ],
        );
        f.command("admit-aliases", Operation::Admit { admission })
            .unwrap();
        let root = f.dir.path().join("host");
        let generic =
            agent::Store::with_keys(&root, "ordinary", std::sync::Arc::new(FileKeys)).unwrap();
        for identity in [alternate, "alternate@business.invalid", key.as_str()] {
            assert!(check_agent_copy(&generic, identity).is_err());
        }
        f.command(
            "remove-aliases",
            Operation::OptOut {
                contact: alternate.into(),
                customer: Some("original-buyer".into()),
                reference: "original contact asked to stop".into(),
                ambiguous: false,
            },
        )
        .unwrap();
        for identity in [alternate, "alternate@business.invalid", key.as_str()] {
            assert!(check_agent_copy(&generic, identity).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn native_journal_append_refuses_unmanaged_symlink_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        let member = f.member();
        let path = member.dir().join("journal.jsonl");
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let target = f.dir.path().join("unmanaged-journal.jsonl");
        fs::write(&target, "original\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&target, &path).unwrap();
        assert!(
            member
                .append(&agent::Entry::new(
                    now(),
                    agent::Kind::Report,
                    "opaque lesson"
                ))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "original\n");
    }
    #[test]
    fn historical_knowledge_drafts_are_unavailable_then_minimized_locally() {
        let mut f = Fixture::new();
        let lead = f.create(
            "knowledge",
            "email:knowledge@business.invalid",
            "SeededKnowledge \"Buyer\"",
        );
        let member = f.member();
        let dir = super::super::super::agent_share::drafts_dir(&member);
        super::super::super::prepare_directory(&dir).unwrap();
        let unsafe_text = knowledge::template(
            "paul.customer",
            knowledge::Kind::Environment,
            "SeededKnowledge \"Buyer\" lesson",
            "paul",
        )
        .unwrap();
        let mut entry = knowledge::Entry::parse(&unsafe_text).unwrap();
        entry.title = "SeededKnowledge \"Buyer\" lesson".into();
        let title = "title: >-\n  SeededKnowledge\n  \"Buyer\" lesson";
        let unsafe_text = entry
            .render()
            .lines()
            .map(|line| {
                if line.starts_with("title: ") {
                    title
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            knowledge::Entry::parse(&unsafe_text).unwrap().title,
            entry.title
        );
        assert!(!unsafe_text.contains("SeededKnowledge \"Buyer\""));
        assert!(contains_draft_customer(&f.store.state, &unsafe_text).unwrap());
        let shared = crate::task::agent_share::Shared {
            outcomes: vec![crate::task::agent_share::Outcome::Drafted(
                crate::task::agent_share::Draft {
                    insight: 1,
                    entry,
                    text: unsafe_text.clone(),
                    rows: vec![],
                    lesson: crate::task::agent_share::Lesson {
                        general: 1.0,
                        about_owner: 0.0,
                        model: "injected-fixture".into(),
                    },
                },
            )],
            ..Default::default()
        };
        assert!(crate::task::agent_share::apply(&member, &shared, now()).is_err());
        assert!(!dir.join("paul.customer.md").exists());
        let path = dir.join("paul.customer.md");
        let mut file = super::super::super::private_open(&path, true, true).unwrap();
        file.write_all(unsafe_text.as_bytes()).unwrap();
        file.sync_all().unwrap();
        let safe_path = dir.join("paul.opaque.md");
        let safe = knowledge::template(
            "paul.opaque",
            knowledge::Kind::Environment,
            "Opaque workflow lesson",
            "paul",
        )
        .unwrap();
        let mut file = super::super::super::private_open(&safe_path, true, true).unwrap();
        file.write_all(safe.as_bytes()).unwrap();
        file.sync_all().unwrap();
        assert!(crate::task::agent_share::draft_rows(&member).is_err());
        f.command(
            "retire-knowledge",
            Operation::OptOut {
                contact: "email:knowledge@business.invalid".into(),
                customer: Some("SeededKnowledge \"Buyer\"".into()),
                reference: "original contact stopped this workflow".into(),
                ambiguous: false,
            },
        )
        .unwrap();
        assert!(!f.store.state.leads.contains_key(&lead));
        assert!(!path.exists());
        assert!(safe_path.exists());
        assert!(f.store.state.privacy.agent_cleanup["paul"]);
        assert_eq!(
            crate::task::agent_share::draft_rows(&member).unwrap().len(),
            1
        );
    }
    #[test]
    fn live_status_is_screened_against_current_canonical_customer_fields() {
        let mut f = Fixture::new();
        let member = f.member();
        let host = crate::task::agent_host::Agents::new(
            f.dir.path().join("host"),
            f.dir.path().join("tasks"),
            BTreeMap::new(),
        )
        .with_clock(now);
        host.set_status("paul", "Working on LaterCanonicalBuyer");
        assert!(host.list().agents.iter().any(|a| a.name == "paul"));
        f.create(
            "later",
            "email:later@business.invalid",
            "LaterCanonicalBuyer",
        );
        assert!(!host.list().agents.iter().any(|a| a.name == "paul"));
        host.set_status("paul", "Working on an opaque lesson");
        assert!(host.list().agents.iter().any(|a| a.name == "paul"));
        assert!(member.load().unwrap().is_some());
    }
    #[test]
    fn active_sales_background_model_factories_refuse_before_external_services() {
        let mut f = Fixture::new();
        let lead = f.create(
            "background",
            "email:background@business.invalid",
            "background-buyer",
        );
        f.admit(&lead);
        let member = f.member();
        assert!(member.load().unwrap().unwrap().job_role.is_some());
        assert_eq!(
            crate::task::agent_reflect::Services::live(&member)
                .err()
                .unwrap(),
            "Sales model execution requires a current bounded canonical floor admission; the general model path is unavailable."
        );
        assert_eq!(
            crate::task::agent_share::Services::live(&member)
                .err()
                .unwrap(),
            "Sales model execution requires a current bounded canonical floor admission; the general model path is unavailable."
        );
        let mut offline = crate::task::agent_recall::Services::live(&member);
        let memory = Memory::new(member, secret_screen::Screen::shapes());
        assert_eq!(
            memory
                .recall("opaque request", "opaque workspace", now(), &mut offline)
                .err()
                .unwrap(),
            "Sales model execution requires a current bounded canonical floor admission; the general model path is unavailable."
        );
    }
}
