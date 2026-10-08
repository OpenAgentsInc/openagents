//! Paul's bounded native sales queue. Canonical records supply facts; provider
//! absence, unsupported claims, and missing qualification remain visible.
use super::*;
use crate::task::{agent, agent_steer};
use agents::{AgentAccess, Anchor};
use coder_host::access::crew::JobRole;

pub mod steering;

pub const SCHEMA: &str = "openagents.paul-sales-control.v1";
const MAX_ASSIGNMENTS: usize = 16;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: String,
    pub revision: u64,
    pub anchor: Anchor,
    pub owner_credential: PathBuf,
    pub assignments: Vec<PathBuf>,
    pub permitted_requesters: Vec<String>,
}
impl Binding {
    pub fn sha256(&self) -> Result<String> {
        if self.schema != SCHEMA
            || self.revision == 0
            || self.anchor.name != "paul"
            || self.assignments.len() > MAX_ASSIGNMENTS
            || self.permitted_requesters.is_empty()
            || self.permitted_requesters.len() > 8
            || !self.owner_credential.is_absolute()
            || self.assignments.iter().any(|p| !p.is_absolute())
        {
            return Err("Paul binding needs explicit private credentials, current native identity, and named requesters".into());
        }
        for requester in &self.permitted_requesters {
            id(requester)?;
        }
        if self
            .assignments
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.assignments.len()
            || self
                .permitted_requesters
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.permitted_requesters.len()
        {
            return Err("Paul binding repeats an assignment or requester".into());
        }
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "Paul binding serialization failed")?,
        ))
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    binding: Option<Binding>,
    owner: Option<String>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        match (&self.binding, &self.owner) {
            (None, None) => Ok(()),
            (Some(b), Some(o)) => {
                b.sha256()?;
                id(o)
            }
            _ => Err("Paul controller ownership is incomplete".into()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeadRef {
    pub lead: String,
    pub revision: u64,
    pub assignment: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct QueueRow {
    pub original: LeadRef,
    pub recorded_stage: Stage,
    pub pending_drafts: usize,
    pub meetings: Vec<meetings::AgentMeeting>,
    pub policy_sha256: String,
    pub playbook: agents::Artifact,
    pub earned_revenue: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Pipeline {
    pub schema: String,
    pub binding_sha256: String,
    pub rows: Vec<QueueRow>,
    pub idle: bool,
    pub model_available: bool,
    pub qualification_inferred: bool,
    pub external_effects: bool,
    pub coder_session: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    request: String,
    binding_sha256: String,
    expected: Vec<LeadRef>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Answer {
    pub pipeline: Pipeline,
    pub evidence_sha256: String,
    pub expense: Option<expenses::Reservation>,
    pub reply: String,
    pub headline: String,
}
/// This bounded native verification source makes no model call and has no bill.
pub fn source() -> expenses::Source {
    expenses::Source {
        basis: expenses::Basis::LocalDeterministic,
        kind: expenses::Kind::Verification,
        source_revision: digest(b"openagents.paul.native-pipeline.v1"),
        price_revision: digest(b"native-verification-no-model-no-bill.v1"),
        recipient: "human:operator".into(),
        max_input_bytes: 8192,
        max_output_tokens: 20_480,
        max_attempts: 1,
        max_elapsed_secs: 20,
        input_usd_millionths_per_million: 0,
        output_usd_millionths_per_million: 0,
    }
}
impl Store {
    fn paul_owner(&mut self, binding: &Binding) -> Result<Access> {
        let owner = self.authenticate(&Self::read_credential(&binding.owner_credential)?)?;
        self.admin(&owner)?;
        if self
            .state
            .paul
            .owner
            .as_deref()
            .is_some_and(|p| p != owner.principal)
        {
            return Err("Paul's original controller owner changed".into());
        }
        if self.sales_agent_anchor(&owner, "paul")? != binding.anchor {
            return Err("Paul's current key or charter changed".into());
        }
        let root = self.dir.parent().ok_or("sales host root is unavailable")?;
        let native = agent::Store::with_keys(root, "paul", self.native_keys.clone())?;
        let record = native.load()?.ok_or("Paul is unavailable")?;
        crate::task::agent_memory::Memory::new(native.clone(), secret_screen::Screen::host())
            .entries()
            .map_err(|_| "Paul memory is unreadable; repair it before resuming sales controls")?;
        if record.job_role != Some(JobRole::SalesLead)
            || record.state != agent::State::Active
            || record.crew_charter.as_ref().is_none_or(|c| !c.drafting)
        {
            return Err("Paul's native sales role is stopped, paused, or unavailable".into());
        }
        Ok(owner)
    }
    fn paul_assignments(&mut self, binding: &Binding) -> Result<Vec<AgentAccess>> {
        let mut result = Vec::new();
        let mut leads = std::collections::BTreeSet::new();
        for path in &binding.assignments {
            let access = self.authenticate_sales_agent(&Self::read_credential(path)?)?;
            let (lead, _, _, native) = self.checked_sales_agent(&access)?;
            if native.anchor != binding.anchor || !leads.insert(lead.id.clone()) {
                return Err("Paul binding includes another member or repeats a lead".into());
            }
            result.push(access);
        }
        Ok(result)
    }
    pub fn configure_paul(
        &mut self,
        owner: &Access,
        binding: &Binding,
        approval: &str,
    ) -> Result<String> {
        self.refresh()?;
        self.admin(owner)?;
        let sha = binding.sha256()?;
        if sha != approval
            || binding.revision
                != self
                    .state
                    .paul
                    .binding
                    .as_ref()
                    .map(|b| {
                        b.revision
                            .checked_add(1)
                            .ok_or("Paul binding revision overflow")
                    })
                    .transpose()?
                    .unwrap_or(1)
        {
            return Err("approve the exact next Paul binding revision".into());
        }
        let original = self.paul_owner(binding)?;
        if original.principal != owner.principal {
            return Err("Paul binding owner credential does not match the caller".into());
        }
        self.paul_assignments(binding)?;
        let mut next = self.state.clone();
        next.paul.binding = Some(binding.clone());
        next.paul.owner = Some(owner.principal.clone());
        self.persist(next)?;
        Ok(sha)
    }
    fn paul_pipeline(
        &mut self,
        binding: &Binding,
        expected: Option<&[LeadRef]>,
    ) -> Result<Pipeline> {
        self.refresh()?;
        self.paul_owner(binding)?;
        if self
            .state
            .paul
            .binding
            .as_ref()
            .map(Binding::sha256)
            .transpose()?
            .as_deref()
            != Some(binding.sha256()?.as_str())
        {
            return Err("Paul binding changed".into());
        }
        let accesses = self.paul_assignments(binding)?;
        let mut rows = Vec::new();
        for access in &accesses {
            let context = self.read_sales_agent(access)?;
            let (_, grant, policy, _) = self.checked_sales_agent(access)?;
            let policy_sha256 = grant.policy_sha256.clone();
            let playbook = policy.playbook.clone();
            rows.push(QueueRow {
                original: LeadRef {
                    lead: context.lead,
                    revision: context.revision,
                    assignment: context.assignment,
                },
                recorded_stage: context.stage,
                pending_drafts: context
                    .drafts
                    .iter()
                    .filter(|d| d.state == agents::DraftState::Proposed)
                    .count(),
                meetings: self.sales_agent_meetings(access)?,
                policy_sha256,
                playbook,
                earned_revenue: false,
            });
        }
        if let Some(expected) = expected {
            if serde_json::to_vec(expected).map_err(|_| "Paul scope serialization failed")?
                != serde_json::to_vec(&rows.iter().map(|r| &r.original).collect::<Vec<_>>())
                    .map_err(|_| "Paul scope serialization failed")?
            {
                return Err("Paul's current admitted assignments or lead revisions changed".into());
            }
        }
        Ok(Pipeline {
            schema: SCHEMA.into(),
            binding_sha256: binding.sha256()?,
            idle: rows.is_empty(),
            rows,
            model_available: false,
            qualification_inferred: false,
            external_effects: false,
            coder_session: "paul-coder".into(),
        })
    }
    /// Read the current private queue under the original sales-owner authority.
    /// This creates no reservation and remains readable during expense holds.
    pub fn read_paul_pipeline(&mut self, owner: &Access) -> Result<Pipeline> {
        self.refresh()?;
        self.admin(owner)?;
        if self.state.paul.owner.as_deref() != Some(owner.principal.as_str()) {
            return Err("Paul queue read requires the original configured sales owner".into());
        }
        let binding = self
            .state
            .paul
            .binding
            .clone()
            .ok_or("Paul controller is unconfigured")?;
        let current = self.paul_owner(&binding)?;
        if current.principal != owner.principal {
            return Err("Paul original controller ownership changed".into());
        }
        self.paul_pipeline(&binding, None)
    }

    /// Consume the canonical store before native helper execution; no lock spans
    /// the adapter. The body contains opaque request and original revision pins.
    pub fn ask_paul_pipeline(mut self, requester: &str, request: &str) -> Result<Answer> {
        id(request)?;
        self.refresh()?;
        let binding = self
            .state
            .paul
            .binding
            .clone()
            .ok_or("Paul has no owner-approved sales binding")?;
        if !binding.permitted_requesters.iter().any(|p| p == requester) {
            return Err("Paul's sales controller requester is not admitted".into());
        }
        let pipeline = self.paul_pipeline(&binding, None)?;
        let root = self
            .dir
            .parent()
            .ok_or("sales host root is unavailable")?
            .to_path_buf();
        let clock = self.clock;
        let keys = self.native_keys.clone();
        let owner = self.paul_owner(&binding)?;
        let accesses = self.paul_assignments(&binding)?;
        let query = Query {
            request: request.into(),
            binding_sha256: binding.sha256()?,
            expected: pipeline.rows.iter().map(|r| r.original.clone()).collect(),
        };
        let body = serde_json::to_vec(&query).map_err(|_| "Paul request serialization failed")?;
        let mut declared = source();
        declared.recipient = format!("human:{}", owner.principal);
        if accesses.is_empty() {
            return pipeline_answer(pipeline, None);
        }
        let input = expenses::Input {
            request: digest(format!("paul-request:{request}").as_bytes()),
            attempt: 1,
            source: declared.clone(),
            input_bytes: body.len() as u64,
            input_sha256: digest(&body),
        };
        let admission = self.reserve_sales_model(&accesses[0], &input)?;
        drop(self);
        let mut adapter = NativePipeline {
            root: root.clone(),
            clock,
            keys: keys.clone(),
            binding: binding.clone(),
            source: declared,
        };
        let (pipeline, execution) = admission.execute(&body, &mut adapter)?;
        let mut store = Self::open_with_clock(&root, clock)?;
        store.native_keys = keys;
        store.paul_pipeline(&binding, Some(&query.expected))?;
        let owner = store.paul_owner(&binding)?;
        let receipt = store.settle_sales_model(
            &owner,
            &execution.receipt().id,
            &expenses::Settlement {
                request: digest(format!("paul-result:{request}").as_bytes()),
                estimated_usd_millionths: Some(0),
                billed_usd_millionths: None,
                evidence_sha256: digest(
                    &serde_json::to_vec(&pipeline)
                        .map_err(|_| "Paul pipeline serialization failed")?,
                ),
            },
        )?;
        drop(execution);
        pipeline_answer(pipeline, Some(receipt))
    }
}
struct NativePipeline {
    root: PathBuf,
    clock: fn() -> u64,
    keys: std::sync::Arc<dyn crate::task::agent_key::KeyStore>,
    binding: Binding,
    source: expenses::Source,
}
impl expenses::Adapter for NativePipeline {
    type Output = Pipeline;
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn execute(&mut self, body: &[u8], caps: &expenses::Source) -> Result<Pipeline> {
        if caps != &self.source || body.len() as u64 > caps.max_input_bytes {
            return Err("Paul verification source or byte cap changed".into());
        }
        let query: Query =
            serde_json::from_slice(body).map_err(|_| "Paul request shape changed")?;
        if query.binding_sha256 != self.binding.sha256()? {
            return Err("Paul input binding changed".into());
        }
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.keys.clone();
        store.paul_pipeline(&self.binding, Some(&query.expected))
    }
}
fn pipeline_answer(pipeline: Pipeline, expense: Option<expenses::Reservation>) -> Result<Answer> {
    let evidence_sha256 =
        digest(&serde_json::to_vec(&pipeline).map_err(|_| "Paul pipeline serialization failed")?);
    let reply = if pipeline.idle {
        "The admitted sales queue is empty. I'm idle.".into()
    } else {
        format!(
            "The canonical queue has {} currently admitted lead records. Stages are owner-recorded; qualification and earned revenue are not inferred. General model work remains unavailable.",
            pipeline.rows.len()
        )
    };
    Ok(Answer {
        pipeline,
        evidence_sha256,
        expense,
        reply,
        headline: "canonical sales evidence".into(),
    })
}

/// Route the verified queue through the shared steering loop. This planner
/// selects one typed native read; it neither calls a model nor prompts Coder.
/// General work needs a separately admitted provider and priced reservation.
pub fn steer_pipeline(
    record: &agent::Record,
    answer: &Answer,
    hands: &mut dyn agent_steer::Hands,
) -> agent_steer::Steered {
    struct NativePlanner {
        reply: String,
    }
    impl agent_steer::Planner for NativePlanner {
        fn plan(
            &mut self,
            _: &agent_steer::Ask,
        ) -> Result<(agent_steer::Plan, agent_steer::Spent)> {
            Ok((
                agent_steer::Plan {
                    understanding: "Read the currently admitted canonical sales queue".into(),
                    answer_directly: true,
                    reply_if_direct: Some(self.reply.clone()),
                    steps: vec![],
                    verify: None,
                },
                agent_steer::Spent {
                    model: String::new(),
                    usd: Some(0.0),
                    calls: 0,
                    ..Default::default()
                },
            ))
        }
        fn report(&mut self, _: &agent_steer::Ask) -> Result<Option<(String, agent_steer::Spent)>> {
            Ok(None)
        }
    }
    let mut mind = agent_steer::Mind {
        planner: Box::new(NativePlanner {
            reply: answer.reply.clone(),
        }),
        judge: None,
        unjudged: "native canonical verification makes no inference call".into(),
    };
    let policy = agent_steer::Policy {
        schema: agent_steer::POLICY_SCHEMA.into(),
        rules: vec![],
        computers: Vec::new(),
    };
    let mut result = agent_steer::run(
        hands,
        &mut mind,
        &agent_steer::Input {
            record,
            request: "sales pipeline",
            briefing: &answer.evidence_sha256,
            core: None,
            cwd: "",
            policy: &policy,
            note: None,
        },
    );
    // A completed read establishes only a queue snapshot, never an accomplished
    // sale, qualified draft, model availability, or successful Coder task.
    result.report.headline = if answer.pipeline.idle {
        "sales queue idle".into()
    } else {
        "sales queue checked; models unavailable".into()
    };
    result
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchRequest {
    pub lead: String,
    pub helper: claims::helpers::Request,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResearchAnswer {
    pub binding_sha256: String,
    pub original: LeadRef,
    pub helper: claims::helpers::Record,
    pub factual_model_answer: bool,
    pub outbound_authority: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct PracticeRef {
    pub id: String,
    pub original_sha256: String,
    pub persona_sha256: String,
    pub stop: training::Stop,
    pub original_expenses: Vec<String>,
    pub certification_inferred: bool,
}
impl Store {
    fn paul_binding_for(&mut self, requester: &str) -> Result<Binding> {
        self.refresh()?;
        let binding = self
            .state
            .paul
            .binding
            .clone()
            .ok_or("Paul has no owner-approved sales binding")?;
        if !binding.permitted_requesters.iter().any(|p| p == requester) {
            return Err("Paul's sales controller requester is not admitted".into());
        }
        self.paul_pipeline(&binding, None)?;
        Ok(binding)
    }
    /// Research selects only a current assignment and exact reviewed claims.
    /// Free-form customer text never reaches a factual model answer.
    pub fn ask_paul_research(
        mut self,
        requester: &str,
        request_id: &str,
        request: &ResearchRequest,
    ) -> Result<ResearchAnswer> {
        id(request_id)?;
        let binding = self.paul_binding_for(requester)?;
        let owner = self.paul_owner(&binding)?;
        let pipeline = self.paul_pipeline(&binding, None)?;
        let original = pipeline
            .rows
            .iter()
            .find(|r| r.original.lead == request.lead)
            .ok_or("Paul research lead is outside current admitted assignments")?
            .original
            .clone();
        let mut selected = None;
        for access in self.paul_assignments(&binding)? {
            if self.read_sales_agent(&access)?.lead == request.lead {
                selected = Some(access);
                break;
            }
        }
        let access = selected.ok_or("Paul research assignment changed")?;
        let root = self
            .dir
            .parent()
            .ok_or("sales host root is unavailable")?
            .to_path_buf();
        let clock = self.clock;
        let keys = self.native_keys.clone();
        let helper = self.run_sales_claim_helper(
            &owner,
            &access,
            &request.helper,
            &digest(format!("paul-research:{request_id}").as_bytes()),
        )?;
        let mut current = Store::open_with_clock(&root, clock)?;
        current.native_keys = keys;
        let binding_now = current.paul_binding_for(requester)?;
        if binding_now.sha256()? != binding.sha256()? {
            return Err("Paul controller changed before retaining research".into());
        }
        let current_pipeline = current.paul_pipeline(
            &binding,
            Some(
                &pipeline
                    .rows
                    .iter()
                    .map(|r| r.original.clone())
                    .collect::<Vec<_>>(),
            ),
        )?;
        if current_pipeline
            .rows
            .iter()
            .all(|r| r.original.lead != original.lead)
        {
            return Err("Paul research assignment changed".into());
        }
        Ok(ResearchAnswer {
            binding_sha256: binding.sha256()?,
            original,
            helper,
            factual_model_answer: false,
            outbound_authority: false,
        })
    }
    /// Read only original opaque synthetic run and reservation references. This
    /// does not schedule practice, create customers, or award certification.
    pub fn ask_paul_practice(&mut self, requester: &str) -> Result<Vec<PracticeRef>> {
        let binding = self.paul_binding_for(requester)?;
        let owner = self.paul_owner(&binding)?;
        let runs = self.sales_roleplay_schedule(&owner, &binding.anchor, None, 64)?;
        let output = runs
            .into_iter()
            .filter(|r| r.schedule.agent == binding.anchor)
            .map(|r| PracticeRef {
                id: r.schedule.id,
                original_sha256: r.schedule_sha256,
                persona_sha256: r.persona_sha256,
                stop: r.stop,
                original_expenses: r.attempts,
                certification_inferred: false,
            })
            .collect();
        self.paul_binding_for(requester)?;
        Ok(output)
    }
}

impl Store {
    /// This explicit owner control reviews a canonical draft. It grants Paul
    /// no approval capability and accepts no other administrative operation.
    pub fn review_paul_draft(&mut self, owner: &Access, bytes: &[u8]) -> Result<Receipt> {
        let command: agents::OwnerCommand = serde_json::from_slice(bytes)
            .map_err(|_| "Paul draft review requires an exact private owner command")?;
        if !matches!(
            command.operation,
            agents::OwnerOperation::ReviewDraft { .. }
        ) {
            return Err("Paul owner draft review accepts no other administrative operation".into());
        }
        self.apply_sales_agent_owner(owner, bytes, None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftRequest {
    pub lead: String,
    pub expected_lead_revision: u64,
    pub helper_reference: String,
}
impl Store {
    /// Propose the exact current reviewed helper body under Paul's original
    /// assignment. Model prose and caller-supplied bodies confer no authority.
    pub fn ask_paul_draft(
        &mut self,
        requester: &str,
        request_id: &str,
        request: &DraftRequest,
    ) -> Result<Receipt> {
        id(request_id)?;
        text(&request.lead, 160)?;
        text(&request.helper_reference, 160)?;
        let binding = self.paul_binding_for(requester)?;
        let owner = self.paul_owner(&binding)?;
        self.paul_pipeline(&binding, None)?;
        let mut selected = None;
        for access in self.paul_assignments(&binding)? {
            if self.read_sales_agent(&access)?.lead == request.lead {
                selected = Some(access);
                break;
            }
        }
        let access = selected.ok_or("Paul draft requires a current admitted assignment")?;
        let helper = self.sales_claim_helper(&owner, &request.helper_reference)?;
        if helper.lead != request.lead
            || helper.author != binding.anchor
            || helper.answer.recommendation != claims::helpers::Recommendation::OwnerReviewRequired
            || helper.answer.citations.is_empty()
        {
            return Err("Paul draft requires original current reviewed helper evidence".into());
        }
        let body = helper
            .answer
            .draft_body
            .clone()
            .ok_or("The reviewed helper did not produce a draft body")?;
        let command = agents::AgentCommand {
            schema: agents::AGENT_COMMAND_SCHEMA.into(),
            id: digest(format!("paul-draft:{request_id}").as_bytes()),
            expected_lead_revision: request.expected_lead_revision,
            operation: agents::AgentOperation::ProposeDraft {
                body,
                template: helper.artifact.clone(),
                check_refs: vec![helper.artifact.clone()],
                recommendation: Some(helper.artifact),
            },
        };
        let bytes = serde_json::to_vec(&command)
            .map_err(|_| "Paul reviewed draft command serialization failed")?;
        // The canonical agent mutation verifies original helper expenses and
        // the current measured qualification gate. It grants no send or approval.
        self.apply_sales_agent(&access, &bytes)
    }
}
