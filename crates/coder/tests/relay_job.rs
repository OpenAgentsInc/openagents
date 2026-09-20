//! Live NIP-CJ contract: a mock worker answers a `RelayDoor` turn over a
//! real relay. Set `CODER_RELAY` (for example `ws://127.0.0.1:8080`),
//! `CODER_WORKER_SECRET`, and `CODER_SECRET_KEY` to run; unset skips.

use std::time::{Duration, Instant};

use coder::generate::{Door, Generate, Message, Meta, Role};
use coder::relay::{Identity, RelayDoor};
use coder::{Agent, Permit, Recorder};
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::nip44;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};

const REQUEST_KIND: u16 = 25_900;
const RESULT_KIND: u16 = 26_900;
const FEEDBACK_KIND: u16 = 27_000;
const AUTH_KIND: u16 = 22_242;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn hex_secret(byte: u8) -> String {
    (0..32).map(|_| format!("{byte:02x}")).collect()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn signer(byte: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&hex_secret(byte)).unwrap()
}

fn xonly(byte: u8) -> XOnlyPublicKey {
    let secret = SecretKey::from_byte_array([byte; 32]).unwrap();
    Keypair::from_secret_key(&Secp256k1::new(), &secret)
        .x_only_public_key()
        .0
}

async fn send(socket: &mut Socket, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

async fn read_json(socket: &mut Socket) -> Value {
    loop {
        let message = socket.next().await.unwrap().unwrap();
        if let tungstenite::Message::Text(text) = message
            && let Ok(value) = serde_json::from_str::<Value>(&text)
        {
            return value;
        }
    }
}

/// Connect, answer the AUTH challenge, return the socket.
async fn authenticated_socket(url: &str, key_byte: u8) -> Socket {
    let (mut socket, _) = connect_async(url).await.unwrap();
    let challenge = read_json(&mut socket).await;
    assert_eq!(challenge[0], "AUTH");
    let auth = signer(key_byte).sign(
        unix_now(),
        AUTH_KIND,
        vec![
            Tag::new(vec!["relay".into(), url.to_string()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().unwrap().into(),
            ]),
        ],
        String::new(),
    );
    let id = auth.id.clone();
    send(&mut socket, json!(["AUTH", auth])).await;
    let ok = read_json(&mut socket).await;
    assert_eq!(ok, json!(["OK", id, true, ""]));
    socket
}

/// The mock worker: auths as `key`, answers the first job request tagged to
/// it with a judgment, two partials, and a result. Each test gives it a key
/// of its own, so two running at once on one relay do not take each other's
/// jobs.
async fn mock_worker(url: String, key: u8, task: &str) {
    let worker_secret = SecretKey::from_byte_array([key; 32]).unwrap();
    let mut socket = authenticated_socket(&url, key).await;
    send(
        &mut socket,
        json!(["REQ", "jobs", {"kinds": [REQUEST_KIND], "#p": [xonly(key).to_string()]}]),
    )
    .await;

    let request = loop {
        let frame = read_json(&mut socket).await;
        if frame[0] == "EVENT" {
            break serde_json::from_value::<Event>(frame[2].clone()).unwrap();
        }
    };
    let payload = {
        let conversation = nip44::conversation_key(&worker_secret, &{
            let bytes: [u8; 32] = hex::decode(&request.pubkey).try_into().unwrap();
            XOnlyPublicKey::from_byte_array(bytes).unwrap()
        });
        serde_json::from_str::<Value>(&nip44::decrypt(&request.content, &conversation).unwrap())
            .unwrap()
    };
    assert_eq!(payload["task"], task);

    let customer_bytes: [u8; 32] = hex::decode(&request.pubkey).try_into().unwrap();
    let customer = XOnlyPublicKey::from_byte_array(customer_bytes).unwrap();
    let conversation = nip44::conversation_key(&worker_secret, &customer);
    let publish = |kind: u16, content: Value| {
        let ciphertext = nip44::encrypt(
            &content.to_string(),
            &conversation,
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .unwrap();
        signer(key).sign(
            unix_now(),
            kind,
            vec![
                Tag::new(vec!["e".into(), request.id.clone()]),
                Tag::new(vec!["p".into(), request.pubkey.clone()]),
            ],
            ciphertext,
        )
    };

    for event in [
        publish(
            FEEDBACK_KIND,
            json!({"v":2,"type":"judgment","verdict":"respond","line":"respond 1.00 · conf 1.00"}),
        ),
        publish(
            FEEDBACK_KIND,
            json!({"v":2,"type":"partial","seq":0,"delta":"hello"}),
        ),
        publish(
            FEEDBACK_KIND,
            json!({"v":2,"type":"partial","seq":1,"delta":" there"}),
        ),
        publish(
            RESULT_KIND,
            json!({
                "v": 2,
                "type": "result",
                "text": "hello there",
                "usage": {"input": 10, "output": 2},
                "model": "test/worker-model",
            }),
        ),
    ] {
        send(&mut socket, json!(["EVENT", event])).await;
    }
    for _ in 0..4 {
        let ack = read_json(&mut socket).await;
        assert_eq!(ack[0], "OK");
        assert_eq!(ack[2], true);
    }
}

mod hex {
    pub fn decode(text: &str) -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let high = (pair[0] as char).to_digit(16).unwrap();
                let low = (pair[1] as char).to_digit(16).unwrap();
                ((high << 4) | low) as u8
            })
            .collect()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_job_turn_streams_feedback_and_a_result() {
    let Ok(url) = std::env::var("CODER_RELAY") else {
        eprintln!("skipped: set CODER_RELAY (and the secret envs) to run");
        return;
    };
    unsafe { std::env::set_var("CODER_SECRET_KEY", hex_secret(0x0b)) };

    tokio::spawn(mock_worker(url.clone(), 0xaa, "say hi in one word"));
    tokio::time::sleep(Duration::from_millis(300)).await;

    let door = RelayDoor::new(url, xonly(0xaa), Identity::load().unwrap());
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let mut seen = String::new();
    let mut judgment = String::new();
    let mut model = String::new();
    let (text, usage) = door
        .generate(
            "be terse",
            &input,
            &mut |delta| seen.push_str(delta),
            &mut |meta| match meta {
                Meta::Judgment(line) => judgment = line,
                Meta::Model(name) => model = name,
            },
        )
        .await
        .unwrap();

    assert_eq!(judgment, "respond 1.00 · conf 1.00");
    assert_eq!(seen, "hello there");
    assert_eq!(text, "hello there");
    assert_eq!(usage.unwrap().input_tokens, 10);
    // The worker names the model it answered through, and the door hands
    // it on. Without this the run's evidence cannot say what produced it.
    assert_eq!(model, "test/worker-model");
}

/// A worker that reads a job and then goes quiet: it authenticates,
/// subscribes, and answers with one judgment feedback and nothing else.
async fn stalling_worker(url: String) {
    let mut socket = authenticated_socket(&url, 0xad).await;
    send(
        &mut socket,
        json!(["REQ", "stall", {"kinds": [REQUEST_KIND], "#p": [xonly(0xad).to_string()]}]),
    )
    .await;

    let request = loop {
        let frame = read_json(&mut socket).await;
        if frame[0] == "EVENT" {
            break serde_json::from_value::<Event>(frame[2].clone()).unwrap();
        }
    };
    let worker_secret = SecretKey::from_byte_array([0xad; 32]).unwrap();
    let customer_bytes: [u8; 32] = hex::decode(&request.pubkey).try_into().unwrap();
    let customer = XOnlyPublicKey::from_byte_array(customer_bytes).unwrap();
    let conversation = nip44::conversation_key(&worker_secret, &customer);
    let ciphertext = nip44::encrypt(
        &json!({"v":2,"type":"judgment","verdict":"respond","line":"respond 1.00"}).to_string(),
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    let event = signer(0xad).sign(
        unix_now(),
        FEEDBACK_KIND,
        vec![
            Tag::new(vec!["e".into(), request.id.clone()]),
            Tag::new(vec!["p".into(), request.pubkey.clone()]),
        ],
        ciphertext,
    );
    send(&mut socket, json!(["EVENT", event])).await;
    // And then nothing, for longer than the answer wait under test.
    tokio::time::sleep(Duration::from_secs(30)).await;
}

/// A worker that is not there fails in the short wait, and says so.
///
/// Nobody subscribes for key `0xac`, so the request is published, fanned
/// out to nothing, and left. The contact wait is what bounds that, not the
/// answer wait: an absent worker used to cost the whole 180 seconds and
/// then report a silence that could not say whether anyone had been
/// listening.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_absent_worker_fails_in_the_contact_wait() {
    let Ok(url) = std::env::var("CODER_RELAY") else {
        eprintln!("skipped: set CODER_RELAY (and the secret envs) to run");
        return;
    };
    unsafe { std::env::set_var("CODER_SECRET_KEY", hex_secret(0x0d)) };

    let door = RelayDoor::new(url, xonly(0xac), Identity::load().unwrap())
        .waiting(Duration::from_secs(2), Duration::from_secs(60));
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let started = Instant::now();
    let error = door
        .generate("be terse", &input, &mut |_| {}, &mut |_| {})
        .await
        .expect_err("nobody is listening");
    let waited = started.elapsed();

    assert_eq!(error.cause(), "worker_absent");
    assert_eq!(error.refusal(), None);
    assert!(error.to_string().contains("no worker answered"), "{error}");
    // The short wait, not the long one.
    assert!(waited < Duration::from_secs(30), "waited {waited:?}");
}

/// A worker that answers and then stops is not an absent one.
///
/// The judgment feedback proves somebody read the job, so the wait becomes
/// the answer wait and the failure says the worker stalled. This is the
/// whole of what an ephemeral protocol lets a client tell apart without a
/// prompt acknowledgement, and it is the part worth having: a slow worker
/// and a missing one are different problems.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_worker_that_starts_and_stops_is_not_an_absent_one() {
    let Ok(url) = std::env::var("CODER_RELAY") else {
        eprintln!("skipped: set CODER_RELAY (and the secret envs) to run");
        return;
    };
    unsafe { std::env::set_var("CODER_SECRET_KEY", hex_secret(0x0e)) };

    tokio::spawn(stalling_worker(url.clone()));
    tokio::time::sleep(Duration::from_millis(300)).await;

    let door = RelayDoor::new(url, xonly(0xad), Identity::load().unwrap())
        .waiting(Duration::from_secs(2), Duration::from_secs(6));
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let started = Instant::now();
    let error = door
        .generate("be terse", &input, &mut |_| {}, &mut |_| {})
        .await
        .expect_err("the worker never finished");
    let waited = started.elapsed();

    assert_eq!(error.cause(), "worker_stalled");
    assert_eq!(error.refusal(), None);
    assert!(error.to_string().contains("stopped mid-answer"), "{error}");
    // Hearing from the worker moved the door off the contact wait.
    assert!(waited > Duration::from_secs(3), "waited {waited:?}");
}

