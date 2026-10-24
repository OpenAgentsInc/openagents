//! Native backend: a `webgpu.h` implementation (wgpu-native, or Dawn) reached
//! through bindings that `build.rs` generates from the implementation's header.
//! Handles are the implementation's object pointers.
//!
//! wgpu-native is the implementation this backend is tested against. The Dawn
//! differences that are cheap to keep (device polling) sit behind `af_dawn`.
//!
//! Without a window ([`NativeOptions::surface`] is `None`) frames render into
//! an offscreen texture that [`crate::Gpu::surface_texture`] exposes.
//!
//! Environment: `AF_WGPU_LOG=warn|info|debug|trace` forwards wgpu-native's log
//! to stderr; `AF_GPU_DISABLE_FEATURES=timestamp-query,subgroups,...` keeps
//! those optional features off the device, for A/B runs.

// The header's constants keep their C names, including in match patterns.
#![allow(non_upper_case_globals)]

use crate::*;
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_void};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

#[allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code, unnecessary_transmutes, clippy::all)]
mod sys {
    include!(concat!(env!("OUT_DIR"), "/webgpu.rs"));
}
use sys::*;

pub type Handle = usize;

/// `WGPU_WHOLE_SIZE`; a C macro bindgen does not translate.
const WHOLE_SIZE: u64 = u64::MAX;

/// A native window to present into. The handles are borrowed: the caller keeps
/// the window alive for the life of the [`crate::Gpu`].
#[derive(Clone, Copy, Debug)]
pub enum SurfaceSource {
    /// A `CAMetalLayer*` (macOS, iOS).
    MetalLayer(*mut c_void),
    /// An Xlib `Display*` and `Window`.
    Xlib { display: *mut c_void, window: u64 },
    /// A Wayland `wl_display*` and `wl_surface*`.
    Wayland { display: *mut c_void, surface: *mut c_void },
    /// A Win32 `HINSTANCE` and `HWND`.
    Windows { hinstance: *mut c_void, hwnd: *mut c_void },
}

/// How [`crate::Gpu::new_native`] picks an adapter and where frames go.
#[derive(Clone, Debug)]
pub struct NativeOptions {
    /// Initial output size in pixels.
    pub width: u32,
    pub height: u32,
    /// `vulkan`, `metal`, `d3d12`, `gl`, or `None` for the implementation's choice.
    pub backend: Option<String>,
    pub low_power: bool,
    /// A window to present into; `None` renders offscreen.
    pub surface: Option<SurfaceSource>,
    /// Format of the offscreen target (`Rgba8Unorm` or `Bgra8Unorm`).
    pub offscreen_format: TextureFormat,
}

impl Default for NativeOptions {
    fn default() -> Self {
        NativeOptions { width: 1280, height: 720, backend: None, low_power: false, surface: None, offscreen_format: TextureFormat::Rgba8Unorm }
    }
}

struct PendingRead {
    ticket: ReadTicket,
    buffer: WGPUBuffer,
    offset: u64,
    size: u64,
    /// 0 pending, 1 mapped, 2 failed. Boxed so the map callback's pointer stays valid.
    status: Box<AtomicU32>,
}

type ErrorLog = Mutex<Vec<String>>;

pub struct Device {
    instance: WGPUInstance,
    adapter: WGPUAdapter,
    device: WGPUDevice,
    queue: WGPUQueue,
    surface: WGPUSurface,
    format: TextureFormat,
    size: Cell<[u32; 2]>,
    offscreen: Cell<WGPUTexture>,
    current: Cell<WGPUTexture>,
    /// Leaked: device callbacks may run until the implementation drops the device.
    errors: &'static ErrorLog,
    reads: RefCell<Vec<PendingRead>>,
    next_ticket: Cell<u32>,
}

fn sv(s: &str) -> WGPUStringView {
    WGPUStringView { data: s.as_ptr().cast(), length: s.len() }
}

