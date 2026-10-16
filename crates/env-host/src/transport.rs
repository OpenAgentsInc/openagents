//! One durable CJ book per allocator, with domain mutation under the same fence.
use crate::{
    Result,
    allocator::{Allocator, Bundle, State},
};
use nostr::{
    contracts,
    domain::{Event, Tag},
    execution,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub type Reply = (Event, BTreeMap<String, Vec<u8>>);
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Transport {
    service: execution::Service,
    replies: BTreeMap<String, Event>,
    files: BTreeMap<String, Vec<Value>>,
    outcomes: BTreeMap<String, Value>,
}
impl Allocator {
    fn ensure_transport(&self, state: &mut State) {
        state.transport.get_or_insert_with(|| Transport {
            service: execution::Service::new(self.pubkey(), 1, 9_007_199_254_740_991),
            replies: BTreeMap::new(),
            files: BTreeMap::new(),
            outcomes: BTreeMap::new(),
        });
    }
    fn bundle(&self, state: &State) -> Result<Value> {
        let schemas: Value =
            serde_json::from_slice(nostr::environment::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let schema = |v: Value| {
            self.store.put(
                &contracts::jcs(&v).map_err(|e| e.to_string())?,
                "application/schema+json",
                None,
            )
        };
        let input = schema(
            json!({"$defs":schemas["$defs"],"oneOf":[{"$ref":"#/$defs/environment-request"},{"$ref":"#/$defs/environment-control"},{"$ref":"#/$defs/environment-attach"},{"$ref":"#/$defs/environment-participant-admission"},{"$ref":"#/$defs/environment-attachment-control"},{"$ref":"#/$defs/environment-fencing"}]}),
        )?;
        let output = schema(json!({"$defs":schemas["$defs"],"$ref":"#/$defs/environment-result"}))?;
        let policy=self.store.put_json(&json!({"v":"openagents.environment-controller-policy.v1","requires":[],"allocator":self.pubkey(),"provider":self.provider_binding(),"policy":state.policy}))?;
        let id = format!("{}:environment/controller", self.pubkey());
        let provider = self.provider_binding();
        let effects = json!({"reads":["environment"],"writes":["environment"],"network":if provider=="environment/local-directory"{json!([])}else{json!(["environment-provider"])},"process":provider!="environment/local-directory","delegates":false,"spend":false});
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Manage admitted environment leases and attachments.","input":input,"output":output,"effects":effects,"minimum":{"output_bytes":1048576},"support":{"bounds":{"output_bytes":"enforced"},"cancellation":"unsupported","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"environment-controller","interface":"openagents.environment-controller.v1"}});
        nostr::cap::parse_definition(&card).map_err(|e| e.to_string())?;
        let target = json!({"id":id,"artifact":self.store.put(&contracts::jcs(&card).map_err(|e|e.to_string())?,"application/json",Some("openagents.cap.v1"))?});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":policy["digest"].as_str().ok_or("policy")?.trim_start_matches("sha256:"),"recipient":self.pubkey(),"policy":policy,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.environment-controller-requirements.v1","requires":[],"policy":policy,"operation":"lease-and-attachment-management"}))?;
        Ok(json!({"target":target,"lock":lock,"context":context,"requirements":requirements}))
    }
    pub fn transport(&self) -> Result<Value> {
        let _guard = self.store.lock()?;
        self.bundle(&self.state()?)
    }
    fn check_cj(
        &self,
        state: &State,
        opened: &execution::Opened,
        execute: &execution::Execute,
    ) -> Result<()> {
        let bundle = self.bundle(state)?;
        let mut pins = BTreeMap::new();
        for field in ["target", "lock", "context", "requirements"] {
            if execute.payload[field] != bundle[field] {
                return Err("identity_mismatch: environment endpoint pins differ".into());
            }
            let r = if field == "target" {
                &bundle[field]["artifact"]
            } else {
                &bundle[field]
            };
            let bytes = self.store.get(r)?;
            pins.insert(contracts::digest_bytes(&bytes), bytes);
        }
        if execute.input_artifact.is_some()
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute.bounds.output_bytes != Some(1048576)
            || execute.bounds.wall_ms.is_some()
        {
            return Err("cannot_enforce: environment endpoint bounds differ".into());
        }
        if !self.cj_principal(state, &opened.principal, &execute.input) {
            return Err("not_admitted: environment participant is not authorized".into());
        }
        nostr::environment::validate(&execute.input).map_err(|e| e.to_string())?;
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))
    }
    pub(crate) fn authenticate_cj(
        &self,
        state: &mut State,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value, Value)> {
        let opened = execution::open_request(
            event,
            self.pubkey(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("environment execute required".into());
        };
        let book = state
            .transport
            .as_ref()
            .ok_or("not_admitted: use the durable environment endpoint")?;
        let key = book
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = book
            .service
            .claims
            .get(&key)
            .ok_or("not_admitted: invocation was not admitted")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || !claim.dispatch_intent
            || claim.result.is_some()
        {
            return Err("revoked: environment invocation was fenced".into());
        }
        self.check_cj(state, &opened, execute)?;
        let reference = self.note_cj_input(state, event, &execute.input, &opened.principal)?;
        Ok((opened.principal, execute.input.clone(), reference))
    }
    pub(crate) fn remember_cj(&self, state: &mut State, event: &Event, value: &Value) {
        if event.kind == execution::REQUEST_KIND {
            if let Some(book) = &mut state.transport {
                book.outcomes.insert(event.id.clone(), value.clone());
            }
        }
    }
    fn cached_files(&self, state: &State, key: &str) -> Result<BTreeMap<String, Vec<u8>>> {
        state
            .transport
            .as_ref()
            .ok_or("transport")?
            .files
            .get(key)
            .ok_or("reply files")?
            .iter()
            .map(|r| {
                let b = self.store.get(r)?;
                Ok((contracts::digest_bytes(&b), b))
            })
            .collect()
    }
    fn finish_cj(
        &self,
        state: &mut State,
        event: &Event,
        opened: &execution::Opened,
        execute: &execution::Execute,
        value: &Value,
        now: u64,
    ) -> Result<Reply> {
        let key = state.transport.as_ref().ok_or("transport")?.service.key(
            &opened.principal,
            &execute.request,
            execute.attempt,
        );
        if let Some(reply) = state
            .transport
            .as_ref()
            .ok_or("transport")?
            .replies
            .get(&key)
        {
            return Ok((reply.clone(), self.cached_files(state, &key)?));
        }
        let mut refs = state.transport.as_ref().ok_or("transport")?.files[&key].clone();
        refs.truncate(3);
        let mut records = refs
            .iter()
            .map(|r| self.store.json(r))
            .collect::<Result<Vec<_>>>()?;
        let output = self.store.put_json(value)?;
        refs.push(output.clone());
        // Return only artifacts named by this authorized operation's result.
        // Referenced input, policy, claim and arbitrary nested hashes grant no read.
        for field in ["lease", "state"] {
            if contracts::parse_artifact(&value[field]).is_ok() {
                refs.push(value[field].clone());
            }
        }
        let resolved=nostr::run::append_envelope(&mut records,self.pubkey(),"resolved",json!({"outcome":"completed","dispatched":true,"output":output,"receipts":[],"usage":{"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now).map_err(|e|e.to_string())?;
        let settled=nostr::run::append_envelope(&mut records,self.pubkey(),"settled",json!({"outcome":"completed","unknowns":[],"verification":"not_run","integration":"not_requested","results":[output],"reservations":[]}),now).map_err(|e|e.to_string())?;
        let terminal = self.store.put_json(&settled)?;
        refs.extend([self.store.put_json(&resolved)?, terminal.clone()]);
        if refs
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(r["size"].as_u64()?))
            .is_none_or(|n| n > 1048576)
        {
            return Err("unknown: environment response exceeds its byte limit".into());
        }
        let book = state.transport.as_mut().ok_or("transport")?;
        let result = book
            .service
            .observe(
                &key,
                execution::Report {
                    outcome: "completed",
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
        let peer = opened
            .principal
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "caller")?;
        let reply = execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            execution::RESULT_KIND,
            vec![
                Tag::new(vec!["e".into(), event.id.clone()]),
                Tag::new(vec!["p".into(), opened.principal.clone()]),
            ],
            &result,
        )
        .map_err(|e| format!("{e:?}"))?;
        book.files.insert(key.clone(), refs);
        book.replies.insert(key.clone(), reply.clone());
        self.save(state)?;
        Ok((reply, self.cached_files(state, &key)?))
    }
    pub fn serve(
        &self,
        event: &Event,
        artifacts: &BTreeMap<String, Vec<u8>>,
        declarations: &[Event],
        now: u64,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let opened = execution::open_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("environment execute required".into());
        };
        let key;
        {
            let _guard = self.store.lock()?;
            let mut state = self.state()?;
            self.ensure_transport(&mut state);
            let book = state.transport.as_ref().ok_or("transport")?;
            key = book
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            if let Some(reply) = book.replies.get(&key) {
                let claim = &book.service.claims[&key];
                if claim.fingerprint != execute.fingerprint
                    || claim.execute_event != event.id
                    || now > claim.retain_until
                {
                    return Err(
                        "idempotency_conflict: changed or expired environment invocation".into(),
                    );
                }
                return Ok((reply.clone(), self.cached_files(&state, &key)?));
            }
            if book.service.claims.contains_key(&key) {
                return Err(
                    "unknown: interrupted environment operation requires recovery; no replay"
                        .into(),
                );
            }
            execution::open_request(
                event,
                self.pubkey(),
                &self.secret,
                now,
                execution::Window::DEFAULT,
            )
            .map_err(|e| format!("{e:?}"))?;
            self.check_cj(&state, &opened, execute)?;
            let book = state.transport.as_mut().ok_or("transport")?;
            if book.service.claims.len() >= 4096
                || book.service.claims.values().any(|c| c.run == execute.run)
                || execute.input["run"] == execute.run
            {
                return Err("conflict: environment RUN identity or retention bound".into());
            }
            let (claim, root) = match book
                .service
                .prepare(&opened, now, &workspace_host::artifact::token())
                .map_err(|e| format!("{e:?}"))?
            {
                execution::Admission::Reserved { claim, root } => (claim, root),
                _ => return Err("unexpected retransmission".into()),
            };
            let root_ref =
                self.store
                    .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
            let input = self.store.put_json(&execute.input)?;
            let mut records = vec![contracts::parse_strict(&root).map_err(|e| e.to_string())?];
            let admitted=nostr::run::append_envelope(&mut records,self.pubkey(),"admitted",json!({"enforcement":[],"reservations":[],"deadlines":{"request":execute.deadline},"retention":execute.retain_until,"input":input}),now).map_err(|e|e.to_string())?;
            let dispatched=nostr::run::append_envelope(&mut records,self.pubkey(),"dispatched",json!({"binding":execute.payload["target"],"attempt":execute.attempt,"input":input,"context":execute.payload["context"],"effect":"environment-management","generation":0}),now).map_err(|e|e.to_string())?;
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
            self.save(&state)?;
        }
        let value = self.submit(
            &Bundle {
                event: event.clone(),
                artifacts: artifacts.clone(),
                declarations: declarations.to_vec(),
            },
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let now = now.saturating_add(started.elapsed().as_secs());
        if now >= execute.deadline {
            return Err("unknown: response deadline passed; recover the saved result".into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.finish_cj(&mut state, event, &opened, execute, &value, now)
    }
    /// Recover an exact saved domain result; an interrupted provider stays unknown.
    /// Never call submit, materialize, create or destroy on this path.
    pub fn recover(&self, event: &Event, now: u64) -> Result<Reply> {
        let opened = execution::open_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err("environment execute required".into());
        };
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let book = state
            .transport
            .as_ref()
            .ok_or("no environment invocation")?;
        let key = book
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        let claim = book
            .service
            .claims
            .get(&key)
            .ok_or("no environment invocation")?;
        if claim.execute_event != event.id
            || claim.fingerprint != execute.fingerprint
            || now > claim.retain_until
        {
            return Err("idempotency_conflict: recovery identity differs".into());
        }
        if let Some(reply) = book.replies.get(&key) {
            return Ok((reply.clone(), self.cached_files(&state, &key)?));
        }
        let known = book.outcomes.get(&event.id).cloned();
        let value = if let Some(value) = known {
            value
        } else {
            self.interrupted_cj(&mut state, &opened.principal, &execute.input, now)?
        };
        self.finish_cj(&mut state, event, &opened, execute, &value, now)
    }
}

/// Build a management execute with exact endpoint pins; discovery grants no rights.
pub fn invoke(
    secret: &secp256k1::SecretKey,
    allocator: &str,
    bundle: &Value,
    input: Value,
    request: String,
    now: u64,
) -> Result<Event> {
    let signer = nostr::domain::RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let peer = allocator
        .parse::<secp256k1::XOnlyPublicKey>()
        .map_err(|_| "allocator key")?;
    let deadline = now.checked_add(300).ok_or("deadline")?;
    let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":workspace_host::artifact::token(),"target":bundle["target"],"lock":bundle["lock"],"input":input,"context":bundle["context"],"requirements":bundle["requirements"],"bounds":{"jobs":1,"output_bytes":1048576,"spend_microunits":0},"deadline":deadline,"retain_until":deadline+86400});
    execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(secret, &peer),
        nonce: secp256k1::rand::random(),
        created_at: now,
    }
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), allocator.into()]),
            Tag::new(vec!["expiration".into(), deadline.to_string()]),
        ],
        &payload,
    )
    .map_err(|e| format!("{e:?}"))
}