/// A worker that declines: auths as key byte 0xAB and answers the first
/// job request with NIP-CJ `status: error` feedback carrying a code.
async fn declining_worker(url: String) {
    let worker_secret = SecretKey::from_byte_array([0xab; 32]).unwrap();
    let mut socket = authenticated_socket(&url, 0xab).await;
    send(
        &mut socket,
        json!(["REQ", "refusals", {"kinds": [REQUEST_KIND], "#p": [xonly(0xab).to_string()]}]),
    )
    .await;

    let request = loop {
        let frame = read_json(&mut socket).await;
        if frame[0] == "EVENT" {
            break serde_json::from_value::<Event>(frame[2].clone()).unwrap();
        }
    };
    let customer_bytes: [u8; 32] = hex::decode(&request.pubkey).try_into().unwrap();
    let customer = XOnlyPublicKey::from_byte_array(customer_bytes).unwrap();
    let conversation = nip44::conversation_key(&worker_secret, &customer);
    let content = json!({
        "v": 2,
        "type": "status",
        "status": "error",
        "code": "quota_exhausted",
        "message": "free allowance used",
    })
    .to_string();
    let ciphertext = nip44::encrypt(
        &content,
        &conversation,
        secp256k1::rand::random::<[u8; 32]>(),
    )
    .unwrap();
    let event = signer(0xab).sign(
        unix_now(),
        FEEDBACK_KIND,
        vec![
            Tag::new(vec!["e".into(), request.id.clone()]),
            Tag::new(vec!["p".into(), request.pubkey.clone()]),
        ],
        ciphertext,
    );
    send(&mut socket, json!(["EVENT", event])).await;
}

