//! Length-prefixed JSON messages: a 4-byte big-endian length, then that many
//! bytes of UTF-8 JSON. Used by the enroll ALPN and the control socket.

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Code, Error, Result, fail};

/// Write one message.
///
/// # Errors
/// `bounds` when the encoded message exceeds `max`; `unavailable` when the
/// write fails.
pub async fn write_message<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
    max: usize,
) -> Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|_| Error::new(Code::Malformed, "message does not encode"))?;
    if body.len() > max {
        return fail(Code::Bounds, "message exceeds its bound");
    }
    let len = u32::try_from(body.len()).map_err(|_| Error::new(Code::Bounds, "message length"))?;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend(len.to_be_bytes());
    frame.extend(body);
    writer
        .write_all(&frame)
        .await
        .map_err(|_| Error::new(Code::Unavailable, "write failed"))?;
    writer
        .flush()
        .await
        .map_err(|_| Error::new(Code::Unavailable, "write failed"))
}

/// Read one message. `Ok(None)` when the peer closed before a length.
///
/// # Errors
/// `bounds` for a length over `max`; `malformed` for a body that is not the
/// expected JSON; `unavailable` for a stream that ends inside a message.
pub async fn read_message<R: AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
    max: usize,
) -> Result<Option<T>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(_) => return fail(Code::Unavailable, "read failed"),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > max {
        return fail(Code::Bounds, "message exceeds its bound");
    }
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|_| Error::new(Code::Unavailable, "stream ended inside a message"))?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|_| Error::new(Code::Malformed, "message is not the expected JSON"))
}
