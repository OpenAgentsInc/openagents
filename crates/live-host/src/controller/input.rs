//! Explicit device-operation authority. Owning a conversation does not appoint
//! its owner as an input approver.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputGrant {
    pub plan: Value,
    pub owner: String,
    pub approver: String,
    pub operation: Value,
    pub expires_at: u64,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    generation: u64,
    rules: Vec<Value>,
}
fn validate(schema: &Value, value: &Value) -> Result<()> {
    let bytes = contracts::jcs(schema).map_err(contract)?;
    let digest = digest_bytes(&bytes);
    let closure =
        contracts::prepare_closure(&BTreeMap::from([(digest.clone(), bytes)])).map_err(contract)?;
    contracts::validate_instance(&closure, &digest, value).map_err(contract)
}
impl Controller {
    /// Publish the actual installed operation; this grants no input rights.
    pub fn device_operation(&self) -> Result<Value> {
        let operation = self
            .adapters
            .borrow()
            .device
            .input_operation()
            .ok_or("unsupported_feature: installed device has no input contract")?;
        if operation.effects["spend"] != false
            || operation.effects["delegates"] != false
            || operation.implementation.is_empty()
            || operation.implementation.len() > 1_048_576
            || operation.binding.is_empty()
            || operation.binding.len() > 1_048_576
        {
            return Err("malformed: installed device identity".into());
        }
        let artifact = |bytes: &[u8], media: &str| self.store.put(bytes, media, None);
        let implementation = artifact(&operation.implementation, "application/octet-stream")?;
        let binding = artifact(&operation.binding, "application/json")?;
        let input = artifact(
            &contracts::jcs(&operation.input_schema).map_err(contract)?,
            contracts::SCHEMA_MEDIA_TYPE,
        )?;
        let output = artifact(
            &contracts::jcs(&operation.output_schema).map_err(contract)?,
            contracts::SCHEMA_MEDIA_TYPE,
        )?;
        let observation = artifact(
            &contracts::jcs(&operation.observation_schema).map_err(contract)?,
            contracts::SCHEMA_MEDIA_TYPE,
        )?;
        let preconditions = artifact(
            &contracts::jcs(&operation.preconditions_schema).map_err(contract)?,
            contracts::SCHEMA_MEDIA_TYPE,
        )?;
        let id = format!("{}:live/device-input", self.pubkey());
        let body = json!({"v":1,"requires":[],"id":id,"profile":"native",
            "summary":"Apply one separately approved device input.","input":input,"output":output,
            "effects":operation.effects,"minimum":{},"support":{"bounds":{},
                "cancellation":"before_dispatch","idempotency":"none","evidence":["openagents.live-input-outcome.v1"]},
            "binding_contract":{"operation":"live-device-input","interface":"openagents.live-device.v1"},
            "meta":{"implementation":implementation,"binding":binding,"observation_schema":observation,"preconditions_schema":preconditions}});
        nostr::cap::parse_definition(&body).map_err(|e| e.to_string())?;
        Ok(
            json!({"id":id,"artifact":self.store.put(&contracts::jcs(&body).map_err(contract)?,
            "application/json",Some("openagents.cap.v1"))?}),
        )
    }
    /// Host-key administration of exact plan, owner, approver, and operation.
    /// Replacing the set invalidates every earlier rule, including a regrant.
    pub fn set_input_grants(&self, grants: Vec<InputGrant>, now: u64) -> Result<Vec<Value>> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        if grants.len() > 256 {
            return Err("limit_exceeded: device input grants".into());
        }
        let generation = state
            .input_rules
            .generation
            .checked_add(1)
            .filter(|v| i128::from(*v) <= contracts::SAFE_INTEGER)
            .ok_or("limit_exceeded: input grant generation")?;
        let operation = if grants.is_empty() {
            None
        } else {
            Some(self.device_operation()?)
        };
        let mut rules = Vec::new();
        let mut seen = BTreeSet::new();
        for grant in grants {
            let session = state
                .sessions
                .values()
                .find(|s| s.plan == grant.plan)
                .ok_or("not_admitted: current input plan required")?;
            if session.owner != grant.owner
                || !state.policy.owners.contains(&grant.owner)
                || grant.approver != grant.owner
                || Some(&grant.operation) != operation.as_ref()
                || grant.expires_at <= now
                || grant.expires_at > now.saturating_add(3600)
                || grant.expires_at > session.body["expires_at"].as_u64().ok_or("plan expiry")?
            {
                return Err("not_admitted: unsupported device input grant".into());
            }
            let key = (digest_of(&grant.plan).to_owned(), grant.owner.clone());
            if !seen.insert(key) {
                return Err("conflict: duplicate input grant".into());
            }
            let rule = json!({"v":"openagents.live-input-rule.v1","requires":[],"controller":self.pubkey(),
                "generation":generation,"plan":grant.plan,"owner":grant.owner,"approver":grant.approver,
                "operation":grant.operation,"starts_at":now,"expires_at":grant.expires_at});
            rules.push(self.sign(&mut state, &rule, &grant.owner, now)?);
        }
        state.input_rules = Book {
            generation,
            rules: rules.clone(),
        };
        self.save(&state)?;
        Ok(rules)
    }
    pub(super) fn input_rule(
        &self,
        state: &State,
        session: &Session,
        operation: &Value,
        now: u64,
    ) -> Result<(Value, Value)> {
        if !state.policy.owners.contains(&session.owner) {
            return Err("revoked: input owner".into());
        }
        if self.device_operation()? != *operation {
            return Err("identity_mismatch: installed input operation".into());
        }
        for reference in &state.input_rules.rules {
            let rule = self.body(reference, "openagents.live-input-rule.v1")?;
            if rule["plan"] == session.plan
                && rule["owner"] == session.owner
                && rule["operation"] == *operation
                && rule["generation"] == state.input_rules.generation
                && rule["starts_at"].as_u64().is_some_and(|t| t <= now)
                && rule["expires_at"].as_u64().is_some_and(|t| t > now)
                && Self::signed_by(state, reference, self.pubkey())
            {
                return Ok((reference.clone(), rule));
            }
        }
        Err("not_admitted: no current device input approver".into())
    }
}

