//! Owner-installed scope, durable admission, and existing local task commands.
use crate::client::{envelope, pubkey, random_id};
use crate::*;
use nostr::domain::Tag;
use nostr::{control, execution};
use secp256k1::SecretKey;
use std::path::Path;

mod retained;
mod setup;

#[cfg(test)]
std::thread_local! {
    pub(crate) static FAIL_AFTER_LOCAL_APPLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static FAIL_FINAL_REPLY_SAVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static FAIL_BEFORE_LOCAL_APPLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Invitation {
    pub body: Value,
    pub event: Event,
    pub grant_expires: u64,
    pub consumed: Option<String>,
    pub answer: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Access {
    pub body: Value,
    pub event: Event,
    pub revoked: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandRecord {
    fingerprint: String,
    input: Event,
    local: Vec<u8>,
    admitted_at: u64,
    answer: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    request: Event,
    input: Event,
    root: Vec<u8>,
    reply: Option<Reply>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    schema: String,
    pub setup: Setup,
    service: execution::Service,
    pub blobs: Blobs,
    invitations: BTreeMap<String, Invitation>,
    pub grants: BTreeMap<String, Access>,
    commands: BTreeMap<String, CommandRecord>,
    jobs: BTreeMap<String, Job>,
    pub snapshots: BTreeMap<String, crate::view::Snapshot>,
    sources: BTreeMap<String, Event>,
}
pub struct Host {
    store: crate::store::Store,
    pub(crate) document: Document,
    secret: SecretKey,
}
impl Host {
    pub fn open(directory: &Path, setup: Setup, secret: SecretKey) -> Result<Self> {
        validate_setup(&setup, &secret)?;
        let (store, bytes) = crate::store::Store::open(directory)?;
        let document = if let Some(bytes) = bytes {
            let document: Document = serde_json::from_value(
                nostr::contracts::parse_strict_bounded(&bytes, crate::store::MAX_BYTES)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if document.schema != "openagents.control-host.v1"
                || serde_json::to_value(&document.setup).map_err(|e| e.to_string())?
                    != serde_json::to_value(&setup).map_err(|e| e.to_string())?
                || document.service.worker != setup.authority
            {
                return Err("control store has another frozen authority or scope".into());
            }
            document
        } else {
            Document {
                schema: "openagents.control-host.v1".into(),
                blobs: setup.blobs.clone(),
                service: execution::Service::new(&setup.authority, 1, setup.retain_until),
                setup,
                invitations: BTreeMap::new(),
                grants: BTreeMap::new(),
                commands: BTreeMap::new(),
                jobs: BTreeMap::new(),
                snapshots: BTreeMap::new(),
                sources: BTreeMap::new(),
            }
        };
        let mut host = Self {
            store,
            document,
            secret,
        };
        retained::validate(&host.document, &host.secret)?;
        host.current_task()?;
        host.save()?;
        Ok(host)
    }
    pub fn setup(&self) -> &Setup {
        &self.document.setup
    }
    /// Admit the command before resolving its bounded, exact private text.
    pub fn required_text(&self, cj: &Event, input: &Event, now: u64) -> Result<Option<Value>> {
        self.store.ensure_healthy()?;
        let opened = execution::open_request(
            cj,
            &self.setup().authority,
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("control execute required".into());
        };
        let key = self
            .document
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        if now >= execute.deadline && !self.document.jobs.contains_key(&key) {
            return Err("control request deadline passed".into());
        }
        let artifact =
            nostr::private_artifact::open(input, &self.secret).map_err(|e| e.to_string())?;
        let mut reference = execute
            .input_artifact
            .clone()
            .ok_or("control input artifact required")?;
        let event = reference
            .event
            .take()
            .ok_or("original control input declaration required")?;
        if event.id != input.id
            || event.pubkey != input.pubkey
            || event.kind != input.kind
            || reference != *artifact.artifact()
            || artifact.signer() != opened.principal
        {
            return Err("control input provenance differs".into());
        }
        let body = control::parse(artifact.inline_bytes().ok_or("control input unavailable")?)
            .map_err(|e| e.to_string())?;
        let operation = self
            .setup()
            .operations
            .get(role(&body)?)
            .ok_or("control operation missing")?;
        check_operation(self.setup(), operation, execute, &body)?;
        self.authorize(&body, &opened.principal, now, false)?;
        if self
            .document
            .jobs
            .get(&key)
            .is_some_and(|job| job.reply.is_some())
        {
            return Ok(None);
        }
        if body["v"] == control::COMMAND && body["action"] == "steer" {
            Ok(Some(body["payload"]["message"].clone()))
        } else {
            Ok(None)
        }
    }
    pub fn handle_text(
        &mut self,
        cj: &Event,
        input: &Event,
        text: &client::TextDelivery,
        now: u64,
    ) -> Result<Reply> {
        let Some(reference) = self.required_text(cj, input, now)? else {
            return self.handle(cj, input, &Blobs::default(), now);
        };
        let bytes = text.verify(&reference, &input.pubkey, &self.secret)?;
        let missing = [&text.declaration, &text.carrier]
            .iter()
            .filter(|event| !self.document.sources.contains_key(&event.id))
            .count();
        if self.document.sources.len() + missing > 512 {
            return Err("control source retention bound reached".into());
        }
        self.document
            .sources
            .insert(text.declaration.id.clone(), text.declaration.clone());
        self.document
            .sources
            .insert(text.carrier.id.clone(), text.carrier.clone());
        self.save()?;
        let mut attachments = Blobs::default();
        attachments.0.insert(digest(&reference)?, bytes);
        self.handle(cj, input, &attachments, now)
    }
    pub(crate) fn save(&mut self) -> Result<()> {
        self.store.save(&self.document)
    }
    fn current_task(&self) -> Result<coder::task::Task> {
        self.store.ensure_healthy()?;
        let setup = self.setup();
        let task = coder::task::Store::open(&setup.task_directory)
            .map_err(|e| e.to_string())?
            .show(&setup.task_id)
            .map_err(|e| e.to_string())?;
        if task.intent_digest != setup.intent_digest {
            return Err("control task mapping no longer matches the owner-installed intent".into());
        }
        Ok(task)
    }
    /// A local owner action; this method is never selected by a remote role.
    pub fn invite(
        &mut self,
        client: &str,
        rights: &[String],
        now: u64,
        pair_expires: u64,
        grant_expires: u64,
    ) -> Result<Event> {
        self.current_task()?;
        let setup = self.setup();
        if self.document.invitations.len() >= 128
            || pair_expires <= now
            || pair_expires - now > setup.max_pairing_seconds
            || grant_expires <= pair_expires
            || grant_expires - now > setup.max_grant_seconds
            || grant_expires > setup.retain_until
        {
            return Err("invitation count or lifetime exceeds the owner policy".into());
        }
        let body = json!({"v":control::INVITATION,"requires":[],"invitation":random_id(),"challenge":random_id(),"owner":setup.owner,"authority":setup.authority,"client":client,"scope":setup.scope,"rights":rights,"policy":setup.policy,"issued_at":now,"expires_at":pair_expires});
        control::validate(&body).map_err(|e| e.to_string())?;
        let event = envelope(&body, &self.secret, client, now, setup.retain_until)?;
        let reference = self
            .document
            .blobs
            .insert_json(&body, control::INVITATION)?;
        self.document.invitations.insert(
            digest(&reference)?,
            Invitation {
                body,
                event: event.clone(),
                grant_expires,
                consumed: None,
                answer: None,
            },
        );
        self.save()?;
        Ok(event)
    }
    /// Authenticate original envelopes and admit the exact registered CJ role.
    /// Attachment bytes are inert and never followed as locators.
    pub fn handle(
        &mut self,
        cj: &Event,
        input: &Event,
        attachments: &Blobs,
        now: u64,
    ) -> Result<Reply> {
        self.store.ensure_healthy()?;
        let setup = self.setup().clone();
        let opened = execution::open_request(
            cj,
            &setup.authority,
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("this host exposes only scoped control operations".into());
        };
        let artifact =
            nostr::private_artifact::open(input, &self.secret).map_err(|e| e.to_string())?;
        let mut input_identity = execute
            .input_artifact
            .clone()
            .ok_or("CJ must name its exact input artifact")?;
        let declaration = input_identity
            .event
            .take()
            .ok_or("CJ input needs its original declaring event")?;
        if declaration.id != input.id
            || declaration.pubkey != input.pubkey
            || declaration.kind != input.kind
            || artifact.signer() != opened.principal
            || input_identity != *artifact.artifact()
        {
            return Err("control input is not the CJ principal's exact signed artifact".into());
        }
        let bytes = artifact
            .inline_bytes()
            .ok_or("control input requires retained inline JSON")?;
        let body = control::parse(bytes).map_err(|e| e.to_string())?;
        let input_ref = reference(
            bytes,
            "application/json",
            body["v"].as_str().ok_or("control input schema")?,
        );
        let role = role(&body)?;
        let operation = setup
            .operations
            .get(role)
            .ok_or("control role is not installed")?;
        check_operation(&setup, operation, execute, &body)?;
        let key = self
            .document
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        if let Some(job) = self.document.jobs.get(&key) {
            let claim = self
                .document
                .service
                .claims
                .get(&key)
                .ok_or("control CJ journal is incomplete")?;
            if claim.fingerprint != execute.fingerprint || job.input.id != input.id {
                return Err("control CJ idempotency conflict".into());
            }
            self.authorize(&body, &opened.principal, now, true)?;
            if let Some(reply) = &job.reply {
                // The content remains exact; the outer response binds this relay retry.
                return self.readdress(reply, cj, &opened.principal, now);
            }
            if now >= execute.deadline {
                return Err("unresolved control request deadline passed".into());
            }
        } else {
            if self.document.jobs.len() >= 256 {
                return Err("control CJ retention limit reached".into());
            }
            self.authorize(&body, &opened.principal, now, false)?;
            let admission = self
                .document
                .service
                .prepare(&opened, now, &random_id())
                .map_err(|e| format!("{e:?}"))?;
            let execution::Admission::Reserved { root, .. } = admission else {
                return Err("control CJ journal is incomplete".into());
            };
            self.document.jobs.insert(
                key.clone(),
                Job {
                    request: cj.clone(),
                    input: input.clone(),
                    root,
                    reply: None,
                },
            );
            self.document
                .blobs
                .insert_json(&body, body["v"].as_str().ok_or("input schema")?)?;
            self.save()?;
        }
        let root = self
            .document
            .jobs
            .get(&key)
            .ok_or("CJ journal missing")?
            .root
            .clone();
        let feedback = self
            .document
            .service
            .acknowledge(&key, &root)
            .map_err(|e| format!("{e:?}"))?;
        self.document
            .service
            .intend(&key)
            .map_err(|e| format!("{e:?}"))?;
        self.save()?;
        let operation_answer = match role {
            "pair" => self.pair(&body, &input_ref, &opened.principal, now),
            "command" => self.command(&body, &input_ref, input, attachments, now),
            "read" => crate::view::read(self, &body, &input_ref, &opened.principal, now),
            "revoke" => self.revoke(&body, &input_ref, now),
            _ => Err("unsupported control role".into()),
        };
        let (answer, mut authorized) = match operation_answer {
            Ok(answer) => answer,
            Err(_) => {
                self.store.ensure_healthy()?;
                let pending_effect = role == "command"
                    && self.document.commands.values().any(|record| {
                        record.fingerprint == digest(&input_ref).unwrap_or_default()
                            && record.answer.is_none()
                    });
                if !pending_effect {
                    let result=self.document.service.observe(&key,execution::Report{outcome:"refused",dispatched:false,output:None,artifacts:vec![],receipts:vec![],spend:None,verification:Some("not_run"),integration:Some("not_requested"),code:Some("unavailable".into()),message:Some("The requested control input or retained evidence is unavailable under the installed bounds.".into())}).map_err(|e|format!("{e:?}"))?;
                    let reply =
                        self.reply_events(cj, &opened.principal, &feedback, &result, vec![], now)?;
                    self.document
                        .jobs
                        .get_mut(&key)
                        .ok_or("CJ journal missing")?
                        .reply = Some(reply.clone());
                    self.save()?;
                    return Ok(reply);
                }
                let mut payload = execution::refusal_result(
                    &execute.request,
                    execute.attempt,
                    &execute.run,
                    "unavailable",
                    "The control operation did not establish a result; retry its exact identity.",
                );
                payload["outcome"] = json!("unknown");
                payload["dispatched"] = Value::Null;
                let reply =
                    self.reply_events(cj, &opened.principal, &feedback, &payload, vec![], now)?;
                // A retained local command intent is reconciled on an exact retry;
                // unavailable effects are not turned into a terminal success.
                return Ok(reply);
            }
        };
        control::validate(&answer).map_err(|e| e.to_string())?;
        let answer_ref = self
            .document
            .blobs
            .insert_json(&answer, answer["v"].as_str().ok_or("answer schema")?)?;
        let answer_event = envelope(
            &answer,
            &self.secret,
            &opened.principal,
            now,
            setup.retain_until,
        )?;
        let answer_ref = client::event_reference(&answer_event, &answer_ref)?;
        authorized.push(answer_event);
        let artifacts = authorized
            .iter()
            .map(|event| {
                let opened = nostr::private_artifact::open(event, &self.secret)
                    .map_err(|e| e.to_string())?;
                let body = nostr::contracts::parse_strict(
                    opened
                        .inline_bytes()
                        .ok_or("answer artifact is unavailable")?,
                )
                .map_err(|e| e.to_string())?;
                let r = reference(
                    opened.inline_bytes().ok_or("answer bytes")?,
                    "application/json",
                    body["v"].as_str().ok_or("answer version")?,
                );
                client::event_reference(event, &r)
            })
            .collect::<Result<Vec<_>>>()?;
        let result = self
            .document
            .service
            .observe(
                &key,
                execution::Report {
                    outcome: "completed",
                    dispatched: true,
                    output: Some(json!({"artifact":answer_ref})),
                    artifacts,
                    receipts: vec![],
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let reply =
            self.reply_events(cj, &opened.principal, &feedback, &result, authorized, now)?;
        self.document
            .jobs
            .get_mut(&key)
            .ok_or("CJ journal missing")?
            .reply = Some(reply.clone());
        #[cfg(test)]
        if FAIL_FINAL_REPLY_SAVE.with(|fault| fault.replace(false)) {
            crate::store::FAIL_NEXT_SAVE.with(|fault| fault.set(true));
        }
        self.save()?;
        Ok(reply)
    }
    fn authorize(&self, body: &Value, principal: &str, now: u64, _retry: bool) -> Result<()> {
        match role(body)? {
            "pair" => {
                let invitation = self
                    .document
                    .invitations
                    .get(&digest(&body["invitation"])?)
                    .ok_or("pairing is not invited")?;
                let identical_consumed = invitation.consumed.as_ref().is_some_and(|digest| {
                    nostr::contracts::jcs(body)
                        .is_ok_and(|bytes| &nostr::contracts::digest_bytes(&bytes) == digest)
                });
                if invitation.body["client"] != principal
                    || body["client"] != principal
                    || body["challenge"] != invitation.body["challenge"]
                    || (!identical_consumed && now >= number(&invitation.body, "expires_at")?)
                    || !subset(&body["rights"], &invitation.body["rights"])
                {
                    return Err("pairing does not match its current owner admission".into());
                }
                if identical_consumed {
                    self.access_events(
                        invitation.answer.as_ref().ok_or("pairing answer missing")?,
                        principal,
                        now,
                    )?;
                }
            }
            "revoke" => {
                if principal != self.setup().owner {
                    return Err("revocation requires the installed owner".into());
                }
                if !self.document.grants.contains_key(&digest(&body["grant"])?) {
                    return Err("grant is unavailable".into());
                }
            }
            action => {
                let grant = self.grant(body, principal, now)?;
                let right = if action == "read" {
                    "observe"
                } else {
                    body["action"].as_str().ok_or("command action")?
                };
                if !grant["rights"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|v| v == right))
                {
                    return Err("control right is not admitted".into());
                }
                if action == "command" {
                    let issued = number(body, "issued_at")?;
                    let expires = number(body, "expires_at")?;
                    let retained_result = self
                        .document
                        .commands
                        .get(&format!(
                            "{}|{}",
                            digest(&body["grant"])?,
                            body["command"].as_str().ok_or("command identity")?
                        ))
                        .is_some_and(|record| {
                            record.answer.is_some()
                                && nostr::contracts::jcs(body).is_ok_and(|bytes| {
                                    nostr::contracts::digest_bytes(&bytes) == record.fingerprint
                                })
                        });
                    if issued > now
                        || (!retained_result && now >= expires)
                        || expires.saturating_sub(issued) > self.setup().max_command_seconds
                        || expires > number(grant, "expires_at")?
                    {
                        return Err("control command is stale".into());
                    }
                }
            }
        }
        self.current_task()?;
        Ok(())
    }
    fn grant(&self, body: &Value, principal: &str, now: u64) -> Result<&Value> {
        let access = self
            .document
            .grants
            .get(&digest(&body["grant"])?)
            .ok_or("control grant is unavailable")?;
        if access.body["client"] != principal
            || access.body["scope"] != self.setup().scope
            || body["scope"] != self.setup().scope
            || access.body["policy"] != self.setup().policy
            || access.revoked.is_some()
            || now >= number(&access.body, "expires_at")?
            || now < number(&access.body, "issued_at")?
        {
            return Err("control grant is not current or admitted".into());
        }
        Ok(&access.body)
    }
    fn pair(
        &mut self,
        body: &Value,
        input_ref: &Value,
        principal: &str,
        now: u64,
    ) -> Result<(Value, Vec<Event>)> {
        let id = digest(&body["invitation"])?;
        let invitation = self
            .document
            .invitations
            .get(&id)
            .ok_or("invitation unavailable")?
            .clone();
        let fingerprint = digest(input_ref)?;
        if let Some(previous) = &invitation.consumed {
            if previous != &fingerprint {
                return Ok((
                    access_result(
                        input_ref,
                        "refused",
                        Value::Null,
                        Some("idempotency_conflict"),
                    ),
                    vec![],
                ));
            }
            let answer = invitation.answer.ok_or("pairing result missing")?;
            let events = self.access_events(&answer, principal, now)?;
            return Ok((answer, events));
        }
        let (answer, events) = if body["accepted"] != true || now >= invitation.grant_expires {
            (
                access_result(input_ref, "refused", Value::Null, Some("not_admitted")),
                vec![],
            )
        } else {
            let admission = json!({"v":"openagents.control-owner-admission.v1","requires":[],"owner":self.setup().owner,"authority":self.setup().authority,"invitation":body["invitation"],"client":principal,"rights":body["rights"],"scope":self.setup().scope,"grant_expires":invitation.grant_expires});
            let admission_ref = self
                .document
                .blobs
                .insert_json(&admission, "openagents.control-owner-admission.v1")?;
            let grant = json!({"v":control::GRANT,"requires":[],"grant":random_id(),"epoch":0,"owner":self.setup().owner,"authority":self.setup().authority,"client":principal,"scope":self.setup().scope,"rights":body["rights"],"policy":self.setup().policy,"invitation":body["invitation"],"pairing":input_ref,"admission":admission_ref,"issued_at":now,"expires_at":invitation.grant_expires});
            control::validate(&grant).map_err(|e| e.to_string())?;
            let grant_ref = self.document.blobs.insert_json(&grant, control::GRANT)?;
            let event = envelope(
                &grant,
                &self.secret,
                principal,
                now,
                self.setup().retain_until,
            )?;
            self.document.grants.insert(
                digest(&grant_ref)?,
                Access {
                    body: grant,
                    event: event.clone(),
                    revoked: None,
                },
            );
            (
                access_result(input_ref, "granted", grant_ref, None),
                vec![event],
            )
        };
        let invitation = self
            .document
            .invitations
            .get_mut(&id)
            .ok_or("invitation missing")?;
        invitation.consumed = Some(fingerprint);
        invitation.answer = Some(answer.clone());
        self.save()?;
        Ok((answer, events))
    }
    fn access_events(&self, answer: &Value, principal: &str, now: u64) -> Result<Vec<Event>> {
        if answer["access"].is_null() {
            return Ok(vec![]);
        }
        if let Some(grant) = self.document.grants.get(&digest(&answer["access"])?) {
            if grant.revoked.is_some()
                || now >= number(&grant.body, "expires_at")?
                || grant.body["client"] != principal
            {
                return Err("paired grant is no longer active".into());
            }
            return Ok(vec![grant.event.clone()]);
        }
        Err("retained access artifact unavailable".into())
    }
    fn revoke(&mut self, body: &Value, input_ref: &Value, now: u64) -> Result<(Value, Vec<Event>)> {
        let id = digest(&body["grant"])?;
        let access = self
            .document
            .grants
            .get(&id)
            .ok_or("grant unavailable")?
            .clone();
        let revocation = if let Some(revocation) = access.revoked {
            revocation
        } else {
            let authorization = json!({"v":"openagents.control-owner-revocation.v1","requires":[],"owner":self.setup().owner,"request":input_ref,"scope":self.setup().scope});
            let authorization_ref = self
                .document
                .blobs
                .insert_json(&authorization, "openagents.control-owner-revocation.v1")?;
            json!({"v":control::REVOCATION,"requires":[],"grant":body["grant"],"epoch":1,"authority":self.setup().authority,"request":input_ref,"authorization":authorization_ref,"revoked_at":now})
        };
        let reference = self
            .document
            .blobs
            .insert_json(&revocation, control::REVOCATION)?;
        self.document
            .grants
            .get_mut(&id)
            .ok_or("grant missing")?
            .revoked = Some(revocation.clone());
        self.save()?;
        let event = envelope(
            &revocation,
            &self.secret,
            &self.setup().owner,
            now,
            self.setup().retain_until,
        )?;
        Ok((
            access_result(input_ref, "revoked", reference, None),
            vec![event],
        ))
    }
    fn command(
        &mut self,
        body: &Value,
        input_ref: &Value,
        input: &Event,
        attachments: &Blobs,
        now: u64,
    ) -> Result<(Value, Vec<Event>)> {
        let key = format!(
            "{}|{}",
            digest(&body["grant"])?,
            body["command"].as_str().ok_or("command identity")?
        );
        let fingerprint = digest(input_ref)?;
        if let Some(record) = self.document.commands.get(&key) {
            if record.fingerprint != fingerprint {
                return Ok((
                    command_result(
                        input_ref,
                        "conflict",
                        Value::Null,
                        Some("idempotency_conflict"),
                    ),
                    vec![],
                ));
            }
            if let Some(answer) = &record.answer {
                return self.command_answer(answer, &input.pubkey, now);
            }
        } else {
            if self.document.commands.len() >= 256 {
                return Err("control command retention bound".into());
            }
            let task = self.current_task()?;
            if task.revision != number(body, "expected_revision")?
                || !matches!(
                    task.status,
                    coder::task::Status::Queued | coder::task::Status::Running
                )
            {
                return Ok((
                    command_result(input_ref, "conflict", Value::Null, Some("conflict")),
                    vec![],
                ));
            }
            let action = if body["action"] == "steer" {
                if body["payload"]["replaces"] != json!([]) {
                    return Ok((
                        command_result(
                            input_ref,
                            "refused",
                            Value::Null,
                            Some("unsupported_feature"),
                        ),
                        vec![],
                    ));
                }
                let message =
                    parse_artifact(&body["payload"]["message"]).map_err(|e| e.to_string())?;
                if message.media_type != "text/plain" || message.size > 32 * 1024 {
                    return Err("unsupported steering message".into());
                }
                let text = std::str::from_utf8(attachments.resolve(&message)?)
                    .map_err(|_| "steering message must be UTF-8")?
                    .to_owned();
                coder::task::Action::Correct {
                    prompt: text,
                    reason: "Scoped Nostr client correction".into(),
                }
            } else {
                coder::task::Action::Cancel {
                    reason: body["payload"]["reason"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or("Scoped Nostr client cancellation")
                        .into(),
                }
            };
            let local = coder::task::Command {
                schema: coder::task::COMMAND_SCHEMA.into(),
                command_id: format!(
                    "ctrl-{}",
                    nostr::contracts::digest_bytes(key.as_bytes()).trim_start_matches("sha256:")
                ),
                task_id: self.setup().task_id.clone(),
                expected_revision: Some(task.revision),
                action,
            };
            let local = serde_json::to_vec(&local).map_err(|e| e.to_string())?;
            coder::task::parse_command(&local).map_err(|e| e.to_string())?;
            self.document.commands.insert(
                key.clone(),
                CommandRecord {
                    fingerprint,
                    input: input.clone(),
                    local,
                    admitted_at: now,
                    answer: None,
                },
            );
            self.save()?;
        }
        let record = self
            .document
            .commands
            .get(&key)
            .ok_or("command journal unavailable")?
            .clone();
        #[cfg(test)]
        if FAIL_BEFORE_LOCAL_APPLY.with(|fault| fault.replace(false)) {
            return Err("synthetic failure after retained local intent".into());
        }
        let applied = coder::task::Store::open(&self.setup().task_directory)
            .map_err(|e| e.to_string())?
            .apply(&record.local);
        #[cfg(test)]
        if applied.is_ok() && FAIL_AFTER_LOCAL_APPLY.with(|fault| fault.replace(false)) {
            return Err("synthetic failure after the durable local effect".into());
        }
        let answer = match applied {
            Ok(_) => {
                let receipt = json!({"v":control::COMMAND_RECEIPT,"requires":[],"command":input_ref,"authority":self.setup().authority,"admitted_at":record.admitted_at,"disposition":if body["action"]=="steer" {"correction_recorded"}else{"cancel_requested"}});
                let reference = self
                    .document
                    .blobs
                    .insert_json(&receipt, control::COMMAND_RECEIPT)?;
                command_result(input_ref, "accepted", reference, None)
            }
            Err(
                coder::task::Error::Conflict
                | coder::task::Error::RevisionMismatch
                | coder::task::Error::InvalidTransition,
            ) => command_result(input_ref, "conflict", Value::Null, Some("conflict")),
            Err(_) => {
                return Err("local control effect is uncertain; retry the exact command".into());
            }
        };
        self.document
            .commands
            .get_mut(&key)
            .ok_or("command journal missing")?
            .answer = Some(answer.clone());
        self.save()?;
        self.command_answer(&answer, &input.pubkey, now)
    }
    fn command_answer(
        &self,
        answer: &Value,
        principal: &str,
        now: u64,
    ) -> Result<(Value, Vec<Event>)> {
        let events = if answer["receipt"].is_null() {
            vec![]
        } else {
            let receipt = self.document.blobs.json(&answer["receipt"])?;
            vec![envelope(
                &receipt,
                &self.secret,
                principal,
                now,
                self.setup().retain_until,
            )?]
        };
        Ok((answer.clone(), events))
    }
    pub(crate) fn sign_artifact(&self, body: &Value, principal: &str, now: u64) -> Result<Event> {
        envelope(
            body,
            &self.secret,
            principal,
            now,
            self.setup().retain_until,
        )
    }
    fn reply_events(
        &self,
        cj: &Event,
        principal: &str,
        feedback: &Value,
        result: &Value,
        artifacts: Vec<Event>,
        now: u64,
    ) -> Result<Reply> {
        let tags = vec![
            Tag::new(vec!["p".into(), principal.into()]),
            Tag::new(vec!["e".into(), cj.id.clone()]),
        ];
        Ok(Reply {
            feedback: client::seal(
                &self.secret,
                principal,
                execution::FEEDBACK_KIND,
                tags.clone(),
                feedback,
                now,
            )?,
            result: client::seal(
                &self.secret,
                principal,
                execution::RESULT_KIND,
                tags,
                result,
                now,
            )?,
            artifacts,
        })
    }
    fn readdress(&self, reply: &Reply, cj: &Event, principal: &str, now: u64) -> Result<Reply> {
        let original = self
            .document
            .jobs
            .values()
            .find(|job| {
                job.reply
                    .as_ref()
                    .is_some_and(|stored| stored.result.id == reply.result.id)
            })
            .ok_or("reply journal unavailable")?;
        let opened = execution::open_request(
            &original.request,
            &self.setup().authority,
            &self.secret,
            original.request.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = opened.body else {
            return Err("retained request type".into());
        };
        let key = self
            .document
            .service
            .key(principal, &execute.request, execute.attempt);
        let claim = self
            .document
            .service
            .claims
            .get(&key)
            .ok_or("claim unavailable")?;
        let mut service = self.document.service.clone();
        let feedback = service
            .acknowledge(&key, &original.root)
            .map_err(|e| format!("{e:?}"))?;
        self.reply_events(
            cj,
            principal,
            &feedback,
            claim.result.as_ref().ok_or("result unavailable")?,
            reply.artifacts.clone(),
            now,
        )
    }
}

fn access_result(request: &Value, status: &str, access: Value, reason: Option<&str>) -> Value {
    json!({"v":control::ACCESS_RESULT,"requires":[],"request":request,"status":status,"access":access,"reason":reason})
}
fn command_result(command: &Value, status: &str, receipt: Value, reason: Option<&str>) -> Value {
    json!({"v":control::COMMAND_RESULT,"requires":[],"command":command,"status":status,"receipt":receipt,"reason":reason})
}
pub(crate) fn digest(reference: &Value) -> Result<String> {
    Ok(parse_artifact(reference).map_err(|e| e.to_string())?.digest)
}
pub(crate) fn number(value: &Value, key: &str) -> Result<u64> {
    value[key].as_u64().ok_or_else(|| format!("missing {key}"))
}
fn subset(left: &Value, right: &Value) -> bool {
    left.as_array().is_some_and(|items| {
        items
            .iter()
            .all(|item| right.as_array().is_some_and(|all| all.contains(item)))
    })
}
fn role(body: &Value) -> Result<&'static str> {
    match body["v"].as_str() {
        Some(control::PAIRING) => Ok("pair"),
        Some(control::COMMAND) => Ok("command"),
        Some(control::READ) => Ok("read"),
        Some(control::REVOKE) => Ok("revoke"),
        _ => Err("artifact has no installed control operation".into()),
    }
}

fn validate_setup(setup: &Setup, secret: &SecretKey) -> Result<()> {
    if setup.schema != "openagents.control-setup.v1"
        || setup.authority != pubkey(secret)
        || setup.scope["controller"] != setup.authority
        || setup.max_pairing_seconds == 0
        || setup.max_pairing_seconds > 3600
        || setup.max_grant_seconds == 0
        || setup.max_grant_seconds > 86400
        || setup.max_command_seconds == 0
        || setup.max_command_seconds > 600
        || setup.operations.len() != 4
    {
        return Err("unsupported control setup or lifetime policy".into());
    }
    // Validate principals and scope using the shared wire contract.
    control::validate(&json!({"v":control::INVITATION,"requires":[],"invitation":"a".repeat(64),"challenge":"b".repeat(64),"owner":setup.owner,"authority":setup.authority,"client":pubkey(&SecretKey::new(&mut secp256k1::rand::rng())),"scope":setup.scope,"rights":["observe"],"policy":setup.policy,"issued_at":1,"expires_at":2})).map_err(|e|e.to_string())?;
    let policy = setup.blobs.json(&setup.policy)?;
    if policy
        != json!({"v":"openagents.control-disclosure.v1","requires":[],"state":"task-projection","history":"atif-projection","max_items":128,"max_bytes":32768,"max_snapshot_steps":1024})
    {
        return Err("unsupported control disclosure policy".into());
    }
    for role in ["pair", "command", "read", "revoke"] {
        let operation = setup
            .operations
            .get(role)
            .ok_or("control operation missing")?;
        let target =
            nostr::contracts::parse_definition(&operation.target).map_err(|e| e.to_string())?;
        let definition: Value =
            nostr::contracts::parse_strict(setup.blobs.resolve(&target.artifact)?)
                .map_err(|e| e.to_string())?;
        let parsed = nostr::cap::parse_definition(&definition).map_err(|e| e.to_string())?;
        if parsed.id != target.id
            || parsed.transport != "native"
            || definition["binding_contract"]["operation"] != role
            || definition["binding_contract"]["interface"] != "openagents.control-host.v1"
        {
            return Err("control CAP identity or native operation binding differs".into());
        }
        let lock = nostr::contracts::parse_lock(&setup.blobs.json(&operation.lock)?)
            .map_err(|e| e.to_string())?;
        if lock.root != target.id
            || lock.entries.len() != 1
            || lock.entries[0].definition != target
            || !lock.entries[0].dependencies.is_empty()
        {
            return Err("control requires the owner-installed single-operation lock".into());
        }
        setup.blobs.bytes(&operation.context)?;
        setup.blobs.bytes(&operation.requirements)?;
        for (field, schema) in [
            (
                "input",
                match role {
                    "pair" => control::PAIRING,
                    "command" => control::COMMAND,
                    "read" => control::READ,
                    _ => control::REVOKE,
                },
            ),
            (
                "output",
                match role {
                    "pair" | "revoke" => control::ACCESS_RESULT,
                    "command" => control::COMMAND_RESULT,
                    _ => control::VIEW,
                },
            ),
        ] {
            let r = nostr::contracts::parse_schema_ref(&definition[field])
                .map_err(|e| e.to_string())?;
            let bytes = setup.blobs.resolve(&r.0)?;
            let value = nostr::contracts::parse_strict(bytes).map_err(|e| e.to_string())?;
            if value["properties"]["v"]["const"] != schema {
                return Err("control CAP schema does not pin its registered role".into());
            }
            nostr::contracts::prepare_closure(&BTreeMap::from([(r.0.digest, bytes.to_vec())]))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn check_operation(
    setup: &Setup,
    operation: &Operation,
    execute: &execution::Execute,
    body: &Value,
) -> Result<()> {
    if execute.target
        != nostr::contracts::parse_definition(&operation.target).map_err(|e| e.to_string())?
        || execute.lock != parse_artifact(&operation.lock).map_err(|e| e.to_string())?
        || execute.context != parse_artifact(&operation.context).map_err(|e| e.to_string())?
        || execute.requirements
            != parse_artifact(&operation.requirements).map_err(|e| e.to_string())?
        || execute.parent.is_some()
        || execute.bounds.wall_ms.is_some()
        || execute.bounds.output_bytes.is_none_or(|v| v < 1048576)
        || execute.bounds.spend_microunits.is_some()
        || execute.bounds.jobs.is_some()
    {
        return Err(
            "CJ operation differs from the installed CAP binding or supported bounds".into(),
        );
    }
    let definition = nostr::contracts::parse_strict(setup.blobs.resolve(&execute.target.artifact)?)
        .map_err(|e| e.to_string())?;
    let schema =
        nostr::contracts::parse_schema_ref(&definition["input"]).map_err(|e| e.to_string())?;
    let closure = nostr::contracts::prepare_closure(&BTreeMap::from([(
        schema.0.digest.clone(),
        setup.blobs.resolve(&schema.0)?.to_vec(),
    )]))
    .map_err(|e| e.to_string())?;
    nostr::contracts::validate_instance(&closure, &schema.0.digest, body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    #[test]
    fn pending_text_retry_at_source_bound_retains_no_new_events() {
        let mut fixture = crate::tests::Fixture::new();
        let grant = fixture.pair(&["steer"]);
        let key = fixture.client;
        let now = crate::tests::NOW;
        let text = client::text(
            "Exactly one correction",
            &key,
            &fixture.setup.authority,
            now,
            fixture.setup.retain_until,
        )
        .unwrap();
        let command = fixture.command(
            &grant,
            "steer",
            json!({"message":text.reference,"replaces":[]}),
            1,
        );
        let (input, r) = fixture.input(&command, &key);
        let cj = client::request(
            &key,
            &fixture.setup.authority,
            &fixture.setup.operations["command"],
            &input,
            &r,
            now,
            now + 100,
            fixture.setup.retain_until,
            "full-source-retry",
        )
        .unwrap();
        FAIL_AFTER_LOCAL_APPLY.with(|fault| fault.set(true));
        fixture
            .host
            .handle_text(&cj, &input, &text, now + 1)
            .unwrap();
        for index in 0..510 {
            let event = client::envelope(
                &json!({"v":"openagents.control-fixture-source.v1","index":index}),
                &key,
                &fixture.setup.authority,
                now,
                fixture.setup.retain_until,
            )
            .unwrap();
            fixture
                .host
                .document
                .sources
                .insert(event.id.clone(), event);
        }
        fixture.host.save().unwrap();
        let reply = fixture
            .host
            .handle_text(&cj, &input, &text, now + 2)
            .unwrap();
        assert_eq!(
            client::result(
                &reply.result,
                &cj,
                &fixture.setup.authority,
                "full-source-retry",
                &key
            )
            .unwrap()["outcome"],
            "completed"
        );
        assert_eq!(fixture.host.document.sources.len(), 512);
        assert_eq!(
            coder::task::Store::open(&fixture.tasks)
                .unwrap()
                .show("synthetic")
                .unwrap()
                .revision,
            2
        );
    }
}
