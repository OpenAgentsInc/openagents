//! Owner-scoped native POL approval admission and single-use delivery.
use super::*;
use crate::engine::{ActionProfile, ApprovalRequest};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalGrant {
    pub owner: String,
    pub operation: Value,
    pub expires_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    generation: u64,
    rules: BTreeMap<String, Value>,
}
impl Book {
    pub(super) fn invalidate(&mut self) -> Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .filter(|v| i128::from(*v) <= contracts::SAFE_INTEGER)
            .ok_or("limit_exceeded: approval policy generation")?;
        self.rules.clear();
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pending {
    rule: Value,
    action: Value,
    request: Value,
    method: String,
    native_request: Value,
    #[serde(default)]
    pub(super) decision: Option<Value>,
}

fn key(owner: &str, operation: &Value) -> String {
    format!("{owner}:{}", digest_of(&operation["artifact"]))
}

impl SessionHost {
    pub(super) fn approval_authority_current(
        &self,
        state: &State,
        session: &Session,
        turn: &str,
        now: u64,
    ) -> Result<()> {
        for interaction in session.interactions.iter().filter(|i| i.turn == turn) {
            if let Some(pending) = &interaction.approval {
                let (rule, _, _) = self.approval_rule(state, session, &pending.method, now)?;
                if rule != pending.rule
                    || interaction.claim.is_none() && interaction.expires_at <= now
                {
                    return Err("stale: native approval authority expired or changed".into());
                }
                if let Some(decision) = &pending.decision {
                    if self.store.json(decision)?["expires_at"]
                        .as_u64()
                        .is_none_or(|v| v <= now)
                    {
                        return Err("stale: native approval decision expired".into());
                    }
                } else if interaction.claim.is_some() {
                    return Err("unknown: native approval decision is unavailable".into());
                }
            }
        }
        Ok(())
    }
    fn native_action(&self, adapter: &Adapter, method: &str) -> Result<(Value, ActionProfile)> {
        let engines = self.engines.borrow();
        let engine = engines.get(&adapter.engine).ok_or("engine")?;
        let action = engine
            .approval_actions()
            .into_iter()
            .find(|a| a.method == method)
            .ok_or("unsupported_feature: native action method is unavailable")?;
        let profile = engine.profile();
        if action.effects != profile.effects {
            return Err("cannot_enforce: native action effects differ from this execution".into());
        }
        let operation =
            crate::interface::native_action(&self.store, self.pubkey(), &profile, &action)?;
        if adapter.actions.get(method) != Some(&operation) {
            return Err(
                "identity_mismatch: native action differs from the admitted implementation".into(),
            );
        }
        Ok((operation, action))
    }

    fn approval_rule(
        &self,
        state: &State,
        session: &Session,
        method: &str,
        now: u64,
    ) -> Result<(Value, Value, ActionProfile)> {
        if !state.policy.owners.contains(&session.owner) {
            return Err("not_admitted: session owner was revoked".into());
        }
        self.require(state, session, "approvals")?;
        let (operation, profile) = self.native_action(&state.adapters[&session.adapter], method)?;
        let reference = state
            .approvals
            .rules
            .get(&key(&session.owner, &operation))
            .ok_or("not_admitted: no current approval rule")?
            .clone();
        let rule = self.body(&reference, "openagents.session-approval-rule.v1")?;
        if rule["generation"] != state.approvals.generation
            || rule["owner"] != session.owner
            || rule["approver"] != session.owner
            || rule["operation"] != operation
            || rule["starts_at"].as_u64().is_none_or(|v| v > now)
            || rule["expires_at"].as_u64().is_none_or(|v| v <= now)
        {
            return Err("stale: approval rule is not current".into());
        }
        Ok((reference, operation, profile))
    }

