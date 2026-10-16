use super::*;
use coordination_host::book::{Coordinator, Policy as CoordPolicy};
use env_host::{
    allocator::{Allocator, MaterializeSpec, Policy as EnvPolicy},
    local::LocalDirectories,
};
use nostr::execution;

struct Real {
    fx: Fx,
    coordinator: Coordinator,
    scope: Value,
    frame: Value,
    target: Value,
    adapter: Value,
    allocator_root: PathBuf,
    resource_root: PathBuf,
    lease: Value,
    materialization: Value,
    lease_id: String,
}
impl Real {
    fn allocator(&self) -> Allocator {
        Allocator::open(
            self.allocator_root.clone(),
            secret(9),
            Box::new(LocalDirectories::new(self.resource_root.clone()).unwrap()),
        )
        .unwrap()
        .with_claims(Box::new(
            Coordinator::open(self.fx._temp.path().join("coord"), secret(OWNER)).unwrap(),
        ))
    }
    fn host(&self, engine: LoopEngine) -> SessionHost {
        SessionHost::open(
            self.fx.root.clone(),
            secret(HOST),
            vec![Box::new(engine)],
            Box::new(self.allocator()),
        )
        .unwrap()
    }
    fn signed(&self, body: Value) -> nostr::domain::Event {
        workspace_host::host::request(&secret(OWNER), &pk(OWNER), body, self.fx.now).unwrap()
    }
    fn revision(&self) -> Value {
        self.coordinator.scope(&self.scope).unwrap()["revision"].clone()
    }
    fn copy_from(&mut self, store: &workspace_host::artifact::Artifacts, roots: Vec<Value>) {
        let mut queue = roots;
        let mut seen = BTreeSet::new();
        while let Some(reference) = queue.pop() {
            let digest = reference["digest"].as_str().unwrap().to_string();
            if !seen.insert(digest.clone()) {
                continue;
            }
            let bytes = store.get(&reference).unwrap();
            self.coordinator
                .put_local_bytes(&bytes, "application/octet-stream", None)
                .unwrap();
            self.fx.artifacts.insert(digest, bytes.clone());
            if let Ok(body) = nostr::contracts::parse_strict(&bytes) {
                let mut nodes = vec![&body];
                while let Some(value) = nodes.pop() {
                    if nostr::contracts::parse_artifact(value).is_ok() {
                        queue.push(value.clone());
                        continue;
                    }
                    match value {
                        Value::Object(fields) => nodes.extend(
                            fields
                                .iter()
                                .filter(|(k, _)| k.as_str() != "meta")
                                .map(|(_, v)| v),
                        ),
                        Value::Array(items) => nodes.extend(items),
                        _ => {}
                    }
                }
            }
        }
    }
    fn new() -> Self {
        let fx = Fx::new();
        let coordinator =
            Coordinator::initialize(fx._temp.path().join("coord"), secret(OWNER)).unwrap();
        let allocator_root = fx._temp.path().join("allocator");
        let resource_root = fx._temp.path().join("resources");
        Allocator::initialize(
            allocator_root.clone(),
            secret(9),
            EnvPolicy {
                owners: BTreeSet::from([pk(OWNER)]),
                controllers: BTreeSet::new(),
                readers: BTreeSet::from([pk(OWNER)]),
                account: "local".into(),
                max_ttl_seconds: 3600,
                max_lifetime_seconds: 86_400,
            },
        )
        .unwrap();
        let mut r = Self {
            fx,
            coordinator,
            scope: Value::Null,
            frame: Value::Null,
            target: Value::Null,
            adapter: Value::Null,
            allocator_root,
            resource_root,
            lease: Value::Null,
            materialization: Value::Null,
            lease_id: String::new(),
        };
        let host = r.host(LoopEngine::mid_turn());
        r.adapter = host.admit_adapter("loop-mid-turn", r.fx.now).unwrap();
        let description = host.store().json(&r.adapter).unwrap();
        r.target = description["adapter"].clone();
        r.copy_from(host.store(), vec![r.adapter.clone()]);
        r.coordinator
            .register_executor(&r.target, &pk(HOST), &r.fx.artifacts)
            .unwrap();
        let cap = nostr::contracts::parse_strict(&host.store().get(&r.target["artifact"]).unwrap())
            .unwrap();
        let mut effects = cap["effects"].clone();
        effects["delegates"] = json!(true);
        r.scope = r
            .coordinator
            .create_scope(
                CoordPolicy {
                    recipients: BTreeSet::from([pk(OWNER), pk(HOST), pk(STRANGER)]),
                    checkers: BTreeSet::from([pk(STRANGER)]),
                    calls: 16,
                    concurrency: 8,
                    max_claim_seconds: 300,
                    max_container_seconds: None,
                    max_depth: 8,
                    until: r.fx.now + 7200,
                    effects,
                    parent: None,
                },
                vec![],
                vec![r.target.clone()],
                r.fx.now,
            )
            .unwrap();
        r.coordinator
            .control_grants(
                &r.signed(json!({"v":"openagents.coordination-native-control.v1",
            "requires":[],"coordinator":r.scope,"command":workspace_host::artifact::token(),
            "expected_revision":r.revision(),"actor":pk(HOST),"roles":["read","claim"],
            "resources":[],"targets":[],"all":true,"until":r.fx.now+7200,
            "revoke":false,"scope_revoke":false})),
                r.fx.now,
            )
            .unwrap();
        let acceptance = r
            .coordinator
            .put_local_json(
                &json!({"v":"openagents.coordination-native-check-policy.v1",
            "requires":[],"check":"exact-admitted-output"}),
            )
            .unwrap();
        let task = workspace_host::artifact::token();
        let objective = r
            .coordinator
            .put_local_bytes(b"Managed session", "text/plain", None)
            .unwrap();
        let snapshot = r
            .coordinator
            .put_local_json(&json!({"v":"openagents.snapshot.v1","requires":[],
            "scope":"session-test","captured_at":r.fx.now,"sources":[],"coverage":"complete"}))
            .unwrap();
        let precedence = r
            .coordinator
            .put_local_json(&json!({"v":"openagents.fixture-policy.v1","requires":[]}))
            .unwrap();
        let instructions=r.coordinator.put_local_json(&json!({"v":"openagents.instructions.v1","requires":[],
            "task":task,"revision":0,"resolver":{"id":format!("{}:coord/empty",pk(OWNER)),"artifact":precedence},
            "precedence":precedence,"snapshot":snapshot,"entries":[]})).unwrap();
        r.frame = r
            .coordinator
            .put_local_json(&json!({"v":"openagents.task-frame.v1","requires":[],
            "task":task,"owner":pk(OWNER),"controller":pk(OWNER),"revision":0,"previous":null,
            "objective":objective,"origin":"user","constraints":[],"acceptance":[acceptance],
            "snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],
            "unresolved":[],"corrections":[]}))
            .unwrap();
        r.coordinator
            .frame(
                &r.signed(
                    json!({"v":"openagents.coordination-native-frame.v1","requires":[],
            "coordinator":r.scope,"command":workspace_host::artifact::token(),
            "expected_revision":r.revision(),"frame":r.frame}),
                ),
                &r.coordinator.get_local_bytes(&r.frame).unwrap(),
                r.fx.now,
            )
            .unwrap();
        let coord_store =
            workspace_host::artifact::Artifacts::new(r.fx._temp.path().join("coord")).unwrap();
        r.copy_from(&coord_store, vec![r.frame.clone()]);
        let requirements = put_json(
            &mut r.fx.artifacts,
            &json!({"v":"openagents.environment-requirements.v1",
            "requires":[],"recipients":[pk(HOST)],"assurance":"host","opaque_models":true,"bounds":["wall_ms"]}),
        );
        let base = put(
            &mut r.fx.artifacts,
            b"base",
            "application/octet-stream",
            None,
        );
        let engine = description["engine"].clone();
        r.fx.artifacts.insert(
            engine["digest"].as_str().unwrap().into(),
            host.store().get(&engine).unwrap(),
        );
        let configuration = put_json(&mut r.fx.artifacts, &json!({"runtime":"loop"}));
        let definition = put_json(
            &mut r.fx.artifacts,
            &json!({"v":"openagents.environment-definition.v1",
            "requires":[],"components":[{"id":"base","role":"base","artifact":base},
                {"id":"runtime","role":"runtime","artifact":engine}],
            "workspace":null,"bootstrap":null,"runtime":{"id":format!("{}:runtime/loop",pk(HOST)),"artifact":engine},
            "configuration":configuration,"model":null,"requirements":requirements}),
        );
        let allowance = r.coordinator.scope(&r.scope).unwrap()["allowance"].clone();
        let bytes = r.coordinator.get_local_bytes(&allowance).unwrap();
        r.fx.artifacts.insert(digest_bytes(&bytes), bytes);
        let allocator = r.allocator();
        let request = json!({"v":"openagents.environment-request.v1","requires":[],
            "request":workspace_host::artifact::token(),"run":workspace_host::artifact::token(),
            "owner":pk(OWNER),"allocator":pk(9),"definition":definition,"frame":r.frame,
            "policy":precedence,"reservation":allowance,"mode":"create","resource":null,
            "effects":cap["effects"],"bounds":[
                {"bound":"wall_ms","ceiling":600_000,"assurance":"host","mechanism":"env-host"},
                {"bound":"spend_microunits","ceiling":0,"currency":"USD","assurance":"host","mechanism":"env-host"}],
            "ttl_seconds":1800,"idle_seconds":1800,"not_after":r.fx.now+7200,
            "retain_until":r.fx.now+90_000,"cleanup":"destroy"});
        let allocated = allocator
            .submit(&r.env_bundle(OWNER, &request), r.fx.now)
            .unwrap();
        assert_eq!(allocated["status"], "accepted", "{allocated}");
        r.lease = allocated["lease"].clone();
        r.lease_id = allocator.store().json(&r.lease).unwrap()["lease"]
            .as_str()
            .unwrap()
            .into();
        let assurance = put_json(&mut r.fx.artifacts, &json!({"host":"fixture"}));
        allocator
            .store()
            .put_json(&json!({"host":"fixture"}))
            .unwrap();
        allocator
            .materialize(
                &r.lease_id,
                &MaterializeSpec {
                    participants: vec![json!({"principal":pk(HOST),"role":"executor",
                "runtime":{"id":format!("{}:runtime/loop",pk(HOST)),"artifact":engine},
                "model":null,"configuration":configuration,"assurance":assurance})],
                    enforcement: json!([{"bound":"wall_ms","ceiling":600_000,
                "assurance":"host","mechanism":"env-host"}]),
                    evidence: vec![],
                    bootstrap: None,
                },
                r.fx.now,
            )
            .unwrap();
        r.materialization = allocator
            .store()
            .json(&allocator.status(&r.lease_id).unwrap()["state"])
            .unwrap()["materialization"]
            .clone();
        r.copy_from(
            allocator.store(),
            vec![r.lease.clone(), r.materialization.clone()],
        );
        r
    }
    fn env_bundle(&self, signer: u8, body: &Value) -> env_host::allocator::Bundle {
        env_host::allocator::Bundle {
            event: env_host::allocator::envelope(&secret(signer), &pk(9), body, self.fx.now)
                .unwrap(),
            artifacts: self.fx.artifacts.clone(),
            declarations: vec![],
        }
    }
    fn opening(&mut self, host: &SessionHost) -> Value {
        let description = host.store().json(&self.adapter).unwrap();
        let recipient = put_json(&mut self.fx.artifacts, &json!({"worker":pk(HOST)}));
        let model = put_json(&mut self.fx.artifacts, &json!({"model":"loop"}));
        let policy = put_json(&mut self.fx.artifacts, &json!({"owner":pk(OWNER)}));
        let schema = serde_json::to_vec(&LoopEngine::mid_turn().profile().settings_schema).unwrap();
        let schema = put(&mut self.fx.artifacts, &schema, SCHEMA_MEDIA_TYPE, None);
        let settings = put_json(&mut self.fx.artifacts, &json!({"effort":"low"}));
        let requested = put_json(
            &mut self.fx.artifacts,
            &json!({"v":"openagents.session-configuration.v1",
            "requires":[],"adapter":description["adapter"],"recipient":recipient,"model":model,
            "settings_schema":schema,"settings":settings,"policy":policy,"enforcement":null,"bounds":[]}),
        );
        json!({"v":"openagents.session-open.v1","requires":[],"command":self.fx.next(),
            "session":self.fx.next(),"owner":pk(OWNER),"authority":pk(HOST),"workspace":null,
            "lease":self.lease,"materialization":self.materialization,"adapter":self.adapter,
            "requested":requested,"authorization":policy,"source":{"kind":"new","artifact":null},
            "issued_at":self.fx.now,"expires_at":self.fx.now+300})
    }
    fn job(&mut self, body: &Value) -> (nostr::domain::Event, Binding) {
        for bytes in self.fx.artifacts.values() {
            self.coordinator
                .put_local_bytes(bytes, "application/octet-stream", None)
                .unwrap();
        }
        let input = self.coordinator.put_local_json(body).unwrap();
        let frame: Value =
            serde_json::from_slice(&self.coordinator.get_local_bytes(&self.frame).unwrap())
                .unwrap();
        let descriptor: Value =
            serde_json::from_slice(&self.coordinator.get_local_bytes(&self.scope).unwrap())
                .unwrap();
        let cap = nostr::contracts::parse_strict(
            &self
                .coordinator
                .get_local_bytes(&self.target["artifact"])
                .unwrap(),
        )
        .unwrap();
        let lock=self.coordinator.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],
            "root":self.target,"entries":[{"id":self.target["id"],"definition":self.target,"dependencies":[]}]})).unwrap();
        let context = self
            .coordinator
            .put_local_json(&json!({"v":"openagents.context.v1","requires":[],
            "task":frame["task"],"recipient":pk(HOST),"policy":descriptor["policy"],
            "entries":[],"omissions":[],"coverage":"complete"}))
            .unwrap();
        let proposal=self.coordinator.propose(&self.signed(json!({"v":"openagents.task-proposal.v1",
            "requires":[],"task":workspace_host::artifact::token(),"coordinator":self.scope,"frame":self.frame,
            "parent":null,"target":self.target,"lock":lock,"input":input,"context":context,
            "dependencies":[],"effects":cap["effects"],"bounds":[
                {"bound":"calls","ceiling":2,"assurance":"host","mechanism":"native-ledger"}],
            "acceptance":frame["acceptance"][0],"reuse":"never"})),&BTreeMap::new(),self.fx.now).unwrap()["value"].clone();
        let claim=self.coordinator.claim(&self.signed(json!({"v":"openagents.claim-request.v1","requires":[],
            "proposal":proposal,"worker":pk(HOST),"resources":[],"expected_revision":self.revision(),
            "requested_until":self.fx.now+300,"reservation":self.coordinator.scope(&self.scope).unwrap()["allowance"]})),
            self.fx.now).unwrap()["value"].clone();
        let event = self
            .coordinator
            .prepare_environment(
                &self.signed(json!({"v":"openagents.coordination-native-dispatch.v1",
            "requires":[],"coordinator":self.scope,"claim":claim})),
                self.fx.now,
            )
            .unwrap();
        let opened = execution::open_environment_request(
            &event,
            &pk(HOST),
            &secret(HOST),
            self.fx.now,
            execution::Window::DEFAULT,
        )
        .unwrap();
        let execution::Body::Execute(execute) = opened.body else {
            panic!()
        };
        let claim_body: Value =
            serde_json::from_slice(&self.coordinator.get_local_bytes(&claim).unwrap()).unwrap();
        let parent: Value = serde_json::from_slice(
            &self
                .coordinator
                .get_local_bytes(&claim_body["reservation"])
                .unwrap(),
        )
        .unwrap();
        let invocation = put_json(
            &mut self.fx.artifacts,
            &json!({"v":"openagents.reservation.v1",
            "requires":[],"id":workspace_host::artifact::token(),"parent":parent["id"],
            "currency":"USD","ceiling_microunits":0,"spent_microunits":0,"unknown":false}),
        );
        let bytes = self.coordinator.get_local_bytes(&claim).unwrap();
        self.fx.artifacts.insert(digest_bytes(&bytes), bytes);
        let payload = put_json(&mut self.fx.artifacts, &execute.payload);
        let declaration = json!({"v":"openagents.environment-execute.v1","requires":[],"caller":pk(OWNER),
            "event":{"id":event.id,"pubkey":event.pubkey,"kind":event.kind},"body":payload});
        let declaration_ref = put_json(&mut self.fx.artifacts, &declaration);
        let allocator = self.allocator();
        let attach = json!({"v":"openagents.environment-attach.v1","requires":[],"lease":self.lease,
            "expected_revision":allocator.status(&self.lease_id).unwrap()["revision"],
            "materialization":self.materialization,"worker":pk(HOST),"controller":pk(OWNER),"generation":0,
            "execute":declaration_ref,"input_digest":nostr::contracts::digest_value(&execute.payload["input"]).unwrap(),
            "claim":claim,"reservation":invocation});
        let mut bundle = self.env_bundle(OWNER, &attach);
        bundle.declarations.push(
            env_host::allocator::envelope(&secret(OWNER), &pk(9), &declaration, self.fx.now)
                .unwrap(),
        );
        let attached = allocator.submit(&bundle, self.fx.now).unwrap();
        assert_eq!(attached["status"], "accepted", "{attached}");
        let grant_ref = attached["value"].clone();
        let grant = allocator.store().json(&grant_ref).unwrap();
        let mat = allocator.store().json(&self.materialization).unwrap();
        let projection = b"session input";
        let projection_ref = put(
            &mut self.fx.artifacts,
            projection,
            "application/octet-stream",
            None,
        );
        let input = json!({"v":"openagents.environment-participant-input.v1","requires":[],
            "attachment":grant_ref,"participant":pk(HOST),"projection":projection_ref});
        let input_ref = put_json(&mut self.fx.artifacts, &input);
        let admission = env_host::participant::Participant::new(secret(HOST))
            .admit(
                &pk(9),
                &env_host::participant::Admit {
                    grant: &grant,
                    grant_ref: &grant_ref,
                    attach: &attach,
                    input: &input,
                    input_ref: &input_ref,
                    materialization: &mat,
                    materialization_ref: &self.materialization,
                    projection,
                    enforcement: &mat["enforcement"],
                    execute: Some((&event, &execute.payload)),
                },
                self.fx.now,
            )
            .unwrap();
        let admitted = allocator
            .submit(
                &env_host::allocator::Bundle {
                    event: admission,
                    artifacts: self.fx.artifacts.clone(),
                    declarations: vec![],
                },
                self.fx.now,
            )
            .unwrap();
        assert_eq!(admitted["status"], "accepted", "{admitted}");
        self.attachment_control(&allocator, &grant_ref, "activate");
        let binding = Binding {
            attachment: grant["attachment"].as_str().unwrap().into(),
            fingerprint: execute.fingerprint,
            grant: grant_ref,
        };
        (event, binding)
    }
    fn attachment_control(&self, allocator: &Allocator, grant: &Value, action: &str) {
        let body = json!({"v":"openagents.environment-attachment-control.v1","requires":[],
            "attachment":grant,"command":workspace_host::artifact::token(),
            "expected_revision":allocator.status(&self.lease_id).unwrap()["revision"],
            "action":action,"evidence":[]});
        let result = allocator
            .submit(&self.env_bundle(OWNER, &body), self.fx.now)
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
    }
    fn release_attachment(&self, grant: &Value) {
        let allocator = self.allocator();
        self.attachment_control(&allocator, grant, "drain");
        let event = env_host::participant::Participant::new(secret(HOST))
            .fence(&pk(9), grant, true, self.fx.now)
            .unwrap();
        assert_eq!(
            allocator
                .submit(
                    &env_host::allocator::Bundle {
                        event,
                        artifacts: BTreeMap::new(),
                        declarations: vec![],
                    },
                    self.fx.now
                )
                .unwrap()["status"],
            "accepted"
        );
    }
    fn submit_command(&mut self, host: &SessionHost, opening: &Value, steps: u64) -> Value {
        let session = opening["session"].as_str().unwrap();
        let authorization = put_json(&mut self.fx.artifacts, &json!({"owner":pk(OWNER)}));
        let input = put_json(
            &mut self.fx.artifacts,
            &json!({"prompt":"once","steps":steps}),
        );
        let context = put_json(&mut self.fx.artifacts, &json!({"session":session}));
        let id = self.fx.next();
        let intent = put_json(
            &mut self.fx.artifacts,
            &json!({"v":"openagents.turn-intent.v1","requires":[],
            "intent":id,"task_frame":self.frame,"input":input,"context":context,
            "configuration":host.status(session).unwrap()["requested"],"authorization":authorization,"bounds":[],
            "expires_at":self.fx.now+300}),
        );
        json!({"v":"openagents.session-command.v1","requires":[],"command":self.fx.next(),
            "scope":{"authority":pk(HOST),"session":session,"generation":0},
            "expected_revision":host.status(session).unwrap()["revision"],"authorization":authorization,
            "issued_at":self.fx.now,"expires_at":self.fx.now+300,"action":"submit","payload":{"intent":intent}})
    }
    fn opened(&mut self, host: &SessionHost) -> Value {
        let opening = self.opening(host);
        let (event, binding) = self.job(&opening);
        let reply = host
            .serve(&event, &self.fx.artifacts, &binding, self.fx.now)
            .unwrap();
        self.coordinator
            .record_result(&reply.0, &reply.1, self.fx.now)
            .unwrap();
        self.release_attachment(&binding.grant);
        opening
    }
    fn input(&mut self, command: &mut Value, body: Value) {
        let reference = &command["payload"]["intent"];
        let mut intent: Value =
            serde_json::from_slice(&self.fx.artifacts[reference["digest"].as_str().unwrap()])
                .unwrap();
        intent["input"] = put_json(&mut self.fx.artifacts, &body);
        command["payload"]["intent"] = put_json(&mut self.fx.artifacts, &intent);
    }
    fn question(&mut self, host: &SessionHost, session: &str) -> Value {
        let status = host.status(session).unwrap();
        let response = put_json(&mut self.fx.artifacts, &json!({"text":"main"}));
        json!({"v":"openagents.session-interaction-response.v1","requires":[],
            "interaction":status["interactions"][0]["interaction"],
            "scope":{"authority":pk(HOST),"session":session,"generation":status["generation"]},
            "responder":pk(OWNER),"response":response,"expires_at":self.fx.now+300})
    }
}

