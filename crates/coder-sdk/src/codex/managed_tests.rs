use super::*;
use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

struct GateWriter {
    inner: WriteHalf<DuplexStream>,
    left: Arc<AtomicUsize>,
}
impl AsyncWrite for GateWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let limit = self.left.load(Ordering::Relaxed);
        if limit == 0 {
            return Poll::Ready(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)));
        }
        match Pin::new(&mut self.inner).poll_write(cx, &bytes[..bytes.len().min(limit)]) {
            Poll::Ready(Ok(n)) => {
                self.left.fetch_sub(n, Ordering::Relaxed);
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

struct Peer {
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    gate: Arc<AtomicUsize>,
}
fn pair(size: usize) -> (Managed, Peer) {
    let (client, peer) = tokio::io::duplex(size);
    let (reader, writer) = tokio::io::split(client);
    let (pr, pw) = tokio::io::split(peer);
    let gate = Arc::new(AtomicUsize::new(usize::MAX));
    (
        Managed::over(
            BufReader::new(reader),
            GateWriter {
                inner: writer,
                left: gate.clone(),
            },
        ),
        Peer {
            reader: BufReader::new(pr),
            writer: pw,
            gate,
        },
    )
}
fn deadline() -> Instant {
    Instant::now() + std::time::Duration::from_secs(5)
}
impl Peer {
    async fn read(&mut self) -> Value {
        let mut line = String::new();
        self.reader.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }
    async fn send(&mut self, body: Value) {
        self.raw(format!("{}\n", body).as_bytes()).await;
    }
    async fn raw(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).await.unwrap();
    }
    async fn answer(&mut self, id: i64, result: Value) {
        self.send(json!({"id":id,"result":result})).await;
    }
}
async fn idle() -> (Managed, Peer) {
    let (mut port, mut peer) = pair(2 * FRAME);
    let init = port
        .initialize("managed-test", "1", deadline())
        .await
        .unwrap();
    assert_eq!(peer.read().await["method"], "initialize");
    peer.answer(init.id, json!({"userAgent":"codex/fixture"}))
        .await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Answer{request,result:Ok(_)} if request==init)
    );
    assert_eq!(port.phase(), Phase::Initialized);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(2), peer.read())
            .await
            .is_err()
    );
    port.initialized(deadline()).await.unwrap();
    assert_eq!(peer.read().await["method"], "initialized");
    let open=port.open(json!({"cwd":"/fixture","model":"local","approvalPolicy":"untrusted","sandbox":"read-only"}),deadline()).await.unwrap();
    let seen = peer.read().await;
    assert_eq!(seen["method"], "thread/start");
    assert_eq!(seen["params"]["sandbox"], "read-only");
    peer.answer(open.id, json!({"thread":{"id":"thread-1"},"model":"local"}))
        .await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Answer{request,result:Ok(_)} if request==open)
    );
    assert_eq!(port.phase(), Phase::Idle);
    port.take_captures();
    (port, peer)
}
async fn running() -> (Managed, Peer) {
    let (mut port, mut peer) = idle().await;
    let start = port
        .start(
            vec![json!({"type":"text","text":"private prompt"})],
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(port.phase(), Phase::Starting);
    let body = peer.read().await;
    assert_eq!(body["method"], "turn/start");
    assert_eq!(body["params"]["threadId"], "thread-1");
    peer.answer(start.id, json!({"turn":{"id":"turn-1"}})).await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Answer{request,result:Ok(_)} if request==start)
    );
    assert_eq!(port.phase(), Phase::Running);
    (port, peer)
}
fn approval() -> Value {
    json!({"id":51,"method":"item/commandExecution/requestApproval","params":{
        "threadId":"thread-1","turnId":"turn-1","itemId":"call-1","command":"hash the selected input"}})
}
fn terminal() -> Value {
    json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"completed"}}})
}

