//! The WebSocket mapping of the direct channel.
//!
//! [`WebSocket`] turns a WebSocket connection into the ordered byte stream the
//! handshake and [`Channel`](crate::channel::Channel) read and write, so TCP
//! and WebSocket share one handshake, one frame format, one sequence rule,
//! and one set of bounds. The mapping adds only message rules:
//!
//! - Each binary message carries exactly one frame, length prefix included.
//!   A message whose prefix disagrees with its size refuses as `malformed`.
//! - A message over [`MAX_MESSAGE_BYTES`] refuses as `limit_exceeded`. The
//!   bound applies to each WebSocket frame header, so an oversized message
//!   refuses before its payload is read.
//! - A text message refuses as `malformed`. Ping and pong messages carry no
//!   channel data.
//! - A WebSocket close ends the stream. Only a channel close frame, which is
//!   encrypted and sequenced, proves that the peer closed the channel.
//!
//! The mapping neither offers nor accepts a subprotocol or an extension.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use futures_util::{Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self, Bytes, Message};

use crate::channel::MAX_FRAME_BYTES;
use crate::{Error, Refusal, Result};

/// Largest binary message: one frame and its four-byte length prefix.
pub use crate::websocket_frame::{MAX_MESSAGE_BYTES, check_message};

/// The WebSocket limits a direct channel runs with: no message or frame
/// larger than [`MAX_MESSAGE_BYTES`].
#[must_use]
pub fn config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES))
}

/// Answer a WebSocket upgrade on an accepted connection. The host serves
/// every path; the hint URL names the one its clients request.
///
/// # Errors
/// Reports a failed upgrade as `unavailable`.
pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(stream: S) -> Result<WebSocket<S>> {
    tokio_tungstenite::accept_async_with_config(stream, Some(config()))
        .await
        .map(WebSocket::new)
        .map_err(|_| Error::new(Refusal::Unavailable, "websocket upgrade failed"))
}

/// Upgrade an open connection to `url`, a `ws` or `wss` URL from a hint.
/// A `wss` URL needs a TLS stream the caller already opened.
///
/// # Errors
/// Reports a malformed URL as `malformed` and a failed upgrade as
/// `unavailable`.
pub async fn client<S: AsyncRead + AsyncWrite + Unpin>(
    url: &str,
    stream: S,
) -> Result<WebSocket<S>> {
    let parsed = url::Url::parse(url).map_err(|_| Error::new(Refusal::Malformed, "URL"))?;
    if !matches!(parsed.scheme(), "ws" | "wss") {
        return Err(Error::new(Refusal::Malformed, "websocket URL scheme"));
    }
    tokio_tungstenite::client_async_with_config(url, stream, Some(config()))
        .await
        .map(|(socket, _)| WebSocket::new(socket))
        .map_err(|_| Error::new(Refusal::Unavailable, "websocket upgrade failed"))
}

/// A WebSocket connection seen as the direct channel's byte stream.
pub struct WebSocket<S> {
    inner: WebSocketStream<S>,
    /// The unread rest of the current message.
    incoming: Bytes,
    /// Written bytes not yet sent: at most one partial frame after a send.
    outgoing: Vec<u8>,
    ended: bool,
}

