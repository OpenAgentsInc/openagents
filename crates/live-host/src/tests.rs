use crate::{
    controller::{Adapters, Bundle, Controller, Policy, envelope},
    media::{LocalBroker, SyntheticDevice, SyntheticSource},
};
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::Event;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const HOST: u8 = 1;
const OWNER: u8 = 2;
const ALICE: u8 = 3;
const AGENT: u8 = 4;
const BOB: u8 = 5;
const STRANGER: u8 = 6;
const TASK: &str = "7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a";

fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut v =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        v["schema"] = json!(schema);
    }
    v
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(map, &jcs(body).unwrap(), body["v"].as_str())
}
fn reference(body: &Value) -> Value {
    let bytes = jcs(body).unwrap();
    json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":body["v"]})
}
fn bound(name: &str, ceiling: u64) -> Value {
    json!({"bound":name,"ceiling":ceiling,"assurance":"host","mechanism":"live-host"})
}

struct Fx {
    _temp: tempfile::TempDir,
    root: PathBuf,
    artifacts: BTreeMap<String, Vec<u8>>,
    now: u64,
    counter: u8,
    session: String,
    plan: Value,
    approvals: BTreeMap<String, Value>,
    input_declarations: Vec<Event>,
    asr: Value,
    destination: Value,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("live");
        Controller::initialize(
            root.clone(),
            secret(HOST),
            Policy {
                owners: BTreeSet::from([pk(OWNER)]),
                tasks: BTreeSet::from([TASK.to_string()]),
            },
        )
        .unwrap();
        let mut artifacts = BTreeMap::new();
        let asr = json!({"id":format!("{}:live/asr",pk(HOST)),"artifact":put(&mut artifacts,b"asr build",None)});
        let destination = put(&mut artifacts, b"recipient: transcription service", None);
        Self {
            _temp: temp,
            root,
            artifacts,
            now: crate::now(),
            counter: 0x40,
            session: "5e".repeat(32),
            plan: Value::Null,
            approvals: BTreeMap::new(),
            input_declarations: Vec::new(),
            asr,
            destination,
        }
    }
    fn controller(&self) -> Controller {
        self.controller_with(LocalBroker::default(), SyntheticDevice::default())
    }
    fn controller_with(&self, broker: LocalBroker, device: SyntheticDevice) -> Controller {
        Controller::open(
            self.root.clone(),
            secret(HOST),
            Adapters {
                broker: Box::new(broker),
                source: Box::new(SyntheticSource::default()),
                device: Box::new(device),
            },
        )
        .unwrap()
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        format!("{:02x}", self.counter).repeat(32)
    }
    fn plan_body(
        &mut self,
        previous: Value,
        revision: u64,
        epoch: u64,
        on_disconnect: &str,
    ) -> Value {
        let mut a = std::mem::take(&mut self.artifacts);
        let group = put(&mut a, b"group disclosure", None);
        let conversation = put_json(
            &mut a,
            &json!({"v":"openagents.live-conversation.v1","requires":[],"scope":"group",
            "participants":[pk(OWNER),pk(ALICE),pk(AGENT),pk(BOB)],"disclosure":group}),
        );
        let consent = put_json(
            &mut a,
            &json!({"v":"openagents.live-consent-policy.v1","requires":[],
            "requires_consent":["record","transcribe"],"on_disconnect":on_disconnect,"max_revocation_delay_ms":0}),
        );
        let reservation = put_json(
            &mut a,
            &json!({"v":"openagents.reservation.v1","requires":[],"id":"aa".repeat(32),
            "parent":null,"currency":"SAT","ceiling_microunits":1000,"spent_microunits":0,"unknown":false}),
        );
        let tracks = json!([
            {"id":"mic","source":{"scope":"device","id":"mic-1"},"media":"audio","direction":"capture",
             "recipients":[self.destination],"retention":{"mode":"bounded","until":self.now+3600},"processing":[self.asr]},
            {"id":"screen","source":{"scope":"display","id":"window-7"},"media":"image","direction":"capture",
             "recipients":[],"retention":{"mode":"bounded","until":self.now+3600},"processing":[]},
            {"id":"speaker","source":{"scope":"device","id":"out-1"},"media":"audio","direction":"playback",
             "recipients":[],"retention":{"mode":"none","until":null},"processing":[]}]);
        let snapshot = put_json(
            &mut a,
            &json!({"v":"openagents.snapshot.v1","requires":[],
            "scope":format!("live/{TASK}"),"captured_at":self.now,"sources":[],"coverage":"complete"}),
        );
        let precedence = put_json(&mut a, &json!({"order":[]}));
        let instructions = put_json(
            &mut a,
            &json!({"v":"openagents.instructions.v1","requires":[],
            "task":TASK,"revision":0,"resolver":{"id":format!("{}:live/empty",pk(OWNER)),"artifact":precedence},
            "precedence":precedence,"snapshot":snapshot,"entries":[]}),
        );
        let objective = put_json(&mut a, &json!({"task":"bounded live interaction"}));
        let acceptance = put_json(&mut a, &json!({"criterion":"exact input and observation"}));
        let frame = put_json(
            &mut a,
            &json!({"v":"openagents.task-frame.v1","requires":[],"task":TASK,
            "owner":pk(OWNER),"controller":pk(OWNER),"revision":0,"previous":null,"objective":objective,
            "origin":"user","constraints":[],"acceptance":[acceptance],"snapshot":snapshot,"instructions":instructions,
            "variables":[],"attempts":[],"unresolved":[],"corrections":[]}),
        );
        let body = json!({"v":"openagents.live-plan.v1","requires":[],"session":self.session,"task":TASK,
            "previous":previous,"expected_revision":revision,"frame":frame,"conversation":conversation,
            "owner":pk(OWNER),"controller":pk(HOST),"generation":0,"media_epoch":epoch,
            "transport":{"id":format!("{}:live/transport",pk(HOST)),"artifact":put(&mut a,b"transport profile",None)},
            "bindings":[put(&mut a,b"capture binding",None)],"tracks":tracks,"disclosure":put(&mut a,b"disclosure",None),
            "consent_policy":consent,"reservation":reservation,
            "bounds":[bound("wall_ms",3_600_000),bound("storage_bytes",1_000_000),bound("concurrency",4)],
            "expires_at":self.now+3600});
        self.artifacts = a;
        body
    }
    fn bundle(&self, signer: u8, body: &Value, declarations: Vec<Event>) -> Bundle {
        Bundle {
            event: envelope(&secret(signer), &pk(HOST), body, self.now).unwrap(),
            artifacts: self.artifacts.clone(),
            declarations: declarations
                .into_iter()
                .chain(self.input_declarations.clone())
                .collect(),
        }
    }
    fn send(&self, c: &Controller, signer: u8, body: &Value) -> Value {
        c.submit(&self.bundle(signer, body, Vec::new()), self.now)
            .unwrap()
    }
    fn admit_plan(&mut self, c: &Controller) {
        let body = self.plan_body(Value::Null, 0, 0, "pause");
        let result = self.send(c, OWNER, &body);
        assert_eq!(result["status"], "accepted", "{result}");
        self.plan = reference(&body);
    }
    fn revision(&self, c: &Controller) -> Value {
        c.status(&self.session).unwrap()["revision"].clone()
    }
    fn consent(&mut self, who: u8, rights: Value, tracks: Value) -> (Value, Event) {
        let body = json!({"v":"openagents.live-consent.v1","requires":[],"plan":self.plan,"participant":pk(who),
            "rights":rights,"tracks":tracks,"expires_at":self.now+1800});
        (
            reference(&body),
            envelope(&secret(who), &pk(HOST), &body, self.now).unwrap(),
        )
    }
    fn participant(
        &mut self,
        c: &Controller,
        signer: u8,
        who: u8,
        rights: Value,
        tracks: Value,
        consent: Vec<(Value, Event)>,
    ) -> Value {
        let body = json!({"v":"openagents.live-participant.v1","requires":[],"request":self.next(),"plan":self.plan,
            "participant":pk(who),"action":"admit","expected_revision":self.revision(c),"tracks":tracks,"rights":rights,
            "consent":consent.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),"expires_at":self.now+1800});
        c.submit(
            &self.bundle(signer, &body, consent.into_iter().map(|(_, e)| e).collect()),
            self.now,
        )
        .unwrap()
    }
    fn control(&mut self, c: &Controller, action: &str) -> (Value, Value) {
        let body = json!({"v":"openagents.live-control.v1","requires":[],"request":self.next(),"plan":self.plan,
            "action":action,"expected_revision":self.revision(c),"reason":""});
        (self.send(c, OWNER, &body), body)
    }
    /// Plan, Alice (publisher with consent), Bob (publisher), agent (playback), started.
    fn active(&mut self, c: &Controller) {
        self.admit_plan(c);
        let consent = self.consent(
            ALICE,
            json!(["record", "transcribe", "publish"]),
            json!(["mic", "screen"]),
        );
        assert_eq!(
            self.participant(
                c,
                OWNER,
                ALICE,
                json!(["publish", "record"]),
                json!(["mic", "screen"]),
                vec![consent]
            )["status"],
            "accepted"
        );
        assert_eq!(
            self.participant(c, OWNER, BOB, json!(["publish"]), json!(["mic"]), vec![])["status"],
            "accepted"
        );
        assert_eq!(
            self.participant(
                c,
                OWNER,
                AGENT,
                json!(["playback", "subscribe"]),
                json!(["speaker", "mic"]),
                vec![]
            )["status"],
            "accepted"
        );
        assert_eq!(self.control(c, "start").0["status"], "accepted");
    }
    fn input_floor(&mut self, c: &Controller, who: u8, nonce: &str) -> Value {
        let body = json!({"v":"openagents.live-input-floor-request.v1","requires":[],"request":self.next(),"plan":self.plan,
            "participant":pk(who),"track":"mic","destination":self.destination,"processing":self.asr,
            "expected_revision":self.revision(c),"action":"acquire","until":self.now+600,"nonce":nonce});
        self.send(c, who, &body)
    }
    fn speak_floor(&mut self, c: &Controller, who: u8, nonce: &str) -> Value {
        let body = json!({"v":"openagents.live-floor-request.v1","requires":[],"request":self.next(),"plan":self.plan,
            "participant":pk(who),"track":"speaker","expected_revision":self.revision(c),"action":"acquire",
            "until":self.now+600,"nonce":nonce});
        self.send(c, who, &body)
    }
    fn capture(&mut self, c: &Controller, track: &str, purpose: &str) -> Value {
        let body = json!({"v":"openagents.live-capture.v1","requires":[],"request":self.next(),"plan":self.plan,
            "track":track,"expected_revision":self.revision(c),"limits":[bound("storage_bytes",10_000)],"purpose":purpose});
        self.send(c, OWNER, &body)
    }
}