#[test]
fn real_cj_open_and_turn_share_actual_environment_and_call_fences() {
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opening(&host);
    let (event, binding) = r.job(&opening);
    let reply = host
        .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert_eq!(reply.0.kind, execution::RESULT_KIND);
    assert_eq!(
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap(),
        reply
    );
    r.coordinator
        .record_result(&reply.0, &reply.1, r.fx.now)
        .unwrap();
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 1);
    let session = opening["session"].as_str().unwrap().to_string();
    assert_eq!(host.status(&session).unwrap()["phase"], "ready");
    assert_eq!(host.started("loop-mid-turn"), 0);
    r.release_attachment(&binding.grant);
    let authorization = put_json(&mut r.fx.artifacts, &json!({"owner":pk(OWNER)}));
    let input = put_json(&mut r.fx.artifacts, &json!({"prompt":"once","steps":2}));
    let requested = opening["requested"].clone();
    let context = put_json(&mut r.fx.artifacts, &json!({"session":session}));
    let intent_id = r.fx.next();
    let intent = put_json(
        &mut r.fx.artifacts,
        &json!({"v":"openagents.turn-intent.v1","requires":[],
        "intent":intent_id,"task_frame":r.frame,"input":input,"context":context,
        "configuration":requested,"authorization":authorization,"bounds":[],"expires_at":r.fx.now+300}),
    );
    let command = json!({"v":"openagents.session-command.v1","requires":[],"command":r.fx.next(),
        "scope":{"authority":pk(HOST),"session":session,"generation":0},
        "expected_revision":host.status(&session).unwrap()["revision"],"authorization":authorization,
        "issued_at":r.fx.now,"expires_at":r.fx.now+300,"action":"submit","payload":{"intent":intent}});
    let (event, binding) = r.job(&command);
    let started = host
        .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert_eq!(started.0.kind, execution::FEEDBACK_KIND);
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap(),
        started
    );
    assert_eq!(
        host.poll(&event, r.fx.now).unwrap().0.kind,
        execution::FEEDBACK_KIND
    );
    let terminal = host.poll(&event, r.fx.now).unwrap();
    assert_eq!(terminal.0.kind, execution::RESULT_KIND);
    assert_eq!(host.poll(&event, r.fx.now).unwrap(), terminal);
    assert_eq!(host.started("loop-mid-turn"), 1);
    let result = r
        .coordinator
        .record_result(&terminal.0, &terminal.1, r.fx.now)
        .unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
}

