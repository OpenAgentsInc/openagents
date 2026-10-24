//! Browser backend: calls cross into the JavaScript import table in
//! `apps/render-lab/gpu-imports.ts`. Handles are indexes into that table.
//! Descriptors cross once, at creation, as JSON; per-frame calls pass integers.
//!
//! Pass commands are batched by default (ADR 0004): they are encoded into one
//! word buffer ([`crate::batch`]) and replayed by a single `replay` call when
//! the pass ends, when another pass's command arrives, or before anything is
//! released or submitted (so a freed table slot is never reused while queued
//! words still name it). A bind that repeats the one already set in the pass
//! is not recorded. `set_batched(false)` restores one import call per command,
//! with every bind made.

use crate::batch::{self, Command};
use crate::*;
use serde_json::{Value, json};
use std::collections::VecDeque;

pub type Handle = u32;

pub struct Device {
    #[allow(dead_code)]
    pub(crate) handle: Handle,
}

mod ffi {
#[link(wasm_import_module = "af_gpu")]
unsafe extern "C" {

    pub fn host_device() -> u32;
    pub fn host_info(out: *mut u8, cap: u32) -> u32;
    pub fn release(kind: u32, handle: u32);
    pub fn create(kind: u32, json: *const u8, len: u32) -> u32;
    pub fn create_shader_module(label: *const u8, label_len: u32, code: *const u8, code_len: u32) -> u32;
    pub fn get_bind_group_layout(pipeline: u32, is_render: u32, index: u32) -> u32;
    pub fn create_view(texture: u32, json: *const u8, len: u32) -> u32;
    pub fn write_buffer(buffer: u32, offset: f64, data: *const u8, len: u32);
    pub fn write_texture(texture: u32, mip: u32, layer: u32, data: *const u8, len: u32, bytes_per_row: u32, w: u32, h: u32, d: u32);
    pub fn create_command_encoder() -> u32;
    pub fn begin_compute_pass(encoder: u32, json: *const u8, len: u32) -> u32;
    pub fn begin_render_pass(encoder: u32, json: *const u8, len: u32) -> u32;
    pub fn copy_buffer_to_buffer(encoder: u32, src: u32, src_offset: f64, dst: u32, dst_offset: f64, size: f64);
    pub fn clear_buffer(encoder: u32, buffer: u32, offset: f64, size: f64);
    pub fn resolve_queries(encoder: u32, set: u32, first: u32, count: u32, dst: u32, offset: f64);
    pub fn finish(encoder: u32) -> u32;
    pub fn submit(buffers: *const u32, count: u32);
    pub fn set_pipeline(pass: u32, pipeline: u32);
    pub fn set_bind_group(pass: u32, index: u32, group: u32);
    pub fn dispatch(pass: u32, x: u32, y: u32, z: u32);
    pub fn dispatch_indirect(pass: u32, buffer: u32, offset: f64);
    pub fn end_pass(pass: u32);
    pub fn set_vertex_buffer(pass: u32, slot: u32, buffer: u32, offset: f64, size: f64);
    pub fn set_index_buffer(pass: u32, buffer: u32, uint32: u32, offset: f64, size: f64);
    pub fn draw(pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32);
    pub fn draw_indexed(pass: u32, indices: u32, instances: u32, first_index: u32, base_vertex: i32, first_instance: u32);
    pub fn draw_indirect(pass: u32, buffer: u32, offset: f64);
    pub fn draw_indexed_indirect(pass: u32, buffer: u32, offset: f64);
    pub fn read_buffer(buffer: u32, offset: f64, size: f64, ticket: u32);
    pub fn surface_view() -> u32;
    pub fn surface_format() -> u32;
    pub fn take_error(out: *mut u8, cap: u32) -> u32;
    pub fn now() -> f64;
    pub fn log(message: *const u8, len: u32);
    pub fn replay(pass: u32, words: *const u32, len: u32);
}
}

