use crate::{
    engine::{Engine, LoopEngine},
    host::{Binding, Bindings, Bundle, Policy, SessionHost, envelope},
};
use nostr::contracts::{SCHEMA_MEDIA_TYPE, digest_bytes, jcs};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const HOST: u8 = 1;
const OWNER: u8 = 2;
const CONTROLLER: u8 = 3;
const STRANGER: u8 = 4;

#[derive(Clone, Default)]
struct Admitted(Rc<RefCell<BTreeSet<(String, String)>>>);
impl Bindings for Admitted {
    fn with_current(
        &self,
        request: &env_host::runtime::RuntimeUse<'_>,
        now: u64,
        effect: &mut dyn FnMut() -> crate::Result<()>,
    ) -> crate::Result<()> {
        self.check(request.attachment, request.fingerprint, now)?;
        effect()
    }
    fn check(&self, attachment: &str, fingerprint: &str, _: u64) -> crate::Result<()> {
        if self
            .0
            .borrow()
            .contains(&(attachment.into(), fingerprint.into()))
        {
            Ok(())
        } else {
            Err("not_admitted: attachment is not active".into())
        }
    }
}

fn put(
    map: &mut BTreeMap<String, Vec<u8>>,
    bytes: &[u8],
    media: &str,
    schema: Option<&str>,
) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut value = json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":media});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(
        map,
        &jcs(body).unwrap(),
        "application/json",
        body["v"].as_str(),
    )
}
fn hex(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

struct Fx {
    _temp: tempfile::TempDir,
    root: PathBuf,
    bindings: Admitted,
    artifacts: BTreeMap<String, Vec<u8>>,
    now: u64,
    counter: u8,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("sessions");
        SessionHost::initialize(
            root.clone(),
            secret(HOST),
            Policy {
                owners: BTreeSet::from([pk(OWNER)]),
                controllers: BTreeSet::from([pk(CONTROLLER)]),
            },
        )
        .unwrap();
        Self {
            _temp: temp,
            root,
            bindings: Admitted::default(),
            artifacts: BTreeMap::new(),
            now: crate::now(),
            counter: 0x40,
        }
    }
    fn host(&self) -> SessionHost {
        self.host_with(LoopEngine::mid_turn(), LoopEngine::boundary())
    }
    fn host_with(&self, a: LoopEngine, b: LoopEngine) -> SessionHost {
        SessionHost::open(
            self.root.clone(),
            secret(HOST),
            vec![Box::new(a), Box::new(b)],
            Box::new(self.bindings.clone()),
        )
        .unwrap()
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        hex(self.counter)
    }
    fn binding(&mut self) -> Binding {
        let attachment = self.next();
        let fingerprint = format!("sha256:{}", self.next());
        self.bindings
            .0
            .borrow_mut()
            .insert((attachment.clone(), fingerprint.clone()));
        let grant = put(
            &mut self.artifacts,
            attachment.as_bytes(),
            "application/json",
            Some("openagents.environment-attachment.v1"),
        );
        Binding {
            attachment,
            fingerprint,
            grant,
        }
    }
    fn configuration(&mut self, host: &SessionHost, adapter: &Value, effort: &str) -> Value {
        let description = host.store().json(adapter).unwrap();
        let schema = serde_json::to_vec(&LoopEngine::mid_turn().profile().settings_schema).unwrap();
        let body = json!({"v":"openagents.session-configuration.v1","requires":[],"adapter":description["adapter"],
            "recipient":put(&mut self.artifacts,b"recipient","application/json",None),
            "model":put(&mut self.artifacts,b"model","application/json",None),
            "settings_schema":put(&mut self.artifacts,&schema,SCHEMA_MEDIA_TYPE,None),
            "settings":put(&mut self.artifacts,&serde_json::to_vec(&json!({"effort":effort})).unwrap(),"application/json",None),
            "policy":put(&mut self.artifacts,b"policy","application/json",None),"enforcement":null,"bounds":[]});
        put_json(&mut self.artifacts, &body)
    }
    fn env_refs(&mut self) -> (Value, Value) {
        let art = |s: &str| json!({"digest":digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json","schema":s});
        let lease = json!({"v":"openagents.environment-lease.v1","requires":[],"lease":hex(0x11),
            "request":art("openagents.environment-request.v1"),"allocator":pk(9),"created_at":self.now,
            "not_after":self.now+3600,"retain_until":self.now+7200,"reservation":art("openagents.reservation.v1")});
        let plain = |s: &str| json!({"digest":digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json"});
        let runtime = json!({"id":format!("{}:runtime/loop",pk(9)),"artifact":plain("runtime")});
        let materialization = json!({"v":"openagents.environment-materialization.v1","requires":[],
            "lease":put_json(&mut self.artifacts,&lease),"definition":art("openagents.environment-definition.v1"),
            "resource":art("openagents.environment-resource.v1"),"generation":hex(0x12),
            "components":[{"id":"base","role":"base","artifact":plain("base")}],"runtime":runtime,
            "configuration":plain("config"),"model":null,"workspace":null,
            "participants":[{"principal":pk(HOST),"role":"executor","runtime":runtime,"model":null,"configuration":plain("config"),"assurance":plain("a")}],
            "bootstrap":null,"enforcement":art("openagents.environment-enforcement.v1"),"evidence":[],"observed_at":self.now});
        (
            put_json(&mut self.artifacts, &lease),
            put_json(&mut self.artifacts, &materialization),
        )
    }
    fn bundle(&self, signer: u8, body: &Value) -> Bundle {
        Bundle {
            event: envelope(&secret(signer), &pk(HOST), body, self.now).unwrap(),
            artifacts: self.artifacts.clone(),
        }
    }
    fn open_body(&mut self, host: &SessionHost, adapter: &Value, source: Value) -> Value {
        let (lease, materialization) = self.env_refs();
        let requested = self.configuration(host, adapter, "low");
        json!({"v":"openagents.session-open.v1","requires":[],"command":self.next(),"session":self.next(),
            "owner":pk(OWNER),"authority":pk(HOST),"workspace":null,"lease":lease,"materialization":materialization,
            "adapter":adapter,"requested":requested,"authorization":put(&mut self.artifacts,b"authorization","application/json",None),
            "source":source,"issued_at":self.now,"expires_at":self.now+600})
    }
    fn open(&mut self, host: &SessionHost, engine: &str) -> (String, Value) {
        let adapter = host.admit_adapter(engine, self.now).unwrap();
        let body = self.open_body(host, &adapter, json!({"kind":"new","artifact":null}));
        let binding = self.binding();
        let result = host
            .submit(&self.bundle(OWNER, &body), Some(&binding), self.now)
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
        (body["session"].as_str().unwrap().into(), adapter)
    }
    fn intent(&mut self, host: &SessionHost, session: &str, input: Value, expires: u64) -> Value {
        let admission = host
            .store()
            .json(&host.status(session).unwrap()["admission"])
            .unwrap();
        let body = json!({"v":"openagents.turn-intent.v1","requires":[],"intent":self.next(),
            "task_frame":put(&mut self.artifacts,b"frame","application/json",None),
            "input":put(&mut self.artifacts,&serde_json::to_vec(&input).unwrap(),"application/json",None),
            "context":put(&mut self.artifacts,b"context","application/json",None),
            "configuration":admission["requested"],"authorization":put(&mut self.artifacts,b"authorization","application/json",None),
            "bounds":[],"expires_at":expires});
        put_json(&mut self.artifacts, &body)
    }
    fn command(
        &mut self,
        host: &SessionHost,
        session: &str,
        action: &str,
        payload: Value,
    ) -> Value {
        let status = host.status(session).unwrap();
        json!({"v":"openagents.session-command.v1","requires":[],"command":self.next(),
            "scope":{"authority":pk(HOST),"session":session,"generation":status["generation"]},
            "expected_revision":status["revision"],"authorization":put(&mut self.artifacts,b"authorization","application/json",None),
            "issued_at":self.now,"expires_at":self.now+600,"action":action,"payload":payload})
    }
    fn send(
        &mut self,
        host: &SessionHost,
        signer: u8,
        body: &Value,
        binding: Option<&Binding>,
    ) -> Value {
        host.submit(&self.bundle(signer, body), binding, self.now)
            .unwrap()
    }
    fn finish(&self, host: &SessionHost, session: &str) -> Value {
        for _ in 0..16 {
            let observed = host.observe(session, self.now).unwrap();
            if !observed["result"].is_null() {
                return host.store().json(&observed["result"]).unwrap();
            }
        }
        panic!("turn did not finish")
    }
}

#[test]
fn an_admitted_session_runs_a_turn_only_under_a_binding() {
    let mut fx = Fx::new();
    let host = fx.host();
    let adapter = host.admit_adapter("loop-mid-turn", fx.now).unwrap();
    let body = fx.open_body(&host, &adapter, json!({"kind":"new","artifact":null}));
    // Without a current ENV attachment nothing opens.
    assert_eq!(fx.send(&host, OWNER, &body, None)["reason"], "not_admitted");
    let binding = fx.binding();
    let opened = fx.send(&host, OWNER, &body, Some(&binding));
    assert_eq!(opened["status"], "accepted", "{opened}");
    let receipt = host.store().json(&opened["receipt"]).unwrap();
    assert_eq!(
        (
            receipt["before_revision"].clone(),
            receipt["after_revision"].clone()
        ),
        (json!(0), json!(0))
    );
    assert_eq!(
        fx.send(&host, OWNER, &body, Some(&binding))["status"],
        "duplicate"
    );
    let mut changed = body.clone();
    changed["expires_at"] = json!(fx.now + 601);
    assert_eq!(
        fx.send(&host, OWNER, &changed, Some(&binding))["reason"],
        "idempotency_conflict"
    );
    let session = body["session"].as_str().unwrap();
    let status = host.status(session).unwrap();
    // Requested and effective configuration are both retained and differ.
    let admission = host.store().json(&status["admission"]).unwrap();
    assert_ne!(admission["requested"], admission["effective"]);
    assert!(!host.store().json(&admission["effective"]).unwrap()["enforcement"].is_null());

    let intent = fx.intent(
        &host,
        session,
        json!({"prompt":"hello","steps":2}),
        fx.now + 600,
    );
    let submit = fx.command(&host, session, "submit", json!({"intent":intent}));
    assert_eq!(
        fx.send(&host, OWNER, &submit, None)["reason"],
        "not_admitted"
    );
    assert_eq!(host.started("loop-mid-turn"), 0);
    let submit = fx.command(&host, session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, STRANGER, &submit, Some(&binding))["reason"],
        "not_admitted"
    );
    let accepted = fx.send(&host, OWNER, &submit, Some(&binding));
    assert_eq!(accepted["status"], "accepted", "{accepted}");
    assert_eq!(host.status(session).unwrap()["phase"], "running");
    let result = fx.finish(&host, session);
    assert_eq!(result["outcome"], "completed");
    assert_eq!(result["cause"], "normal");
    assert!(!result["quiescence"].is_null());
    let status = host.status(session).unwrap();
    assert_eq!(status["phase"], "ready");
    let history = host.store().json(&status["history"]).unwrap();
    assert!(history["native"].as_array().unwrap().len() >= 3);
    assert!(!history["cut"].is_null());
    assert!(!history["resume"].is_null());
}

#[test]
fn queue_law_promotes_only_the_head_after_quiescence() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let first = fx.intent(
        &host,
        &session,
        json!({"prompt":"one","steps":2}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":first}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    // A second submit while running conflicts; enqueue is the queue path.
    let again = fx.intent(&host, &session, json!({"prompt":"x"}), fx.now + 600);
    let submit = fx.command(&host, &session, "submit", json!({"intent":again}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &submit, Some(&binding))["reason"],
        "conflict"
    );
    let a = fx.intent(&host, &session, json!({"prompt":"a"}), fx.now + 600);
    let b = fx.intent(&host, &session, json!({"prompt":"b"}), fx.now + 600);
    let stale = fx.intent(&host, &session, json!({"prompt":"stale"}), fx.now + 5);
    for intent in [&a, &b, &stale] {
        let enqueue = fx.command(&host, &session, "enqueue", json!({"intent":intent}));
        assert_eq!(
            fx.send(&host, CONTROLLER, &enqueue, None)["status"],
            "accepted"
        );
    }
    let ids: Vec<String> = host.status(&session).unwrap()["queue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["intent"].as_str().unwrap().to_string())
        .collect();
    let wrong = fx.command(
        &host,
        &session,
        "reorder",
        json!({"intents":[ids[1], ids[0]]}),
    );
    assert_eq!(fx.send(&host, OWNER, &wrong, None)["reason"], "conflict");
    let reorder = fx.command(
        &host,
        &session,
        "reorder",
        json!({"intents":[ids[2], ids[1], ids[0]]}),
    );
    assert_eq!(fx.send(&host, OWNER, &reorder, None)["status"], "accepted");
    let cancel = fx.command(&host, &session, "cancel_queued", json!({"intent":ids[1]}));
    assert_eq!(fx.send(&host, OWNER, &cancel, None)["status"], "accepted");
    // The running turn is not quiescent, so nothing promotes.
    let binding = fx.binding();
    assert!(
        host.promote(&session, Some(&binding), fx.now)
            .unwrap_err()
            .starts_with("conflict")
    );
    fx.finish(&host, &session);
    // The stale head expires at promotion instead of being edited or run.
    fx.now += 60;
    let expired = host.promote(&session, Some(&binding), fx.now).unwrap();
    assert_eq!(expired["state"], "expired");
    let promoted = host.promote(&session, Some(&binding), fx.now).unwrap();
    assert_eq!(promoted["state"], "promoted");
    let queue = host.status(&session).unwrap()["queue"].clone();
    assert_eq!(queue[2]["state"], "promoted");
    assert_eq!(queue[1]["state"], "cancelled");
    assert_eq!(host.started("loop-mid-turn"), 2);
}

#[test]
fn stale_revisions_generations_and_expired_commands_refuse() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let mut archive = fx.command(&host, &session, "archive", json!({"archived":true}));
    let other = fx.command(&host, &session, "archive", json!({"archived":false}));
    assert_eq!(fx.send(&host, OWNER, &archive, None)["status"], "accepted");
    assert_eq!(fx.send(&host, OWNER, &other, None)["reason"], "conflict");
    archive = fx.command(&host, &session, "archive", json!({"archived":false}));
    archive["scope"]["generation"] = json!(7);
    assert_eq!(fx.send(&host, OWNER, &archive, None)["reason"], "stale");
    let mut expired = fx.command(&host, &session, "archive", json!({"archived":false}));
    expired["issued_at"] = json!(fx.now - 100);
    expired["expires_at"] = json!(fx.now - 1);
    assert_eq!(fx.send(&host, OWNER, &expired, None)["reason"], "stale");
    // Configure refuses an unsupported setting rather than dropping it.
    let adapter = host
        .store()
        .json(&host.status(&session).unwrap()["admission"])
        .unwrap()["adapter"]
        .clone();
    let requested = fx.configuration(&host, &adapter, "max");
    let configure = fx.command(&host, &session, "configure", json!({"requested":requested}));
    assert_eq!(
        fx.send(&host, OWNER, &configure, None)["reason"],
        "unsupported_feature"
    );
    let requested = fx.configuration(&host, &adapter, "high");
    let configure = fx.command(&host, &session, "configure", json!({"requested":requested}));
    assert_eq!(
        fx.send(&host, OWNER, &configure, None)["status"],
        "accepted"
    );
}

fn correction(fx: &mut Fx, mode: &str, text: &str) -> Value {
    let content = put(
        &mut fx.artifacts,
        &serde_json::to_vec(&json!({"text":text})).unwrap(),
        "application/json",
        None,
    );
    put_json(
        &mut fx.artifacts,
        &json!({"v":"openagents.session-steer-correction.v1","requires":[],"correction":content,"mode":mode}),
    )
}

#[test]
fn native_mid_turn_steering_is_consumed_only_with_engine_evidence() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"work","steps":3}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    let turn = host.status(&session).unwrap()["active_turn"].clone();
    let emulated = correction(&mut fx, "emulated", "nope");
    let steer = fx.command(
        &host,
        &session,
        "steer",
        json!({"turn":turn,"expected_turn_revision":0,"correction":emulated}),
    );
    assert_eq!(
        fx.send(&host, OWNER, &steer, None)["reason"],
        "unsupported_feature"
    );
    let native = correction(&mut fx, "native", "use tabs");
    let stale = fx.command(
        &host,
        &session,
        "steer",
        json!({"turn":turn,"expected_turn_revision":5,"correction":native}),
    );
    assert_eq!(fx.send(&host, OWNER, &stale, None)["reason"], "conflict");
    let steer = fx.command(
        &host,
        &session,
        "steer",
        json!({"turn":turn,"expected_turn_revision":0,"correction":native}),
    );
    assert_eq!(fx.send(&host, OWNER, &steer, None)["status"], "accepted");
    let receipt = host
        .store()
        .json(&host.status(&session).unwrap()["steers"][0])
        .unwrap();
    assert_eq!(receipt["acknowledgment"], "engine_replay");
    assert_eq!(receipt["consumed"], true);
    let result = fx.finish(&host, &session);
    let output: Value =
        serde_json::from_slice(&host.store().get(&result["outputs"][0]).unwrap()).unwrap();
    assert_eq!(output["corrections"], json!([{"text":"use tabs"}]));
}

#[test]
fn boundary_steering_is_emulated_only_when_chosen_and_carried_by_the_next_turn() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-boundary");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"first","steps":5}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    let turn = host.status(&session).unwrap()["active_turn"].clone();
    let native = correction(&mut fx, "native", "now");
    let steer = fx.command(
        &host,
        &session,
        "steer",
        json!({"turn":turn,"expected_turn_revision":0,"correction":native}),
    );
    assert_eq!(
        fx.send(&host, OWNER, &steer, None)["reason"],
        "unsupported_feature"
    );
    let emulated = correction(&mut fx, "emulated", "carry me");
    let steer = fx.command(
        &host,
        &session,
        "steer",
        json!({"turn":turn,"expected_turn_revision":0,"correction":emulated}),
    );
    assert_eq!(fx.send(&host, OWNER, &steer, None)["status"], "accepted");
    let receipt = host
        .store()
        .json(&host.status(&session).unwrap()["steers"][0])
        .unwrap();
    assert_eq!(receipt["consumed"], false);
    let result = fx.finish(&host, &session);
    assert_eq!(
        (result["outcome"].clone(), result["cause"].clone()),
        (json!("cancelled"), json!("user_interrupt"))
    );
    // The next turn starts under a fresh binding and carries the correction.
    let next = fx.intent(
        &host,
        &session,
        json!({"prompt":"second","steps":1}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":next}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &submit, Some(&binding))["status"],
        "accepted"
    );
    let consumed = host
        .store()
        .json(&host.status(&session).unwrap()["steers"][1])
        .unwrap();
    assert_eq!(
        (
            consumed["acknowledgment"].clone(),
            consumed["consumed"].clone()
        ),
        (json!("next_turn_start"), json!(true))
    );
    let result = fx.finish(&host, &session);
    let output: Value =
        serde_json::from_slice(&host.store().get(&result["outputs"][0]).unwrap()).unwrap();
    assert_eq!(output["corrections"], json!([{"text":"carry me"}]));
}

fn answer(
    fx: &mut Fx,
    host: &SessionHost,
    session: &str,
    interaction: &Value,
    signer: u8,
    text: Value,
) -> Value {
    let response = put(
        &mut fx.artifacts,
        &serde_json::to_vec(&text).unwrap(),
        "application/json",
        None,
    );
    let generation = host.status(session).unwrap()["generation"].clone();
    let body = json!({"v":"openagents.session-interaction-response.v1","requires":[],"interaction":interaction,
        "scope":{"authority":pk(HOST),"session":session,"generation":generation},"responder":pk(signer),
        "response":response,"expires_at":fx.now+600});
    fx.send(host, signer, &body, None)
}

#[test]
fn the_first_authorized_answer_wins_and_is_delivered_once() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"ask","steps":1,"ask":"which branch?"}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    host.observe(&session, fx.now).unwrap();
    let status = host.status(&session).unwrap();
    assert_eq!(status["phase"], "blocked");
    let interaction = status["interactions"][0]["interaction"].clone();
    assert_eq!(
        answer(
            &mut fx,
            &host,
            &session,
            &interaction,
            CONTROLLER,
            json!({"text":"main"})
        )["reason"],
        "not_admitted"
    );
    assert_eq!(
        answer(
            &mut fx,
            &host,
            &session,
            &interaction,
            OWNER,
            json!({"text":"main","token":"x"})
        )["reason"],
        "malformed"
    );
    let first = answer(
        &mut fx,
        &host,
        &session,
        &interaction,
        OWNER,
        json!({"text":"main"}),
    );
    assert_eq!(first["status"], "accepted", "{first}");
    let dup = answer(
        &mut fx,
        &host,
        &session,
        &interaction,
        OWNER,
        json!({"text":"main"}),
    );
    assert_eq!(dup["status"], "duplicate", "{dup}");
    assert_eq!(
        answer(
            &mut fx,
            &host,
            &session,
            &interaction,
            OWNER,
            json!({"text":"dev"})
        )["status"],
        "conflict"
    );
    let result = fx.finish(&host, &session);
    let output: Value =
        serde_json::from_slice(&host.store().get(&result["outputs"][0]).unwrap()).unwrap();
    assert_eq!(output["answer"], json!({"text":"main"}));
}

