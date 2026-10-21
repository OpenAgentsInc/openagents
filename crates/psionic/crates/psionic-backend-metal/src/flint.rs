//! Flint (`RecursiveSI/flint`) on Metal with Recursive's MLX affine INT4
//! weights, read straight from the safetensors shards.
//!
//! The kernels in `kernels/flint.metal` adapt the decode arithmetic of
//! Recursive's `rec` engine (Apache-2.0); see that file's notice. The model
//! definition (configuration, template, tokenizer, sampling) lives in
//! `psionic-flint`; this module only runs the network on the GPU.
use metal::{
    Buffer, CommandBuffer, CommandBufferRef, CommandQueue, CompileOptions, ComputeCommandEncoder,
    ComputeCommandEncoderRef, ComputePipelineState, Device, MTLLanguageVersion, MTLResourceOptions,
    MTLSize,
};
use objc::{msg_send, sel, sel_impl};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::c_void,
    fs::File,
    io::Read,
    path::Path,
};

pub const HIDDEN: usize = 2048;
pub const HEADS: usize = 32;
pub const KV_HEADS: usize = 4;
pub const HEAD_DIM: usize = 128;
pub const EXPERTS: usize = 256;
pub const TOP_K: usize = 6;
pub const EXPERT_FF: usize = 512;
pub const SHARED_FF: usize = 1024;
pub const DENSE_FF: usize = 6144;
pub const WINDOW: usize = 513;
/// Largest prefill chunk; longer prompts run in several chunks.
pub const MAX_CHUNK: usize = 2048;
/// Largest top-k the GPU ranks; larger requests read every logit back.
pub const MAX_TOP_K: usize = 32;
/// Decode steps per command buffer when the GPU picks each next token.
pub const BLOCK: usize = 8;

/// How the GPU picks each next token in [`FlintMetal::decode_resident`].
#[derive(Clone, Copy, Debug)]
pub struct GpuSampling {
    /// Take the highest logit (lower ID on ties).
    pub greedy: bool,
    pub temperature: f32,
    pub top_p: f32,
    /// Candidates ranked on the GPU, at most [`MAX_TOP_K`].
    pub top_k: usize,
}

/// The result of one step: the greedy token and, when asked for, the
/// leading `k` candidates (highest logit first, lower ID first on ties).
#[derive(Clone, Debug, Default)]
pub struct Step {
    pub greedy: u32,
    pub ranked: Vec<(usize, f32)>,
}

/// A tensor inside a loaded shard: shard index and byte offset from the file start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlintTensor {
    pub shard: usize,
    pub offset: u64,
}

/// An MLX affine-quantized matrix: packed codes, group scales and group biases.
#[derive(Clone, Copy, Debug)]
pub struct FlintQuant {
    pub weight: FlintTensor,
    pub scales: FlintTensor,
    pub biases: FlintTensor,
}

#[derive(Clone, Debug)]
pub enum FlintMlp {
    Dense {
        gate: FlintQuant,
        up: FlintQuant,
        down: FlintQuant,
    },
    Sparse {
        router: FlintTensor,
        gate: FlintQuant,
        up: FlintQuant,
        down: FlintQuant,
        shared_gate: FlintQuant,
        shared_up: FlintQuant,
        shared_down: FlintQuant,
    },
}

#[derive(Clone, Debug)]
pub struct FlintLayer {
    /// Full attention without RoPE; otherwise a 513-token window with RoPE.
    pub global: bool,
    pub rotary: bool,
    pub input_norm: FlintTensor,
    pub post_norm: FlintTensor,
    pub q: FlintQuant,
    pub k: FlintQuant,
    pub v: FlintQuant,
    pub o: FlintQuant,
    pub q_norm: FlintTensor,
    pub k_norm: FlintTensor,
    pub sinks: FlintTensor,
    pub mlp: FlintMlp,
}

#[derive(Clone, Debug)]
pub struct FlintWeights {
    /// INT8 embedding (group 64).
    pub embed: FlintQuant,
    pub layers: Vec<FlintLayer>,
    pub norm: FlintTensor,
    /// INT4 vocabulary head.
    pub head: FlintQuant,
    pub vocab: usize,
}

struct Scratch {
    h: Buffer,
    n: Buffer,
    q: Buffer,
    kv_new: Buffer,
    a: Buffer,
    partial: Buffer,
    stats: Buffer,
    probs: Buffer,
    routing: Buffer,
    act: Buffer,
    logits: Buffer,
    choice: Buffer,
    cand_v: Buffer,
    cand_i: Buffer,
    top_v: Buffer,
    top_i: Buffer,
    bad: Buffer,
    /// Two token slots, so the next step can be encoded before its token is known.
    token_slots: Buffer,
    /// Per-step uniform draws and chosen tokens, two blocks' worth each.
    draws: Buffer,
    out_tokens: Buffer,
}

struct ChunkScratch {
    h: Buffer,
    n: Buffer,
    qkv: Buffer,
    a: Buffer,
    act: Buffer,
    probs: Buffer,
    ids: Buffer,
    weights: Buffer,
    counts: Buffer,
    offsets: Buffer,
    tiles: Buffer,
    slot_rows: Buffer,
    slot_of: Buffer,
    expert_act: Buffer,
    expert_down: Buffer,
    shared_act: Buffer,
    shared_down: Buffer,
    tokens: Buffer,
}

/// The loaded network. One request runs at a time; the caller serializes.
pub struct FlintMetal {
    device: Device,
    queue: CommandQueue,
    pipelines: BTreeMap<&'static str, ComputePipelineState>,
    shards: Vec<Buffer>,
    weights: FlintWeights,
    scratch: Scratch,
    chunk: ChunkScratch,
    gpu_seconds: Cell<f64>,
}

/// Request-owned KV cache and position.
pub struct FlintSession {
    kv: Vec<Buffer>,
    caps: Vec<usize>,
    position: usize,
    capacity: usize,
}

