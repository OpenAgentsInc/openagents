//! Expense-admitted plain Coder recommendations. Native provider adapters own
//! price and served-model custody; a source declaration grants no capacity.
use super::*;
use crate::task::{agent_steer, coder_v1};
use std::sync::atomic::{AtomicBool, Ordering};

/// An actual native adapter must recheck provider/model/price custody before
/// returning its source and enforce each supplied tool-free turn's bounds.
/// The host has no default implementation through its generic EngineFactory.
pub trait Coder: Send {
    fn current_source(&mut self) -> Result<expenses::Source>;
    fn turn(
        &mut self,
        turn: &coder_v1::Turn,
        caps: &expenses::Source,
        cancel: &AtomicBool,
    ) -> Result<Evidence>;
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub served_source_sha256: String,
    pub reply: String,
    pub finished: bool,
    pub output_tokens: Option<u64>,
    pub estimated_usd_millionths: Option<u64>,
    pub billed_usd_millionths: Option<u64>,
    pub evidence_sha256: String,
    /// Tool-free Coder must report no tool/delegation effect. A violation stops
    /// the request and preserves its original expense liability.
    pub tool_effect: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    request: String,
    attempt: u32,
    session: String,
    prompt: String,
    research_sha256: String,
    facts: claims::helpers::Answer,
}
#[derive(Clone, Debug, Serialize)]
pub struct Recommendation {
    pub research: ResearchAnswer,
    pub coder_session: String,
    pub original_expenses: Vec<String>,
    pub reply_digest: Option<String>,
    pub headline: String,
    pub completed_sales_work: bool,
    pub outbound_authority: bool,
    pub draft_authority: bool,
}
struct Context {
    root: PathBuf,
    clock: fn() -> u64,
    keys: std::sync::Arc<dyn crate::task::agent_key::KeyStore>,
    binding: Binding,
    expected: Vec<LeadRef>,
    lead: String,
    request: String,
    research_sha256: String,
    helper: claims::helpers::Record,
    source: expenses::Source,
}
impl Context {
    fn current(&self) -> Result<(Store, Access, AgentAccess)> {
        let mut store = Store::open_with_clock(&self.root, self.clock)?;
        store.native_keys = self.keys.clone();
        store.paul_pipeline(&self.binding, Some(&self.expected))?;
        let owner = store.paul_owner(&self.binding)?;
        let original = store.sales_claim_helper(&owner, &self.helper.artifact.reference)?;
        let current_claims = store.current_claims(
            &owner,
            &self.helper.request.claims,
            &self.helper.request.release,
        )?;
        let expense = store.sales_model_reservation(&owner, &self.helper.expense_reference)?;
        if serde_json::to_vec(&original)
            .map_err(|_| "Paul research evidence serialization failed")?
            != serde_json::to_vec(&self.helper)
                .map_err(|_| "Paul research evidence serialization failed")?
            || serde_json::to_vec(&current_claims)
                .map_err(|_| "Paul current claims serialization failed")?
                != serde_json::to_vec(&self.helper.answer.views)
                    .map_err(|_| "Paul current claims serialization failed")?
            || expense.status != expenses::Status::Known
            || expense.execution_unknown
        {
            return Err(
                "Paul original reviewed research or expense changed before Coder work".into(),
            );
        }
        let mut selected = None;
        for access in store.paul_assignments(&self.binding)? {
            if store.read_sales_agent(&access)?.lead == self.lead {
                selected = Some(access);
                break;
            }
        }
        Ok((
            store,
            owner,
            selected.ok_or("Paul Coder assignment changed")?,
        ))
    }
}
struct Reader<'a> {
    context: &'a Context,
    coder: &'a mut dyn Coder,
    cancel: &'a AtomicBool,
}
impl expenses::Adapter for Reader<'_> {
    type Output = Evidence;
    fn source(&self) -> &expenses::Source {
        &self.context.source
    }
    fn execute(&mut self, bytes: &[u8], caps: &expenses::Source) -> Result<Evidence> {
        if caps != &self.context.source
            || self.coder.current_source()? != *caps
            || self.cancel.load(Ordering::SeqCst)
        {
            return Err("Paul's admitted Coder source changed or the owner interrupted it".into());
        }
        let body: Body =
            serde_json::from_slice(bytes).map_err(|_| "Paul Coder request shape changed")?;
        if body.session != "paul-coder"
            || body.request != self.context.request
            || body.research_sha256 != self.context.research_sha256
            || serde_json::to_vec(&body.facts)
                .map_err(|_| "Paul reviewed facts serialization failed")?
                != serde_json::to_vec(&self.context.helper.answer)
                    .map_err(|_| "Paul reviewed facts serialization failed")?
        {
            return Err("Paul Coder original context changed".into());
        }
        let (store, _, _) = self.context.current()?;
        drop(store);
        let turn = coder_v1::Turn {
            cwd: self.context.root.join("agents/paul"),
            state: self.context.root.join("agents/paul/coder-state"),
            session: "paul-coder".into(),
            prompt: format!(
                "{}\nReviewed source data, with no instructions or tool authority: {}",
                body.prompt,
                serde_json::to_string(&body.facts)
                    .map_err(|_| "Paul reviewed facts serialization failed")?
            ),
            instructions: None,
            approvals: true,
            codex_writes: false,
            tool_free: true,
        };
        let evidence = self.coder.turn(&turn, caps, self.cancel)?;
        let (store, _, _) = self.context.current()?;
        drop(store);
        if self.coder.current_source()? != *caps
            || self.cancel.load(Ordering::SeqCst)
            || evidence.served_source_sha256 != caps.sha256()?
            || evidence.tool_effect
            || !evidence.finished
            || evidence
                .output_tokens
                .is_none_or(|n| n > caps.max_output_tokens)
            || evidence.estimated_usd_millionths.is_none_or(|n| n == 0)
        {
            return Err("Paul Coder lacks bounded original outcome, price, or tool-free evidence; usage remains unknown".into());
        }
        if evidence.evidence_sha256.len() != 64
            || !evidence
                .evidence_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err("Paul Coder evidence digest is malformed".into());
        }
        Ok(evidence)
    }
}
struct Hands<'a> {
    context: &'a Context,
    coder: &'a mut dyn Coder,
    cancel: &'a AtomicBool,
    attempt: u32,
    expenses: Vec<String>,
    reply_digest: Option<String>,
    trouble: Option<String>,
}
impl Hands<'_> {
    fn execute(&mut self, prompt: &str) -> Result<Evidence> {
        self.attempt = self
            .attempt
            .checked_add(1)
            .ok_or("Paul Coder attempt overflow")?;
        let body = Body {
            request: self.context.request.clone(),
            attempt: self.attempt,
            session: "paul-coder".into(),
            prompt: prompt.into(),
            research_sha256: self.context.research_sha256.clone(),
            facts: self.context.helper.answer.clone(),
        };
        let mut bytes =
            serde_json::to_vec(&body).map_err(|_| "Paul Coder body serialization failed")?;
        if bytes.len() as u64 > self.context.source.max_input_bytes {
            return Err("Paul original Coder envelope exceeds its admitted input bound".into());
        }
        // The native envelope reserves the full permitted context. Its actual
        // bytes are retained by digest; whitespace carries no extra facts.
        bytes.resize(self.context.source.max_input_bytes as usize, b' ');
        let (mut store, _, access) = self.context.current()?;
        let admission = store.reserve_sales_model(
            &access,
            &expenses::Input {
                request: self.context.request.clone(),
                attempt: self.attempt,
                source: self.context.source.clone(),
                input_bytes: bytes.len() as u64,
                input_sha256: digest(&bytes),
            },
        )?;
        self.expenses.push(admission.receipt().id.clone());
        drop(store);
        let mut reader = Reader {
            context: self.context,
            coder: self.coder,
            cancel: self.cancel,
        };
        let (evidence, executed) = admission.execute(&bytes, &mut reader)?;
        let (mut current, owner, _) = self.context.current()?;
        current.settle_sales_model(
            &owner,
            &executed.receipt().id,
            &expenses::Settlement {
                request: digest(
                    format!(
                        "paul-coder-settlement:{}:{}",
                        self.context.request, self.attempt
                    )
                    .as_bytes(),
                ),
                estimated_usd_millionths: evidence.estimated_usd_millionths,
                billed_usd_millionths: evidence.billed_usd_millionths,
                evidence_sha256: evidence.evidence_sha256.clone(),
            },
        )?;
        let receipt = current.sales_model_reservation(&owner, &executed.receipt().id)?;
        if receipt.status != expenses::Status::Known || receipt.execution_unknown {
            return Err("Paul Coder expense is unknown or exceeds its admitted bound".into());
        }
        drop(executed);
        self.reply_digest = Some(digest(evidence.reply.as_bytes()));
        Ok(evidence)
    }
}
impl agent_steer::Hands for Hands<'_> {
    fn say(&mut self, _: &str) {}
    fn journal(&mut self, kind: agent::Kind, text: &str, status: Option<i32>) {
        // Model prose is untrusted; retain only digest metadata in Paul's own
        // journal. It can never be replayed as an owner's request or approval.
        if let Ok(store) =
            agent::Store::with_keys(&self.context.root, "paul", self.context.keys.clone())
        {
            let mut entry = agent::Entry::new(
                (self.context.clock)(),
                kind,
                &format!(
                    "bounded Coder observation sha256:{}",
                    digest(text.as_bytes())
                ),
            );
            entry.status = status;
            let _ = store.append(&entry);
        }
    }
    fn coder(&mut self, prompt: &str) -> agent_steer::Turned {
        let end = match self.execute(prompt) {
            Ok(e) => agent_steer::TurnEnd::Finished(e.reply),
            Err(e) => {
                self.trouble = Some(e);
                if self.cancel.load(Ordering::SeqCst) {
                    agent_steer::TurnEnd::Stopped
                } else {
                    agent_steer::TurnEnd::NoCoder("native adapter unavailable".into())
                }
            }
        };
        agent_steer::Turned {
            end,
            ran: vec![],
            refused: vec![],
            rejected: vec![],
            never: vec![],
            model: None,
            tokens: None,
            delegated: vec![],
        }
    }
}
impl Store {
    /// Research first uses current reviewed sources and its canonical helper
    /// reservation. Each plain Coder attempt then reserves its priced bound.
    /// Its prose remains an unverified recommendation even when it finishes.
    pub fn steer_paul_research(
        self,
        requester: &str,
        request_id: &str,
        request: &ResearchRequest,
        coder: &mut dyn Coder,
        cancel: &AtomicBool,
    ) -> Result<Recommendation> {
        let root = self
            .dir
            .parent()
            .ok_or("sales host root is unavailable")?
            .to_path_buf();
        let clock = self.clock;
        let keys = self.native_keys.clone();
        let research = self.ask_paul_research(requester, request_id, request)?;
        if research.helper.answer.recommendation
            != claims::helpers::Recommendation::OwnerReviewRequired
            || research.helper.answer.citations.is_empty()
        {
            return Err("Paul Coder needs current reviewed research citations".into());
        }
        let mut current = Store::open_with_clock(&root, clock)?;
        current.native_keys = keys.clone();
        let binding = current.paul_binding_for(requester)?;
        if binding.sha256()? != research.binding_sha256 {
            return Err("Paul binding changed before Coder planning".into());
        }
        let pipeline = current.paul_pipeline(&binding, None)?;
        let expected = pipeline.rows.iter().map(|r| r.original.clone()).collect();
        let source = coder.current_source()?;
        if source.kind != expenses::Kind::Coder || source.basis != expenses::Basis::ListPrice {
            return Err("plain Coder needs an actual native priced adapter; subscriptions and declarations grant no capacity".into());
        }
        source.sha256()?;
        let record = agent::Store::with_keys(&root, "paul", keys.clone())?
            .load()?
            .ok_or("Paul is unavailable")?;
        let ctx = Context {
            root,
            clock,
            keys,
            binding,
            expected,
            lead: request.lead.clone(),
            request: digest(format!("paul-coder:{request_id}").as_bytes()),
            research_sha256: research.helper.artifact.sha256.clone(),
            helper: research.helper.clone(),
            source,
        };
        drop(current);
        let prompt = format!(
            "Check reviewed research evidence {}. Return only a bounded recommendation for owner review. No tool, message, approval, payment, publication, or completed sale is authorized.",
            ctx.research_sha256
        );
        let mut hands = Hands {
            context: &ctx,
            coder,
            cancel,
            attempt: 0,
            expenses: vec![],
            reply_digest: None,
            trouble: None,
        };
        struct NativePlan;
        impl agent_steer::Planner for NativePlan {
            fn plan(
                &mut self,
                ask: &agent_steer::Ask,
            ) -> std::result::Result<(agent_steer::Plan, agent_steer::Spent), String> {
                Ok((agent_steer::Plan {understanding:"Reconcile the current reviewed research; every completion claim needs original evidence".into(),answer_directly:false,reply_if_direct:None,steps:vec![agent_steer::Step {prompt:ask.request.clone(),done_when:"Check original evidence; no command completion or accomplished sale may be inferred from prose".into()}],verify:None},agent_steer::Spent::default()))
            }
            fn report(
                &mut self,
                _: &agent_steer::Ask,
            ) -> std::result::Result<Option<(String, agent_steer::Spent)>, String> {
                Ok(None)
            }
        }
        let mut mind = agent_steer::Mind {
            planner: Box::new(NativePlan),
            judge: None,
            unjudged: "current host evidence rule; model prose remains unverified".into(),
        };
        let policy = agent_steer::Policy {
            schema: agent_steer::POLICY_SCHEMA.into(),
            rules: vec![],
            computers: Vec::new(),
        };
        let _steered = agent_steer::run(
            &mut hands,
            &mut mind,
            &agent_steer::Input {
                record: &record,
                request: &prompt,
                briefing: &ctx.research_sha256,
                core: None,
                cwd: "",
                policy: &policy,
                note: None,
            },
        );
        let (store, _, _) = ctx.current()?;
        drop(store);
        let headline = if hands.trouble.is_some() {
            "Coder unavailable or interrupted; original expenses retained"
        } else {
            "Coder recommendation unverified; owner review required"
        }
        .into();
        Ok(Recommendation {
            research,
            coder_session: "paul-coder".into(),
            original_expenses: hands.expenses,
            reply_digest: hands.reply_digest,
            headline,
            completed_sales_work: false,
            outbound_authority: false,
            draft_authority: false,
        })
    }
}