impl Controller {
    #[allow(clippy::type_complexity)]
    pub(super) fn device_input(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        body: &Value,
        input: &Value,
        now: u64,
        received: std::time::Instant,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        if signer != session.owner {
            return Err(refuse("not_admitted", "device input admission"));
        }
        if session.state != "active" {
            return Err(refuse("unavailable", "session is not active"));
        }
        let proposal = self.body(&body["proposal"], "openagents.live-input.v1")?;
        if !same(&proposal["plan"], &session.plan) {
            return Err(refuse("stale", "proposal names another plan"));
        }
        let key = format!("input:{}", proposal["request"].as_str().unwrap_or_default());
        if state.priors.contains_key(&format!("{}:{key}", session.id)) {
            // A changed approval or action under the same proposal is not an
            // automatic retry; the original attempt is reconciled first.
            return Err(refuse(
                "idempotency_conflict",
                "reconcile the original input first",
            ));
        }
        let observation_ref = &proposal["observation"];
        if !session
            .observations
            .values()
            .any(|o| same(o, observation_ref))
        {
            return Err(refuse(
                "not_admitted",
                "observation is not from this session",
            ));
        }
        let observation = self.body(observation_ref, "openagents.live-observation.v1")?;
        if observation["meta"]["media_epoch"] != session.epoch {
            return Err("stale: observation media epoch".into());
        }
        let freshness = self.body(&observation["freshness"], "openagents.live-freshness.v1")?;
        let (generation, focus) = self.adapters.borrow().device.state();
        let captured_at = live::number(&observation, "captured_at").map_err(contract)?;
        let age_ms = now.saturating_sub(captured_at).saturating_mul(1000);
        let max_age = freshness["max_age_ms"]
            .as_u64()
            .ok_or("malformed: observation age")?;
        let focus_version = freshness["focus_version"]
            .as_u64()
            .ok_or("cannot_enforce: focus version required")?;
        let original = *self
            .capture_clocks
            .borrow()
            .get(digest_of(observation_ref))
            .ok_or("cannot_enforce: capture clock is unavailable; obtain a new observation")?;
        let started = std::time::Instant::now();
        if original.elapsed().as_millis() >= u128::from(max_age)
            || captured_at > now
            || age_ms >= max_age
            || observation["host_generation"] != generation
            || focus_version != focus
        {
            return Err(refuse(
                "stale",
                "the observation no longer matches the device",
            ));
        }
        let (allowed, output_schema, expires) =
            self.check_input_approval(state, session, &proposal, &body["proposal"], body, now)?;
        if !allowed {
            let outcome=self.sign(state,&json!({"v":"openagents.live-input-outcome.v1","requires":[],
                "admission":input,"dispatched":false,"outcome":"refused","observed":null,"reason":"not_admitted"}),
                &session.owner,now)?;
            state.priors.insert(
                format!("{}:{key}", session.id),
                Prior {
                    fingerprint: digest_of(input).into(),
                    result: json!({"value":outcome}),
                },
            );
            return Ok((Some(outcome), Vec::new(), "refused"));
        }
        let input_bytes = self.bytes(&proposal["input"])?;
        let mut guard = InputGuard {
            generation,
            focus,
            deadline: original
                .checked_add(std::time::Duration::from_millis(max_age))
                .ok_or("limit_exceeded: device input deadline")?
                .min(
                    started
                        .checked_add(std::time::Duration::from_millis(max_age - age_ms))
                        .ok_or("limit_exceeded: device input deadline")?,
                ),
            observation: observation.clone(),
            binding: self.bytes(&proposal["binding"])?,
            preconditions: proposal["preconditions"]
                .as_array()
                .ok_or("malformed: input preconditions")?
                .iter()
                .map(|reference| self.bytes(reference))
                .collect::<Result<Vec<_>>>()?,
        };

        guard.deadline = guard.deadline.min(
            received
                .checked_add(std::time::Duration::from_secs(expires.saturating_sub(now)))
                .ok_or("limit_exceeded: input authority deadline")?,
        );
        if std::time::Instant::now() >= guard.deadline {
            return Err("stale: input authority expired before dispatch".into());
        }

        // Journal dispatch durably before applying the input.
        self.record(
            session,
            "dispatched",
            json!({"request":digest_of(input),"effect":"device-input"}),
            now,
        )?;
        state.priors.insert(
            format!("{}:{key}", session.id),
            Prior {
                fingerprint: digest_of(input).into(),
                result: json!({"value":null}),
            },
        );
        let mut snapshot = self.state()?;
        snapshot
            .sessions
            .insert(session.id.clone(), session.clone());
        snapshot.priors.insert(
            format!("{}:{key}", session.id),
            Prior {
                fingerprint: digest_of(input).into(),
                result: json!({"value":null}),
            },
        );
        snapshot.signers.extend(state.signers.clone());
        snapshot.declarations.extend(state.declarations.clone());
        self.save(&snapshot)?;
        let applied = self.adapters.borrow_mut().device.apply_checked(
            proposal["operation"]["id"].as_str().unwrap_or_default(),
            &input_bytes,
            &guard,
        );
        let (outcome, observed, status, dispatched, reason) = match applied {
            Applied::Applied(bytes) => {
                if bytes.len() > 1_048_576 {
                    ("unknown", None, "unknown", true, Some("limit_exceeded"))
                } else {
                    let valid = contracts::parse_strict_bounded(&bytes, 1_048_576)
                        .map_err(contract)
                        .and_then(|body| validate(&output_schema, &body))
                        .is_ok();
                    let observed = Some(self.store.put(
                        &bytes,
                        if valid {
                            "application/json"
                        } else {
                            "application/octet-stream"
                        },
                        None,
                    )?);
                    if valid {
                        ("applied", observed, "accepted", true, None)
                    } else {
                        ("unknown", observed, "unknown", true, Some("malformed"))
                    }
                }
            }
            Applied::Refused(code) => ("refused", None, "refused", false, Some(code)),
            Applied::Unknown => ("unknown", None, "unknown", true, None),
        };
        let record = json!({"v":"openagents.live-input-outcome.v1","requires":[],"admission":input,"dispatched":dispatched,
            "outcome":outcome,"observed":observed,"reason":reason});
        let record = self.sign(state, &record, &session.owner.clone(), now)?;
        let kind = if status == "unknown" {
            "unknown"
        } else {
            "observed"
        };
        self.record(session, kind, json!({"input":digest_of(&record)}), now)?;
        state.priors.insert(
            format!("{}:{key}", session.id),
            Prior {
                fingerprint: digest_of(input).into(),
                result: json!({"value":record}),
            },
        );
        Ok((Some(record), Vec::new(), status))
    }
}

