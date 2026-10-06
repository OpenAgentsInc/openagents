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

/// Writes a bounded group without changing individual framing or request order.
pub(super) async fn write_frame_batch<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frames: &[Vec<u8>],
    max: usize,
) -> Result<(), String> {
    if frames.is_empty() || frames.len() > super::client::PIPELINE_CAPACITY {
        return Err("Chamber frame batch exceeds request budget".into());
    }
    let mut size = 0usize;
    for bytes in frames {
        if bytes.is_empty() || bytes.len() > max || u32::try_from(bytes.len()).is_err() {
            return Err("Chamber frame exceeds byte budget".into());
        }
        size = size
            .checked_add(bytes.len())
            .and_then(|size| size.checked_add(4))
            .ok_or("Chamber frame batch length overflow")?;
    }
    let mut batch = Vec::with_capacity(size);
    for bytes in frames {
        batch.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        batch.extend_from_slice(bytes);
    }
    writer
        .write_all(&batch)
        .await
        .map_err(|_| "Cannot write chamber frame batch")?;
    writer
        .flush()
        .await
        .map_err(|_| "Cannot flush chamber frame batch")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn batched_frames_preserve_boundaries_through_partial_writes() {
        let (mut writer, mut reader) = tokio::io::duplex(7);
        let frames = vec![vec![1; 13], vec![2; 3], vec![3; 19]];
        let (sent, received) = tokio::join!(write_frame_batch(&mut writer, &frames, 19), async {
            let mut received = Vec::new();
            for _ in 0..3 {
                received.push(read_frame(&mut reader, 19).await.unwrap());
            }
            received
        });
        sent.unwrap();
        assert_eq!(received, frames);
    }

    #[tokio::test]
    async fn invalid_batch_sends_no_prefix_and_cannot_exceed_pipeline_capacity() {
        for frames in [
            vec![vec![1], vec![2; 9]],
            vec![vec![1], Vec::new()],
            vec![vec![1]; super::super::client::PIPELINE_CAPACITY + 1],
            Vec::new(),
        ] {
            let (mut writer, mut reader) = tokio::io::duplex(64);
            assert!(write_frame_batch(&mut writer, &frames, 8).await.is_err());
            drop(writer);
            let mut received = Vec::new();
            reader.read_to_end(&mut received).await.unwrap();
            assert!(
                received.is_empty(),
                "An invalid batch cannot send an earlier valid request"
            );
        }
    }
}