/// # Safety
/// `view` must be a valid string view from the implementation.
unsafe fn text(view: WGPUStringView) -> String {
    if view.data.is_null() {
        return String::new();
    }
    if view.length == usize::MAX {
        return unsafe { CStr::from_ptr(view.data) }.to_string_lossy().into_owned();
    }
    let bytes = unsafe { std::slice::from_raw_parts(view.data.cast::<u8>(), view.length) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn push_error(log: &ErrorLog, message: String) {
    if let Ok(mut e) = log.lock()
        && e.len() < 64
    {
        e.push(message);
    }
}

unsafe extern "C" fn on_uncaptured_error(_: *const WGPUDevice, kind: WGPUErrorType, message: WGPUStringView, log: *mut c_void, _: *mut c_void) {
    let log = unsafe { &*(log as *const ErrorLog) };
    let label = match kind {
        WGPUErrorType_Validation => "validation",
        WGPUErrorType_OutOfMemory => "out of memory",
        WGPUErrorType_Internal => "internal",
        _ => "error",
    };
    push_error(log, format!("{label}: {}", unsafe { text(message) }));
}

unsafe extern "C" fn on_device_lost(_: *const WGPUDevice, reason: WGPUDeviceLostReason, message: WGPUStringView, log: *mut c_void, _: *mut c_void) {
    if reason == WGPUDeviceLostReason_Destroyed || reason == WGPUDeviceLostReason_CallbackCancelled {
        return;
    }
    let log = unsafe { &*(log as *const ErrorLog) };
    push_error(log, format!("device lost ({reason}): {}", unsafe { text(message) }));
}

struct Request<T> {
    done: bool,
    value: T,
    message: String,
}

unsafe extern "C" fn on_adapter(status: WGPURequestAdapterStatus, adapter: WGPUAdapter, message: WGPUStringView, slot: *mut c_void, _: *mut c_void) {
    let slot = unsafe { &mut *(slot as *mut Request<WGPUAdapter>) };
    slot.done = true;
    if status == WGPURequestAdapterStatus_Success {
        slot.value = adapter;
    } else {
        slot.message = unsafe { text(message) };
    }
}

unsafe extern "C" fn on_device(status: WGPURequestDeviceStatus, device: WGPUDevice, message: WGPUStringView, slot: *mut c_void, _: *mut c_void) {
    let slot = unsafe { &mut *(slot as *mut Request<WGPUDevice>) };
    slot.done = true;
    if status == WGPURequestDeviceStatus_Success {
        slot.value = device;
    } else {
        slot.message = unsafe { text(message) };
    }
}

unsafe extern "C" fn on_mapped(status: WGPUMapAsyncStatus, _: WGPUStringView, flag: *mut c_void, _: *mut c_void) {
    let flag = unsafe { &*(flag as *const AtomicU32) };
    flag.store(if status == WGPUMapAsyncStatus_Success { 1 } else { 2 }, Ordering::Release);
}

unsafe extern "C" fn on_work_done(_: WGPUQueueWorkDoneStatus, _: WGPUStringView, flag: *mut c_void, _: *mut c_void) {
    unsafe { &*(flag as *const AtomicU32) }.store(1, Ordering::Release);
}

/// Processes events until `done` or a 10 s timeout.
fn wait_for(instance: WGPUInstance, done: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > Duration::from_secs(10) {
            return false;
        }
        unsafe { wgpuInstanceProcessEvents(instance) };
        if !done() {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    true
}

fn backend_type(name: Option<&str>) -> Result<WGPUBackendType, String> {
    Ok(match name.map(str::to_ascii_lowercase).as_deref() {
        None | Some("") | Some("auto") => WGPUBackendType_Undefined,
        Some("vulkan") => WGPUBackendType_Vulkan,
        Some("metal") => WGPUBackendType_Metal,
        Some("d3d12") => WGPUBackendType_D3D12,
        Some("gl") | Some("opengl") => WGPUBackendType_OpenGL,
        Some(other) => return Err(format!("unknown backend {other}")),
    })
}

fn backend_name(kind: WGPUBackendType) -> &'static str {
    match kind {
        WGPUBackendType_Vulkan => "vulkan",
        WGPUBackendType_Metal => "metal",
        WGPUBackendType_D3D12 => "d3d12",
        WGPUBackendType_D3D11 => "d3d11",
        WGPUBackendType_OpenGL => "gl",
        WGPUBackendType_OpenGLES => "gles",
        WGPUBackendType_Null => "null",
        _ => "unknown",
    }
}

const IMPLEMENTATION: &str = if cfg!(af_dawn) { "dawn" } else { "wgpu-native" };

const FEATURES: [(WGPUFeatureName, Feature, &str); 3] = [
    (WGPUFeatureName_TimestampQuery, Feature::TimestampQuery, "timestamp-query"),
    (WGPUFeatureName_Subgroups, Feature::Subgroups, "subgroups"),
    (WGPUFeatureName_IndirectFirstInstance, Feature::IndirectFirstInstance, "indirect-first-instance"),
];

/// wgpu-native: normalizes timestamps to nanoseconds (as browsers and Dawn
/// report them) and limits the instance to the requested backend.
#[cfg(not(af_dawn))]
fn create_instance(backend: Option<&str>) -> WGPUInstance {
    let backends = match backend.map(str::to_ascii_lowercase).as_deref() {
        Some("vulkan") => WGPUInstanceBackend_Vulkan,
        Some("metal") => WGPUInstanceBackend_Metal,
        Some("d3d12") => WGPUInstanceBackend_DX12,
        Some("gl") | Some("opengl") => WGPUInstanceBackend_GL,
        _ => WGPUInstanceBackend_All,
    };
    let mut extras = WGPUInstanceExtras {
        chain: WGPUChainedStruct { next: null_mut(), sType: WGPUSType_InstanceExtras },
        backends,
        flags: WGPUInstanceFlag_AutomaticTimestampNormalization,
        ..Default::default()
    };
    install_log();
    let desc = WGPUInstanceDescriptor { nextInChain: &mut extras.chain, ..Default::default() };
    unsafe { wgpuCreateInstance(&desc) }
}

#[cfg(not(af_dawn))]
unsafe extern "C" fn on_log(level: WGPULogLevel, message: WGPUStringView, _: *mut c_void) {
    eprintln!("[wgpu-native {level}] {}", unsafe { text(message) });
}

/// `AF_WGPU_LOG=error|warn|info|debug|trace` forwards wgpu-native's log to stderr.
#[cfg(not(af_dawn))]
fn install_log() {
    let level = match std::env::var("AF_WGPU_LOG").unwrap_or_default().as_str() {
        "error" => WGPULogLevel_Error,
        "warn" => WGPULogLevel_Warn,
        "info" => WGPULogLevel_Info,
        "debug" => WGPULogLevel_Debug,
        "trace" => WGPULogLevel_Trace,
        _ => return,
    };
    unsafe {
        wgpuSetLogCallback(Some(on_log), null_mut());
        wgpuSetLogLevel(level);
    }
}

#[cfg(af_dawn)]
fn create_instance(_: Option<&str>) -> WGPUInstance {
    unsafe { wgpuCreateInstance(null()) }
}

fn create_surface(instance: WGPUInstance, source: SurfaceSource) -> WGPUSurface {
    let chain = |s_type| WGPUChainedStruct { next: null_mut(), sType: s_type };
    let mut metal;
    let mut xlib;
    let mut wayland;
    let mut windows;
    let next: *mut WGPUChainedStruct = match source {
        SurfaceSource::MetalLayer(layer) => {
            metal = WGPUSurfaceSourceMetalLayer { chain: chain(WGPUSType_SurfaceSourceMetalLayer), layer };
            &mut metal.chain
        }
        SurfaceSource::Xlib { display, window } => {
            xlib = WGPUSurfaceSourceXlibWindow { chain: chain(WGPUSType_SurfaceSourceXlibWindow), display, window };
            &mut xlib.chain
        }
        SurfaceSource::Wayland { display, surface } => {
            wayland = WGPUSurfaceSourceWaylandSurface { chain: chain(WGPUSType_SurfaceSourceWaylandSurface), display, surface };
            &mut wayland.chain
        }
        SurfaceSource::Windows { hinstance, hwnd } => {
            windows = WGPUSurfaceSourceWindowsHWND { chain: chain(WGPUSType_SurfaceSourceWindowsHWND), hinstance, hwnd };
            &mut windows.chain
        }
    };
    let desc = WGPUSurfaceDescriptor { nextInChain: next, label: sv("surface") };
    unsafe { wgpuInstanceCreateSurface(instance, &desc) }
}

fn surface_format_for(surface: WGPUSurface, adapter: WGPUAdapter) -> TextureFormat {
    let mut caps = WGPUSurfaceCapabilities::default();
    unsafe { wgpuSurfaceGetCapabilities(surface, adapter, &mut caps) };
    let formats = if caps.formats.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(caps.formats, caps.formatCount) } };
    let format = if formats.contains(&WGPUTextureFormat_BGRA8Unorm) || !formats.contains(&WGPUTextureFormat_RGBA8Unorm) {
        TextureFormat::Bgra8Unorm
    } else {
        TextureFormat::Rgba8Unorm
    };
    unsafe { wgpuSurfaceCapabilitiesFreeMembers(caps) };
    format
}

