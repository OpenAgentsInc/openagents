//! A device-memory cache of DeepSeek-V4.1 routed experts (#12463).
//!
//! `--experts host` (#12418) keeps every routed expert in page-locked host
//! memory and reads the selected ones over the bus. This module adds a cache
//! of the most-used experts in device memory, with the rest computed on the
//! host CPU or read over the bus, so one GPU with less memory than the model
//! serves all 40 layers.
//!
//! - **Slots.** Each layer gets the same number of device slots per routed
//!   projection. A per-layer slot map on the device gives each expert a slot
//!   ([`V41_LANE_BUS`] or [`V41_LANE_HOST`] when it isn't cached), so the
//!   matrix-vector kernel picks each selected expert's weights on the device
//!   and the decode step stays one captured graph.
//! - **Host lane.** Experts on [`V41_LANE_HOST`] are computed by CPU threads
//!   in the GPU kernel's arithmetic order ([`crate::v41_expert_lane`]), so the
//!   output is bit-equal. After routing, a kernel posts the token's FP8
//!   activation and the selected ids to a mailbox in page-locked memory; a
//!   poller thread computes the host-lane experts while the GPU runs the
//!   cached ones; a second kernel waits for the host's answer before the
//!   expert sum. The owner allowed host arithmetic inside a GPU stage on
//!   2026-10-21; whole CPU stages stay excluded.
//! - **Swaps.** The step logs every selected id. Each expert's heat counts
//!   its uses and decays by [`V41SwapPolicy::decay`] every
//!   [`V41SwapPolicy::window`] tokens. A host expert replaces the coldest
//!   cached expert of its layer when its heat passes that expert's heat ×
//!   [`V41SwapPolicy::ratio`] + [`V41SwapPolicy::margin`]. A thread copies the
//!   incoming expert into page-locked staging during a step; the copy into
//!   its slot and the slot-map change happen between steps, when no graph
//!   reads them.
//!
//! The weights and the arithmetic are the same in every lane, so a cached
//! stage answers exactly as a stage with every expert in device memory.
//!
//! The design follows Project Maya (`mw00/project-maya` at `451f9cb6a2`,
//! `src/ds4/ds4_model.cu`: profile-seeded placement, decayed-frequency swaps
//! by heat × 1.25 + 1.0 with decay 0.9 every 256 tokens, and the host
//! mailbox), reimplemented in Rust; MIT.

use crate::clef_prefill::{int, span};
use crate::v41_expert_lane::{
    V41DecodedActivation, V41ExpertDims, V41ExpertWeights, V41LaneIsa, v41_expert_down,
    v41_expert_gate_up, v41_expert_hidden,
};
use crate::v41_matvec::{V41_BLOCK, V41MatvecFormat, V41Output, V41Slots, check_v41_matvec_dims};
use crate::{CudaBackend, CudaBuffer, CudaHostBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use rayon::prelude::*;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering, fence};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// A slot-map entry for an expert read from host memory over the bus.
pub const V41_LANE_BUS: i32 = -1;
/// A slot-map entry for an expert the host CPU computes.
pub const V41_LANE_HOST: i32 = -2;
/// Selected experts the mailbox carries at most.
pub const V41_LANE_MAX_SLOTS: usize = 8;

// The mailbox layout of `v41_expert_cache.cu`.
const POST: usize = 0;
const DONE: usize = 4;
const LAYER: usize = 8;
const MASK: usize = 12;
const ERROR: usize = 16;
const IDS: usize = 32;
const WEIGHTS: usize = 64;
const PAYLOAD: usize = 128;

