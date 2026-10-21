#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Stage links over loopback: the handshake, refusals, replay and
//! tampering, bounds, backpressure, and per-hop cost.

use std::time::{Duration, Instant};

use stage_link::{
    FRAME_OVERHEAD, Frame, Hop, Key, Kind, Limits, LinkError, Listener, Refusal, connect,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn hop(generation: u64, from: u16) -> Hop {
    Hop {
        group: "sha256:group".into(),
        generation,
        from,
        to: from + 1,
    }
}

fn frame(generation: u64, step: u64, state: usize) -> Frame {
    Frame {
        kind: Kind::State,
        generation,
        request_id: "req-1".into(),
        attempt: 1,
        step,
        meta: br#"{"seed":7}"#.to_vec(),
        state: (0..state).map(|i| (i % 251) as u8).collect(),
    }
}

/// Accept one link at `expected` with `key`, in the background.
async fn listen(
    key: Key,
    expected: Hop,
    limits: Limits,
) -> (
    String,
    tokio::task::JoinHandle<Result<stage_link::Receiver, LinkError>>,
) {
    let listener = Listener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let task = tokio::spawn(async move {
        let offer = listener.accept(&limits).await?;
        offer.accept(&key, &expected, &limits).await
    });
    (address, task)
}

#[tokio::test]
async fn frames_cross_a_link_in_order_with_their_wire_size() {
    let key = Key::generate().unwrap();
    let limits = Limits::default();
    let (address, accepting) = listen(key.clone(), hop(2, 0), limits.clone()).await;
    let mut sender = connect(&address, &key, &hop(2, 0), &limits).await.unwrap();
    let mut receiver = accepting.await.unwrap().unwrap();
    let mut sent = 0;
    for step in 0..5 {
        sent += sender.send(&frame(2, step, 44_300)).await.unwrap();
    }
    let cancel = Frame {
        kind: Kind::Cancel,
        state: vec![],
        meta: vec![],
        ..frame(2, 5, 0)
    };
    sent += sender.send(&cancel).await.unwrap();
    for step in 0..5 {
        assert_eq!(receiver.recv().await.unwrap(), frame(2, step, 44_300));
    }
    assert_eq!(receiver.recv().await.unwrap(), cancel);
    sender.close().await.unwrap();
    assert_eq!(receiver.recv().await, Err(LinkError::Closed));
    assert_eq!(receiver.bytes(), sent as u64);
    assert_eq!(
        frame(2, 0, 44_300).wire_len(),
        44_300 + FRAME_OVERHEAD + 5 + 10
    );
}

#[tokio::test]
async fn the_wrong_key_generation_hop_or_group_is_refused_before_any_frame() {
    let key = Key::generate().unwrap();
    let limits = Limits::default();
    let cases = [
        (Key::generate().unwrap(), hop(2, 0), Refusal::Auth),
        // A late stage of generation 1, with generation 1's key.
        (key.clone(), hop(1, 0), Refusal::Generation),
        (key.clone(), hop(2, 1), Refusal::Hop),
        (
            key.clone(),
            Hop {
                group: "sha256:other".into(),
                ..hop(2, 0)
            },
            Refusal::Group,
        ),
    ];
    for (their_key, their_hop, why) in cases {
        let (address, accepting) = listen(key.clone(), hop(2, 0), limits.clone()).await;
        let sent = connect(&address, &their_key, &their_hop, &limits).await;
        assert_eq!(sent.err(), Some(LinkError::Refused(why)), "{why}");
        assert_eq!(
            accepting.await.unwrap().err(),
            Some(LinkError::Refused(why))
        );
    }
}

#[tokio::test]
async fn a_sender_refuses_a_listener_without_the_key() {
    let limits = Limits::default();
    let (address, accepting) = listen(Key::generate().unwrap(), hop(2, 0), limits.clone()).await;
    let sent = connect(&address, &Key::generate().unwrap(), &hop(2, 0), &limits).await;
    assert_eq!(sent.err(), Some(LinkError::Refused(Refusal::Auth)));
    assert!(accepting.await.unwrap().is_err());
}

/// What a man in the middle does to the second frame.
#[derive(Clone, Copy)]
enum Meddle {
    Replay,
    Flip,
}

/// A proxy between a sender and a listener that forwards the handshake and
/// then meddles with the second frame.
async fn proxy(upstream: String, meddle: Meddle) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();
        let server = TcpStream::connect(upstream).await.unwrap();
        let (mut server_read, mut server_write) = server.into_split();
        let (mut client_read, mut client_write) = client.split();
        let back = async {
            let _ = tokio::io::copy(&mut server_read, &mut client_write).await;
        };
        let forth = async {
            // Client hello: header, generation, hop, group, nonce.
            let mut head = [0u8; 6 + 8 + 2 + 2 + 1];
            client_read.read_exact(&mut head).await.unwrap();
            let mut rest = vec![0u8; usize::from(head[18]) + 32];
            client_read.read_exact(&mut rest).await.unwrap();
            server_write.write_all(&head).await.unwrap();
            server_write.write_all(&rest).await.unwrap();
            let mut finish = [0u8; 6 + 32];
            client_read.read_exact(&mut finish).await.unwrap();
            server_write.write_all(&finish).await.unwrap();
            for n in 0.. {
                let Ok(len) = client_read.read_u32().await else {
                    return;
                };
                let mut body = vec![0u8; len as usize];
                client_read.read_exact(&mut body).await.unwrap();
                let mut bytes = len.to_be_bytes().to_vec();
                bytes.extend(&body);
                if n == 1 {
                    match meddle {
                        Meddle::Replay => {
                            let _ = server_write.write_all(&bytes).await;
                        }
                        Meddle::Flip => *bytes.last_mut().unwrap() ^= 1,
                    }
                }
                if server_write.write_all(&bytes).await.is_err() {
                    return;
                }
            }
        };
        tokio::join!(back, forth);
    });
    address
}