#[test]
fn a_lost_delivery_stays_unknown_and_is_not_redelivered() {
    let mut fx = Fx::new();
    let mut engine = LoopEngine::mid_turn();
    engine.fail_answer = true;
    let host = fx.host_with(engine, LoopEngine::boundary());
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"ask","ask":"ok?"}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    host.observe(&session, fx.now).unwrap();
    let interaction = host.status(&session).unwrap()["interactions"][0]["interaction"].clone();
    let lost = answer(
        &mut fx,
        &host,
        &session,
        &interaction,
        OWNER,
        json!({"text":"yes"}),
    );
    assert_eq!(lost["status"], "unknown");
    assert_eq!(
        host.status(&session).unwrap()["interactions"][0]["state"],
        "unknown"
    );
    let retry = answer(
        &mut fx,
        &host,
        &session,
        &interaction,
        OWNER,
        json!({"text":"yes"}),
    );
    assert_eq!(retry["status"], "unknown");
}

#[test]
fn restart_turns_a_running_turn_into_unknown_and_never_reruns_it() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"long","steps":9}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    let queued = fx.intent(&host, &session, json!({"prompt":"next"}), fx.now + 600);
    let enqueue = fx.command(&host, &session, "enqueue", json!({"intent":queued}));
    fx.send(&host, OWNER, &enqueue, None);
    drop(host);
    let host = fx.host();
    let observed = host.observe(&session, fx.now).unwrap();
    let result = host.store().json(&observed["result"]).unwrap();
    assert_eq!(
        (result["outcome"].clone(), result["cause"].clone()),
        (json!("unknown"), json!("lost_observation"))
    );
    assert!(result["quiescence"].is_null());
    assert_eq!(host.status(&session).unwrap()["phase"], "unknown");
    let binding = fx.binding();
    assert!(host.promote(&session, Some(&binding), fx.now).is_err());
    assert_eq!(host.started("loop-mid-turn"), 0);
}

