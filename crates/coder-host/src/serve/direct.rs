//! Direct channels: accept, authenticate, recheck, and serve.
//!
//! `coder-reach` proves both keys and checks the grant before any data
//! flows. After that, the host rechecks the channel's grant before every
//! message and on a timer, and closes the channel with a closing message
//! naming the code as soon as the grant stops admitting it. Opening a
//! channel grants no right: each NIP-HOST request is admitted by
//! `coder-access`, and each terminal operation by `coder-pty`.

use std::sync::Arc;

use coder_pty::host::{FrameSink, SinkError};
use coder_pty::wire::{Frame, TerminalResult};
use coder_reach::channel::{Acceptor, Binding, GrantRefusal};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use super::{Shared, relay::host_request, terminal};
use crate::authority::Grants;
use crate::message::{Assembler, DecodeError, ToDevice, ToHost, fragments};
use crate::unix_time;

/// Messages queued for one channel before frames are refused as full.
const OUTBOUND: usize = 512;

pub(super) async fn listen(
    shared: Arc<Shared>,
    listener: TcpListener,
    acceptor: Arc<Acceptor<Grants>>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        tokio::spawn(session(shared.clone(), acceptor.clone(), stream));
    }
}

/// Serve one direct channel over any ordered byte stream: a TCP connection,
/// or a WebSocket connection seen through `coder_reach::websocket`.
pub(super) async fn session<S>(shared: Arc<Shared>, acceptor: Arc<Acceptor<Grants>>, stream: S)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let Ok(now) = unix_time() else { return };
    let Ok(channel) = acceptor.accept(stream, now).await else {
        return;
    };
    let binding = channel.binding().clone();
    let (mut reader, mut writer) = channel.into_split();

    let (outbound, mut queue) = mpsc::channel::<ToDevice>(OUTBOUND);
    // Terminal attachments hold senders of their own, so the queue never
    // closes by itself; the session stops the writer explicitly.
    let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
    let writer_task = tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                message = queue.recv() => match message {
                    Some(message) => message,
                    None => break,
                },
                _ = &mut stopped => break,
            };
            let closing = matches!(message, ToDevice::Closing(_));
            let Ok(parts) = fragments(&message.encode()) else {
                continue;
            };
            for part in parts {
                if writer.send(&part).await.is_err() {
                    return;
                }
            }
            if closing {
                break;
            }
        }
        let _ = writer.close().await;
    });

    // The reader runs alone: a data frame read is not cancellation safe.
    let (inbound, mut messages) = mpsc::channel::<Vec<u8>>(64);
    let reader_task = tokio::spawn(async move {
        let mut assembler = Assembler::default();
        while let Ok(Some(payload)) = reader.recv().await {
            match assembler.push(&payload) {
                Ok(Some(message)) => {
                    if inbound.send(message).await.is_err() {
                        return;
                    }
                }
                Ok(None) => {}
                Err(_) => return,
            }
        }
    });

    // The host records when it last saw the device: at admission, then at
    // most once per resolution interval while messages arrive.
    let seen_every = std::time::Duration::from_secs(coder_access::host::SEEN_RESOLUTION);
    shared.authority.touch(&binding.client, &binding.grant);
    let mut seen = std::time::Instant::now();
    let mut ticker = tokio::time::interval(shared.config.recheck_every);
    loop {
        let next = tokio::select! {
            message = messages.recv() => match message {
                Some(message) => Some(message),
                None => break,
            },
            _ = ticker.tick() => None,
        };
        if let Err(code) = recheck(&shared, &binding) {
            let _ = outbound.send(ToDevice::Closing(code.into())).await;
            break;
        }
        if let Some(bytes) = next {
            if seen.elapsed() >= seen_every {
                shared.authority.touch(&binding.client, &binding.grant);
                seen = std::time::Instant::now();
            }
            serve_message(&shared, &binding, &outbound, &bytes);
        }
    }
    reader_task.abort();
    drop(outbound);
    let _ = stop.send(());
    let _ = writer_task.await;
}

/// The channel's grant, at its epoch, must still admit it.
fn recheck(shared: &Shared, binding: &Binding) -> Result<(), &'static str> {
    let now = unix_time().map_err(|_| "unavailable")?;
    shared
        .authority
        .check(&binding.client, &binding.grant, binding.epoch, now)
        .map(|_| ())
        .map_err(|refusal| match refusal {
            GrantRefusal::Unknown => "not_admitted",
            GrantRefusal::Revoked => "revoked",
            GrantRefusal::EpochMismatch | GrantRefusal::Expired => "stale",
        })
}

fn serve_message(
    shared: &Arc<Shared>,
    binding: &Binding,
    outbound: &mpsc::Sender<ToDevice>,
    bytes: &[u8],
) {
    match ToHost::decode(bytes) {
        Ok(ToHost::Ping(nonce)) => {
            let _ = outbound.try_send(ToDevice::Pong(nonce));
        }
        Ok(ToHost::Call(event)) => {
            // A channel carries only its own device's requests.
            if event.pubkey != binding.client {
                return;
            }
            let (shared, outbound) = (shared.clone(), outbound.clone());
            tokio::spawn(async move {
                let Ok(relay) = shared.config.primary().map(str::to_owned) else {
                    return;
                };
                if let Some(reply) = host_request(&shared, event, &relay).await {
                    let _ = outbound.send(ToDevice::Answer(reply)).await;
                }
            });
        }
        Ok(ToHost::Terminal(request)) => {
            let (shared, outbound) = (shared.clone(), outbound.clone());
            let principal = binding.client.clone();
            tokio::spawn(async move {
                let sink_queue = outbound.clone();
                let worker = shared.clone();
                let result = tokio::task::spawn_blocking(move || {
                    terminal::run(&worker, &principal, &request, || {
                        Box::new(ChannelSink(sink_queue))
                    })
                })
                .await;
                if let Ok(result) = result {
                    let _ = outbound.send(ToDevice::Result(result)).await;
                }
            });
        }
        Err(DecodeError::Terminal(Some(request), refusal)) => {
            let result = TerminalResult::from_outcome(request, Err(refusal));
            let _ = outbound.try_send(ToDevice::Result(result));
        }
        Err(_) => {}
    }
}

/// Delivers an attachment's frames into the channel's outbound queue.
struct ChannelSink(mpsc::Sender<ToDevice>);

impl FrameSink for ChannelSink {
    fn deliver(&mut self, frame: &Frame) -> Result<(), SinkError> {
        match self.0.try_send(ToDevice::Frame(frame.clone())) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(SinkError::Full),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(SinkError::Closed),
        }
    }
}
