//! The direct-channel handshake and frame tests, run over WebSocket.
//!
//! Each case mirrors one in `tests/channel.rs`: the same handshake, frame
//! format, sequence rule, and bounds, carried one frame per binary message.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_reach::channel::{
    Acceptor, ClientConfig, ClientHello, FrameKind, GrantCheck, GrantRefusal, MAX_DATA_BYTES,
    MAX_FRAME_BYTES, UNAUTHENTICATED, connect, read_frame, write_frame,
};
use coder_reach::websocket::{self, MAX_MESSAGE_BYTES, WebSocket};
use coder_reach::{Refusal, new_id, pubkey};
use futures_util::SinkExt;
use secp256k1::SecretKey;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

const NOW: u64 = 1_800_000_000;
const TIMEOUT: Duration = Duration::from_secs(5);

fn key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}

type GrantTable = HashMap<String, (String, u64, bool)>;

#[derive(Clone, Default)]
struct Grants(Arc<Mutex<GrantTable>>);

impl Grants {
    fn admit(&self, device: &SecretKey, grant: &str, epoch: u64) {
        self.0
            .lock()
            .unwrap()
            .insert(pubkey(device), (grant.to_owned(), epoch, false));
    }
    fn revoke(&self, device: &SecretKey) {
        if let Some(entry) = self.0.lock().unwrap().get_mut(&pubkey(device)) {
            entry.2 = true;
        }
    }
}

impl GrantCheck for Grants {
    fn check(&self, device: &str, grant: &str, epoch: u64, _now: u64) -> Result<(), GrantRefusal> {
        match self.0.lock().unwrap().get(device) {
            Some((id, _, _)) if id != grant => Err(GrantRefusal::Unknown),
            Some((_, _, true)) => Err(GrantRefusal::Revoked),
            Some((_, current, false)) if *current != epoch => Err(GrantRefusal::EpochMismatch),
            Some(_) => Ok(()),
            None => Err(GrantRefusal::Unknown),
        }
    }
}

struct Setup {
    acceptor: Arc<Acceptor<Grants>>,
    grants: Grants,
    listener: TcpListener,
    grant: String,
}

impl Setup {
    fn url(&self) -> String {
        format!("ws://{}/", self.listener.local_addr().unwrap())
    }

    /// Accept one connection and answer its WebSocket upgrade.
    async fn upgrade(&self) -> WebSocket<TcpStream> {
        let (stream, _) = self.listener.accept().await.unwrap();
        websocket::accept(stream).await.unwrap()
    }
}

async fn setup(generation: u64) -> Setup {
    let grants = Grants::default();
    let grant = new_id();
    grants.admit(&key(9), &grant, 0);
    let acceptor = Arc::new(Acceptor::new(key(2), generation, grants.clone(), TIMEOUT));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    Setup {
        acceptor,
        grants,
        listener,
        grant,
    }
}

fn client(setup: &Setup, host: &SecretKey, generation: u64) -> ClientConfig {
    ClientConfig {
        device: key(9),
        host: pubkey(host),
        grant: setup.grant.clone(),
        epoch: 0,
        generation,
        timeout: TIMEOUT,
    }
}

async fn dial(url: &str) -> WebSocket<TcpStream> {
    let address = url.trim_start_matches("ws://").trim_end_matches('/');
    let stream = TcpStream::connect(address).await.unwrap();
    websocket::client(url, stream).await.unwrap()
}

type Opened = coder_reach::Result<coder_reach::channel::Channel<WebSocket<TcpStream>>>;

async fn run(setup: &Setup, config: ClientConfig) -> (Opened, Opened) {
    let url = setup.url();
    tokio::join!(
        async { setup.acceptor.accept(setup.upgrade().await, NOW).await },
        async { connect(dial(&url).await, &config, NOW).await }
    )
}

/// Send one raw binary message to the host and return its refusal code.
async fn host_refusal(setup: &Setup, message: Vec<u8>) -> Refusal {
    let url = setup.url();
    let (host, ()) = tokio::join!(
        async { setup.acceptor.accept(setup.upgrade().await, NOW).await },
        async {
            let address = url.trim_start_matches("ws://").trim_end_matches('/');
            let stream = TcpStream::connect(address).await.unwrap();
            let (mut socket, _) = tokio_tungstenite::client_async(url.as_str(), stream)
                .await
                .unwrap();
            let _ = socket.send(Message::Binary(message.into())).await;
            // Hold the socket until the host drops it.
            let _ = futures_util::StreamExt::next(&mut socket).await;
        }
    );
    host.err().unwrap().code
}

#[tokio::test]
async fn handshake_succeeds_and_carries_sequenced_encrypted_data() {
    let setup = setup(3).await;
    let (host, client) = run(&setup, client(&setup, &key(2), 3)).await;
    let mut host = host.unwrap();
    let mut client = client.unwrap();
    assert_eq!(host.binding(), client.binding());
    assert_eq!(host.binding().generation, 3);
    assert_eq!(host.binding().grant, setup.grant);

    client.send(b"first").await.unwrap();
    client.send(&vec![7; MAX_DATA_BYTES]).await.unwrap();
    assert_eq!(host.recv().await.unwrap().unwrap(), b"first");
    assert_eq!(host.recv().await.unwrap().unwrap(), vec![7; MAX_DATA_BYTES]);
    host.send(b"reply").await.unwrap();
    assert_eq!(client.recv().await.unwrap().unwrap(), b"reply");
    assert_eq!(
        client
            .send(&vec![0; MAX_DATA_BYTES + 1])
            .await
            .unwrap_err()
            .code,
        Refusal::LimitExceeded
    );
    client.close().await.unwrap();
    assert_eq!(host.recv().await.unwrap(), None);
}

