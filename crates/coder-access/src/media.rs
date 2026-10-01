//! Images a device attaches to a Coder task (`artifact.put`, then
//! `task.create` naming them).
//!
//! A device never sends a path. It sends each image's exact bytes to the
//! host that owns the workspace in sequential chunks small enough for every
//! NIP-HOST binding, each chunk naming the image's SHA-256 digest, media
//! type, and size. The host keeps the chunks for that device only, checks
//! the digest and the image type once the last chunk arrives, and binds the
//! verified bytes to the task that names them. Only PNG and JPEG images of
//! at most [`MAX_IMAGE_BYTES`] are accepted, at most [`MAX_IMAGES`] a task.
//! A chunk is idempotent: sending the same bytes again at an offset the host
//! already holds changes nothing, so a retry after a lost reply is safe, and
//! the reply says how many bytes the host holds so a client resumes there.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::{Code, Error, Result};

/// The most bytes of one image.
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
/// The most images one task carries.
pub const MAX_IMAGES: usize = 4;
/// The bytes of every chunk but the last: small enough that the signed,
/// encrypted request stays far below each binding's message bound.
pub const CHUNK_BYTES: u64 = 32 * 1024;
/// The longest image name a task records.
pub const MAX_NAME: usize = 96;

/// The image types a task accepts.
pub const PNG: &str = "image/png";
pub const JPEG: &str = "image/jpeg";

/// The media type `bytes` start with, from their signature alone: PNG or
/// JPEG, else `None`.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(PNG)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(JPEG)
    } else {
        None
    }
}

/// The `sha256:<hex>` digest of `bytes`.
#[must_use]
pub fn digest(bytes: &[u8]) -> String {
    nostr::contracts::digest_bytes(bytes)
}

/// The 64 lowercase hex digits of a `sha256:` digest, the only form a host
/// uses in a file name.
///
/// # Errors
/// Refuses any other form.
pub fn hex(digest: &str) -> Result<&str> {
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| Error::new(Code::Malformed, "image digest must be sha256"))?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::new(Code::Malformed, "image digest must be 64 hex"));
    }
    Ok(hex)
}

/// One attached image a task names. The host resolves it to bytes it
/// already holds and verified; the reference carries no bytes and no path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRef {
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    /// The person's name for the image, shown and never used as a path.
    pub name: String,
}

impl ImageRef {
    /// # Errors
    /// Refuses a reference outside its bounds.
    pub fn validate(&self) -> Result<()> {
        hex(&self.digest)?;
        media_type(&self.media_type)?;
        if self.size == 0 || self.size > MAX_IMAGE_BYTES {
            return Err(Error::new(Code::Bounds, "image size exceeds its bound"));
        }
        if self.name.chars().count() > MAX_NAME || self.name.chars().any(char::is_control) {
            return Err(Error::new(Code::Malformed, "image name exceeds its bound"));
        }
        Ok(())
    }
}

/// Check a list of image references: at most [`MAX_IMAGES`], each valid,
/// no digest twice.
///
/// # Errors
/// Refuses the first reference outside its bounds.
pub fn validate_all(images: &[ImageRef]) -> Result<()> {
    if images.len() > MAX_IMAGES {
        return Err(Error::new(Code::Bounds, "a task carries at most 4 images"));
    }
    for (index, image) in images.iter().enumerate() {
        image.validate()?;
        if images[..index]
            .iter()
            .any(|seen| seen.digest == image.digest)
        {
            return Err(Error::new(Code::Malformed, "an image is named twice"));
        }
    }
    Ok(())
}

fn media_type(value: &str) -> Result<()> {
    if value == PNG || value == JPEG {
        Ok(())
    } else {
        Err(Error::new(
            Code::Unsupported,
            "only PNG and JPEG images are accepted",
        ))
    }
}

/// One chunk of an image on its way to the host (`artifact.put`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPut {
    pub digest: String,
    pub media_type: String,
    pub size: u64,
    /// Where this chunk starts: a multiple of [`CHUNK_BYTES`].
    pub offset: u64,
    /// The chunk's bytes, standard base64 with padding.
    pub data: String,
}