#[test]
fn participants_need_controller_admission_and_their_own_consent() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.admit_plan(&c);
    // Recording needs Alice's own signed consent; the owner cannot supply it.
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            ALICE,
            json!(["publish", "record"]),
            json!(["mic"]),
            vec![]
        )["reason"],
        "not_admitted"
    );
    let forged = {
        let body = json!({"v":"openagents.live-consent.v1","requires":[],"plan":fx.plan,"participant":pk(ALICE),
            "rights":["record"],"tracks":["mic"],"expires_at":fx.now+1800});
        (
            reference(&body),
            envelope(&secret(OWNER), &pk(HOST), &body, fx.now).unwrap(),
        )
    };
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            ALICE,
            json!(["publish", "record"]),
            json!(["mic"]),
            vec![forged]
        )["reason"],
        "not_admitted"
    );
    // Participants cannot admit themselves.
    let consent = fx.consent(ALICE, json!(["record"]), json!(["mic"]));
    assert_eq!(
        fx.participant(
            &c,
            ALICE,
            ALICE,
            json!(["publish", "record"]),
            json!(["mic"]),
            vec![consent.clone()]
        )["reason"],
        "not_admitted"
    );
    // A right must fit its tracks' direction.
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            AGENT,
            json!(["playback"]),
            json!(["mic"]),
            vec![]
        )["reason"],
        "incompatible"
    );
    // Strangers outside the conversation are refused.
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            STRANGER,
            json!(["subscribe"]),
            json!(["mic"]),
            vec![]
        )["reason"],
        "not_admitted"
    );
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            ALICE,
            json!(["publish", "record"]),
            json!(["mic"]),
            vec![consent]
        )["status"],
        "accepted"
    );
    let token = c.take_credential(&fx.session, &pk(ALICE)).unwrap();
    assert!(c.take_credential(&fx.session, &pk(ALICE)).is_none());
    c.with_adapters(|a| {
        assert!(
            a.broker
                .redeem(&token, &pk(ALICE), "mic", "publish", fx.now)
                .is_ok()
        );
        assert!(
            a.broker
                .redeem(&token, &pk(STRANGER), "mic", "publish", fx.now)
                .is_err()
        );
        assert!(
            a.broker
                .redeem(&token, &pk(ALICE), "mic", "playback", fx.now)
                .is_err()
        );
    });
    // The credential never reaches controller state or artifacts.
    for entry in walk(&fx.root) {
        let bytes = std::fs::read(&entry).unwrap();
        assert!(
            !bytes.windows(token.len()).any(|w| w == token.as_bytes()),
            "token leaked into {}",
            entry.display()
        );
    }
    // Lifecycle: status reads do not mutate; exact retries are duplicates.
    let revision = fx.revision(&c);
    fx.control(&c, "status");
    assert_eq!(fx.revision(&c), revision);
    let (started, body) = fx.control(&c, "start");
    assert_eq!(started["status"], "accepted");
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "duplicate");
    let mut changed = body.clone();
    changed["reason"] = json!("again");
    assert_eq!(
        fx.send(&c, OWNER, &changed)["reason"],
        "idempotency_conflict"
    );
}

