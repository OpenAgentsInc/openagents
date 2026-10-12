//! Device snapshots of completed CUDA prefix state (#11442).
//!
//! A snapshot is one device buffer holding, in a fixed order, every layer's
//! conv and DeltaNet state, the first `prefix` rows of every attention
//! layer's f16 K/V caches, and the first `prefix` projected memory rows. The
//! prefix never overlaps a question or option span
//! ([`crate::clef::prefix::reusable_boundary`]), so span sums start from zero
//! after a restore exactly as they do in a cold request at that point.

use std::sync::{Arc, Weak};

use super::*;

pub(super) struct Snapshot {
    buffer: CudaBuffer,
    prefix: usize,
}

/// A batched request's capture that waits for the request to finish before
/// it is published; dropping the request's owner token releases it.
pub(super) struct PendingPrefix {
    pub(super) owner: Weak<()>,
    pub(super) bytes: usize,
    pub(super) snapshot: Snapshot,
}

/// The device regions a snapshot holds, in order: `(buffer, bytes)`.
fn regions<'a>(
    request: &'a Request,
    dims: Dims,
    prefix: usize,
) -> Result<Vec<(&'a CudaBuffer, usize)>, String> {
    let kv = prefix_bytes(prefix, dims.kv_heads * dims.head_dim, 2)?;
    let memory = prefix_bytes(prefix, dims.width, 4)?;
    let mut regions = Vec::new();
    for buffer in request.conv.iter().chain(&request.delta).flatten() {
        regions.push((buffer, buffer.byte_len()));
    }
    for buffer in request
        .key_cache
        .iter()
        .chain(&request.value_cache)
        .flatten()
    {
        if kv > buffer.byte_len() {
            return Err(String::from("prefix K/V does not fit the request"));
        }
        regions.push((buffer, kv));
    }
    if memory > request.memory.byte_len() {
        return Err(String::from("prefix memory does not fit the request"));
    }
    regions.push((&request.memory, memory));
    Ok(regions)
}

fn prefix_bytes(prefix: usize, width: usize, scalar_bytes: usize) -> Result<usize, String> {
    prefix
        .checked_mul(width)
        .and_then(|value| value.checked_mul(scalar_bytes))
        .ok_or_else(|| String::from("prefix size overflow"))
}

impl Snapshot {
    /// Device bytes of a snapshot of `request` after `prefix` tokens.
    pub(super) fn byte_len(request: &Request, dims: Dims, prefix: usize) -> Result<usize, String> {
        regions(request, dims, prefix)?
            .iter()
            .try_fold(0usize, |total, (_, bytes)| total.checked_add(*bytes))
            .ok_or_else(|| String::from("prefix size overflow"))
    }

    /// Copies the state after `prefix` tokens into a new device buffer. The
    /// caller must have waited for the chunk that ends at `prefix`. `Ok(None)`
    /// when free device memory cannot hold the copy now.
    pub(super) fn capture(
        backend: &mut CudaBackend,
        request: &Request,
        dims: Dims,
        prefix: usize,
    ) -> Result<Option<Self>, String> {
        let regions = regions(request, dims, prefix)?;
        let total: usize = regions.iter().map(|(_, bytes)| bytes).sum();
        match admit_allocation(backend, "prefix_snapshot", &[total as u64]) {
            Ok(()) => {}
            Err(error) if memory_admission::is_memory_refusal(&error) => return Ok(None),
            Err(error) => return Err(error),
        }
        // Every region is a whole number of f32 words (f16 rows are even).
        let buffer = backend.f32_buffer(total.div_ceil(4).max(1)).map_err(err)?;
        let mut submission = backend.begin_submission().map_err(err)?;
        let mut offset = 0;
        for (source, bytes) in regions {
            submission
                .copy_buffer_region(source, 0, &buffer, offset, bytes)
                .map_err(err)?;
            offset += bytes;
        }
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        Ok(Some(Self { buffer, prefix }))
    }

    /// Writes the snapshot into a fresh request of the same trunk.
    pub(super) fn restore(
        &self,
        backend: &mut CudaBackend,
        request: &Request,
        dims: Dims,
    ) -> Result<(), String> {
        let regions = regions(request, dims, self.prefix)?;
        let total: usize = regions.iter().map(|(_, bytes)| bytes).sum();
        // Snapshots belong to one loaded trunk, so a mismatch is an internal
        // error; check the whole layout before the first write.
        if total.div_ceil(4).max(1) * 4 != self.buffer.byte_len() {
            return Err(String::from("prefix snapshot does not match the request"));
        }
        let mut submission = backend.begin_submission().map_err(err)?;
        let mut offset = 0;
        for (target, bytes) in regions {
            submission
                .copy_buffer_region(&self.buffer, offset, target, 0, bytes)
                .map_err(err)?;
            offset += bytes;
        }
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        Ok(())
    }
}