/// A worker that declines is not a transport that broke.
///
/// The turn reaches a worker, the worker says no with a code, and the
/// caller gets a `GenerateError::Refused` carrying that code as a field.
/// An unreachable relay and an absent worker both answer `refusal()` with
/// `None`, which is the line `gym::eval::classify` draws.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_declining_worker_refuses_with_a_code() {
    let Ok(url) = std::env::var("CODER_RELAY") else {
        eprintln!("skipped: set CODER_RELAY (and the secret envs) to run");
        return;
    };
    unsafe { std::env::set_var("CODER_SECRET_KEY", hex_secret(0x0c)) };

    tokio::spawn(declining_worker(url.clone()));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let door = RelayDoor::new(url, xonly(0xab), Identity::load().unwrap());
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let error = door
        .generate("be terse", &input, &mut |_| {}, &mut |_| {})
        .await
        .expect_err("the worker declined");

    assert_eq!(error.cause(), "worker_declined");
    assert_eq!(error.refusal(), Some("quota_exhausted"));
    assert_eq!(
        error.to_string(),
        "the worker declined (quota_exhausted): free allowance used"
    );
}

/// A relay run's trace names the model the worker used.
///
/// The session header cannot: it is written when the session opens, and a
/// relay door does not know what will answer until something does. So it
/// records `unknown` and the answer step carries the model the NIP-CJ
/// result named. A trace that said `relay` there named a model that does
/// not exist, and `gym compare` refuses a comparison when door identity
/// moves — which it cannot do for an identity it cannot read.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_relay_trace_names_the_model_the_worker_used() {
    let Ok(url) = std::env::var("CODER_RELAY") else {
        eprintln!("skipped: set CODER_RELAY (and the secret envs) to run");
        return;
    };
    unsafe { std::env::set_var("CODER_SECRET_KEY", hex_secret(0x0f)) };

    tokio::spawn(mock_worker(url.clone(), 0xae, "say hi in one word"));
    tokio::time::sleep(Duration::from_millis(300)).await;

    let door = Door::Relay(Box::new(RelayDoor::new(
        url,
        xonly(0xae),
        Identity::load().unwrap(),
    )));
    let traces = tempfile::tempdir().unwrap();
    let recorder = Recorder::open(traces.path(), door.model(), door.name(), "/tmp/repo").unwrap();
    let path = recorder.path().to_path_buf();
    let mut agent = Agent::new(None, door).with_trace(Some(recorder));

    agent.push_user("say hi in one word");
    let coder::Turned { text: reply, .. } = agent
        .turn(
            false,
            Permit::executing(),
            &mut |_| {},
            &mut |_| {},
            &mut |_| {},
        )
        .await
        .expect("the worker answered");
    agent.finish_trace();
    assert_eq!(reply, "hello there");

    let recording = atif::log::read(&path).expect("the trace reads back");
    // The header says which transport, and says it does not know the model.
    assert_eq!(recording.session.door, "relay");
    assert_eq!(recording.session.model, "unknown");

    let document = recording.document();
    let answer = document["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["source"] == "agent")
        .expect("an answer step");
    assert_eq!(answer["message"], "hello there");
    assert_eq!(answer["model_name"], "test/worker-model");
}