#[tokio::test]
async fn native_acknowledgments_and_late_control_replies_keep_the_original_turn() {
    let (mut port, mut peer) = running().await;
    assert_eq!(
        port.steer("other", vec![], deadline()).await,
        Err(Error::ChangedSource)
    );
    let steer = port
        .steer(
            "turn-1",
            vec![json!({"type":"text","text":"correction"})],
            deadline(),
        )
        .await
        .unwrap();
    let sent = peer.read().await;
    assert_eq!(sent["params"]["expectedTurnId"], "turn-1");
    peer.send(terminal()).await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Terminal{turn,status,..} if turn=="turn-1" && status=="completed")
    );
    assert_eq!(port.phase(), Phase::Idle);
    assert_eq!(port.start(vec![], deadline()).await, Err(Error::State));
    peer.answer(steer.id, json!({})).await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Answer{request,..} if request.turn.as_deref()==Some("turn-1"))
    );
    assert_eq!(port.phase(), Phase::Idle);
    assert_eq!(
        port.steer("turn-1", vec![], deadline()).await,
        Err(Error::ChangedSource)
    );
    let captures = port.take_captures();
    assert!(
        captures
            .iter()
            .any(|c| c.direction == Direction::Received && strict(&c.bytes).unwrap() == terminal())
    );
    assert!(!format!("{captures:?}").contains("private prompt"));
}

#[tokio::test]
async fn permission_delivery_needs_the_original_request_and_never_repeats() {
    for allow in [true, false] {
        let (mut port, mut peer) = running().await;
        peer.send(approval()).await;
        let Event::Reverse(request) = port.next(deadline()).await.unwrap() else {
            panic!()
        };
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(2), peer.read())
                .await
                .is_err()
        );
        let mut changed = request.clone();
        changed.params["command"] = json!("another operation");
        assert_eq!(
            port.permission(&changed, allow, deadline()).await,
            Err(Error::ChangedSource)
        );
        let (mut other, _) = running().await;
        assert_eq!(
            other.permission(&request, allow, deadline()).await,
            Err(Error::ChangedSource)
        );
        port.permission(&request, allow, deadline()).await.unwrap();
        let response = peer.read().await;
        assert_eq!(response["id"], 51);
        assert_eq!(
            response["result"]["decision"],
            if allow { "accept" } else { "decline" }
        );
        assert_eq!(
            port.permission(&request, allow, deadline()).await,
            Err(Error::ChangedSource)
        );
        peer.send(approval()).await;
        assert!(matches!(
            port.next(deadline()).await.unwrap(),
            Event::Fault { .. }
        ));
        assert_eq!(port.phase(), Phase::Faulted);
    }
}

#[tokio::test]
async fn unknown_required_methods_remain_faulted_after_denial_and_terminal_evidence() {
    let (mut port, mut peer) = running().await;
    peer.send(json!({"id":"reverse","method":"unknown/required","params":{"secret":"private"}}))
        .await;
    let Event::Reverse(request) = port.next(deadline()).await.unwrap() else {
        panic!()
    };
    assert_eq!(port.phase(), Phase::Faulted);
    assert_eq!(
        port.steer("turn-1", vec![], deadline()).await,
        Err(Error::ChangedSource)
    );
    assert!(port.permission(&request, true, deadline()).await.is_err());
    port.deny_unsupported(&request, deadline()).await.unwrap();
    let denied = peer.read().await;
    assert_eq!(denied["error"]["code"], -32601);
    let stop = port.interrupt("turn-1", deadline()).await.unwrap();
    peer.read().await;
    peer.answer(stop.id, json!({})).await;
    port.next(deadline()).await.unwrap();
    peer.send(terminal()).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Terminal { .. }
    ));
    assert_eq!(port.phase(), Phase::Faulted);
    assert!(port.start(vec![], deadline()).await.is_err());
    assert!(!format!("{request:?}").contains("private"));
}

