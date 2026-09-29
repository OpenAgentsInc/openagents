//! One QUIC bidirectional stream as an ordered byte stream.
//!
//! NIP-REACH runs "over any ordered byte stream"; [`IrohStream`] joins a
//! stream's two halves into one `AsyncRead + AsyncWrite` value and keeps its
//! connection alive while it lives.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use iroh::EndpointId;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// How long a dropped stream keeps its connection open so the peer can
/// receive the last bytes written, such as a close or refusal frame.
pub const LINGER: Duration = Duration::from_secs(5);

/// A QUIC bidirectional stream and the connection that carries it.
pub struct IrohStream {
    connection: Connection,
    send: Option<SendStream>,
    recv: RecvStream,
}

impl std::fmt::Debug for IrohStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IrohStream")
            .field("remote", &self.connection.remote_id())
            .finish_non_exhaustive()
    }
}

impl IrohStream {
    /// Open a stream as the dialing side.
    ///
    /// # Errors
    /// Reports a closed connection.
    pub async fn open(connection: Connection) -> io::Result<Self> {
        let (send, recv) = connection.open_bi().await.map_err(io::Error::other)?;
        Ok(Self {
            connection,
            send: Some(send),
            recv,
        })
    }

    /// Accept the peer's first stream as the answering side.
    ///
    /// # Errors
    /// Reports a closed connection.
    pub async fn accept(connection: Connection) -> io::Result<Self> {
        let (send, recv) = connection.accept_bi().await.map_err(io::Error::other)?;
        Ok(Self {
            connection,
            send: Some(send),
            recv,
        })
    }

    /// The iroh key of the peer, proven by the QUIC handshake. It identifies
    /// a route, never a principal.
    #[must_use]
    pub fn remote_id(&self) -> EndpointId {
        self.connection.remote_id()
    }

    /// The connection, for closing it or reading its paths.
    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

impl AsyncRead for IrohStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for IrohStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.send.as_mut() {
            Some(send) => AsyncWrite::poll_write(Pin::new(send), cx, buf),
            None => Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.send.as_mut() {
            Some(send) => Pin::new(send).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.send.as_mut() {
            Some(send) => Pin::new(send).poll_shutdown(cx),
            None => Poll::Ready(Ok(())),
        }
    }
}

impl Drop for IrohStream {
    fn drop(&mut self) {
        let Some(mut send) = self.send.take() else {
            return;
        };
        // Dropping the last handle closes the connection at once, which can
        // lose bytes still in flight. Finish the stream and hold the
        // connection until the peer acknowledges them, or for `LINGER`.
        let _ = send.finish();
        let stopped = send.stopped();
        let connection = self.connection.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = tokio::time::timeout(LINGER, stopped).await;
                drop(send);
                drop(connection);
            });
        }
    }
}