#[tokio::test]
async fn split_halves_keep_sequence_and_close_in_order() {
    let setup = setup(4).await;
    let (host, client) = run(&setup, client(&setup, &key(2), 4)).await;
    let (mut host_reader, mut host_writer) = host.unwrap().into_split();
    let (mut client_reader, mut client_writer) = client.unwrap().into_split();
    let ((), (to_client, to_host)) = tokio::join!(
        async {
            for n in 0..3u8 {
                host_writer.send(&[n]).await.unwrap();
            }
            client_writer.send(b"request").await.unwrap();
        },
        async {
            let mut got = Vec::new();
            for _ in 0..3 {
                got.push(client_reader.recv().await.unwrap().unwrap());
            }
            (got, host_reader.recv().await.unwrap().unwrap())
        }
    );
    assert_eq!(to_client, vec![vec![0], vec![1], vec![2]]);
    assert_eq!(to_host, b"request");
    host_writer.close().await.unwrap();
    assert_eq!(client_reader.recv().await.unwrap(), None);
}

#[tokio::test]
async fn wrong_host_key_is_refused() {
    let setup = setup(1).await;
    let (host, client) = run(&setup, client(&setup, &key(5), 1)).await;
    assert_eq!(host.unwrap_err().code, Refusal::IdentityMismatch);
    let err = client.unwrap_err();
    assert_eq!(err.code, Refusal::IdentityMismatch);
    assert_eq!(err.detail, UNAUTHENTICATED);
}

#[tokio::test]
async fn replayed_nonce_is_refused() {
    let setup = setup(1).await;
    // Record a genuine hello with a capturing WebSocket server.
    let capture = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let capture_url = format!("ws://{}/", capture.local_addr().unwrap());
    let config = client(&setup, &key(2), 1);
    let recorder = tokio::spawn(async move {
        let (stream, _) = capture.accept().await.unwrap();
        let mut socket = websocket::accept(stream).await.unwrap();
        read_frame(&mut socket).await.unwrap()
    });
    let _ = connect(dial(&capture_url).await, &config, NOW).await;
    let hello = recorder.await.unwrap();
    let parsed: ClientHello = serde_json::from_slice(&hello.body).unwrap();
    assert_eq!(parsed.host, pubkey(&key(2)));

    let url = setup.url();
    let mut replies = Vec::new();
    for _ in 0..2 {
        let (host, reply) = tokio::join!(
            async { setup.acceptor.accept(setup.upgrade().await, NOW).await },
            async {
                let mut socket = dial(&url).await;
                write_frame(&mut socket, hello.kind, hello.seq, &hello.body)
                    .await
                    .unwrap();
                read_frame(&mut socket).await.unwrap()
            }
        );
        replies.push((host.err().map(|e| e.code), reply.kind));
    }
    assert_eq!(replies[0].1, FrameKind::HostProof);
    assert_eq!(replies[1], (Some(Refusal::Replayed), FrameKind::Refusal));
}

#[tokio::test]
async fn revoked_grant_is_refused_after_the_device_proves_its_key() {
    let setup = setup(1).await;
    setup.grants.revoke(&key(9));
    let (host, client) = run(&setup, client(&setup, &key(2), 1)).await;
    assert_eq!(host.unwrap_err().code, Refusal::Revoked);
    let err = client.err().unwrap();
    assert_eq!(err.code, Refusal::Revoked);
    assert_ne!(err.detail, UNAUTHENTICATED);
}

#[tokio::test]
async fn stale_host_generation_is_refused() {
    let setup = setup(4).await;
    let (host, client) = run(&setup, client(&setup, &key(2), 3)).await;
    let err = client.err().unwrap();
    assert_eq!(err.code, Refusal::Stale);
    assert_ne!(err.detail, UNAUTHENTICATED);
    assert!(host.is_err());
}

#[tokio::test]
async fn oversized_frame_is_refused() {
    let setup = setup(1).await;
    // A frame that claims more than the bound, with no body.
    let len = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
    assert_eq!(
        host_refusal(&setup, len.to_be_bytes().to_vec()).await,
        Refusal::LimitExceeded
    );
    // A message over the bound, refused by its WebSocket frame header.
    let mut big = len.to_be_bytes().to_vec();
    big.resize(MAX_MESSAGE_BYTES + 1, 0);
    assert_eq!(host_refusal(&setup, big).await, Refusal::LimitExceeded);
}

#[tokio::test]
async fn a_message_must_carry_exactly_one_frame() {
    let setup = setup(1).await;
    let mut two = Vec::new();
    for _ in 0..2 {
        two.extend(9u32.to_be_bytes());
        two.push(FrameKind::ClientHello as u8);
        two.extend(0u64.to_be_bytes());
    }
    assert_eq!(host_refusal(&setup, two).await, Refusal::Malformed);
    let mut short = 20u32.to_be_bytes().to_vec();
    short.extend([1; 9]);
    assert_eq!(host_refusal(&setup, short).await, Refusal::Malformed);
}
