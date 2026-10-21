//! DeepSeek-V4.1 stage kernels on Metal (#12420).
//!
//! The kernels in `kernels/v41/` port the CUDA backend's V4.1 kernels
//! (`psionic_backend_cuda::v41_*`, #12391–#12394) launch for launch: each
//! CUDA launcher becomes one Metal dispatch with the same grid, thread roles,
//! and floating-point order. Kernel arguments are structs of GPU addresses
//! (Metal 3), so every wrapper here keeps its CUDA counterpart's signature,
//! with [`V41Buffer`] in place of `CudaBuffer` and [`V41Submission`] in place
//! of `CudaSubmission`. The shapes, configuration types, and host models are
//! the CUDA backend's, re-exported from there.
//!
//! A submission records dispatches; [`V41Metal::run`] encodes one or more
//! recordings, each repeated a number of times, into one command buffer and
//! waits once. That replaces CUDA graph capture: a stage records its
//! per-token body once and replays it for every token of a step.
//!
//! Precision differences from CUDA, all documented in
//! `docs/psionic/v41-metal-plan.md`: Apple GPUs flush float32 subnormals to
//! zero, and they have no double precision, so `exp` and `log1p` are
//! float-float implementations checked exhaustively against the host,
//! RoPE reads host-computed `cos` and `sin` tables, and the sampler adds in
//! a software binary64.

pub mod matvec;

use metal::{
    Buffer, CompileOptions, ComputePipelineState, Device, MTLResourceOptions, MTLResourceUsage,
    MTLSize,
};
use objc::{msg_send, sel, sel_impl};
use psionic_runtime::RuntimeError;
use std::{collections::BTreeMap, ffi::c_void, sync::Mutex};

/// Every V4.1 kernel source, in compile order.
const SOURCES: [&str; 3] = [
    include_str!("../kernels/v41/common.metal"),
    include_str!("../kernels/v41/matvec.metal"),
    include_str!("../kernels/v41/checks.metal"),
];

/// The concatenated kernel source the runtime compiles; its SHA-256 is the
/// stage bundle's kernel identity.
#[must_use]
pub fn kernel_source() -> String {
    SOURCES.join("\n")
}

const PIPELINES: &[&str] = &[
    "v41_act_quant",
    "v41_block_matvec",
    "v41_check_exp",
    "v41_check_log1p",
    "v41_check_double",
];

pub(crate) fn invalid(context: &str, message: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Backend(format!("{context}: {message}"))
}

pub(crate) fn int(n: usize, what: &str) -> Result<i32, RuntimeError> {
    i32::try_from(n).map_err(|_| invalid("v41", format!("{what} {n} exceeds i32")))
}

/// A Metal buffer in shared (unified) memory.
#[derive(Debug)]
pub struct V41Buffer {
    buffer: Buffer,
    len: usize,
}