fn configure_surface(dev: &Device, width: u32, height: u32) {
    if dev.surface.is_null() {
        return;
    }
    let config = WGPUSurfaceConfiguration {
        device: dev.device,
        format: texture_format(dev.format),
        usage: (texture_usage::RENDER_ATTACHMENT | texture_usage::COPY_SRC) as u64,
        width: width.max(1),
        height: height.max(1),
        alphaMode: WGPUCompositeAlphaMode_Auto,
        presentMode: WGPUPresentMode_Fifo,
        ..Default::default()
    };
    unsafe { wgpuSurfaceConfigure(dev.surface, &config) };
}

pub fn new_device(options: &NativeOptions) -> Result<(Device, Vec<Feature>, AdapterInfo), String> {
    let backend = backend_type(options.backend.as_deref())?;
    let instance = create_instance(options.backend.as_deref());
    if instance.is_null() {
        return Err(format!("{IMPLEMENTATION}: could not create an instance"));
    }
    let surface = options.surface.map(|s| create_surface(instance, s)).unwrap_or(null_mut());

    let adapter_options = WGPURequestAdapterOptions {
        featureLevel: WGPUFeatureLevel_Core,
        powerPreference: if options.low_power { WGPUPowerPreference_LowPower } else { WGPUPowerPreference_HighPerformance },
        backendType: backend,
        compatibleSurface: surface,
        ..Default::default()
    };
    let mut adapter_slot: Request<WGPUAdapter> = Request { done: false, value: null_mut(), message: String::new() };
    let callback = WGPURequestAdapterCallbackInfo {
        mode: WGPUCallbackMode_AllowProcessEvents,
        callback: Some(on_adapter),
        userdata1: (&mut adapter_slot as *mut Request<WGPUAdapter>).cast(),
        ..Default::default()
    };
    unsafe { wgpuInstanceRequestAdapter(instance, &adapter_options, callback) };
    let slot_ptr = &adapter_slot as *const Request<WGPUAdapter>;
    wait_for(instance, || unsafe { (*slot_ptr).done });
    let adapter = adapter_slot.value;
    if adapter.is_null() {
        unsafe { wgpuInstanceRelease(instance) };
        return Err(format!("{IMPLEMENTATION}: no adapter ({})", adapter_slot.message));
    }

    let mut wanted = Vec::new();
    let mut features = Vec::new();
    let mut names = Vec::new();
    let disabled = std::env::var("AF_GPU_DISABLE_FEATURES").unwrap_or_default();
    for (raw, feature, name) in FEATURES {
        if unsafe { wgpuAdapterHasFeature(adapter, raw) } != 0 && !disabled.split(',').any(|d| d.trim() == name) {
            wanted.push(raw);
            features.push(feature);
            names.push(name.to_string());
        }
    }
    let mut limits = WGPULimits::default();
    unsafe { wgpuAdapterGetLimits(adapter, &mut limits) };
    limits.nextInChain = null_mut();
    // Immediates need a native feature this backend does not request.
    limits.maxImmediateSize = WGPU_LIMIT_U32_UNDEFINED;

    let errors: &'static ErrorLog = Box::leak(Box::new(Mutex::new(Vec::new())));
    let log_ptr = errors as *const ErrorLog as *mut c_void;
    let device_desc = WGPUDeviceDescriptor {
        label: sv("af-gpu"),
        requiredFeatureCount: wanted.len(),
        requiredFeatures: wanted.as_ptr(),
        requiredLimits: &limits,
        deviceLostCallbackInfo: WGPUDeviceLostCallbackInfo {
            mode: WGPUCallbackMode_AllowSpontaneous,
            callback: Some(on_device_lost),
            userdata1: log_ptr,
            ..Default::default()
        },
        uncapturedErrorCallbackInfo: WGPUUncapturedErrorCallbackInfo { callback: Some(on_uncaptured_error), userdata1: log_ptr, ..Default::default() },
        ..Default::default()
    };
    let mut device_slot: Request<WGPUDevice> = Request { done: false, value: null_mut(), message: String::new() };
    let callback = WGPURequestDeviceCallbackInfo {
        mode: WGPUCallbackMode_AllowProcessEvents,
        callback: Some(on_device),
        userdata1: (&mut device_slot as *mut Request<WGPUDevice>).cast(),
        ..Default::default()
    };
    unsafe { wgpuAdapterRequestDevice(adapter, &device_desc, callback) };
    let slot_ptr = &device_slot as *const Request<WGPUDevice>;
    wait_for(instance, || unsafe { (*slot_ptr).done });
    let device = device_slot.value;
    if device.is_null() {
        unsafe {
            wgpuAdapterRelease(adapter);
            wgpuInstanceRelease(instance);
        }
        return Err(format!("{IMPLEMENTATION}: no device ({})", device_slot.message));
    }

    let mut raw_info = WGPUAdapterInfo::default();
    unsafe { wgpuAdapterGetInfo(adapter, &mut raw_info) };
    let description = unsafe { [text(raw_info.device), text(raw_info.description)] }.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    let info = AdapterInfo {
        backend: format!("{IMPLEMENTATION}-{}", backend_name(raw_info.backendType)),
        vendor: unsafe { text(raw_info.vendor) },
        architecture: unsafe { text(raw_info.architecture) },
        description,
        features: names,
    };
    unsafe { wgpuAdapterInfoFreeMembers(raw_info) };

    let format = if surface.is_null() { options.offscreen_format } else { surface_format_for(surface, adapter) };
    let dev = Device {
        instance,
        adapter,
        device,
        queue: unsafe { wgpuDeviceGetQueue(device) },
        surface,
        format,
        size: Cell::new([options.width.max(1), options.height.max(1)]),
        offscreen: Cell::new(null_mut()),
        current: Cell::new(null_mut()),
        errors,
        reads: RefCell::new(Vec::new()),
        next_ticket: Cell::new(1),
    };
    configure_surface(&dev, options.width, options.height);
    Ok((dev, features, info))
}

