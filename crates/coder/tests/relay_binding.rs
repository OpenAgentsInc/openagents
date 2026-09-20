//! NIP-CJ job binding, against a relay that lies.
//!
//! `relay_job.rs` proves the wire contract against a real relay. This
//! file proves the client against a dishonest one: a loopback relay that
//! takes the job request and then fans out signed worker events under
//! subscription labels of its choosing — an old job's result relabeled as
//! this job's, an answer meant for another customer, a partial delivered
//! twice, a payload claiming a version this NIP does not define.
//!
//! The relay controls the label; the signature covers the rest. Every
//! test runs entirely on loopback with keys generated in the test, so
//! none of them need `CODER_RELAY` or skip.

use std::time::Duration;

use coder::generate::{Generate, GenerateError, Message, Meta, Role, Usage};
use coder::relay::{FEEDBACK_KIND, Identity, REQUEST_KIND, RESULT_KIND, RelayDoor};
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite};

const CONTACT: Duration = Duration::from_secs(2);
const ANSWER: Duration = Duration::from_secs(5);

/// The worker key every test uses; each test gives the door its pubkey.
const WORKER: u8 = 0xaa;
/// The customer key every test uses.
const CLIENT: u8 = 0x0b;
/// A second key, for events that are not the worker's.
const IMPOSTOR: u8 = 0xcc;

type Server = WebSocketStream<TcpStream>;

/// The frames a test's relay sends after it takes the job request, built
/// from the request it saw and the label the door subscribed under.
/// Complete `["EVENT", label, event]` frames, label included, because
/// which label an event arrives under is part of what these tests vary.
type Script = Box<dyn FnOnce(&Event, &str) -> Vec<Value> + Send>;

fn identity(byte: u8) -> Identity {
    Identity::from_secret(SecretKey::from_byte_array([byte; 32]).unwrap()).unwrap()
}

fn xonly(pubkey: &str) -> XOnlyPublicKey {
    let mut bytes = [0u8; 32];
    for (index, pair) in pubkey.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).unwrap();
        let low = (pair[1] as char).to_digit(16).unwrap();
        bytes[index] = ((high << 4) | low) as u8;
    }
    XOnlyPublicKey::from_byte_array(bytes).unwrap()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn send(socket: &mut Server, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

/// The next JSON frame, or `None` when the client hangs up.
async fn read(socket: &mut Server) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Some(value);
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

/// The lying relay. It challenges, takes the subscription and the job
/// request, acknowledges the publish, then sends whatever the test's
/// script built — which is usually not what the door subscribed for.
/// Afterwards it holds the socket open until the door hangs up, so a turn
/// that rejects every poisoned event ends on its own wait rather than on
/// the socket closing.
async fn relay(listener: TcpListener, script: Script) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(tcp).await.unwrap();
    send(&mut socket, json!(["AUTH", "binding-test-challenge"])).await;

    let mut subscription = String::new();
    let request = loop {
        let Some(frame) = read(&mut socket).await else {
            return;
        };
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let id = frame[1]["id"].as_str().unwrap_or_default().to_string();
                send(&mut socket, json!(["OK", id, true, ""])).await;
            }
            "REQ" => {
                subscription = frame[1].as_str().unwrap_or_default().to_string();
            }
            "EVENT" => {
                let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
                send(&mut socket, json!(["OK", event.id, true, ""])).await;
                break event;
            }
            _ => {}
        }
    };

    for frame in script(&request, &subscription) {
        // The door may have refused the turn mid-script — a flood test
        // ends on the cap, not on the last frame — so a dead socket just
        // ends the send loop.
        if socket
            .send(tungstenite::Message::Text(frame.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
    }
    while read(&mut socket).await.is_some() {}
}

/// Start a scripted relay on loopback and return its `ws://` URL.
async fn spawn_relay(script: Script) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(relay(listener, script));
    url
}

/// One worker frame as the relay fans it out: `signer` signs it, `e_tag`
/// and `p_tag` are what the signature covers, and `label` is only the
/// subscription the relay claims it arrived under. The payload encrypts
/// to the request's author regardless of who `p_tag` names, so a door
/// that accepted a poisoned frame would read the text.
fn worker_frame(
    signer: &Identity,
    label: &str,
    request: &Event,
    kind: u16,
    e_tag: &str,
    p_tag: &str,
    payload: &Value,
) -> Value {
    signed_frame(
        signer,
        label,
        request,
        kind,
        vec![
            Tag::new(vec!["e".into(), e_tag.into()]),
            Tag::new(vec!["p".into(), p_tag.into()]),
        ],
        payload,
    )
}