// Kind codes shared with the import table.
const BUFFER: u32 = 1;
const TEXTURE: u32 = 2;
const SAMPLER: u32 = 3;
const COMPUTE_PIPELINE: u32 = 4;
const RENDER_PIPELINE: u32 = 5;
const BIND_GROUP: u32 = 6;
const QUERY_SET: u32 = 7;

/// Whether pass commands are batched, and the commands not yet replayed.
struct Batch {
    on: bool,
    recorder: batch::Recorder,
}

thread_local! {
    static BATCH: std::cell::RefCell<Batch> =
        const { std::cell::RefCell::new(Batch { on: true, recorder: batch::Recorder::new() }) };
    static NEXT_TICKET: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
    static READS: std::cell::RefCell<VecDeque<(ReadTicket, Vec<u8>)>> = Default::default();
}

pub fn set_batched(on: bool) {
    BATCH.with(|b| {
        let mut b = b.borrow_mut();
        replay(&mut b.recorder);
        b.recorder.forget();
        b.on = on;
    });
}

pub fn batched() -> bool {
    BATCH.with(|b| b.borrow().on)
}

/// Replays the recorded words, if any, with one call.
fn replay(recorder: &mut batch::Recorder) {
    if !recorder.words.is_empty() {
        unsafe { ffi::replay(recorder.pass, recorder.words.as_ptr(), recorder.words.len() as u32) }
        recorder.words.clear();
    }
}

/// Replays the recorded words, if any, with one call.
pub fn flush() {
    BATCH.with(|b| replay(&mut b.borrow_mut().recorder));
}

/// Records a pass command when batching; `false` means "make the call".
/// One borrow per command: this runs for every draw. Inlined, so each call
/// site records its own command kind without matching on it at run time.
#[inline(always)]
fn queue(pass: Handle, command: Command) -> bool {
    BATCH.with(|b| {
        let mut b = b.borrow_mut();
        if !b.on {
            return false;
        }
        let recorder = &mut b.recorder;
        if recorder.pass != pass {
            replay(recorder);
            recorder.forget();
            recorder.pass = pass;
        }
        recorder.record(&command);
        true
    })
}

/// Ends a pass: replays its words, and the next pass starts with nothing bound.
fn end_pass(pass: Handle) {
    if queue(pass, Command::EndPass) {
        BATCH.with(|b| {
            let recorder = &mut b.borrow_mut().recorder;
            replay(recorder);
            recorder.pass = 0;
        });
    } else {
        unsafe { ffi::end_pass(pass) }
    }
}

/// Called by JavaScript to reserve memory for a finished read.
#[unsafe(no_mangle)]
pub extern "C" fn af_gpu_alloc(len: u32) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len as usize);
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    ptr
}

/// Called by JavaScript when a read finishes; takes ownership of the bytes.
#[unsafe(no_mangle)]
pub extern "C" fn af_gpu_read_done(ticket: u32, ptr: *mut u8, len: u32) {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    READS.with(|r| r.borrow_mut().push_back((ReadTicket(ticket), bytes)));
}

fn create_json(kind: u32, value: &Value) -> Handle {
    let text = value.to_string();
    unsafe { ffi::create(kind, text.as_ptr(), text.len() as u32) }
}

fn kind_code(kind: Kind) -> u32 {
    kind as u32 + 1
}

