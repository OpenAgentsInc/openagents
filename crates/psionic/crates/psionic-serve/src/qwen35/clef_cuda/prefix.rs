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

#[derive(Clone)]
pub(super) struct Snapshot {
    buffer: CudaBuffer,
    prefix: usize,
}

/// A snapshot taken inside the chunk that contains its end (#11442): the
/// chunk's conv and blocked DeltaNet scan write the state after `at` tokens
/// straight into the regions at `conv` / `delta` (f32 element offsets per
/// layer), and the positional regions (K/V, memory rows) are copied once
/// that chunk has completed ([`Snapshot::copy_positional`]). A miss then
/// runs its cold chunking unchanged, so it costs only the copies. `failed`
/// is set when the chunk could not write the state (the per-token scan
/// ran); the snapshot is then dropped.
pub(super) struct SnapTarget {
    pub(super) at: usize,
    pub(super) bytes: usize,
    pub(super) snapshot: Snapshot,
    pub(super) conv: Vec<Option<usize>>,
    pub(super) delta: Vec<Option<usize>>,
    pub(super) failed: std::cell::Cell<bool>,
}

/// Whether a miss can take its snapshot inside a chunk: every chunk starts
/// on the blocked scan's 16-token grid and that scan runs.
pub(super) fn mid_chunk(chunk: usize) -> bool {
    chunk % PREFIX_ALIGN == 0 && knobs().scan.block_variant().is_some() && !knobs().skip_delta
}

/// Allocates a snapshot of `request`'s first `prefix` tokens for a capture
/// inside a chunk, within the budget after evicting older snapshots.
pub(super) fn arm_within_budget(
    state: &mut DeviceState,
    request: &Request,
    dims: Dims,
    prefix: usize,
) -> Result<Option<SnapTarget>, String> {
    reap_pending(state);
    let bytes = Snapshot::byte_len(request, dims, prefix)?;
    let entry = bytes + prefix * size_of::<u32>();
    if !state.prefix.admits(prefix, bytes) || !state.prefix.make_room(entry + pending_bytes(state))
    {
        return Ok(None);
    }
    let Some(snapshot) = Snapshot::allocate(&mut state.backend, request, dims, prefix)? else {
        return Ok(None);
    };
    let (mut conv, mut delta) = (Vec::new(), Vec::new());
    let mut offset = 0;
    for (buffers, out) in [(&request.conv, &mut conv), (&request.delta, &mut delta)] {
        for buffer in buffers {
            out.push(buffer.as_ref().map(|buffer| {
                let at = offset / 4;
                offset += buffer.byte_len();
                at
            }));
        }
    }
    Ok(Some(SnapTarget {
        at: prefix,
        bytes,
        snapshot,
        conv,
        delta,
        failed: std::cell::Cell::new(false),
    }))
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

    /// An uninitialized snapshot buffer of `request`'s first `prefix` tokens.
    /// `Ok(None)` when free device memory cannot hold it now.
    fn allocate(
        backend: &mut CudaBackend,
        request: &Request,
        dims: Dims,
        prefix: usize,
    ) -> Result<Option<Self>, String> {
        let total = Self::byte_len(request, dims, prefix)?;
        match admit_allocation(backend, "prefix_snapshot", &[total as u64]) {
            Ok(()) => {}
            Err(error) if memory_admission::is_memory_refusal(&error) => return Ok(None),
            Err(error) => return Err(error),
        }
        let buffer = backend.f32_buffer(total.div_ceil(4).max(1)).map_err(err)?;
        Ok(Some(Self { buffer, prefix }))
    }

    pub(super) fn buffer(&self) -> &CudaBuffer {
        &self.buffer
    }

    /// Copies the K/V and memory rows of the first `prefix` tokens (every
    /// region after the recurrent state, which the chunk wrote already).
    /// The caller must have waited for the chunk that contains `prefix`.
    pub(super) fn copy_positional(
        &self,
        backend: &mut CudaBackend,
        request: &Request,
        dims: Dims,
    ) -> Result<(), String> {
        let recurrent = request.conv.iter().chain(&request.delta).flatten().count();
        let mut submission = backend.begin_submission().map_err(err)?;
        let mut offset = 0;
        for (index, (source, bytes)) in regions(request, dims, self.prefix)?.into_iter().enumerate()
        {
            if index >= recurrent {
                submission
                    .copy_buffer_region(source, 0, &self.buffer, offset, bytes)
                    .map_err(err)?;
            }
            offset += bytes;
        }
        submission.commit(CudaCommandWait::Completed).map_err(err)?;
        Ok(())
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

/// Drops the pending capture owned by `owner`, if any.
pub(super) fn drop_pending(state: &mut DeviceState, owner: &Arc<()>) {
    let weak = Arc::downgrade(owner);
    state
        .pending_prefix
        .retain(|pending| !pending.owner.ptr_eq(&weak));
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

/// Snapshot budget: `PSIONIC_CLEF_PREFIX_CACHE_BYTES`; `0` turns reuse off.
/// Unset, CUDA reuses prefixes by default within [`default_capacity`] where
/// reuse is exact and free for misses ([`exact_by_default`]), and not
/// elsewhere. An explicit budget may be up to 32 GiB and never more than a
/// quarter of the device (an 80 GB card can hold 20 GB of prefixes).
/// Returns the budget and whether it was set explicitly.
pub(super) fn capacity(total_device_bytes: u64) -> Result<(usize, bool), String> {
    let Some(value) = std::env::var_os("PSIONIC_CLEF_PREFIX_CACHE_BYTES") else {
        let budget = if exact_by_default() {
            default_capacity(total_device_bytes)
        } else {
            0
        };
        return Ok((budget, false));
    };
    Ok((parse_capacity(value.to_str(), total_device_bytes)?, true))
}

/// The trunk is bitwise chunk-invariant on the 16-token grid (Hopper GEMM
/// and attention, blocked DeltaNet scan: the sm_90 defaults), so a restored
/// prefix gives exactly the cold logits (#11442, measured on the H100), and
/// a miss snapshots inside its chunk ([`mid_chunk`]) at the cost of copies.
pub(super) fn exact_by_default() -> bool {
    let knobs = knobs();
    knobs.hopper
        && knobs.hopper_attention
        && knobs.scan.block_variant().is_some()
        && !(knobs.skip_delta || knobs.skip_gemm || knobs.skip_dequant || knobs.skip_attention)
}

/// The budget when none is set: a 32nd of the device, at most 2 GiB (an
/// 80 GB card keeps about ten 4k-token Clef-Flash states or four 27B ones).
pub(super) fn default_capacity(total_device_bytes: u64) -> usize {
    usize::try_from(DEFAULT_MAX_CAPACITY.min(total_device_bytes / 32)).unwrap_or(0)
}

pub(super) const DEFAULT_MAX_CAPACITY: u64 = 2 << 30;

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
        assert_eq!(default_capacity(h100), 2 << 30);
        assert_eq!(default_capacity(40 << 30), 1280 << 20);
        assert_eq!(default_capacity(0), 0);
    }
}