/// The same with the tag list spelled out, for events that are missing
/// the tags the binding is checked against.
fn signed_frame(
    signer: &Identity,
    label: &str,
    request: &Event,
    kind: u16,
    tags: Vec<Tag>,
    payload: &Value,
) -> Value {
    let customer = xonly(&request.pubkey);
    let conversation = nip44::conversation_key(signer.secret(), &customer);
    let ciphertext = nip44::encrypt(
        &payload.to_string(),
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    let event = signer.signer().sign(unix_now(), kind, tags, ciphertext);
    json!(["EVENT", label, event])
}

/// The same, bound the honest way: this request's `e` tag and author's
/// `p` tag.
fn bound_frame(
    signer: &Identity,
    label: &str,
    request: &Event,
    kind: u16,
    payload: &Value,
) -> Value {
    worker_frame(
        signer,
        label,
        request,
        kind,
        &request.id,
        &request.pubkey,
        payload,
    )
}

/// A correct answer: a judgment, two sequenced partials, and a result.
fn honest_answer(worker: &Identity, label: &str, request: &Event) -> Vec<Value> {
    vec![
        bound_frame(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"judgment","verdict":"respond","line":"respond 1.00"}),
        ),
        bound_frame(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"partial","seq":0,"delta":"hello"}),
        ),
        bound_frame(
            worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"partial","seq":1,"delta":" there"}),
        ),
        bound_frame(
            worker,
            label,
            request,
            RESULT_KIND,
            &json!({
                "v": 2,
                "type": "result",
                "text": "hello there",
                "usage": {"input": 10, "output": 2},
                "model": "test/worker-model",
            }),
        ),
    ]
}

/// A job request the client signed an hour ago. A result bound to it is
/// valid in every way — right worker, right signature, right recipient,
/// right encryption — except that it is not this turn's.
fn stale_request() -> Event {
    identity(CLIENT).signer().sign(
        unix_now() - 3_600,
        REQUEST_KIND,
        vec![Tag::new(vec!["p".into(), identity(WORKER).pubkey().into()])],
        "an old job's ciphertext".to_string(),
    )
}

/// What one turn produced: the door's outcome, everything it streamed,
/// and every piece of sideband it emitted.
struct Outcome {
    result: Result<(String, Option<Usage>), GenerateError>,
    streamed: String,
    metas: Vec<Meta>,
}

/// Run one turn against the scripted relay at `url`.
async fn run_turn(url: String) -> Outcome {
    let door = RelayDoor::new(url, xonly(identity(WORKER).pubkey()), identity(CLIENT))
        .waiting(CONTACT, ANSWER);
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let mut streamed = String::new();
    let mut metas = Vec::new();
    let result = door
        .generate(
            "be terse",
            &input,
            &mut |delta| streamed.push_str(delta),
            &mut |meta| metas.push(meta),
        )
        .await;
    Outcome {
        result,
        streamed,
        metas,
    }
}

/// The happy path over loopback: feedback and a result that are bound
/// correctly behave exactly as they did before the checks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bound_answer_completes_the_turn() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        honest_answer(&worker, label, request)
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, usage) = outcome.result.expect("the worker answered");
    assert_eq!(text, "hello there");
    assert_eq!(outcome.streamed, "hello there");
    assert_eq!(usage.unwrap().input_tokens, 10);
    assert!(
        outcome
            .metas
            .contains(&Meta::Judgment("respond 1.00".into()))
    );
    assert!(
        outcome
            .metas
            .contains(&Meta::Model("test/worker-model".into()))
    );
}

/// The audit's case. A result the worker genuinely signed — for a job
/// this same client published an hour ago — arrives under this job's
/// subscription label. Signature verification passes and the payload
/// decrypts; only the `e` tag says it is not this turn's answer. The door
/// must wait out its contact deadline rather than deliver stale text.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_relabeled_old_result_is_not_the_answer() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let old = stale_request();
        vec![worker_frame(
            &worker,
            label,
            request,
            RESULT_KIND,
            &old.id,
            &old.pubkey,
            &json!({"v":2,"type":"result","text":"STALE ANSWER"}),
        )]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome.result.expect_err("a replay is not an answer");
    assert_eq!(error.cause(), "worker_absent");
    assert_eq!(outcome.streamed, "");
}