fn info_text() -> String {
    let mut buf = vec![0u8; 4096];
    let len = unsafe { ffi::host_info(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    String::from_utf8_lossy(&buf[..len.min(buf.len())]).into_owned()
}

pub fn from_host() -> (Device, Vec<Feature>, AdapterInfo) {
    let handle = unsafe { ffi::host_device() };
    let info: Value = serde_json::from_str(&info_text()).unwrap_or(Value::Null);
    let names: Vec<String> = info["features"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let mut features = Vec::new();
    for (name, feature) in [
        ("timestamp-query", Feature::TimestampQuery),
        ("subgroups", Feature::Subgroups),
        ("indirect-first-instance", Feature::IndirectFirstInstance),
    ] {
        if names.iter().any(|n| n == name) {
            features.push(feature);
        }
    }
    let text = |k: &str| info[k].as_str().unwrap_or("").to_string();
    let adapter = AdapterInfo {
        backend: "browser-webgpu".into(),
        vendor: text("vendor"),
        architecture: text("architecture"),
        description: text("description"),
        features: names,
    };
    (Device { handle }, features, adapter)
}

pub fn release(kind: Kind, handle: Handle) {
    // The freed slot may name a new object next; nothing recorded or cached
    // may still refer to it.
    BATCH.with(|b| {
        let recorder = &mut b.borrow_mut().recorder;
        replay(recorder);
        recorder.forget();
    });
    if handle != 0 {
        unsafe { ffi::release(kind_code(kind), handle) }
    }
}

pub fn create_buffer(_: &Device, desc: &BufferDescriptor) -> Handle {
    create_json(BUFFER, &serde_json::to_value(desc).unwrap())
}

pub fn create_texture(_: &Device, desc: &TextureDescriptor) -> Handle {
    create_json(TEXTURE, &serde_json::to_value(desc).unwrap())
}

pub fn create_view(texture: Handle, desc: &TextureViewDescriptor) -> Handle {
    let text = serde_json::to_string(desc).unwrap();
    unsafe { ffi::create_view(texture, text.as_ptr(), text.len() as u32) }
}

pub fn create_sampler(_: &Device, desc: &SamplerDescriptor) -> Handle {
    create_json(SAMPLER, &serde_json::to_value(desc).unwrap())
}

pub fn create_shader_module(_: &Device, label: &str, wgsl: &str) -> Handle {
    unsafe { ffi::create_shader_module(label.as_ptr(), label.len() as u32, wgsl.as_ptr(), wgsl.len() as u32) }
}

pub fn create_compute_pipeline(_: &Device, label: &str, module: Handle, entry: &str) -> Handle {
    create_json(COMPUTE_PIPELINE, &json!({"label": label, "module": module, "entryPoint": entry}))
}

pub fn create_render_pipeline(_: &Device, d: &RenderPipelineDescriptor) -> Handle {
    let mut value = json!({
        "label": d.label,
        "module": d.module.raw,
        "vertexEntry": d.vertex_entry,
        "buffers": d.buffers,
        "cullMode": d.cull_mode,
        "sampleCount": d.sample_count,
        "targets": d.targets,
    });
    if let Some(f) = d.fragment_entry {
        value["fragmentEntry"] = json!(f);
    }
    if let Some(depth) = d.depth {
        value["depth"] = serde_json::to_value(depth).unwrap();
    }
    create_json(RENDER_PIPELINE, &value)
}

pub fn compute_bind_group_layout(pipeline: Handle, index: u32) -> Handle {
    unsafe { ffi::get_bind_group_layout(pipeline, 0, index) }
}

pub fn render_bind_group_layout(pipeline: Handle, index: u32) -> Handle {
    unsafe { ffi::get_bind_group_layout(pipeline, 1, index) }
}

pub fn create_bind_group(_: &Device, label: &str, layout: Handle, entries: &[BindGroupEntry]) -> Handle {
    let entries: Vec<Value> = entries
        .iter()
        .map(|e| match &e.resource {
            BindingResource::Buffer { buffer, offset, size } => {
                json!({"binding": e.binding, "buffer": buffer.raw, "offset": offset, "size": size})
            }
            BindingResource::Sampler(s) => json!({"binding": e.binding, "sampler": s.raw}),
            BindingResource::TextureView(v) => json!({"binding": e.binding, "view": v.raw}),
        })
        .collect();
    create_json(BIND_GROUP, &json!({"label": label, "layout": layout, "entries": entries}))
}

pub fn create_timestamp_queries(_: &Device, count: u32) -> Handle {
    create_json(QUERY_SET, &json!({"type": "timestamp", "count": count}))
}

pub fn create_command_encoder(_: &Device) -> Handle {
    unsafe { ffi::create_command_encoder() }
}

pub fn write_buffer(_: &Device, buffer: Handle, offset: u64, data: &[u8]) {
    unsafe { ffi::write_buffer(buffer, offset as f64, data.as_ptr(), data.len() as u32) }
}

pub fn write_texture(_: &Device, texture: Handle, mip: u32, layer: u32, data: &[u8], bytes_per_row: u32, size: Extent) {
    unsafe {
        ffi::write_texture(texture, mip, layer, data.as_ptr(), data.len() as u32, bytes_per_row, size.width, size.height, size.depth_or_array_layers)
    }
}

pub fn submit(_: &Device, buffers: &[Handle]) {
    flush();
    unsafe { ffi::submit(buffers.as_ptr(), buffers.len() as u32) }
}

fn timestamps_json(t: Option<PassTimestamps>) -> Value {
    match t {
        Some(t) => json!({"querySet": t.query_set.raw, "begin": t.begin, "end": t.end}),
        None => Value::Null,
    }
}

pub fn begin_compute_pass(encoder: Handle, label: &str, t: Option<PassTimestamps>) -> Handle {
    let text = json!({"label": label, "timestamps": timestamps_json(t)}).to_string();
    unsafe { ffi::begin_compute_pass(encoder, text.as_ptr(), text.len() as u32) }
}

pub fn begin_render_pass(encoder: Handle, d: &RenderPassDescriptor) -> Handle {
    let color: Vec<Value> = d
        .color
        .iter()
        .map(|c| {
            json!({
                "view": c.view.raw,
                "resolveTarget": c.resolve_target.map(|r| r.raw),
                "clear": match c.load { LoadOp::Clear(v) => json!(v), LoadOp::Load => Value::Null },
                "store": c.store,
            })
        })
        .collect();
    let depth = d.depth.as_ref().map(|z| json!({"view": z.view.raw, "clear": z.clear, "store": z.store}));
    let text = json!({"label": d.label, "color": color, "depth": depth, "timestamps": timestamps_json(d.timestamps)}).to_string();
    unsafe { ffi::begin_render_pass(encoder, text.as_ptr(), text.len() as u32) }
}

pub fn copy_buffer_to_buffer(encoder: Handle, src: Handle, so: u64, dst: Handle, d: u64, size: u64) {
    unsafe { ffi::copy_buffer_to_buffer(encoder, src, so as f64, dst, d as f64, size as f64) }
}

pub fn clear_buffer(encoder: Handle, buffer: Handle, offset: u64, size: u64) {
    unsafe { ffi::clear_buffer(encoder, buffer, offset as f64, size as f64) }
}

pub fn resolve_queries(encoder: Handle, set: Handle, first: u32, count: u32, dst: Handle, offset: u64) {
    unsafe { ffi::resolve_queries(encoder, set, first, count, dst, offset as f64) }
}

pub fn copy_texture_to_buffer(_: Handle, _: Handle, _: Handle, _: u32, _: Extent) {
    unimplemented!("the web host captures the canvas directly")
}

pub fn finish(encoder: Handle) -> Handle {
    flush();
    unsafe { ffi::finish(encoder) }
}

pub fn compute_set_pipeline(pass: Handle, p: Handle) {
    if !queue(pass, Command::SetPipeline { pipeline: p }) {
        unsafe { ffi::set_pipeline(pass, p) }
    }
}
pub fn render_set_pipeline(pass: Handle, p: Handle) {
    compute_set_pipeline(pass, p)
}
pub fn compute_set_bind_group(pass: Handle, i: u32, g: Handle) {
    if !queue(pass, Command::SetBindGroup { index: i, group: g }) {
        unsafe { ffi::set_bind_group(pass, i, g) }
    }
}
pub fn render_set_bind_group(pass: Handle, i: u32, g: Handle) {
    compute_set_bind_group(pass, i, g)
}
pub fn dispatch(pass: Handle, x: u32, y: u32, z: u32) {
    if !queue(pass, Command::Dispatch { x, y, z }) {
        unsafe { ffi::dispatch(pass, x, y, z) }
    }
}
pub fn dispatch_indirect(pass: Handle, b: Handle, o: u64) {
    if !queue(pass, Command::DispatchIndirect { buffer: b, offset: o }) {
        unsafe { ffi::dispatch_indirect(pass, b, o as f64) }
    }
}
pub fn end_compute_pass(pass: Handle) {
    end_pass(pass)
}
pub fn end_render_pass(pass: Handle) {
    end_compute_pass(pass)
}
/// `u64::MAX` (WGPU_WHOLE_SIZE) crosses as -1, which the host turns into an omitted size.
fn whole(size: u64) -> f64 {
    if size == u64::MAX { -1.0 } else { size as f64 }
}

pub fn set_vertex_buffer(pass: Handle, slot: u32, b: Handle, o: u64, s: u64) {
    if !queue(pass, Command::SetVertexBuffer { slot, buffer: b, offset: o, size: s }) {
        unsafe { ffi::set_vertex_buffer(pass, slot, b, o as f64, whole(s)) }
    }
}
pub fn set_index_buffer(pass: Handle, b: Handle, f: IndexFormat, o: u64, s: u64) {
    let uint32 = f == IndexFormat::Uint32;
    if !queue(pass, Command::SetIndexBuffer { buffer: b, uint32, offset: o, size: s }) {
        unsafe { ffi::set_index_buffer(pass, b, uint32 as u32, o as f64, whole(s)) }
    }
}
pub fn draw(pass: Handle, v: u32, i: u32, fv: u32, fi: u32) {
    if !queue(pass, Command::Draw { vertices: v, instances: i, first_vertex: fv, first_instance: fi }) {
        unsafe { ffi::draw(pass, v, i, fv, fi) }
    }
}
pub fn draw_indexed(pass: Handle, n: u32, i: u32, fi: u32, bv: i32, fin: u32) {
    let command = Command::DrawIndexed { indices: n, instances: i, first_index: fi, base_vertex: bv, first_instance: fin };
    if !queue(pass, command) {
        unsafe { ffi::draw_indexed(pass, n, i, fi, bv, fin) }
    }
}
pub fn draw_indirect(pass: Handle, b: Handle, o: u64) {
    if !queue(pass, Command::DrawIndirect { buffer: b, offset: o }) {
        unsafe { ffi::draw_indirect(pass, b, o as f64) }
    }
}
pub fn draw_indexed_indirect(pass: Handle, b: Handle, o: u64) {
    if !queue(pass, Command::DrawIndexedIndirect { buffer: b, offset: o }) {
        unsafe { ffi::draw_indexed_indirect(pass, b, o as f64) }
    }
}

pub fn read_buffer(_: &Device, buffer: Handle, offset: u64, size: u64) -> ReadTicket {
    let ticket = NEXT_TICKET.with(|t| {
        let v = t.get();
        t.set(v + 1);
        v
    });
    unsafe { ffi::read_buffer(buffer, offset as f64, size as f64, ticket) }
    ReadTicket(ticket)
}

pub fn completed_reads(_: &Device) -> Vec<(ReadTicket, Vec<u8>)> {
    READS.with(|r| r.borrow_mut().drain(..).collect())
}

pub fn surface_view(_: &Device) -> Handle {
    unsafe { ffi::surface_view() }
}

pub fn surface_format(_: &Device) -> TextureFormat {
    if unsafe { ffi::surface_format() } == 1 { TextureFormat::Bgra8Unorm } else { TextureFormat::Rgba8Unorm }
}

pub fn present(_: &Device) {}
pub fn resize_surface(_: &Device, _: u32, _: u32) {}
pub fn wait_idle(_: &Device) {}
pub fn surface_texture(_: &Device) -> Option<Handle> {
    None
}

pub fn take_errors(_: &Device) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let len = unsafe { ffi::take_error(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if len == 0 {
            break;
        }
        out.push(String::from_utf8_lossy(&buf[..len.min(buf.len())]).into_owned());
    }
    out
}

pub fn now_ms() -> f64 {
    unsafe { ffi::now() }
}

pub fn log(message: &str) {
    unsafe { ffi::log(message.as_ptr(), message.len() as u32) }
}
