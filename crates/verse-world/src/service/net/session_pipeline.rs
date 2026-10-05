//! Ordered admission proceeds independently of ordered durable reply delivery.
use super::*;
use std::collections::VecDeque;

const CAPACITY: usize = 8;
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
) -> Result<Gate, String> {
    let (reply, receive) = oneshot::channel();
    let (progress, admission) = oneshot::channel();
    send.send(Event::Request {
        id,
        bytes: bytes.clone(),
        reply,
        progress: Some(progress),
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
    let (read, mut write) = tokio::io::split(stream);
    // Partial frame reads are owned by this task and are never cancelled by reply polling.
    let mut reader = Reader::new(read);
    let mut pending = VecDeque::<Delivery>::new();
    let mut gate: Option<Gate> = None;
    let mut retry: Option<(Vec<u8>, bool, tokio::time::Instant, tokio::time::Instant)> = None;
    let mut terminal: Option<String> = None;
    loop {
        if terminal.is_some() && pending.is_empty() && gate.is_none() && retry.is_none() {
            return Err(terminal.unwrap());
        }
        tokio::select! {
            biased;
            _ = slot.cancelled() => return Err("Chamber connection was superseded".into()),
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
                gate = Some(submit(send, id, bytes, wait, deadline).await?);
            }
            result = async { pending.front_mut().unwrap().receive(&last).await }, if !pending.is_empty() => {
                let (bytes, authenticated) = result?;
                pending.pop_front();
                let bytes = refusal_prefix(bytes, &last)?;
                let response: ResponseHeader = serde_json::from_slice(&bytes).map_err(|_| "Invalid chamber response")?;
                timeout(WRITE, write_frame(&mut write, &bytes, MAX_RESPONSE_BYTES))
                    .await.map_err(|_| "Chamber write timed out")??;
                last = response.refusal_template();
                if !authenticated { return Err("Chamber connection is not authenticated".into()); }
            }
            incoming = reader.receive.recv(), if terminal.is_none() && gate.is_none()
                && retry.is_none() && pending.len() < CAPACITY => {
                let bytes = match incoming {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(error)) => { terminal = Some(error); continue; }
                    None => { terminal = Some("Chamber request reader stopped".into()); continue; }
                };
                if window.elapsed() >= Duration::from_secs(1) { window = Instant::now(); count = 0; }
                count += 1;
                if count > REQUESTS_PER_SECOND { return Err("Chamber request rate exceeded".into()); }
                if let Some(guard) = guard { guard.admit(&bytes)?; }
                let request = Request::decode(&bytes)?;
                if !slot.request(&request.body) {
                    pending.push_back(Delivery::RateLimited(request.request_id));
                    continue;
                }
                let wait = movement_storage_wait(true, &request.body);
                gate = Some(submit(send, id, bytes, wait,
                    tokio::time::Instant::now() + Duration::from_secs(2)).await?);
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
                world_step: 0,
                life: super::super::super::wire::Life {
                    instance: 1,
                    actor: 1,
                    generation: 1,
                },
                epoch: 1,
                accepted_sequence: sequence,
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
}