fn walk(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn input_floors_bind_one_source_to_one_destination_and_epoch() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    // Playback rights are not input authority.
    assert_eq!(
        fx.input_floor(&c, AGENT, &"a1".repeat(32))["reason"],
        "not_admitted"
    );
    let nonce = "b1".repeat(32);
    let lease = fx.input_floor(&c, ALICE, &nonce);
    assert_eq!(lease["status"], "accepted", "{lease}");
    assert_eq!(
        c.store().json(&lease["value"]).unwrap()["participant"],
        pk(ALICE)
    );
    assert_eq!(
        fx.input_floor(&c, BOB, &"b2".repeat(32))["reason"],
        "conflict"
    );
    let epoch = c.status(&fx.session).unwrap()["epoch"].as_u64().unwrap();
    c.forward(
        &fx.session,
        &nonce,
        &pk(ALICE),
        "mic",
        epoch,
        b"pcm",
        fx.now,
    )
    .unwrap();
    // Wrong-member audio stays outside the processing context.
    assert!(
        c.forward(&fx.session, &nonce, &pk(BOB), "mic", epoch, b"pcm", fx.now)
            .is_err()
    );
    let next = c.reconnect(&fx.session, fx.now).unwrap();
    assert_eq!(next, epoch + 1);
    // Old-room audio and old leases are fenced after reconnection.
    assert!(
        c.forward(
            &fx.session,
            &nonce,
            &pk(ALICE),
            "mic",
            epoch,
            b"pcm",
            fx.now
        )
        .unwrap_err()
        .starts_with("stale")
    );
    assert!(
        c.forward(&fx.session, &nonce, &pk(ALICE), "mic", next, b"pcm", fx.now)
            .is_err()
    );
    assert_eq!(c.status(&fx.session).unwrap()["disclosed_bytes"], 3);
}

#[test]
fn speaking_floors_yield_to_barge_in_and_never_replay() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    assert_eq!(
        fx.speak_floor(&c, ALICE, &"c0".repeat(32))["reason"],
        "not_admitted"
    );
    let nonce = "c1".repeat(32);
    assert_eq!(fx.speak_floor(&c, AGENT, &nonce)["status"], "accepted");
    assert_eq!(
        fx.speak_floor(&c, AGENT, &"c2".repeat(32))["reason"],
        "conflict"
    );
    let epoch = c.status(&fx.session).unwrap()["epoch"].as_u64().unwrap();
    c.play(&fx.session, "speaker", &nonce, epoch, fx.now)
        .unwrap();
    let interruption = c
        .barge_in(&fx.session, "speaker", 1200, 1800, fx.now)
        .unwrap();
    let body = c.store().json(&interruption).unwrap();
    assert_eq!(
        (
            body["confirmed_ms"].clone(),
            body["uncertain_until_ms"].clone()
        ),
        (json!(1200), json!(1800))
    );
    assert!(
        c.play(&fx.session, "speaker", &nonce, epoch, fx.now)
            .is_err()
    );
    let nonce = "c3".repeat(32);
    assert_eq!(fx.speak_floor(&c, AGENT, &nonce)["status"], "accepted");
    let next = c.reconnect(&fx.session, fx.now).unwrap();
    assert!(
        c.play(&fx.session, "speaker", &nonce, next, fx.now)
            .is_err()
    );
}