#[test]
fn resume_preserves_an_exclusive_lineage_and_import_is_read_only() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, adapter) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(&host, &session, json!({"prompt":"one"}), fx.now + 600);
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    fx.finish(&host, &session);
    let history = host.export(&session, fx.now).unwrap();
    let bytes = host.store().get(&history).unwrap();
    fx.artifacts.insert(digest_bytes(&bytes), bytes);
    // The lineage is still bound to an open session.
    let resume = fx.open_body(&host, &adapter, json!({"kind":"resume","artifact":history}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &resume, Some(&binding))["reason"],
        "conflict"
    );
    let close = fx.command(&host, &session, "close", json!({"reason":"done"}));
    assert_eq!(fx.send(&host, OWNER, &close, None)["status"], "accepted");
    let resume = fx.open_body(&host, &adapter, json!({"kind":"resume","artifact":history}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &resume, Some(&binding))["status"],
        "accepted"
    );
    let resumed = resume["session"].as_str().unwrap();
    assert_eq!(
        host.status(resumed).unwrap()["lineage"],
        host.status(&session).unwrap()["lineage"]
    );
    // Fork is unsupported by this adapter and refuses.
    let fork = fx.open_body(&host, &adapter, json!({"kind":"fork","artifact":history}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &fork, Some(&binding))["reason"],
        "unsupported_feature"
    );

    let content = b"{\"foreign\":\"transcript\"}";
    let source = json!({"v":"openagents.session-import-source.v1","requires":[],
        "content":{"digest":digest_bytes(content),"size":content.len(),"media_type":"application/octet-stream"},
        "format":{"digest":digest_bytes(b"fmt"),"size":3,"media_type":SCHEMA_MEDIA_TYPE},
        "importer":{"id":format!("{}:session/importer",pk(HOST)),"artifact":{"digest":digest_bytes(b"imp"),"size":3,"media_type":"application/octet-stream"}},
        "engine":null,"native_session":"abc","captured_at":fx.now,
        "authorization":{"digest":digest_bytes(b"auth"),"size":4,"media_type":"application/json"},"coverage":"partial"});
    let foreign = host.import_history(&source, content, fx.now).unwrap();
    let bytes = host.store().get(&foreign).unwrap();
    fx.artifacts.insert(digest_bytes(&bytes), bytes);
    let import = fx.open_body(&host, &adapter, json!({"kind":"import","artifact":foreign}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &import, Some(&binding))["status"],
        "accepted"
    );
    let imported = import["session"].as_str().unwrap().to_string();
    assert_eq!(host.status(&imported).unwrap()["read_only"], true);
    let intent = fx.intent(&host, &imported, json!({"prompt":"x"}), fx.now + 600);
    let submit = fx.command(&host, &imported, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &submit, Some(&binding))["reason"],
        "not_admitted"
    );
}