impl V41Buffer {
    /// Bytes in the buffer.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.len
    }

    /// The buffer's GPU address.
    #[must_use]
    pub fn gpu_address(&self) -> u64 {
        self.buffer.gpu_address()
    }

    /// The underlying Metal buffer.
    #[must_use]
    pub fn raw(&self) -> &Buffer {
        &self.buffer
    }

    /// The buffer's bytes; the GPU must not be writing it.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: a shared allocation of `len` bytes, read while no command
        // buffer that writes it is in flight (every run waits).
        unsafe { std::slice::from_raw_parts(self.buffer.contents().cast::<u8>(), self.len) }
    }

    /// The buffer's bytes for writing; the GPU must not be using it.
    #[allow(clippy::mut_from_ref)]
    #[must_use]
    pub fn bytes_mut(&self) -> &mut [u8] {
        // SAFETY: as `bytes`; callers write only between runs.
        unsafe { std::slice::from_raw_parts_mut(self.buffer.contents().cast::<u8>(), self.len) }
    }

    /// A copy of the buffer's bytes.
    ///
    /// # Errors
    ///
    /// Never; the signature matches `CudaBuffer::read_bytes`.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        Ok(self.bytes().to_vec())
    }

    /// Writes `bytes` at the start of the buffer.
    ///
    /// # Errors
    ///
    /// Refuses more bytes than the buffer holds.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.write_bytes_at_offset(0, bytes)
    }

    /// Writes `bytes` at `offset`.
    ///
    /// # Errors
    ///
    /// Refuses a write past the end.
    pub fn write_bytes_at_offset(
        &mut self,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        self.write_at(offset, bytes)
    }

    /// Writes `bytes` at `offset` through a shared reference.
    ///
    /// # Errors
    ///
    /// Refuses a write past the end.
    pub fn write_at(&self, offset: usize, bytes: &[u8]) -> Result<(), RuntimeError> {
        let end = offset
            .checked_add(bytes.len())
            .filter(|end| *end <= self.len)
            .ok_or_else(|| {
                invalid(
                    "v41 buffer",
                    format!("write of {} at {offset} past {}", bytes.len(), self.len),
                )
            })?;
        self.bytes_mut()[offset..end].copy_from_slice(bytes);
        Ok(())
    }

    /// The buffer as `f32` values.
    #[must_use]
    pub fn f32s(&self) -> Vec<f32> {
        self.bytes()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// The buffer as `i32` values.
    #[must_use]
    pub fn i32s(&self) -> Vec<i32> {
        self.bytes()
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }
}

/// Little-endian bytes of plain values.
pub(crate) fn le_bytes<T: Copy, const N: usize>(values: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|v| f(*v)).collect()
}

/// Kernel arguments laid out as a C struct: GPU addresses and 64-bit
/// integers on 8-byte boundaries, 32-bit values on 4-byte boundaries.
#[derive(Clone, Debug, Default)]
pub(crate) struct Args {
    bytes: Vec<u8>,
}

impl Args {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn align(&mut self, to: usize) {
        while !self.bytes.len().is_multiple_of(to) {
            self.bytes.push(0);
        }
    }

    /// A GPU address (0 for a null pointer).
    pub(crate) fn ptr(mut self, address: u64) -> Self {
        self.align(8);
        self.bytes.extend_from_slice(&address.to_le_bytes());
        self
    }

    pub(crate) fn i64(mut self, value: i64) -> Self {
        self.align(8);
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn i32(mut self, value: i32) -> Self {
        self.align(4);
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn f32(mut self, value: f32) -> Self {
        self.align(4);
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    fn finish(mut self) -> Vec<u8> {
        self.align(16);
        self.bytes
    }
}

/// One recorded dispatch.
#[derive(Clone, Debug)]
struct Dispatch {
    kernel: &'static str,
    args: Vec<u8>,
    groups: (usize, usize, usize),
    threads: usize,
    threadgroup_bytes: usize,
}

/// A sequence of dispatches and the buffers they reach.
#[derive(Clone, Debug, Default)]
pub struct V41Recording {
    dispatches: Vec<Dispatch>,
    resources: Vec<Buffer>,
    addresses: BTreeMap<u64, usize>,
}

impl V41Recording {
    /// Dispatches in the recording.
    #[must_use]
    pub fn len(&self) -> usize {
        self.dispatches.len()
    }

    /// Whether the recording is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dispatches.is_empty()
    }

    /// Per-kernel dispatch counts.
    #[must_use]
    pub fn kernel_counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for d in &self.dispatches {
            *counts.entry(d.kernel).or_insert(0) += 1;
        }
        counts
    }

    fn track(&mut self, buffer: &V41Buffer) {
        let address = buffer.gpu_address();
        if !self.addresses.contains_key(&address) {
            self.addresses.insert(address, self.resources.len());
            self.resources.push(buffer.buffer.clone());
        }
    }
}

/// Records kernel launches for [`V41Metal::run`]. The `v41_*` methods live
/// in the kernel modules.
pub struct V41Submission<'a> {
    metal: &'a V41Metal,
    recording: V41Recording,
}

