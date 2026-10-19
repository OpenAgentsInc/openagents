//! Bounded local sales helpers over the reviewed claim register. Source text
//! supplies evidence only; it cannot select tools, recipients, or permissions.
use super::*;
use crate::task::sales::{agents, expenses};

pub const SCHEMA: &str = "openagents.sales-claim-helper.v1";
const PREFIX: &str = "sales-helper-";

pub(in crate::task::sales) fn retire_lead(state: &mut super::super::State, lead: &str) {
    state.claims.helpers.retain(|_, record| record.lead != lead);
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    Claims,
    CurrentPrices,
    CitedAnswer,
    Recommendation,
}
impl Query {
    fn kind(self) -> expenses::Kind {
        match self {
            Self::Claims => expenses::Kind::Claim,
            Self::CurrentPrices => expenses::Kind::Price,
            Self::CitedAnswer => expenses::Kind::CitedAnswer,
            Self::Recommendation => expenses::Kind::Recommendation,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub query: Query,
    pub release: String,
    pub claims: Vec<Pin>,
}
/// Pins the reviewed register and evidence digests, without disclosing host paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub claim: Pin,
    pub source: Pin,
    pub claim_review_sha256: String,
    pub source_input_sha256: String,
    pub source_review_sha256: String,
    pub evidence_sha256: Vec<String>,
    pub reviewer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    OwnerReviewRequired,
    ReturnForReview,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub views: Vec<ClaimView>,
    pub citations: Vec<Citation>,
    /// Exact reviewed clauses with their limits and price terms. No generated facts.
    pub draft_body: Option<String>,
    pub recommendation: Recommendation,
    pub outbound_authority: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: String,
    pub artifact: agents::Artifact,
    pub request: Request,
    pub answer: Answer,
    pub lead: String,
    pub assignment: String,
    pub author: agents::Anchor,
    pub policy_sha256: String,
    pub playbook: agents::Artifact,
    pub expense_reference: String,
    pub recorded_at: u64,
}
impl Record {
    fn content_sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(&(
                &self.request,
                &self.answer,
                &self.lead,
                &self.assignment,
                &self.author,
                &self.policy_sha256,
                &self.playbook,
                &self.expense_reference,
                self.recorded_at,
            ))
            .map_err(|e| e.to_string())?,
        ))
    }
    pub(super) fn check(&self, key: &str) -> Result<()> {
        if self.schema != SCHEMA
            || key != self.artifact.reference
            || !key.starts_with(PREFIX)
            || self.answer.outbound_authority
            || self.artifact.sha256 != self.content_sha256()?
            || self.request.claims.is_empty()
            || self.request.claims.len() > 8
        {
            return Err("sales helper record identity or bounds disagree".into());
        }
        Ok(())
    }
}
/// Publish this exact zero-cost source in the shared model policy before use.
pub fn source(query: Query, recipient: &str) -> expenses::Source {
    expenses::Source {
        basis: expenses::Basis::LocalDeterministic,
        kind: query.kind(),
        source_revision: digest(SCHEMA.as_bytes()),
        price_revision: digest(b"local-reviewed-claim-read-zero-model-cost.v1"),
        recipient: recipient.into(),
        max_input_bytes: 4096,
        max_output_tokens: 8192,
        max_attempts: 1,
        max_elapsed_secs: 30,
        input_usd_millionths_per_million: 0,
        output_usd_millionths_per_million: 0,
    }
}
struct Reader<'a> {
    root: PathBuf,
    owner: &'a Access,
    agent: &'a agents::AgentAccess,
    source: expenses::Source,
    clock: fn() -> u64,
    keys: std::sync::Arc<dyn crate::task::agent_key::KeyStore>,
}
impl expenses::Adapter for Reader<'_> {
    type Output = Answer;
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn execute(&mut self, bytes: &[u8], caps: &expenses::Source) -> Result<Answer> {
        if caps != &self.source || bytes.len() > caps.max_input_bytes as usize {
            return Err("sales claim helper source or input changed".into());
        }
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid sales helper request")?;
        if request.query.kind() != caps.kind {
            return Err("sales helper kind changed".into());
        }
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.keys.clone();
        store.read_sales_agent(self.agent)?;
        let answer = store.claim_helper_answer(self.owner, &request)?;
        store.read_sales_agent(self.agent)?;
        if serde_json::to_vec(&answer)
            .map_err(|e| e.to_string())?
            .len()
            > caps.max_output_tokens as usize
        {
            return Err("sales helper answer exceeds its output bound".into());
        }
        Ok(answer)
    }
}
impl Store {
    pub(crate) fn claim_helper_answer(
        &mut self,
        owner: &Access,
        request: &Request,
    ) -> Result<Answer> {
        self.admin(owner)?;
        self.current_helper_answer(request)
    }
    pub(crate) fn current_helper_answer(&self, request: &Request) -> Result<Answer> {
        let views = self.current_claim_views(&request.claims, &request.release)?;
        let mut citations = Vec::new();
        let mut clauses = Vec::new();
        let mut accepted = true;
        for view in &views {
            let Some(record) = self.state.claims.claims.get(&view.pin.key()?) else {
                accepted = false;
                continue;
            };
            let Some(source) = self.state.claims.sources.get(&record.input.source.key()?) else {
                accepted = false;
                continue;
            };
            let evidence_sha256 = match &source.input.evidence {
                Evidence::Capability { contract, .. } => vec![contract.sha256.clone()],
                Evidence::RetailPrice { book, .. } => vec![book.sha256.clone()],
                Evidence::PilotOffer { template } => vec![template.sha256.clone()],
                Evidence::Comparison {
                    manifest,
                    public_review,
                    report_sha256,
                } => vec![
                    manifest.sha256.clone(),
                    public_review.sha256.clone(),
                    report_sha256.clone(),
                ],
            };
            let Some(review_sha256) = view.reviewed_sha256.as_ref() else {
                accepted = false;
                continue;
            };
            citations.push(Citation {
                claim: view.pin.clone(),
                source: record.input.source.clone(),
                claim_review_sha256: review_sha256.clone(),
                source_input_sha256: source.input_sha256.clone(),
                source_review_sha256: source.input.review.sha256.clone(),
                evidence_sha256,
                reviewer: record.owner.clone(),
            });
            match &view.verdict {
                Verdict::Allowed { claim }
                    if request.query != Query::CurrentPrices || claim.price.is_some() =>
                {
                    clauses.push(serde_json::to_string_pretty(claim).map_err(|e| e.to_string())?)
                }
                _ => accepted = false,
            }
        }
        let body = clauses.join("\n\n");
        if body.len() > 4096 {
            accepted = false;
        }
        Ok(Answer {
            views,
            citations,
            draft_body: accepted.then_some(body),
            recommendation: if accepted {
                Recommendation::OwnerReviewRequired
            } else {
                Recommendation::ReturnForReview
            },
            outbound_authority: false,
        })
    }
    /// Consumes the store so no host lock spans helper execution. All calls,
    /// including unknown answers, use the same admitted expense ledger.
    pub fn run_sales_claim_helper(
        mut self,
        owner: &Access,
        agent: &agents::AgentAccess,
        request: &Request,
        request_id: &str,
    ) -> Result<Record> {
        self.admin(owner)?;
        hash(&request.release, 40)?;
        id(request_id)?;
        if request.claims.is_empty() || request.claims.len() > 8 {
            return Err("sales helper needs 1 to 8 distinct claim pins".into());
        }
        let mut seen = BTreeSet::new();
        for pin in &request.claims {
            if !seen.insert(pin.key()?) {
                return Err("sales helper repeats a claim revision".into());
            }
        }
        let context = self.read_sales_agent(agent)?;
        let playbook = self
            .state
            .agents
            .policies
            .get(
                &self.state.leads[&context.lead].agent_records.assignments[&context.assignment]
                    .policy_sha256,
            )
            .ok_or("sales helper policy is unavailable")?
            .policy
            .playbook
            .clone();
        let policy_sha256 = self.state.leads[&context.lead].agent_records.assignments
            [&context.assignment]
            .policy_sha256
            .clone();
        let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let source = source(request.query, &format!("human:{}", owner.principal()));
        let admission = self.reserve_sales_model(
            agent,
            &expenses::Input {
                request: request_id.into(),
                attempt: 1,
                source: source.clone(),
                input_bytes: bytes.len() as u64,
                input_sha256: digest(&bytes),
            },
        )?;
        let root = self
            .dir
            .parent()
            .ok_or("sales host root is unavailable")?
            .to_path_buf();
        let clock = self.clock;
        let keys = self.native_keys.clone();
        drop(self);
        let mut reader = Reader {
            root: root.clone(),
            owner,
            agent,
            source,
            clock,
            keys: keys.clone(),
        };
        let (answer, executed) = admission.execute(&bytes, &mut reader)?;
        let mut store = Store::open_with_clock(&root, clock)?;
        store.native_keys = keys;
        let current = store.read_sales_agent(agent)?;
        if current.assignment != context.assignment || current.agent != context.agent {
            return Err("sales helper assignment changed before retention".into());
        }
        let mut record = Record {
            schema: SCHEMA.into(),
            artifact: agents::Artifact {
                reference: format!("{PREFIX}{}", &random_token()[..48]),
                sha256: String::new(),
            },
            request: request.clone(),
            answer,
            lead: context.lead,
            assignment: context.assignment,
            author: context.agent,
            policy_sha256,
            playbook,
            expense_reference: executed.receipt().id.clone(),
            recorded_at: clock(),
        };
        record.artifact.sha256 = record.content_sha256()?;
        let evidence = record.artifact.sha256.clone();
        store.settle_sales_model(
            owner,
            &record.expense_reference,
            &expenses::Settlement {
                request: digest(format!("helper-settlement:{request_id}").as_bytes()),
                estimated_usd_millionths: Some(0),
                billed_usd_millionths: None,
                evidence_sha256: evidence,
            },
        )?;
        if store.state.claims.helpers.len() >= MAX_REVISIONS {
            return Err("sales helper history is full".into());
        }
        let mut next = store.state.clone();
        next.claims
            .helpers
            .insert(record.artifact.reference.clone(), record.clone());
        store.persist(next)?;
        drop(executed);
        Ok(record)
    }
    #[cfg(test)]
    pub(in crate::task::sales) fn validate_sales_helper_artifacts(
        &self,
        lead: &str,
        assignment: &str,
        refs: &[agents::Artifact],
        recommendation: Option<&agents::Artifact>,
        body: &str,
    ) -> Result<()> {
        if !refs
            .iter()
            .chain(recommendation)
            .any(|r| r.reference.starts_with(PREFIX))
        {
            return Ok(());
        }
        let grant = self
            .state
            .leads
            .get(lead)
            .and_then(|value| value.agent_records.assignments.get(assignment))
            .ok_or("sales helper assignment is unavailable")?;
        let native = agents::native::Native::read(
            self.dir.parent().ok_or("sales host root is unavailable")?,
            &grant.anchor.name,
            (self.clock)(),
            self.native_keys.clone(),
        )?;
        self.validate_sales_helper_artifacts_with_native(
            lead,
            assignment,
            refs,
            recommendation,
            body,
            &native,
        )
    }
    pub(in crate::task::sales) fn validate_sales_helper_artifacts_with_native(
        &self,
        lead: &str,
        assignment: &str,
        refs: &[agents::Artifact],
        recommendation: Option<&agents::Artifact>,
        body: &str,
        native: &agents::native::Native,
    ) -> Result<()> {
        for artifact in refs.iter().chain(recommendation) {
            if !artifact.reference.starts_with(PREFIX) {
                continue;
            }
            let record = self
                .state
                .claims
                .helpers
                .get(&artifact.reference)
                .ok_or("sales helper reference is unavailable")?;
            record.check(&artifact.reference)?;
            let lead_record = self
                .state
                .leads
                .get(lead)
                .ok_or("sales helper lead is unavailable")?;
            let grant = lead_record
                .agent_records
                .assignments
                .get(assignment)
                .ok_or("sales helper assignment is unavailable")?;
            let policy = self
                .state
                .agents
                .policies
                .get(&grant.policy_sha256)
                .ok_or("sales helper policy is unavailable")?;
            if native.anchor != record.author
                || policy.revoked_at.is_some()
                || policy.policy.expires_at <= (self.clock)()
                || self.state.agents.current.get(&policy.policy.id) != Some(&grant.policy_sha256)
                || policy.policy.playbook != record.playbook
            {
                return Err("sales helper native identity or policy changed".into());
            }
            native.recheck()?;
            let current = self.current_helper_answer(&record.request)?;
            let expense = self
                .state
                .expenses
                .reservation(&record.expense_reference)
                .ok_or("sales helper original expense is unavailable")?;
            if artifact.sha256 != record.artifact.sha256
                || record.lead != lead
                || record.assignment != assignment
                || record.author != grant.anchor
                || record.policy_sha256 != grant.policy_sha256
                || !grant.active
                || grant.expires_at <= (self.clock)()
                || current.recommendation != Recommendation::OwnerReviewRequired
                || current.draft_body.as_deref() != Some(body)
                || serde_json::to_vec(&current).map_err(|e| e.to_string())?
                    != serde_json::to_vec(&record.answer).map_err(|e| e.to_string())?
                || expense.status != expenses::Status::Known
                || expense.execution_unknown
                || expense.native != record.author
                || expense.lead != record.lead
                || expense.assignment != record.assignment
                || expense.input.input_sha256
                    != digest(&serde_json::to_vec(&record.request).map_err(|e| e.to_string())?)
                || expense.input.source
                    != source(record.request.query, &expense.input.source.recipient)
                || expense.settlements.last().is_none_or(|s| {
                    s.estimated_usd_millionths != Some(0)
                        || s.billed_usd_millionths.is_some()
                        || s.evidence_sha256 != record.artifact.sha256
                })
            {
                return Err("sales helper evidence, price, assignment, or original expense changed; return the draft for review".into());
            }
        }
        Ok(())
    }
    pub fn sales_claim_helper(&mut self, owner: &Access, reference: &str) -> Result<Record> {
        self.refresh()?;
        self.admin(owner)?;
        self.state
            .claims
            .helpers
            .get(reference)
            .cloned()
            .ok_or("sales helper result is unavailable".into())
    }
}
