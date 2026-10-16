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
        let body = json!({"v":"openagents.live-plan.v1","requires":[],"session":self.session,"task":TASK,
            "previous":previous,"expected_revision":revision,"frame":put(&mut a,b"frame",None),"conversation":conversation,
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
            declarations,
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
}

fn proposal(fx: &mut Fx, c: &Controller, observation: &Value, input: &[u8]) -> (Value, Value) {
    let mut a = std::mem::take(&mut fx.artifacts);
    let body = json!({"v":"openagents.live-input.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "frame":put(&mut a,b"frame",None),"observation":observation,
        "operation":{"id":format!("{}:live/click",pk(HOST)),"artifact":put(&mut a,b"click op",None)},
        "binding":put(&mut a,b"pointer binding",None),"input":put(&mut a,input,None),"preconditions":[]});
    let reference = put_json(&mut a, &body);
    let action = put(
        &mut a,
        &serde_json::to_vec(&json!({"proposal":reference})).unwrap(),
        None,
    );
    fx.artifacts = a;
    let _ = c;
    (reference, action)
}

fn admission(
    fx: &mut Fx,
    c: &Controller,
    proposal: &Value,
    action: &Value,
    approval: &[u8],
) -> Value {
    let approval = put(&mut fx.artifacts, approval, None);
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
    let observation = fx.capture(&c, "screen", "observation")["value"].clone();
    let (p, action) = proposal(&mut fx, &c, &observation, br#"{"click":[10,20]}"#);
    // An action that pins another proposal is refused.
    let other = put(&mut fx.artifacts, br#"{"proposal":"other"}"#, None);
    let wrong = admission(&mut fx, &c, &p, &other, b"approval");
    assert_eq!(fx.send(&c, OWNER, &wrong)["reason"], "identity_mismatch");
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
    let (p2, action2) = proposal(&mut fx, &c, &observation, br#"{"click":[1,1]}"#);
    let stale = admission(&mut fx, &c, &p2, &action2, b"approval");
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