#[tokio::test]
async fn partial_reads_keep_original_bytes_and_duplicate_json_keys_refuse() {
    let (mut port, mut peer) = running().await;
    port.take_captures();
    let raw=b"{ \"method\": \"item/agentMessage/delta\", \"params\": {\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"delta\":\"text\"} }\n";
    peer.raw(&raw[..20]).await;
    assert!(matches!(
        port.next(Instant::now() + std::time::Duration::from_millis(2))
            .await
            .unwrap(),
        Event::Waiting
    ));
    peer.raw(&raw[20..]).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Notification { .. }
    ));
    assert_eq!(port.take_captures()[0].bytes, raw);
    peer.raw(b"{\"method\":\"item/started\",\"params\":{\"item\":{\"id\":\"a\",\"id\":\"b\"}}}\n")
        .await;
    assert!(matches!(port.next(deadline()).await, Err(Error::Malformed)));
    assert_eq!(port.phase(), Phase::Faulted);
    assert_eq!(port.take_captures().len(), 1);
}

#[tokio::test]
async fn a_partial_write_cannot_retry_or_accept_an_impossible_acknowledgment() {
    let (mut port, mut peer) = pair(8);
    assert_eq!(
        port.initialize(
            "test",
            "1",
            Instant::now() + std::time::Duration::from_millis(2)
        )
        .await,
        Err(Error::UnknownDelivery)
    );
    assert_eq!(port.phase(), Phase::Unknown);
    assert_eq!(
        port.initialize("test", "1", deadline()).await,
        Err(Error::State)
    );
    let captures = port.take_captures();
    assert_eq!(captures[0].bytes.len(), 8);
    assert!(!captures[0].complete);
    let mut fragment = [0u8; 8];
    peer.reader.read_exact(&mut fragment).await.unwrap();
    let writer = tokio::spawn(async move {
        peer.answer(1, json!({"userAgent":"fixture"})).await;
    });
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Fault { .. }
    ));
    writer.await.unwrap();
}

#[tokio::test]
async fn a_lost_acknowledgment_can_only_be_observed_without_sending_again() {
    let (mut port, mut peer) = pair(2 * FRAME);
    let sent = port.initialize("test", "1", deadline()).await.unwrap();
    peer.read().await;
    assert!(matches!(
        port.next(Instant::now()).await,
        Err(Error::UnknownDelivery)
    ));
    assert_eq!(
        port.initialize("test", "1", deadline()).await,
        Err(Error::State)
    );
    peer.answer(sent.id, json!({"userAgent":"fixture"})).await;
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Answer{request,..} if request==sent)
    );
    assert_eq!(port.phase(), Phase::Initialized);
    assert_eq!(
        port.take_captures()
            .iter()
            .filter(|c| c.direction == Direction::Sent)
            .count(),
        1
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(2), peer.read())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_lost_permission_write_is_consumed_before_any_retry() {
    let (mut port, mut peer) = running().await;
    peer.send(approval()).await;
    let Event::Reverse(request) = port.next(deadline()).await.unwrap() else {
        panic!()
    };
    port.take_captures();
    peer.gate.store(8, Ordering::Relaxed);
    assert_eq!(
        port.permission(&request, true, deadline()).await,
        Err(Error::UnknownDelivery)
    );
    assert_eq!(port.phase(), Phase::Unknown);
    assert!(port.permission(&request, true, deadline()).await.is_err());
    let mut fragment = [0u8; 8];
    peer.reader.read_exact(&mut fragment).await.unwrap();
    let captures = port.take_captures();
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].bytes, fragment);
    assert!(!captures[0].complete);
    peer.gate.store(usize::MAX, Ordering::Relaxed);
    peer.send(approval()).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Fault { .. }
    ));
    assert!(port.permission(&request, true, deadline()).await.is_err());
}