impl<S> std::fmt::Debug for WebSocket<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocket")
            .field("incoming", &self.incoming.len())
            .field("outgoing", &self.outgoing.len())
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> WebSocket<S> {
    /// Wrap an upgraded connection. Configure it with [`config`] so
    /// oversized messages refuse before their payload is read.
    #[must_use]
    pub fn new(inner: WebSocketStream<S>) -> Self {
        Self {
            inner,
            incoming: Bytes::new(),
            outgoing: Vec::new(),
            ended: false,
        }
    }

    /// Hand each complete buffered frame to the socket as its own binary
    /// message.
    fn poll_send(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while let Some(len) = complete_frame(&self.outgoing)? {
            ready!(Pin::new(&mut self.inner).poll_ready(cx)).map_err(socket_error)?;
            let message: Vec<u8> = self.outgoing.drain(..len).collect();
            Pin::new(&mut self.inner)
                .start_send(Message::Binary(message.into()))
                .map_err(socket_error)?;
        }
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for WebSocket<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if !this.incoming.is_empty() {
                let n = buf.remaining().min(this.incoming.len());
                buf.put_slice(&this.incoming.split_to(n));
                return Poll::Ready(Ok(()));
            }
            if this.ended {
                return Poll::Ready(Ok(()));
            }
            match ready!(Pin::new(&mut this.inner).poll_next(cx)) {
                None | Some(Ok(Message::Close(_))) => this.ended = true,
                Some(Ok(Message::Binary(message))) => {
                    check_message(&message).map_err(into_io)?;
                    this.incoming = message;
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Text(_) | Message::Frame(_))) => {
                    return Poll::Ready(Err(refusal(
                        Refusal::Malformed,
                        "a direct channel carries only binary messages",
                    )));
                }
                Some(Err(error)) => return Poll::Ready(Err(socket_error(error))),
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for WebSocket<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        ready!(this.poll_send(cx))?;
        this.outgoing.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        ready!(this.poll_send(cx))?;
        Pin::new(&mut this.inner)
            .poll_flush(cx)
            .map_err(socket_error)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        ready!(this.poll_send(cx))?;
        // A partial frame can never be completed now; it is not sent.
        this.outgoing.clear();
        Pin::new(&mut this.inner)
            .poll_close(cx)
            .map_err(socket_error)
    }
}

/// The size of the first complete frame in `buffer`, if there is one.
fn complete_frame(buffer: &[u8]) -> io::Result<Option<usize>> {
    let Some(prefix) = buffer.first_chunk::<4>() else {
        return Ok(None);
    };
    let len = u32::from_be_bytes(*prefix) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(refusal(
            Refusal::LimitExceeded,
            "frame exceeds the maximum size",
        ));
    }
    Ok((buffer.len() >= 4 + len).then_some(4 + len))
}

fn into_io(error: Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn refusal(code: Refusal, detail: &'static str) -> io::Error {
    into_io(Error::new(code, detail))
}

fn socket_error(error: tungstenite::Error) -> io::Error {
    match error {
        tungstenite::Error::Capacity(_) => {
            refusal(Refusal::LimitExceeded, "message exceeds the frame bound")
        }
        tungstenite::Error::Protocol(_) | tungstenite::Error::Utf8(_) => {
            refusal(Refusal::Malformed, "websocket protocol violation")
        }
        tungstenite::Error::Io(error) => error,
        _ => io::Error::new(io::ErrorKind::BrokenPipe, "websocket closed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_must_be_exactly_one_bounded_frame() {
        let frame = |len: u32, body: usize| {
            let mut bytes = len.to_be_bytes().to_vec();
            bytes.extend(std::iter::repeat_n(0, body));
            bytes
        };
        assert!(check_message(&frame(9, 9)).is_ok());
        let code = |bytes: &[u8]| check_message(bytes).err().map(|e| e.code);
        assert_eq!(code(&frame(9, 18)), Some(Refusal::Malformed));
        assert_eq!(code(&frame(9, 5)), Some(Refusal::Malformed));
        assert_eq!(code(&frame(3, 3)), Some(Refusal::Malformed));
        assert_eq!(code(&[0, 0]), Some(Refusal::Malformed));
        let over = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
        assert_eq!(code(&frame(over, 0)), Some(Refusal::LimitExceeded));
        assert_eq!(
            code(&vec![0; MAX_MESSAGE_BYTES + 1]),
            Some(Refusal::LimitExceeded)
        );
    }

    #[test]
    fn buffered_writes_split_on_frame_boundaries() {
        let mut buffer = 9u32.to_be_bytes().to_vec();
        buffer.extend([0; 9]);
        buffer.extend(10u32.to_be_bytes());
        assert_eq!(complete_frame(&buffer).unwrap(), Some(13));
        assert_eq!(complete_frame(&buffer[13..]).unwrap(), None);
        assert_eq!(complete_frame(&[0, 0]).unwrap(), None);
    }
}
