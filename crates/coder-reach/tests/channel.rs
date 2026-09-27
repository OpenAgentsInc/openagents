//! Loopback socket tests for the direct-channel handshake.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_reach::channel::{
    Acceptor, ClientConfig, ClientHello, FrameKind, GrantCheck, GrantRefusal, HostProof,
    MAX_DATA_BYTES, MAX_FRAME_BYTES, UNAUTHENTICATED, connect, read_frame, transcript, write_frame,
};
use coder_reach::{Refusal, new_id, pubkey};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const NOW: u64 = 1_800_000_000;
const TIMEOUT: Duration = Duration::from_secs(5);

fn key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}

/// A stand-in for the host's grant store: device → (grant, epoch, revoked).
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

/// Run one accept on the listener and one connect; return both results.
type Opened = coder_reach::Result<coder_reach::channel::Channel<TcpStream>>;

async fn run(setup: &Setup, config: ClientConfig) -> (Opened, Opened) {
    let addr = setup.listener.local_addr().unwrap();
    let acceptor = Arc::clone(&setup.acceptor);
    let (host, client) = tokio::join!(
        async {
            let (stream, _) = setup.listener.accept().await.unwrap();
            acceptor.accept(stream, NOW).await
        },
        async {
            let stream = TcpStream::connect(addr).await.unwrap();
            connect(stream, &config, NOW).await
        }
    );
    (host, client)
}

#[tokio::test]
async fn handshake_succeeds_and_carries_sequenced_encrypted_data() {
    let setup = setup(3).await;
    let (host, client) = run(&setup, client(&setup, &key(2), 3)).await;
    let mut host = host.unwrap();
    let mut client = client.unwrap();
    assert_eq!(host.binding(), client.binding());
    assert_eq!(host.binding().generation, 3);
    assert_eq!(host.binding().client, pubkey(&key(9)));
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
async fn data_is_not_plaintext_on_the_wire() {
    // A relay-free observer between the two sockets sees only ciphertext.
    let setup = setup(1).await;
    let addr = setup.listener.local_addr().unwrap();
    let acceptor = Arc::clone(&setup.acceptor);
    let tap = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tap_addr = tap.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);
    let proxy = tokio::spawn(async move {
        let (mut inbound, _) = tap.accept().await.unwrap();
        let mut outbound = TcpStream::connect(addr).await.unwrap();
        let (mut ri, mut wi) = inbound.split();
        let (mut ro, mut wo) = outbound.split();
        let up = async {
            let mut buf = [0; 4096];
            loop {
                let n = ri.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    let _ = wo.shutdown().await;
                    break;
                }
                captured_clone.lock().unwrap().extend_from_slice(&buf[..n]);
                wo.write_all(&buf[..n]).await.unwrap();
            }
        };
        let down = async {
            let mut buf = [0; 4096];
            loop {
                let n = ro.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    let _ = wi.shutdown().await;
                    break;
                }
                wi.write_all(&buf[..n]).await.unwrap();
            }
        };
        tokio::join!(up, down);
    });
    let config = client(&setup, &key(2), 1);
    let (host, client) = tokio::join!(
        async {
            let (stream, _) = setup.listener.accept().await.unwrap();
            acceptor.accept(stream, NOW).await
        },
        async {
            let stream = TcpStream::connect(tap_addr).await.unwrap();
            connect(stream, &config, NOW).await
        }
    );
    let mut host = host.unwrap();
    let mut client = client.unwrap();
    let secret = b"terminal output that must stay private";
    client.send(secret).await.unwrap();
    assert_eq!(host.recv().await.unwrap().unwrap(), secret);
    client.close().await.unwrap();
    assert_eq!(host.recv().await.unwrap(), None);
    drop(host);
    proxy.await.unwrap();
    let wire = captured.lock().unwrap().clone();
    assert!(!wire.windows(secret.len()).any(|w| w == secret));
}

#[tokio::test]
async fn wrong_host_key_is_refused() {
    let setup = setup(1).await;
    // The client expects key 5; the listener proves key 2.
    let (host, client) = run(&setup, client(&setup, &key(5), 1)).await;
    assert_eq!(host.unwrap_err().code, Refusal::IdentityMismatch);
    let err = client.unwrap_err();
    assert_eq!(err.code, Refusal::IdentityMismatch);
    assert_eq!(err.detail, UNAUTHENTICATED);
}

#[tokio::test]
async fn impersonating_host_fails_signature_check() {
    // A server that claims the expected host key but holds another key.
    let setup = setup(1).await;
    let expected = key(2);
    let impostor = key(6);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let frame = read_frame(&mut stream).await.unwrap();
        let hello: ClientHello = serde_json::from_slice(&frame.body).unwrap();
        let mut proof = HostProof {
            v: "openagents.reach-host-proof.v1".into(),
            requires: vec![],
            nonce: new_id(),
            ephemeral: pubkey(&key(7)),
            generation: hello.generation,
            signature: String::new(),
        };
        let digest = transcript(&hello, &proof).unwrap();
        let mut hash = Sha256::new();
        hash.update(b"openagents.reach-host-proof.v1\0");
        hash.update(digest);
        let message: [u8; 32] = hash.finalize().into();
        let keypair = Keypair::from_secret_key(&Secp256k1::new(), &impostor);
        proof.signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&message, &keypair)
            .to_string();
        let body = serde_json::to_vec(&proof).unwrap();
        write_frame(&mut stream, FrameKind::HostProof, 0, &body)
            .await
            .unwrap();
        // The client must not send a proof to an unproven host.
        let mut rest = Vec::new();
        let _ = stream.read_to_end(&mut rest).await;
        rest
    });
    let stream = TcpStream::connect(addr).await.unwrap();
    let err = connect(stream, &client(&setup, &expected, 1), NOW)
        .await
        .err()
        .unwrap();
    assert_eq!(err.code, Refusal::IdentityMismatch);
    assert!(server.await.unwrap().is_empty());
}

