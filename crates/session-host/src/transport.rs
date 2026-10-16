//! Durable CJ execution over separately admitted session capabilities.
use nostr::{domain::Event, execution};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Transport {
    pub(crate) service: execution::Service,
    pub(crate) replies: BTreeMap<String, Event>,
    pub(crate) feedback: BTreeMap<String, Event>,
    pub(crate) files: BTreeMap<String, Vec<Value>>,
    pub(crate) inputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub(crate) until: BTreeMap<String, u64>,
    #[serde(default)]
    pub(crate) outcomes: BTreeMap<String, Value>,
    #[serde(default)]
    pub(crate) denied: BTreeSet<String>,
    #[serde(default)]
    pub(crate) requests: BTreeMap<String, Event>,
}

pub(crate) struct Target {
    pub(crate) adapter: Value,
    pub(crate) engine: Value,
    pub(crate) input: Value,
}

pub(crate) enum JobProgress {
    Pending,
    Done {
        outcome: &'static str,
        output: Value,
        files: Vec<Value>,
    },
}

pub type Reply = (Event, BTreeMap<String, Vec<u8>>);

impl crate::host::SessionHost {
    fn seal_cj(
        &self,
        event: &Event,
        actor: &str,
        kind: u16,
        body: &Value,
        now: u64,
    ) -> crate::Result<Event> {
        let peer = actor
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "caller key")?;
        execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            kind,
            vec![
                nostr::domain::Tag::new(vec!["e".into(), event.id.clone()]),
                nostr::domain::Tag::new(vec!["p".into(), actor.into()]),
            ],
            body,
        )
        .map_err(|e| format!("{e:?}"))
    }
    fn cj_cached(
        &self,
        state: &crate::host::State,
        key: &str,
        reply: &Event,
    ) -> crate::Result<Reply> {
        let refs = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .files
            .get(key)
            .ok_or("reply artifacts")?;
        let mut files = BTreeMap::new();
        for reference in refs {
            let bytes = self.store.get(reference)?;
            files.insert(nostr::contracts::digest_bytes(&bytes), bytes);
        }
        Ok((reply.clone(), files))
    }
    fn finish_runtime(
        &self,
        state: &mut crate::host::State,
        event: &Event,
        opened: &execution::Opened,
        execute: &execution::Execute,
        key: &str,
        observe: bool,
        now: u64,
    ) -> crate::Result<Reply> {
        let input = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .inputs
            .get(key)
            .cloned();
        let expired = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .until
            .get(key)
            .is_none_or(|deadline| now >= *deadline)
            || self
                .runtime_clocks
                .borrow()
                .get(key)
                .is_some_and(|(started, millis)| {
                    started.elapsed().as_millis() >= u128::from(*millis)
                });
        if observe
            && expired
            && let Some(input) = &input
        {
            self.cj_stop(state, input, now, "deadline")?;
        }
        let progress =
            match input {
                Some(input) => {
                    if observe && !expired {
                        self.cj_observe(state, event, execute, key, &input, now)?
                    } else {
                        self.cj_progress(state, &input, &opened.principal, key, false, now)?
                    }
                }
                None => return Err(
                    "unknown: native admission was not acknowledged; execution was not repeated"
                        .into(),
                ),
            };
        let book = state.transport.as_ref().ok_or("transport")?;
        let mut refs = book.files.get(key).ok_or("RUN artifacts")?.clone();
        refs.truncate(3);
        match progress {
            JobProgress::Pending => {
                if let Some(reply) = book.feedback.get(key) {
                    let reply = reply.clone();
                    self.save(state)?;
                    return self.cj_cached(state, key, &reply);
                }
                let note = state
                    .transport
                    .as_mut()
                    .ok_or("transport")?
                    .service
                    .progress(key, 0, "running")
                    .map_err(|e| format!("{e:?}"))?;
                let reply = self.seal_cj(
                    event,
                    &opened.principal,
                    execution::FEEDBACK_KIND,
                    &note,
                    now,
                )?;
                state
                    .transport
                    .as_mut()
                    .ok_or("transport")?
                    .feedback
                    .insert(key.into(), reply.clone());
                self.save(state)?;
                self.cj_cached(state, key, &reply)
            }
            JobProgress::Done {
                mut outcome,
                mut output,
                files,
            } => {
                let mut files = self.cj_files(state, &opened.principal, files)?;
                let bound = execute.bounds.output_bytes.ok_or("output bound")?;
                let size = files
                    .iter()
                    .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?))
                    .ok_or("reply size")?;
                if size > bound.saturating_sub(16_384) {
                    let input = &execute.payload["input"]["artifact"];
                    let status = if outcome == "unknown" {
                        "unknown"
                    } else {
                        outcome = "failed";
                        "refused"
                    };
                    let failure = self.control_result(
                        state,
                        &opened.principal,
                        input,
                        status,
                        None,
                        Some("limit_exceeded"),
                        now,
                    )?;
                    output = self.store.put_json(&failure)?;
                    files = vec![output.clone()];
                }
                refs.extend(files);
                let mut records = refs[..3]
                    .iter()
                    .map(|r| self.store.json(r))
                    .collect::<crate::Result<Vec<_>>>()?;
                let kind = if outcome == "unknown" {
                    "unknown"
                } else {
                    "resolved"
                };
                let detail = if outcome == "unknown" {
                    serde_json::json!({"attempt":execute.attempt,"reservation":null,"evidence":[]})
                } else {
                    serde_json::json!({"outcome":outcome,"dispatched":true,"output":output,
                        "receipts":[],"usage":{"calls":1,"spend_microunits":0},
                        "verification":"not_run","integration":"not_requested"})
                };
                let record =
                    nostr::run::append_envelope(&mut records, self.pubkey(), kind, detail, now)
                        .map_err(|e| e.to_string())?;
                refs.push(self.store.put_json(&record)?);
                let terminal = if outcome == "unknown" {
                    refs.last().ok_or("unknown record")?.clone()
                } else {
                    let record = nostr::run::append_envelope(
                        &mut records,
                        self.pubkey(),
                        "settled",
                        serde_json::json!({"outcome":outcome,"unknowns":[],"verification":"not_run",
                            "integration":"not_requested","results":[output],"reservations":[]}),
                        now,
                    )
                    .map_err(|e| e.to_string())?;
                    let r = self.store.put_json(&record)?;
                    refs.push(r.clone());
                    r
                };
                let mut unique = BTreeMap::new();
                for reference in refs {
                    unique.insert(
                        reference["digest"].as_str().ok_or("digest")?.to_string(),
                        reference,
                    );
                }
                let refs = unique.into_values().collect::<Vec<_>>();
                let total = refs
                    .iter()
                    .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?))
                    .ok_or("reply size")?;
                if total > execute.bounds.output_bytes.ok_or("output bound")? {
                    self.save(state)?;
                    return Err("unknown: response exceeds the original output allowance".into());
                }
                let result = state
                    .transport
                    .as_mut()
                    .ok_or("transport")?
                    .service
                    .observe(
                        key,
                        execution::Report {
                            outcome,
                            dispatched: true,
                            output: Some(output),
                            artifacts: refs.clone(),
                            receipts: vec![terminal],
                            spend: Some(0),
                            verification: Some("not_run"),
                            integration: Some("not_requested"),
                            code: None,
                            message: None,
                        },
                    )
                    .map_err(|e| format!("{e:?}"))?;
                let reply = self.seal_cj(
                    event,
                    &opened.principal,
                    execution::RESULT_KIND,
                    &result,
                    now,
                )?;
                let book = state.transport.as_mut().ok_or("transport")?;
                book.files.insert(key.into(), refs);
                book.replies.insert(key.into(), reply.clone());
                self.save(state)?;
                self.cj_cached(state, key, &reply)
            }
        }
    }

    /// Admit one exact ENV-bound session operation. A started turn returns
    /// progress until poll observes its actual native terminal result.
    pub fn serve(
        &self,
        event: &Event,
        artifacts: &BTreeMap<String, Vec<u8>>,
        binding: &crate::host::Binding,
        now: u64,
    ) -> crate::Result<Reply> {
        let opened = execution::open_environment_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("session execute required".into());
        };
        if execute.payload["requires"] != serde_json::json!([nostr::environment::BINDING_FEATURE])
            || execute.input_artifact.is_none()
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute
                .bounds
                .wall_ms
                .is_none_or(|v| !(1000..=300_000).contains(&v))
            || execute
                .bounds
                .output_bytes
                .is_none_or(|v| !(65_536..=1_048_576).contains(&v))
        {
            return Err("cannot_enforce: session execution profile or bounds differ".into());
        }
        if artifacts.len() > 256 || artifacts.values().map(Vec::len).sum::<usize>() > 8 * 1_048_576
        {
            return Err("limit_exceeded: runtime input artifacts".into());
        }
        for (digest, bytes) in artifacts {
            if nostr::contracts::digest_bytes(bytes) != *digest {
                return Err("identity_mismatch: input artifact bytes".into());
            }
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        state.transport.get_or_insert_with(|| Transport {
            service: execution::Service::new(self.pubkey(), 64, 9_007_199_254_740_991),
            replies: BTreeMap::new(),
            feedback: BTreeMap::new(),
            files: BTreeMap::new(),
            inputs: BTreeMap::new(),
            until: BTreeMap::new(),
            outcomes: BTreeMap::new(),
            denied: BTreeSet::new(),
            requests: BTreeMap::new(),
        });
        let key = state.transport.as_ref().ok_or("transport")?.service.key(
            &opened.principal,
            &execute.request,
            execute.attempt,
        );
        self.cj_reply_reader(&state, &opened.principal, &key)?;
        if let Some(claim) = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .service
            .claims
            .get(&key)
        {
            if claim.execute_event != event.id
                || claim.fingerprint != execute.fingerprint
                || now > claim.retain_until
            {
                return Err("idempotency_conflict: runtime invocation differs or expired".into());
            }
            if let Some(reply) = state
                .transport
                .as_ref()
                .ok_or("transport")?
                .replies
                .get(&key)
            {
                return self.cj_cached(&state, &key, reply);
            }
            return self.finish_runtime(&mut state, event, &opened, execute, &key, false, now);
        }
        execution::open_environment_request(
            event,
            self.pubkey(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let target = self.cj_target(&state, &execute.payload["target"])?;
        if self.cj_run_in_use(&state, &execute.run)? {
            return Err("idempotency_conflict: RUN already belongs to a native operation".into());
        }
        let book = state.transport.as_mut().ok_or("transport")?;
        if book.service.claims.len() >= 4096
            || book.service.claims.values().any(|c| c.run == execute.run)
        {
            return Err("limit_exceeded: runtime invocation retention or RUN identity".into());
        }
        let (claim, root) = match book
            .service
            .prepare(&opened, now, &workspace_host::artifact::token())
            .map_err(|e| format!("{e:?}"))?
        {
            execution::Admission::Reserved { claim, root } => (claim, root),
            _ => return Err("unexpected runtime retransmission".into()),
        };
        let root_ref =
            self.store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
        let mut records = vec![nostr::contracts::parse_strict(&root).map_err(|e| e.to_string())?];
        let input = execute.payload["input"]["artifact"].clone();
        let admitted=nostr::run::append_envelope(&mut records,self.pubkey(),"admitted",
            serde_json::json!({"enforcement":[],"reservations":[],"deadlines":{"request":execute.deadline},
                "retention":execute.retain_until,"input":input}),now).map_err(|e|e.to_string())?;
        let dispatched=nostr::run::append_envelope(&mut records,self.pubkey(),"dispatched",
            serde_json::json!({"binding":execute.payload["target"],"attempt":execute.attempt,
                "input":input,"context":execute.payload["context"],"effect":"session-runtime","generation":0}),now)
            .map_err(|e|e.to_string())?;
        book.files.insert(
            key.clone(),
            vec![
                root_ref,
                self.store.put_json(&admitted)?,
                self.store.put_json(&dispatched)?,
            ],
        );
        book.service
            .acknowledge(&claim.key, &root)
            .map_err(|e| format!("{e:?}"))?;
        book.service
            .intend(&claim.key)
            .map_err(|e| format!("{e:?}"))?;
        let wall_ms = execute.bounds.wall_ms.ok_or("wall bound")?;
        book.until.insert(
            key.clone(),
            execute.deadline.min(now.saturating_add(wall_ms / 1000)),
        );
        book.requests.insert(key.clone(), event.clone());
        self.save(&state)?;
        self.runtime_clocks
            .borrow_mut()
            .insert(key.clone(), (std::time::Instant::now(), wall_ms));
        self.dispatch_cj(
            state, event, &opened, execute, artifacts, binding, &target, &key, now,
        )?;
        let mut state = self.state()?;
        self.finish_runtime(&mut state, event, &opened, execute, &key, false, now)
    }

    /// Observe the original native turn. Never repeats its admission or input.
    pub fn poll(&self, event: &Event, now: u64) -> crate::Result<Reply> {
        let opened = execution::open_environment_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("session execute required".into());
        };
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let book = state.transport.as_ref().ok_or("no runtime invocation")?;
        let key = book
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = book
            .service
            .claims
            .get(&key)
            .ok_or("no runtime invocation")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err("idempotency_conflict: runtime observation differs or expired".into());
        }
        self.cj_reply_reader(&state, &opened.principal, &key)?;
        if let Some(reply) = book.replies.get(&key) {
            return self.cj_cached(&state, &key, reply);
        }
        self.finish_runtime(&mut state, event, &opened, execute, &key, true, now)
    }

    /// Advance retained work without a client. This cannot admit or resend input.
    /// Reader revocation withholds replies while cleanup continues locally.
    pub fn advance(&self, now: u64) -> crate::Result<usize> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let Some(book) = &state.transport else {
            return Ok(0);
        };
        let pending: Vec<_> = book
            .requests
            .iter()
            .filter(|(key, _)| {
                book.service
                    .claims
                    .get(*key)
                    .is_some_and(|c| c.outcome.is_none())
            })
            .map(|(key, event)| (key.clone(), event.clone()))
            .collect();
        let mut advanced = 0;
        for (key, event) in pending {
            let opened = execution::open_environment_request(
                &event,
                self.pubkey(),
                &self.secret,
                event.created_at,
                execution::Window::DEFAULT,
            )
            .map_err(|e| format!("{e:?}"))?;
            let execution::Body::Execute(execute) = &opened.body else {
                continue;
            };
            if self
                .cj_reply_reader(&state, &opened.principal, &key)
                .is_err()
            {
                let input = state
                    .transport
                    .as_ref()
                    .and_then(|b| b.inputs.get(&key))
                    .cloned();
                if let Some(input) = input {
                    self.cj_stop(&mut state, &input, now, "policy")?;
                }
                self.save(&state)?;
            } else {
                // One unavailable invocation must not prevent cleanup of others.
                let _ = self.finish_runtime(&mut state, &event, &opened, execute, &key, true, now);
                self.save(&state)?;
            }
            advanced += 1;
        }
        Ok(advanced)
    }
}
