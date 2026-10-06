//! The binary-message bound shared by native and browser channel carriage.
use crate::channel::{HEADER_BYTES, MAX_FRAME_BYTES};
use crate::{Refusal, Result, fail};
pub const MAX_MESSAGE_BYTES: usize = 4 + MAX_FRAME_BYTES;
/// Check that a received binary message is exactly one bounded frame.
///
/// # Errors
/// Refuses a message over [`MAX_MESSAGE_BYTES`] or whose prefix claims more
/// than [`MAX_FRAME_BYTES`] as `limit_exceeded`, and a message shorter than a
/// frame header or whose prefix disagrees with its size as `malformed`.
pub fn check_message(message: &[u8]) -> Result<()> {
    if message.len() > MAX_MESSAGE_BYTES {
        return fail(Refusal::LimitExceeded, "message exceeds the frame bound");
    }
    let Some(prefix) = message.first_chunk::<4>() else {
        return fail(Refusal::Malformed, "message shorter than a frame");
    };
    let len = u32::from_be_bytes(*prefix) as usize;
    if len > MAX_FRAME_BYTES {
        return fail(Refusal::LimitExceeded, "frame exceeds the maximum size");
    }
    if len < HEADER_BYTES {
        return fail(Refusal::Malformed, "frame shorter than its header");
    }
    if message.len() != 4 + len {
        return fail(Refusal::Malformed, "a message must carry exactly one frame");
    }
    Ok(())
}