impl ArtifactPut {
    /// # Errors
    /// Refuses a chunk outside its bounds or of the wrong length.
    pub fn validate(&self) -> Result<()> {
        self.bytes().map(|_| ())
    }

    /// The chunk's bytes, after every bound is checked.
    ///
    /// # Errors
    /// As [`ArtifactPut::validate`].
    pub fn bytes(&self) -> Result<Vec<u8>> {
        hex(&self.digest)?;
        media_type(&self.media_type)?;
        if self.size == 0 || self.size > MAX_IMAGE_BYTES {
            return Err(Error::new(Code::Bounds, "image size exceeds its bound"));
        }
        if !self.offset.is_multiple_of(CHUNK_BYTES) || self.offset >= self.size {
            return Err(Error::new(Code::Malformed, "chunk offset is out of place"));
        }
        // Base64 of one chunk is at most 4/3 of it, rounded up to 4.
        if self.data.len() as u64 > CHUNK_BYTES.div_ceil(3) * 4 {
            return Err(Error::new(Code::Bounds, "chunk exceeds its bound"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|_| Error::new(Code::Malformed, "chunk is not base64"))?;
        let expected = CHUNK_BYTES.min(self.size - self.offset);
        if bytes.len() as u64 != expected {
            return Err(Error::new(Code::Malformed, "chunk has the wrong length"));
        }
        Ok(bytes)
    }
}

/// What the host holds of one image after a chunk (`artifact`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactState {
    pub digest: String,
    /// The bytes held, from the start.
    pub received: u64,
    /// Every byte arrived and matched the digest and the image type.
    pub complete: bool,
}

/// An image a device is about to send: its exact bytes and the reference a
/// task names it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    pub reference: ImageRef,
    pub bytes: std::sync::Arc<Vec<u8>>,
}

impl Upload {
    /// An upload of `bytes`, refused before anything is sent when the bytes
    /// are not a PNG or JPEG image or exceed [`MAX_IMAGE_BYTES`].
    ///
    /// # Errors
    /// The bytes are empty, too large, or not PNG or JPEG.
    pub fn new(name: &str, bytes: std::sync::Arc<Vec<u8>>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Err(Error::new(Code::Bounds, "Images must be 8 MiB or smaller."));
        }
        let media_type = sniff(&bytes)
            .ok_or_else(|| Error::new(Code::Unsupported, "Choose a PNG or JPEG image."))?;
        let name: String = name
            .chars()
            .filter(|ch| !ch.is_control())
            .take(MAX_NAME)
            .collect();
        Ok(Self {
            reference: ImageRef {
                digest: digest(&bytes),
                media_type: media_type.into(),
                size: bytes.len() as u64,
                name: if name.is_empty() {
                    "Image".into()
                } else {
                    name
                },
            },
            bytes,
        })
    }

    /// The chunks that carry this image, in order, starting at `from` (a
    /// multiple of [`CHUNK_BYTES`]; a resumed upload starts where the host
    /// said it holds).
    #[must_use]
    pub fn chunks(&self, from: u64) -> Vec<ArtifactPut> {
        let mut chunks = Vec::new();
        let mut offset = from - from % CHUNK_BYTES;
        while offset < self.reference.size {
            let end = (offset + CHUNK_BYTES).min(self.reference.size);
            // Offsets are below MAX_IMAGE_BYTES, which fits in usize.
            let (start, end) = (offset as usize, end as usize);
            chunks.push(ArtifactPut {
                digest: self.reference.digest.clone(),
                media_type: self.reference.media_type.clone(),
                size: self.reference.size,
                offset,
                data: base64::engine::general_purpose::STANDARD.encode(&self.bytes[start..end]),
            });
            offset += CHUNK_BYTES;
        }
        chunks
    }
}