#[test]
fn captures_and_transcripts_keep_scope_clock_and_corrections() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    assert_eq!(
        fx.capture(&c, "speaker", "observation")["reason"],
        "not_admitted"
    );
    let screen = fx.capture(&c, "screen", "observation");
    assert_eq!(screen["status"], "accepted", "{screen}");
    let observation = c.store().json(&screen["value"]).unwrap();
    assert_eq!(observation["geometry"]["width_px"], 1280);
    // Recording the mic needs every publisher's recording consent; Bob gave none.
    assert_eq!(fx.capture(&c, "mic", "recording")["reason"], "not_admitted");
    // A different purpose cannot bypass consent while retaining the same bytes.
    assert_eq!(
        fx.capture(&c, "mic", "task_input")["reason"],
        "not_admitted"
    );
    let consent = fx.consent(BOB, json!(["record", "transcribe"]), json!(["mic"]));
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            BOB,
            json!(["publish"]),
            json!(["mic"]),
            vec![consent]
        )["status"],
        "accepted"
    );
    let mic = fx.capture(&c, "mic", "task_input");
    let mic_ref = mic["value"].clone();
    let mic_body = c.store().json(&mic_ref).unwrap();
    let interval = mic_body["interval"].clone();
    let text = c
        .store()
        .put(b"\"fix the build\"", "application/json", None)
        .unwrap();
    let segment = |clock: &str, start: u64, end: u64| json!({"id":"s1","interval":{"clock":clock,"start_ms":start,"end_ms":end},"text":text,"speaker":"Alice?","final":false});
    let wrong = json!({"id":format!("{}:live/other",pk(HOST)),"artifact":c.store().put(b"other",  "application/json", None).unwrap()});
    assert!(
        c.transcribe(&fx.session, &mic_ref, &wrong, vec![], "partial", fx.now)
            .is_err()
    );
    let start = interval["start_ms"].as_u64().unwrap();
    assert!(
        c.transcribe(
            &fx.session,
            &mic_ref,
            &fx.asr,
            vec![segment("other-clock", start, start + 10)],
            "partial",
            fx.now
        )
        .unwrap_err()
        .starts_with("incompatible")
    );
    assert!(
        c.transcribe(
            &fx.session,
            &mic_ref,
            &fx.asr,
            vec![segment("host-monotonic", start, start + 99_999)],
            "partial",
            fx.now
        )
        .is_err()
    );
    let first = c
        .transcribe(
            &fx.session,
            &mic_ref,
            &fx.asr,
            vec![segment("host-monotonic", start, start + 500)],
            "partial",
            fx.now,
        )
        .unwrap();
    let mut corrected = segment("host-monotonic", start, start + 500);
    corrected["final"] = json!(true);
    let second = c
        .transcribe(
            &fx.session,
            &mic_ref,
            &fx.asr,
            vec![corrected],
            "complete",
            fx.now,
        )
        .unwrap();
    let second = c.store().json(&second).unwrap();
    assert_eq!(second["revision"], 1);
    assert_eq!(second["previous"], first);
    assert!(
        c.transcribe(
            &fx.session,
            &mic_ref,
            &fx.asr,
            vec![],
            "partial",
            fx.now + 3601
        )
        .unwrap_err()
        .starts_with("not_admitted")
    );
}

