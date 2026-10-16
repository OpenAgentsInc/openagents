use super::*;
use crate::{
    delivery::{Delivery, FrameContext, Mailbox, Sink},
    resident::read_reply,
};
fn setup() -> (Fx, Controller, PathBuf) {
    let mut fx = Fx::new();
    let c = fx.controller();
    let root = fx._temp.path().join("mailbox");
    c.install_sink(Box::new(Mailbox::open(root.clone()).unwrap()))
        .unwrap();
    let query = json!({"v":"openagents.livehost-media.v1","requires":[]});
    let event =
        workspace_host::host::request(&secret(OWNER), &pk(HOST), query.clone(), fx.now).unwrap();
    let reply = c.inspect(&event, fx.now).unwrap();
    let descriptor = read_reply(&reply, &pk(HOST), &secret(OWNER), &query).unwrap();
    fx.destination = descriptor["media"]["destination"].clone();
    fx.asr = descriptor["media"]["operation"].clone();
    let mut body = fx.plan_body(Value::Null, 0, 0, "pause");
    body["tracks"] = json!([
        {"id":"mic","source":{"scope":"device","id":"text-source"},"media":"text","direction":"capture",
            "recipients":[fx.destination],"retention":{"mode":"bounded","until":fx.now+3600},"processing":[fx.asr]},
        {"id":"speaker","source":{"scope":"device","id":"text-output"},"media":"text","direction":"playback",
            "recipients":[fx.destination],"retention":{"mode":"bounded","until":fx.now+3600},"processing":[]}]);
    assert_eq!(fx.send(&c, OWNER, &body)["status"], "accepted");
    fx.plan = reference(&body);
    let consent = fx.consent(ALICE, json!(["record"]), json!(["mic"]));
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            ALICE,
            json!(["publish"]),
            json!(["mic"]),
            vec![consent]
        )["status"],
        "accepted"
    );
    let consent = fx.consent(AGENT, json!(["record"]), json!(["speaker"]));
    assert_eq!(
        fx.participant(
            &c,
            OWNER,
            AGENT,
            json!(["playback"]),
            json!(["speaker"]),
            vec![consent]
        )["status"],
        "accepted"
    );
    assert_eq!(fx.control(&c, "start").0["status"], "accepted");
    (fx, c, root)
}
fn frame(fx: &Fx, who: u8, nonce: &str, sequence: u64, token: &str, direction: &str) -> Value {
    json!({"v":"openagents.livehost-frame.v1","requires":[],"session":fx.session,"plan":fx.plan,
        "track":if direction=="input" {"mic"} else {"speaker"},"epoch":0,"nonce":nonce,"sequence":sequence,
        "token":token,"bytes":format!("private text from {who}").as_bytes(),"direction":direction})
}
fn send(c: &Controller, fx: &Fx, who: u8, body: &Value) -> crate::Result<Value> {
    let event =
        workspace_host::host::request(&secret(who), &pk(HOST), body.clone(), fx.now).unwrap();
    let reply = c.frame(&event, fx.now)?;
    read_reply(&reply, &pk(HOST), &secret(who), body)
}
fn messages(root: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .collect()
}
#[test]
fn real_text_delivery_enforces_signer_floor_sequence_epoch_and_private_retention() {
    let (mut fx, c, root) = setup();
    let nonce = "8c".repeat(32);
    assert_eq!(fx.input_floor(&c, ALICE, &nonce)["status"], "accepted");
    let token = c.take_credential(&fx.session, &pk(ALICE)).unwrap();
    let body = frame(&fx, ALICE, &nonce, 0, &token, "input");
    assert!(send(&c, &fx, BOB, &body).is_err());
    let mut changed = body.clone();
    changed["session"] = json!("ab".repeat(32));
    assert!(send(&c, &fx, ALICE, &changed).is_err());
    changed = body.clone();
    changed["epoch"] = json!(1);
    assert!(send(&c, &fx, ALICE, &changed).is_err());
    changed = body.clone();
    changed["sequence"] = json!(2);
    assert!(
        send(&c, &fx, ALICE, &changed)
            .unwrap_err()
            .starts_with("conflict")
    );
    assert!(messages(&root).is_empty());
    let socket = fx.root.join("frames.sock");
    let mut server = crate::resident::Server::bind(&c, socket.clone()).unwrap();
    let event =
        workspace_host::host::request(&secret(ALICE), &pk(HOST), body.clone(), fx.now).unwrap();
    let response = super::resident::roundtrip(
        &mut server,
        &socket,
        crate::resident::Request::new(crate::resident::Operation::Frame { event }),
        fx.now,
    );
    let crate::resident::Outcome::Ok { reply } = response.outcome else {
        panic!("frame reply");
    };
    let result = read_reply(&reply, &pk(HOST), &secret(ALICE), &body).unwrap();
    drop(server);
    assert_eq!(result["status"], "accepted");
    assert_eq!(
        std::fs::read(messages(&root)[0].clone()).unwrap(),
        b"private text from 3"
    );
    assert_eq!(send(&c, &fx, ALICE, &body).unwrap(), result);
    assert_eq!(messages(&root).len(), 1);
    changed = body.clone();
    changed["bytes"] = json!(b"changed bytes");
    assert!(
        send(&c, &fx, ALICE, &changed)
            .unwrap_err()
            .starts_with("idempotency_conflict")
    );
    let out_nonce = "8d".repeat(32);
    assert_eq!(fx.speak_floor(&c, AGENT, &out_nonce)["status"], "accepted");
    let out_token = c.take_credential(&fx.session, &pk(AGENT)).unwrap();
    let out = frame(&fx, AGENT, &out_nonce, 0, &out_token, "playback");
    assert_eq!(send(&c, &fx, AGENT, &out).unwrap()["status"], "accepted");
    assert_eq!(messages(&root).len(), 2);
    for entry in std::fs::read_dir(fx.root.join("artifacts")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        for private in [
            token.as_bytes(),
            b"private text from 3",
            b"private text from 4",
        ] {
            assert!(!bytes.windows(private.len()).any(|w| w == private));
        }
    }
    let query = json!({"v":"openagents.livehost-frame-status.v1","requires":[],"session":fx.session,"frame":reference(&body)});
    let event =
        workspace_host::host::request(&secret(OWNER), &pk(HOST), query.clone(), fx.now).unwrap();
    let reply = c.inspect(&event, fx.now).unwrap();
    assert_eq!(
        read_reply(&reply, &pk(HOST), &secret(OWNER), &query).unwrap()["outcome"],
        result
    );
    let event = workspace_host::host::request(&secret(BOB), &pk(HOST), query, fx.now).unwrap();
    assert!(
        c.inspect(&event, fx.now)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    let revoke = json!({"v":"openagents.live-participant.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "participant":pk(ALICE),"action":"revoke","expected_revision":fx.revision(&c),"tracks":[],"rights":[],"consent":[],"expires_at":fx.now+600});
    assert_eq!(fx.send(&c, OWNER, &revoke)["status"], "accepted");
    let mut next = body.clone();
    next["sequence"] = json!(1);
    assert!(send(&c, &fx, ALICE, &next).is_err());
    assert_eq!(messages(&root).len(), 2);
    // A publish grant alone does not renew recording consent.
    assert_eq!(
        fx.participant(&c, OWNER, ALICE, json!(["publish"]), json!(["mic"]), vec![])["status"],
        "accepted"
    );
    assert_eq!(fx.input_floor(&c, ALICE, &nonce)["status"], "accepted");
    next["token"] = json!(c.take_credential(&fx.session, &pk(ALICE)).unwrap());
    assert!(send(&c, &fx, ALICE, &next).is_err());
    assert_eq!(messages(&root).len(), 2);
    let absent = json!({"v":"openagents.livehost-frame-status.v1","requires":[],"session":fx.session,"frame":{"digest":"sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","size":1,"media_type":"application/json"}});
    let event = workspace_host::host::request(&secret(BOB), &pk(HOST), absent, fx.now).unwrap();
    assert!(
        c.inspect(&event, fx.now)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    c.reconnect(&fx.session, fx.now).unwrap();
    assert!(send(&c, &fx, ALICE, &body).is_err());
    c.expire_media(fx.now + 3600).unwrap();
    assert!(messages(&root).is_empty());
}
#[test]
fn crash_after_mailbox_delivery_retains_unknown_without_duplicate_writes() {
    struct Crash(Mailbox);
    impl Sink for Crash {
        fn binding(&self) -> Vec<u8> {
            self.0.binding()
        }
        fn media(&self) -> &'static str {
            "text"
        }
        fn expire(&mut self, now: u64) -> crate::Result<()> {
            self.0.expire(now)
        }
        fn push(&mut self, context: &FrameContext, bytes: &[u8]) -> Delivery {
            assert!(matches!(self.0.push(context, bytes), Delivery::Delivered));
            panic!("injected crash after mailbox write")
        }
    }
    let (mut fx, c, root) = setup();
    let nonce = "8e".repeat(32);
    assert_eq!(fx.input_floor(&c, ALICE, &nonce)["status"], "accepted");
    let token = c.take_credential(&fx.session, &pk(ALICE)).unwrap();
    // Replacing the same native recipient preserves its exact binding.
    c.remove_sink().unwrap();
    c.install_sink(Box::new(Crash(Mailbox::open(root.clone()).unwrap())))
        .unwrap();
    let body = frame(&fx, ALICE, &nonce, 0, &token, "input");
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| send(&c, &fx, ALICE, &body)))
            .is_err()
    );
    assert_eq!(messages(&root).len(), 1);
    drop(c);
    let c = fx.controller();
    c.install_sink(Box::new(Mailbox::open(root.clone()).unwrap()))
        .unwrap();
    assert!(send(&c, &fx, ALICE, &body).is_err());
    let query = json!({"v":"openagents.livehost-frame-status.v1","requires":[],"session":fx.session,"frame":reference(&body)});
    let event =
        workspace_host::host::request(&secret(ALICE), &pk(HOST), query.clone(), fx.now).unwrap();
    let reply = c.inspect(&event, fx.now).unwrap();
    let status = read_reply(&reply, &pk(HOST), &secret(ALICE), &query).unwrap();
    assert_eq!(status["outcome"]["status"], "unknown");
    assert_eq!(messages(&root).len(), 1);
    c.expire_media(fx.now + 3600).unwrap();
    assert!(messages(&root).is_empty());
}

