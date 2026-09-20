//! Live NIP-CJ contract: a mock worker answers a `RelayDoor` turn over a
//! real relay. Set `CODER_RELAY` (for example `ws://127.0.0.1:8080`),
//! `CODER_WORKER_SECRET`, and `CODER_SECRET_KEY` to run; unset skips.

use coder::generate::{Generate, Message, Meta, Role};
use coder::relay::{Identity, RelayDoor};
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

/// The mock worker: auths as key byte 0xAA, answers the first job request
/// with a judgment, two partials, and a result.
async fn mock_worker(url: String) {
    let worker_secret = SecretKey::from_byte_array([0xaa; 32]).unwrap();
    let mut socket = authenticated_socket(&url, 0xaa).await;
    send(
        &mut socket,
        json!(["REQ", "jobs", {"kinds": [REQUEST_KIND], "#p": [xonly(0xaa).to_string()]}]),
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
    assert_eq!(payload["task"], "say hi in one word");

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
        signer(0xaa).sign(
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
            json!({"v":1,"type":"judgment","verdict":"respond","line":"respond 1.00 · conf 1.00"}),
        ),
        publish(
            FEEDBACK_KIND,
            json!({"v":1,"type":"partial","delta":"hello"}),
        ),
        publish(
            FEEDBACK_KIND,
            json!({"v":1,"type":"partial","delta":" there"}),
        ),
        publish(
            RESULT_KIND,
            json!({"v":1,"type":"result","text":"hello there","usage":{"input":10,"output":2}}),
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

    tokio::spawn(mock_worker(url.clone()));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let door = RelayDoor::new(url, xonly(0xaa), Identity::load().unwrap());
    let input = vec![Message {
        role: Role::User,
        text: "say hi in one word".to_string(),
    }];
    let mut seen = String::new();
    let mut judgment = String::new();
    let (text, usage) = door
        .generate(
            "be terse",
            &input,
            &mut |delta| seen.push_str(delta),
            &mut |meta| {
                let Meta::Judgment(line) = meta;
                judgment = line;
            },
        )
        .await
        .unwrap();

    assert_eq!(judgment, "respond 1.00 · conf 1.00");
    assert_eq!(seen, "hello there");
    assert_eq!(text, "hello there");
    assert_eq!(usage.unwrap().input_tokens, 10);
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
        "v": 1,
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