#[test]
fn cj_lost_start_and_restart_keep_the_original_charge_and_unknown_outcome() {
    let mut r = Real::new();
    let mut engine = LoopEngine::mid_turn();
    engine.fail_start = true;
    let host = r.host(engine);
    let opening = r.opened(&host);
    let command = r.submit_command(&host, &opening, 2);
    let (event, binding) = r.job(&command);
    let pending = host
        .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert_eq!(pending.0.kind, execution::FEEDBACK_KIND);
    assert_eq!(host.started("loop-mid-turn"), 1);
    drop(host);
    let host = r.host(LoopEngine::mid_turn());
    let result = host.poll(&event, r.fx.now).unwrap();
    let opened = execution::open_environment_request(
        &event,
        &pk(HOST),
        &secret(HOST),
        r.fx.now,
        execution::Window::DEFAULT,
    )
    .unwrap();
    let execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let body = execution::bind_worker_event(
        &result.0,
        &execution::Pending {
            worker: &pk(HOST),
            customer: &pk(OWNER),
            request: &execute.request,
            attempt: execute.attempt,
            execute_event: &event.id,
        },
        &secret(OWNER),
    )
    .unwrap();
    assert_eq!(body["outcome"], "unknown");
    assert!(
        r.coordinator
            .record_result(&result.0, &result.1, r.fx.now)
            .is_err()
    );
    assert_eq!(
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap(),
        result
    );
    assert_eq!(host.poll(&event, r.fx.now).unwrap(), result);
    assert_eq!(host.started("loop-mid-turn"), 0);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    assert_eq!(
        host.status(opening["session"].as_str().unwrap()).unwrap()["phase"],
        "unknown"
    );
}