#[tokio::test]
async fn cancelling_a_read_preserves_its_frame_and_eof_keeps_the_partial_source() {
    let (mut port, mut peer) = running().await;
    port.take_captures();
    peer.raw(b"{ \"method\":").await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(2), port.next(deadline()))
            .await
            .is_err()
    );
    assert_eq!(port.partial_capture().unwrap().bytes, b"{ \"method\":");
    peer.raw(b"\"item/started\",\"params\":{}}\n").await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Notification { .. }
    ));
    assert_eq!(
        port.take_captures()[0].bytes,
        b"{ \"method\":\"item/started\",\"params\":{}}\n"
    );
    peer.raw(b"{\"unfinished\":").await;
    peer.writer.shutdown().await.unwrap();
    assert!(matches!(
        port.next(deadline()).await,
        Err(Error::Disconnected)
    ));
    let captures = port.take_captures();
    assert_eq!(captures[0].bytes, b"{\"unfinished\":");
    assert!(!captures[0].complete);
}

#[tokio::test]
async fn native_identity_errors_and_missing_terminal_status_never_complete_a_turn() {
    for mode in ["thread", "turn", "status", "unknown", "request-id"] {
        let (mut port, mut peer) = running().await;
        port.take_captures();
        let mut body = terminal();
        match mode {
            "thread" => body["params"]["threadId"] = json!("another-thread"),
            "turn" => body["params"]["turn"]["id"] = json!("another-turn"),
            "status" => {
                body["params"]["turn"]
                    .as_object_mut()
                    .unwrap()
                    .remove("status");
            }
            "unknown" => body = json!({"method":"unknown/notification","params":{}}),
            "request-id" => body = json!({"id":9999,"result":{}}),
            _ => unreachable!(),
        }
        peer.send(body).await;
        assert!(
            matches!(port.next(deadline()).await.unwrap(), Event::Fault { .. }),
            "{mode}"
        );
        assert_eq!(port.turn(), Some("turn-1"));
        assert_eq!(port.phase(), Phase::Faulted);
        assert!(port.start(vec![], deadline()).await.is_err());
        assert!(
            port.take_captures()
                .iter()
                .all(|c| c.direction == Direction::Received)
        );
    }
}

#[tokio::test]
async fn an_acknowledgment_cannot_clear_an_earlier_required_method_fault() {
    let (mut port, mut peer) = pair(2 * FRAME);
    let init = port.initialize("test", "1", deadline()).await.unwrap();
    peer.read().await;
    peer.send(json!({"method":"unknown/notification","params":{}}))
        .await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Fault { .. }
    ));
    peer.answer(init.id, json!({"userAgent":"fixture"})).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Answer { .. }
    ));
    assert_eq!(port.phase(), Phase::Faulted);
    assert_eq!(port.initialized(deadline()).await, Err(Error::State));
}

#[tokio::test]
async fn frame_and_capture_limits_refuse_without_discarding_source_bytes() {
    let (mut port, mut peer) = running().await;
    port.take_captures();
    peer.raw(&vec![b' '; FRAME + 1]).await;
    assert!(matches!(port.next(deadline()).await, Err(Error::Limit)));
    let prefix = port.partial_capture().unwrap();
    assert!(!prefix.complete);
    assert!(prefix.bytes.len() <= FRAME);
    assert_eq!(port.phase(), Phase::Faulted);
    let (mut port, mut peer) = running().await;
    port.take_captures();
    for _ in 0..CAPTURES {
        peer.send(json!({"method":"item/started","params":{}}))
            .await;
        port.next(deadline()).await.unwrap();
    }
    peer.send(json!({"method":"item/completed","params":{}}))
        .await;
    assert!(matches!(port.next(deadline()).await, Err(Error::Limit)));
    assert_eq!(port.take_captures().len(), CAPTURES);
    assert!(
        matches!(port.next(deadline()).await.unwrap(),Event::Notification{method,..} if method=="item/completed")
    );
}