impl Drop for Device {
    fn drop(&mut self) {
        wait_idle(self);
        unsafe { wgpuInstanceProcessEvents(self.instance) };
        for read in self.reads.borrow_mut().drain(..) {
            if read.status.load(Ordering::Acquire) == 0 {
                // A callback may still arrive; keep its flag alive.
                Box::leak(read.status);
            }
            unsafe { wgpuBufferRelease(read.buffer) };
        }
        unsafe {
            for t in [self.offscreen.get(), self.current.get()] {
                if !t.is_null() {
                    wgpuTextureRelease(t);
                }
            }
            if !self.surface.is_null() {
                wgpuSurfaceUnconfigure(self.surface);
                wgpuSurfaceRelease(self.surface);
            }
            wgpuQueueRelease(self.queue);
            wgpuDeviceRelease(self.device);
            wgpuAdapterRelease(self.adapter);
            wgpuInstanceRelease(self.instance);
        }
    }
}

fn texture_format(f: TextureFormat) -> WGPUTextureFormat {
    match f {
        TextureFormat::Rgba8Unorm => WGPUTextureFormat_RGBA8Unorm,
        TextureFormat::Bgra8Unorm => WGPUTextureFormat_BGRA8Unorm,
        TextureFormat::Rgba16Float => WGPUTextureFormat_RGBA16Float,
        TextureFormat::Rgba32Float => WGPUTextureFormat_RGBA32Float,
        TextureFormat::R32Uint => WGPUTextureFormat_R32Uint,
        TextureFormat::Rg32Uint => WGPUTextureFormat_RG32Uint,
        TextureFormat::R32Float => WGPUTextureFormat_R32Float,
        TextureFormat::Rg16Float => WGPUTextureFormat_RG16Float,
        TextureFormat::Depth32Float => WGPUTextureFormat_Depth32Float,
    }
}

fn vertex_format(f: VertexFormat) -> WGPUVertexFormat {
    match f {
        VertexFormat::Float32x2 => WGPUVertexFormat_Float32x2,
        VertexFormat::Float32x3 => WGPUVertexFormat_Float32x3,
        VertexFormat::Float32x4 => WGPUVertexFormat_Float32x4,
        VertexFormat::Uint32 => WGPUVertexFormat_Uint32,
    }
}

fn compare(f: CompareFunction) -> WGPUCompareFunction {
    match f {
        CompareFunction::Less => WGPUCompareFunction_Less,
        CompareFunction::LessEqual => WGPUCompareFunction_LessEqual,
        CompareFunction::Greater => WGPUCompareFunction_Greater,
        CompareFunction::GreaterEqual => WGPUCompareFunction_GreaterEqual,
        CompareFunction::Always => WGPUCompareFunction_Always,
    }
}

fn filter(f: FilterMode) -> WGPUFilterMode {
    match f {
        FilterMode::Nearest => WGPUFilterMode_Nearest,
        FilterMode::Linear => WGPUFilterMode_Linear,
    }
}

fn address(a: AddressMode) -> WGPUAddressMode {
    match a {
        AddressMode::ClampToEdge => WGPUAddressMode_ClampToEdge,
        AddressMode::Repeat => WGPUAddressMode_Repeat,
    }
}

fn timestamp_writes(t: Option<PassTimestamps>) -> Option<WGPUPassTimestampWrites> {
    t.map(|t| WGPUPassTimestampWrites {
        nextInChain: null_mut(),
        querySet: t.query_set.raw() as WGPUQuerySet,
        beginningOfPassWriteIndex: t.begin,
        endOfPassWriteIndex: t.end,
    })
}

fn opt_ptr<T>(v: &Option<T>) -> *const T {
    v.as_ref().map_or(null(), |v| v as *const T)
}

pub fn release(kind: Kind, h: Handle) {
    if h == 0 {
        return;
    }
    unsafe {
        match kind {
            Kind::Buffer => wgpuBufferRelease(h as WGPUBuffer),
            Kind::Texture => wgpuTextureRelease(h as WGPUTexture),
            Kind::TextureView => wgpuTextureViewRelease(h as WGPUTextureView),
            Kind::Sampler => wgpuSamplerRelease(h as WGPUSampler),
            Kind::ShaderModule => wgpuShaderModuleRelease(h as WGPUShaderModule),
            Kind::BindGroupLayout => wgpuBindGroupLayoutRelease(h as WGPUBindGroupLayout),
            Kind::BindGroup => wgpuBindGroupRelease(h as WGPUBindGroup),
            Kind::ComputePipeline => wgpuComputePipelineRelease(h as WGPUComputePipeline),
            Kind::RenderPipeline => wgpuRenderPipelineRelease(h as WGPURenderPipeline),
            Kind::QuerySet => wgpuQuerySetRelease(h as WGPUQuerySet),
            Kind::CommandEncoder => wgpuCommandEncoderRelease(h as WGPUCommandEncoder),
            Kind::CommandBuffer => wgpuCommandBufferRelease(h as WGPUCommandBuffer),
            Kind::ComputePass => wgpuComputePassEncoderRelease(h as WGPUComputePassEncoder),
            Kind::RenderPass => wgpuRenderPassEncoderRelease(h as WGPURenderPassEncoder),
        }
    }
}

pub fn create_buffer(dev: &Device, d: &BufferDescriptor) -> Handle {
    let desc = WGPUBufferDescriptor { label: sv(d.label), usage: d.usage as u64, size: d.size, ..Default::default() };
    unsafe { wgpuDeviceCreateBuffer(dev.device, &desc) as Handle }
}

pub fn create_texture(dev: &Device, d: &TextureDescriptor) -> Handle {
    let desc = WGPUTextureDescriptor {
        label: sv(d.label),
        usage: d.usage as u64,
        dimension: WGPUTextureDimension_2D,
        size: WGPUExtent3D { width: d.size.width, height: d.size.height, depthOrArrayLayers: d.size.depth_or_array_layers },
        format: texture_format(d.format),
        mipLevelCount: d.mip_level_count,
        sampleCount: d.sample_count,
        ..Default::default()
    };
    unsafe { wgpuDeviceCreateTexture(dev.device, &desc) as Handle }
}

pub fn create_view(texture: Handle, d: &TextureViewDescriptor) -> Handle {
    let desc = WGPUTextureViewDescriptor {
        dimension: match d.dimension {
            None => WGPUTextureViewDimension_Undefined,
            Some(TextureViewDimension::D2) => WGPUTextureViewDimension_2D,
            Some(TextureViewDimension::D2Array) => WGPUTextureViewDimension_2DArray,
        },
        baseMipLevel: d.base_mip_level,
        mipLevelCount: d.mip_level_count.unwrap_or(WGPU_MIP_LEVEL_COUNT_UNDEFINED),
        baseArrayLayer: d.base_array_layer,
        arrayLayerCount: d.array_layer_count.unwrap_or(WGPU_ARRAY_LAYER_COUNT_UNDEFINED),
        aspect: WGPUTextureAspect_All,
        ..Default::default()
    };
    unsafe { wgpuTextureCreateView(texture as WGPUTexture, &desc) as Handle }
}