/// Drops pending captures whose request ended without publishing them.
pub(super) fn reap_pending(state: &mut DeviceState) {
    state
        .pending_prefix
        .retain(|pending| pending.owner.upgrade().is_some());
}

/// Bytes held by pending captures.
pub(super) fn pending_bytes(state: &DeviceState) -> usize {
    state
        .pending_prefix
        .iter()
        .map(|pending| pending.bytes)
        .sum()
}

/// Captures `request`'s first `prefix` tokens if the budget has room after
/// evicting older snapshots. Each entry's budget includes its token key.
pub(super) fn capture_within_budget(
    state: &mut DeviceState,
    request: &Request,
    dims: Dims,
    prefix: usize,
) -> Result<Option<(usize, Snapshot)>, String> {
    reap_pending(state);
    let bytes = Snapshot::byte_len(request, dims, prefix)?;
    let entry = bytes + prefix * size_of::<u32>();
    if !state.prefix.admits(prefix, bytes) || !state.prefix.make_room(entry + pending_bytes(state))
    {
        return Ok(None);
    }
    Ok(Snapshot::capture(&mut state.backend, request, dims, prefix)?.map(|s| (bytes, s)))
}

/// Publishes the pending capture owned by `owner`, if any.
pub(super) fn publish_pending(
    state: &mut DeviceState,
    owner: &Arc<()>,
    tokens: &[u32],
    chunk: usize,
) {
    let weak = Arc::downgrade(owner);
    if let Some(index) = state
        .pending_prefix
        .iter()
        .position(|pending| pending.owner.ptr_eq(&weak))
    {
        let pending = state.pending_prefix.swap_remove(index);
        state
            .prefix
            .insert(tokens, chunk, pending.bytes, pending.snapshot);
    }
}

/// Snapshot budget: `PSIONIC_CLEF_PREFIX_CACHE_BYTES`, off when unset or 0.
/// CUDA keeps several snapshots, so the cap is 32 GiB and never more than a
/// quarter of the device (an 80 GB card can hold 20 GB of prefixes).
pub(super) fn capacity(total_device_bytes: u64) -> Result<usize, String> {
    let Some(value) = std::env::var_os("PSIONIC_CLEF_PREFIX_CACHE_BYTES") else {
        return Ok(0);
    };
    parse_capacity(value.to_str(), total_device_bytes)
}

pub(super) const MAX_CAPACITY: u64 = 32 << 30;

/// At most this many snapshots, whatever the budget.
pub(super) const MAX_ENTRIES: usize = 64;

fn parse_capacity(value: Option<&str>, total_device_bytes: u64) -> Result<usize, String> {
    let capacity: u64 = value
        .ok_or("prefix cache size must be an integer")?
        .trim()
        .parse()
        .map_err(|_| "prefix cache size must be an integer")?;
    let limit = MAX_CAPACITY.min(total_device_bytes / 4);
    if capacity > limit {
        return Err(format!(
            "prefix cache size exceeds {limit} bytes (32 GiB or a quarter of the device)"
        ));
    }
    usize::try_from(capacity).map_err(|_| String::from("prefix cache size overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_is_bounded_by_32_gib_and_a_quarter_of_the_device() {
        let h100 = 80 * (1u64 << 30);
        assert_eq!(parse_capacity(Some("0"), h100), Ok(0));
        assert_eq!(
            parse_capacity(Some("8589934592"), h100),
            Ok(8 << 30) // 8 GiB
        );
        assert_eq!(parse_capacity(Some("21474836480"), h100), Ok(20 << 30));
        assert!(parse_capacity(Some("21474836481"), h100).is_err());
        // a 16 GB card: at most 4 GiB
        assert!(parse_capacity(Some("4294967297"), 16 << 30).is_err());
        assert!(parse_capacity(Some("34359738369"), 1 << 40).is_err());
        assert!(parse_capacity(Some("lots"), h100).is_err());
        assert!(parse_capacity(None, h100).is_err());
    }
}