#[test]
fn children_are_managed_only_with_admission_and_authorization() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (parent, _) = fx.open(&host, "loop-mid-turn");
    let (child, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(&host, &parent, json!({"prompt":"delegate"}), fx.now + 600);
    let submit = fx.command(&host, &parent, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    let turn = host.status(&parent).unwrap()["active_turn"]
        .as_str()
        .unwrap()
        .to_string();
    let native = host.status(&parent).unwrap()["history"].clone();
    let native = host.store().json(&native).unwrap()["native"][0].clone();
    assert!(
        host.link_child(
            &parent,
            &hex(0x99),
            Some(&child),
            None,
            Some(json!(native)),
            fx.now
        )
        .is_err()
    );
    let authorization = host
        .store()
        .put(b"delegated", "application/json", None)
        .unwrap();
    let managed = host
        .link_child(
            &parent,
            &turn,
            Some(&child),
            None,
            Some(authorization),
            fx.now,
        )
        .unwrap();
    assert_eq!(
        host.store().json(&managed).unwrap()["relationship"],
        "managed"
    );
    let observed = host
        .link_child(&parent, &turn, None, Some(native), None, fx.now)
        .unwrap();
    assert_eq!(
        host.store().json(&observed).unwrap()["relationship"],
        "observed"
    );
}

#[test]
fn a_real_environment_allocator_without_an_attachment_admits_nothing() {
    let mut fx = Fx::new();
    let temp = tempfile::tempdir().unwrap();
    env_host::allocator::Allocator::initialize(
        temp.path().join("env"),
        secret(9),
        env_host::allocator::Policy::default(),
    )
    .unwrap();
    let allocator = env_host::allocator::Allocator::open(
        temp.path().join("env"),
        secret(9),
        Box::new(env_host::local::LocalDirectories::new(temp.path().join("res")).unwrap()),
    )
    .unwrap();
    let host = SessionHost::open(
        fx.root.clone(),
        secret(HOST),
        vec![Box::new(LoopEngine::mid_turn())],
        Box::new(allocator),
    )
    .unwrap();
    let adapter = host.admit_adapter("loop-mid-turn", fx.now).unwrap();
    let body = fx.open_body(&host, &adapter, json!({"kind":"new","artifact":null}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &body, Some(&binding))["reason"],
        "not_admitted"
    );
    assert_eq!(host.started("loop-mid-turn"), 0);
    let _: &dyn Engine = &LoopEngine::mid_turn();
}

#[test]
fn lost_start_keeps_the_original_pending_turn_and_never_starts_it_again() {
    let mut fx = Fx::new();
    let mut engine = LoopEngine::mid_turn();
    engine.fail_start = true;
    let host = fx.host_with(engine, LoopEngine::boundary());
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"once","steps":1}),
        fx.now + 600,
    );
    let command = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    let first = fx.send(&host, OWNER, &command, Some(&binding));
    assert_eq!(first["status"], "unknown", "{first}");
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(host.status(&session).unwrap()["phase"], "running");
    let retry = fx.send(&host, OWNER, &command, Some(&binding));
    assert_eq!(retry, first);
    assert_eq!(host.started("loop-mid-turn"), 1);
    let restarted = fx.host();
    let retry = fx.send(&restarted, OWNER, &command, Some(&binding));
    assert_eq!(retry, first);
    assert_eq!(restarted.started("loop-mid-turn"), 0);
    restarted.observe(&session, fx.now).unwrap();
    assert_eq!(restarted.status(&session).unwrap()["phase"], "unknown");
}
#[test]
fn revoked_controllers_cannot_read_cached_commands_or_promote_their_queue() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"queued","steps":1}),
        fx.now + 600,
    );
    let command = fx.command(&host, &session, "enqueue", json!({"intent":intent}));
    assert_eq!(
        fx.send(&host, CONTROLLER, &command, None)["status"],
        "accepted"
    );
    assert_eq!(
        fx.send(&host, STRANGER, &command, None)["reason"],
        "not_admitted"
    );
    host.set_policy(Policy {
        owners: BTreeSet::from([pk(OWNER)]),
        controllers: BTreeSet::new(),
    })
    .unwrap();
    assert_eq!(
        fx.send(&host, CONTROLLER, &command, None)["reason"],
        "not_admitted"
    );
    let binding = fx.binding();
    assert!(host.promote(&session, Some(&binding), fx.now).is_err());
    assert_eq!(host.started("loop-mid-turn"), 0);
}
#[test]
fn a_cached_question_answer_still_requires_the_current_recipient() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"ask","steps":1,"ask":"branch?"}),
        fx.now + 600,
    );
    let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
    let binding = fx.binding();
    fx.send(&host, OWNER, &submit, Some(&binding));
    host.observe(&session, fx.now).unwrap();
    let interaction = host.status(&session).unwrap()["interactions"][0]["interaction"].clone();
    let response = put_json(&mut fx.artifacts, &json!({"text":"main"}));
    let body = json!({"v":"openagents.session-interaction-response.v1","requires":[],"interaction":interaction,"scope":{"authority":pk(HOST),"session":session,"generation":0},"responder":pk(OWNER),"response":response,"expires_at":fx.now+600});
    assert_eq!(fx.send(&host, OWNER, &body, None)["status"], "accepted");
    assert_eq!(
        fx.send(&host, STRANGER, &body, None)["reason"],
        "not_admitted"
    );
    host.set_policy(Policy {
        owners: BTreeSet::new(),
        controllers: BTreeSet::new(),
    })
    .unwrap();
    assert_eq!(fx.send(&host, OWNER, &body, None)["reason"], "not_admitted");
}