impl V41Submission<'_> {
    /// The GPU address of `count` elements of `size` bytes at element
    /// `offset` of `buffer`, after checking they fit; the buffer joins the
    /// recording's resources.
    pub(crate) fn span(
        &mut self,
        buffer: &V41Buffer,
        offset: usize,
        count: usize,
        size: usize,
        label: &str,
    ) -> Result<u64, RuntimeError> {
        let end = offset
            .checked_add(count)
            .and_then(|n| n.checked_mul(size))
            .ok_or_else(|| invalid("v41", format!("{label} size overflow")))?;
        if end > buffer.len {
            return Err(invalid(
                "v41",
                format!("{label} needs {end} bytes; the buffer holds {}", buffer.len),
            ));
        }
        self.recording.track(buffer);
        Ok(buffer.gpu_address() + (offset * size) as u64)
    }

    /// An optional span: 0 (a null pointer) for `None`.
    pub(crate) fn span_opt(
        &mut self,
        buffer: Option<&V41Buffer>,
        offset: usize,
        count: usize,
        size: usize,
        label: &str,
    ) -> Result<u64, RuntimeError> {
        match buffer {
            Some(b) => self.span(b, offset, count, size, label),
            None => Ok(0),
        }
    }

    /// Records one dispatch of `groups` threadgroups of `threads` threads.
    pub(crate) fn launch(
        &mut self,
        kernel: &'static str,
        args: Args,
        groups: (usize, usize, usize),
        threads: usize,
        threadgroup_bytes: usize,
    ) -> Result<(), RuntimeError> {
        let pipeline = self
            .metal
            .pipelines
            .get(kernel)
            .ok_or_else(|| invalid("v41", format!("no kernel {kernel}")))?;
        if threads == 0 || threads as u64 > pipeline.max_total_threads_per_threadgroup() {
            return Err(invalid(
                kernel,
                format!(
                    "{threads} threads; the pipeline allows {}",
                    pipeline.max_total_threads_per_threadgroup()
                ),
            ));
        }
        if groups.0 == 0 || groups.1 == 0 || groups.2 == 0 {
            return Ok(()); // an empty grid, as a CUDA launch with no blocks
        }
        if threadgroup_bytes > self.metal.max_threadgroup_bytes {
            return Err(invalid(
                kernel,
                format!(
                    "{threadgroup_bytes} bytes of threadgroup memory; the device allows {}",
                    self.metal.max_threadgroup_bytes
                ),
            ));
        }
        self.recording.dispatches.push(Dispatch {
            kernel,
            args: args.finish(),
            groups,
            threads,
            threadgroup_bytes: threadgroup_bytes.next_multiple_of(16),
        });
        Ok(())
    }

    /// Ends recording.
    #[must_use]
    pub fn finish(self) -> V41Recording {
        self.recording
    }

    /// Runs the recording once and waits; returns the GPU seconds.
    ///
    /// # Errors
    ///
    /// Any command buffer failure.
    pub fn commit(self) -> Result<f64, RuntimeError> {
        let metal = self.metal;
        let recording = self.recording;
        metal.run(&[(&recording, 1)])
    }
}

/// Bounds the GPU work in flight from this process: one run at a time.
static RUN_LOCK: Mutex<()> = Mutex::new(());

/// The Metal device, queue, and compiled V4.1 kernels.
pub struct V41Metal {
    device: Device,
    queue: metal::CommandQueue,
    pipelines: BTreeMap<&'static str, ComputePipelineState>,
    max_threadgroup_bytes: usize,
}

impl std::fmt::Debug for V41Metal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V41Metal")
            .field("device", &self.device.name())
            .finish_non_exhaustive()
    }
}

/// `MTLBarrierScopeBuffers`.
const BARRIER_SCOPE_BUFFERS: u64 = 1;