pub fn create_sampler(dev: &Device, d: &SamplerDescriptor) -> Handle {
    let desc = WGPUSamplerDescriptor {
        label: sv("sampler"),
        addressModeU: address(d.address_mode_u),
        addressModeV: address(d.address_mode_v),
        addressModeW: WGPUAddressMode_ClampToEdge,
        magFilter: filter(d.mag_filter),
        minFilter: filter(d.min_filter),
        mipmapFilter: match d.mipmap_filter {
            FilterMode::Nearest => WGPUMipmapFilterMode_Nearest,
            FilterMode::Linear => WGPUMipmapFilterMode_Linear,
        },
        lodMinClamp: 0.0,
        lodMaxClamp: 32.0,
        compare: WGPUCompareFunction_Undefined,
        maxAnisotropy: d.max_anisotropy.unwrap_or(1).max(1),
        ..Default::default()
    };
    unsafe { wgpuDeviceCreateSampler(dev.device, &desc) as Handle }
}

pub fn create_shader_module(dev: &Device, label: &str, wgsl: &str) -> Handle {
    let mut source = WGPUShaderSourceWGSL { chain: WGPUChainedStruct { next: null_mut(), sType: WGPUSType_ShaderSourceWGSL }, code: sv(wgsl) };
    let desc = WGPUShaderModuleDescriptor { nextInChain: &mut source.chain, label: sv(label) };
    unsafe { wgpuDeviceCreateShaderModule(dev.device, &desc) as Handle }
}

pub fn create_compute_pipeline(dev: &Device, label: &str, module: Handle, entry: &str) -> Handle {
    let desc = WGPUComputePipelineDescriptor {
        label: sv(label),
        compute: WGPUComputeState { module: module as WGPUShaderModule, entryPoint: sv(entry), ..Default::default() },
        ..Default::default()
    };
    unsafe { wgpuDeviceCreateComputePipeline(dev.device, &desc) as Handle }
}

pub fn create_render_pipeline(dev: &Device, d: &RenderPipelineDescriptor) -> Handle {
    let module = d.module.raw() as WGPUShaderModule;
    let attributes: Vec<Vec<WGPUVertexAttribute>> = d
        .buffers
        .iter()
        .map(|b| {
            b.attributes
                .iter()
                .map(|a| WGPUVertexAttribute { nextInChain: null_mut(), format: vertex_format(a.format), offset: a.offset, shaderLocation: a.shader_location })
                .collect()
        })
        .collect();
    let buffers: Vec<WGPUVertexBufferLayout> = d
        .buffers
        .iter()
        .zip(&attributes)
        .map(|(b, a)| WGPUVertexBufferLayout {
            nextInChain: null_mut(),
            stepMode: match b.step_mode {
                VertexStepMode::Vertex => WGPUVertexStepMode_Vertex,
                VertexStepMode::Instance => WGPUVertexStepMode_Instance,
            },
            arrayStride: b.array_stride,
            attributeCount: a.len(),
            attributes: a.as_ptr(),
        })
        .collect();
    let premultiplied = WGPUBlendComponent { operation: WGPUBlendOperation_Add, srcFactor: WGPUBlendFactor_One, dstFactor: WGPUBlendFactor_OneMinusSrcAlpha };
    let blend = WGPUBlendState { color: premultiplied, alpha: premultiplied };
    let targets: Vec<WGPUColorTargetState> = d
        .targets
        .iter()
        .map(|t| WGPUColorTargetState {
            nextInChain: null_mut(),
            format: texture_format(t.format),
            blend: if t.blend { &blend } else { null() },
            writeMask: WGPUColorWriteMask_All,
        })
        .collect();
    let fragment = d.fragment_entry.map(|entry| WGPUFragmentState {
        module,
        entryPoint: sv(entry),
        targetCount: targets.len(),
        targets: targets.as_ptr(),
        ..Default::default()
    });
    let keep = WGPUStencilFaceState {
        compare: WGPUCompareFunction_Always,
        failOp: WGPUStencilOperation_Keep,
        depthFailOp: WGPUStencilOperation_Keep,
        passOp: WGPUStencilOperation_Keep,
    };
    let depth = d.depth.map(|z| WGPUDepthStencilState {
        format: texture_format(z.format),
        depthWriteEnabled: if z.depth_write_enabled { WGPUOptionalBool_True } else { WGPUOptionalBool_False },
        depthCompare: compare(z.depth_compare),
        stencilFront: keep,
        stencilBack: keep,
        stencilReadMask: !0,
        stencilWriteMask: !0,
        ..Default::default()
    });
    let desc = WGPURenderPipelineDescriptor {
        label: sv(d.label),
        vertex: WGPUVertexState {
            module,
            entryPoint: sv(d.vertex_entry),
            bufferCount: buffers.len(),
            buffers: buffers.as_ptr(),
            ..Default::default()
        },
        primitive: WGPUPrimitiveState {
            topology: WGPUPrimitiveTopology_TriangleList,
            frontFace: WGPUFrontFace_CCW,
            cullMode: match d.cull_mode {
                CullMode::None => WGPUCullMode_None,
                CullMode::Back => WGPUCullMode_Back,
            },
            ..Default::default()
        },
        depthStencil: opt_ptr(&depth),
        multisample: WGPUMultisampleState { nextInChain: null_mut(), count: d.sample_count.max(1), mask: !0, alphaToCoverageEnabled: 0 },
        fragment: opt_ptr(&fragment),
        ..Default::default()
    };
    unsafe { wgpuDeviceCreateRenderPipeline(dev.device, &desc) as Handle }
}

pub fn compute_bind_group_layout(pipeline: Handle, index: u32) -> Handle {
    unsafe { wgpuComputePipelineGetBindGroupLayout(pipeline as WGPUComputePipeline, index) as Handle }
}

pub fn render_bind_group_layout(pipeline: Handle, index: u32) -> Handle {
    unsafe { wgpuRenderPipelineGetBindGroupLayout(pipeline as WGPURenderPipeline, index) as Handle }
}