#[test]
fn cj_expiry_stops_the_original_turn_and_revocation_blocks_cached_results() {
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opened(&host);
    let command = r.submit_command(&host, &opening, 100);
    let (event, binding) = r.job(&command);
    host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    let expired = r.fx.now + 301;
    let result = host.poll(&event, expired).unwrap();
    assert_eq!(result.0.kind, execution::RESULT_KIND);
    assert_eq!(
        host.status(opening["session"].as_str().unwrap()).unwrap()["phase"],
        "ready"
    );
    assert_eq!(host.started("loop-mid-turn"), 1);
    let output = result
        .1
        .values()
        .filter_map(|bytes| serde_json::from_slice::<Value>(bytes).ok())
        .find(|b| b["v"] == "openagents.session-turn-result.v1")
        .unwrap();
    assert_eq!(output["cause"], "deadline");
    r.coordinator
        .record_result(&result.0, &result.1, expired)
        .unwrap();
    host.set_policy(Policy::default()).unwrap();
    assert!(
        host.poll(&event, expired)
            .unwrap_err()
            .contains("not_admitted")
    );
    assert!(
        host.serve(&event, &r.fx.artifacts, &binding, expired)
            .unwrap_err()
            .contains("not_admitted")
    );
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
}

#[test]
fn cj_oversized_native_output_is_a_bounded_failure_without_repeating_the_turn() {
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opened(&host);
    let mut command = r.submit_command(&host, &opening, 1);
    let reference = &command["payload"]["intent"];
    let mut intent: Value =
        serde_json::from_slice(&r.fx.artifacts[reference["digest"].as_str().unwrap()]).unwrap();
    intent["input"] = put_json(
        &mut r.fx.artifacts,
        &json!({"prompt":"x".repeat(1_048_000),"steps":1}),
    );
    command["payload"]["intent"] = put_json(&mut r.fx.artifacts, &intent);
    let (event, binding) = r.job(&command);
    host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    let result = host.poll(&event, r.fx.now).unwrap();
    assert_eq!(result.0.kind, execution::RESULT_KIND);
    assert!(result.1.values().map(Vec::len).sum::<usize>() < 65_536);
    let body = r
        .coordinator
        .record_result(&result.0, &result.1, r.fx.now)
        .unwrap();
    assert_eq!(body["outcome"], "failed");
    assert_eq!(host.poll(&event, r.fx.now).unwrap(), result);
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
}