/// The same poisoned event ahead of the real answer: the turn skips it
/// and finishes with the text that is actually bound to this request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_for_another_job_is_skipped_and_the_real_one_lands() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let old = stale_request();
        let mut frames = vec![
            worker_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &old.id,
                &old.pubkey,
                &json!({"v":2,"type":"partial","seq":0,"delta":"STALE"}),
            ),
            worker_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &old.id,
                &old.pubkey,
                &json!({"v":2,"type":"result","text":"STALE ANSWER"}),
            ),
        ];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
    assert_eq!(outcome.streamed, "hello there");
}

/// A result correctly `e`-tagged to this request but `p`-tagged to a
/// different customer is not this turn's either — the relay is the one
/// that decided to deliver it here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_addressed_to_someone_else_is_not_the_answer() {
    let worker = identity(WORKER);
    let stranger = identity(0xdd).pubkey().to_string();
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![worker_frame(
            &worker,
            label,
            request,
            RESULT_KIND,
            &request.id,
            &stranger,
            &json!({"v":2,"type":"result","text":"SOMEONE ELSES ANSWER"}),
        )];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// One partial delivered twice is one delta, not two. The relay can
/// resend any event it holds; the event id is what makes a resend
/// recognizable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_duplicate_partial_does_not_display_twice() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let first = bound_frame(
            &worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"partial","seq":0,"delta":"he"}),
        );
        vec![
            first.clone(),
            first,
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":1,"delta":"llo"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"type":"result","text":"hello"}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the result lands");
    assert_eq!(text, "hello");
    assert_eq!(outcome.streamed, "hello");
}

/// A correctly bound event signed by a key that is not the worker's says
/// nothing about this job, even though it decrypts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_signed_by_someone_else_is_not_the_workers() {
    let worker = identity(WORKER);
    let impostor = identity(IMPOSTOR);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![bound_frame(
            &impostor,
            label,
            request,
            RESULT_KIND,
            &json!({"v":2,"type":"result","text":"FORGED ANSWER"}),
        )];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// A signed event from the worker in a kind this protocol does not
/// answer with is not feedback, whatever it is `e`-tagged to.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_event_of_an_unexpected_kind_is_not_an_answer() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![bound_frame(
            &worker,
            label,
            request,
            REQUEST_KIND,
            &json!({"v":2,"type":"result","text":"WRONG KIND ANSWER"}),
        )];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// A correctly bound payload that claims a version this NIP does not
/// define is not read: a future `type` cannot smuggle text through a
/// field this version would not have.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_payload_that_claims_another_version_is_not_read() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":99,"type":"partial","seq":0,"delta":"POISON"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":99,"type":"result","text":"POISON ANSWER"}),
            ),
        ];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the v1 answer lands");
    assert_eq!(text, "hello there");
    assert_eq!(outcome.streamed, "hello there");
}

/// The check runs the other way too: an answer that is bound correctly
/// is this turn's under any label, because the label was never the
/// identity. A relay that delivers the real result under a label the
/// door never opened is still delivering the real result.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bound_answer_lands_under_a_label_the_door_never_opened() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, _label| {
        honest_answer(&worker, "the-relay-made-this-up", request)
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the bound answer lands");
    assert_eq!(text, "hello there");
    assert_eq!(outcome.streamed, "hello there");
}

/// A typed refusal is still a refusal: correctly bound `status: error`
/// feedback declines with its code, as it did before the checks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bound_refusal_still_refuses_with_its_code() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![bound_frame(
            &worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({
                "v": 2,
                "type": "status",
                "status": "error",
                "code": "quota_exhausted",
                "message": "free allowance used",
            }),
        )]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome.result.expect_err("the worker declined");
    assert_eq!(error.cause(), "worker_declined");
    assert_eq!(error.refusal(), Some("quota_exhausted"));
}