#[test]
fn lost_queue_promotion_consumes_the_head_before_the_engine_effect() {
    let mut fx = Fx::new();
    let mut engine = LoopEngine::mid_turn();
    engine.fail_start = true;
    let host = fx.host_with(engine, LoopEngine::boundary());
    let (session, _) = fx.open(&host, "loop-mid-turn");
    let intent = fx.intent(
        &host,
        &session,
        json!({"prompt":"once","steps":1}),
        fx.now + 600,
    );
    let enqueue = fx.command(&host, &session, "enqueue", json!({"intent":intent}));
    assert_eq!(fx.send(&host, OWNER, &enqueue, None)["status"], "accepted");
    let binding = fx.binding();
    assert!(host.promote(&session, Some(&binding), fx.now).is_err());
    assert_eq!(host.started("loop-mid-turn"), 1);
    fx.finish(&host, &session);
    assert!(host.promote(&session, Some(&binding), fx.now).is_err());
    assert_eq!(host.started("loop-mid-turn"), 1);
}
#[test]
fn cached_open_requires_the_current_owner_even_with_the_original_body() {
    let mut fx = Fx::new();
    let host = fx.host();
    let adapter = host.admit_adapter("loop-mid-turn", fx.now).unwrap();
    let body = fx.open_body(&host, &adapter, json!({"kind":"new","artifact":null}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &body, Some(&binding))["status"],
        "accepted"
    );
    assert_eq!(
        fx.send(&host, STRANGER, &body, Some(&binding))["reason"],
        "not_admitted"
    );
    host.set_policy(Policy {
        owners: BTreeSet::new(),
        controllers: BTreeSet::new(),
    })
    .unwrap();
    assert_eq!(
        fx.send(&host, OWNER, &body, Some(&binding))["reason"],
        "not_admitted"
    );
}

