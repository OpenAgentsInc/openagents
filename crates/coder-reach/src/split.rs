//! The two halves of a split direct channel.
//!
//! A host answers a device's requests while it streams terminal output to the
//! same device, so reading and writing run in separate tasks. Splitting keeps
//! every rule of an unsplit [`Channel`](crate::channel::Channel): each
//! direction has its own session key and sequence number, every frame is
//! authenticated, and a frame out of sequence closes the reader.

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::channel::{
    Binding, FrameKind, MAX_DATA_BYTES, open_frame, read_frame, seal_frame, write_frame,
};
use crate::{Error, Refusal, Result, fail};

/// The receiving half of a split channel.
pub struct ChannelReader<R> {
    stream: R,
    key: [u8; 32],
    seq: u64,
    binding: Binding,
    closed: bool,
}

impl<R> std::fmt::Debug for ChannelReader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Keys stay out of debug output.
        f.debug_struct("ChannelReader")
            .field("binding", &self.binding)
            .field("seq", &self.seq)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl<R: AsyncRead + Unpin> ChannelReader<R> {
    pub(crate) fn new(stream: R, key: [u8; 32], seq: u64, binding: Binding, closed: bool) -> Self {
        Self {
            stream,
            key,
            seq,
            binding,
            closed,
        }
    }

    /// The identities, grant, and generation this channel is bound to.
    #[must_use]
    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    /// Receive the next data frame; `None` after the peer closes.
    ///
    /// This is not cancellation safe: dropping the future part way through a
    /// frame loses that frame, and the next read refuses as out of sequence.
    /// Run it in a task of its own rather than inside a `select!`.
    ///
    /// # Errors
    /// Refuses out-of-order, unauthenticated, oversized, or unexpected frames
    /// and closes this half.
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        if self.closed {
            return Ok(None);
        }
        let result = self.read().await;
        match result {
            Ok((FrameKind::Data, data)) => Ok(Some(data)),
            Ok((FrameKind::Close, _)) => {
                self.closed = true;
                Ok(None)
            }
            Ok(_) => {
                self.closed = true;
                fail(Refusal::Malformed, "unexpected frame kind")
            }
            Err(error) => {
                self.closed = true;
                Err(error)
            }
        }
    }

    async fn read(&mut self) -> Result<(FrameKind, Vec<u8>)> {
        let frame = read_frame(&mut self.stream).await?;
        if frame.seq != self.seq {
            return fail(Refusal::Malformed, "frame out of sequence");
        }
        let data = open_frame(&self.key, &frame)?;
        self.seq += 1;
        Ok((frame.kind, data))
    }
}

/// The sending half of a split channel.
pub struct ChannelWriter<W> {
    stream: W,
    key: [u8; 32],
    seq: u64,
    closed: bool,
}

impl<W> std::fmt::Debug for ChannelWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelWriter")
            .field("seq", &self.seq)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl<W: AsyncWrite + Unpin> ChannelWriter<W> {
    pub(crate) fn new(stream: W, key: [u8; 32], seq: u64, closed: bool) -> Self {
        Self {
            stream,
            key,
            seq,
            closed,
        }
    }

    /// Send one data frame.
    ///
    /// # Errors
    /// Refuses payloads over [`MAX_DATA_BYTES`] or a closed half, and reports
    /// write failures as `unavailable`.
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        if self.closed {
            return fail(Refusal::Unavailable, "channel closed");
        }
        if data.len() > MAX_DATA_BYTES {
            return fail(
                Refusal::LimitExceeded,
                "data exceeds the frame payload bound",
            );
        }
        self.write(FrameKind::Data, data).await
    }

    /// Send a close frame and shut the stream down.
    ///
    /// # Errors
    /// Reports a failed write.
    pub async fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.write(FrameKind::Close, &[]).await?;
        self.closed = true;
        self.stream
            .shutdown()
            .await
            .map_err(|_| Error::new(Refusal::Unavailable, "shutdown failed"))
    }

    async fn write(&mut self, kind: FrameKind, data: &[u8]) -> Result<()> {
        let body = seal_frame(&self.key, kind, self.seq, data)?;
        if let Err(error) = write_frame(&mut self.stream, kind, self.seq, &body).await {
            self.closed = true;
            return Err(error);
        }
        self.seq += 1;
        Ok(())
    }
}