    /// Host-key administration. Ownership alone does not appoint an approver.
    pub fn set_approval_grants(&self, grants: Vec<ApprovalGrant>, now: u64) -> Result<Value> {
        if grants.len() > 64 {
            return Err("limit_exceeded: approval grants".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let mut unique = BTreeSet::new();
        for grant in &grants {
            grant
                .owner
                .parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "malformed: approval owner")?;
            if !state.policy.owners.contains(&grant.owner)
                || grant.expires_at <= now
                || i128::from(grant.expires_at) > contracts::SAFE_INTEGER
            {
                return Err("not_admitted: approval owner or expiry".into());
            }
            contracts::parse_definition(&grant.operation).map_err(contract)?;
            let mut known = false;
            for adapter in state.adapters.values() {
                for method in adapter.actions.keys() {
                    if self.native_action(adapter, method)?.0 == grant.operation {
                        known = true;
                    }
                }
            }
            if !known || !unique.insert(key(&grant.owner, &grant.operation)) {
                return Err(
                    "not_admitted: native approval operation is unknown or duplicated".into(),
                );
            }
        }
        state.approvals.invalidate()?;
        for grant in grants {
            let rule = json!({"v":"openagents.session-approval-rule.v1","requires":[],
                "generation":state.approvals.generation,"owner":grant.owner,"approver":grant.owner,
                "operation":grant.operation,"starts_at":now,"expires_at":grant.expires_at});
            let reference = self.sign(&mut state, &rule, &grant.owner, now)?;
            state
                .approvals
                .rules
                .insert(key(&grant.owner, &grant.operation), reference);
        }
        self.save(&state)?;
        self.cancel_approval_turns(&mut state, now)?;
        self.save(&state)?;
        Ok(json!({"generation":state.approvals.generation,"rules":state.approvals.rules}))
    }

    pub(super) fn cancel_approval_turns(&self, state: &mut State, now: u64) -> Result<()> {
        let turns = state
            .sessions
            .values()
            .filter_map(|s| {
                let turn = s.active.as_ref()?;
                s.interactions
                    .iter()
                    .any(|i| i.turn == turn.id && i.approval.is_some())
                    .then(|| (s.id.clone(), turn.id.clone()))
            })
            .collect::<Vec<_>>();
        for (session, turn) in turns {
            self.stop_turn(state, &session, &turn, now, "policy")?;
        }
        Ok(())
    }

    pub(super) fn approval_probe(
        &self,
        state: &State,
        session: &Session,
        request: &ApprovalRequest,
        now: u64,
    ) -> Result<()> {
        let (_, _, profile) = self.approval_rule(state, session, &request.method, now)?;
        if request.input.len() > 32_768
            || request.capture.coverage != crate::engine::CaptureCoverage::Complete
            || request.capture.payload.is_none()
            || request.capture.direction != "from_engine"
        {
            return Err("cannot_enforce: native approval input or capture".into());
        }
        let value = contracts::parse_strict_bounded(&request.input, 32_768).map_err(contract)?;
        validate_against(
            &contracts::jcs(&profile.input_schema).map_err(contract)?,
            &value,
        )?;
        let engine = &state.adapters[&session.adapter].engine;
        let native = self.native_session(session, engine)?;
        self.engines
            .borrow()
            .get(engine)
            .ok_or("engine")?
            .check_approval(native, request)?;
        if let Some(prior) = session
            .interactions
            .iter()
            .find(|i| i.native_id == request.native_id)
        {
            let pending = prior
                .approval
                .as_ref()
                .ok_or("identity_mismatch: native request kind changed")?;
            if self.pending_native(pending)? != *request {
                return Err("identity_mismatch: native approval bytes changed".into());
            }
        }
        Ok(())
    }

    pub(super) fn open_approval(
        &self,
        state: &mut State,
        session: &mut Session,
        request: ApprovalRequest,
        now: u64,
    ) -> Result<()> {
        self.approval_probe(state, session, &request, now)?;
        if session
            .interactions
            .iter()
            .any(|i| i.native_id == request.native_id)
        {
            return Ok(());
        }
        let (rule, operation, profile) =
            self.approval_rule(state, session, &request.method, now)?;
        let turn = session.active.clone().ok_or("active turn")?;
        let intent = self.store.json(&turn.intent)?;
        let mut expiry = self.store.json(&rule)?["expires_at"]
            .as_u64()
            .ok_or("rule expiry")?
            .min(intent["expires_at"].as_u64().ok_or("intent expiry")?)
            .min(now.saturating_add(3600));
        if let Some(book) = &state.transport {
            for (key, input) in &book.inputs {
                if state
                    .turn_commands
                    .get(digest_of(input))
                    .is_some_and(|t| t["session"] == session.id && t["turn"] == turn.id)
                {
                    expiry = expiry.min(*book.until.get(key).ok_or("runtime expiry")?);
                }
            }
        }
        if expiry <= now {
            return Err("stale: native approval expired".into());
        }
        let native_request = self
            .retain_captures(state, session, vec![request.capture.clone()], now)?
            .pop()
            .ok_or("native request capture")?;
        let adapter = state.adapters[&session.adapter].reference.clone();
        let engine = self.store.json(&adapter)?["engine"].clone();
        let binding=self.sign(state,&json!({"v":"openagents.session-action-binding.v1","requires":[],
            "scope":session.scope(self.pubkey()),"turn":turn.id,"native_request":native_request,
            "adapter":adapter,"engine":engine,"method":request.method,"native_id":request.native_id}),&session.owner,now)?;
        let preconditions=self.sign(state,&json!({"v":"openagents.session-action-preconditions.v1","requires":[],
            "scope":session.scope(self.pubkey()),"turn":turn.id,"turn_revision":session.turn_revision.ok_or("turn revision")?,
            "effective":session.effective,"rule":rule,"native_request":native_request}),&session.owner,now)?;
        let snapshot=self.sign(state,&json!({"v":"openagents.snapshot.v1","requires":[],
            "scope":format!("session/{}/{}",session.id,turn.id),"captured_at":now,"sources":[],"coverage":"complete"}),&session.owner,now)?;
        let input = self.store.put(&request.input, "application/json", None)?;
        let context = self
            .store
            .put_json(&json!({"scope":session.scope(self.pubkey()),
            "turn_context_digest":intent["context"]["digest"],"native_request":native_request}))?;
        let action = json!({"v":nostr::policy::ACTION_SCHEMA,"requires":[],"task_frame":intent["task_frame"],
            "snapshot":snapshot,"operation":operation,"binding":binding,"input":input,"context":context,
            "effects":profile.effects,"bounds":[],"recipient":null,"preconditions":preconditions,"nonce":token()});
        nostr::policy::validate(&action).map_err(contract)?;
        let action = self.sign(state, &action, &session.owner, now)?;
        let approval = json!({"v":nostr::policy::REQUEST_SCHEMA,"requires":[],"request":token(),"action":action,
            "requester":self.pubkey(),"approver":session.owner,"expires_at":expiry,"reviews":[]});
        let approval = self.sign(state, &approval, &session.owner, now)?;
        let id = token();
        let body = json!({"v":"openagents.session-interaction.v1","requires":[],"interaction":id,
            "scope":session.scope(self.pubkey()),"turn":turn.id,"turn_revision":session.turn_revision.ok_or("turn revision")?,
            "native_request":native_request,"kind":"approval","recipient":session.owner,"body":approval,
            "expires_at":expiry,"state":"pending"});
        let reference = self.sign(state, &body, &session.owner, now)?;
        session.interactions.push(Interaction {
            id,
            current: reference.clone(),
            reference,
            native_id: request.native_id,
            recipient: session.owner.clone(),
            turn: turn.id,
            expires_at: expiry,
            state: "pending".into(),
            claim: None,
            approval: Some(Pending {
                rule,
                action,
                request: approval,
                method: request.method,
                native_request,
                decision: None,
            }),
        });
        session.phase = "blocked".into();
        Ok(())
    }

    fn pending_native(&self, pending: &Pending) -> Result<ApprovalRequest> {
        let action = self.store.json(&pending.action)?;
        let native = self.store.json(&pending.native_request)?;
        Ok(ApprovalRequest {
            native_id: native["native_id"]
                .as_str()
                .ok_or("native approval ID")?
                .into(),
            method: pending.method.clone(),
            input: self.store.get(&action["input"])?,
            capture: Capture {
                coverage: crate::engine::CaptureCoverage::Complete,
                direction: "from_engine",
                method: native["method"].as_str().ok_or("native method")?.into(),
                native_id: native["native_id"].as_str().map(str::to_string),
                payload: Some(self.store.get(&native["payload"])?),
            },
        })
    }

    pub(super) fn normalize_approval(
        &self,
        state: &State,
        actor: &str,
        decision: &Value,
        input: &Value,
        session_id: &str,
    ) -> Result<Value> {
        nostr::policy::validate(decision).map_err(contract)?;
        let session = state.sessions.get(session_id).ok_or("session")?;
        let interaction = session
            .interactions
            .iter()
            .find(|i| {
                i.approval
                    .as_ref()
                    .is_some_and(|p| p.request == decision["request"])
            })
            .ok_or("not_admitted: approval request is not in this execution")?;
        let original = self.store.json(&interaction.reference)?;
        Ok(
            json!({"v":"openagents.session-interaction-response.v1","requires":[],"interaction":interaction.reference,
            "scope":original["scope"],"responder":actor,"response":input,"expires_at":decision["expires_at"]}),
        )
    }

    pub(super) fn check_approval_actor(
        &self,
        state: &State,
        session: &Session,
        interaction: &Interaction,
        actor: &str,
        input: &Value,
        body: &Value,
        now: u64,
    ) -> Result<()> {
        let pending = interaction
            .approval
            .as_ref()
            .ok_or("approval interaction")?;
        let (current, _, _) = self.approval_rule(state, session, &pending.method, now)?;
        if current != pending.rule
            || actor != session.owner
            || actor != interaction.recipient
            || *input != body["response"]
            || self.store.json(input)?["v"] != nostr::policy::DECISION_SCHEMA
            || !state
                .signers
                .get(digest_of(input))
                .is_some_and(|s| s.contains(actor))
        {
            return Err(
                "not_admitted: approval requires its current rule and direct approver signature"
                    .into(),
            );
        }
        Ok(())
    }

    pub(super) fn validate_approval(
        &self,
        state: &State,
        session: &Session,
        interaction: &Interaction,
        actor: &str,
        input: &Value,
        body: &Value,
        now: u64,
    ) -> Result<(ApprovalRequest, bool)> {
        self.check_approval_actor(state, session, interaction, actor, input, body, now)?;
        let pending = interaction.approval.as_ref().ok_or("approval")?;
        let decision =
            nostr::policy::verify(input, &self.bytes(input)?, nostr::policy::DECISION_SCHEMA)
                .map_err(contract)?;
        let request = nostr::policy::verify(
            &pending.request,
            &self.store.get(&pending.request)?,
            nostr::policy::REQUEST_SCHEMA,
        )
        .map_err(contract)?;
        let action = nostr::policy::verify(
            &pending.action,
            &self.store.get(&pending.action)?,
            nostr::policy::ACTION_SCHEMA,
        )
        .map_err(contract)?;
        let conditions = self.body(
            &action["preconditions"],
            "openagents.session-action-preconditions.v1",
        )?;
        let active = session
            .active
            .as_ref()
            .ok_or("stale: native approval turn ended")?;
        let intent = self.store.json(&active.intent)?;
        let expires = decision["expires_at"].as_u64().ok_or("decision expiry")?;
        if decision["request"] != pending.request
            || decision["action"] != pending.action
            || decision["approver"] != actor
            || request["approver"] != actor
            || expires <= now
            || expires > request["expires_at"].as_u64().ok_or("request expiry")?
            || action["task_frame"] != intent["task_frame"]
            || conditions["scope"] != session.scope(self.pubkey())
            || conditions["turn"] != active.id
            || Some(
                conditions["turn_revision"]
                    .as_u64()
                    .ok_or("turn revision")?,
            ) != session.turn_revision
            || conditions["effective"] != session.effective
            || conditions["rule"] != pending.rule
            || conditions["native_request"] != pending.native_request
        {
            return Err("stale: approval action, request, expiry, or conditions changed".into());
        }
        let native_request = self.pending_native(pending)?;
        self.approval_probe(state, session, &native_request, now)?;
        Ok((native_request, decision["decision"] == "approve"))
    }
}