#[test]
fn a_read_only_attachment_probe_cannot_authorize_runtime_disclosure() {
    struct Probe;
    impl Bindings for Probe {
        fn check(&self, _: &str, _: &str, _: u64) -> crate::Result<()> {
            Ok(())
        }
    }
    let mut fx = Fx::new();
    let host = SessionHost::open(
        fx.root.clone(),
        secret(HOST),
        vec![Box::new(LoopEngine::mid_turn())],
        Box::new(Probe),
    )
    .unwrap();
    let adapter = host.admit_adapter("loop-mid-turn", fx.now).unwrap();
    let body = fx.open_body(&host, &adapter, json!({"kind":"new","artifact":null}));
    let binding = fx.binding();
    let result = fx.send(&host, OWNER, &body, Some(&binding));
    assert_eq!(result["status"], "refused");
    assert_eq!(result["reason"], "cannot_enforce");
    assert!(host.status(body["session"].as_str().unwrap()).is_err());
}

struct OpeningEngine {
    engine: LoopEngine,
    root: PathBuf,
    calls: Rc<std::cell::Cell<usize>>,
    crash: bool,
}
impl Engine for OpeningEngine {
    fn profile(&self) -> crate::engine::Profile {
        self.engine.profile()
    }
    fn open(&mut self, resume: Option<&[u8]>) -> crate::Result<String> {
        let signer =
            nostr::domain::RelaySigner::from_secret_hex(&secret(HOST).display_secret().to_string())
                .unwrap();
        let store = workspace_host::artifact::Artifacts::new(self.root.clone()).unwrap();
        let saved: Value = store.load(&signer).unwrap();
        let pending = saved["sessions"]
            .as_object()
            .unwrap()
            .values()
            .find(|s| s["opening"] == true)
            .expect("opening saved before native effect");
        assert_eq!(pending["phase"], "unknown");
        assert!(pending["native"].is_null());
        assert_eq!(pending["quiescent"], false);
        assert_eq!(pending["opening_run"].as_array().unwrap().len(), 3);
        assert!(
            saved["priors"]
                .as_object()
                .unwrap()
                .values()
                .any(|p| p["result"]["status"] == "unknown")
        );
        self.calls.set(self.calls.get() + 1);
        let opened = self.engine.open(resume);
        assert!(!self.crash, "simulated process death after native opening");
        opened
    }
    fn start(&mut self, native: &str, turn: &str, input: &[u8]) -> crate::Result<()> {
        self.engine.start(native, turn, input)
    }
    fn step(&mut self, native: &str) -> crate::engine::Observation {
        self.engine.step(native)
    }
    fn steer(&mut self, native: &str, turn: &str, correction: &[u8]) -> crate::Result<bool> {
        self.engine.steer(native, turn, correction)
    }
    fn interrupt(&mut self, native: &str, turn: &str) -> crate::Result<()> {
        self.engine.interrupt(native, turn)
    }
    fn answer(&mut self, native: &str, id: &str, response: &[u8]) -> crate::Result<()> {
        self.engine.answer(native, id, response)
    }
    fn resume_state(&mut self, native: &str) -> Option<Vec<u8>> {
        self.engine.resume_state(native)
    }
    fn captures(&mut self) -> Vec<crate::engine::Capture> {
        self.engine.captures()
    }
    fn started(&self) -> usize {
        self.engine.started()
    }
}

