//! Real CJ/ENV admission around the SDK port and a local Rust app-server peer.
use super::*;
use crate::codex::{CodexEngine, Connector, Installation};
use crate::engine::IoBudget;
use crate::transport::Reply;
use coder_sdk::codex::managed::Managed;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Steer,
    Interrupt,
    Required,
    Unsupported,
    LostStart,
    ChangedConfiguration,
    WrongSteer,
}
#[derive(Clone)]
struct LocalPeer {
    mode: Mode,
    requests: Arc<Mutex<Vec<Value>>>,
}
impl LocalPeer {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
    fn engine(&self) -> CodexEngine {
        CodexEngine::new(Box::new(self.clone())).unwrap()
    }
    fn count(&self, method: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v["method"] == method)
            .count()
    }
}
impl Connector for LocalPeer {
    fn installation(&self) -> Installation {
        Installation {
            engine: b"isolated Rust Codex protocol peer v1".to_vec(),
            protocol: br#"{"type":"object"}"#.to_vec(),
            model: "loop".into(),
            provider: "local-free".into(),
            cwd: "/isolated".into(),
            effects: json!({"reads":["artifact"],"writes":[],"network":[],
                "process":false,"delegates":false,"spend":false}),
        }
    }
    fn connect(
        &mut self,
        _configuration: &crate::engine::Configuration,
        budget: IoBudget,
    ) -> crate::Result<Managed> {
        assert!(budget.deadline() > std::time::Instant::now());
        let (client, peer) = tokio::io::duplex(1_048_576);
        let (reader, writer) = tokio::io::split(client);
        let requests = self.requests.clone();
        let mode = self.mode;
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let (reader,mut writer)=tokio::io::split(peer);
                let mut reader=BufReader::new(reader);
                loop {
                    let mut line=String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0)==0 { break; }
                    let message:Value=serde_json::from_str(&line).unwrap();
                    requests.lock().unwrap().push(message.clone());
                    let id=&message["id"];
                    let method=message["method"].as_str().unwrap_or("");
                    let mut messages=Vec::new();
                    match method {
                        "initialize" => messages.push(json!({"id":id,"result":{"userAgent":"isolated-native-peer"}})),
                        "initialized" => continue,
                        "thread/start" => {
                            assert_eq!(message["params"]["model"],"loop");
                            assert_eq!(message["params"]["ephemeral"],true);
                            assert_eq!(message["params"]["sandbox"],"readOnly");
                            messages.push(json!({"id":id,"result":{"thread":{"id":"native-thread"},
                                "model":if mode==Mode::ChangedConfiguration {"different"} else {"loop"},
                                "modelProvider":"local-free","cwd":"/isolated","reasoningEffort":"low",
                                "approvalPolicy":"unlessTrusted","sandbox":{"type":"readOnly"}}}));
                        }
                        "turn/start" => {
                            assert_eq!(message["params"]["threadId"],"native-thread");
                            let input:Value=serde_json::from_str(message["params"]["input"][0]["text"].as_str().unwrap()).unwrap();
                            assert!(!input["task_frame"].as_str().unwrap().is_empty());
                            if mode==Mode::LostStart {
                                writer.write_all(b"{\"id\":").await.unwrap(); break;
                            }
                            messages.push(json!({"id":id,"result":{"turn":{"id":"native-turn","status":"inProgress"}}}));
                            if matches!(mode,Mode::Required|Mode::Unsupported) {
                                messages.push(json!({"id":"permission-source","method":if mode==Mode::Required {"item/commandExecution/requestApproval"} else {"future/required"},
                                    "params":{"threadId":"native-thread","turnId":"native-turn","itemId":"command-1",
                                    "command":"must never run"}}));
                            }
                        }
                        "turn/steer" => {
                            assert_eq!(message["params"]["expectedTurnId"],"native-turn");
                            messages.push(json!({"id":id,"result":{"turnId":if mode==Mode::WrongSteer {"another-turn"} else {"native-turn"}}}));
                            if mode!=Mode::WrongSteer {
                                messages.push(json!({"method":"item/agentMessage/delta","params":{
                                    "threadId":"native-thread","turnId":"native-turn","delta":"corrected result"}}));
                                messages.push(terminal("completed"));
                            }
                        }
                        "turn/interrupt" => {
                            assert_eq!(message["params"]["turnId"],"native-turn");
                            messages.push(json!({"id":id,"result":{}}));
                            messages.push(terminal("interrupted"));
                        }
                        "" => { if mode==Mode::Unsupported { assert_eq!(message["error"]["code"],-32601); } else { assert_eq!(message["result"]["decision"],"decline"); } continue; }
                        _ => panic!("unexpected native operation {method}"),
                    }
                    for message in messages {
                        let mut bytes=serde_json::to_vec(&message).unwrap(); bytes.push(b'\n');
                        if writer.write_all(&bytes).await.is_err() { return; }
                    }
                }
            });
        });
        Ok(Managed::over(BufReader::new(reader), writer))
    }
}
fn terminal(status: &str) -> Value {
    json!({"method":"turn/completed","params":{"threadId":"native-thread",
        "turn":{"id":"native-turn","status":status}}})
}
fn running(mode: Mode) -> (Real, LocalPeer, SessionHost, Value, nostr::domain::Event) {
    let peer = LocalPeer::new(mode);
    let mut r = Real::for_engine(peer.engine());
    let host = r.host(peer.engine());
    let opening = r.opened(&host);
    let command = r.submit_command(&host, &opening, 1);
    let (event, binding) = r.job(&command);
    let reply = host
        .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
        .unwrap();
    assert_eq!(reply.0.kind, execution::FEEDBACK_KIND);
    (r, peer, host, opening, event)
}
fn done(r: &Real, host: &SessionHost, event: &nostr::domain::Event) -> Reply {
    for _ in 0..12 {
        let reply = host.poll(event, r.fx.now).unwrap();
        if reply.0.kind == execution::RESULT_KIND {
            return reply;
        }
    }
    panic!("native terminal was not retained")
}

