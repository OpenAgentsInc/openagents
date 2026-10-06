//! Bounded chamber framing shared by native and browser clients.
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[cfg(not(target_arch = "wasm32"))]
pub trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}
#[cfg(target_arch = "wasm32")]
pub trait Transport: AsyncRead + AsyncWrite + Unpin {}
#[cfg(target_arch = "wasm32")]
impl<T: AsyncRead + AsyncWrite + Unpin> Transport for T {}

/// Reads a big-endian u32 byte length before allocating its bounded JSON payload.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Vec<u8>, String> {
    let size = reader
        .read_u32()
        .await
        .map_err(|_| "Chamber frame header unavailable")? as usize;
    if size == 0 || size > max {
        return Err("Chamber frame exceeds byte budget".into());
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| "Chamber frame payload incomplete")?;
    Ok(bytes)
}
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    max: usize,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > max {
        return Err("Chamber frame exceeds byte budget".into());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| "Chamber frame length overflow")?;
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(bytes);
    writer
        .write_all(&frame)
        .await
        .map_err(|_| "Cannot write chamber frame payload")?;
    writer
        .flush()
        .await
        .map_err(|_| "Cannot flush chamber frame")?;
    Ok(())
}