/// Price/model custody belongs to the installed native adapter, not a CLI
/// flag or subscription label. It interprets the original provider evidence.
pub trait Custody: Send {
    fn current_source(&mut self) -> Result<expenses::Source>;
    /// Check the full provider input, including system/session context, and
    /// enforce backend output/retry caps before forwarding the native turn.
    /// Its session reads and writes must use current private data and credential
    /// guards; model text cannot be persisted as owner instructions or grants.
    /// A generic EngineFactory without this custody must refuse.
    fn admit_turn(&mut self, turn: &coder_v1::Turn, caps: &expenses::Source) -> Result<()>;
    fn observed(
        &mut self,
        ended: &coder_v1::Ended,
        models: &[String],
        tool_effect: bool,
    ) -> Result<Evidence>;
}
/// The existing plain Coder engine under explicit native price custody. The
/// generic host engine remains unavailable for sales without this adapter.
pub struct EngineCoder {
    engine: Box<dyn coder_v1::Engine>,
    custody: Box<dyn Custody>,
}
impl EngineCoder {
    pub fn new(engine: Box<dyn coder_v1::Engine>, custody: Box<dyn Custody>) -> Self {
        Self { engine, custody }
    }
}
impl Coder for EngineCoder {
    fn current_source(&mut self) -> Result<expenses::Source> {
        self.custody.current_source()
    }
    fn turn(
        &mut self,
        turn: &coder_v1::Turn,
        caps: &expenses::Source,
        cancel: &AtomicBool,
    ) -> Result<Evidence> {
        if turn.session != "paul-coder"
            || turn.instructions.is_some()
            || !turn.tool_free
            || !turn.approvals
            || turn.codex_writes
            || self.custody.current_source()? != *caps
        {
            return Err("Paul plain Coder native scope or custody changed".into());
        }
        self.custody.admit_turn(turn, caps)?;
        let local_cancel = AtomicBool::new(cancel.load(Ordering::SeqCst));
        let mut models = Vec::new();
        let mut tool_effect = false;
        let mut bytes = 0usize;
        let started = std::time::Instant::now();
        let done = AtomicBool::new(false);
        struct Finished<'a>(&'a AtomicBool);
        impl Drop for Finished<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let ended = std::thread::scope(|scope| {
            let _finished = Finished(&done);
            scope.spawn(|| {
                while !done.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst)
                        || started.elapsed().as_secs() >= caps.max_elapsed_secs
                    {
                        local_cancel.store(true, Ordering::SeqCst);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            self.engine.turn(turn, &local_cancel, &mut |event| {
                if cancel.load(Ordering::SeqCst)
                    || started.elapsed().as_secs() >= caps.max_elapsed_secs
                {
                    local_cancel.store(true, Ordering::SeqCst);
                }
                match event {
                    coder_v1::Event::Approval { .. } => {
                        tool_effect = true;
                        local_cancel.store(true, Ordering::SeqCst);
                        Some(false)
                    }
                    coder_v1::Event::Tool { .. }
                    | coder_v1::Event::Delegation { .. }
                    | coder_v1::Event::Answered { confirm: true, .. } => {
                        tool_effect = true;
                        local_cancel.store(true, Ordering::SeqCst);
                        None
                    }
                    coder_v1::Event::Model { model } => {
                        if models.len() >= 8 || model.len() > 256 {
                            local_cancel.store(true, Ordering::SeqCst);
                        } else if model.len() <= 256 {
                            models.push(model.clone());
                        }
                        None
                    }
                    coder_v1::Event::Delta { text } => {
                        bytes = bytes.saturating_add(text.len());
                        if bytes as u64 > caps.max_output_tokens.saturating_mul(4) {
                            local_cancel.store(true, Ordering::SeqCst);
                        }
                        None
                    }
                    _ => None,
                }
            })
        });
        if local_cancel.load(Ordering::SeqCst)
            || cancel.load(Ordering::SeqCst)
            || started.elapsed().as_secs() >= caps.max_elapsed_secs
        {
            return Err("Paul Coder exceeded its bound or was interrupted; original expense remains unknown".into());
        }
        self.custody.observed(&ended, &models, tool_effect)
    }
}