#[test]
fn codex_sdk_steering_and_output_use_one_original_charged_turn() {
    let (mut r, peer, host, opening, event) = running(Mode::Steer);
    let session = opening["session"].as_str().unwrap();
    let body = host.store().json(&r.adapter).unwrap();
    for feature in [
        "approvals",
        "questions",
        "resume",
        "configure",
        "fork",
        "close",
    ] {
        assert_eq!(
            body["features"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["feature"] == feature)
                .unwrap()["support"],
            "unsupported"
        );
    }
    let correction = correction(&mut r.fx, "native", "correct it");
    let steer = r.fx.command(
        &host,
        session,
        "steer",
        json!({"turn":host.status(session).unwrap()["active_turn"],
        "expected_turn_revision":host.status(session).unwrap()["turn_revision"],"correction":correction}),
    );
    let reply = host
        .continue_turn(&event, &r.fx.bundle(OWNER, &steer), r.fx.now)
        .unwrap();
    assert_eq!(reply["status"], "accepted", "{reply}");
    assert_eq!(
        host.continue_turn(&event, &r.fx.bundle(OWNER, &steer), r.fx.now)
            .unwrap()["status"],
        "duplicate"
    );
    let result = done(&r, &host, &event);
    assert_eq!(
        r.coordinator
            .record_result(&result.0, &result.1, r.fx.now)
            .unwrap()["outcome"],
        "completed"
    );
    assert_eq!(peer.count("turn/start"), 1);
    assert_eq!(peer.count("turn/steer"), 1);
    assert!(
        result
            .1
            .values()
            .filter_map(|b| serde_json::from_slice::<Value>(b).ok())
            .any(|body| body["text"] == "corrected result")
    );
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
    let history = host.export(session, r.fx.now).unwrap();
    let history = host.store().json(&history).unwrap();
    assert_eq!(history["coverage"], "complete");
    assert!(history.to_string().contains("native"));
}

#[test]
fn codex_sdk_interrupt_ack_is_distinct_from_terminal_evidence() {
    let (mut r, peer, host, opening, event) = running(Mode::Interrupt);
    let session = opening["session"].as_str().unwrap();
    let interrupt = r.fx.command(
        &host,
        session,
        "interrupt",
        json!({"turn":host.status(session).unwrap()["active_turn"],
        "expected_turn_revision":host.status(session).unwrap()["turn_revision"],"reason":"test stop"}),
    );
    let reply = host
        .continue_turn(&event, &r.fx.bundle(OWNER, &interrupt), r.fx.now)
        .unwrap();
    assert_eq!(reply["status"], "accepted", "{reply}");
    assert_eq!(peer.count("turn/interrupt"), 1);
    let result = done(&r, &host, &event);
    assert_eq!(
        r.coordinator
            .record_result(&result.0, &result.1, r.fx.now)
            .unwrap()["outcome"],
        "cancelled"
    );
    assert_eq!(peer.count("turn/start"), 1);
}

#[test]
fn codex_sdk_unsupported_requests_are_denied_only_during_cleanup() {
    for mode in [Mode::Required, Mode::Unsupported] {
        let (r, peer, host, _opening, event) = running(mode);
        host.poll(&event, r.fx.now).unwrap();
        for _ in 0..3 {
            host.advance(r.fx.now).unwrap();
        }
        let requests = peer.requests.lock().unwrap();
        let replies: Vec<_> = requests
            .iter()
            .filter(|v| v["id"] == "permission-source")
            .collect();
        assert_eq!(replies.len(), 1);
        if mode == Mode::Required {
            assert_eq!(replies[0]["result"]["decision"], "decline");
        } else {
            assert_eq!(replies[0]["error"]["code"], -32601);
        }
        assert_eq!(
            requests
                .iter()
                .filter(|v| v["method"] == "turn/start")
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|v| v["method"] == "turn/interrupt")
                .count(),
            1
        );
        assert!(!requests.iter().any(|v| v["result"]["decision"] == "accept"));
    }
}