#[test]
fn opening_intent_survives_lost_acknowledgment_and_process_death() {
    for crash in [false, true] {
        let mut fx = Fx::new();
        let calls = Rc::new(std::cell::Cell::new(0));
        let mut engine = LoopEngine::mid_turn();
        engine.fail_open = !crash;
        let host = SessionHost::open(
            fx.root.clone(),
            secret(HOST),
            vec![Box::new(OpeningEngine {
                engine,
                root: fx.root.clone(),
                calls: calls.clone(),
                crash,
            })],
            Box::new(fx.bindings.clone()),
        )
        .unwrap();
        let adapter = host.admit_adapter("loop-mid-turn", fx.now).unwrap();
        let body = fx.open_body(&host, &adapter, json!({"kind":"new","artifact":null}));
        let session = body["session"].as_str().unwrap().to_string();
        let binding = fx.binding();
        let bundle = fx.bundle(OWNER, &body);
        let initial = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            host.submit(&bundle, Some(&binding), fx.now).unwrap()
        }));
        if crash {
            assert!(initial.is_err());
        } else {
            assert_eq!(initial.unwrap()["status"], "unknown");
        }
        assert_eq!(calls.get(), 1);
        let retry = fx.send(&host, OWNER, &body, Some(&binding));
        assert_eq!(retry["status"], "unknown");
        assert_eq!(calls.get(), 1);
        assert_eq!(host.status(&session).unwrap()["phase"], "unknown");
        let restarted = fx.host();
        assert_eq!(fx.send(&restarted, OWNER, &body, Some(&binding)), retry);
        assert_eq!(restarted.started("loop-mid-turn"), 0);
        assert_eq!(
            restarted.observe(&session, fx.now).unwrap()["phase"],
            "unknown"
        );
        let close = fx.command(&restarted, &session, "close", json!({"reason":"done"}));
        assert_eq!(
            fx.send(&restarted, OWNER, &close, None)["status"],
            "refused"
        );
        let mut changed = body.clone();
        changed["command"] = json!(fx.next());
        assert_eq!(
            fx.send(&restarted, OWNER, &changed, Some(&binding))["status"],
            "conflict"
        );
        // Cached uncertainty is still private after owner revocation.
        restarted.set_policy(Policy::default()).unwrap();
        assert_eq!(
            fx.send(&restarted, OWNER, &body, Some(&binding))["status"],
            "refused"
        );
    }
}