pub fn create_bind_group(dev: &Device, label: &str, layout: Handle, entries: &[BindGroupEntry]) -> Handle {
    let raw: Vec<WGPUBindGroupEntry> = entries
        .iter()
        .map(|e| {
            let mut out = WGPUBindGroupEntry { binding: e.binding, size: WHOLE_SIZE, ..Default::default() };
            match &e.resource {
                BindingResource::Buffer { buffer, offset, size } => {
                    out.buffer = buffer.raw() as WGPUBuffer;
                    out.offset = *offset;
                    out.size = size.unwrap_or(WHOLE_SIZE);
                }
                BindingResource::Sampler(s) => out.sampler = s.raw() as WGPUSampler,
                BindingResource::TextureView(v) => out.textureView = v.raw() as WGPUTextureView,
            }
            out
        })
        .collect();
    let desc = WGPUBindGroupDescriptor {
        label: sv(label),
        layout: layout as WGPUBindGroupLayout,
        entryCount: raw.len(),
        entries: raw.as_ptr(),
        ..Default::default()
    };
    unsafe { wgpuDeviceCreateBindGroup(dev.device, &desc) as Handle }
}

pub fn create_timestamp_queries(dev: &Device, count: u32) -> Handle {
    let desc = WGPUQuerySetDescriptor { label: sv("timestamps"), type_: WGPUQueryType_Timestamp, count, ..Default::default() };
    unsafe { wgpuDeviceCreateQuerySet(dev.device, &desc) as Handle }
}

pub fn create_command_encoder(dev: &Device) -> Handle {
    unsafe { wgpuDeviceCreateCommandEncoder(dev.device, null()) as Handle }
}

pub fn write_buffer(dev: &Device, buffer: Handle, offset: u64, data: &[u8]) {
    unsafe { wgpuQueueWriteBuffer(dev.queue, buffer as WGPUBuffer, offset, data.as_ptr().cast(), data.len()) }
}

pub fn write_texture(dev: &Device, texture: Handle, mip: u32, layer: u32, data: &[u8], bytes_per_row: u32, size: Extent) {
    let dst = WGPUTexelCopyTextureInfo {
        texture: texture as WGPUTexture,
        mipLevel: mip,
        origin: WGPUOrigin3D { x: 0, y: 0, z: layer },
        aspect: WGPUTextureAspect_All,
    };
    let layout = WGPUTexelCopyBufferLayout { offset: 0, bytesPerRow: bytes_per_row, rowsPerImage: size.height };
    let extent = WGPUExtent3D { width: size.width, height: size.height, depthOrArrayLayers: size.depth_or_array_layers };
    unsafe { wgpuQueueWriteTexture(dev.queue, &dst, data.as_ptr().cast(), data.len(), &layout, &extent) }
}

pub fn submit(dev: &Device, buffers: &[Handle]) {
    let raw: Vec<WGPUCommandBuffer> = buffers.iter().map(|&b| b as WGPUCommandBuffer).collect();
    unsafe { wgpuQueueSubmit(dev.queue, raw.len(), raw.as_ptr()) }
}

pub fn begin_compute_pass(encoder: Handle, label: &str, t: Option<PassTimestamps>) -> Handle {
    let writes = timestamp_writes(t);
    let desc = WGPUComputePassDescriptor { nextInChain: null_mut(), label: sv(label), timestampWrites: opt_ptr(&writes) };
    unsafe { wgpuCommandEncoderBeginComputePass(encoder as WGPUCommandEncoder, &desc) as Handle }
}