/// A worker-signed event with no `e` tag names no job at all, however
/// the relay labels it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_event_that_names_no_job_is_not_the_answer() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![signed_frame(
            &worker,
            label,
            request,
            RESULT_KIND,
            vec![Tag::new(vec!["p".into(), request.pubkey.clone()])],
            &json!({"v":2,"type":"result","text":"UNTAGGED ANSWER"}),
        )];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// A worker-signed event with no `p` tag names no recipient; the relay
/// delivering it here does not make it this terminal's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_event_that_names_no_recipient_is_not_the_answer() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![signed_frame(
            &worker,
            label,
            request,
            RESULT_KIND,
            vec![Tag::new(vec!["e".into(), request.id.clone()])],
            &json!({"v":2,"type":"result","text":"UNADDRESSED ANSWER"}),
        )];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// The deduplication set holds only events that passed the checks. A
/// forged event that copies a genuine result's id ahead of it — a relay
/// can arrange that — must not suppress the genuine one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forgery_cannot_claim_a_real_events_id() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let genuine = bound_frame(
            &worker,
            label,
            request,
            RESULT_KIND,
            &json!({"v":2,"type":"result","text":"hello there"}),
        );
        // Same event, same id, signature corrupted after the fact.
        let mut forged: Event = serde_json::from_value(genuine[2].clone()).unwrap();
        forged.sig = "11".repeat(64);
        vec![json!(["EVENT", label, forged]), genuine]
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome
        .result
        .expect("the forgery does not suppress the real result");
    assert_eq!(text, "hello there");
}

/// Version 1 is the contract and the field is required: a bound payload
/// that names no version is not read either.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_payload_that_names_no_version_is_not_read() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"type":"partial","delta":"POISON"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"type":"result","text":"POISON ANSWER"}),
            ),
        ];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the v1 answer lands");
    assert_eq!(text, "hello there");
    assert_eq!(outcome.streamed, "hello there");
}

/// A `26900` event whose payload says `status: error` is not a refusal:
/// the kind and the type have to agree before either is read. And a
/// result-kind payload with no `type` at all is not a result.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kind_and_type_that_do_not_pair_are_not_read() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let mut frames = vec![
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({
                    "v": 2,
                    "type": "status",
                    "status": "error",
                    "code": "internal",
                    "message": "a result kind carrying a refusal",
                }),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"text":"TYPELESS ANSWER"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"result","text":"MISFILED ANSWER"}),
            ),
        ];
        frames.extend(honest_answer(&worker, label, request));
        frames
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the real answer lands");
    assert_eq!(text, "hello there");
}

/// A bound event that decrypts to something the protocol never named —
/// a known version with an unknown type — is not contact either. Nothing
/// follows it, so the turn reports the worker absent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_payload_with_an_unknown_type_is_not_contact() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![bound_frame(
            &worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"mystery","delta":"POISON"}),
        )]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome.result.expect_err("an unknown type is not contact");
    assert_eq!(error.cause(), "worker_absent");
    assert_eq!(outcome.streamed, "");
}

/// A version-2 partial with no `seq` is malformed, and malformed data
/// does not establish that a worker is there.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_partial_without_a_sequence_is_not_contact() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![bound_frame(
            &worker,
            label,
            request,
            FEEDBACK_KIND,
            &json!({"v":2,"type":"partial","delta":"POISON"}),
        )]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome
        .result
        .expect_err("a sequenceless partial is malformed");
    assert_eq!(error.cause(), "worker_absent");
    assert_eq!(outcome.streamed, "");
}

/// Deltas stream in the signed order, not the arrival order. A relay
/// that delivers `seq` 0, then `seq` 2 — skipping `seq` 1 — then `seq` 1
/// late gets exactly `seq` 0 displayed: the gap closes the stream and
/// nothing after it is text. The result still completes the job.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partials_stream_only_in_the_signed_order() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let partial = |seq: u64, delta: &str| {
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":seq,"delta":delta}),
            )
        };
        vec![
            partial(0, "first"),
            partial(2, "GAP"),
            partial(1, "LATE"),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"type":"result","text":"the whole answer"}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the result lands");
    assert_eq!(text, "the whole answer");
    assert_eq!(outcome.streamed, "first");
}