#[test]
fn cj_continuations_answer_and_steer_only_the_original_charged_turn() {
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opened(&host);
    let session = opening["session"].as_str().unwrap();
    let mut command = r.submit_command(&host, &opening, 2);
    r.input(
        &mut command,
        json!({"prompt":"work","steps":2,"ask":"which branch?"}),
    );
    let (event, binding) = r.job(&command);
    host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    host.poll(&event, r.fx.now).unwrap();
    assert_eq!(host.status(session).unwrap()["phase"], "blocked");
    let response = r.question(&host, session);
    let mut expired = response.clone();
    expired["expires_at"] = json!(r.fx.now);
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &expired), r.fx.now)
            .unwrap()["reason"],
        "stale"
    );
    assert_eq!(
        host.submit(&r.fx.bundle(OWNER, &response), None, r.fx.now)
            .unwrap()["status"],
        "refused"
    );
    let result = host
        .continue_turn(&event, &r.fx.bundle(OWNER, &response), r.fx.now)
        .unwrap();
    assert_eq!(result["status"], "accepted", "{result}");
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &response), r.fx.now)
            .unwrap()["status"],
        "duplicate"
    );
    let correction = correction(&mut r.fx, "native", "use tabs");
    let status = host.status(session).unwrap();
    let steer = r.fx.command(
        &host,
        session,
        "steer",
        json!({"turn":status["active_turn"],
        "expected_turn_revision":status["turn_revision"],"correction":correction}),
    );
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &steer), r.fx.now)
            .unwrap()["status"],
        "accepted"
    );
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &steer), r.fx.now)
            .unwrap()["status"],
        "duplicate"
    );
    assert!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &command), r.fx.now)
            .is_err()
    );
    host.poll(&event, r.fx.now).unwrap();
    let result = host.poll(&event, r.fx.now).unwrap();
    let result = r
        .coordinator
        .record_result(&result.0, &result.1, r.fx.now)
        .unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &steer), r.fx.now)
            .unwrap()["status"],
        "duplicate"
    );
}