fn appoint_input(fx: &Fx, c: &Controller) {
    c.set_input_grants(
        vec![crate::controller::InputGrant {
            plan: fx.plan.clone(),
            owner: pk(OWNER),
            approver: pk(OWNER),
            operation: c.device_operation().unwrap(),
            expires_at: fx.now + 1800,
        }],
        fx.now,
    )
    .unwrap();
}
fn input_proposal(fx: &mut Fx, c: &Controller, observation: &Value, input: &[u8]) -> Value {
    let mut a = std::mem::take(&mut fx.artifacts);
    let plan = c.store().json(&fx.plan).unwrap();
    let operation = c.device_operation().unwrap();
    let definition: Value =
        serde_json::from_slice(&c.store().get(&operation["artifact"]).unwrap()).unwrap();
    let binding = c.store().get(&definition["meta"]["binding"]).unwrap();
    let body = json!({"v":"openagents.live-input.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "frame":plan["frame"],"observation":observation,"operation":c.device_operation().unwrap(),
        "binding":put(&mut a,&binding,None),"input":put(&mut a,input,None),"preconditions":[]});
    let reference = put_json(&mut a, &body);
    fx.artifacts = a;
    reference
}
fn proposal(fx: &mut Fx, c: &Controller, observation: &Value, input: &[u8]) -> (Value, Value) {
    let reference = input_proposal(fx, c, observation, input);
    let body: Value =
        serde_json::from_slice(&fx.artifacts[reference["digest"].as_str().unwrap()]).unwrap();
    let prepared = fx.send(c, OWNER, &body);
    assert_eq!(prepared["status"], "accepted", "{prepared}");
    let request = prepared["value"].clone();
    let action = c.store().json(&request).unwrap()["action"].clone();
    fx.approvals
        .insert(action["digest"].as_str().unwrap().into(), request);
    (reference, action)
}
fn admission(
    fx: &mut Fx,
    c: &Controller,
    proposal: &Value,
    action: &Value,
    reason: &[u8],
) -> Value {
    let request = fx.approvals[action["digest"].as_str().unwrap()].clone();
    let request_body = c.store().json(&request).unwrap();
    let decision = json!({"v":nostr::policy::DECISION_SCHEMA,"requires":[],"request":request,"action":action,
        "approver":pk(OWNER),"decision":"approve","expires_at":request_body["expires_at"],
        "reason":String::from_utf8(reason.to_vec()).unwrap()});
    let approval = put_json(&mut fx.artifacts, &decision);
    fx.input_declarations.push(
        workspace_host::host::request(&secret(OWNER), &pk(HOST), decision.clone(), fx.now).unwrap(),
    );
    json!({"v":"openagents.live-input-admission.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "expected_revision":fx.revision(c),"proposal":proposal,"action":action,"approvals":[approval]})
}

#[test]
fn device_input_needs_a_fresh_observation_and_never_retries_a_lost_reply() {
    let mut fx = Fx::new();
    let device = SyntheticDevice {
        lose_reply: false,
        ..SyntheticDevice::default()
    };
    let c = fx.controller_with(LocalBroker::default(), device);
    fx.active(&c);
    appoint_input(&fx, &c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (p, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    // A changed action cannot borrow the original signed decision.
    let mut changed_action = c.store().json(&action).unwrap();
    changed_action["input"] = put(&mut fx.artifacts, b"another proposal", None);
    let other = put_json(&mut fx.artifacts, &changed_action);
    let mut wrong = admission(&mut fx, &c, &p, &action, b"approval");
    wrong["action"] = other;
    assert_eq!(fx.send(&c, OWNER, &wrong)["reason"], "not_admitted");
    let body = admission(&mut fx, &c, &p, &action, b"approval");
    let applied = fx.send(&c, OWNER, &body);
    assert_eq!(applied["status"], "accepted", "{applied}");
    assert_eq!(
        c.store().json(&applied["value"]).unwrap()["outcome"],
        "applied"
    );
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "duplicate");
    let changed = admission(&mut fx, &c, &p, &action, b"another approval");
    assert_eq!(
        fx.send(&c, OWNER, &changed)["reason"],
        "idempotency_conflict"
    );
    // A focus change makes the old observation stale.
    c.with_adapters(|a| {
        a.device = Box::new(SyntheticDevice {
            focus: 1,
            lose_reply: true,
            ..SyntheticDevice::default()
        })
    });
    let p2 = input_proposal(&mut fx, &c, &observation, br#"{"click":[1,1]}"#);
    let stale: Value =
        serde_json::from_slice(&fx.artifacts[p2["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(fx.send(&c, OWNER, &stale)["reason"], "stale");
    let fresh = fx.capture(&c, "screen", "observation")["value"].clone();
    let (p3, action3) = proposal(&mut fx, &c, &fresh, br#"{"type":"text"}"#);
    let lost = admission(&mut fx, &c, &p3, &action3, b"approval");
    let unknown = fx.send(&c, OWNER, &lost);
    assert_eq!(unknown["status"], "unknown");
    assert_eq!(fx.send(&c, OWNER, &lost)["status"], "unknown");
    let applied = c.with_adapters(|a| a.device.state());
    assert_eq!(applied, (0, 1));
}

#[test]
fn stop_with_an_unconfirmed_revocation_stays_unknown() {
    let mut fx = Fx::new();
    let mut broker = LocalBroker::default();
    broker.revocation_unknown = true;
    let c = fx.controller_with(broker, SyntheticDevice::default());
    fx.active(&c);
    let (stopped, body) = fx.control(&c, "stop");
    assert_eq!(stopped["status"], "unknown");
    assert_eq!(stopped["state"], "unknown");
    assert!(
        !c.status(&fx.session).unwrap()["unresolved"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "unknown");
}

#[test]
fn replacement_plans_advance_the_epoch_and_fence_old_admissions() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let old = fx.plan.clone();
    let skipped = fx.plan_body(old.clone(), fx.revision(&c).as_u64().unwrap(), 3, "pause");
    assert_eq!(fx.send(&c, OWNER, &skipped)["reason"], "stale");
    let body = fx.plan_body(old.clone(), fx.revision(&c).as_u64().unwrap(), 1, "stop");
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "accepted");
    fx.plan = reference(&body);
    let status = c.status(&fx.session).unwrap();
    assert_eq!(status["epoch"], 1);
    assert_eq!(status["members"], json!([]));
    assert_eq!(status["state"], "admitted");
    // A request against the superseded plan is stale.
    let control = json!({"v":"openagents.live-control.v1","requires":[],"request":fx.next(),"plan":old,
        "action":"start","expected_revision":fx.revision(&c),"reason":""});
    assert_eq!(fx.send(&c, OWNER, &control)["reason"], "stale");
    // The pinned policy, not a guess, decides what a disconnect does.
    assert_eq!(fx.control(&c, "start").0["status"], "accepted");
    assert_eq!(c.disconnect(&fx.session, fx.now).unwrap(), "closed");
    // Joining clients cannot pick the controller generation or epoch.
    let mut fx2 = Fx::new();
    fx2.session = "6e".repeat(32);
    let mut body = fx2.plan_body(Value::Null, 0, 0, "pause");
    body["generation"] = json!(4);
    let c2 = fx2.controller();
    assert_eq!(fx2.send(&c2, OWNER, &body)["reason"], "stale");
}

#[test]
fn input_checks_at_the_native_boundary_and_never_falls_back_to_unchecked_input() {
    use crate::media::{Applied, Device, InputGuard};
    use std::{cell::RefCell, rc::Rc};
    struct Racing(Rc<RefCell<SyntheticDevice>>);
    impl Device for Racing {
        fn input_operation(&self) -> Option<crate::media::InputOperation> {
            self.0.borrow().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            self.0.borrow().state()
        }
        fn apply(&mut self, _operation: &str, _input: &[u8]) -> Applied {
            panic!("unchecked input")
        }
        fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
            let mut device = self.0.borrow_mut();
            device.focus += 1;
            device.apply_checked(operation, input, guard)
        }
    }
    struct Unchecked;
    impl Device for Unchecked {
        fn input_operation(&self) -> Option<crate::media::InputOperation> {
            SyntheticDevice::default().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            (0, 0)
        }
        fn apply(&mut self, _operation: &str, _input: &[u8]) -> Applied {
            panic!("unchecked input")
        }
    }
    for racing in [false, true] {
        let mut fx = Fx::new();
        let c = fx.controller();
        fx.active(&c);
        appoint_input(&fx, &c);
        let observation = fx.capture(&c, "screen", "observation")["value"].clone();
        let device = Rc::new(RefCell::new(SyntheticDevice::default()));
        c.with_adapters(|a| {
            a.device = if racing {
                Box::new(Racing(device.clone()))
            } else {
                Box::new(Unchecked)
            }
        });
        let (proposal, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
        let body = admission(&mut fx, &c, &proposal, &action, b"approval");
        let result = fx.send(&c, OWNER, &body);
        assert_eq!(result["status"], "refused", "{result}");
        let outcome = c.store().json(&result["value"]).unwrap();
        assert_eq!(outcome["dispatched"], false);
        assert_eq!(outcome["outcome"], "refused");
        assert_eq!(
            outcome["reason"],
            if racing {
                "stale"
            } else {
                "unsupported_feature"
            }
        );
        let _ = fx.send(&c, OWNER, &body);
        assert!(device.borrow().applied.is_empty());
        assert_eq!(device.borrow().focus, if racing { 1 } else { 0 });
    }
}

#[test]
fn device_input_cannot_refresh_a_capture_clock_by_restart_or_wall_time() {
    for restart in [false, true] {
        let mut fx = Fx::new();
        let c = fx.controller();
        fx.active(&c);
        appoint_input(&fx, &c);
        let observation = fx.capture(&c, "screen", "observation")["value"].clone();
        let (proposal, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
        let c = if restart {
            drop(c);
            fx.controller()
        } else {
            for clock in c.capture_clocks.borrow_mut().values_mut() {
                *clock = std::time::Instant::now() - std::time::Duration::from_secs(31);
            }
            c
        };
        let body = admission(&mut fx, &c, &proposal, &action, b"approval");
        let result = fx.send(&c, OWNER, &body);
        assert_eq!(result["status"], "refused", "{result}");
        assert_eq!(
            result["reason"],
            if restart { "cannot_enforce" } else { "stale" }
        );
    }
}

fn tracked_input(c: &Controller) -> std::rc::Rc<std::cell::RefCell<SyntheticDevice>> {
    use crate::media::{Applied, Device, InputGuard, InputOperation};
    struct Tracked(std::rc::Rc<std::cell::RefCell<SyntheticDevice>>);
    impl Device for Tracked {
        fn input_operation(&self) -> Option<InputOperation> {
            self.0.borrow().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            self.0.borrow().state()
        }
        fn apply(&mut self, _operation: &str, _input: &[u8]) -> Applied {
            panic!("unchecked input")
        }
        fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
            self.0.borrow_mut().apply_checked(operation, input, guard)
        }
    }
    let device = std::rc::Rc::new(std::cell::RefCell::new(SyntheticDevice::default()));
    c.with_adapters(|a| a.device = Box::new(Tracked(device.clone())));
    device
}

#[test]
fn device_input_approvals_need_current_appointment_and_original_exact_signed_decisions() {
    for mode in [
        "unsigned",
        "forged",
        "wrong-action",
        "expired",
        "long-expiry",
        "revoked",
        "regranted",
        "revision",
        "epoch",
    ] {
        let mut fx = Fx::new();
        let c = fx.controller();
        fx.active(&c);
        let device = tracked_input(&c);
        appoint_input(&fx, &c);
        let observation = fx.capture(&c, "screen", "observation")["value"].clone();
        let (proposal, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
        let mut body = admission(&mut fx, &c, &proposal, &action, b"approval");
        let mut decision: Value =
            serde_json::from_slice(&fx.artifacts[body["approvals"][0]["digest"].as_str().unwrap()])
                .unwrap();
        fx.input_declarations.clear();
        let mut signer = OWNER;
        match mode {
            "forged" => signer = STRANGER,
            "wrong-action" => {
                decision["action"]["digest"] = json!(format!("sha256:{}", "de".repeat(32)))
            }
            "expired" => decision["expires_at"] = json!(fx.now),
            "long-expiry" => {
                decision["expires_at"] = json!(decision["expires_at"].as_u64().unwrap() + 1)
            }
            "revoked" => {
                c.set_input_grants(Vec::new(), fx.now).unwrap();
            }
            "regranted" => appoint_input(&fx, &c),
            "revision" => {
                fx.capture(&c, "screen", "observation");
            }
            "epoch" => {
                c.reconnect(&fx.session, fx.now).unwrap();
            }
            _ => {}
        }
        body["approvals"] = json!([put_json(&mut fx.artifacts, &decision)]);
        body["expected_revision"] = fx.revision(&c);
        if mode != "unsigned" {
            fx.input_declarations.push(
                workspace_host::host::request(&secret(signer), &pk(HOST), decision, fx.now)
                    .unwrap(),
            );
        }
        let result = fx.send(&c, OWNER, &body);
        assert_eq!(result["status"], "refused", "{mode}: {result}");
        assert!(device.borrow().applied.is_empty(), "{mode}");
    }
}

#[test]
fn ownership_and_opaque_approval_bytes_cannot_prepare_or_admit_input() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let device = tracked_input(&c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let reference = input_proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let proposal_body: Value =
        serde_json::from_slice(&fx.artifacts[reference["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(fx.send(&c, OWNER, &proposal_body)["reason"], "not_admitted");
    appoint_input(&fx, &c);
    let (reference, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let approval = put(&mut fx.artifacts, b"approval", None);
    let body = json!({"v":"openagents.live-input-admission.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "expected_revision":fx.revision(&c),"proposal":reference,"action":action,"approvals":[approval]});
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "refused");
    assert!(device.borrow().applied.is_empty());
}

#[test]
fn input_denial_is_single_use_and_cannot_be_replaced_with_an_approval() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let device = tracked_input(&c);
    appoint_input(&fx, &c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (proposal_ref, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let mut body = admission(&mut fx, &c, &proposal_ref, &action, b"deny");
    let mut decision: Value =
        serde_json::from_slice(&fx.artifacts[body["approvals"][0]["digest"].as_str().unwrap()])
            .unwrap();
    decision["decision"] = json!("deny");
    body["approvals"] = json!([put_json(&mut fx.artifacts, &decision)]);
    fx.input_declarations =
        vec![workspace_host::host::request(&secret(OWNER), &pk(HOST), decision, fx.now).unwrap()];
    let denied = fx.send(&c, OWNER, &body);
    assert_eq!(denied["status"], "refused", "{denied}");
    let outcome = c.store().json(&denied["value"]).unwrap();
    assert_eq!(outcome["dispatched"], false);
    assert_eq!(outcome["outcome"], "refused");
    let approve = admission(&mut fx, &c, &proposal_ref, &action, b"approve later");
    assert_eq!(
        fx.send(&c, OWNER, &approve)["reason"],
        "idempotency_conflict"
    );
    assert!(device.borrow().applied.is_empty());
    let (fresh, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let approve = admission(&mut fx, &c, &fresh, &action, b"a new explicit action");
    assert_eq!(fx.send(&c, OWNER, &approve)["status"], "accepted");
    assert_eq!(device.borrow().applied.len(), 1);
}

#[test]
fn invalid_native_output_remains_unknown_without_repeating_the_input() {
    use crate::media::{Applied, Device, InputGuard, InputOperation};
    use std::{cell::RefCell, rc::Rc};
    struct Damaged(Rc<RefCell<SyntheticDevice>>);
    impl Device for Damaged {
        fn input_operation(&self) -> Option<InputOperation> {
            self.0.borrow().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            self.0.borrow().state()
        }
        fn apply(&mut self, _operation: &str, _input: &[u8]) -> Applied {
            panic!("unchecked input")
        }
        fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
            match self.0.borrow_mut().apply_checked(operation, input, guard) {
                Applied::Applied(_) => Applied::Applied(b"not-json".to_vec()),
                other => other,
            }
        }
    }
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let device = Rc::new(RefCell::new(SyntheticDevice::default()));
    c.with_adapters(|a| a.device = Box::new(Damaged(device.clone())));
    appoint_input(&fx, &c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (proposal, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let body = admission(&mut fx, &c, &proposal, &action, b"approve");
    let result = fx.send(&c, OWNER, &body);
    assert_eq!(result["status"], "unknown");
    let outcome = c.store().json(&result["value"]).unwrap();
    assert_eq!(outcome["outcome"], "unknown");
    assert_eq!(outcome["reason"], "malformed");
    assert_eq!(c.store().get(&outcome["observed"]).unwrap(), b"not-json");
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "unknown");
    let changed = admission(&mut fx, &c, &proposal, &action, b"again");
    assert_eq!(
        fx.send(&c, OWNER, &changed)["reason"],
        "idempotency_conflict"
    );
    assert_eq!(device.borrow().applied.len(), 1);
}

#[test]
fn input_authority_expiry_still_applies_inside_a_delayed_native_callback() {
    use crate::media::{Applied, Device, InputGuard, InputOperation};
    use std::{cell::RefCell, rc::Rc};
    struct Delayed(Rc<RefCell<SyntheticDevice>>);
    impl Device for Delayed {
        fn input_operation(&self) -> Option<InputOperation> {
            self.0.borrow().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            self.0.borrow().state()
        }
        fn apply(&mut self, _operation: &str, _input: &[u8]) -> Applied {
            panic!("unchecked input")
        }
        fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            self.0.borrow_mut().apply_checked(operation, input, guard)
        }
    }
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let device = Rc::new(RefCell::new(SyntheticDevice::default()));
    c.with_adapters(|a| a.device = Box::new(Delayed(device.clone())));
    c.set_input_grants(
        vec![crate::controller::InputGrant {
            plan: fx.plan.clone(),
            owner: pk(OWNER),
            approver: pk(OWNER),
            operation: c.device_operation().unwrap(),
            expires_at: fx.now + 1,
        }],
        fx.now,
    )
    .unwrap();
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (proposal, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    let body = admission(&mut fx, &c, &proposal, &action, b"approve");
    let result = fx.send(&c, OWNER, &body);
    assert_eq!(result["status"], "refused", "{result}");
    let outcome = c.store().json(&result["value"]).unwrap();
    assert_eq!(outcome["reason"], "stale");
    assert_eq!(outcome["dispatched"], false);
    assert!(device.borrow().applied.is_empty());
}

#[test]
fn device_input_contracts_refuse_changed_frames_bindings_and_script_inputs() {
    for mode in [
        "frame",
        "binding",
        "input",
        "operation",
        "conditions",
        "bounds",
    ] {
        let mut fx = Fx::new();
        let c = fx.controller();
        fx.active(&c);
        let device = tracked_input(&c);
        appoint_input(&fx, &c);
        let observation = fx.capture(&c, "screen", "observation")["value"].clone();
        let reference = input_proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
        let mut body: Value =
            serde_json::from_slice(&fx.artifacts[reference["digest"].as_str().unwrap()]).unwrap();
        match mode {
            "frame" => body["frame"] = put(&mut fx.artifacts, b"another frame", None),
            "binding" => body["binding"] = put(&mut fx.artifacts, b"another device", None),
            "input" => {
                body["input"] = put(
                    &mut fx.artifacts,
                    br#"{"shell":"must never execute"}"#,
                    None,
                )
            }
            "conditions" => {
                body["preconditions"] = json!([put_json(
                    &mut fx.artifacts,
                    &json!({"unsupported":"condition"})
                )])
            }
            "bounds" => body["input"] = put(&mut fx.artifacts, br#"{"click":[1280,800]}"#, None),
            "operation" => {
                body["operation"]["artifact"]["digest"] =
                    json!(format!("sha256:{}", "de".repeat(32)))
            }
            _ => unreachable!(),
        }
        let result = fx.send(&c, OWNER, &body);
        assert_eq!(result["status"], "refused", "{mode}: {result}");
        assert!(device.borrow().applied.is_empty());
    }
}

#[test]
fn input_cannot_read_another_signers_artifact_by_hash_or_carried_declaration() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let writes = tracked_input(&c);
    appoint_input(&fx, &c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let private = json!({"type":"another caller's private input"});
    let private_bytes = jcs(&private).unwrap();
    let mut supplied = BTreeMap::new();
    let private_ref = put(&mut supplied, &private_bytes, None);
    let stranger_status = json!({"v":"openagents.live-control.v1","requires":[],"request":fx.next(),
        "plan":fx.plan,"action":"status","expected_revision":0,"reason":""});
    let mut bundle = fx.bundle(STRANGER, &stranger_status, vec![]);
    bundle.artifacts = supplied;
    let refused = c.submit(&bundle, fx.now).unwrap();
    assert_eq!(refused["reason"], "not_admitted");
    assert!(refused["plan"].is_null());
    let reference = input_proposal(&mut fx, &c, &observation, b"{\"click\":[1,2]}");
    let mut proposal: Value =
        serde_json::from_slice(&fx.artifacts[reference["digest"].as_str().unwrap()]).unwrap();
    proposal["input"] = private_ref.clone();
    // The owner has the hash, but not the plaintext from the other request.
    assert_eq!(fx.send(&c, OWNER, &proposal)["reason"], "not_admitted");
    let mut inline_ref = private_ref.clone();
    inline_ref["schema"] = json!("private.input.v1");
    let encrypted = nostr::private_artifact::seal(
        &json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":inline_ref,
            "inline":private,"issued_at":fx.now,"retain_until":fx.now+3600}),
        &secret(STRANGER),
        &pk(HOST).parse().unwrap(),
        &fx.next(),
        fx.now,
        secp256k1::rand::random(),
    )
    .unwrap();
    let carried = fx.bundle(OWNER, &proposal, vec![encrypted]);
    assert_eq!(
        c.submit(&carried, fx.now).unwrap()["reason"],
        "not_admitted"
    );
    assert!(writes.borrow().applied.is_empty());
    // Supplying the actual bytes proves knowledge; preparation is still inert.
    fx.artifacts
        .insert(digest_bytes(&private_bytes), private_bytes);
    assert_eq!(fx.send(&c, OWNER, &proposal)["status"], "accepted");
    assert!(writes.borrow().applied.is_empty());
}

#[test]
fn current_session_rights_protect_cached_results_and_refusal_details() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let status = json!({"v":"openagents.live-control.v1","requires":[],"request":fx.next(),
        "plan":fx.plan,"action":"status","expected_revision":0,"reason":""});
    for who in [ALICE, STRANGER] {
        let result = fx.send(&c, who, &status);
        assert_eq!(result["reason"], "not_admitted");
        assert!(result["plan"].is_null());
        assert!(result["record"].is_null());
        assert_eq!(result["revision"], 0);
    }
    let floor = json!({"v":"openagents.live-input-floor-request.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "participant":pk(ALICE),"track":"mic","destination":fx.destination,"processing":fx.asr,
        "expected_revision":fx.revision(&c),"action":"acquire","until":fx.now+600,"nonce":"6b".repeat(32)});
    assert_eq!(fx.send(&c, ALICE, &floor)["status"], "accepted");
    assert_eq!(fx.send(&c, ALICE, &floor)["status"], "duplicate");
    let revoke = json!({"v":"openagents.live-participant.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "participant":pk(ALICE),"action":"revoke","expected_revision":fx.revision(&c),"tracks":[],"rights":[],
        "consent":[],"expires_at":fx.now+1800});
    assert_eq!(fx.send(&c, OWNER, &revoke)["status"], "accepted");
    let replay = fx.send(&c, ALICE, &floor);
    assert_eq!(replay["reason"], "not_admitted");
    assert!(replay["plan"].is_null());
    assert!(replay["value"].is_null());
    let mut self_revoke = revoke;
    self_revoke["request"] = json!(fx.next());
    assert_eq!(fx.send(&c, ALICE, &self_revoke)["reason"], "not_admitted");
}

#[test]
fn interrupted_device_write_recovers_unknown_without_reentering_the_device() {
    use crate::media::{Applied, Device, InputGuard, InputOperation};
    struct Crash(std::rc::Rc<std::cell::RefCell<SyntheticDevice>>);
    impl Device for Crash {
        fn input_operation(&self) -> Option<InputOperation> {
            self.0.borrow().input_operation()
        }
        fn state(&self) -> (u64, u64) {
            self.0.borrow().state()
        }
        fn apply(&mut self, _: &str, _: &[u8]) -> Applied {
            panic!("unchecked input")
        }
        fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
            assert!(matches!(
                self.0.borrow_mut().apply_checked(operation, input, guard),
                Applied::Applied(_)
            ));
            panic!("injected crash after native write");
        }
    }
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let writes = std::rc::Rc::new(std::cell::RefCell::new(SyntheticDevice::default()));
    c.with_adapters(|a| a.device = Box::new(Crash(writes.clone())));
    appoint_input(&fx, &c);
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (proposal, action) = proposal(&mut fx, &c, &observation, b"{\"click\":[10,20]}");
    let admission = admission(&mut fx, &c, &proposal, &action, b"approve");
    let bundle = fx.bundle(OWNER, &admission, vec![]);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.submit(&bundle, fx.now)))
            .is_err()
    );
    assert_eq!(writes.borrow().applied.len(), 1);
    drop(c);
    let c = fx.controller();
    let fresh = tracked_input(&c);
    // Original authority and capture may expire while recovering; observation
    // of a completed dispatch must never depend on sending another input.
    let result = c.submit(&bundle, fx.now + 2000).unwrap();
    assert_eq!(result["status"], "unknown", "{result}");
    let outcome = c.store().json(&result["value"]).unwrap();
    assert_eq!(outcome["dispatched"], true);
    assert_eq!(outcome["outcome"], "unknown");
    assert_eq!(c.submit(&bundle, fx.now + 2000).unwrap(), result);
    assert!(fresh.borrow().applied.is_empty());
    let mut changed = admission;
    changed["request"] = json!(fx.next());
    changed["expected_revision"] = fx.revision(&c);
    assert_eq!(
        fx.send(&c, OWNER, &changed)["reason"],
        "idempotency_conflict"
    );
    assert!(fresh.borrow().applied.is_empty());
}

#[path = "tests/document.rs"]
mod document;

#[cfg(unix)]
#[path = "tests/resident.rs"]
mod resident;