#[tokio::test]
async fn replayed_nonce_is_refused() {
    let setup = setup(1).await;
    let addr = setup.listener.local_addr().unwrap();
    // Record a genuine hello by running a real client against a capturing server.
    let capture = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let capture_addr = capture.local_addr().unwrap();
    let config = client(&setup, &key(2), 1);
    let recorder = tokio::spawn(async move {
        let (mut stream, _) = capture.accept().await.unwrap();
        read_frame(&mut stream).await.unwrap()
    });
    let stream = TcpStream::connect(capture_addr).await.unwrap();
    let _ = connect(stream, &config, NOW).await;
    let hello = recorder.await.unwrap();

    // Present it to the host twice: the first earns a proof, the second a refusal.
    let mut replies = Vec::new();
    for _ in 0..2 {
        let acceptor = Arc::clone(&setup.acceptor);
        let (host, reply) = tokio::join!(
            async {
                let (stream, _) = setup.listener.accept().await.unwrap();
                acceptor.accept(stream, NOW).await
            },
            async {
                let mut stream = TcpStream::connect(addr).await.unwrap();
                write_frame(&mut stream, hello.kind, hello.seq, &hello.body)
                    .await
                    .unwrap();
                let reply = read_frame(&mut stream).await.unwrap();
                drop(stream);
                reply
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
async fn wrong_epoch_and_unknown_grant_are_refused() {
    let setup = setup(1).await;
    let mut config = client(&setup, &key(2), 1);
    config.epoch = 1;
    let (_, client) = run(&setup, config).await;
    assert_eq!(client.err().unwrap().code, Refusal::Stale);
    let mut config = client_for_unknown(&setup);
    config.grant = new_id();
    let (_, client) = run(&setup, config).await;
    assert_eq!(client.err().unwrap().code, Refusal::NotAdmitted);
}

fn client_for_unknown(setup: &Setup) -> ClientConfig {
    client(setup, &key(2), 1)
}

#[tokio::test]
async fn stale_host_generation_is_refused() {
    let setup = setup(4).await;
    // Presence said generation 3; the host restarted into generation 4.
    let (host, client) = run(&setup, client(&setup, &key(2), 3)).await;
    let err = client.err().unwrap();
    assert_eq!(err.code, Refusal::Stale);
    assert_ne!(err.detail, UNAUTHENTICATED);
    // The client left before proving its key, so the host admitted nothing.
    assert!(host.is_err());
    // The host applies the same rule when it decides a verdict.
    let hello = ClientHello {
        v: "openagents.reach-hello.v1".into(),
        requires: vec![],
        client: pubkey(&key(9)),
        host: pubkey(&key(2)),
        grant: setup.grant.clone(),
        epoch: 0,
        generation: 3,
        nonce: new_id(),
        ephemeral: pubkey(&key(8)),
        issued_at: NOW,
    };
    assert_eq!(
        setup.acceptor.admit(&hello, NOW).unwrap_err().code,
        Refusal::Stale
    );
    let current = ClientHello {
        generation: 4,
        ..hello
    };
    setup.acceptor.admit(&current, NOW).unwrap();
}

#[tokio::test]
async fn oversized_frame_is_refused_before_reading_its_body() {
    let setup = setup(1).await;
    let addr = setup.listener.local_addr().unwrap();
    let acceptor = Arc::clone(&setup.acceptor);
    let (host, ()) = tokio::join!(
        async {
            let (stream, _) = setup.listener.accept().await.unwrap();
            acceptor.accept(stream, NOW).await
        },
        async {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            // Claim a frame just over the bound and send no body.
            let len = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
            stream.write_all(&len.to_be_bytes()).await.unwrap();
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest).await;
        }
    );
    assert_eq!(host.err().unwrap().code, Refusal::LimitExceeded);
}

#[tokio::test]
async fn stale_hello_time_is_refused() {
    let setup = setup(1).await;
    let addr = setup.listener.local_addr().unwrap();
    let acceptor = Arc::clone(&setup.acceptor);
    let config = client(&setup, &key(2), 1);
    let (host, client) = tokio::join!(
        async {
            let (stream, _) = setup.listener.accept().await.unwrap();
            acceptor.accept(stream, NOW).await
        },
        async {
            let stream = TcpStream::connect(addr).await.unwrap();
            connect(stream, &config, NOW - 600).await
        }
    );
    assert_eq!(host.err().unwrap().code, Refusal::Stale);
    assert_eq!(client.err().unwrap().code, Refusal::Stale);
}

#[tokio::test]
async fn split_halves_keep_sequence_and_close_in_order() {
    let setup = setup(4).await;
    let (host, client) = run(&setup, client(&setup, &key(2), 4)).await;
    let (mut host_reader, mut host_writer) = host.unwrap().into_split();
    let (mut client_reader, mut client_writer) = client.unwrap().into_split();
    assert_eq!(host_reader.binding().generation, 4);

    // Both directions run at once, each with its own sequence numbers.
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
    assert_eq!(
        host_writer.send(b"late").await.unwrap_err().code,
        Refusal::Unavailable
    );
}