#[test]
fn current_coord_revocation_and_environment_drain_block_native_continuations() {
    for drain in [false, true] {
        let mut r = Real::new();
        let host = r.host(LoopEngine::mid_turn());
        let opening = r.opened(&host);
        let session = opening["session"].as_str().unwrap();
        let mut command = r.submit_command(&host, &opening, 2);
        r.input(
            &mut command,
            json!({"prompt":"work","steps":2,"ask":"which branch?"}),
        );
        let (event, binding) = r.job(&command);
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap();
        host.poll(&event, r.fx.now).unwrap();
        let response = r.question(&host, session);
        if drain {
            r.attachment_control(&r.allocator(), &binding.grant, "drain");
        } else {
            r.coordinator
                .control_grants(
                    &r.signed(json!({"v":"openagents.coordination-native-control.v1",
                "requires":[],"coordinator":r.scope,"command":workspace_host::artifact::token(),
                "expected_revision":r.revision(),"actor":pk(HOST),"roles":["read","claim"],
                "resources":[],"targets":[],"all":true,"until":r.fx.now+7200,
                "revoke":true,"scope_revoke":false})),
                    r.fx.now,
                )
                .unwrap();
        }
        assert!(
            host.continue_turn(&event, &r.fx.bundle(OWNER, &response), r.fx.now)
                .is_err()
        );
        let stopped = host.poll(&event, r.fx.now).unwrap();
        assert_eq!(stopped.0.kind, execution::RESULT_KIND);
        let turn = stopped
            .1
            .values()
            .filter_map(|b| serde_json::from_slice::<Value>(b).ok())
            .find(|b| b["v"] == "openagents.session-turn-result.v1")
            .unwrap();
        assert_eq!(turn["outcome"], "cancelled");
        assert_eq!(turn["cause"], "policy");
        assert_eq!(host.started("loop-mid-turn"), 1);
        assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    }
}

