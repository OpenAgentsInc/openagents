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

const COMPRESSED: u32 = 1 << 31;
const COMPRESSION_THRESHOLD: usize = 4096;

// Both encoded and decoded lengths remain inside the existing message budget.
#[inline(never)]
fn encode_frame(bytes: &[u8], max: usize) -> Result<Vec<u8>, String> {
    if bytes.is_empty() || bytes.len() > max || bytes.len() >= COMPRESSED as usize {
        return Err("Chamber frame exceeds byte budget".into());
    }
    if bytes.len() >= COMPRESSION_THRESHOLD {
        let compressed = miniz_oxide::deflate::compress_to_vec(bytes, 1);
        // Keep raw framing when compression barely shrinks the payload.
        if compressed.len() + 4 <= bytes.len() - bytes.len() / 8 {
            let size = (compressed.len() + 4) as u32;
            let mut frame = Vec::with_capacity(size as usize + 4);
            frame.extend_from_slice(&(size | COMPRESSED).to_be_bytes());
            frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            frame.extend_from_slice(&compressed);
            return Ok(frame);
        }
    }
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(bytes);
    Ok(frame)
}

/// Reads bounded raw JSON or a DEFLATE payload with a bounded decoded length.
pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Vec<u8>, String> {
    let header = reader
        .read_u32()
        .await
        .map_err(|_| "Chamber frame header unavailable")?;
    let size = (header & !COMPRESSED) as usize;
    if size == 0 || size > max {
        return Err("Chamber frame exceeds byte budget".into());
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| "Chamber frame payload incomplete")?;
    decode_frame(header, bytes, max)
}

// Keep the bounded codec shared across generic native, TLS, and REACH readers.
#[inline(never)]
fn decode_frame(header: u32, bytes: Vec<u8>, max: usize) -> Result<Vec<u8>, String> {
    if header & COMPRESSED == 0 {
        return Ok(bytes);
    }
    if bytes.len() <= 4 {
        return Err("Compressed chamber frame is incomplete".into());
    }
    let decoded = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    if decoded == 0 || decoded > max {
        return Err("Decoded chamber frame exceeds byte budget".into());
    }
    let result = miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[4..], decoded)
        .map_err(|_| "Compressed chamber frame is invalid or exceeds its decoded length")?;
    if result.len() != decoded {
        return Err("Decoded chamber frame length mismatch".into());
    }
    Ok(result)
}
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    max: usize,
) -> Result<(), String> {
    let frame = encode_frame(bytes, max)?;
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
    // Encode every frame before sending any prefix, including validation failures.
    let encoded = frames
        .iter()
        .map(|bytes| encode_frame(bytes, max))
        .collect::<Result<Vec<_>, _>>()?;
    let size = encoded
        .iter()
        .try_fold(0usize, |size, frame| size.checked_add(frame.len()))
        .ok_or("Chamber frame batch length overflow")?;
    let mut batch = Vec::with_capacity(size);
    for frame in encoded {
        batch.extend_from_slice(&frame);
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
    async fn large_scene_frames_shrink_and_round_trip_with_small_ordered_replies() {
        let scene =
            br#"{"actors":[{"name":"Cultist of Anthropic","hp":15}],"geometry":[]}"#.repeat(2048);
        let small = br#"{"accepted":true}"#.to_vec();
        let encoded = encode_frame(&scene, scene.len()).unwrap();
        assert_ne!(
            u32::from_be_bytes(encoded[..4].try_into().unwrap()) & COMPRESSED,
            0
        );
        assert!(encoded.len() < scene.len() / 4);
        assert_eq!(&encode_frame(&small, scene.len()).unwrap()[4..], small);
        let frames = vec![scene, small];
        let max = frames[0].len();
        let (mut writer, mut reader) = tokio::io::duplex(17);
        let (sent, received) = tokio::join!(write_frame_batch(&mut writer, &frames, max), async {
            vec![
                read_frame(&mut reader, max).await.unwrap(),
                read_frame(&mut reader, max).await.unwrap(),
            ]
        });
        sent.unwrap();
        assert_eq!(received, frames);
    }

    #[tokio::test]
    async fn compressed_frames_enforce_encoded_and_decoded_bounds() {
        let data = vec![b'a'; 8192];
        let compressed = miniz_oxide::deflate::compress_to_vec(&data, 1);
        for (declared, payload) in [
            (8192u32, compressed.clone()), // Declared expansion exceeds the reader budget.
            (32, compressed.clone()),      // Actual expansion exceeds the declared length.
            (8193, compressed.clone()),    // The declared length must match exactly.
            (8192, vec![0xff; 8]),         // Invalid compressed data.
            (0, compressed),
        ] {
            let mut encoded = (((payload.len() + 4) as u32) | COMPRESSED)
                .to_be_bytes()
                .to_vec();
            encoded.extend_from_slice(&declared.to_be_bytes());
            encoded.extend_from_slice(&payload);
            let max = if declared == 8192 && payload.len() > 8 {
                4096
            } else {
                16384
            };
            assert!(read_frame(&mut encoded.as_slice(), max).await.is_err());
        }
        let oversized = (COMPRESSED | 65).to_be_bytes().to_vec();
        assert!(read_frame(&mut oversized.as_slice(), 64).await.is_err());
    }

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