/// Send `uploads` with `put`, one chunk at a time, resuming each image where
/// the host says it holds, and return their references once the host holds
/// every image complete.
///
/// # Errors
/// The first refusal or transport error; a host that never reports an image
/// complete refuses as `unavailable`. Nothing a task names is created here.
pub fn send(
    uploads: &[Upload],
    mut put: impl FnMut(&ArtifactPut) -> Result<ArtifactState>,
) -> Result<Vec<ImageRef>> {
    if uploads.len() > MAX_IMAGES {
        return Err(Error::new(Code::Bounds, "a task carries at most 4 images"));
    }
    for upload in uploads {
        let mut from = 0;
        // Each pass sends the rest; a host that lost bytes says where to
        // start again, at most a few times.
        let mut complete = false;
        for _ in 0..3 {
            let mut state = None;
            for chunk in upload.chunks(from) {
                let answered = put(&chunk)?;
                if answered.digest != upload.reference.digest {
                    return Err(Error::new(
                        Code::Malformed,
                        "the host answered another image",
                    ));
                }
                if answered.complete {
                    state = Some(answered);
                    break;
                }
                // The host holds fewer bytes than were sent: resume there.
                if answered.received
                    < chunk.offset + CHUNK_BYTES.min(upload.reference.size - chunk.offset)
                {
                    state = Some(answered);
                    break;
                }
                state = Some(answered);
            }
            match state {
                Some(state) if state.complete => {
                    complete = true;
                    break;
                }
                Some(state) => from = state.received,
                None => break,
            }
        }
        if !complete {
            return Err(Error::new(
                Code::Unavailable,
                "the host did not hold the whole image",
            ));
        }
    }
    let references: Vec<ImageRef> = uploads.iter().map(|u| u.reference.clone()).collect();
    validate_all(&references)?;
    Ok(references)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn png(len: usize) -> Arc<Vec<u8>> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend((0..len).map(|i| (i % 251) as u8));
        Arc::new(bytes)
    }

    #[test]
    fn chunks_carry_the_exact_bytes_and_validate() {
        let upload = Upload::new("Shot.png", png(100_000)).unwrap();
        let chunks = upload.chunks(0);
        assert_eq!(chunks.len(), 4);
        let mut joined = Vec::new();
        for chunk in &chunks {
            joined.extend(chunk.bytes().unwrap());
        }
        assert_eq!(joined, *upload.bytes);
        assert_eq!(digest(&joined), upload.reference.digest);
        assert_eq!(upload.chunks(CHUNK_BYTES * 3).len(), 1);
    }

    #[test]
    fn oversized_and_unsupported_images_are_refused_before_send() {
        let big = Upload::new("big.png", png(MAX_IMAGE_BYTES as usize)).unwrap_err();
        assert_eq!(big.code, Code::Bounds);
        let gif = Upload::new("a.gif", Arc::new(b"GIF89a....".to_vec())).unwrap_err();
        assert_eq!(gif.code, Code::Unsupported);
        assert!(Upload::new("e", Arc::new(vec![])).is_err());
        let mut chunk = Upload::new("a", png(10)).unwrap().chunks(0).remove(0);
        chunk.media_type = "image/gif".into();
        assert!(chunk.validate().is_err());
        chunk.media_type = PNG.into();
        chunk.offset = 1;
        assert!(chunk.validate().is_err());
    }

    #[test]
    fn send_resumes_where_the_host_holds() {
        let upload = Upload::new("a.jpg", {
            let mut bytes = vec![0xff, 0xd8, 0xff];
            bytes.extend(vec![7; 70_000]);
            Arc::new(bytes)
        })
        .unwrap();
        assert_eq!(upload.reference.media_type, JPEG);
        let mut held: Vec<u8> = Vec::new();
        let mut lost_once = false;
        let references = send(std::slice::from_ref(&upload), |chunk| {
            let bytes = chunk.bytes()?;
            if chunk.offset == held.len() as u64 {
                held.extend(bytes);
            }
            // The host restarted after the first chunk and lost it.
            if !lost_once && held.len() as u64 == CHUNK_BYTES * 2 {
                lost_once = true;
                held.truncate(CHUNK_BYTES as usize);
            }
            let complete = held.len() as u64 == chunk.size && digest(&held) == chunk.digest;
            Ok(ArtifactState {
                digest: chunk.digest.clone(),
                received: held.len() as u64,
                complete,
            })
        })
        .unwrap();
        assert_eq!(held, *upload.bytes);
        assert_eq!(references, vec![upload.reference.clone()]);
        assert!(validate_all(&[upload.reference.clone(), upload.reference]).is_err());
    }
}