impl Controller {
    fn checked_operation(
        &self,
        proposal: &Value,
        session: &Session,
    ) -> Result<crate::media::InputOperation> {
        if proposal["frame"] != session.body["frame"]
            || proposal["plan"] != session.plan
            || proposal["operation"] != self.device_operation()?
        {
            return Err("identity_mismatch: device proposal operation or frame".into());
        }
        let frame = nostr::work::parse(&self.bytes(&proposal["frame"])?).map_err(contract)?;
        if proposal["frame"]["schema"] != "openagents.task-frame.v1"
            || frame["v"] != "openagents.task-frame.v1"
            || frame["task"] != session.task
            || frame["owner"] != session.owner
            || frame["controller"] != session.owner
        {
            return Err("not_admitted: current owner-controlled input frame required".into());
        }
        let profile = self
            .adapters
            .borrow()
            .device
            .input_operation()
            .ok_or("unsupported_feature: installed device contract")?;
        if self.bytes(&proposal["binding"])? != profile.binding {
            return Err("identity_mismatch: installed device binding".into());
        }
        let input = contracts::parse_strict_bounded(&self.bytes(&proposal["input"])?, 1_048_576)
            .map_err(contract)?;
        validate(&profile.input_schema, &input)?;
        if !session
            .observations
            .values()
            .any(|reference| reference == &proposal["observation"])
        {
            return Err("not_admitted: observation is not from this input session".into());
        }
        let observation = self.body(&proposal["observation"], "openagents.live-observation.v1")?;
        validate(&profile.observation_schema, &observation)?;
        let conditions = proposal["preconditions"]
            .as_array()
            .ok_or("malformed: input preconditions")?
            .iter()
            .map(|reference| {
                contracts::parse_strict_bounded(&self.bytes(reference)?, 1_048_576)
                    .map_err(contract)
            })
            .collect::<Result<Vec<_>>>()?;
        validate(&profile.preconditions_schema, &json!(conditions))?;
        Ok(profile)
    }
    fn conditions(
        &self,
        session: &Session,
        proposal: &Value,
        rule: &Value,
        revision: u64,
    ) -> Value {
        json!({"v":"openagents.live-input-preconditions.v1","requires":[],"controller":self.pubkey(),
            "session":session.id,"plan":session.plan,"revision":revision,"media_epoch":session.epoch,
            "rule":rule,"observation":proposal["observation"],"binding":proposal["binding"],
            "conditions":proposal["preconditions"]})
    }
    pub(super) fn prepare_input(
        &self,
        state: &mut State,
        session: &mut Session,
        signer: &str,
        proposal: &Value,
        reference: &Value,
        now: u64,
    ) -> Result<(Option<Value>, Vec<Value>, &'static str)> {
        if signer != session.owner || session.state != "active" {
            return Err("not_admitted: device proposal owner".into());
        }
        let profile = self.checked_operation(proposal, session)?;
        let (rule, authority) = self.input_rule(state, session, &proposal["operation"], now)?;
        if state.priors.contains_key(&format!(
            "{}:input:{}",
            session.id,
            proposal["request"].as_str().ok_or("input request")?
        )) {
            return Err("idempotency_conflict: original input was already decided".into());
        }
        if !session
            .observations
            .values()
            .any(|r| r == &proposal["observation"])
        {
            return Err("not_admitted: proposal observation".into());
        }
        let observation = self.body(&proposal["observation"], "openagents.live-observation.v1")?;
        let freshness = self.body(&observation["freshness"], "openagents.live-freshness.v1")?;
        let clock = *self
            .capture_clocks
            .borrow()
            .get(digest_of(&proposal["observation"]))
            .ok_or("cannot_enforce: original capture clock required")?;
        let (generation, focus) = self.adapters.borrow().device.state();
        let max_age = freshness["max_age_ms"].as_u64().ok_or("observation age")?;
        let expires = authority["expires_at"]
            .as_u64()
            .ok_or("rule expiry")?
            .min(session.body["expires_at"].as_u64().ok_or("plan expiry")?)
            .min(
                observation["captured_at"]
                    .as_u64()
                    .ok_or("capture time")?
                    .saturating_add(max_age / 1000),
            );
        if expires <= now
            || clock.elapsed().as_millis() >= u128::from(max_age)
            || observation["host_generation"] != generation
            || freshness["focus_version"] != focus
            || observation["meta"]["media_epoch"] != session.epoch
        {
            return Err("stale: input proposal observation".into());
        }
        let preconditions = self.sign(
            state,
            &self.conditions(session, proposal, &rule, session.revision + 1),
            &session.owner,
            now,
        )?;
        // The native observation and its conditions are pinned separately.
        // This profile supplies no additional CTX source readers.
        let snapshot = self.sign(
            state,
            &json!({"v":"openagents.snapshot.v1","requires":[],
            "scope":format!("live/{}",session.id),"captured_at":observation["captured_at"],
            "sources":[],"coverage":"partial"}),
            &session.owner,
            now,
        )?;
        let action = json!({"v":nostr::policy::ACTION_SCHEMA,"requires":[],"task_frame":proposal["frame"],
            "snapshot":snapshot,"operation":proposal["operation"],"binding":proposal["binding"],"input":reference,
            "context":session.body["conversation"],"effects":profile.effects,"bounds":session.body["bounds"],
            "recipient":proposal["binding"],"preconditions":preconditions,"nonce":token()});
        nostr::policy::validate(&action).map_err(contract)?;
        let action = self.sign(state, &action, &session.owner, now)?;
        let request=self.sign(state,&json!({"v":nostr::policy::REQUEST_SCHEMA,"requires":[],"request":token(),
            "action":action,"requester":self.pubkey(),"approver":authority["approver"],"expires_at":expires,"reviews":[]}),&session.owner,now)?;
        Ok((Some(request), Vec::new(), "accepted"))
    }
    fn check_input_approval(
        &self,
        state: &State,
        session: &Session,
        proposal: &Value,
        reference: &Value,
        admission: &Value,
        now: u64,
    ) -> Result<(bool, Value, u64)> {
        let profile = self.checked_operation(proposal, session)?;
        let (rule, authority) = self.input_rule(state, session, &proposal["operation"], now)?;
        let approvals = admission["approvals"]
            .as_array()
            .ok_or("malformed: input approvals")?;
        if approvals.len() != 1 {
            return Err(
                "unsupported_feature: exactly one appointed input approver required".into(),
            );
        }
        let decision = nostr::policy::verify(
            &approvals[0],
            &self.bytes(&approvals[0])?,
            nostr::policy::DECISION_SCHEMA,
        )
        .map_err(contract)?;
        let action = nostr::policy::verify(
            &admission["action"],
            &self.bytes(&admission["action"])?,
            nostr::policy::ACTION_SCHEMA,
        )
        .map_err(contract)?;
        let request = nostr::policy::verify(
            &decision["request"],
            &self.bytes(&decision["request"])?,
            nostr::policy::REQUEST_SCHEMA,
        )
        .map_err(contract)?;
        let approver = authority["approver"].as_str().ok_or("input approver")?;
        if !Self::signed_by(state, &approvals[0], approver)
            || !Self::signed_by(state, &admission["action"], self.pubkey())
            || !Self::signed_by(state, &decision["request"], self.pubkey())
            || decision["approver"] != approver
            || request["approver"] != approver
            || request["requester"] != self.pubkey()
            || request["action"] != admission["action"]
            || decision["action"] != admission["action"]
            || decision["expires_at"]
                .as_u64()
                .is_none_or(|t| t <= now || t > request["expires_at"].as_u64().unwrap_or(0))
            || request["expires_at"]
                .as_u64()
                .is_none_or(|t| t <= now || t > authority["expires_at"].as_u64().unwrap_or(0))
            || action["input"] != *reference
            || action["operation"] != proposal["operation"]
            || action["binding"] != proposal["binding"]
            || action["task_frame"] != proposal["frame"]
            || action["effects"] != profile.effects
            || action["context"] != session.body["conversation"]
            || action["bounds"] != session.body["bounds"]
            || action["recipient"] != proposal["binding"]
            || request["reviews"] != json!([])
        {
            return Err("not_admitted: input decision signer, expiry, action, or scope".into());
        }
        let conditions = self.body(
            &action["preconditions"],
            "openagents.live-input-preconditions.v1",
        )?;
        if conditions != self.conditions(session, proposal, &rule, session.revision) {
            return Err("stale: input action conditions changed".into());
        }
        Ok((
            decision["decision"] == "approve",
            profile.output_schema,
            decision["expires_at"].as_u64().ok_or("decision expiry")?,
        ))
    }
}
