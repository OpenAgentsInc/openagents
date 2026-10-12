//! Host snapshots of completed Metal prefix state.

use super::{ClefMetalBuffer, Dims, Request};

pub(super) struct Snapshot {
    conv: Vec<Option<Vec<u8>>>,
    delta: Vec<Option<Vec<u8>>>,
    keys: Vec<Option<Vec<u8>>>,
    values: Vec<Option<Vec<u8>>>,
    memory: Vec<u8>,
}

impl Snapshot {
    pub(super) fn byte_len(request: &Request, dims: Dims, prefix: usize) -> Result<usize, String> {
        let kv = prefix_bytes(prefix, dims.kv_heads, dims.head_dim, 2)?;
        let memory = prefix_bytes(prefix, 1, dims.width, 4)?;
        let mut bytes = size_of::<Self>()
            .checked_add(memory)
            .ok_or("prefix size overflow")?;
        for buffers in [
            &request.conv,
            &request.delta,
            &request.key_cache,
            &request.value_cache,
        ] {
            bytes = bytes
                .checked_add(
                    buffers
                        .len()
                        .checked_mul(size_of::<Option<Vec<u8>>>())
                        .ok_or("prefix size overflow")?,
                )
                .ok_or("prefix size overflow")?;
        }
        for buffer in request.conv.iter().chain(&request.delta).flatten() {
            bytes = bytes
                .checked_add(buffer.byte_len())
                .ok_or("prefix size overflow")?;
        }
        for _ in request
            .key_cache
            .iter()
            .chain(&request.value_cache)
            .flatten()
        {
            bytes = bytes.checked_add(kv).ok_or("prefix size overflow")?;
        }
        Ok(bytes)
    }

    /// The caller must wait for the prefix's final command buffer first.
    pub(super) fn capture(request: &Request, dims: Dims, prefix: usize) -> Result<Self, String> {
        let kv = prefix_bytes(prefix, dims.kv_heads, dims.head_dim, 2)?;
        let read = |buffers: &[Option<ClefMetalBuffer>], len: Option<usize>| {
            buffers
                .iter()
                .map(|buffer| {
                    buffer
                        .as_ref()
                        .map(|buffer| buffer.read_bytes(0, len.unwrap_or(buffer.byte_len())))
                        .transpose()
                })
                .collect::<Result<Vec<_>, String>>()
        };
        Ok(Self {
            conv: read(&request.conv, None)?,
            delta: read(&request.delta, None)?,
            keys: read(&request.key_cache, Some(kv))?,
            values: read(&request.value_cache, Some(kv))?,
            memory: request
                .memory
                .read_bytes(0, prefix_bytes(prefix, 1, dims.width, 4)?)?,
        })
    }

    pub(super) fn restore(&self, request: &Request) -> Result<(), String> {
        let pairs = [
            (&self.conv, &request.conv),
            (&self.delta, &request.delta),
            (&self.keys, &request.key_cache),
            (&self.values, &request.value_cache),
        ];
        // Validate the entire layout before the first write. Snapshots belong
        // to one loaded trunk, so a mismatch indicates an internal error.
        for (saved, target) in &pairs {
            if saved.len() != target.len() {
                return Err(String::from("prefix layer count does not match"));
            }
            for (saved, target) in saved.iter().zip(target.iter()) {
                match (saved, target) {
                    (None, None) => {}
                    (Some(saved), Some(target)) if saved.len() <= target.byte_len() => {}
                    _ => return Err(String::from("prefix buffer does not match")),
                }
            }
        }
        if self.memory.len() > request.memory.byte_len() {
            return Err(String::from("prefix memory does not fit"));
        }
        for (saved, target) in pairs {
            for (saved, target) in saved.iter().zip(target.iter()) {
                if let (Some(saved), Some(target)) = (saved, target) {
                    target.write_bytes(0, saved)?;
                }
            }
        }
        request.memory.write_bytes(0, &self.memory)
    }
}

fn prefix_bytes(
    prefix: usize,
    heads: usize,
    width: usize,
    scalar_bytes: usize,
) -> Result<usize, String> {
    prefix
        .checked_mul(heads)
        .and_then(|v| v.checked_mul(width))
        .and_then(|v| v.checked_mul(scalar_bytes))
        .ok_or_else(|| String::from("prefix size overflow"))
}
