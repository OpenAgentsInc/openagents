//! Local sales model admission. Usage telemetry never grants spending authority.
//! Every uncertain call keeps its original bound, including after midnight.
use super::*;
use agents::{AgentAccess, Anchor};

pub const FLOOR_USD_MILLIONTHS: u64 = 5_000_000;
const MAX_RECORDS: usize = 4096;
const MAX_GRANTS: usize = 128;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Planner,
    Reporter,
    Coder,
    Jev,
    Claim,
    Price,
    CitedAnswer,
    Recommendation,
    Embedding,
    Reflection,
    Verification,
    Correction,
    Training,
    DayPlan,
    Retry,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    LocalDeterministic,
    ListPrice,
}

/// A host adapter's reviewed list-price ceiling. The adapter must enforce its
/// input, output, and attempt caps; this declaration is not remote attestation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub basis: Basis,
    pub kind: Kind,
    pub source_revision: String,
    pub price_revision: String,
    pub recipient: String,
    pub max_input_bytes: u64,
    pub max_output_tokens: u64,
    pub max_attempts: u32,
    pub max_elapsed_secs: u64,
    pub input_usd_millionths_per_million: u64,
    pub output_usd_millionths_per_million: u64,
}
impl Source {
    pub fn sha256(&self) -> Result<String> {
        token(&self.source_revision)?;
        token(&self.price_revision)?;
        text(&self.recipient, 256)?;
        if self.max_input_bytes == 0
            || self.max_input_bytes > 8_000_000
            || self.max_output_tokens == 0
            || self.max_output_tokens > 1_000_000
            || !(1..=16).contains(&self.max_attempts)
            || !(1..=600).contains(&self.max_elapsed_secs)
            || self.input_usd_millionths_per_million > 1_000_000_000
            || self.output_usd_millionths_per_million > 1_000_000_000
            || (self.basis == Basis::ListPrice
                && (self.input_usd_millionths_per_million == 0
                    || self.output_usd_millionths_per_million == 0))
            || (self.basis == Basis::LocalDeterministic
                && (self.input_usd_millionths_per_million != 0
                    || self.output_usd_millionths_per_million != 0
                    || self.max_attempts != 1
                    || !matches!(
                        self.kind,
                        Kind::Claim
                            | Kind::Price
                            | Kind::CitedAnswer
                            | Kind::Recommendation
                            | Kind::Verification
                            | Kind::Training
                    )))
        {
            return Err(
                "sales model source needs finite priced input, output, and attempt caps".into(),
            );
        }
        Ok(digest(
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
        ))
    }
    /// One byte per input token is conservative. Cached tokens have the full
    /// input price; every possible provider retry consumes its own bound.
    pub fn upper_bound(&self, input_bytes: u64) -> Result<u64> {
        self.sha256()?;
        if input_bytes == 0 || input_bytes > self.max_input_bytes {
            return Err("sales model input exceeds its original source bound".into());
        }
        let cost = (u128::from(input_bytes) * u128::from(self.input_usd_millionths_per_million)
            + u128::from(self.max_output_tokens)
                * u128::from(self.output_usd_millionths_per_million))
        .div_ceil(1_000_000)
            * u128::from(self.max_attempts);
        u64::try_from(cost).map_err(|_| "sales model cost bound exceeds its range".into())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub revision: u64,
    pub floor_daily_usd_millionths: u64,
    pub agent_daily_usd_millionths: u64,
    pub request_usd_millionths: u64,
    pub sources: Vec<Source>,
}
impl Policy {
    pub fn sha256(&self) -> Result<String> {
        if self.schema != "openagents.sales-model-policy.v1"
            || self.revision == 0
            || self.floor_daily_usd_millionths == 0
            || self.floor_daily_usd_millionths > FLOOR_USD_MILLIONTHS
            || self.agent_daily_usd_millionths == 0
            || self.agent_daily_usd_millionths > self.floor_daily_usd_millionths
            || self.request_usd_millionths == 0
            || self.request_usd_millionths > self.agent_daily_usd_millionths
            || self.sources.is_empty()
            || self.sources.len() > 32
        {
            return Err(
                "sales model policy requires explicit floor, agent, request, and source limits"
                    .into(),
            );
        }
        let sources = self
            .sources
            .iter()
            .map(Source::sha256)
            .collect::<Result<Vec<_>>>()?;
        if sources
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != sources.len()
        {
            return Err("sales model policy contains duplicate sources".into());
        }
        Ok(digest(
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub request: String,
    pub attempt: u32,
    pub source: Source,
    pub input_bytes: u64,
    pub input_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub request: String,
    /// A known list-price estimate, distinct from provider billing. None is unknown.
    pub estimated_usd_millionths: Option<u64>,
    pub billed_usd_millionths: Option<u64>,
    pub evidence_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub schema: String,
    pub id: String,
    pub floor: String,
    pub lead: String,
    pub assignment: String,
    pub native: Anchor,
    pub actor: String,
    pub policy_sha256: String,
    pub sales_policy_sha256: String,
    pub day: u64,
    pub admitted_at: u64,
    pub deadline_at: u64,
    pub status: Status,
    /// An interrupted local read can cost zero while its result remains unknown.
    pub execution_unknown: bool,
    pub input: Input,
    pub maximum_usd_millionths: u64,
    pub request_limit_usd_millionths: u64,
    pub settlements: Vec<Settlement>,
    pub training: Option<TrainingContext>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Reserved,
    Unknown,
    Known,
    Breach,
}
/// Owner-approved synthetic context. This grants no lead or outbound authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrainingContext {
    pub agent: String,
    pub anchor: Anchor,
    pub persona_sha256: String,
    pub run: String,
    pub synthetic_source_sha256: String,
}
enum Authority {
    Sales(AgentAccess),
    Training {
        owner: Access,
        context: TrainingContext,
    },
}
/// A new call owns an OS lock until settlement. A retry only returns its
/// original receipt and never grants another execution.
pub struct Admission {
    reservation: Reservation,
    custody: Option<(PathBuf, File)>,
    root: PathBuf,
    root_directory: File,
    authority: Authority,
    clock: fn() -> u64,
    keys: std::sync::Arc<dyn super::super::agent_key::KeyStore>,
}
/// A local host adapter must use the exact reviewed source and enforce text-only
/// provider input, output, attempt, and elapsed-time caps before network work.
/// It must reserve helpers separately; tools and hidden downstream calls are
/// unsupported by this contract.
pub trait Adapter {
    type Output: Serialize;
    fn source(&self) -> &Source;
    fn execute(&mut self, body: &[u8], caps: &Source) -> Result<Self::Output>;
}
/// Retains attempt custody after execution, without another execution method.
pub struct Executed {
    admission: Admission,
}
impl Executed {
    pub fn receipt(&self) -> &Reservation {
        self.admission.receipt()
    }
}
impl Admission {
    pub fn receipt(&self) -> &Reservation {
        &self.reservation
    }
    pub fn may_execute(&self) -> bool {
        self.custody.is_some()
    }
    pub fn verify_input(&self, bytes: &[u8]) -> Result<()> {
        let (path, file) = self
            .custody
            .as_ref()
            .ok_or("original sales model attempt has no new execution right")?;
        super::super::verify_same_file(path, file)
            .map_err(|_| "sales model attempt custody changed")?;
        if bytes.len() as u64 != self.reservation.input.input_bytes
            || digest(bytes) != self.reservation.input.input_sha256
        {
            return Err("sales model original input changed".into());
        }
        agents::native::same_directory(&self.root, &self.root_directory)?;
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.keys.clone();
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "sales model input must be bounded UTF-8 text")?;
        super::privacy::check_credentials(&store.state, text)?;
        // An arbitrary caller body is not a typed, field-scoped projection.
        // Native helpers can look up admitted fields through their own Store
        // fence; provider input cannot carry protected customer material.
        if super::privacy::contains_customer(&store.state, text)? || text.contains('@') {
            return Err("sales expense input cannot disclose protected customer material".into());
        }
        let native = match &self.authority {
            Authority::Sales(access) => {
                let (_, assignment, _, native) = store.checked_sales_agent(access)?;
                if assignment.anchor != self.reservation.native
                    || assignment.reference != self.reservation.assignment
                {
                    return Err("sales model original assignment changed".into());
                }
                store.recheck_sales_agent(access, &native)?;
                native
            }
            Authority::Training { owner, context } => {
                store.admin(owner)?;
                if self.reservation.training.as_ref() != Some(context) {
                    return Err("synthetic training context changed".into());
                }
                let native = agents::native::Native::read(
                    &self.root,
                    &context.agent,
                    (self.clock)(),
                    self.keys.clone(),
                )?;
                if native.anchor != context.anchor {
                    return Err("synthetic training native authority changed".into());
                }
                native.recheck()?;
                native
            }
        };
        if native
            .expense_scope()
            .is_none_or(|s| s.floor != self.reservation.floor || s.actor != self.reservation.actor)
            || store.state.expenses.current.as_ref() != Some(&self.reservation.policy_sha256)
            || (self.clock)() > self.reservation.deadline_at
            || store
                .state
                .expenses
                .reservations
                .get(&self.reservation.id)
                .is_none_or(|r| r.status != Status::Reserved || r != &self.reservation)
        {
            return Err("sales model original execution authority changed".into());
        }
        native.recheck()?;
        Ok(())
    }
    /// Consume a fresh execution right once. Failed or interrupted work drops
    /// its lock and becomes unknown on canonical recovery, never refunded.
    pub fn execute<A: Adapter>(
        self,
        body: &[u8],
        adapter: &mut A,
    ) -> Result<(A::Output, Executed)> {
        self.verify_input(body)?;
        std::str::from_utf8(body).map_err(|_| "sales model input must be bounded UTF-8 text")?;
        if adapter.source() != &self.reservation.input.source {
            return Err("sales model adapter source or price changed before execution".into());
        }
        let result = adapter.execute(body, &self.reservation.input.source)?;
        if adapter.source() != &self.reservation.input.source {
            return Err("sales model adapter source or price changed during execution".into());
        }
        self.verify_input(body)?;
        let text = serde_json::to_string(&result)
            .map_err(|_| "sales expense result serialization failed")?;
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.keys.clone();
        super::privacy::check_credentials(&store.state, &text)?;
        if super::privacy::contains_customer(&store.state, &text)? || text.contains('@') {
            return Err("sales expense result cannot disclose protected customer material".into());
        }
        Ok((result, Executed { admission: self }))
    }
}
impl Reservation {
    fn held(&self) -> bool {
        matches!(self.status, Status::Unknown | Status::Breach)
    }
    fn charged(&self) -> u64 {
        self.settlements
            .last()
            .map_or(self.maximum_usd_millionths, |s| {
                s.estimated_usd_millionths
                    .unwrap_or(self.maximum_usd_millionths)
                    .max(s.billed_usd_millionths.unwrap_or(0))
            })
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    policies: BTreeMap<String, Policy>,
    current: Option<String>,
    floor: Option<String>,
    last_clock: u64,
    reservations: BTreeMap<String, Reservation>,
}
impl Book {
    pub(super) fn admitted_policy(&self, source: &Source) -> Result<(&str, &Policy)> {
        let sha = self
            .current
            .as_deref()
            .ok_or("sales model policy is unavailable")?;
        let policy = self
            .policies
            .get(sha)
            .ok_or("sales model policy is unavailable")?;
        if !policy.sources.contains(source) {
            return Err("sales decision source is no longer admitted".into());
        }
        Ok((sha, policy))
    }
    /// Original attribution only; this lookup grants no execution authority.
    pub(super) fn reservations(&self) -> impl Iterator<Item = &Reservation> {
        self.reservations.values()
    }
    pub(super) fn reservation(&self, id: &str) -> Option<&Reservation> {
        self.reservations.get(id)
    }
    /// A dead attempt's OS lock releases on crash. Recovery never frees its cost.
    pub(super) fn recover(&mut self, dir: &Path, now: u64) -> Result<bool> {
        self.clock(now)?;
        let mut changed = false;
        for r in self
            .reservations
            .values_mut()
            .filter(|r| r.status == Status::Reserved)
        {
            let path = dir.join(format!("model-{}.lock", r.id));
            let alive = super::super::private_open(&path, false, true)
                .ok()
                .is_some_and(|f| matches!(f.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
            if !alive || now > r.deadline_at {
                r.execution_unknown = true;
                if r.input.source.basis == Basis::LocalDeterministic {
                    r.status = Status::Known;
                    r.settlements.push(Settlement {
                        request: "native-local-zero-recovery".into(),
                        estimated_usd_millionths: Some(0),
                        billed_usd_millionths: None,
                        evidence_sha256: r.input.source.sha256()?,
                    });
                } else {
                    r.status = Status::Unknown;
                }
                changed = true;
            }
        }
        if changed {
            self.last_clock = now;
        }
        Ok(changed)
    }
    pub(super) fn check(&self) -> Result<()> {
        if self.policies.len() > MAX_GRANTS
            || self.reservations.len() > MAX_RECORDS
            || self
                .current
                .as_ref()
                .is_some_and(|v| !self.policies.contains_key(v))
        {
            return Err("sales model book exceeds its retained scope".into());
        }
        for (sha, p) in &self.policies {
            if p.sha256()? != *sha {
                return Err("sales model policy changed".into());
            }
        }
        for (id, r) in &self.reservations {
            token(&r.actor)?;
            token(&r.floor)?;
            token(&r.input.input_sha256)?;
            token(&r.sales_policy_sha256)?;
            super::id(&r.input.request)?;
            if r.schema != "openagents.sales-model-reservation.v1"
                || id != &r.id
                || *id
                    != digest(
                        format!("{}:{}:{}", r.floor, r.input.request, r.input.attempt).as_bytes(),
                    )
                || Some(&r.floor) != self.floor.as_ref()
                || !self.policies.contains_key(&r.policy_sha256)
                || r.maximum_usd_millionths != r.input.source.upper_bound(r.input.input_bytes)?
                || !(1..=1024).contains(&r.input.attempt)
                || r.day != agents::business_day(r.admitted_at)?
                || r.deadline_at
                    != r.admitted_at
                        .checked_add(r.input.source.max_elapsed_secs)
                        .ok_or("sales expense deadline exceeds its bound")?
                || r.maximum_usd_millionths > r.request_limit_usd_millionths
                || r.training.as_ref().is_some_and(|t| {
                    !r.lead.is_empty()
                        || !r.assignment.is_empty()
                        || t.anchor != r.native
                        || t.agent != r.native.name
                })
                || r.training.is_none() && (r.lead.is_empty() || r.assignment.is_empty())
                || r.settlements.len() > 32
            {
                return Err("sales model original reservation changed".into());
            }
            if !self.policies[&r.policy_sha256]
                .sources
                .contains(&r.input.source)
            {
                return Err("sales expense source is outside its original policy".into());
            }
            for settlement in &r.settlements {
                super::id(&settlement.request)?;
                token(&settlement.evidence_sha256)?;
                if r.input.source.basis == Basis::LocalDeterministic
                    && (settlement.estimated_usd_millionths != Some(0)
                        || settlement.billed_usd_millionths.is_some())
                {
                    return Err("local expense history cannot contain provider billing".into());
                }
            }
        }
        Ok(())
    }
    fn clock(&self, now: u64) -> Result<()> {
        if now < self.last_clock {
            Err("sales wall clock moved backwards; expense admission is unavailable".into())
        } else {
            Ok(())
        }
    }
}
impl Store {
    pub fn sales_model_reservations(
        &mut self,
        access: &Access,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Reservation>> {
        self.refresh()?;
        self.admin(access)?;
        if !(1..=100).contains(&limit) {
            return Err("sales model history page bound is 1 to 100".into());
        }
        Ok(self
            .state
            .expenses
            .reservations
            .iter()
            .filter(|(id, _)| after.is_none_or(|a| id.as_str() > a))
            .take(limit)
            .map(|(_, r)| r.clone())
            .collect())
    }
    fn model_admission(
        &self,
        authority: Authority,
        r: Reservation,
        custody: Option<(PathBuf, File)>,
    ) -> Result<Admission> {
        Ok(Admission {
            reservation: r,
            custody,
            root: self
                .dir
                .parent()
                .ok_or("sales host root is unavailable")?
                .into(),
            root_directory: self.root_directory.try_clone().map_err(|e| e.to_string())?,
            authority,
            clock: self.clock,
            keys: self.native_keys.clone(),
        })
    }
    /// Records explicit owner-reviewed source caps. Supersession never reprices
    /// an original call, and no new policy can exceed the fixed $5 floor ceiling.
    pub fn publish_sales_model_policy(
        &mut self,
        access: &Access,
        policy: &Policy,
        approved: &str,
    ) -> Result<String> {
        self.refresh()?;
        self.admin(access)?;
        super::privacy::check_credentials(
            &self.state,
            &serde_json::to_string(policy)
                .map_err(|_| "sales expense policy serialization failed")?,
        )?;
        let sha = policy.sha256()?;
        if approved != sha {
            return Err("approve the exact private sales model policy digest".into());
        }
        let now = (self.clock)();
        self.state.expenses.clock(now)?;
        if let Some(current) = &self.state.expenses.current {
            if current == &sha {
                return Ok(sha);
            }
            if self.state.expenses.policies[current]
                .revision
                .checked_add(1)
                != Some(policy.revision)
            {
                return Err("sales model policy revision changed".into());
            }
        } else if policy.revision != 1 {
            return Err("sales model policy starts at revision one".into());
        }
        if self.state.expenses.policies.len() >= MAX_GRANTS {
            return Err("sales model policy history is full".into());
        }
        let mut next = self.state.clone();
        let floor = digest(
            format!(
                "sales-model-floor:{}:{}",
                next.salt,
                next.owner.as_deref().ok_or("sales owner is unavailable")?
            )
            .as_bytes(),
        );
        if next.expenses.floor.as_ref().is_some_and(|v| v != &floor) {
            return Err("original sales floor ownership changed".into());
        }
        next.expenses.floor = Some(floor);
        next.expenses.current = Some(sha.clone());
        next.expenses.policies.insert(sha.clone(), policy.clone());
        next.expenses.last_clock = now;
        self.persist(next)?;
        Ok(sha)
    }
    /// Reserve before a host adapter makes any request. Drop this Store before
    /// network work; reopen it to settle. Unknown charges stop all new floor work.
    pub fn reserve_sales_model(
        &mut self,
        access: &AgentAccess,
        input: &Input,
    ) -> Result<Admission> {
        self.refresh()?;
        id(&input.request)?;
        super::privacy::check_credentials(
            &self.state,
            &serde_json::to_string(input)
                .map_err(|_| "sales expense input serialization failed")?,
        )?;
        token(&input.input_sha256)?;
        if input.attempt == 0 || input.attempt > 1024 {
            return Err("sales model attempt exceeds its scope".into());
        }
        let (lead, assignment, sales_policy, native) = self.checked_sales_agent(access)?;
        let lead_id = lead.id.clone();
        let assignment_id = assignment.reference.clone();
        let sales_policy_sha256 = assignment.policy_sha256.clone();
        let execution_budget = sales_policy.execution_budget_usd_millionths;
        let recipient_allowed = sales_policy
            .data_recipients
            .contains(&input.source.recipient)
            && lead
                .details
                .data
                .recipients
                .contains(&input.source.recipient);
        self.reserve_expense(
            Authority::Sales(access.clone()),
            input,
            native,
            lead_id,
            assignment_id,
            sales_policy_sha256,
            execution_budget,
            recipient_allowed,
            None,
        )
    }
    /// Reserve synthetic training under current owner and native charter custody.
    /// This shares every floor, agent, request, clock, and unknown-cost limit.
    pub fn reserve_sales_training(
        &mut self,
        owner: &Access,
        context: &TrainingContext,
        input: &Input,
    ) -> Result<Admission> {
        self.refresh()?;
        self.admin(owner)?;
        super::privacy::check_credentials(
            &self.state,
            &serde_json::to_string(context)
                .map_err(|_| "synthetic context serialization failed")?,
        )?;
        super::privacy::check_credentials(
            &self.state,
            &serde_json::to_string(input).map_err(|_| "synthetic input serialization failed")?,
        )?;
        id(&context.agent)?;
        id(&context.run)?;
        token(&context.persona_sha256)?;
        token(&context.synthetic_source_sha256)?;
        id(&input.request)?;
        token(&input.input_sha256)?;
        if !(1..=1024).contains(&input.attempt)
            || !matches!(
                input.source.kind,
                Kind::Training
                    | Kind::Jev
                    | Kind::Reflection
                    | Kind::Verification
                    | Kind::Correction
                    | Kind::Retry
            )
        {
            return Err("synthetic training work is outside its explicit context".into());
        }
        let native = agents::native::Native::read(
            self.dir.parent().ok_or("host root is unavailable")?,
            &context.agent,
            (self.clock)(),
            self.native_keys.clone(),
        )?;
        if native.anchor != context.anchor {
            return Err("synthetic training native anchor changed".into());
        }
        let owner_copy = Access {
            principal: owner.principal.clone(),
            token_digest: owner.token_digest.clone(),
        };
        self.reserve_expense(
            Authority::Training {
                owner: owner_copy,
                context: context.clone(),
            },
            input,
            native,
            String::new(),
            String::new(),
            digest(
                &serde_json::to_vec(context)
                    .map_err(|_| "synthetic context serialization failed")?,
            ),
            FLOOR_USD_MILLIONTHS,
            true,
            Some(context.clone()),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn reserve_expense(
        &mut self,
        authority: Authority,
        input: &Input,
        native: agents::native::Native,
        lead_id: String,
        assignment_id: String,
        sales_policy_sha256: String,
        execution_budget: u64,
        recipient_allowed: bool,
        training: Option<TrainingContext>,
    ) -> Result<Admission> {
        let now = (self.clock)();
        let book = &self.state.expenses;
        book.clock(now)?;
        let policy_sha = book
            .current
            .as_ref()
            .ok_or("sales model reservation policy is unavailable")?;
        let policy = &book.policies[policy_sha];
        if !policy.sources.contains(&input.source) || !recipient_allowed {
            return Err("sales model source or recipient is outside current native policy".into());
        }
        let maximum = input.source.upper_bound(input.input_bytes)?;
        let request_limit = policy.request_usd_millionths.min(execution_budget);
        if (maximum == 0 && input.source.basis != Basis::LocalDeterministic)
            || maximum > request_limit
        {
            return Err("sales model call exceeds its explicit native request budget".into());
        }
        let floor = book
            .floor
            .as_ref()
            .ok_or("original sales floor is unavailable")?;
        let actor = native.expense_scope().map_or_else(
            || digest(format!("{floor}:{}:{}", native.anchor.owner, native.anchor.name).as_bytes()),
            |s| s.actor.clone(),
        );
        if native.expense_scope().is_some_and(|s| s.floor != *floor) {
            return Err("native identity belongs to another original expense floor; migrate its complete canonical book".into());
        }
        let key = digest(format!("{floor}:{}:{}", input.request, input.attempt).as_bytes());
        if let Some(old) = book.reservations.get(&key) {
            if old.input != *input
                || old.lead != lead_id
                || old.assignment != assignment_id
                || old.native != native.anchor
                || old.training != training
            {
                return Err(
                    "sales model request or attempt conflicts with its original reservation".into(),
                );
            }
            native.recheck()?;
            return self.model_admission(authority, old.clone(), None);
        }
        if book.reservations.values().any(Reservation::held) {
            return Err(
                "unknown sales model usage holds the floor; review it before new work".into(),
            );
        }
        if book.reservations.len() >= MAX_RECORDS {
            return Err(
                "sales model history is full; retain it during reviewed maintenance".into(),
            );
        }
        let day = agents::business_day(now)?;
        let sum = |filter: &dyn Fn(&Reservation) -> bool| -> Result<u64> {
            book.reservations
                .values()
                .filter(|r| filter(r))
                .try_fold(maximum, |n, r| {
                    n.checked_add(r.charged())
                        .ok_or_else(|| "sales expense sum exceeds its range".into())
                })
        };
        if sum(&|r| r.day == day || r.status == Status::Reserved)?
            > policy.floor_daily_usd_millionths
            || sum(&|r| (r.day == day || r.status == Status::Reserved) && r.actor == actor)?
                > policy.agent_daily_usd_millionths
            || sum(&|r| r.input.request == input.request)? > request_limit
        {
            return Err("sales model floor, agent, or original request budget is exhausted".into());
        }
        let original_limit = book
            .reservations
            .values()
            .filter(|r| r.input.request == input.request)
            .map(|r| r.request_limit_usd_millionths)
            .min()
            .unwrap_or(request_limit)
            .min(request_limit);
        if sum(&|r| r.input.request == input.request)? > original_limit {
            return Err("original sales model request allocation is exhausted".into());
        }
        let path = self.dir.join(format!("model-{key}.lock"));
        let guard = super::super::open_lock(&path).map_err(|e| e.to_string())?;
        guard
            .try_lock()
            .map_err(|_| "sales model original attempt custody is unavailable")?;
        let r = Reservation {
            schema: "openagents.sales-model-reservation.v1".into(),
            id: key.clone(),
            floor: floor.clone(),
            lead: lead_id,
            assignment: assignment_id,
            native: native.anchor.clone(),
            actor: actor.clone(),
            policy_sha256: policy_sha.clone(),
            sales_policy_sha256,
            day,
            admitted_at: now,
            deadline_at: now
                .checked_add(input.source.max_elapsed_secs)
                .ok_or("sales model deadline exceeds its range")?,
            status: Status::Reserved,
            execution_unknown: false,
            input: input.clone(),
            maximum_usd_millionths: maximum,
            request_limit_usd_millionths: original_limit,
            settlements: vec![],
            training,
        };
        native.recheck()?;
        let native_name = native.anchor.name.clone();
        native.bind_expense_scope(&super::super::agent::SalesModelScope {
            floor: r.floor.clone(),
            actor,
        })?;
        drop(native);
        let current = agents::native::Native::read(
            self.dir.parent().ok_or("host root is unavailable")?,
            &native_name,
            now,
            self.native_keys.clone(),
        )?;
        if current.anchor != r.native {
            return Err("native expense authority changed before reservation".into());
        }
        current.recheck()?;
        let mut next = self.state.clone();
        next.expenses.last_clock = now;
        next.expenses.reservations.insert(key, r.clone());
        self.persist(next)?;
        self.model_admission(authority, r, Some((path, guard)))
    }
    /// Owner reconciliation preserves the original reservation after retirement.
    /// Known estimates and actual billing remain separate; an over-bound charge
    /// is retained as a breach and keeps the floor stopped.
    pub fn settle_sales_model(
        &mut self,
        access: &Access,
        reservation: &str,
        input: &Settlement,
    ) -> Result<Reservation> {
        self.refresh()?;
        self.admin(access)?;
        id(&input.request)?;
        super::privacy::check_credentials(
            &self.state,
            &serde_json::to_string(input)
                .map_err(|_| "sales expense settlement serialization failed")?,
        )?;
        token(&input.evidence_sha256)?;
        let now = (self.clock)();
        self.state.expenses.clock(now)?;
        let mut r = self
            .state
            .expenses
            .reservations
            .get(reservation)
            .cloned()
            .ok_or("original sales model reservation is unavailable")?;
        if r.input.source.basis == Basis::LocalDeterministic
            && (input.estimated_usd_millionths != Some(0) || input.billed_usd_millionths.is_some())
        {
            return Err(
                "deterministic local helpers have zero model cost and no provider billing".into(),
            );
        }
        if let Some(old) = r.settlements.iter().find(|s| s.request == input.request) {
            if old != input {
                return Err("sales model settlement retry changed".into());
            }
            return Ok(r);
        }
        if r.settlements.len() >= 32 {
            return Err("sales model reconciliation history is full".into());
        }
        if r.settlements.iter().any(|s| {
            s.estimated_usd_millionths
                .is_some_and(|v| input.estimated_usd_millionths != Some(v))
                || s.billed_usd_millionths
                    .is_some_and(|v| input.billed_usd_millionths != Some(v))
        }) {
            return Err("known original sales model settlement cannot be rewritten".into());
        }
        r.settlements.push(input.clone());
        r.status = if input.estimated_usd_millionths.is_none() {
            Status::Unknown
        } else if input
            .estimated_usd_millionths
            .is_some_and(|v| v > r.maximum_usd_millionths)
            || input
                .billed_usd_millionths
                .is_some_and(|v| v > r.maximum_usd_millionths)
        {
            Status::Breach
        } else {
            Status::Known
        };
        let mut next = self.state.clone();
        next.expenses.last_clock = now;
        next.expenses
            .reservations
            .insert(reservation.into(), r.clone());
        self.persist(next)?;
        Ok(r)
    }
    pub fn sales_model_reservation(
        &mut self,
        access: &Access,
        reservation: &str,
    ) -> Result<Reservation> {
        self.refresh()?;
        self.admin(access)?;
        self.state
            .expenses
            .reservations
            .get(reservation)
            .cloned()
            .ok_or("original sales model reservation is unavailable".into())
    }
}
