//! A fixed initializer has its own original CJ request and shared COORD charge.
use super::*;
use nostr::execution;

impl Allocator {
    fn bootstrap_bundle(
        &self,
        state: &State,
        claim: &Value,
        admission: &Event,
        request: &str,
    ) -> Result<Value> {
        contracts::parse_artifact(claim).map_err(contract)?;
        nostr::coordination::id(&json!(request)).map_err(contract)?;
        let schema = |v: Value| {
            self.store.put(
                &contracts::jcs(&v).map_err(contract)?,
                "application/schema+json",
                None,
            )
        };
        let defs: Value = serde_json::from_slice(env::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let input = schema(
            json!({"type":"object","additionalProperties":false,"required":["v","requires","owner","request","components"],"properties":{"v":{"const":"openagents.environment-native-bootstrap-input.v1"},"requires":{"const":[]},"owner":{"type":"string","pattern":"^[a-f0-9]{64}$"},"request":{"type":"string","pattern":"^[a-f0-9]{64}$"},"components":{"type":"array","minItems":1,"maxItems":64,"items":{"type":"object","additionalProperties":false,"required":["id","artifact"],"properties":{"id":{"type":"string","pattern":"^[a-z0-9_-]{1,64}$"},"artifact":defs["$defs"]["artifact"]}}}}}),
        )?;
        let output = schema(
            json!({"type":"object","additionalProperties":false,"required":["v","requires","lease","request","manifest","accounting"],"properties":{"v":{"const":"openagents.environment-native-bootstrap-result.v1"},"requires":{"const":[]},"lease":defs["$defs"]["artifact"],"request":{"type":"string","pattern":"^[a-f0-9]{64}$"},"manifest":defs["$defs"]["artifact"],"accounting":defs["$defs"]["artifact"]}}),
        )?;
        let policy=self.store.put_json(&json!({"v":"openagents.environment-bootstrap-policy.v1","requires":[],"allocator":self.pubkey(),"provider":self.provider_binding(),"policy":state.policy}))?;
        let id = format!("{}:environment/bootstrap-manifest", self.pubkey());
        let effects = json!({"reads":["environment"],"writes":["environment"],"network":[],"process":false,"delegates":false,"spend":false});
        let card = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Initialize an environment from the selected components.","input":input,"output":output,"effects":effects,"minimum":{"output_bytes":1048576},"support":{"bounds":{"output_bytes":"enforced"},"cancellation":"unsupported","idempotency":"request_attempt","evidence":[]},"binding_contract":{"operation":"environment-bootstrap-manifest","interface":"openagents.environment-native-bootstrap.v1"}});
        nostr::cap::parse_definition(&card).map_err(contract)?;
        let target = json!({"id":id,"artifact":self.store.put(&jcs(&card).map_err(contract)?,"application/json",Some("openagents.cap.v1"))?});
        let lock=self.store.put_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":id,"definition":target,"dependencies":[]}]}))?;
        let context=self.store.put_json(&json!({"v":"openagents.context.v1","requires":[],"task":policy["digest"].as_str().ok_or("policy")?.trim_start_matches("sha256:"),"recipient":self.pubkey(),"policy":policy,"entries":[],"omissions":[],"coverage":"complete"}))?;
        let requirements=self.store.put_json(&json!({"v":"openagents.environment-native-bootstrap-requirements.v1","requires":[],"claim":claim,"admission":admission,"request":request}))?;
        Ok(
            json!({"target":target,"lock":lock,"context":context,"requirements":requirements,"effects":effects}),
        )
    }
    /// Inert local distribution. The actual operation verifies the original owner
    /// and current budget authority before initialization.
    pub fn bootstrap_binding(
        &self,
        claim: &Value,
        admission: &Event,
        request: &str,
    ) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let bundle = self.bootstrap_bundle(&state, claim, admission, request)?;
        for key in ["target", "lock", "context", "requirements"] {
            let r = if key == "target" {
                &bundle[key]["artifact"]
            } else {
                &bundle[key]
            };
            self.remember_input(&mut state, &admission.pubkey, r)?;
        }
        self.save(&state)?;
        Ok(bundle)
    }
    pub(super) fn validate_bootstrap_plan(
        &self,
        state: &State,
        definition: &Value,
        request: &Value,
    ) -> Result<()> {
        if !self.provider.borrow().manifest_bootstrap() || self.claims.is_none() {
            return Err(refuse(
                "unsupported_feature",
                "native bootstrap provider and current claim authority required",
            ));
        }
        let plan = &definition["bootstrap"];
        if !state
            .known
            .get(request["owner"].as_str().ok_or("owner")?)
            .is_some_and(|k| k.contains(digest_of(&plan["requirements"])))
        {
            return Err(refuse(
                "not_admitted",
                "bootstrap requirements were not supplied or shared",
            ));
        }
        let requirements =
            contracts::parse_strict(&self.bytes(&plan["requirements"])?).map_err(contract)?;
        if requirements["v"] != "openagents.environment-native-bootstrap-requirements.v1"
            || requirements["requires"] != json!([])
            || requirements.as_object().is_none_or(|o| o.len() != 5)
        {
            return Err(refuse("cannot_enforce", "bootstrap requirements differ"));
        }
        let admission: Event = serde_json::from_value(requirements["admission"].clone())
            .map_err(|_| "malformed: bootstrap allowance")?;
        let bundle = self.bootstrap_bundle(
            state,
            &requirements["claim"],
            &admission,
            requirements["request"]
                .as_str()
                .ok_or("bootstrap request")?,
        )?;
        for key in ["target", "lock", "context", "requirements", "effects"] {
            if plan[key] != bundle[key] {
                return Err(refuse("identity_mismatch", "native bootstrap pins differ"));
            }
        }
        if !state
            .known
            .get(request["owner"].as_str().ok_or("owner")?)
            .is_some_and(|k| k.contains(digest_of(&plan["input"])))
        {
            return Err(refuse(
                "not_admitted",
                "bootstrap input was not supplied or shared",
            ));
        }
        let input = contracts::parse_strict(&self.bytes(&plan["input"])?).map_err(contract)?;
        let wanted = Self::bootstrap_components(definition)?;
        if input["v"] != "openagents.environment-native-bootstrap-input.v1"
            || input["requires"] != json!([])
            || input["owner"] != request["owner"]
            || input["request"] != request["request"]
            || input["components"] != wanted
            || input.as_object().is_none_or(|o| o.len() != 5)
        {
            return Err(refuse(
                "identity_mismatch",
                "bootstrap input differs from the admitted resource",
            ));
        }
        let bytes = wanted
            .as_array()
            .ok_or("components")?
            .iter()
            .try_fold(self.bytes(&plan["input"])?.len() as u64, |n, c| {
                n.checked_add(c["artifact"]["size"].as_u64()?)
            })
            .ok_or("bootstrap byte bound")?;
        if bytes > 1048576 {
            return Err(refuse("limit_exceeded", "bootstrap component read ceiling"));
        }
        nostr::coordination::bounds_valid(&plan["bounds"]).map_err(contract)?;
        if env::ceiling(&plan["bounds"], "calls") != Some(1)
            || env::ceiling(&plan["bounds"], "wall_ms").is_none_or(|n| n == 0 || n > 30000)
            || env::ceiling(&plan["bounds"], "input_bytes") != Some(1048576)
            || env::ceiling(&plan["bounds"], "output_bytes") != Some(1048576)
            || plan["bounds"].as_array().is_none_or(|a| a.len() != 4)
        {
            return Err(refuse("cannot_enforce", "bootstrap bounds differ"));
        }
        for bound in plan["bounds"].as_array().ok_or("bounds")? {
            let mechanism = match bound["bound"].as_str() {
                Some("calls") => "native-ledger",
                Some("wall_ms") => "native-response-deadline",
                Some("input_bytes" | "output_bytes") => "native-bytes",
                _ => return Err("unsupported bootstrap bound".into()),
            };
            if bound["assurance"] != "host" || bound["mechanism"] != mechanism {
                return Err(refuse(
                    "cannot_enforce",
                    "bootstrap enforcement mechanism differs",
                ));
            }
            if let Some(ceiling) =
                env::ceiling(&request["bounds"], bound["bound"].as_str().ok_or("bound")?)
            {
                if bound["ceiling"].as_u64().is_none_or(|n| n > ceiling) {
                    return Err(refuse(
                        "limit_exceeded",
                        "bootstrap exceeds the environment request",
                    ));
                }
            }
        }
        Ok(())
    }
    fn bootstrap_components(definition: &Value) -> Result<Value> {
        let mut components = definition["components"]
            .as_array()
            .ok_or("components")?
            .iter()
            .map(|c| json!({"id":c["id"],"artifact":c["artifact"]}))
            .collect::<Vec<_>>();
        if !definition["workspace"].is_null() {
            components.push(json!({"id":"workspace","artifact":definition["workspace"]}));
        }
        components.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        if components.is_empty()
            || components.len() > 64
            || components
                .windows(2)
                .any(|pair| pair[0]["id"] == pair[1]["id"])
        {
            return Err(refuse(
                "cannot_enforce",
                "bootstrap component identities differ",
            ));
        }
        Ok(json!(components))
    }
    fn bootstrap_receipt(
        &self,
        lease: &Lease,
        execute: &execution::Execute,
        input: &Value,
        bytes: &[u8],
    ) -> Result<Value> {
        let receipt = contracts::parse_strict(bytes).map_err(contract)?;
        let resource = lease.realized.as_ref().ok_or("resource")?;
        if receipt["v"] != "openagents.environment-native-bootstrap-provider-receipt.v1"
            || receipt["requires"] != json!([])
            || receipt["request"] != execute.request
            || receipt["resource"] != resource.resource
            || receipt["generation"] != resource.generation
            || receipt["input"] != contracts::digest_value(input).map_err(contract)?
            || receipt["components"] != input["components"]
            || receipt["work_ms"]
                .as_u64()
                .is_none_or(|n| n == 0 || Some(n) > execute.bounds.wall_ms)
            || receipt["finished_at"].as_u64().is_none_or(|n| {
                n >= execute.deadline
                    || n < lease
                        .bootstrap
                        .as_ref()
                        .and_then(|b| b["started_at"].as_u64())
                        .unwrap_or(0)
            })
            || receipt["initialized"]
                .as_str()
                .is_none_or(|s| !s.starts_with("sha256:") || s.len() != 71)
            || receipt.as_object().is_none_or(|o| o.len() != 10)
        {
            return Err(refuse(
                "identity_mismatch",
                "original bootstrap receipt differs",
            ));
        }
        self.store.put_json(&receipt)
    }
    pub(super) fn run_bootstrap(
        &self,
        state: &mut State,
        lease: &mut Lease,
        event: &Event,
        now: u64,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let definition = self.body(
            &lease.request["definition"],
            "openagents.environment-definition.v1",
        )?;
        self.validate_bootstrap_plan(state, &definition, &lease.request)?;
        let plan = &definition["bootstrap"];
        let opened = execution::open_request(
            event,
            self.pubkey(),
            &self.secret,
            event.created_at,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = &opened.body else {
            return Err(refuse("not_admitted", "bootstrap execute required"));
        };
        if opened.principal != lease.owner
            || !state.policy.owners.contains(&opened.principal)
            || execute.payload["input"]["artifact"] != plan["input"]
            || execute.attempt != 1
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits != Some(0)
            || execute.bounds.output_bytes != Some(1048576)
            || execute
                .bounds
                .wall_ms
                .is_none_or(|n| n == 0 || Some(n) != env::ceiling(&plan["bounds"], "wall_ms"))
            || execute.deadline > lease.effective_expiry()
        {
            return Err(refuse(
                "cannot_enforce",
                "bootstrap caller, identity, time, or bounds differ",
            ));
        }
        for key in ["target", "lock", "context", "requirements"] {
            if execute.payload[key] != plan[key] {
                return Err(refuse("identity_mismatch", "bootstrap execute pins differ"));
            }
        }
        let mut pins = BTreeMap::new();
        for key in ["target", "lock", "input", "context", "requirements"] {
            let r = if key == "target" {
                &plan[key]["artifact"]
            } else {
                &plan[key]
            };
            let bytes = self.bytes(r)?;
            pins.insert(digest_bytes(&bytes), bytes);
        }
        execution::require_pins(execute, &pins).map_err(|e| format!("{e:?}"))?;
        let input = contracts::parse_strict(&self.bytes(&plan["input"])?).map_err(contract)?;
        let requirements =
            contracts::parse_strict(&self.bytes(&plan["requirements"])?).map_err(contract)?;
        if requirements["request"] != execute.request {
            return Err(refuse(
                "identity_mismatch",
                "bootstrap accounting request differs",
            ));
        }
        self.ensure_transport(state);
        let key = state.transport.as_ref().ok_or("transport")?.service.key(
            &opened.principal,
            &execute.request,
            execute.attempt,
        );
        if let Some(previous) = &lease.bootstrap {
            if previous["event"] != serde_json::to_value(event).map_err(|e| e.to_string())? {
                return Err(refuse("idempotency_conflict", "bootstrap identity changed"));
            }
            if previous["terminal"].is_object() {
                if previous["success"] != true {
                    return Err("not_admitted: original bootstrap did not succeed".into());
                }
                return Ok(previous["terminal"].clone());
            }
            if state
                .transport
                .as_ref()
                .ok_or("transport")?
                .files
                .get(&key)
                .is_some_and(|r| r.len() == 2)
            {
                self.refuse_bootstrap(
                    state,
                    lease,
                    event,
                    &opened,
                    execute,
                    &key,
                    "unavailable",
                    now,
                )?;
                return Err(
                    "not_admitted: interrupted bootstrap was fenced before initialization".into(),
                );
            }
            // Recovery observes only the original marker. No placement, admission,
            // initializer, or allowance is repeated after an unknown effect.
            let resource = lease.realized.as_ref().ok_or("resource")?;
            let bytes = self
                .provider
                .borrow_mut()
                .bootstrap_result(resource, &execute.request)?
                .ok_or("unknown: original bootstrap result is still unobserved")?;
            let manifest = self.bootstrap_receipt(lease, execute, &input, &bytes)?;
            let admission: Event = serde_json::from_value(requirements["admission"].clone())
                .map_err(|_| "bootstrap allowance")?;
            let charge = self
                .claims
                .as_ref()
                .ok_or("claim authority")?
                .bootstrap_receipt(&admission, &lease.owner, now)?;
            let accounting = self.store.put_json(&charge)?;
            lease.bootstrap.as_mut().ok_or("bootstrap")?["accounting"] = accounting.clone();
            let value = json!({"v":"openagents.environment-native-bootstrap-result.v1","requires":[],"lease":lease.lease_ref,"request":execute.request,"manifest":manifest,"accounting":accounting});
            state.leases.insert(lease.id.clone(), lease.clone());
            let reply = self.finish_cj(state, event, &opened, execute, &value, now)?;
            return self.retain_bootstrap(state, lease, execute, &key, reply, now);
        }
        execution::open_request(
            event,
            self.pubkey(),
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let allocation_run_used = state.leases.values().any(|l| l.run == execute.run);
        let book = state.transport.as_mut().ok_or("transport")?;
        if allocation_run_used
            || book.service.claims.len() >= 4096
            || book.service.claims.values().any(|c| c.run == execute.run)
            || execute.run == lease.run
        {
            return Err(refuse(
                "conflict",
                "bootstrap RUN identity or retention bound",
            ));
        }
        let (claim, root) = match book
            .service
            .prepare(&opened, now, &token())
            .map_err(|e| format!("{e:?}"))?
        {
            execution::Admission::Reserved { claim, root } => (claim, root),
            _ => return Err(refuse("conflict", "bootstrap request already admitted")),
        };
        let root_ref =
            self.store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
        let mut records = vec![contracts::parse_strict(&root).map_err(contract)?];
        let admitted=nostr::run::append_envelope(&mut records,self.pubkey(),"admitted",json!({"enforcement":plan["bounds"],"reservations":[lease.reservation_ref],"deadlines":{"request":execute.deadline},"retention":execute.retain_until,"input":plan["input"]}),now).map_err(contract)?;
        book.files
            .insert(key.clone(), vec![root_ref, self.store.put_json(&admitted)?]);
        book.service
            .acknowledge(&claim.key, &root)
            .map_err(|e| format!("{e:?}"))?;
        book.service
            .intend(&claim.key)
            .map_err(|e| format!("{e:?}"))?;
        self.note_cj_input(state, event, &input, &opened.principal)?;
        lease.bootstrap = Some(json!({"event":event,"terminal":null,"started_at":now}));
        state.leases.insert(lease.id.clone(), lease.clone());
        self.save(state)?;
        state.leases.remove(&lease.id);
        let admission: Event = serde_json::from_value(requirements["admission"].clone())
            .map_err(|_| "bootstrap allowance")?;
        let resource = lease.realized.clone().ok_or("resource")?;
        let claims = self.claims.as_ref().ok_or("claim authority")?;
        let mut entered = false;
        let applied = claims.bootstrap(
            &admission,
            &lease.owner,
            &requirements["claim"],
            &lease.request["reservation"],
            &execute.request,
            &contracts::digest_value(&input).map_err(contract)?,
            &lease.request["frame"],
            execute.deadline,
            now,
            &mut || {
                if now.saturating_add(started.elapsed().as_secs()) >= execute.deadline {
                    return Err("stale: bootstrap expired before initialization".into());
                }
                entered=true;
                let dispatched=nostr::run::append_envelope(&mut records,self.pubkey(),"dispatched",json!({"binding":plan["target"],"attempt":1,"input":plan["input"],"context":plan["context"],"effect":"native-manifest-initialization","generation":0}),now).map_err(contract)?;
                state.transport.as_mut().ok_or("transport")?.files.get_mut(&key).ok_or("bootstrap files")?.push(self.store.put_json(&dispatched)?);
                state.leases.insert(lease.id.clone(),lease.clone());self.save(state)?;state.leases.remove(&lease.id);
                let bytes =
                    self.provider
                        .borrow_mut()
                        .bootstrap(&resource, &execute.request, &input)?;
                Ok(json!(bytes))
            },
        );
        let (charge, bytes) = match applied {
            Ok(result) => result,
            Err(error) => {
                if !entered {
                    self.refuse_bootstrap(
                        state,
                        lease,
                        event,
                        &opened,
                        execute,
                        &key,
                        code(&error),
                        now,
                    )?;
                }
                return Err(error);
            }
        };
        let bytes: Vec<u8> =
            serde_json::from_value(bytes).map_err(|_| "bootstrap receipt bytes")?;
        let manifest = self.bootstrap_receipt(lease, execute, &input, &bytes)?;
        let accounting = self.store.put_json(&charge)?;
        lease.bootstrap.as_mut().ok_or("bootstrap")?["accounting"] = accounting.clone();
        let now = now.saturating_add(started.elapsed().as_secs());
        if now >= execute.deadline
            || started.elapsed().as_millis()
                > u128::from(execute.bounds.wall_ms.ok_or("wall bound")?)
        {
            return Err("unknown: bootstrap response exceeded its deadline".into());
        }
        let value = json!({"v":"openagents.environment-native-bootstrap-result.v1","requires":[],"lease":lease.lease_ref,"request":execute.request,"manifest":manifest,"accounting":accounting});
        state.leases.insert(lease.id.clone(), lease.clone());
        let reply = self.finish_cj(state, event, &opened, execute, &value, now)?;
        self.retain_bootstrap(state, lease, execute, &key, reply, now)
    }
    fn retain_bootstrap(
        &self,
        state: &mut State,
        lease: &mut Lease,
        execute: &execution::Execute,
        key: &str,
        reply: crate::transport::Reply,
        now: u64,
    ) -> Result<Value> {
        let result = state.transport.as_ref().ok_or("transport")?.service.claims[key]
            .result
            .as_ref()
            .ok_or("bootstrap result")?;
        let terminal =
            nostr::run::terminal_from_result(result, &reply.1, self.pubkey()).map_err(contract)?;
        if result["outcome"] != "completed" || result["run"] != execute.run {
            return Err("unknown: bootstrap did not complete".into());
        }
        let output = self.store.json(&result["output"])?;
        if output["v"] != "openagents.environment-native-bootstrap-result.v1"
            || output["lease"] != lease.lease_ref
            || output["request"] != execute.request
        {
            return Err("identity_mismatch: bootstrap terminal result differs".into());
        }
        lease.bootstrap.as_mut().ok_or("bootstrap intent")?["terminal"] = terminal.clone();
        lease.bootstrap.as_mut().ok_or("bootstrap intent")?["success"] = json!(true);
        state.leases.insert(lease.id.clone(), lease.clone());
        self.save(state)?;
        state.leases.remove(&lease.id);
        let _ = now;
        Ok(terminal)
    }
}

impl Allocator {
    fn refuse_bootstrap(
        &self,
        state: &mut State,
        lease: &mut Lease,
        event: &Event,
        opened: &execution::Opened,
        execute: &execution::Execute,
        key: &str,
        reason: &str,
        now: u64,
    ) -> Result<()> {
        let mut refs = state.transport.as_ref().ok_or("transport")?.files[key].clone();
        let mut records = refs
            .iter()
            .map(|r| self.store.json(r))
            .collect::<Result<Vec<_>>>()?;
        if records.iter().any(|r| r["record"]["type"] == "dispatched") {
            return Err("unknown: a dispatched bootstrap cannot be refused as unstarted".into());
        }
        let resolved=nostr::run::append_envelope(&mut records,self.pubkey(),"resolved",json!({"outcome":"failed","dispatched":false,"output":null,"receipts":[],"usage":{"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now).map_err(contract)?;
        let settled=nostr::run::append_envelope(&mut records,self.pubkey(),"settled",json!({"outcome":"failed","unknowns":[],"verification":"not_run","integration":"not_requested","results":[],"reservations":[]}),now).map_err(contract)?;
        let terminal = self.store.put_json(&settled)?;
        refs.extend([self.store.put_json(&resolved)?, terminal.clone()]);
        let book = state.transport.as_mut().ok_or("transport")?;
        let result = book
            .service
            .observe(
                key,
                execution::Report {
                    outcome: "failed",
                    dispatched: false,
                    output: None,
                    artifacts: refs.clone(),
                    receipts: vec![terminal.clone()],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: Some(reason.into()),
                    message: Some("The environment could not be initialized.".into()),
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let peer = opened
            .principal
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "owner")?;
        let response = execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            execution::RESULT_KIND,
            vec![
                nostr::domain::Tag::new(vec!["e".into(), event.id.clone()]),
                nostr::domain::Tag::new(vec!["p".into(), opened.principal.clone()]),
            ],
            &result,
        )
        .map_err(|e| format!("{e:?}"))?;
        book.replies.insert(key.into(), response);
        book.files.insert(key.into(), refs);
        let intent = lease.bootstrap.as_mut().ok_or("bootstrap intent")?;
        intent["terminal"] = terminal;
        intent["success"] = json!(false);
        intent["request"] = json!(execute.request);
        state.leases.insert(lease.id.clone(), lease.clone());
        self.save(state)?;
        state.leases.remove(&lease.id);
        Ok(())
    }
}