#[test]
fn lost_native_steering_and_answer_replies_are_not_delivered_again() {
    for answering in [false, true] {
        let mut r = Real::new();
        let mut engine = LoopEngine::mid_turn();
        engine.fail_steer = !answering;
        engine.fail_answer = answering;
        let host = r.host(engine);
        let opening = r.opened(&host);
        let session = opening["session"].as_str().unwrap();
        let mut command = r.submit_command(&host, &opening, 2);
        if answering {
            r.input(
                &mut command,
                json!({"prompt":"work","steps":2,"ask":"branch?"}),
            );
        }
        let (event, binding) = r.job(&command);
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap();
        let control = if answering {
            host.poll(&event, r.fx.now).unwrap();
            r.question(&host, session)
        } else {
            let correction = correction(&mut r.fx, "native", "only once");
            let status = host.status(session).unwrap();
            r.fx.command(
                &host,
                session,
                "steer",
                json!({"turn":status["active_turn"],
                "expected_turn_revision":status["turn_revision"],"correction":correction}),
            )
        };
        let bundle = r.fx.bundle(OWNER, &control);
        let unknown = host.continue_turn(&event, &bundle, r.fx.now).unwrap();
        assert_eq!(unknown["status"], "unknown");
        assert_eq!(
            host.continue_turn(&event, &bundle, r.fx.now).unwrap(),
            unknown
        );
        drop(host);
        let host = r.host(LoopEngine::mid_turn());
        assert_eq!(
            host.continue_turn(&event, &bundle, r.fx.now).unwrap(),
            unknown
        );
        assert_eq!(host.started("loop-mid-turn"), 0);
        assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    }
}