#[test]
fn codex_sdk_lost_start_and_configuration_mismatch_never_repeat() {
    for mode in [Mode::LostStart, Mode::ChangedConfiguration] {
        let peer = LocalPeer::new(mode);
        let mut r = Real::for_engine(peer.engine());
        let host = r.host(peer.engine());
        let opening = r.opening(&host);
        let (open, binding) = r.job(&opening);
        let first = host
            .serve(&open, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap();
        let (event, binding) = if mode == Mode::LostStart {
            r.coordinator
                .record_result(&first.0, &first.1, r.fx.now)
                .unwrap();
            r.release_attachment(&binding.grant);
            let command = r.submit_command(&host, &opening, 1);
            r.job(&command)
        } else {
            (open, binding)
        };
        let first = host
            .serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
                    .unwrap(),
                first
            );
        }
        assert_eq!(peer.count("thread/start"), 1);
        assert_eq!(
            peer.count("turn/start"),
            usize::from(mode == Mode::LostStart)
        );
        drop(host);
        let host = r.host(peer.engine());
        host.serve(&event, &r.fx.artifacts, &binding, r.fx.now)
            .unwrap();
        assert_eq!(peer.count("thread/start"), 1);
        assert_eq!(
            peer.count("turn/start"),
            usize::from(mode == Mode::LostStart)
        );
    }
}

#[test]
fn generative_context_requires_owner_access_before_intent_consumption() {
    for queued in [false, true] {
        let peer = LocalPeer::new(Mode::Interrupt);
        let mut fx = Fx::new();
        let host = SessionHost::open(
            fx.root.clone(),
            secret(HOST),
            vec![Box::new(peer.engine())],
            Box::new(fx.bindings.clone()),
        )
        .unwrap();
        let (session, _) = fx.open(&host, "codex-managed");
        let intent = fx.intent(
            &host,
            &session,
            json!({"prompt":"private controller input"}),
            fx.now + 600,
        );
        let command = fx.command(
            &host,
            &session,
            if queued { "enqueue" } else { "submit" },
            json!({"intent":intent}),
        );
        let binding = fx.binding();
        let result = fx.send(&host, CONTROLLER, &command, Some(&binding));
        if queued {
            assert_eq!(result["status"], "accepted");
            // Promotion reads the original controller's bytes, but must not
            // disclose them through owner-visible native output.
            host.promote(&session, Some(&binding), fx.now).unwrap_err();
            let body: Value =
                serde_json::from_slice(&fx.artifacts[intent["digest"].as_str().unwrap()]).unwrap();
            let cancel = fx.command(
                &host,
                &session,
                "cancel_queued",
                json!({"intent":body["intent"]}),
            );
            assert_eq!(fx.send(&host, OWNER, &cancel, None)["status"], "accepted");
        } else {
            assert_eq!(result["reason"], "not_admitted", "{result}");
        }
        assert_eq!(peer.count("turn/start"), 0);
        // The owner explicitly supplies the bytes; the refusal consumed no
        // execution intent and cannot silently authorize a queued delivery.
        let submit = fx.command(&host, &session, "submit", json!({"intent":intent}));
        let result = fx.send(&host, OWNER, &submit, Some(&binding));
        assert_eq!(result["status"], "accepted", "{result}");
        assert_eq!(peer.count("turn/start"), 1);
    }
}

#[test]
fn codex_sdk_changed_steer_acknowledgment_is_unknown_and_cannot_repeat() {
    let (mut r, peer, host, opening, event) = running(Mode::WrongSteer);
    let session = opening["session"].as_str().unwrap();
    let correction = correction(&mut r.fx, "native", "correct it");
    let command=r.fx.command(&host,session,"steer",json!({"turn":host.status(session).unwrap()["active_turn"],
        "expected_turn_revision":host.status(session).unwrap()["turn_revision"],"correction":correction}));
    let bundle = r.fx.bundle(OWNER, &command);
    let first = host.continue_turn(&event, &bundle, r.fx.now).unwrap();
    assert_eq!(first["status"], "unknown");
    assert_eq!(
        host.continue_turn(&event, &bundle, r.fx.now).unwrap(),
        first
    );
    assert_eq!(peer.count("turn/steer"), 1);
    assert_eq!(peer.count("turn/start"), 1);
}

#[test]
fn codex_sdk_expiry_stops_the_original_native_turn_without_new_input() {
    let (r, peer, host, _opening, event) = running(Mode::Interrupt);
    for clock in host.runtime_clocks.borrow_mut().values_mut() {
        clock.0 = std::time::Instant::now() - std::time::Duration::from_secs(301);
    }
    host.advance(r.fx.now).unwrap();
    let result = done(&r, &host, &event);
    assert_eq!(
        r.coordinator
            .record_result(&result.0, &result.1, r.fx.now)
            .unwrap()["outcome"],
        "cancelled"
    );
    assert_eq!(peer.count("turn/interrupt"), 1);
    assert_eq!(peer.count("turn/start"), 1);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 2);
}