#[tokio::test]
async fn a_replayed_or_altered_frame_is_refused() {
    let key = Key::generate().unwrap();
    let limits = Limits::default();
    for (meddle, want) in [
        (
            Meddle::Replay,
            LinkError::Sequence {
                got: 1,
                expected: 2,
            },
        ),
        (Meddle::Flip, LinkError::Mac),
    ] {
        let (upstream, accepting) = listen(key.clone(), hop(4, 1), limits.clone()).await;
        let address = proxy(upstream, meddle).await;
        let mut sender = connect(&address, &key, &hop(4, 1), &limits).await.unwrap();
        let mut receiver = accepting.await.unwrap().unwrap();
        for step in 0..3 {
            sender.send(&frame(4, step, 1_000)).await.unwrap();
        }
        assert_eq!(receiver.recv().await.unwrap().step, 0);
        match meddle {
            Meddle::Replay => {
                assert_eq!(receiver.recv().await.unwrap().step, 1);
                assert_eq!(receiver.recv().await, Err(want));
            }
            Meddle::Flip => assert_eq!(receiver.recv().await, Err(want)),
        }
    }
}

#[tokio::test]
async fn frames_over_the_limits_or_from_another_generation_are_refused() {
    let key = Key::generate().unwrap();
    let small = Limits {
        max_state: 1_000,
        ..Limits::default()
    };
    let (address, accepting) = listen(key.clone(), hop(2, 0), small.clone()).await;
    let mut sender = connect(&address, &key, &hop(2, 0), &Limits::default())
        .await
        .unwrap();
    let mut receiver = accepting.await.unwrap().unwrap();
    // The sender won't send another generation's frame on this link.
    assert_eq!(
        sender.send(&frame(1, 0, 10)).await,
        Err(LinkError::Generation {
            got: 1,
            expected: 2
        })
    );
    // The receiver refuses a frame over its limits from the length alone.
    sender.send(&frame(2, 0, 50_000)).await.unwrap();
    assert!(matches!(
        receiver.recv().await,
        Err(LinkError::TooLarge { .. })
    ));
    // And a sender refuses a section over its own limits.
    let (address, accepting) = listen(key.clone(), hop(2, 0), small.clone()).await;
    let mut sender = connect(&address, &key, &hop(2, 0), &small).await.unwrap();
    let _receiver = accepting.await.unwrap().unwrap();
    assert!(matches!(
        sender.send(&frame(2, 0, 1_001)).await,
        Err(LinkError::TooLarge {
            len: 1_001,
            limit: 1_000
        })
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stage_that_stops_reading_fills_a_bounded_queue_and_fails_the_link() {
    let key = Key::generate().unwrap();
    let limits = Limits {
        queue: 2,
        send_timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let (address, accepting) = listen(key.clone(), hop(2, 0), limits.clone()).await;
    let mut sender = connect(&address, &key, &hop(2, 0), &limits).await.unwrap();
    // Connected and never read: the slow consumer.
    let _receiver = accepting.await.unwrap().unwrap();
    let started = Instant::now();
    let mut sent = 0;
    let error = loop {
        match sender.send(&frame(2, sent, 1 << 20)).await {
            Ok(_) => sent += 1,
            Err(error) => break error,
        }
        assert!(sent < 1_000, "the queue never filled");
    };
    eprintln!(
        "slow consumer: {sent} frames of 1 MiB accepted, peak queue {}, failed after {:?}: {error}",
        sender.peak(),
        started.elapsed()
    );
    assert!(matches!(error, LinkError::Backpressure { .. }), "{error}");
    assert!(sender.peak() <= 2);
    // The link stays failed; nothing is queued behind it.
    assert!(sender.send(&frame(2, sent, 10)).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loopback_cost_per_hop_for_v41_stage_state() {
    // Four stages: s0 sends 44,300 bytes (W9's boundary-10 decode
    // message) through s1 and s2 to s3, which answers s0 with the
    // 4-byte token over its own link, as the head would hear it.
    const HOPS: u16 = 3;
    const TOKENS: u64 = 300;
    let key = Key::generate().unwrap();
    let limits = Limits::default();
    let mut listeners = Vec::new();
    for _ in 0..=HOPS {
        listeners.push(Listener::bind("127.0.0.1:0").await.unwrap());
    }
    let addresses: Vec<String> = listeners
        .iter()
        .map(|l| l.local_addr().unwrap().to_string())
        .collect();
    // Stage i accepts from i - 1; the front (index HOPS + 1's role) is
    // listener 0, accepting the last stage's token.
    let mut receivers = Vec::new();
    let mut accepts = Vec::new();
    for (i, listener) in listeners.into_iter().enumerate() {
        let key = key.clone();
        let limits = limits.clone();
        let expected = if i == 0 {
            Hop {
                group: "sha256:group".into(),
                generation: 1,
                from: HOPS,
                to: HOPS + 1,
            }
        } else {
            hop(1, u16::try_from(i).unwrap() - 1)
        };
        accepts.push(tokio::spawn(async move {
            listener
                .accept(&limits)
                .await
                .unwrap()
                .accept(&key, &expected, &limits)
                .await
                .unwrap()
        }));
    }
    let mut senders = Vec::new();
    for i in 0..=HOPS {
        let (to, at) = if i == HOPS {
            (HOPS + 1, &addresses[0])
        } else {
            (i + 1, &addresses[usize::from(i) + 1])
        };
        let hop = Hop {
            group: "sha256:group".into(),
            generation: 1,
            from: i,
            to,
        };
        senders.push(connect(at, &key, &hop, &limits).await.unwrap());
    }
    for accept in accepts {
        receivers.push(accept.await.unwrap());
    }
    // receivers[0] is the front; receivers[i] is stage i.
    let mut front = receivers.remove(0);
    let mut first = senders.remove(0);
    let mut relays = Vec::new();
    for (mut receiver, mut sender) in receivers.into_iter().zip(senders) {
        relays.push(tokio::spawn(async move {
            while let Ok(frame) = receiver.recv().await {
                let out = if sender.hop().to == HOPS + 1 {
                    Frame {
                        state: vec![1, 2, 3, 4],
                        ..frame
                    }
                } else {
                    frame
                };
                if sender.send(&out).await.is_err() {
                    break;
                }
            }
            sender.bytes()
        }));
    }
    let mut times = Vec::new();
    for step in 0..TOKENS {
        let started = Instant::now();
        first.send(&frame(1, step, 44_300)).await.unwrap();
        let back = front.recv().await.unwrap();
        assert_eq!(back.step, step);
        times.push(started.elapsed());
    }
    let bytes = first.bytes();
    drop(first);
    for relay in relays {
        relay.await.unwrap();
    }
    times.sort();
    let median = times[times.len() / 2];
    let p90 = times[times.len() * 9 / 10];
    eprintln!(
        "loopback: {TOKENS} tokens, {} bytes per hop per token ({} of state), median {:?} per token over {HOPS} hops plus the token back ({:?} per hop), p90 {:?}",
        bytes / TOKENS,
        44_300,
        median,
        median / u32::from(HOPS + 1),
        p90
    );
    assert_eq!(bytes / TOKENS, frame(1, 0, 44_300).wire_len() as u64);
}