#[tokio::test]
async fn pending_reverse_replays_need_the_exact_original_native_frame() {
    let (mut port, mut peer) = running().await;
    peer.send(approval()).await;
    let Event::Reverse(first) = port.next(deadline()).await.unwrap() else {
        panic!()
    };
    peer.raw(first.source()).await;
    let Event::Reverse(duplicate) = port.next(deadline()).await.unwrap() else {
        panic!()
    };
    assert_eq!(first, duplicate);
    peer.raw(format!(" {}\n", approval()).as_bytes()).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Fault { .. }
    ));
    assert!(port.permission(&first, true, deadline()).await.is_err());
}

#[tokio::test]
async fn an_announced_turn_must_match_its_later_native_acknowledgment() {
    for matches in [true, false] {
        let (mut port, mut peer) = idle().await;
        let start = port
            .start(vec![json!({"type":"text","text":"prompt"})], deadline())
            .await
            .unwrap();
        peer.read().await;
        peer.send(json!({"method":"turn/started","params":{"threadId":"thread-1","turnId":"turn-announced","turn":{"id":"turn-announced"}}})).await;
        assert!(matches!(
            port.next(deadline()).await.unwrap(),
            Event::Notification { .. }
        ));
        assert_eq!(port.phase(), Phase::Starting);
        assert_eq!(port.turn(), None);
        peer.answer(
            start.id,
            json!({"turn":{"id":if matches {"turn-announced"}else{"another-turn"}}}),
        )
        .await;
        let result = port.next(deadline()).await.unwrap();
        if matches {
            assert!(matches!(result, Event::Answer { .. }));
            assert_eq!(port.turn(), Some("turn-announced"));
        } else {
            assert!(matches!(result, Event::Fault { .. }));
            assert_eq!(port.phase(), Phase::Faulted);
            assert_eq!(port.turn(), None);
        }
    }
}

#[tokio::test]
async fn stop_has_a_reserved_slot_and_needs_terminal_evidence_before_new_input() {
    let (mut port, mut peer) = running().await;
    for _ in 0..PENDING - 1 {
        port.steer(
            "turn-1",
            vec![json!({"type":"text","text":"correction"})],
            deadline(),
        )
        .await
        .unwrap();
        peer.read().await;
    }
    assert_eq!(
        port.steer("turn-1", vec![], deadline()).await,
        Err(Error::Limit)
    );
    peer.send(approval()).await;
    let Event::Reverse(request) = port.next(deadline()).await.unwrap() else {
        panic!()
    };
    let stop = port.interrupt("turn-1", deadline()).await.unwrap();
    assert_eq!(peer.read().await["method"], "turn/interrupt");
    assert_eq!(port.phase(), Phase::Stopping);
    assert!(port.permission(&request, true, deadline()).await.is_err());
    assert!(port.interrupt("turn-1", deadline()).await.is_err());
    peer.answer(stop.id, json!({})).await;
    port.next(deadline()).await.unwrap();
    assert_eq!(port.phase(), Phase::Stopping);
    assert!(port.start(vec![], deadline()).await.is_err());
    port.permission(&request, false, deadline()).await.unwrap();
    assert_eq!(peer.read().await["result"]["decision"], "decline");
    peer.send(terminal()).await;
    assert!(matches!(
        port.next(deadline()).await.unwrap(),
        Event::Terminal { .. }
    ));
    assert_eq!(port.phase(), Phase::Idle);
}

#[tokio::test]
async fn expired_or_oversized_input_refuses_before_any_native_bytes() {
    let (mut port, mut peer) = idle().await;
    assert_eq!(
        port.start(vec![], Instant::now()).await,
        Err(Error::Timeout)
    );
    assert_eq!(port.phase(), Phase::Idle);
    assert_eq!(
        port.start(
            vec![json!({"type":"text","text":"x".repeat(FRAME)})],
            deadline()
        )
        .await,
        Err(Error::Limit)
    );
    assert_eq!(port.phase(), Phase::Idle);
    assert!(port.take_captures().is_empty());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(2), peer.read())
            .await
            .is_err()
    );
}