#[test]
fn continuation_holds_both_authorities_and_never_becomes_a_start_permission() {
    use env_host::runtime::{RuntimeContinuation, RuntimeUse};
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opened(&host);
    let command = r.submit_command(&host, &opening, 2);
    let (event, binding) = r.job(&command);
    let engine = host.store().json(&r.adapter).unwrap()["engine"].clone();
    let runtime = RuntimeUse {
        attachment: &binding.attachment,
        fingerprint: &binding.fingerprint,
        grant: &binding.grant,
        controller: &pk(OWNER),
        executor: &pk(HOST),
        lease: &opening["lease"],
        materialization: &opening["materialization"],
        engine: &engine,
    };
    let request = RuntimeContinuation {
        runtime,
        execute_event: &event.id,
        target: &r.target,
    };
    let allocator = r.allocator();
    let mut called = false;
    assert!(
        allocator
            .with_runtime_continuation(&request, r.fx.now, &mut |_, _| {
                called = true;
                Ok(Value::Null)
            })
            .is_err()
    );
    assert!(!called);
    host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert!(
        allocator
            .with_runtime_use(&runtime, r.fx.now, &mut || {
                called = true;
                Ok(())
            })
            .is_err()
    );
    assert!(!called);
    let revoke = r.signed(json!({"v":"openagents.coordination-native-control.v1",
        "requires":[],"coordinator":r.scope,"command":workspace_host::artifact::token(),
        "expected_revision":r.revision(),"actor":pk(HOST),"roles":["read","claim"],
        "resources":[],"targets":[],"all":true,"until":r.fx.now+7200,
        "revoke":true,"scope_revoke":false}));
    let drain = json!({"v":"openagents.environment-attachment-control.v1","requires":[],
        "attachment":binding.grant,"command":workspace_host::artifact::token(),
        "expected_revision":allocator.status(&r.lease_id).unwrap()["revision"],
        "action":"drain","evidence":[]});
    let drain = r.env_bundle(OWNER, &drain);
    let coord_root = r.fx._temp.path().join("coord");
    let env_root = r.allocator_root.clone();
    let resources = r.resource_root.clone();
    let now = r.fx.now;
    allocator
        .with_runtime_continuation(&request, now, &mut |fence, current| {
            let signer = nostr::domain::RelaySigner::from_secret_hex(
                &secret(OWNER).display_secret().to_string(),
            )
            .unwrap();
            let saved: Value = workspace_host::artifact::Artifacts::new(coord_root.clone())
                .unwrap()
                .load(&signer)
                .unwrap();
            assert_eq!(
                saved["scopes"]
                    .as_object()
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()["calls"],
                2
            );
            fence.with_current(&runtime, current, &mut || {
                called = true;
                Ok(())
            })?;
            let stranger = pk(STRANGER);
            let mut wrong = runtime;
            wrong.executor = &stranger;
            assert!(
                fence
                    .with_current(&wrong, current, &mut || panic!("wrong executor"))
                    .is_err()
            );
            for coor in [true, false] {
                let coord_root = coord_root.clone();
                let env_root = env_root.clone();
                let resources = resources.clone();
                let revoke = revoke.clone();
                let drain = drain.clone();
                let refused = std::thread::spawn(move || {
                    let result = if coor {
                        Coordinator::open(coord_root, secret(OWNER))
                            .and_then(|h| h.control_grants(&revoke, now))
                    } else {
                        Allocator::open(
                            env_root,
                            secret(9),
                            Box::new(LocalDirectories::new(resources).unwrap()),
                        )
                        .and_then(|h| h.submit(&drain, now))
                    };
                    result.unwrap_err()
                })
                .join()
                .unwrap();
                assert!(
                    refused.contains("another workspace operation is in progress"),
                    "{refused}"
                );
            }
            Ok(Value::Null)
        })
        .unwrap();
    assert!(called);
    assert_eq!(
        r.coordinator.control_grants(&revoke, now).unwrap()["status"],
        "accepted"
    );
    assert_eq!(allocator.submit(&drain, now).unwrap()["status"], "accepted");
    assert!(
        allocator
            .with_runtime_continuation(&request, now, &mut |_, _| panic!("revoked continuation"))
            .is_err()
    );
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
}

#[test]
fn cj_configuration_applies_native_settings_under_its_own_charge() {
    let mut r = Real::new();
    let host = r.host(LoopEngine::mid_turn());
    let opening = r.opened(&host);
    let session = opening["session"].as_str().unwrap();
    let mut settings: Value =
        serde_json::from_slice(&r.fx.artifacts[opening["requested"]["digest"].as_str().unwrap()])
            .unwrap();
    settings["settings"] = put_json(&mut r.fx.artifacts, &json!({"effort":"high"}));
    let requested = put_json(&mut r.fx.artifacts, &settings);
    let mut command = r.submit_command(&host, &opening, 1);
    command["action"] = json!("configure");
    command["payload"] = json!({"requested":requested});
    assert_eq!(
        host.submit(&r.fx.bundle(OWNER, &command), None, r.fx.now)
            .unwrap()["status"],
        "refused"
    );
    let (event, binding) = r.job(&command);
    let result = host
        .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert_eq!(result.0.kind, execution::RESULT_KIND);
    let result = r
        .coordinator
        .record_result(&result.0, &result.1, r.fx.now)
        .unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(host.status(session).unwrap()["requested"], requested);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    r.release_attachment(&binding.grant);
    let command = r.submit_command(&host, &opening, 1);
    let (event, binding) = r.job(&command);
    host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    let result = host.poll(&event, r.fx.now).unwrap();
    r.coordinator
        .record_result(&result.0, &result.1, r.fx.now)
        .unwrap();
    let turn = result
        .1
        .values()
        .filter_map(|b| serde_json::from_slice::<Value>(b).ok())
        .find(|b| b["v"] == "openagents.session-turn-result.v1")
        .unwrap();
    let output: Value =
        serde_json::from_slice(&result.1[turn["outputs"][0]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["effort"], "high");
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 3);
}