impl V41Metal {
    /// The system's default Metal device with every V4.1 kernel compiled.
    ///
    /// # Errors
    ///
    /// No Metal device, a device without Metal 3 (GPU addresses), or a
    /// kernel that fails to compile.
    pub fn new() -> Result<Self, RuntimeError> {
        let device = Device::system_default().ok_or_else(|| invalid("v41", "no Metal device"))?;
        if !device.supports_family(metal::MTLGPUFamily::Metal3) {
            return Err(invalid("v41", "the GPU lacks Metal 3"));
        }
        let options = CompileOptions::new();
        options.set_fast_math_enabled(false);
        options.set_language_version(metal::MTLLanguageVersion::V3_1);
        // Safe math (no reassociation) and precise math functions; newer
        // SDKs read these instead of fastMathEnabled.
        // SAFETY: both selectors take an NSInteger enum; respondsToSelector:
        // guards older systems.
        unsafe {
            let responds: bool =
                msg_send![options.as_ref(), respondsToSelector: sel!(setMathMode:)];
            if responds {
                let _: () = msg_send![options.as_ref(), setMathMode: 0_i64];
                let _: () = msg_send![options.as_ref(), setMathFloatingPointFunctions: 1_i64];
            }
        }
        let library = device
            .new_library_with_source(&kernel_source(), &options)
            .map_err(|e| invalid("v41 kernels", e))?;
        let mut pipelines = BTreeMap::new();
        for name in PIPELINES {
            let function = library
                .get_function(name, None)
                .map_err(|e| invalid(name, e))?;
            let pipeline = device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|e| invalid(name, e))?;
            if pipeline.thread_execution_width() != 32 {
                return Err(invalid(name, "V4.1 kernels need 32-thread SIMD groups"));
            }
            pipelines.insert(*name, pipeline);
        }
        let queue = device.new_command_queue();
        let max_threadgroup_bytes = device.max_threadgroup_memory_length() as usize;
        Ok(Self {
            device,
            queue,
            pipelines,
            max_threadgroup_bytes,
        })
    }

    /// The GPU's name, for example `Apple M5 Max`.
    #[must_use]
    pub fn device_name(&self) -> String {
        self.device.name().to_string()
    }

    /// The newest Apple GPU family the device supports, as `apple<N>`.
    #[must_use]
    pub fn gpu_family(&self) -> String {
        use metal::MTLGPUFamily::{Apple5, Apple6, Apple7, Apple8, Apple9};
        // Apple10 has no constant in metal-rs 0.33; MTLGPUFamilyApple10 = 1010.
        // SAFETY: supportsFamily: takes an NSInteger family code.
        let apple10: bool = unsafe { msg_send![self.device.as_ref(), supportsFamily: 1010_i64] };
        if apple10 {
            return String::from("apple10");
        }
        for (family, name) in [
            (Apple9, "apple9"),
            (Apple8, "apple8"),
            (Apple7, "apple7"),
            (Apple6, "apple6"),
            (Apple5, "apple5"),
        ] {
            if self.device.supports_family(family) {
                return String::from(name);
            }
        }
        String::from("apple")
    }

    /// The memory Metal recommends a process keep resident, in bytes.
    #[must_use]
    pub fn recommended_working_set(&self) -> u64 {
        self.device.recommended_max_working_set_size()
    }

    /// Bytes currently allocated on the device by this process.
    #[must_use]
    pub fn allocated_bytes(&self) -> u64 {
        self.device.current_allocated_size()
    }

    /// A zeroed shared buffer of `bytes` bytes (at least 16).
    ///
    /// # Errors
    ///
    /// The device refuses the allocation.
    pub fn zeros(&self, bytes: usize) -> Result<V41Buffer, RuntimeError> {
        let len = bytes.max(16);
        let buffer = self
            .device
            .new_buffer(len as u64, MTLResourceOptions::StorageModeShared);
        if buffer.contents().is_null() {
            return Err(invalid("v41", format!("allocating {len} bytes failed")));
        }
        // SAFETY: a fresh shared allocation of `len` bytes.
        unsafe { std::ptr::write_bytes(buffer.contents().cast::<u8>(), 0, len) };
        Ok(V41Buffer { buffer, len: bytes })
    }

    /// A shared buffer holding `bytes`.
    ///
    /// # Errors
    ///
    /// The device refuses the allocation.
    pub fn byte_buffer(&self, bytes: &[u8]) -> Result<V41Buffer, RuntimeError> {
        let buffer = self.zeros(bytes.len())?;
        buffer.write_at(0, bytes)?;
        Ok(buffer)
    }

    /// A zeroed buffer of `len` floats.
    ///
    /// # Errors
    ///
    /// The device refuses the allocation.
    pub fn f32_buffer(&self, len: usize) -> Result<V41Buffer, RuntimeError> {
        self.zeros(len * 4)
    }

    /// A buffer holding `values`.
    ///
    /// # Errors
    ///
    /// The device refuses the allocation.
    pub fn upload_f32(&self, values: &[f32]) -> Result<V41Buffer, RuntimeError> {
        self.byte_buffer(&le_bytes(values, f32::to_le_bytes))
    }

    /// A buffer holding `values`.
    ///
    /// # Errors
    ///
    /// The device refuses the allocation.
    pub fn upload_i32(&self, values: &[i32]) -> Result<V41Buffer, RuntimeError> {
        self.byte_buffer(&le_bytes(values, i32::to_le_bytes))
    }

    /// Starts recording kernel launches.
    #[must_use]
    pub fn begin_submission(&self) -> V41Submission<'_> {
        V41Submission {
            metal: self,
            recording: V41Recording::default(),
        }
    }

    /// Encodes each recording its number of times, in order, into one
    /// command buffer, commits it, and waits. Returns the GPU seconds.
    ///
    /// # Errors
    ///
    /// The command buffer reports an error.
    pub fn run(&self, parts: &[(&V41Recording, usize)]) -> Result<f64, RuntimeError> {
        let _guard = RUN_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let command = self.queue.new_command_buffer();
        let encoder = command.new_compute_command_encoder();
        let mut seen = std::collections::BTreeSet::new();
        for (recording, _) in parts {
            for buffer in &recording.resources {
                if seen.insert(buffer.gpu_address()) {
                    encoder.use_resource(buffer, MTLResourceUsage::Read | MTLResourceUsage::Write);
                }
            }
        }
        for (recording, times) in parts {
            for _ in 0..*times {
                for d in &recording.dispatches {
                    let pipeline = &self.pipelines[d.kernel];
                    encoder.set_compute_pipeline_state(pipeline);
                    encoder.set_bytes(0, d.args.len() as u64, d.args.as_ptr().cast::<c_void>());
                    if d.threadgroup_bytes > 0 {
                        encoder.set_threadgroup_memory_length(0, d.threadgroup_bytes as u64);
                    }
                    encoder.dispatch_thread_groups(
                        MTLSize::new(d.groups.0 as u64, d.groups.1 as u64, d.groups.2 as u64),
                        MTLSize::new(d.threads as u64, 1, 1),
                    );
                    // SAFETY: memoryBarrierWithScope: takes an MTLBarrierScope.
                    unsafe {
                        let _: () =
                            msg_send![encoder, memoryBarrierWithScope: BARRIER_SCOPE_BUFFERS];
                    }
                }
            }
        }
        encoder.end_encoding();
        command.commit();
        command.wait_until_completed();
        // MTLCommandBufferStatusError = 5.
        if command.status() as u64 == 5 {
            return Err(invalid("v41", "the command buffer failed"));
        }
        // SAFETY: GPUStartTime and GPUEndTime are read-only properties of a
        // completed command buffer.
        let seconds = unsafe {
            let start: f64 = msg_send![command, GPUStartTime];
            let end: f64 = msg_send![command, GPUEndTime];
            end - start
        };
        Ok(seconds)
    }
}

#[cfg(test)]
mod checks;