unsafe extern "C" {
    fn psionic_v41_block_matvec_cached(
        weights: *const c_void,
        weight_scales: *const c_void,
        cache_weights: *const c_void,
        cache_scales: *const c_void,
        slot_map: *const c_void,
        matrix_bytes: i64,
        matrix_scale_bytes: i64,
        experts: i32,
        ids: *const c_void,
        act: *const c_void,
        act_scales: *const c_void,
        act_shared: i32,
        out: *mut c_void,
        rows: i32,
        cols: i32,
        slots: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_lane_post(
        ids: *const c_void,
        slot_map: *const c_void,
        experts: i32,
        route_weights: *const c_void,
        act: *const c_void,
        act_scales: *const c_void,
        hidden: i32,
        top_k: i32,
        mailbox: *mut c_void,
        state: *mut c_void,
        layer: i32,
        index: *const c_void,
        ids_log: *mut c_void,
        log_layers: i32,
        log_slot: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_lane_wait(
        mailbox: *mut c_void,
        state: *mut c_void,
        host_rows: *const c_void,
        out: *mut c_void,
        hidden: i32,
        top_k: i32,
        timeout_ns: u64,
        stream: *mut c_void,
    ) -> i32;
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Backend(format!("v41 expert cache: {}", message.into()))
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Where cache misses run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V41MissLane {
    /// Read from page-locked host memory over the bus (needs host experts).
    Bus,
    /// Computed by host CPU threads.
    Host,
    /// Measured at load: the bus only when it's at least 3% faster per
    /// expert, as Maya decides, and only when the experts are page-locked.
    Auto,
}

impl V41MissLane {
    /// Parses `bus`, `host`, or `auto`.
    ///
    /// # Errors
    ///
    /// Refuses anything else.
    pub fn parse(value: &str) -> Result<Self, RuntimeError> {
        match value {
            "bus" | "pcie" => Ok(Self::Bus),
            "host" | "cpu" => Ok(Self::Host),
            "auto" => Ok(Self::Auto),
            other => Err(invalid(format!(
                "miss lane {other}: wants bus, host, or auto"
            ))),
        }
    }
}

/// When a host expert replaces a cached one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41SwapPolicy {
    /// Heat multiplier applied every `window` tokens (0.9).
    pub decay: f32,
    /// Tokens between decays (256).
    pub window: usize,
    /// A host expert must pass the coldest cached expert's heat × `ratio`
    /// + `margin` (1.25 and 1.0).
    pub ratio: f32,
    pub margin: f32,
    /// Swaps applied between two steps, at most.
    pub max_swaps_per_step: usize,
    /// Experts staged in page-locked memory at once (8).
    pub staging: usize,
}

impl Default for V41SwapPolicy {
    fn default() -> Self {
        Self {
            decay: 0.9,
            window: 256,
            ratio: 1.25,
            margin: 1.0,
            max_swaps_per_step: 4,
            staging: 8,
        }
    }
}

/// The cache a stage loads with.
#[derive(Clone, Debug)]
pub struct V41ExpertCacheConfig {
    /// Device slots per layer for each routed projection; 0 fills the
    /// device memory left at load less `reserve_bytes`.
    pub slots_per_layer: usize,
    /// Device memory an automatic size leaves free, for the step graphs and
    /// the runtime (3 GiB).
    pub reserve_bytes: usize,
    pub miss_lane: V41MissLane,
    pub policy: V41SwapPolicy,
    /// Starting heat per model layer, one value per expert (a routing
    /// profile). The hottest experts fill the slots; layers without a
    /// profile start with experts 0, 1, ...
    pub profile: BTreeMap<usize, Vec<f32>>,
    /// Host-lane threads; 0 means one per available core.
    pub host_threads: usize,
    /// How long a step's wait for the host lane may take before the step
    /// fails.
    pub wait_timeout: Duration,
    /// Turn off swaps (a fixed placement, for measurement).
    pub frozen: bool,
}

impl V41ExpertCacheConfig {
    /// A cache of `slots_per_layer` experts per layer with Maya's swap
    /// policy and the miss lane chosen at load.
    #[must_use]
    pub fn new(slots_per_layer: usize) -> Self {
        Self {
            slots_per_layer,
            reserve_bytes: 3 << 30,
            miss_lane: V41MissLane::Auto,
            policy: V41SwapPolicy::default(),
            profile: BTreeMap::new(),
            host_threads: 0,
            wait_timeout: Duration::from_secs(30),
            frozen: false,
        }
    }
}

/// Routed-expert bytes a host lane reads: each projection's packed FP4
/// weights and UE8M0 scales in the checkpoint's layout.
pub trait V41ExpertBytes: Send + Sync {
    /// Projection `projection` (0 for w1, 1 for w2, 2 for w3) of `expert` in
    /// model layer `layer`: `(weights, scales)`.
    ///
    /// # Errors
    ///
    /// Refuses an expert the source doesn't hold.
    fn projection(
        &self,
        layer: usize,
        expert: usize,
        projection: usize,
    ) -> Result<(&[u8], &[u8]), RuntimeError>;
}

/// Routed experts copied into ordinary memory, for small models and tests.
#[derive(Debug, Default)]
pub struct V41OwnedExperts {
    parts: BTreeMap<(usize, usize, usize), (Vec<u8>, Vec<u8>)>,
}

impl V41OwnedExperts {
    /// Reads `layers.L.ffn.experts.E.w{1,2,3}.{weight,scale}` for every
    /// layer in `layers` and expert below `experts` from `source`.
    ///
    /// # Errors
    ///
    /// Refuses a missing tensor.
    pub fn from_source(
        source: &dyn crate::v41_stage::V41TensorSource,
        layers: std::ops::Range<usize>,
        experts: usize,
    ) -> Result<Self, RuntimeError> {
        let mut parts = BTreeMap::new();
        for layer in layers {
            for expert in 0..experts {
                for (p, w) in ["w1", "w2", "w3"].iter().enumerate() {
                    let stem = format!("layers.{layer}.ffn.experts.{expert}.{w}");
                    let weights = source.tensor(&format!("{stem}.weight"))?.bytes;
                    let scales = source.tensor(&format!("{stem}.scale"))?.bytes;
                    parts.insert((layer, expert, p), (weights, scales));
                }
            }
        }
        Ok(Self { parts })
    }
}

impl V41ExpertBytes for V41OwnedExperts {
    fn projection(
        &self,
        layer: usize,
        expert: usize,
        projection: usize,
    ) -> Result<(&[u8], &[u8]), RuntimeError> {
        self.parts
            .get(&(layer, expert, projection))
            .map(|(w, s)| (w.as_slice(), s.as_slice()))
            .ok_or_else(|| {
                invalid(format!(
                    "no expert {expert} w{} in layer {layer}",
                    projection + 1
                ))
            })
    }
}

// ---------------------------------------------------------------------------
// The cached matrix-vector product
// ---------------------------------------------------------------------------

/// A routed projection's experts split between device slots and host
/// memory.
#[derive(Clone, Copy, Debug)]
pub struct V41CachedStack<'a> {
    /// Output rows and input values of one expert's matrix.
    pub rows: usize,
    pub cols: usize,
    /// Experts the slot map covers.
    pub experts: usize,
    /// Every expert, page-locked, which the bus lane reads; `None` when no
    /// expert is on the bus lane.
    pub host: Option<(&'a CudaBuffer, &'a CudaBuffer)>,
    /// The device slots' weights and scales.
    pub cache: (&'a CudaBuffer, &'a CudaBuffer),
    pub cache_slots: usize,
    /// One `i32` per expert: a slot, [`V41_LANE_BUS`], or [`V41_LANE_HOST`].
    pub slot_map: &'a CudaBuffer,
}

impl CudaSubmission {
    /// [`CudaSubmission::v41_block_matvec`] over a [`V41CachedStack`] with FP4
    /// weights: each slot's expert id is looked up in the slot map, and
    /// host-lane experts are skipped (their output rows are left as they
    /// are for [`CudaSubmission::v41_lane_wait`]).
    ///
    /// # Errors
    ///
    /// Refuses bad dimensions and buffers too small for them.
    pub fn v41_block_matvec_cached(
        &mut self,
        stack: V41CachedStack<'_>,
        codes: &CudaBuffer,
        scales: &CudaBuffer,
        slots: V41Slots<'_>,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let format = V41MatvecFormat::Fp4E2M1Row32;
        let (rows, cols) = (stack.rows, stack.cols);
        check_v41_matvec_dims(rows, cols, stack.experts, slots.count)?;
        let matrix_bytes = format.weight_bytes(rows, cols);
        let scale_bytes = format.scale_bytes(rows, cols);
        let ids = slots
            .ids
            .ok_or_else(|| invalid("a cached product needs ids"))?;
        let (host_w, host_s) = match stack.host {
            Some((w, s)) => (
                span(w, 0, matrix_bytes * stack.experts, 1, "host weights")?.cast_const(),
                span(s, 0, scale_bytes * stack.experts, 1, "host scales")?.cast_const(),
            ),
            None => (std::ptr::null(), std::ptr::null()),
        };
        let slots_n = stack.cache_slots.max(1);
        let cache_w = span(stack.cache.0, 0, matrix_bytes * slots_n, 1, "cache weights")?;
        let cache_s = span(stack.cache.1, 0, scale_bytes * slots_n, 1, "cache scales")?;
        let inputs = if slots.shared_input { 1 } else { slots.count };
        let act = span(codes, 0, inputs * cols, 1, "activation codes")?;
        let act_scales = span(scales, 0, inputs * cols / V41_BLOCK, 1, "activation scales")?;
        let to_i64 = |n: usize| i64::try_from(n).map_err(|_| invalid(format!("{n} exceeds i64")));
        // SAFETY: every extent was checked above; the slot map holds slots
        // below `cache_slots`, which the cache owner guarantees.
        let code = unsafe {
            psionic_v41_block_matvec_cached(
                host_w,
                host_s,
                cache_w.cast_const(),
                cache_s.cast_const(),
                span(stack.slot_map, 0, stack.experts, 4, "slot map")?.cast_const(),
                to_i64(matrix_bytes)?,
                to_i64(scale_bytes)?,
                int(stack.experts, "experts")?,
                span(ids, 0, slots.count, 4, "expert ids")?.cast_const(),
                act.cast_const(),
                act_scales.cast_const(),
                i32::from(slots.shared_input),
                span(out, 0, slots.count * rows, 4, "output")?,
                int(rows, "rows")?,
                int(cols, "cols")?,
                int(slots.count, "slots")?,
                i32::from(output == V41Output::Bf16),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 cached expert matvec")
    }

    /// Posts the host lane's work for one layer: see [`V41LaneDevice`].
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_lane_post(
        &mut self,
        lane: &V41LaneDevice,
        ids: &CudaBuffer,
        slot_map: &CudaBuffer,
        experts: usize,
        route_weights: &CudaBuffer,
        act_codes: &CudaBuffer,
        act_scales: &CudaBuffer,
        layer: usize,
        log_slot: usize,
        index: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let (hidden, top_k) = (lane.hidden, lane.top_k);
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_lane_post(
                span(ids, 0, top_k, 4, "expert ids")?.cast_const(),
                span(slot_map, 0, experts, 4, "slot map")?.cast_const(),
                int(experts, "experts")?,
                span(route_weights, 0, top_k, 4, "route weights")?.cast_const(),
                span(act_codes, 0, hidden, 1, "activation codes")?.cast_const(),
                span(act_scales, 0, hidden / V41_BLOCK, 1, "activation scales")?.cast_const(),
                int(hidden, "hidden")?,
                int(top_k, "top-k")?,
                span(&lane.mailbox, 0, lane.mailbox.byte_len(), 1, "mailbox")?,
                span(&lane.state, 0, 2, 4, "lane state")?,
                int(layer, "layer")?,
                span(index, 0, 1, 4, "token index")?.cast_const(),
                span(
                    &lane.ids_log,
                    0,
                    lane.log_rows * lane.log_layers * top_k,
                    4,
                    "ids log",
                )?,
                int(lane.log_layers, "log layers")?,
                int(log_slot, "log slot")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 host lane post")
    }

    /// Waits for the host lane and copies its rows into `out`.
    ///
    /// # Errors
    ///
    /// Refuses buffers too small for the shapes.
    pub fn v41_lane_wait(
        &mut self,
        lane: &V41LaneDevice,
        out: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let (hidden, top_k) = (lane.hidden, lane.top_k);
        // SAFETY: every extent was checked against its buffer.
        let code = unsafe {
            psionic_v41_lane_wait(
                span(&lane.mailbox, 0, lane.mailbox.byte_len(), 1, "mailbox")?,
                span(&lane.state, 0, 2, 4, "lane state")?,
                span(&lane.rows, 0, top_k * hidden, 4, "host rows")?.cast_const(),
                span(out, 0, top_k * hidden, 4, "routed output")?,
                int(hidden, "hidden")?,
                int(top_k, "top-k")?,
                u64::try_from(lane.timeout.as_nanos()).unwrap_or(u64::MAX),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 host lane wait")
    }
}

// ---------------------------------------------------------------------------
// The mailbox
// ---------------------------------------------------------------------------

/// A raw pointer a worker thread may hold. The owner keeps the allocation
/// alive until the thread has stopped.
#[derive(Clone, Copy, Debug)]
struct SendPtr(*mut u8);
// SAFETY: the pointee is page-locked host memory owned by the cache, which
// joins every thread that holds one of these before freeing it.
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// The device side of a host lane: the mailbox, the output rows, the
/// sequence state, and the id log, which the post and wait kernels use.
#[derive(Debug)]
pub struct V41LaneDevice {
    mailbox: CudaBuffer,
    rows: CudaBuffer,
    state: CudaBuffer,
    ids_log: CudaBuffer,
    mailbox_host: SendPtr,
    rows_host: SendPtr,
    ids_log_host: SendPtr,
    hidden: usize,
    top_k: usize,
    log_rows: usize,
    log_layers: usize,
    timeout: Duration,
}

fn mapped(backend: &mut CudaBackend, bytes: usize) -> Result<(CudaBuffer, SendPtr), RuntimeError> {
    let mut host: CudaHostBuffer = backend.host_buffer(bytes)?;
    host.write_bytes(&vec![0; bytes])?;
    let ptr = host.raw_host_ptr().cast::<u8>();
    if ptr.is_null() {
        return Err(invalid("page-locked memory needs the Linux CUDA runtime"));
    }
    Ok((backend.mapped_buffer(host)?, SendPtr(ptr)))
}

impl V41LaneDevice {
    fn new(
        backend: &mut CudaBackend,
        hidden: usize,
        top_k: usize,
        log_rows: usize,
        log_layers: usize,
        timeout: Duration,
    ) -> Result<Self, RuntimeError> {
        if top_k > V41_LANE_MAX_SLOTS || !hidden.is_multiple_of(128) {
            return Err(invalid(format!("top-k {top_k} or width {hidden}")));
        }
        let (mailbox, mailbox_host) = mapped(backend, PAYLOAD + hidden + hidden / V41_BLOCK)?;
        let (rows, rows_host) = mapped(backend, 4 * top_k * hidden)?;
        let (ids_log, ids_log_host) = mapped(backend, 4 * log_rows * log_layers * top_k)?;
        let state = backend.byte_buffer(&[0; 8])?;
        Ok(Self {
            mailbox,
            rows,
            state,
            ids_log,
            mailbox_host,
            rows_host,
            ids_log_host,
            hidden,
            top_k,
            log_rows,
            log_layers,
            timeout,
        })
    }

    fn read_u32(&self, offset: usize) -> u32 {
        // SAFETY: the mailbox is at least PAYLOAD bytes and lives as long as
        // `self`; the device writes it, so the read is volatile.
        unsafe { std::ptr::read_volatile(self.mailbox_host.0.add(offset).cast::<u32>()) }
    }

    /// The step's logged expert ids, `[tokens, log layers, top_k]`.
    fn logged(&self, tokens: usize) -> Vec<i32> {
        let n = tokens.min(self.log_rows) * self.log_layers * self.top_k;
        // SAFETY: the log holds `log_rows × log_layers × top_k` words; the
        // device finished writing them before the step's wait returned.
        unsafe { std::slice::from_raw_parts(self.ids_log_host.0.cast::<i32>(), n).to_vec() }
    }
}

// ---------------------------------------------------------------------------
// The host lane's poller
// ---------------------------------------------------------------------------

/// What the host lane did.
#[derive(Debug, Default)]
struct LaneCounters {
    requests: AtomicU64,
    experts: AtomicU64,
    nanos: AtomicU64,
    failures: AtomicU64,
}

struct Poller {
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Poller {
    #[allow(clippy::too_many_arguments)]
    fn start(
        lane: &V41LaneDevice,
        host: Arc<dyn V41ExpertBytes>,
        dims: V41ExpertDims,
        isa: V41LaneIsa,
        pool: Arc<rayon::ThreadPool>,
        counters: Arc<LaneCounters>,
    ) -> Result<Self, RuntimeError> {
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(false));
        let (mailbox, rows) = (lane.mailbox_host, lane.rows_host);
        let top_k = lane.top_k;
        let thread = {
            let (stop, active) = (Arc::clone(&stop), Arc::clone(&active));
            std::thread::Builder::new()
                .name("v41-host-lane".into())
                .spawn(move || {
                    poll(
                        &stop, &active, mailbox, rows, top_k, &*host, dims, isa, &pool, &counters,
                    );
                })
                .map_err(|e| invalid(format!("host lane thread: {e}")))?
        };
        Ok(Self {
            stop,
            active,
            thread: Some(thread),
        })
    }

    fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Release);
        if on {
            if let Some(t) = &self.thread {
                t.thread().unpark();
            }
        }
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn poll(
    stop: &AtomicBool,
    active: &AtomicBool,
    mailbox: SendPtr,
    rows: SendPtr,
    top_k: usize,
    host: &dyn V41ExpertBytes,
    dims: V41ExpertDims,
    isa: V41LaneIsa,
    pool: &rayon::ThreadPool,
    counters: &LaneCounters,
) {
    let read = |offset: usize| -> u32 {
        // SAFETY: the mailbox outlives this thread (see `SendPtr`).
        unsafe { std::ptr::read_volatile(mailbox.0.add(offset).cast::<u32>()) }
    };
    let mut last = read(POST);
    let mut work = LaneWork::default();
    while !stop.load(Ordering::Acquire) {
        if !active.load(Ordering::Acquire) {
            std::thread::park_timeout(Duration::from_millis(5));
            continue;
        }
        let post = read(POST);
        if post == last {
            std::hint::spin_loop();
            continue;
        }
        fence(Ordering::Acquire);
        last = post;
        let started = Instant::now();
        let layer = read(LAYER) as usize;
        let mask = read(MASK);
        // SAFETY: the device wrote the request before publishing `post`.
        let request = unsafe {
            let ids = std::slice::from_raw_parts(mailbox.0.add(IDS).cast::<i32>(), top_k).to_vec();
            let weights =
                std::slice::from_raw_parts(mailbox.0.add(WEIGHTS).cast::<f32>(), top_k).to_vec();
            let codes = std::slice::from_raw_parts(mailbox.0.add(PAYLOAD), dims.hidden);
            let scales = std::slice::from_raw_parts(
                mailbox.0.add(PAYLOAD + dims.hidden),
                dims.hidden / V41_BLOCK,
            );
            work.x.fill(codes, scales);
            (ids, weights)
        };
        let slots: Vec<usize> = (0..top_k).filter(|s| mask & (1 << s) != 0).collect();
        let outcome =
            pool.install(|| work.run(host, dims, isa, layer, &slots, &request.0, &request.1));
        match outcome {
            Ok(()) => {
                for (i, s) in slots.iter().enumerate() {
                    // SAFETY: the rows region holds `top_k × hidden` floats.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            work.out[i * dims.hidden..(i + 1) * dims.hidden].as_ptr(),
                            rows.0.cast::<f32>().add(s * dims.hidden),
                            dims.hidden,
                        );
                    }
                }
            }
            Err(_) => {
                counters.failures.fetch_add(1, Ordering::Relaxed);
                for s in &slots {
                    // SAFETY: as above.
                    unsafe {
                        std::slice::from_raw_parts_mut(
                            rows.0.cast::<f32>().add(s * dims.hidden),
                            dims.hidden,
                        )
                        .fill(f32::NAN);
                    }
                }
            }
        }
        fence(Ordering::Release);
        // SAFETY: as above; the device reads `done` volatile.
        unsafe { std::ptr::write_volatile(mailbox.0.add(DONE).cast::<u32>(), post) };
        counters.requests.fetch_add(1, Ordering::Relaxed);
        counters
            .experts
            .fetch_add(slots.len() as u64, Ordering::Relaxed);
        counters
            .nanos
            .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
}

/// The host lane's scratch for one request.
#[derive(Default)]
struct LaneWork {
    x: V41DecodedActivation,
    gate: Vec<f32>,
    up: Vec<f32>,
    hidden: Vec<V41DecodedActivation>,
    out: Vec<f32>,
}

/// Rows per task in the host lane's parallel loops.
const LANE_ROWS: usize = 32;

impl LaneWork {
    /// Computes the experts in `slots` (their indices into `ids`) in
    /// parallel: gate and up rows for all of them, then each SwiGLU, then
    /// the down rows.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        host: &dyn V41ExpertBytes,
        dims: V41ExpertDims,
        isa: V41LaneIsa,
        layer: usize,
        slots: &[usize],
        ids: &[i32],
        weights: &[f32],
    ) -> Result<(), RuntimeError> {
        let (h, w, n) = (dims.hidden, dims.width, slots.len());
        let mut experts = Vec::with_capacity(n);
        for s in slots {
            let id = usize::try_from(ids[*s]).map_err(|_| invalid("expert id"))?;
            let w1 = host.projection(layer, id, 0)?;
            let w2 = host.projection(layer, id, 1)?;
            let w3 = host.projection(layer, id, 2)?;
            experts.push(V41ExpertWeights { w1, w2, w3 });
        }
        self.gate.resize(n * w, 0.0);
        self.up.resize(n * w, 0.0);
        let x = &self.x;
        self.gate
            .par_chunks_mut(LANE_ROWS)
            .zip(self.up.par_chunks_mut(LANE_ROWS))
            .enumerate()
            .for_each(|(c, (g, u))| {
                let row = c * LANE_ROWS;
                let (e, r) = (row / w, row % w);
                v41_expert_gate_up(isa, &experts[e], x, r..r + g.len(), g, u);
            });
        self.hidden.resize_with(n, V41DecodedActivation::default);
        let (gate, up) = (&self.gate, &self.up);
        self.hidden
            .par_iter_mut()
            .enumerate()
            .for_each(|(e, decoded)| {
                let mut codes = vec![0; w];
                let mut scales = vec![0; w / V41_BLOCK];
                v41_expert_hidden(
                    dims,
                    &gate[e * w..(e + 1) * w],
                    &up[e * w..(e + 1) * w],
                    weights[slots[e]],
                    &mut codes,
                    &mut scales,
                );
                decoded.fill(&codes, &scales);
            });
        self.out.resize(n * h, 0.0);
        let hidden = &self.hidden;
        self.out
            .par_chunks_mut(LANE_ROWS)
            .enumerate()
            .for_each(|(c, o)| {
                let row = c * LANE_ROWS;
                let (e, r) = (row / h, row % h);
                v41_expert_down(isa, &experts[e], &hidden[e], r..r + o.len(), o);
            });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Staging and swaps
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Staged {
    Free,
    /// Being copied: (stage layer index, expert).
    Filling(usize, usize),
    Ready(usize, usize),
    Failed,
}

struct Stager {
    shared: Arc<(Mutex<StagerState>, Condvar)>,
    buffers: Vec<CudaHostBuffer>,
    thread: Option<JoinHandle<()>>,
}

struct StagerState {
    slots: Vec<Staged>,
    queue: VecDeque<usize>,
    stop: bool,
}

/// Byte offsets of an expert's six parts in a staging buffer:
/// `[w1 w, w1 s, w2 w, w2 s, w3 w, w3 s]`.
fn part_offsets(dims: V41ExpertDims) -> [(usize, usize); 6] {
    let (w13, s13) = V41ExpertDims::fp4_bytes(dims.width, dims.hidden);
    let (w2, s2) = V41ExpertDims::fp4_bytes(dims.hidden, dims.width);
    let sizes = [w13, s13, w2, s2, w13, s13];
    let mut out = [(0, 0); 6];
    let mut at = 0;
    for (i, n) in sizes.into_iter().enumerate() {
        out[i] = (at, n);
        at += n;
    }
    out
}

impl Stager {
    fn start(
        backend: &mut CudaBackend,
        count: usize,
        dims: V41ExpertDims,
        host: Arc<dyn V41ExpertBytes>,
        layers: Vec<usize>,
    ) -> Result<Self, RuntimeError> {
        let bytes = dims.expert_bytes();
        let mut buffers = Vec::with_capacity(count);
        let mut ptrs = Vec::with_capacity(count);
        for _ in 0..count {
            let buffer = backend.host_buffer(bytes)?;
            let ptr = buffer.raw_host_ptr().cast::<u8>();
            if ptr.is_null() {
                return Err(invalid("page-locked memory needs the Linux CUDA runtime"));
            }
            ptrs.push(SendPtr(ptr));
            buffers.push(buffer);
        }
        let shared = Arc::new((
            Mutex::new(StagerState {
                slots: vec![Staged::Free; count],
                queue: VecDeque::new(),
                stop: false,
            }),
            Condvar::new(),
        ));
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("v41-expert-stager".into())
                .spawn(move || stage_loop(&shared, &ptrs, dims, &*host, &layers))
                .map_err(|e| invalid(format!("stager thread: {e}")))?
        };
        Ok(Self {
            shared,
            buffers,
            thread: Some(thread),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StagerState> {
        self.shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Drop for Stager {
    fn drop(&mut self) {
        self.lock().stop = true;
        self.shared.1.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn stage_loop(
    shared: &(Mutex<StagerState>, Condvar),
    ptrs: &[SendPtr],
    dims: V41ExpertDims,
    host: &dyn V41ExpertBytes,
    layers: &[usize],
) {
    let offsets = part_offsets(dims);
    loop {
        let (index, layer, expert) = {
            let mut state = shared
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            loop {
                if state.stop {
                    return;
                }
                if let Some(i) = state.queue.pop_front() {
                    if let Staged::Filling(l, e) = state.slots[i] {
                        break (i, l, e);
                    }
                    continue;
                }
                state = shared
                    .1
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        let mut ok = true;
        for p in 0..3 {
            match host.projection(layers[layer], expert, p) {
                Ok((w, s)) if w.len() == offsets[2 * p].1 && s.len() == offsets[2 * p + 1].1 => {
                    // SAFETY: each staging buffer holds `expert_bytes` and
                    // this thread alone writes a buffer while it's Filling.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            w.as_ptr(),
                            ptrs[index].0.add(offsets[2 * p].0),
                            w.len(),
                        );
                        std::ptr::copy_nonoverlapping(
                            s.as_ptr(),
                            ptrs[index].0.add(offsets[2 * p + 1].0),
                            s.len(),
                        );
                    }
                }
                _ => ok = false,
            }
        }
        let mut state = shared
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.slots[index] = if ok {
            Staged::Ready(layer, expert)
        } else {
            Staged::Failed
        };
    }
}

// ---------------------------------------------------------------------------
// The cache
// ---------------------------------------------------------------------------

/// One stage layer's cache: the device slots of each routed projection and
/// the slot map, with the host's copy of the placement.
#[derive(Debug)]
pub(crate) struct LayerCache {
    /// `[w1, w2, w3]`: slot weights and scales.
    pub(crate) slots: [(CudaBuffer, CudaBuffer); 3],
    pub(crate) slot_map: CudaBuffer,
    /// Which expert each slot holds.
    resident: Vec<Option<usize>>,
    /// The device map's host copy: a slot or a lane per expert.
    map: Vec<i32>,
    heat: Vec<f32>,
}

/// What the cache did, since load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct V41ExpertCacheStats {
    pub slots_per_layer: usize,
    pub layers: usize,
    /// The lane misses use.
    pub miss_lane: Option<V41MissLane>,
    /// Measured at load, microseconds per expert (all three projections, one
    /// token): device slot, bus, and host CPU.
    pub device_expert_us: Option<f64>,
    pub bus_expert_us: Option<f64>,
    pub host_expert_us: Option<f64>,
    /// Selected experts by where they ran.
    pub hits: u64,
    pub bus: u64,
    pub host: u64,
    /// Host-lane requests (layers with any host-lane expert) and their time.
    pub host_requests: u64,
    pub host_seconds: f64,
    pub swaps: u64,
    pub staging_failures: u64,
    pub host_failures: u64,
    /// Device bytes of the slots and maps, and page-locked staging bytes.
    pub device_bytes: usize,
    pub staging_bytes: usize,
}

impl V41ExpertCacheStats {
    /// Hits over all selected experts.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.bus + self.host;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// A stage's expert cache. The stage owns it and calls [`Self::before_step`]
/// and [`Self::after_step`] around each step.
pub(crate) struct ExpertCache {
    pub(crate) config: V41ExpertCacheConfig,
    pub(crate) dims: V41ExpertDims,
    pub(crate) experts: usize,
    /// Model layer of each stage layer.
    layers: Vec<usize>,
    pub(crate) caches: Vec<LayerCache>,
    pub(crate) lane: V41LaneDevice,
    miss_code: i32,
    host: Arc<dyn V41ExpertBytes>,
    counters: Arc<LaneCounters>,
    pool: Arc<rayon::ThreadPool>,
    poller: Option<Poller>,
    stager: Option<Stager>,
    tokens_since_decay: usize,
    stats: V41ExpertCacheStats,
}

impl std::fmt::Debug for ExpertCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpertCache")
            .field("layers", &self.layers)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl ExpertCache {
    /// Allocates the slots and the lane and fills each layer's slots with
    /// its hottest experts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn load(
        backend: &mut CudaBackend,
        config: V41ExpertCacheConfig,
        dims: V41ExpertDims,
        experts: usize,
        top_k: usize,
        layers: Vec<usize>,
        log_rows: usize,
        host: Arc<dyn V41ExpertBytes>,
        bus_available: bool,
    ) -> Result<Self, RuntimeError> {
        let k = if config.slots_per_layer == 0 {
            let free = backend
                .device_memory_info()?
                .map_or(0, |m| usize::try_from(m.free_bytes).unwrap_or(usize::MAX));
            free.saturating_sub(config.reserve_bytes) / (dims.expert_bytes() * layers.len().max(1))
        } else {
            config.slots_per_layer
        };
        let k = k.clamp(1, experts.max(1));
        if config.miss_lane == V41MissLane::Bus && !bus_available {
            return Err(invalid(
                "the bus lane needs the routed experts in page-locked host memory (--experts host)",
            ));
        }
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        let shapes = [
            (dims.width, dims.hidden),
            (dims.hidden, dims.width),
            (dims.width, dims.hidden),
        ];
        let lane = V41LaneDevice::new(
            backend,
            dims.hidden,
            top_k,
            log_rows,
            layers.len(),
            config.wait_timeout,
        )?;
        let mut device_bytes = 0;
        let mut caches = Vec::with_capacity(layers.len());
        for &layer in &layers {
            let mut heat = config
                .profile
                .get(&layer)
                .cloned()
                .unwrap_or_else(|| vec![0.0; experts]);
            heat.resize(experts, 0.0);
            let mut order: Vec<usize> = (0..experts).collect();
            order.sort_by(|a, b| heat[*b].total_cmp(&heat[*a]).then(a.cmp(b)));
            let mut resident = vec![None; k];
            let mut map = vec![V41_LANE_HOST; experts];
            let mut slots = Vec::with_capacity(3);
            for (rows, cols) in shapes {
                let w = backend.byte_buffer(&vec![0; (fp4.weight_bytes(rows, cols) * k).max(4)])?;
                let s = backend.byte_buffer(&vec![0; (fp4.scale_bytes(rows, cols) * k).max(4)])?;
                device_bytes += w.byte_len() + s.byte_len();
                slots.push((w, s));
            }
            let mut slots: [(CudaBuffer, CudaBuffer); 3] =
                slots.try_into().map_err(|_| invalid("three projections"))?;
            for (slot, &expert) in order.iter().take(k).enumerate() {
                for (p, (w, s)) in slots.iter_mut().enumerate() {
                    let (wb, sb) = host.projection(layer, expert, p)?;
                    let (rows, cols) = shapes[p];
                    w.write_bytes_at_offset(slot * fp4.weight_bytes(rows, cols), wb)?;
                    s.write_bytes_at_offset(slot * fp4.scale_bytes(rows, cols), sb)?;
                }
                resident[slot] = Some(expert);
                map[expert] = i32::try_from(slot).map_err(|_| invalid("slot"))?;
            }
            let slot_map = backend.byte_buffer(&map_bytes(&map))?;
            device_bytes += slot_map.byte_len();
            caches.push(LayerCache {
                slots,
                slot_map,
                resident,
                map,
                heat,
            });
        }
        let threads = if config.host_threads == 0 {
            std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
        } else {
            config.host_threads
        };
        let counters = Arc::new(LaneCounters::default());
        let pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("v41-host-lane-{i}"))
                .build()
                .map_err(|e| invalid(format!("host lane threads: {e}")))?,
        );
        let stats = V41ExpertCacheStats {
            slots_per_layer: k,
            layers: layers.len(),
            device_bytes,
            ..V41ExpertCacheStats::default()
        };
        let mut cache = Self {
            config,
            dims,
            experts,
            layers,
            caches,
            lane,
            miss_code: V41_LANE_HOST,
            host,
            counters,
            pool,
            poller: None,
            stager: None,
            tokens_since_decay: 0,
            stats,
        };
        let isa = V41LaneIsa::detect();
        cache.poller = Some(Poller::start(
            &cache.lane,
            Arc::clone(&cache.host),
            dims,
            isa,
            Arc::clone(&cache.pool),
            Arc::clone(&cache.counters),
        )?);
        if !cache.config.frozen && k > 0 && k < experts {
            let staging = cache.config.policy.staging.max(1);
            cache.stats.staging_bytes = staging * dims.expert_bytes();
            cache.stager = Some(Stager::start(
                backend,
                staging,
                dims,
                Arc::clone(&cache.host),
                cache.layers.clone(),
            )?);
        }
        Ok(cache)
    }

    /// Sets the lane cache misses use and writes it into every slot map.
    pub(crate) fn set_miss_lane(&mut self, lane: V41MissLane) -> Result<(), RuntimeError> {
        let code = match lane {
            V41MissLane::Bus => V41_LANE_BUS,
            V41MissLane::Host | V41MissLane::Auto => V41_LANE_HOST,
        };
        self.miss_code = code;
        self.stats.miss_lane = Some(if code == V41_LANE_BUS {
            V41MissLane::Bus
        } else {
            V41MissLane::Host
        });
        for c in &mut self.caches {
            for m in &mut c.map {
                if *m < 0 {
                    *m = code;
                }
            }
            c.slot_map.write_bytes_at_offset(0, &map_bytes(&c.map))?;
        }
        Ok(())
    }

    pub(crate) fn record_lane_times(&mut self, device: f64, bus: Option<f64>, host: f64) {
        self.stats.device_expert_us = Some(device);
        self.stats.bus_expert_us = bus;
        self.stats.host_expert_us = Some(host);
    }

    /// Computes one expert on the host lane's threads, for the load-time
    /// measurement; returns the output.
    pub(crate) fn host_forward(
        &self,
        layer: usize,
        expert: usize,
        codes: &[u8],
        scales: &[u8],
    ) -> Result<Vec<f32>, RuntimeError> {
        let pool = &self.pool;
        let mut work = LaneWork::default();
        work.x.fill(codes, scales);
        let ids = [i32::try_from(expert).map_err(|_| invalid("expert"))?];
        let (host, dims) = (&*self.host, self.dims);
        pool.install(|| work.run(host, dims, V41LaneIsa::detect(), layer, &[0], &ids, &[1.0]))?;
        Ok(work.out)
    }

    /// Between steps: applies staged swaps and wakes the host lane.
    pub(crate) fn before_step(&mut self) -> Result<(), RuntimeError> {
        self.apply_swaps()?;
        if let Some(p) = &self.poller {
            p.set_active(true);
        }
        Ok(())
    }

    /// After a step's wait: counts where the step's experts ran, updates
    /// heat, and stages the next swaps.
    pub(crate) fn after_step(&mut self, tokens: usize) -> Result<(), RuntimeError> {
        if let Some(p) = &self.poller {
            p.set_active(false);
        }
        if self.lane.read_u32(ERROR) != 0 {
            return Err(invalid(format!(
                "the host lane didn't answer within {:?}",
                self.config.wait_timeout
            )));
        }
        let logged = self.lane.logged(tokens);
        let top_k = self.lane.top_k;
        let per_token = self.layers.len() * top_k;
        for row in logged.chunks_exact(per_token) {
            for (l, ids) in row.chunks_exact(top_k).enumerate() {
                let cache = &mut self.caches[l];
                for id in ids {
                    let Ok(id) = usize::try_from(*id) else {
                        continue;
                    };
                    if id >= self.experts {
                        continue;
                    }
                    cache.heat[id] += 1.0;
                    match cache.map[id] {
                        m if m >= 0 => self.stats.hits += 1,
                        V41_LANE_BUS => self.stats.bus += 1,
                        _ => self.stats.host += 1,
                    }
                }
            }
        }
        self.tokens_since_decay += tokens;
        let policy = self.config.policy;
        while policy.window > 0 && self.tokens_since_decay >= policy.window {
            self.tokens_since_decay -= policy.window;
            for c in &mut self.caches {
                for h in &mut c.heat {
                    *h *= policy.decay;
                }
            }
        }
        self.stats.host_requests = self.counters.requests.load(Ordering::Relaxed);
        self.stats.host_seconds = self.counters.nanos.load(Ordering::Relaxed) as f64 * 1e-9;
        self.stats.host_failures = self.counters.failures.load(Ordering::Relaxed);
        if self.stats.host_failures > 0 {
            return Err(invalid("the host lane couldn't compute an expert"));
        }
        self.plan_swaps();
        Ok(())
    }

    /// The coldest resident slot of a layer and its heat.
    fn coldest(cache: &LayerCache) -> Option<(usize, f32)> {
        cache
            .resident
            .iter()
            .enumerate()
            .filter_map(|(slot, e)| e.map(|e| (slot, cache.heat[e])))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    fn wants(policy: V41SwapPolicy, cache: &LayerCache, expert: usize) -> Option<usize> {
        let (slot, cold) = Self::coldest(cache)?;
        (cache.map[expert] < 0 && cache.heat[expert] > cold * policy.ratio + policy.margin)
            .then_some(slot)
    }

    /// Queues the hottest host expert of each layer that beats its layer's
    /// coldest cached expert, while staging buffers are free.
    fn plan_swaps(&mut self) {
        let Some(stager) = &self.stager else {
            return;
        };
        let policy = self.config.policy;
        let mut state = stager.lock();
        let busy: Vec<(usize, usize)> = state
            .slots
            .iter()
            .filter_map(|s| match *s {
                Staged::Filling(l, e) | Staged::Ready(l, e) => Some((l, e)),
                _ => None,
            })
            .collect();
        let mut free: Vec<usize> = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, Staged::Free | Staged::Failed))
            .map(|(i, _)| i)
            .collect();
        if free.is_empty() {
            return;
        }
        // The layers' best candidates, hottest margin first.
        let mut candidates = Vec::new();
        for (l, cache) in self.caches.iter().enumerate() {
            let Some((_, cold)) = Self::coldest(cache) else {
                continue;
            };
            let best = (0..self.experts)
                .filter(|e| cache.map[*e] < 0 && !busy.contains(&(l, *e)))
                .max_by(|a, b| cache.heat[*a].total_cmp(&cache.heat[*b]).then(b.cmp(a)));
            if let Some(e) = best {
                if cache.heat[e] > cold * policy.ratio + policy.margin {
                    candidates.push((cache.heat[e] - cold, l, e));
                }
            }
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, l, e) in candidates {
            let Some(i) = free.pop() else {
                break;
            };
            if state.slots[i] == Staged::Failed {
                self.stats.staging_failures += 1;
            }
            state.slots[i] = Staged::Filling(l, e);
            state.queue.push_back(i);
        }
        drop(state);
        stager.shared.1.notify_all();
    }

    /// Copies staged experts into their slots, while no graph runs.
    fn apply_swaps(&mut self) -> Result<(), RuntimeError> {
        let Some(stager) = &self.stager else {
            return Ok(());
        };
        let ready: Vec<(usize, usize, usize)> = {
            let state = stager.lock();
            state
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| match *s {
                    Staged::Ready(l, e) => Some((i, l, e)),
                    _ => None,
                })
                .take(self.config.policy.max_swaps_per_step)
                .collect()
        };
        let offsets = part_offsets(self.dims);
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        let shapes = [
            (self.dims.width, self.dims.hidden),
            (self.dims.hidden, self.dims.width),
            (self.dims.width, self.dims.hidden),
        ];
        let policy = self.config.policy;
        for (i, l, e) in ready {
            let cache = &mut self.caches[l];
            if let Some(slot) = Self::wants(policy, cache, e) {
                let old = cache.resident[slot];
                // The slot leaves the map before its bytes change.
                if let Some(old) = old {
                    cache.map[old] = self.miss_code;
                    cache
                        .slot_map
                        .write_bytes_at_offset(4 * old, &self.miss_code.to_le_bytes())?;
                }
                let staged = stager.buffers[i].read_bytes_prefix_view();
                for p in 0..3 {
                    let (rows, cols) = shapes[p];
                    let (w_at, w_len) = offsets[2 * p];
                    let (s_at, s_len) = offsets[2 * p + 1];
                    let (w, s) = &mut cache.slots[p];
                    w.write_bytes_at_offset(
                        slot * fp4.weight_bytes(rows, cols),
                        &staged[w_at..w_at + w_len],
                    )?;
                    s.write_bytes_at_offset(
                        slot * fp4.scale_bytes(rows, cols),
                        &staged[s_at..s_at + s_len],
                    )?;
                }
                let code = i32::try_from(slot).map_err(|_| invalid("slot"))?;
                cache.map[e] = code;
                cache
                    .slot_map
                    .write_bytes_at_offset(4 * e, &code.to_le_bytes())?;
                cache.resident[slot] = Some(e);
                self.stats.swaps += 1;
            }
            stager.lock().slots[i] = Staged::Free;
        }
        Ok(())
    }

    /// Device slots per layer.
    pub(crate) fn slots(&self) -> usize {
        self.stats.slots_per_layer
    }

    pub(crate) fn stats(&self) -> V41ExpertCacheStats {
        self.stats.clone()
    }

    /// Each stage layer's current heat, by model layer: a routing profile
    /// for the next load.
    pub(crate) fn profile(&self) -> BTreeMap<usize, Vec<f32>> {
        self.layers
            .iter()
            .zip(&self.caches)
            .map(|(l, c)| (*l, c.heat.clone()))
            .collect()
    }

    /// Which experts each stage layer holds in its slots.
    pub(crate) fn resident(&self) -> BTreeMap<usize, Vec<usize>> {
        self.layers
            .iter()
            .zip(&self.caches)
            .map(|(l, c)| (*l, c.resident.iter().flatten().copied().collect()))
            .collect()
    }

    /// Model layer of stage layer `slot`.
    pub(crate) fn layer_slot(&self, layer: usize) -> Option<usize> {
        self.layers.iter().position(|l| *l == layer)
    }
}

impl Drop for ExpertCache {
    fn drop(&mut self) {
        // Stop the threads before the memory they read goes away.
        if let Some(p) = &self.poller {
            p.set_active(false);
        }
        self.poller.take();
        self.stager.take();
    }
}

fn map_bytes(map: &[i32]) -> Vec<u8> {
    map.iter().flat_map(|m| m.to_le_bytes()).collect()
}

impl CudaHostBuffer {
    /// The whole page-locked buffer as a byte slice, without copying.
    pub(crate) fn read_bytes_prefix_view(&self) -> &[u8] {
        let ptr = self.raw_host_ptr().cast::<u8>();
        if ptr.is_null() {
            return &[];
        }
        // SAFETY: the allocation holds `byte_len` bytes and outlives `self`'s
        // borrow; the stager no longer writes a Ready buffer.
        unsafe { std::slice::from_raw_parts(ptr, self.byte_len()) }
    }
}

/// Page-locked routed experts the stage loaded with `--experts host`, as a
/// host-lane source. The stage keeps the buffers alive longer than the
/// cache that reads them.
pub(crate) struct PinnedExperts {
    /// Per model layer, per projection: weights, bytes per matrix, scales,
    /// bytes per matrix.
    layers: BTreeMap<usize, [(SendPtr, usize, SendPtr, usize); 3]>,
    experts: usize,
}

impl PinnedExperts {
    /// Reads the addresses of each layer's page-locked stacks
    /// (`[w1, w2, w3]` as (weights, scales, rows, cols)).
    pub(crate) fn new(
        layers: &[(usize, [(&CudaBuffer, &CudaBuffer, usize, usize); 3])],
        experts: usize,
    ) -> Result<Self, RuntimeError> {
        let fp4 = V41MatvecFormat::Fp4E2M1Row32;
        let mut out = BTreeMap::new();
        for (layer, parts) in layers {
            let mut entry = Vec::with_capacity(3);
            for (w, s, rows, cols) in parts {
                let wb = fp4.weight_bytes(*rows, *cols);
                let sb = fp4.scale_bytes(*rows, *cols);
                let wp = span(w, 0, wb * experts, 1, "host weights")?;
                let sp = span(s, 0, sb * experts, 1, "host scales")?;
                entry.push((SendPtr(wp.cast()), wb, SendPtr(sp.cast()), sb));
            }
            let entry: [(SendPtr, usize, SendPtr, usize); 3] =
                entry.try_into().map_err(|_| invalid("three projections"))?;
            out.insert(*layer, entry);
        }
        Ok(Self {
            layers: out,
            experts,
        })
    }
}

impl V41ExpertBytes for PinnedExperts {
    fn projection(
        &self,
        layer: usize,
        expert: usize,
        projection: usize,
    ) -> Result<(&[u8], &[u8]), RuntimeError> {
        let parts = self
            .layers
            .get(&layer)
            .ok_or_else(|| invalid(format!("layer {layer} has no host experts")))?;
        if expert >= self.experts || projection > 2 {
            return Err(invalid(format!("expert {expert} w{}", projection + 1)));
        }
        let (w, wb, s, sb) = parts[projection];
        // SAFETY: the stacks hold `experts` matrices and outlive the cache.
        Ok(unsafe {
            (
                std::slice::from_raw_parts(w.0.add(expert * wb), wb),
                std::slice::from_raw_parts(s.0.add(expert * sb), sb),
            )
        })
    }
}

/// What [`crate::v41_stage::V41Stage::load_cached`] needs for a cache.
#[derive(Clone)]
pub struct V41ExpertCacheSetup {
    pub config: V41ExpertCacheConfig,
    /// Where the host lane and swaps read experts. `None` reads the stage's
    /// own page-locked stacks, which needs `experts_on_host`. With a
    /// source and without `experts_on_host`, the stage loads no routed
    /// stacks at all: misses run on the host lane.
    pub host: Option<Arc<dyn V41ExpertBytes>>,
}

impl std::fmt::Debug for V41ExpertCacheSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V41ExpertCacheSetup")
            .field("config", &self.config)
            .field("host", &self.host.is_some())
            .finish()
    }
}