impl FlintSession {
    pub fn position(&self) -> usize {
        self.position
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DecodeParams {
    position: u32,
    cap: u32,
    rotary: u32,
    global: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SampleParams {
    greedy: u32,
    temperature: f32,
    top_p: f32,
    k: u32,
    step: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AttentionParams {
    count: u32,
    cap: u32,
    parts: u32,
    rounded: u32,
    heads: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GemmParams {
    m: u32,
    n: u32,
    k: u32,
    ldx: u32,
    ldy: u32,
    mode: u32,
    gathered: u32,
    tiles: u32,
    expert_stride_w: u32,
    expert_stride_g: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RowParams {
    start: u32,
    cap: u32,
    rotary: u32,
    global: u32,
    count: u32,
}

const PIPELINES: &[&str] = &[
    "flint_embed8",
    "flint_qkv_head",
    "flint_attention_partial",
    "flint_attention_merge",
    "flint_residual_4096",
    "flint_residual_6144",
    "flint_norm_router",
    "flint_moe_gateup",
    "flint_moe_down",
    "flint_dense_gateup",
    "flint_head",
    "flint_argmax",
    "flint_topk_partial",
    "flint_topk_merge",
    "flint_sample",
    "flint_rms_rows",
    "flint_gemm_q4",
    "flint_rows_head_norm",
    "flint_rows_store_kv",
    "flint_rows_attention",
    "flint_rows_route",
    "flint_rows_plan",
    "flint_rows_scatter",
    "flint_rows_combine",
];

fn size(x: usize, y: usize, z: usize) -> MTLSize {
    MTLSize::new(x as u64, y as u64, z as u64)
}

/// Split-K schedule for one decode attention step, as rec chooses it.
fn attention_schedule(global: bool, count: usize) -> (usize, bool, usize) {
    let rounded = global && count >= 1024;
    let parts = if !rounded {
        32
    } else if count == 1024 {
        64
    } else if count <= 8192 {
        128
    } else if count <= 32768 {
        256
    } else if count <= 65536 {
        512
    } else {
        1024
    };
    let heads = if global && count > 1024 { 8 } else { 4 };
    (parts, rounded, heads)
}

/// Kernels present only when the compiler has Metal 4 tensor operations.
const TENSOR_PIPELINES: &[&str] = &["flint_gemm_q4_tensor", "flint_rows_attention_tensor"];

/// Compile the Flint kernels, with Metal 4 tensor operations when available.
pub fn compile(device: &Device) -> Result<BTreeMap<&'static str, ComputePipelineState>, String> {
    let source = include_str!("kernels/flint.metal");
    let options = CompileOptions::new();
    options.set_fast_math_enabled(false);
    // Metal Shading Language 4.0 (0x40000); metal-rs 0.33 has no constant for it.
    // SAFETY: setLanguageVersion: takes an NSUInteger version on MTLCompileOptions.
    unsafe {
        let _: () = msg_send![options.as_ref(), setLanguageVersion: 0x40000u64];
    }
    let (library, tensor) = match device.new_library_with_source(source, &options) {
        Ok(library) => (library, true),
        Err(_) => {
            options.set_language_version(MTLLanguageVersion::V3_1);
            (device.new_library_with_source(source, &options)?, false)
        }
    };
    let mut pipelines = BTreeMap::new();
    let names = PIPELINES
        .iter()
        .chain(if tensor { TENSOR_PIPELINES } else { &[] });
    for name in names {
        let function = library.get_function(name, None)?;
        let pipeline = device.new_compute_pipeline_state_with_function(&function)?;
        if pipeline.thread_execution_width() != 32 {
            return Err("Flint kernels require 32-thread SIMD groups".into());
        }
        pipelines.insert(*name, pipeline);
    }
    Ok(pipelines)
}

fn tensor_allowed() -> bool {
    std::env::var("PSIONIC_FLINT_TENSOR").map_or(true, |v| v != "0")
}

/// The prefill matrix kernel: tensor operations unless PSIONIC_FLINT_TENSOR=0.
fn gemm_kernel(pipelines: &BTreeMap<&'static str, ComputePipelineState>) -> &'static str {
    if tensor_allowed() && pipelines.contains_key("flint_gemm_q4_tensor") {
        "flint_gemm_q4_tensor"
    } else {
        "flint_gemm_q4"
    }
}

/// Keys a chunk must reach before its attention runs on tensor operations.
/// Shorter contexts keep the simdgroup kernel, whose arithmetic stays closer
/// to rec's and costs little there.
pub const TENSOR_ATTENTION_KEYS: usize = 4096;

/// Chunk attention: the tensor-operation kernel (64 positions of one query
/// head per threadgroup) or the simdgroup kernel (8 positions of one KV head).
pub(crate) fn chunk_attention(
    pipelines: &BTreeMap<&'static str, ComputePipelineState>,
    start: usize,
    count: usize,
) -> (&'static str, (usize, usize), usize) {
    if tensor_allowed()
        && start + count > TENSOR_ATTENTION_KEYS
        && pipelines.contains_key("flint_rows_attention_tensor")
    {
        (
            "flint_rows_attention_tensor",
            (count.div_ceil(64), HEADS),
            128,
        )
    } else {
        ("flint_rows_attention", (count.div_ceil(8), KV_HEADS), 256)
    }
}

pub(crate) fn shared_buffer(device: &Device, bytes: usize) -> Buffer {
    let bytes = bytes.max(16);
    let buffer = device.new_buffer(bytes as u64, MTLResourceOptions::StorageModeShared);
    // SAFETY: a fresh shared allocation of exactly `bytes` bytes.
    unsafe { std::ptr::write_bytes(buffer.contents().cast::<u8>(), 0, bytes) };
    buffer
}

/// Per-kernel GPU time, gathered when `PSIONIC_FLINT_PROFILE` is set.
struct Profile {
    queue: CommandQueue,
    command: RefCell<CommandBuffer>,
    times: RefCell<BTreeMap<&'static str, (f64, u32)>>,
}

/// Encoder helper: binds buffers and constants, then dispatches. When
/// profiling, each dispatch runs in its own command buffer and is timed.
pub(crate) struct Pass<'a> {
    encoder: RefCell<ComputeCommandEncoder>,
    pipelines: &'a BTreeMap<&'static str, ComputePipelineState>,
    current: Cell<&'static str>,
    profile: Option<Profile>,
}

fn gpu_seconds(command: &CommandBufferRef) -> f64 {
    // SAFETY: GPUStartTime and GPUEndTime are read-only properties of a completed buffer.
    unsafe {
        let start: f64 = msg_send![command, GPUStartTime];
        let end: f64 = msg_send![command, GPUEndTime];
        end - start
    }
}

impl<'a> Pass<'a> {
    pub(crate) fn new(
        encoder: &'a ComputeCommandEncoderRef,
        pipelines: &'a BTreeMap<&'static str, ComputePipelineState>,
    ) -> Self {
        Self {
            encoder: RefCell::new(encoder.to_owned()),
            pipelines,
            current: Cell::new(""),
            profile: None,
        }
    }
    pub(crate) fn kernel(&self, name: &str) -> &Self {
        let (key, pipeline) = self
            .pipelines
            .get_key_value(name)
            .expect("known Flint kernel");
        self.current.set(key);
        self.encoder.borrow().set_compute_pipeline_state(pipeline);
        self
    }
    pub(crate) fn buf(&self, index: u64, buffer: &Buffer, offset: u64) -> &Self {
        self.encoder
            .borrow()
            .set_buffer(index, Some(buffer), offset);
        self
    }
    pub(crate) fn val<T: Copy>(&self, index: u64, value: &T) -> &Self {
        self.encoder.borrow().set_bytes(
            index,
            std::mem::size_of::<T>() as u64,
            (value as *const T).cast::<c_void>(),
        );
        self
    }
    pub(crate) fn groups(&self, groups: (usize, usize), threads: usize) {
        self.encoder
            .borrow()
            .dispatch_thread_groups(size(groups.0, groups.1, 1), size(threads, 1, 1));
        self.after_dispatch();
    }
    pub(crate) fn threads(&self, total: (usize, usize), threads: usize) {
        self.encoder
            .borrow()
            .dispatch_threads(size(total.0, total.1, 1), size(threads, 1, 1));
        self.after_dispatch();
    }
    fn after_dispatch(&self) {
        let Some(profile) = &self.profile else { return };
        self.encoder.borrow().end_encoding();
        let command = profile.command.borrow().clone();
        command.commit();
        command.wait_until_completed();
        let mut times = profile.times.borrow_mut();
        let entry = times.entry(self.current.get()).or_insert((0.0, 0));
        entry.0 += gpu_seconds(&command);
        entry.1 += 1;
        let next = profile.queue.new_command_buffer().to_owned();
        *self.encoder.borrow_mut() = next.new_compute_command_encoder().to_owned();
        *profile.command.borrow_mut() = next;
    }
}

fn read_into(buffer: &Buffer, path: &Path, bytes: usize) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| format!("opening {}: {e}", path.display()))?;
    // SAFETY: the shared allocation holds `bytes` writable bytes and is not yet used.
    let target = unsafe { std::slice::from_raw_parts_mut(buffer.contents().cast::<u8>(), bytes) };
    let mut done = 0;
    while done < bytes {
        let end = (done + (64 << 20)).min(bytes);
        file.read_exact(&mut target[done..end])
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        done = end;
    }
    Ok(())
}

impl FlintMetal {
    /// Copy each shard file into one shared Metal buffer and compile the kernels.
    pub fn load(shards: &[std::path::PathBuf], weights: FlintWeights) -> Result<Self, String> {
        let device = Device::system_default().ok_or("Metal is unavailable")?;
        let pipelines = compile(&device)?;
        let mut buffers = Vec::new();
        for path in shards {
            let bytes = std::fs::metadata(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?
                .len() as usize;
            if bytes as u64 > device.max_buffer_length() {
                return Err(format!(
                    "{} is larger than this GPU's largest buffer",
                    path.display()
                ));
            }
            let buffer = device.new_buffer(bytes as u64, MTLResourceOptions::StorageModeShared);
            read_into(&buffer, path, bytes)?;
            buffers.push(buffer);
        }
        Self::check_offsets(&weights, &buffers)?;
        let scratch = Scratch {
            h: shared_buffer(&device, HIDDEN * 2),
            n: shared_buffer(&device, HIDDEN * 2),
            q: shared_buffer(&device, HEADS * HEAD_DIM * 2),
            kv_new: shared_buffer(&device, 2 * KV_HEADS * HEAD_DIM * 2),
            a: shared_buffer(&device, HEADS * HEAD_DIM * 2),
            partial: shared_buffer(&device, HEADS * 1024 * HEAD_DIM * 4),
            stats: shared_buffer(&device, HEADS * 1024 * 2 * 4),
            probs: shared_buffer(&device, EXPERTS * 4),
            routing: shared_buffer(&device, 16 * 4),
            act: shared_buffer(&device, DENSE_FF * 2),
            logits: shared_buffer(&device, weights.vocab * 4),
            choice: shared_buffer(&device, 16),
            cand_v: shared_buffer(&device, weights.vocab.div_ceil(2048) * MAX_TOP_K * 4),
            cand_i: shared_buffer(&device, weights.vocab.div_ceil(2048) * MAX_TOP_K * 4),
            top_v: shared_buffer(&device, MAX_TOP_K * 4),
            top_i: shared_buffer(&device, MAX_TOP_K * 4),
            bad: shared_buffer(&device, 16),
            token_slots: shared_buffer(&device, 16),
            draws: shared_buffer(&device, 2 * BLOCK * 4),
            out_tokens: shared_buffer(&device, 2 * BLOCK * 4),
        };
        let c = MAX_CHUNK;
        let slots = c * TOP_K;
        let chunk = ChunkScratch {
            h: shared_buffer(&device, c * HIDDEN * 2),
            n: shared_buffer(&device, c * HIDDEN * 2),
            qkv: shared_buffer(&device, c * 5120 * 2),
            a: shared_buffer(&device, c * 4096 * 2),
            act: shared_buffer(&device, c * DENSE_FF * 2),
            probs: shared_buffer(&device, c * EXPERTS * 4),
            ids: shared_buffer(&device, slots * 4),
            weights: shared_buffer(&device, slots * 4),
            counts: shared_buffer(&device, EXPERTS * 4),
            offsets: shared_buffer(&device, (EXPERTS + 1) * 4),
            tiles: shared_buffer(&device, (1 + 3 * (slots / 64 + EXPERTS + 1)) * 4),
            slot_rows: shared_buffer(&device, slots * 4),
            slot_of: shared_buffer(&device, slots * 4),
            expert_act: shared_buffer(&device, slots * EXPERT_FF * 2),
            expert_down: shared_buffer(&device, slots * HIDDEN * 2),
            shared_act: shared_buffer(&device, c * SHARED_FF * 2),
            shared_down: shared_buffer(&device, c * HIDDEN * 2),
            tokens: shared_buffer(&device, c * 4),
        };
        Ok(Self {
            queue: device.new_command_queue(),
            device,
            pipelines,
            shards: buffers,
            weights,
            scratch,
            chunk,
            gpu_seconds: Cell::new(0.0),
        })
    }

    fn check_offsets(weights: &FlintWeights, shards: &[Buffer]) -> Result<(), String> {
        let mut tensors = vec![weights.norm];
        let quant = |q: &FlintQuant, out: &mut Vec<FlintTensor>| {
            out.extend([q.weight, q.scales, q.biases]);
        };
        quant(&weights.embed, &mut tensors);
        quant(&weights.head, &mut tensors);
        for layer in &weights.layers {
            tensors.extend([
                layer.input_norm,
                layer.post_norm,
                layer.q_norm,
                layer.k_norm,
                layer.sinks,
            ]);
            for q in [&layer.q, &layer.k, &layer.v, &layer.o] {
                quant(q, &mut tensors);
            }
            match &layer.mlp {
                FlintMlp::Dense { gate, up, down } => {
                    for q in [gate, up, down] {
                        quant(q, &mut tensors);
                    }
                }
                FlintMlp::Sparse {
                    router,
                    gate,
                    up,
                    down,
                    shared_gate,
                    shared_up,
                    shared_down,
                } => {
                    tensors.push(*router);
                    for q in [gate, up, down, shared_gate, shared_up, shared_down] {
                        quant(q, &mut tensors);
                    }
                }
            }
        }
        for t in tensors {
            let shard = shards.get(t.shard).ok_or("tensor names a missing shard")?;
            if t.offset % 8 != 0 || t.offset >= shard.length() {
                return Err("a tensor is misaligned or outside its shard".into());
            }
        }
        Ok(())
    }

    pub fn device_name(&self) -> &str {
        self.device.name()
    }

    /// GPU busy time of the commands run since the last call.
    pub fn take_gpu_seconds(&self) -> f64 {
        self.gpu_seconds.replace(0.0)
    }

    pub fn vocab(&self) -> usize {
        self.weights.vocab
    }

    pub fn layer_count(&self) -> usize {
        self.weights.layers.len()
    }

    /// Bytes of GPU memory held for weights.
    pub fn weight_bytes(&self) -> u64 {
        self.shards.iter().map(|b| b.length()).sum()
    }

    /// A session whose full-attention layers hold `capacity` positions.
    pub fn session(&self, capacity: usize) -> Result<FlintSession, String> {
        if capacity == 0 || capacity > 262_144 {
            return Err("context must be between 1 and 262,144 tokens".into());
        }
        let caps: Vec<usize> = self
            .weights
            .layers
            .iter()
            .map(|l| if l.global { capacity } else { WINDOW })
            .collect();
        let kv = caps
            .iter()
            .map(|cap| {
                self.device.new_buffer(
                    (2 * KV_HEADS * cap * HEAD_DIM * 2) as u64,
                    MTLResourceOptions::StorageModePrivate,
                )
            })
            .collect();
        Ok(FlintSession {
            kv,
            caps,
            position: 0,
            capacity,
        })
    }

    fn t(&self, t: FlintTensor) -> (&Buffer, u64) {
        (&self.shards[t.shard], t.offset)
    }

    fn bind_quant(&self, pass: &Pass<'_>, first: u64, q: &FlintQuant) {
        let (w, wo) = self.t(q.weight);
        let (s, so) = self.t(q.scales);
        let (b, bo) = self.t(q.biases);
        pass.buf(first, w, wo)
            .buf(first + 1, s, so)
            .buf(first + 2, b, bo);
    }

    fn run<F: FnOnce(&Pass<'_>)>(&self, encode: F) -> Result<(), String> {
        objc::rc::autoreleasepool(|| {
            let command = self.queue.new_command_buffer().to_owned();
            let encoder = command.new_compute_command_encoder();
            let mut pass = Pass::new(encoder, &self.pipelines);
            if std::env::var_os("PSIONIC_FLINT_PROFILE").is_some() {
                pass.profile = Some(Profile {
                    queue: self.queue.clone(),
                    command: RefCell::new(command.clone()),
                    times: RefCell::new(BTreeMap::new()),
                });
            }
            encode(&pass);
            pass.encoder.borrow().end_encoding();
            let command = match &pass.profile {
                Some(profile) => profile.command.borrow().clone(),
                None => command.clone(),
            };
            command.commit();
            command.wait_until_completed();
            if let Some(profile) = &pass.profile {
                let times = profile.times.borrow();
                let total: f64 = times.values().map(|t| t.0).sum();
                let mut rows: Vec<_> = times.iter().collect();
                rows.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
                eprintln!("flint profile: {:.2} ms GPU", total * 1e3);
                for (name, (seconds, calls)) in rows {
                    eprintln!("  {name:28} {:9.3} ms {calls:5} calls", seconds * 1e3);
                }
            }
            self.gpu_seconds
                .set(self.gpu_seconds.get() + gpu_seconds(&command));
            match command.status() {
                metal::MTLCommandBufferStatus::Completed => Ok(()),
                status => Err(format!("Metal command failed: {status:?}")),
            }
        })
    }

    fn set_token(&self, slot: usize, token: u32) {
        // SAFETY: two u32 slots in a shared buffer; the step reading this slot is not yet committed.
        unsafe { *self.scratch.token_slots.contents().cast::<u32>().add(slot) = token };
    }

    /// Encode one token at `position`; the token is read from `slot` when the step runs.
    fn encode_decode(&self, pass: &Pass<'_>, session: &FlintSession, position: usize, slot: usize) {
        let s = &self.scratch;
        let w = &self.weights;
        pass.kernel("flint_embed8");
        self.bind_quant(pass, 0, &w.embed);
        pass.buf(3, &s.h, 0)
            .buf(4, &s.token_slots, (slot * 4) as u64)
            .val(5, &1u32)
            .threads((HIDDEN, 1), 256);
        for (index, layer) in w.layers.iter().enumerate() {
            let cap = session.caps[index];
            let kv = &session.kv[index];
            let params = DecodeParams {
                position: position as u32,
                cap: cap as u32,
                rotary: layer.rotary as u32,
                global: layer.global as u32,
            };
            pass.kernel("flint_qkv_head").buf(0, &s.h, 0);
            let (b, o) = self.t(layer.input_norm);
            pass.buf(1, b, o);
            self.bind_quant(pass, 2, &layer.q);
            self.bind_quant(pass, 5, &layer.k);
            self.bind_quant(pass, 8, &layer.v);
            let (b, o) = self.t(layer.q_norm);
            pass.buf(11, b, o);
            let (b, o) = self.t(layer.k_norm);
            pass.buf(12, b, o)
                .buf(13, &s.q, 0)
                .buf(14, kv, 0)
                .buf(15, &s.kv_new, 0)
                .val(16, &params)
                .groups((40, 1), 256);
            let count = if layer.global {
                position + 1
            } else {
                (position + 1).min(cap)
            };
            let (parts, rounded, heads) = attention_schedule(layer.global, count);
            let attention = AttentionParams {
                count: count as u32,
                cap: cap as u32,
                parts: parts as u32,
                rounded: rounded as u32,
                heads: heads as u32,
            };
            let tasks = (HEADS / heads) * parts;
            let (b, o) = self.t(layer.sinks);
            pass.kernel("flint_attention_partial")
                .buf(0, &s.q, 0)
                .buf(1, kv, 0)
                .buf(2, b, o)
                .buf(3, &s.partial, 0)
                .buf(4, &s.stats, 0)
                .val(5, &attention)
                .groups((tasks.div_ceil(8), 1), 256);
            pass.kernel("flint_attention_merge")
                .buf(0, &s.a, 0)
                .buf(1, &s.partial, 0)
                .buf(2, &s.stats, 0)
                .val(3, &attention)
                .groups((32, 1), 256);
            pass.kernel("flint_residual_4096");
            self.bind_quant(pass, 0, &layer.o);
            pass.buf(3, &s.a, 0)
                .buf(4, &s.h, 0)
                .groups((2048 / 32, 1), 256);
            let (nb, no) = self.t(layer.post_norm);
            match &layer.mlp {
                FlintMlp::Dense { gate, up, down } => {
                    pass.kernel("flint_norm_router")
                        .buf(0, &s.h, 0)
                        .buf(1, nb, no)
                        .buf(2, nb, no)
                        .buf(3, &s.n, 0)
                        .buf(4, &s.probs, 0)
                        .val(5, &0u32)
                        .groups((1, 1), 256);
                    pass.kernel("flint_dense_gateup").buf(0, &s.n, 0);
                    self.bind_quant(pass, 1, gate);
                    self.bind_quant(pass, 4, up);
                    pass.buf(7, &s.act, 0).groups((DENSE_FF / 32, 1), 256);
                    pass.kernel("flint_residual_6144");
                    self.bind_quant(pass, 0, down);
                    pass.buf(3, &s.act, 0)
                        .buf(4, &s.h, 0)
                        .groups((2048 / 32, 1), 256);
                }
                FlintMlp::Sparse {
                    router,
                    gate,
                    up,
                    down,
                    shared_gate,
                    shared_up,
                    shared_down,
                } => {
                    let (rb, ro) = self.t(*router);
                    pass.kernel("flint_norm_router")
                        .buf(0, &s.h, 0)
                        .buf(1, nb, no)
                        .buf(2, rb, ro)
                        .buf(3, &s.n, 0)
                        .buf(4, &s.probs, 0)
                        .val(5, &1u32)
                        .groups((EXPERTS / 8, 1), 256);
                    pass.kernel("flint_moe_gateup")
                        .buf(0, &s.n, 0)
                        .buf(1, &s.probs, 0);
                    self.bind_quant(pass, 2, gate);
                    self.bind_quant(pass, 5, up);
                    self.bind_quant(pass, 8, shared_gate);
                    self.bind_quant(pass, 11, shared_up);
                    pass.buf(14, &s.act, 0)
                        .buf(15, &s.routing, 0)
                        .groups((4096 / 16, 1), 256);
                    pass.kernel("flint_moe_down")
                        .buf(0, &s.act, 0)
                        .buf(1, &s.routing, 0);
                    self.bind_quant(pass, 2, down);
                    self.bind_quant(pass, 5, shared_down);
                    pass.buf(8, &s.h, 0).groups((2048 / 32, 1), 256);
                }
            }
        }
        let vocab = w.vocab as u32;
        pass.kernel("flint_head").buf(0, &s.h, 0);
        let (b, o) = self.t(w.norm);
        pass.buf(1, b, o);
        self.bind_quant(pass, 2, &w.head);
        pass.buf(5, &s.logits, 0)
            .val(6, &vocab)
            .groups((w.vocab.div_ceil(32), 1), 256);
        pass.kernel("flint_argmax")
            .buf(0, &s.logits, 0)
            .buf(1, &s.choice, 0)
            .val(2, &vocab)
            .groups((1, 1), 1024);
    }

    fn check_room(&self, session: &FlintSession, count: usize) -> Result<(), String> {
        if session.position + count > session.capacity {
            Err("the request exceeds the session context".into())
        } else {
            Ok(())
        }
    }

    /// Run one token; ranks the next token's top `k` candidates on the GPU
    /// when `k` is at most [`MAX_TOP_K`]. Logits stay readable with `logits`.
    pub fn decode(&self, session: &mut FlintSession, token: u32, k: usize) -> Result<Step, String> {
        self.check_room(session, 1)?;
        if token as usize >= self.weights.vocab {
            return Err("token outside the vocabulary".into());
        }
        self.reset_flag();
        self.set_token(0, token);
        self.run(|pass| {
            self.encode_decode(pass, session, session.position, 0);
            self.encode_top_k(pass, k);
        })?;
        session.position += 1;
        self.step(k)
    }

    /// Decode up to `steps` tokens starting with `first`. Each step's command
    /// buffer is encoded while the previous one runs; `next` sees each step's
    /// result and returns the following token, or `None` to stop.
    pub fn decode_stream(
        &self,
        session: &mut FlintSession,
        first: u32,
        steps: usize,
        k: usize,
        mut next: impl FnMut(&Step) -> Result<Option<u32>, String>,
    ) -> Result<(), String> {
        if steps == 0 {
            return Ok(());
        }
        self.check_room(session, steps)?;
        let start = session.position;
        let session_ref: &FlintSession = session;
        let encode = |position: usize, slot: usize| {
            objc::rc::autoreleasepool(|| {
                let command = self.queue.new_command_buffer().to_owned();
                let encoder = command.new_compute_command_encoder();
                let pass = Pass::new(encoder, &self.pipelines);
                self.encode_decode(&pass, session_ref, position, slot);
                self.encode_top_k(&pass, k);
                pass.encoder.borrow().end_encoding();
                command
            })
        };
        let check = |token: u32| {
            if token as usize >= self.weights.vocab {
                Err("token outside the vocabulary".to_string())
            } else {
                Ok(token)
            }
        };
        // Steps that finished on the GPU; the session advances by this many.
        let mut done = 0;
        let outcome = (|| {
            self.reset_flag();
            self.set_token(0, check(first)?);
            let mut running = encode(start, 0);
            running.commit();
            for step in 1..=steps {
                let following = (step < steps).then(|| encode(start + step, step % 2));
                running.wait_until_completed();
                self.gpu_seconds
                    .set(self.gpu_seconds.get() + gpu_seconds(&running));
                if running.status() != metal::MTLCommandBufferStatus::Completed {
                    return Err(format!("Metal command failed: {:?}", running.status()));
                }
                done = step;
                let result = self.step(k)?;
                let Some(token) = next(&result)? else {
                    return Ok(());
                };
                let Some(command) = following else {
                    return Ok(());
                };
                self.set_token(step % 2, check(token)?);
                self.reset_flag();
                command.commit();
                running = command;
            }
            Ok(())
        })();
        session.position = start + done;
        outcome
    }

    fn reset_flag(&self) {
        // SAFETY: the shared flag is idle between commands.
        unsafe { *self.scratch.bad.contents().cast::<u32>() = 0 };
    }

    /// Decode up to `steps` tokens starting with `first`, with the GPU picking
    /// each next token and feeding it to the following step, [`BLOCK`] steps
    /// per command buffer and two buffers in flight. `draw` supplies one
    /// uniform draw per sampled step, in order. `block` sees each block's
    /// tokens and returns whether to continue; steps already running past a
    /// stop are discarded.
    pub fn decode_resident(
        &self,
        session: &mut FlintSession,
        first: u32,
        steps: usize,
        sampling: GpuSampling,
        mut draw: impl FnMut() -> f32,
        mut block: impl FnMut(&[u32]) -> Result<bool, String>,
    ) -> Result<(), String> {
        if steps == 0 {
            return Ok(());
        }
        self.check_room(session, steps)?;
        if first as usize >= self.weights.vocab {
            return Err("token outside the vocabulary".into());
        }
        let greedy = sampling.greedy || sampling.temperature == 0.0;
        if !greedy && !(1..=MAX_TOP_K).contains(&sampling.top_k) {
            return Err(format!("GPU sampling ranks at most {MAX_TOP_K} candidates"));
        }
        let start = session.position;
        let session_ref: &FlintSession = session;
        let s = &self.scratch;
        let mut encode = |parity: usize, position: usize, count: usize| {
            if !greedy {
                for i in 0..count {
                    let value = draw();
                    // SAFETY: this parity's draws are idle until its block commits.
                    unsafe {
                        *s.draws.contents().cast::<f32>().add(parity * BLOCK + i) = value;
                    }
                }
            }
            objc::rc::autoreleasepool(|| {
                let command = self.queue.new_command_buffer().to_owned();
                let encoder = command.new_compute_command_encoder();
                let pass = Pass::new(encoder, &self.pipelines);
                for i in 0..count {
                    self.encode_decode(&pass, session_ref, position + i, 0);
                    if !greedy {
                        self.encode_top_k(&pass, sampling.top_k);
                    }
                    let params = SampleParams {
                        greedy: greedy as u32,
                        temperature: sampling.temperature,
                        top_p: sampling.top_p,
                        k: sampling.top_k as u32,
                        step: (parity * BLOCK + i) as u32,
                    };
                    pass.kernel("flint_sample")
                        .buf(0, &s.top_v, 0)
                        .buf(1, &s.top_i, 0)
                        .buf(2, &s.choice, 0)
                        .buf(3, &s.draws, 0)
                        .buf(4, &s.token_slots, 0)
                        .buf(5, &s.out_tokens, 0)
                        .val(6, &params)
                        .groups((1, 1), 32);
                }
                pass.encoder.borrow().end_encoding();
                command.commit();
                command
            })
        };
        self.reset_flag();
        self.set_token(0, first);
        let mut flight = std::collections::VecDeque::new();
        let mut scheduled = 0;
        for parity in 0..2 {
            if scheduled < steps {
                let count = BLOCK.min(steps - scheduled);
                flight.push_back((encode(parity, start + scheduled, count), parity, count));
                scheduled += count;
            }
        }
        let mut executed = 0;
        let mut outcome = Ok(());
        let mut stopping = false;
        while let Some((command, parity, count)) = flight.pop_front() {
            command.wait_until_completed();
            self.gpu_seconds
                .set(self.gpu_seconds.get() + gpu_seconds(&command));
            executed += count;
            if command.status() != metal::MTLCommandBufferStatus::Completed {
                outcome = Err(format!("Metal command failed: {:?}", command.status()));
                stopping = true;
            }
            if stopping {
                continue;
            }
            // SAFETY: the shared flag and this parity's tokens were written by the completed block.
            let tokens = unsafe {
                if *s.bad.contents().cast::<u32>() != 0 {
                    outcome = Err("the model produced non-finite logits".into());
                    stopping = true;
                    continue;
                }
                std::slice::from_raw_parts(
                    s.out_tokens.contents().cast::<u32>().add(parity * BLOCK),
                    count,
                )
                .to_vec()
            };
            match block(&tokens) {
                Ok(true) => {}
                Ok(false) => {
                    stopping = true;
                    continue;
                }
                Err(error) => {
                    outcome = Err(error);
                    stopping = true;
                    continue;
                }
            }
            if scheduled < steps {
                let count = BLOCK.min(steps - scheduled);
                flight.push_back((encode(parity, start + scheduled, count), parity, count));
                scheduled += count;
            }
        }
        drop(encode);
        session.position = start + executed;
        outcome
    }

    fn encode_top_k(&self, pass: &Pass<'_>, k: usize) {
        if k == 0 || k > MAX_TOP_K {
            return;
        }
        let s = &self.scratch;
        let vocab = self.weights.vocab as u32;
        let slices = self.weights.vocab.div_ceil(2048);
        let (k32, count) = (k as u32, (slices * k) as u32);
        pass.kernel("flint_topk_partial")
            .buf(0, &s.logits, 0)
            .buf(1, &s.cand_v, 0)
            .buf(2, &s.cand_i, 0)
            .buf(3, &s.bad, 0)
            .val(4, &vocab)
            .val(5, &k32)
            .groups((slices, 1), 256);
        pass.kernel("flint_topk_merge")
            .buf(0, &s.cand_v, 0)
            .buf(1, &s.cand_i, 0)
            .buf(2, &s.top_v, 0)
            .buf(3, &s.top_i, 0)
            .val(4, &count)
            .val(5, &k32)
            .groups((1, 1), 256);
    }

    fn step(&self, k: usize) -> Result<Step, String> {
        let s = &self.scratch;
        // SAFETY: the command completed; these shared buffers hold the values read.
        unsafe {
            let greedy = *s.choice.contents().cast::<u32>();
            if k == 0 || k > MAX_TOP_K {
                return Ok(Step {
                    greedy,
                    ranked: Vec::new(),
                });
            }
            if *s.bad.contents().cast::<u32>() != 0 {
                return Err("the model produced non-finite logits".into());
            }
            let v = std::slice::from_raw_parts(s.top_v.contents().cast::<f32>(), k);
            let i = std::slice::from_raw_parts(s.top_i.contents().cast::<u32>(), k);
            Ok(Step {
                greedy,
                ranked: i.iter().zip(v).map(|(i, v)| (*i as usize, *v)).collect(),
            })
        }
    }

    /// Logits of the last decoded or prefilled token (BF16 values as FP32).
    pub fn logits(&self, out: &mut Vec<f32>) {
        out.clear();
        // SAFETY: the shared buffer holds `vocab` floats written by a completed command.
        out.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                self.scratch.logits.contents().cast::<f32>(),
                self.weights.vocab,
            )
        });
    }

    /// Prefill `tokens` in chunks; returns the next-token step after the last one.
    pub fn prefill(
        &self,
        session: &mut FlintSession,
        tokens: &[u32],
        k: usize,
    ) -> Result<Step, String> {
        if tokens.is_empty() {
            return Err("an empty prompt".into());
        }
        self.check_room(session, tokens.len())?;
        if tokens.iter().any(|t| *t as usize >= self.weights.vocab) {
            return Err("token outside the vocabulary".into());
        }
        self.reset_flag();
        let chunks = tokens.chunks(MAX_CHUNK).count();
        for (index, chunk) in tokens.chunks(MAX_CHUNK).enumerate() {
            let rank = if index + 1 == chunks { k } else { 0 };
            if chunk.len() == 1 {
                self.set_token(0, chunk[0]);
                self.run(|pass| {
                    self.encode_decode(pass, session, session.position, 0);
                    self.encode_top_k(pass, rank);
                })?;
            } else {
                // SAFETY: the shared buffer holds MAX_CHUNK u32 values and is idle.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        chunk.as_ptr(),
                        self.chunk.tokens.contents().cast::<u32>(),
                        chunk.len(),
                    );
                }
                self.run(|pass| {
                    self.encode_chunk(pass, session, chunk.len());
                    self.encode_top_k(pass, rank);
                })?;
            }
            session.position += chunk.len();
        }
        self.step(k)
    }

    #[allow(clippy::too_many_arguments)]
    fn gemm(
        &self,
        pass: &Pass<'_>,
        x: (&Buffer, u64),
        w: &FlintQuant,
        w2: Option<&FlintQuant>,
        y: (&Buffer, u64),
        params: GemmParams,
        tile_rows: usize,
    ) {
        let c = &self.chunk;
        pass.kernel(gemm_kernel(&self.pipelines)).buf(0, x.0, x.1);
        self.bind_quant(pass, 1, w);
        self.bind_quant(pass, 4, w2.unwrap_or(w));
        pass.buf(7, y.0, y.1)
            .val(8, &params)
            .buf(9, &c.slot_rows, 0)
            .buf(10, &c.tiles, 0)
            .groups(
                (
                    (params.n as usize).div_ceil(if params.mode == 2 { 32 } else { 64 }),
                    tile_rows,
                ),
                128,
            );
    }

    fn encode_chunk(&self, pass: &Pass<'_>, session: &FlintSession, count: usize) {
        let c = &self.chunk;
        let w = &self.weights;
        let start = session.position;
        let t = count as u32;
        pass.kernel("flint_embed8");
        self.bind_quant(pass, 0, &w.embed);
        pass.buf(3, &c.h, 0)
            .buf(4, &c.tokens, 0)
            .val(5, &t)
            .threads((HIDDEN, count), 256);
        let row_tiles = count.div_ceil(64);
        let dense = |m: usize, n: usize, k: usize, ldx: usize, ldy: usize, mode: u32| GemmParams {
            m: m as u32,
            n: n as u32,
            k: k as u32,
            ldx: ldx as u32,
            ldy: ldy as u32,
            mode,
            gathered: 0,
            tiles: 0,
            expert_stride_w: 0,
            expert_stride_g: 0,
        };
        for (index, layer) in w.layers.iter().enumerate() {
            let cap = session.caps[index];
            let kv = &session.kv[index];
            let (nb, no) = self.t(layer.input_norm);
            pass.kernel("flint_rms_rows")
                .buf(0, &c.h, 0)
                .buf(1, nb, no)
                .buf(2, &c.n, 0)
                .groups((count, 1), 256);
            for (q, n, column) in [
                (&layer.q, 4096, 0),
                (&layer.k, 512, 4096),
                (&layer.v, 512, 4608),
            ] {
                self.gemm(
                    pass,
                    (&c.n, 0),
                    q,
                    None,
                    (&c.qkv, column * 2),
                    dense(count, n, HIDDEN, HIDDEN, 5120, 0),
                    row_tiles,
                );
            }
            let rows = RowParams {
                start: start as u32,
                cap: cap as u32,
                rotary: layer.rotary as u32,
                global: layer.global as u32,
                count: t,
            };
            let (qn, qo) = self.t(layer.q_norm);
            let (kn, ko) = self.t(layer.k_norm);
            pass.kernel("flint_rows_head_norm")
                .buf(0, &c.qkv, 0)
                .buf(1, qn, qo)
                .buf(2, kn, ko)
                .val(3, &rows)
                .groups(((count * 36).div_ceil(8), 1), 256);
            let window = WINDOW as u32;
            let (sb, so) = self.t(layer.sinks);
            let (attention, grid, threads) = chunk_attention(&self.pipelines, start, count);
            pass.kernel(attention)
                .buf(0, &c.qkv, 0)
                .buf(1, kv, 0)
                .buf(2, sb, so)
                .buf(3, &c.a, 0)
                .val(4, &rows)
                .val(5, &window)
                .groups(grid, threads);
            pass.kernel("flint_rows_store_kv")
                .buf(0, &c.qkv, 0)
                .buf(1, kv, 0)
                .val(2, &rows)
                .threads((1024, count), 256);
            self.gemm(
                pass,
                (&c.a, 0),
                &layer.o,
                None,
                (&c.h, 0),
                dense(count, HIDDEN, 4096, 4096, HIDDEN, 1),
                row_tiles,
            );
            let (pb, po) = self.t(layer.post_norm);
            pass.kernel("flint_rms_rows")
                .buf(0, &c.h, 0)
                .buf(1, pb, po)
                .buf(2, &c.n, 0)
                .groups((count, 1), 256);
            match &layer.mlp {
                FlintMlp::Dense { gate, up, down } => {
                    self.gemm(
                        pass,
                        (&c.n, 0),
                        gate,
                        Some(up),
                        (&c.act, 0),
                        dense(count, DENSE_FF, HIDDEN, HIDDEN, DENSE_FF, 2),
                        row_tiles,
                    );
                    self.gemm(
                        pass,
                        (&c.act, 0),
                        down,
                        None,
                        (&c.h, 0),
                        dense(count, HIDDEN, DENSE_FF, DENSE_FF, HIDDEN, 1),
                        row_tiles,
                    );
                }
                FlintMlp::Sparse {
                    router,
                    gate,
                    up,
                    down,
                    shared_gate,
                    shared_up,
                    shared_down,
                } => {
                    // Router logits as a BF16 matrix product; mode 3 applies the sigmoid.
                    let router_quant = FlintQuant {
                        weight: *router,
                        scales: *router,
                        biases: *router,
                    };
                    self.gemm(
                        pass,
                        (&c.n, 0),
                        &router_quant,
                        None,
                        (&c.probs, 0),
                        dense(count, EXPERTS, HIDDEN, HIDDEN, EXPERTS, 3),
                        row_tiles,
                    );
                    pass.kernel("flint_rows_route")
                        .buf(0, &c.probs, 0)
                        .buf(1, &c.ids, 0)
                        .buf(2, &c.weights, 0)
                        .buf(3, &c.counts, 0)
                        .val(4, &t)
                        .groups((count.div_ceil(8), 1), 256);
                    pass.kernel("flint_rows_plan")
                        .buf(0, &c.counts, 0)
                        .buf(1, &c.offsets, 0)
                        .buf(2, &c.tiles, 0)
                        .groups((1, 1), 32);
                    pass.kernel("flint_rows_scatter")
                        .buf(0, &c.ids, 0)
                        .buf(1, &c.offsets, 0)
                        .buf(2, &c.slot_rows, 0)
                        .buf(3, &c.slot_of, 0)
                        .val(4, &t)
                        .threads((EXPERTS, 1), 256);
                    let slots = count * TOP_K;
                    let max_tiles = slots / 64 + EXPERTS;
                    let expert =
                        |n: usize, k: usize, ldx: usize, ldy: usize, mode: u32, gathered: u32| {
                            GemmParams {
                                m: slots as u32,
                                n: n as u32,
                                k: k as u32,
                                ldx: ldx as u32,
                                ldy: ldy as u32,
                                mode,
                                gathered,
                                tiles: max_tiles as u32,
                                expert_stride_w: (n * k / 2) as u32,
                                expert_stride_g: (n * k / 64) as u32,
                            }
                        };
                    self.gemm(
                        pass,
                        (&c.n, 0),
                        gate,
                        Some(up),
                        (&c.expert_act, 0),
                        expert(EXPERT_FF, HIDDEN, HIDDEN, EXPERT_FF, 2, 1),
                        max_tiles,
                    );
                    self.gemm(
                        pass,
                        (&c.expert_act, 0),
                        down,
                        None,
                        (&c.expert_down, 0),
                        expert(HIDDEN, EXPERT_FF, EXPERT_FF, HIDDEN, 0, 2),
                        max_tiles,
                    );
                    self.gemm(
                        pass,
                        (&c.n, 0),
                        shared_gate,
                        Some(shared_up),
                        (&c.shared_act, 0),
                        dense(count, SHARED_FF, HIDDEN, HIDDEN, SHARED_FF, 2),
                        row_tiles,
                    );
                    self.gemm(
                        pass,
                        (&c.shared_act, 0),
                        shared_down,
                        None,
                        (&c.shared_down, 0),
                        dense(count, HIDDEN, SHARED_FF, SHARED_FF, HIDDEN, 0),
                        row_tiles,
                    );
                    pass.kernel("flint_rows_combine")
                        .buf(0, &c.expert_down, 0)
                        .buf(1, &c.shared_down, 0)
                        .buf(2, &c.slot_of, 0)
                        .buf(3, &c.weights, 0)
                        .buf(4, &c.h, 0)
                        .val(5, &t)
                        .threads((HIDDEN, count), 256);
                }
            }
        }
        let s = &self.scratch;
        let vocab = w.vocab as u32;
        pass.kernel("flint_head")
            .buf(0, &c.h, ((count - 1) * HIDDEN * 2) as u64);
        let (b, o) = self.t(w.norm);
        pass.buf(1, b, o);
        self.bind_quant(pass, 2, &w.head);
        pass.buf(5, &s.logits, 0)
            .val(6, &vocab)
            .groups((w.vocab.div_ceil(32), 1), 256);
        pass.kernel("flint_argmax")
            .buf(0, &s.logits, 0)
            .buf(1, &s.choice, 0)
            .val(2, &vocab)
            .groups((1, 1), 1024);
    }
}

#[cfg(test)]
mod tests;