#[test]
fn an_unknown_resume_keeps_its_lineage_exclusive() {
    let mut fx = Fx::new();
    let host = fx.host();
    let (session, adapter) = fx.open(&host, "loop-mid-turn");
    let history = host.export(&session, fx.now).unwrap();
    let bytes = host.store().get(&history).unwrap();
    fx.artifacts.insert(digest_bytes(&bytes), bytes);
    let close = fx.command(
        &host,
        &session,
        "close",
        json!({"reason":"resume elsewhere"}),
    );
    assert_eq!(fx.send(&host, OWNER, &close, None)["status"], "accepted");
    let mut engine = LoopEngine::mid_turn();
    engine.fail_open = true;
    let host = fx.host_with(engine, LoopEngine::boundary());
    let resume = fx.open_body(&host, &adapter, json!({"kind":"resume","artifact":history}));
    let binding = fx.binding();
    assert_eq!(
        fx.send(&host, OWNER, &resume, Some(&binding))["status"],
        "unknown"
    );
    let resumed = resume["session"].as_str().unwrap().to_string();
    let close = fx.command(&host, &resumed, "close", json!({"reason":"try again"}));
    assert_eq!(fx.send(&host, OWNER, &close, None)["status"], "refused");
    let other = fx.open_body(&host, &adapter, json!({"kind":"resume","artifact":history}));
    assert_eq!(
        fx.send(&host, OWNER, &other, Some(&binding))["reason"],
        "conflict"
    );
}