/// Two different events claiming the same `seq` are a replay the event
/// id cannot catch; the sequence does. The second ends the stream.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_repeated_sequence_ends_the_stream() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let partial = |seq: u64, delta: &str| {
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":seq,"delta":delta}),
            )
        };
        vec![
            partial(0, "he"),
            partial(0, "XX"),
            partial(1, "llo"),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"type":"result","text":"the whole answer"}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the result lands");
    assert_eq!(text, "the whole answer");
    assert_eq!(outcome.streamed, "he");
}

/// The legacy contract: a version-1 partial is a liveness signal, never
/// text, because nothing signed its order. A version-1 result still
/// completes the job with the full answer it carries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_v1_partial_is_heard_but_never_streamed() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":1,"type":"partial","delta":"unordered"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":1,"type":"partial","delta":" deltas"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":1,"type":"result","text":"legacy answer"}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let (text, _) = outcome.result.expect("the v1 result completes the job");
    assert_eq!(text, "legacy answer");
    // The deltas proved the worker was there — the turn did not fail as
    // absent — but none of their text was displayed.
    assert_eq!(outcome.streamed, "");
}

/// The dedup set is bounded: a relay fanning out an unending stream of
/// validly bound events is a flood, and a flood is a transport error, not
/// a wait. The door caps one job's events well past any honest answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_event_flood_is_refused() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        // Past the door's 1024-event bound, each distinct because the
        // encryption nonce is.
        (0..1_100)
            .map(|_| {
                bound_frame(
                    &worker,
                    label,
                    request,
                    FEEDBACK_KIND,
                    &json!({"v":2,"type":"judgment","verdict":"respond","line":"respond"}),
                )
            })
            .collect()
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome.result.expect_err("a flood is refused");
    assert_eq!(error.cause(), "stream");
    assert!(error.to_string().contains("events"), "{error}");
}

/// Streamed bytes are bounded the same way: deltas past the cap refuse
/// the turn rather than grow memory for the whole wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_that_outgrows_its_cap_is_refused() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        let delta = "x".repeat(1_024);
        (0..300u64)
            .map(|seq| {
                bound_frame(
                    &worker,
                    label,
                    request,
                    FEEDBACK_KIND,
                    &json!({"v":2,"type":"partial","seq":seq,"delta":delta.as_str()}),
                )
            })
            .collect()
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome
        .result
        .expect_err("a stream past the cap is refused");
    assert_eq!(error.cause(), "stream");
    assert!(error.to_string().contains("deltas"), "{error}");
}

/// Well-formedness gates contact, not just text: a judgment with no
/// `line`, a status with a value the NIP never named, and a version-1
/// partial with no `delta` are all bound, signed, and decryptable — and
/// none of them proves a worker is there.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_payloads_are_not_contact() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"judgment","verdict":"respond"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"status","status":"limbo"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":1,"type":"partial"}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome
        .result
        .expect_err("malformed payloads are not contact");
    assert_eq!(error.cause(), "worker_absent");
    assert_eq!(outcome.streamed, "");
}

/// The result is the answer, whole: an empty result is an empty answer,
/// and the deltas that streamed before it are not a substitute. A
/// truncated prefix must not complete a job.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_empty_result_is_not_completed_by_the_stream() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":0,"delta":"he"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"type":"result","text":""}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome
        .result
        .expect_err("an empty result is not an answer");
    assert_eq!(error.cause(), "stream");
    assert_eq!(outcome.streamed, "he");
}

/// The same after a sequence violation: the stream closed at `seq` 2
/// without `seq` 1, and an empty result cannot be completed by the
/// prefix that survived.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gap_then_an_empty_result_is_not_an_answer() {
    let worker = identity(WORKER);
    let url = spawn_relay(Box::new(move |request, label| {
        vec![
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":0,"delta":"he"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                FEEDBACK_KIND,
                &json!({"v":2,"type":"partial","seq":2,"delta":"GAP"}),
            ),
            bound_frame(
                &worker,
                label,
                request,
                RESULT_KIND,
                &json!({"v":2,"type":"result","text":""}),
            ),
        ]
    }))
    .await;

    let outcome = run_turn(url).await;
    let error = outcome
        .result
        .expect_err("a closed stream cannot complete an empty result");
    assert_eq!(error.cause(), "stream");
    assert_eq!(outcome.streamed, "he");
}
