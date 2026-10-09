//! Ordered admission proceeds independently of ordered durable reply delivery.
use super::*;
use crate::service::transport::write_frame_batch;
use std::{collections::VecDeque, future::Future, pin::Pin};

const CAPACITY: usize = 8;
type WriteFlight<S> = Pin<
    Box<
        dyn Future<Output = Result<(tokio::io::WriteHalf<S>, Vec<(usize, Response, bool)>), String>>
            + Send,
    >,
>;
struct Reader {
    receive: mpsc::Receiver<Result<Vec<u8>, String>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Reader {
    fn new<R: AsyncRead + Unpin + Send + 'static>(mut reader: R) -> Self {
        let (send, receive) = mpsc::channel(CAPACITY);
        let task = tokio::spawn(async move {
            loop {
                let result = timeout(IDLE, read_frame(&mut reader, MAX_REQUEST_BYTES))
                    .await
                    .map_err(|_| "Chamber read timed out".to_string())
                    .and_then(|result| result);
                let failed = result.is_err();
                if send.send(result).await.is_err() || failed {
                    break;
                }
            }
        });
        Self { receive, task }
    }
}
enum Delivery {
    Host(oneshot::Receiver<DispatchReply>),
    Ready(Option<DispatchReply>),
    RateLimited(u64),
}
impl Delivery {
    // Collect only successful replies already available. An unready or failed
    // reply stays at the ordered boundary until earlier writes finish.
    fn take_ready(&mut self, last: &Response) -> Option<(Vec<u8>, bool)> {
        match self {
            Self::Host(receive) => match receive.try_recv() {
                Ok(Ok(reply)) => Some(reply),
                Ok(Err(error)) => {
                    *self = Self::Ready(Some(Err(error)));
                    None
                }
                Err(oneshot::error::TryRecvError::Empty) => None,
                Err(oneshot::error::TryRecvError::Closed) => {
                    *self = Self::Ready(Some(Err("Chamber host stopped".into())));
                    None
                }
            },
            Self::Ready(result) => {
                if result.as_ref().is_some_and(Result::is_ok) {
                    result.take().and_then(Result::ok)
                } else {
                    None
                }
            }
            Self::RateLimited(request_id) => {
                let mut response = last.clone();
                response.request_id = *request_id;
                response.body = Reply::Refused {
                    code: "rate_limited".into(),
                    message: "Chamber request work budget exceeded; no operation was admitted"
                        .into(),
                };
                match response.encode() {
                    Ok(bytes) => Some((bytes, true)),
                    Err(error) => {
                        *self = Self::Ready(Some(Err(error)));
                        None
                    }
                }
            }
        }
    }
    async fn receive(&mut self, last: &Response) -> DispatchReply {
        match self {
            Self::Host(receive) => receive.await.map_err(|_| "Chamber host stopped")?,
            Self::Ready(result) => result.take().ok_or("Chamber reply was already delivered")?,
            Self::RateLimited(request_id) => {
                let mut response = last.clone();
                response.request_id = *request_id;
                response.body = Reply::Refused {
                    code: "rate_limited".into(),
                    message: "Chamber request work budget exceeded; no operation was admitted"
                        .into(),
                };
                response.encode().map(|bytes| (bytes, true))
            }
        }
    }
}
struct Gate {
    bytes: Vec<u8>,
    wait: bool,
    deadline: tokio::time::Instant,
    progress: oneshot::Receiver<RequestProgress>,
    receive: oneshot::Receiver<DispatchReply>,
}
async fn submit(
    send: &mpsc::Sender<Event>,
    id: ConnectionId,
    bytes: Vec<u8>,
    wait: bool,
    deadline: tokio::time::Instant,
    delivered_prefix: Option<Response>,
) -> Result<Gate, String> {
    let (reply, receive) = oneshot::channel();
    let (progress, admission) = oneshot::channel();
    send.send(Event::Request {
        id,
        bytes: bytes.clone(),
        reply,
        progress: Some(progress),
        delivered_prefix,
        queued_at: Instant::now(),
    })
    .await
    .map_err(|_| "Chamber host stopped")?;
    Ok(Gate {
        bytes,
        wait,
        deadline,
        progress: admission,
        receive,
    })
}
/// Local pre-admission refusals describe the latest delivered durable prefix.
/// They cannot regress a header behind earlier pipelined acknowledgments.
fn refusal_prefix(bytes: Vec<u8>, last: &Response) -> Result<Vec<u8>, String> {
    let header: ResponseHeader =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid chamber response")?;
    if header.body.kind == "refused" && header.body.code.as_deref() == Some("storage_busy") {
        let mut response: Response =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid chamber response")?;
        response.tick = last.tick;
        response.control = last.control.clone();
        response.encode()
    } else {
        Ok(bytes)
    }
}
pub(super) async fn run<S: Transport + 'static>(
    stream: S,
    guard: &Option<Box<dyn Guard>>,
    send: &mpsc::Sender<Event>,
    slot: &mut admission::Slot,
    id: ConnectionId,
    mut last: Response,
    mut window: Instant,
    mut count: u32,
) -> Result<(), String> {
    let (read, write) = tokio::io::split(stream);
    let mut writer = Some(write);
    let mut writing: Option<WriteFlight<S>> = None;
    let mut writing_count = 0usize;
    // Partial frame reads are owned by this task and are never cancelled by reply polling.
    let mut reader = Reader::new(read);
    let mut pending = VecDeque::<Delivery>::new();
    let mut gate: Option<Gate> = None;
    let mut retry: Option<(Vec<u8>, bool, tokio::time::Instant, tokio::time::Instant)> = None;
    let mut terminal: Option<String> = None;
    loop {
        if terminal.is_some()
            && pending.is_empty()
            && gate.is_none()
            && retry.is_none()
            && writing.is_none()
        {
            return Err(terminal.unwrap());
        }
        tokio::select! {
            biased;
            _ = slot.cancelled() => return Err("Chamber connection was superseded".into()),
            completed = async { writing.as_mut().unwrap().await }, if writing.is_some() => {
                let (write, delivered) = completed?;
                writing = None;
                writing_count = 0;
                writer = Some(write);
                for (bytes, response, authenticated) in delivered {
                    slot.delivered(bytes, Some(response.tick));
                    last = response;
                    if !authenticated { return Err("Chamber connection is not authenticated".into()); }
                }
            }
            progress = async { (&mut gate.as_mut().unwrap().progress).await }, if gate.is_some() => {
                let current = gate.take().unwrap();
                match progress.map_err(|_| "Chamber host stopped")? {
                    RequestProgress::Queued { authenticated } => {
                        pending.push_back(Delivery::Host(current.receive));
                        if !authenticated { terminal = Some("Chamber connection is not authenticated".into()); }
                    }
                    RequestProgress::Failed(error) => return Err(error),
                    RequestProgress::Busy => {
                        let result = current.receive.await.map_err(|_| "Chamber host stopped")?;
                        let repeat = match &result {
                            Ok((bytes, authenticated)) => {
                                let header: ResponseHeader = serde_json::from_slice(bytes).map_err(|_| "Invalid chamber response")?;
                                current.wait && *authenticated && header.body.kind == "refused"
                                    && header.body.code.as_deref() == Some("storage_busy")
                                    && tokio::time::Instant::now() < current.deadline
                            }
                            Err(_) => false,
                        };
                        if repeat {
                            retry = Some((current.bytes, current.wait, current.deadline,
                                tokio::time::Instant::now() + Duration::from_millis(33)));
                        } else {
                            pending.push_back(Delivery::Ready(Some(result)));
                        }
                    }
                }
            }
            _ = async { tokio::time::sleep_until(retry.as_ref().unwrap().3).await }, if retry.is_some() => {
                let (bytes, wait, deadline, _) = retry.take().unwrap();
                gate = Some(submit(send, id, bytes, wait, deadline, None).await?);
            }
            result = async { pending.front_mut().unwrap().receive(&last).await }, if !pending.is_empty() && writing.is_none() => {
                let (bytes, mut authenticated) = result?;
                pending.pop_front();
                let bytes = refusal_prefix(bytes, &last)?;
                let response: ResponseHeader = serde_json::from_slice(&bytes).map_err(|_| "Invalid chamber response")?;
                if !authenticated {
                    terminal = Some("Chamber connection is not authenticated".into());
                }
                let mut prefix = response.refusal_template();
                let mut frames = vec![bytes];
                let mut delivered = vec![(frames[0].len(), prefix.clone(), authenticated)];
                // A durable fence can release several replies together. Preserve
                // every correlated frame while flushing their ready prefix once.
                while authenticated && frames.len() < CAPACITY {
                    let Some((bytes, next_authenticated)) = pending.front_mut()
                        .and_then(|reply| reply.take_ready(&prefix)) else { break; };
                    pending.pop_front();
                    let bytes = refusal_prefix(bytes, &prefix)?;
                    let response: ResponseHeader = serde_json::from_slice(&bytes)
                        .map_err(|_| "Invalid chamber response")?;
                    prefix = response.refusal_template();
                    authenticated = next_authenticated;
                    if !authenticated { terminal = Some("Chamber connection is not authenticated".into()); }
                    delivered.push((bytes.len(), prefix.clone(), authenticated));
                    frames.push(bytes);
                }
                writing_count = frames.len();
                let mut write = writer.take().expect("Idle chamber reply writer");
                // Keep the partial write future alive across admission polls. A
                // slow reader cannot stall inbound movement or duplicate a frame.
                writing = Some(Box::pin(async move {
                    timeout(WRITE, write_frame_batch(&mut write, &frames, MAX_RESPONSE_BYTES))
                        .await.map_err(|_| "Chamber write timed out")??;
                    Ok((write, delivered))
                }));
            }
            incoming = reader.receive.recv(), if terminal.is_none() && gate.is_none()
                && retry.is_none() && pending.len() + writing_count < CAPACITY => {
                let bytes = match incoming {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(error)) => { terminal = Some(error); continue; }
                    None => { terminal = Some("Chamber request reader stopped".into()); continue; }
                };
                slot.received(bytes.len());
                if window.elapsed() >= Duration::from_secs(1) { window = Instant::now(); count = 0; }
                count += 1;
                if count > REQUESTS_PER_SECOND { return Err("Chamber request rate exceeded".into()); }
                if let Some(guard) = guard { guard.admit(&bytes)?; }
                let request = Request::decode(&bytes)?;
                if !slot.request(&request.body) {
                    pending.push_back(Delivery::RateLimited(request.request_id));
                    continue;
                }
                let delivered_prefix = (matches!(request.body, Body::MovementCredit {})
                    && pending.is_empty() && writing.is_none()).then(|| last.clone());
                let wait = movement_storage_wait(true, &request.body);
                gate = Some(submit(send, id, bytes, wait,
                    tokio::time::Instant::now() + Duration::from_secs(2), delivered_prefix).await?);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(request_id: u64, tick: u64, sequence: u64) -> Response {
        Response {
            version: VERSION,
            request_id,
            instance: 1,
            tick,
            control: Some(Control {
                credit_step: 0,
                world_step: 0,
                life: super::super::super::wire::Life {
                    instance: 1,
                    actor: 1,
                    generation: 1,
                },
                epoch: 1,
                accepted_sequence: sequence,
                applied_movement: None,
                dynamic: Vec::new(),
            }),
            body: Reply::Refused {
                code: "storage_busy".into(),
                message: "No operation was admitted".into(),
            },
        }
    }

    #[test]
    fn storage_refusal_preserves_the_delivered_control_prefix_at_the_same_tick() {
        let last = response(8, 12, 7);
        for tick in [11, 12, 13] {
            let bytes = refusal_prefix(response(9, tick, 0).encode().unwrap(), &last).unwrap();
            let result: Response = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(result.request_id, 9);
            assert_eq!(result.tick, 12);
            assert_eq!(result.control.unwrap().accepted_sequence, 7);
        }
    }

    // A repetitive message compresses below the socket capacity and cannot
    // exercise a blocked write. Fixed-seed varied bytes keep that boundary real.
    fn blocked_write_payload() -> String {
        let mut seed = 0x1234_5678u32;
        std::iter::repeat_with(|| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            char::from(b'!' + (seed % 90) as u8)
        })
        .take(16 * 1024)
        .collect()
    }

    #[tokio::test]
    async fn ready_reply_collection_preserves_unready_and_failed_boundaries() {
        let last = response(8, 12, 7);
        let (send, receive) = oneshot::channel();
        let mut reply = Delivery::Host(receive);
        assert!(reply.take_ready(&last).is_none());
        send.send(Err("Durability failed".into())).unwrap();
        assert!(reply.take_ready(&last).is_none());
        assert_eq!(reply.receive(&last).await.unwrap_err(), "Durability failed");
        let (send, receive) = oneshot::channel();
        let mut reply = Delivery::Host(receive);
        drop(send);
        assert!(reply.take_ready(&last).is_none());
        assert_eq!(
            reply.receive(&last).await.unwrap_err(),
            "Chamber host stopped"
        );
        let mut reply = Delivery::RateLimited(9);
        let (bytes, authenticated) = reply.take_ready(&last).unwrap();
        let decoded: Response = serde_json::from_slice(&bytes).unwrap();
        assert!(authenticated);
        assert_eq!((decoded.request_id, decoded.tick), (9, 12));
        assert_eq!(decoded.control.unwrap().accepted_sequence, 7);
    }

    #[tokio::test]
    async fn credit_fast_read_requires_an_empty_delivery_prefix() {
        let (server, mut client) = tokio::io::duplex(65536);
        let (send, mut events) = mpsc::channel(32);
        let limits = admission::Limits::new();
        let mut slot = limits.open("127.0.0.1".parse().unwrap()).unwrap();
        slot.authenticate([1; 32]).unwrap();
        let keys = [
            super::super::tests::key(1),
            super::super::tests::key(2),
            super::super::tests::key(3),
        ];
        let (id, _) = super::super::tests::gateway(&keys).open(0).unwrap();
        let task = tokio::spawn(async move {
            run(
                server,
                &None,
                &send,
                &mut slot,
                id,
                response(0, 1, 0),
                Instant::now(),
                0,
            )
            .await
        });
        let mut replies = Vec::new();
        for request_id in 1..=2 {
            let bytes = serde_json::to_vec(&Request {
                version: VERSION,
                request_id,
                body: Body::MovementCredit {},
            })
            .unwrap();
            write_frame(&mut client, &bytes, MAX_REQUEST_BYTES)
                .await
                .unwrap();
            let Event::Request {
                reply,
                progress,
                delivered_prefix,
                ..
            } = timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap()
                .unwrap()
            else {
                panic!("Expected credit request")
            };
            assert_eq!(delivered_prefix.is_some(), request_id == 1);
            if let Some(prefix) = delivered_prefix {
                assert_eq!(prefix.tick, 1);
            }
            progress
                .unwrap()
                .send(RequestProgress::Queued {
                    authenticated: true,
                })
                .ok()
                .unwrap();
            replies.push(reply);
        }
        for (index, reply) in replies.into_iter().enumerate() {
            let mut result = response(index as u64 + 1, index as u64 + 2, 0);
            result.body = Reply::Accepted;
            reply.send(Ok((result.encode().unwrap(), true))).unwrap();
        }
        for request_id in 1..=2 {
            let bytes = timeout(
                Duration::from_secs(1),
                read_frame(&mut client, MAX_RESPONSE_BYTES),
            )
            .await
            .unwrap()
            .unwrap();
            let result: Response = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(result.request_id, request_id);
        }
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id: 3,
            body: Body::MovementCredit {},
        })
        .unwrap();
        write_frame(&mut client, &bytes, MAX_REQUEST_BYTES)
            .await
            .unwrap();
        let Event::Request {
            delivered_prefix, ..
        } = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("Expected credit request")
        };
        assert_eq!(delivered_prefix.unwrap().tick, 3);
        task.abort();
    }

    #[tokio::test]
    async fn admission_is_bounded_and_replies_wait_for_ordered_durability() {
        let (server, mut client) = tokio::io::duplex(65536);
        let (send, mut events) = mpsc::channel(32);
        let limits = admission::Limits::new();
        let mut slot = limits.open("127.0.0.1".parse().unwrap()).unwrap();
        slot.authenticate([1; 32]).unwrap();
        let keys = [
            super::super::tests::key(1),
            super::super::tests::key(2),
            super::super::tests::key(3),
        ];
        let (id, _) = super::super::tests::gateway(&keys).open(0).unwrap();
        let task = tokio::spawn(async move {
            run(
                server,
                &None,
                &send,
                &mut slot,
                id,
                response(0, 1, 0),
                Instant::now(),
                0,
            )
            .await
        });
        for request_id in 1..=9 {
            let bytes = serde_json::to_vec(&Request {
                version: VERSION,
                request_id,
                body: Body::Snapshot {},
            })
            .unwrap();
            write_frame(&mut client, &bytes, MAX_REQUEST_BYTES)
                .await
                .unwrap();
        }
        let mut replies = Vec::new();
        for request_id in 1..=8 {
            let Event::Request {
                bytes,
                reply,
                progress,
                ..
            } = timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap()
                .unwrap()
            else {
                panic!("Expected request")
            };
            assert_eq!(Request::decode(&bytes).unwrap().request_id, request_id);
            progress
                .unwrap()
                .send(RequestProgress::Queued {
                    authenticated: true,
                })
                .unwrap_or_else(|_| panic!("Admission receiver stopped"));
            replies.push(reply);
        }
        assert!(
            timeout(Duration::from_millis(30), events.recv())
                .await
                .is_err()
        );
        assert!(
            timeout(
                Duration::from_millis(30),
                read_frame(&mut client, MAX_RESPONSE_BYTES)
            )
            .await
            .is_err()
        );
        // Completing later fences cannot deliver ahead of the first fence.
        for (index, reply) in replies.drain(1..).enumerate() {
            reply
                .send(Ok((
                    response(index as u64 + 2, 1, 0).encode().unwrap(),
                    true,
                )))
                .unwrap();
        }
        assert!(
            timeout(
                Duration::from_millis(30),
                read_frame(&mut client, MAX_RESPONSE_BYTES)
            )
            .await
            .is_err()
        );
        replies
            .pop()
            .unwrap()
            .send(Ok((response(1, 1, 0).encode().unwrap(), true)))
            .unwrap();
        for request_id in 1..=8 {
            let bytes = timeout(
                Duration::from_secs(1),
                read_frame(&mut client, MAX_RESPONSE_BYTES),
            )
            .await
            .unwrap()
            .unwrap();
            let result: Response = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(result.request_id, request_id);
        }
        let Event::Request { bytes, .. } = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("Expected request")
        };
        assert_eq!(Request::decode(&bytes).unwrap().request_id, 9);
        task.abort();
    }
    #[tokio::test]
    async fn blocked_reply_write_keeps_movement_admission_live_and_bounded() {
        let (server, client) = tokio::io::duplex(1024);
        let (mut read, mut write) = tokio::io::split(client);
        let (send, mut events) = mpsc::channel(32);
        let limits = admission::Limits::new();
        let mut slot = limits.open("127.0.0.1".parse().unwrap()).unwrap();
        slot.authenticate([1; 32]).unwrap();
        let keys = [
            super::super::tests::key(1),
            super::super::tests::key(2),
            super::super::tests::key(3),
        ];
        let (id, _) = super::super::tests::gateway(&keys).open(0).unwrap();
        let task = tokio::spawn(async move {
            run(
                server,
                &None,
                &send,
                &mut slot,
                id,
                response(0, 1, 0),
                Instant::now(),
                0,
            )
            .await
        });
        let request = |request_id, body| {
            serde_json::to_vec(&Request {
                version: VERSION,
                request_id,
                body,
            })
            .unwrap()
        };
        write_frame(
            &mut write,
            &request(1, Body::Snapshot {}),
            MAX_REQUEST_BYTES,
        )
        .await
        .unwrap();
        let Event::Request {
            reply, progress, ..
        } = events.recv().await.unwrap()
        else {
            panic!("Missing initial admission")
        };
        progress
            .unwrap()
            .send(RequestProgress::Queued {
                authenticated: true,
            })
            .unwrap_or_else(|_| panic!("Admission receiver stopped"));
        let mut large = response(1, 1, 0);
        large.body = Reply::Refused {
            code: "fixture".into(),
            message: blocked_write_payload(),
        };
        let large = large.encode().unwrap();
        reply.send(Ok((large.clone(), true))).unwrap();
        // No response bytes are consumed while the server's bounded socket fills.
        tokio::time::sleep(Duration::from_millis(40)).await;
        for request_id in 2..=10 {
            let frame = crate::movement::frames::Frame {
                life: verse_engine::core::LifeId {
                    instance: 1,
                    actor: 1,
                    generation: 1,
                },
                epoch: 1,
                sequence: request_id,
                tick: 1,
                start: (request_id - 2) * 4,
                steps: 4,
                segments: vec![crate::movement::frames::Segment {
                    offset: 0,
                    axes: [0.; 2],
                    yaw: 0.,
                    until: (request_id - 2) * 4 + crate::movement::HELD_STEPS,
                    jump: false,
                }],
            };
            timeout(
                Duration::from_secs(1),
                write_frame(
                    &mut write,
                    &request(request_id, Body::MovementFrame { frame }),
                    MAX_REQUEST_BYTES,
                ),
            )
            .await
            .unwrap()
            .unwrap();
        }
        for request_id in 2..=8 {
            let event = timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("Blocked reply write stopped movement admission")
                .unwrap();
            let Event::Request {
                bytes,
                reply,
                progress,
                ..
            } = event
            else {
                panic!("Missing movement admission")
            };
            let request = Request::decode(&bytes).unwrap();
            assert_eq!(request.request_id, request_id);
            assert!(matches!(request.body, Body::MovementFrame { .. }));
            progress
                .unwrap()
                .send(RequestProgress::Queued {
                    authenticated: true,
                })
                .unwrap_or_else(|_| panic!("Admission receiver stopped"));
            reply
                .send(Ok((
                    response(request_id, 1, request_id).encode().unwrap(),
                    true,
                )))
                .unwrap();
        }
        assert!(
            timeout(Duration::from_millis(30), events.recv())
                .await
                .is_err(),
            "Blocked write bypassed the eight-request bound"
        );
        // The partially written first frame must resume without duplicated bytes.
        assert_eq!(
            read_frame(&mut read, MAX_RESPONSE_BYTES).await.unwrap(),
            large
        );
        // Seven ready replies now share a blocked write. Only its one free
        // request slot can admit more work, regardless of the number of flushes.
        let Event::Request {
            bytes,
            reply,
            progress,
            ..
        } = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("Missing single free request slot");
        };
        assert_eq!(Request::decode(&bytes).unwrap().request_id, 9);
        progress
            .unwrap()
            .send(RequestProgress::Queued {
                authenticated: true,
            })
            .unwrap_or_else(|_| panic!("Admission receiver stopped"));
        reply
            .send(Ok((response(9, 1, 9).encode().unwrap(), true)))
            .unwrap();
        assert!(
            timeout(Duration::from_millis(30), events.recv())
                .await
                .is_err(),
            "A blocked reply batch bypassed the eight-request bound"
        );
        for request_id in 2..=9 {
            let bytes = timeout(
                Duration::from_secs(1),
                read_frame(&mut read, MAX_RESPONSE_BYTES),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(
                serde_json::from_slice::<Response>(&bytes)
                    .unwrap()
                    .request_id,
                request_id
            );
        }
        let Event::Request { bytes, .. } = timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("Missing released admission")
        };
        assert_eq!(Request::decode(&bytes).unwrap().request_id, 10);
        task.abort();
        let _ = task.await;
    }
    #[tokio::test]
    async fn authentication_loss_stops_admission_during_a_blocked_reply_write() {
        let (server, client) = tokio::io::duplex(1024);
        let (mut read, mut write) = tokio::io::split(client);
        let (send, mut events) = mpsc::channel(32);
        let limits = admission::Limits::new();
        let mut slot = limits.open("127.0.0.1".parse().unwrap()).unwrap();
        slot.authenticate([1; 32]).unwrap();
        let keys = [
            super::super::tests::key(1),
            super::super::tests::key(2),
            super::super::tests::key(3),
        ];
        let (id, _) = super::super::tests::gateway(&keys).open(0).unwrap();
        let task = tokio::spawn(async move {
            run(
                server,
                &None,
                &send,
                &mut slot,
                id,
                response(0, 1, 0),
                Instant::now(),
                0,
            )
            .await
        });
        let request = |request_id| {
            serde_json::to_vec(&Request {
                version: VERSION,
                request_id,
                body: Body::Snapshot {},
            })
            .unwrap()
        };
        write_frame(&mut write, &request(1), MAX_REQUEST_BYTES)
            .await
            .unwrap();
        let Event::Request {
            reply, progress, ..
        } = events.recv().await.unwrap()
        else {
            panic!("Missing initial admission")
        };
        progress
            .unwrap()
            .send(RequestProgress::Busy)
            .unwrap_or_else(|_| panic!("Admission receiver stopped"));
        let mut denied = response(1, 1, 0);
        denied.control = None;
        denied.body = Reply::Refused {
            code: "fixture".into(),
            message: blocked_write_payload(),
        };
        let denied = denied.encode().unwrap();
        reply.send(Ok((denied.clone(), false))).unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
        write_frame(&mut write, &request(2), MAX_REQUEST_BYTES)
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_millis(40), events.recv())
                .await
                .is_err(),
            "Admission continued after authentication loss"
        );
        assert_eq!(
            read_frame(&mut read, MAX_RESPONSE_BYTES).await.unwrap(),
            denied
        );
        let result = timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.unwrap_err(),
            "Chamber connection is not authenticated"
        );
        assert!(events.recv().await.is_none());
    }
}