pub fn begin_render_pass(encoder: Handle, d: &RenderPassDescriptor) -> Handle {
    let color: Vec<WGPURenderPassColorAttachment> = d
        .color
        .iter()
        .map(|c| {
            let (load, clear) = match c.load {
                LoadOp::Clear([r, g, b, a]) => (WGPULoadOp_Clear, WGPUColor { r, g, b, a }),
                LoadOp::Load => (WGPULoadOp_Load, WGPUColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
            };
            WGPURenderPassColorAttachment {
                nextInChain: null_mut(),
                view: c.view.raw() as WGPUTextureView,
                depthSlice: WGPU_DEPTH_SLICE_UNDEFINED,
                resolveTarget: c.resolve_target.map_or(null_mut(), |r| r.raw() as WGPUTextureView),
                loadOp: load,
                storeOp: if c.store { WGPUStoreOp_Store } else { WGPUStoreOp_Discard },
                clearValue: clear,
            }
        })
        .collect();
    let depth = d.depth.as_ref().map(|z| WGPURenderPassDepthStencilAttachment {
        view: z.view.raw() as WGPUTextureView,
        depthLoadOp: if z.clear.is_some() { WGPULoadOp_Clear } else { WGPULoadOp_Load },
        depthStoreOp: if z.store { WGPUStoreOp_Store } else { WGPUStoreOp_Discard },
        depthClearValue: z.clear.unwrap_or(0.0),
        ..Default::default()
    });
    let writes = timestamp_writes(d.timestamps);
    let desc = WGPURenderPassDescriptor {
        label: sv(d.label),
        colorAttachmentCount: color.len(),
        colorAttachments: color.as_ptr(),
        depthStencilAttachment: opt_ptr(&depth),
        timestampWrites: opt_ptr(&writes),
        ..Default::default()
    };
    unsafe { wgpuCommandEncoderBeginRenderPass(encoder as WGPUCommandEncoder, &desc) as Handle }
}

pub fn copy_buffer_to_buffer(encoder: Handle, src: Handle, src_offset: u64, dst: Handle, dst_offset: u64, size: u64) {
    unsafe { wgpuCommandEncoderCopyBufferToBuffer(encoder as WGPUCommandEncoder, src as WGPUBuffer, src_offset, dst as WGPUBuffer, dst_offset, size) }
}

pub fn clear_buffer(encoder: Handle, buffer: Handle, offset: u64, size: u64) {
    unsafe { wgpuCommandEncoderClearBuffer(encoder as WGPUCommandEncoder, buffer as WGPUBuffer, offset, size) }
}

pub fn resolve_queries(encoder: Handle, set: Handle, first: u32, count: u32, dst: Handle, offset: u64) {
    unsafe { wgpuCommandEncoderResolveQuerySet(encoder as WGPUCommandEncoder, set as WGPUQuerySet, first, count, dst as WGPUBuffer, offset) }
}

pub fn copy_texture_to_buffer(encoder: Handle, texture: Handle, buffer: Handle, bytes_per_row: u32, size: Extent) {
    let src = WGPUTexelCopyTextureInfo {
        texture: texture as WGPUTexture,
        mipLevel: 0,
        origin: WGPUOrigin3D { x: 0, y: 0, z: 0 },
        aspect: WGPUTextureAspect_All,
    };
    let dst = WGPUTexelCopyBufferInfo {
        layout: WGPUTexelCopyBufferLayout { offset: 0, bytesPerRow: bytes_per_row, rowsPerImage: size.height },
        buffer: buffer as WGPUBuffer,
    };
    let extent = WGPUExtent3D { width: size.width, height: size.height, depthOrArrayLayers: size.depth_or_array_layers };
    unsafe { wgpuCommandEncoderCopyTextureToBuffer(encoder as WGPUCommandEncoder, &src, &dst, &extent) }
}

/// Finishes and releases the encoder; `webgpu.h` keeps it alive past finish.
pub fn finish(encoder: Handle) -> Handle {
    let encoder = encoder as WGPUCommandEncoder;
    unsafe {
        let buffer = wgpuCommandEncoderFinish(encoder, null());
        wgpuCommandEncoderRelease(encoder);
        buffer as Handle
    }
}

pub fn compute_set_pipeline(pass: Handle, p: Handle) {
    unsafe { wgpuComputePassEncoderSetPipeline(pass as WGPUComputePassEncoder, p as WGPUComputePipeline) }
}
pub fn render_set_pipeline(pass: Handle, p: Handle) {
    unsafe { wgpuRenderPassEncoderSetPipeline(pass as WGPURenderPassEncoder, p as WGPURenderPipeline) }
}
pub fn compute_set_bind_group(pass: Handle, i: u32, g: Handle) {
    unsafe { wgpuComputePassEncoderSetBindGroup(pass as WGPUComputePassEncoder, i, g as WGPUBindGroup, 0, null()) }
}
pub fn render_set_bind_group(pass: Handle, i: u32, g: Handle) {
    unsafe { wgpuRenderPassEncoderSetBindGroup(pass as WGPURenderPassEncoder, i, g as WGPUBindGroup, 0, null()) }
}
pub fn dispatch(pass: Handle, x: u32, y: u32, z: u32) {
    unsafe { wgpuComputePassEncoderDispatchWorkgroups(pass as WGPUComputePassEncoder, x, y, z) }
}
pub fn dispatch_indirect(pass: Handle, b: Handle, o: u64) {
    unsafe { wgpuComputePassEncoderDispatchWorkgroupsIndirect(pass as WGPUComputePassEncoder, b as WGPUBuffer, o) }
}
/// Ends and releases the pass.
pub fn end_compute_pass(pass: Handle) {
    let pass = pass as WGPUComputePassEncoder;
    unsafe {
        wgpuComputePassEncoderEnd(pass);
        wgpuComputePassEncoderRelease(pass);
    }
}
/// Ends and releases the pass.
pub fn end_render_pass(pass: Handle) {
    let pass = pass as WGPURenderPassEncoder;
    unsafe {
        wgpuRenderPassEncoderEnd(pass);
        wgpuRenderPassEncoderRelease(pass);
    }
}
pub fn set_vertex_buffer(pass: Handle, slot: u32, b: Handle, o: u64, s: u64) {
    unsafe { wgpuRenderPassEncoderSetVertexBuffer(pass as WGPURenderPassEncoder, slot, b as WGPUBuffer, o, s) }
}
pub fn set_index_buffer(pass: Handle, b: Handle, f: IndexFormat, o: u64, s: u64) {
    let format = match f {
        IndexFormat::Uint16 => WGPUIndexFormat_Uint16,
        IndexFormat::Uint32 => WGPUIndexFormat_Uint32,
    };
    unsafe { wgpuRenderPassEncoderSetIndexBuffer(pass as WGPURenderPassEncoder, b as WGPUBuffer, format, o, s) }
}
pub fn draw(pass: Handle, v: u32, i: u32, fv: u32, fi: u32) {
    unsafe { wgpuRenderPassEncoderDraw(pass as WGPURenderPassEncoder, v, i, fv, fi) }
}
pub fn draw_indexed(pass: Handle, n: u32, i: u32, fi: u32, bv: i32, fin: u32) {
    unsafe { wgpuRenderPassEncoderDrawIndexed(pass as WGPURenderPassEncoder, n, i, fi, bv, fin) }
}
pub fn draw_indirect(pass: Handle, b: Handle, o: u64) {
    unsafe { wgpuRenderPassEncoderDrawIndirect(pass as WGPURenderPassEncoder, b as WGPUBuffer, o) }
}
pub fn draw_indexed_indirect(pass: Handle, b: Handle, o: u64) {
    unsafe { wgpuRenderPassEncoderDrawIndexedIndirect(pass as WGPURenderPassEncoder, b as WGPUBuffer, o) }
}

pub fn read_buffer(dev: &Device, buffer: Handle, offset: u64, size: u64) -> ReadTicket {
    let ticket = ReadTicket(dev.next_ticket.get());
    dev.next_ticket.set(ticket.0.wrapping_add(1).max(1));
    let buffer = buffer as WGPUBuffer;
    let status = Box::new(AtomicU32::new(0));
    let callback = WGPUBufferMapCallbackInfo {
        mode: WGPUCallbackMode_AllowProcessEvents,
        callback: Some(on_mapped),
        userdata1: (&*status as *const AtomicU32).cast_mut().cast(),
        ..Default::default()
    };
    unsafe {
        wgpuBufferAddRef(buffer);
        wgpuBufferMapAsync(buffer, WGPUMapMode_Read, offset as usize, size as usize, callback);
    }
    dev.reads.borrow_mut().push(PendingRead { ticket, buffer, offset, size, status });
    ticket
}

/// Polls the device without blocking, then returns the reads that finished.
pub fn completed_reads(dev: &Device) -> Vec<(ReadTicket, Vec<u8>)> {
    poll(dev, false);
    let mut out = Vec::new();
    dev.reads.borrow_mut().retain(|read| match read.status.load(Ordering::Acquire) {
        0 => true,
        state => {
            unsafe {
                if state == 1 {
                    let ptr = wgpuBufferGetConstMappedRange(read.buffer, read.offset as usize, read.size as usize);
                    if !ptr.is_null() {
                        out.push((read.ticket, std::slice::from_raw_parts(ptr.cast::<u8>(), read.size as usize).to_vec()));
                    }
                    wgpuBufferUnmap(read.buffer);
                } else {
                    push_error(dev.errors, format!("buffer read {} failed", read.ticket.0));
                }
                wgpuBufferRelease(read.buffer);
            }
            false
        }
    });
    out
}

fn ensure_offscreen(dev: &Device) -> WGPUTexture {
    let existing = dev.offscreen.get();
    if !existing.is_null() {
        return existing;
    }
    let [width, height] = dev.size.get();
    let desc = TextureDescriptor {
        label: "offscreen target",
        size: Extent { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        format: dev.format,
        usage: texture_usage::RENDER_ATTACHMENT | texture_usage::COPY_SRC | texture_usage::TEXTURE_BINDING,
    };
    let texture = create_texture(dev, &desc) as WGPUTexture;
    dev.offscreen.set(texture);
    texture
}

pub fn surface_view(dev: &Device) -> Handle {
    let texture = if dev.surface.is_null() {
        ensure_offscreen(dev)
    } else {
        if dev.current.get().is_null() {
            let mut st = WGPUSurfaceTexture::default();
            unsafe { wgpuSurfaceGetCurrentTexture(dev.surface, &mut st) };
            if st.status != WGPUSurfaceGetCurrentTextureStatus_SuccessOptimal && st.status != WGPUSurfaceGetCurrentTextureStatus_SuccessSuboptimal {
                push_error(dev.errors, format!("surface texture unavailable (status {}); rendering offscreen this frame", st.status));
                if !st.texture.is_null() {
                    unsafe { wgpuTextureRelease(st.texture) };
                }
                return create_view(ensure_offscreen(dev) as Handle, &TextureViewDescriptor::default());
            }
            dev.current.set(st.texture);
        }
        dev.current.get()
    };
    create_view(texture as Handle, &TextureViewDescriptor::default())
}

pub fn surface_format(dev: &Device) -> TextureFormat {
    dev.format
}

pub fn present(dev: &Device) {
    let current = dev.current.replace(null_mut());
    if !current.is_null() {
        unsafe {
            wgpuSurfacePresent(dev.surface);
            wgpuTextureRelease(current);
        }
    }
}

pub fn resize_surface(dev: &Device, width: u32, height: u32) {
    let size = [width.max(1), height.max(1)];
    if dev.size.get() == size {
        return;
    }
    dev.size.set(size);
    let old = dev.offscreen.replace(null_mut());
    if !old.is_null() {
        unsafe { wgpuTextureRelease(old) };
    }
    configure_surface(dev, size[0], size[1]);
}

/// Processes device events. With `wait`, first blocks (by polling) until the
/// work submitted so far finishes, or 10 s pass.
fn poll(dev: &Device, wait: bool) {
    if wait {
        let flag = Box::new(AtomicU32::new(0));
        let callback = WGPUQueueWorkDoneCallbackInfo {
            mode: WGPUCallbackMode_AllowProcessEvents,
            callback: Some(on_work_done),
            userdata1: (&*flag as *const AtomicU32).cast_mut().cast(),
            ..Default::default()
        };
        unsafe { wgpuQueueOnSubmittedWorkDone(dev.queue, callback) };
        if !wait_for(dev.instance, || flag.load(Ordering::Acquire) != 0) {
            push_error(dev.errors, "timed out waiting for submitted GPU work".into());
            // The callback may still arrive and write the flag.
            Box::leak(flag);
        }
    }
    #[cfg(not(af_dawn))]
    unsafe {
        wgpuDevicePoll(dev.device, 0, null());
    }
    unsafe { wgpuInstanceProcessEvents(dev.instance) };
}

/// Blocks until submitted work finishes.
pub fn wait_idle(dev: &Device) {
    poll(dev, true);
}

/// The offscreen target, with a reference the caller's `Texture` releases.
pub fn surface_texture(dev: &Device) -> Option<Handle> {
    if !dev.surface.is_null() {
        return None;
    }
    let texture = ensure_offscreen(dev);
    unsafe { wgpuTextureAddRef(texture) };
    Some(texture as Handle)
}

pub fn take_errors(dev: &Device) -> Vec<String> {
    dev.errors.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default()
}

pub fn now_ms() -> f64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

pub fn log(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_views_round_trip() {
        let s = "label";
        assert_eq!(unsafe { text(sv(s)) }, "label");
        assert_eq!(unsafe { text(WGPUStringView { data: null(), length: usize::MAX }) }, "");
        let c = c"null-terminated";
        assert_eq!(unsafe { text(WGPUStringView { data: c.as_ptr(), length: usize::MAX }) }, "null-terminated");
    }

    #[test]
    fn usage_bits_match_the_header() {
        assert_eq!(buffer_usage::MAP_READ as u64, WGPUBufferUsage_MapRead);
        assert_eq!(buffer_usage::COPY_DST as u64, WGPUBufferUsage_CopyDst);
        assert_eq!(buffer_usage::STORAGE as u64, WGPUBufferUsage_Storage);
        assert_eq!(buffer_usage::INDIRECT as u64, WGPUBufferUsage_Indirect);
        assert_eq!(buffer_usage::QUERY_RESOLVE as u64, WGPUBufferUsage_QueryResolve);
        assert_eq!(texture_usage::RENDER_ATTACHMENT as u64, WGPUTextureUsage_RenderAttachment);
        assert_eq!(texture_usage::STORAGE_BINDING as u64, WGPUTextureUsage_StorageBinding);
    }

    #[test]
    fn backend_names_parse() {
        assert_eq!(backend_type(Some("Vulkan")).unwrap(), WGPUBackendType_Vulkan);
        assert_eq!(backend_type(None).unwrap(), WGPUBackendType_Undefined);
        assert!(backend_type(Some("glide")).is_err());
    }

    /// Runs only where an adapter exists; CI containers without a GPU skip it.
    #[test]
    fn device_round_trips_a_buffer() {
        let Ok(gpu) = Gpu::new_native(&NativeOptions { width: 64, height: 64, ..Default::default() }) else {
            eprintln!("no adapter; skipping");
            return;
        };
        let src = gpu.create_buffer_init("src", buffer_usage::COPY_SRC, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let dst = gpu.create_buffer(&BufferDescriptor { label: "dst", size: 8, usage: buffer_usage::MAP_READ | buffer_usage::COPY_DST });
        let mut encoder = gpu.create_command_encoder();
        encoder.copy_buffer_to_buffer(&src, 0, &dst, 0, 8);
        gpu.submit(vec![encoder.finish()]);
        let ticket = gpu.read_buffer(&dst, 0, 8);
        gpu.wait_idle();
        let reads = gpu.completed_reads();
        assert_eq!(reads, vec![(ticket, vec![1, 2, 3, 4, 5, 6, 7, 8])]);
        assert!(gpu.shader_errors().is_empty());
    }
}