#[test]
fn native_mailbox_checks_expiry_at_the_actual_write_boundary() {
    struct Slow(Mailbox);
    impl Sink for Slow {
        fn binding(&self) -> Vec<u8> {
            self.0.binding()
        }
        fn media(&self) -> &'static str {
            "text"
        }
        fn expire(&mut self, now: u64) -> crate::Result<()> {
            self.0.expire(now)
        }
        fn push(&mut self, context: &FrameContext, bytes: &[u8]) -> Delivery {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            self.0.push(context, bytes)
        }
    }
    let (mut fx, c, root) = setup();
    c.remove_sink().unwrap();
    c.install_sink(Box::new(Slow(Mailbox::open(root.clone()).unwrap())))
        .unwrap();
    let nonce = "8f".repeat(32);
    let request = json!({"v":"openagents.live-input-floor-request.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "participant":pk(ALICE),"track":"mic","destination":fx.destination,"processing":fx.asr,
        "expected_revision":fx.revision(&c),"action":"acquire","until":fx.now+1,"nonce":nonce});
    assert_eq!(fx.send(&c, ALICE, &request)["status"], "accepted");
    let token = c.take_credential(&fx.session, &pk(ALICE)).unwrap();
    let body = frame(&fx, ALICE, &nonce, 0, &token, "input");
    let result = send(&c, &fx, ALICE, &body).unwrap();
    assert_eq!(result["status"], "refused");
    assert_eq!(result["dispatched"], false);
    assert!(messages(&root).is_empty());
}

#[test]
fn withdrawing_a_recipient_clears_its_copies_and_refuses_more_frames() {
    let (mut fx, c, root) = setup();
    let nonce = "9a".repeat(32);
    assert_eq!(fx.input_floor(&c, ALICE, &nonce)["status"], "accepted");
    let token = c.take_credential(&fx.session, &pk(ALICE)).unwrap();
    let body = frame(&fx, ALICE, &nonce, 0, &token, "input");
    assert_eq!(send(&c, &fx, ALICE, &body).unwrap()["status"], "accepted");
    assert_eq!(messages(&root).len(), 1);
    c.remove_sink().unwrap();
    assert!(messages(&root).is_empty());
    let mut next = body;
    next["sequence"] = json!(1);
    assert!(
        send(&c, &fx, ALICE, &next)
            .unwrap_err()
            .starts_with("unsupported_feature")
    );
    assert!(Mailbox::open(root).is_ok());
}
