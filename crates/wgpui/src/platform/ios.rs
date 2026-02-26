use std::ffi::{c_char, c_void};
use std::time::Duration;

use crate::color::Hsla;
use crate::components::hud::{DotShape, DotsGrid, DotsOrigin};
use crate::geometry::{Bounds, Point, Size};
use crate::renderer::Renderer;
use crate::scene::{Quad, Scene};
use crate::theme;
use crate::{Component, PaintContext, TextSystem};
use web_time::Instant;

const GRID_DOT_DISTANCE: f32 = 32.0;
const TAP_DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputTarget {
    None = 0,
    Composer = 1,
    AuthEmail = 2,
    AuthCode = 3,
}

impl InputTarget {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Composer,
            2 => Self::AuthEmail,
            3 => Self::AuthCode,
            _ => Self::None,
        }
    }

    fn as_u8(self) -> u8 {
        self as u8
    }
}

/// iOS renderer bridge state.
///
/// This struct intentionally owns only rendering/input bridge state.
/// Product-domain state is expected to live outside WGPUI and flow in via
/// explicit bridge calls.
pub struct IosBackgroundState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    scale_factor: f32,
    created_at: Instant,
    last_tap_at: Option<Instant>,

    composer_text: String,
    auth_email: String,
    auth_code: String,
    empty_title: String,
    empty_detail: String,
    auth_status: String,
    operator_status: String,

    active_input_target: InputTarget,

    send_requested: bool,
    new_thread_requested: bool,
    interrupt_requested: bool,
    model_cycle_requested: bool,
    reasoning_cycle_requested: bool,
    send_code_requested: bool,
    verify_code_requested: bool,
    sign_out_requested: bool,
    refresh_workers_requested: bool,
    connect_stream_requested: bool,
    disconnect_stream_requested: bool,
    send_handshake_requested: bool,
    thread_read_requested: bool,
    stop_worker_requested: bool,
    refresh_snapshot_requested: bool,
    mission_retention_cycle_requested: bool,
    mission_watch_active_requested: bool,
    mission_watchlist_only_toggle_requested: bool,
    mission_order_toggle_requested: bool,
    mission_alert_errors_toggle_requested: bool,
    mission_alert_stuck_turns_toggle_requested: bool,
    mission_alert_reconnect_storms_toggle_requested: bool,
    submit_requested: bool,

    mission_mutations_enabled: bool,
    mission_retention_profile: u8,
    mission_watchlist_only: bool,
    mission_order_newest_first: bool,
    mission_alert_errors_enabled: bool,
    mission_alert_stuck_turns_enabled: bool,
    mission_alert_reconnect_storms_enabled: bool,
    mission_filter: u8,
    mission_pin_critical: bool,
}

impl IosBackgroundState {
    /// Create renderer from a live CAMetalLayer pointer.
    ///
    /// # Safety
    ///
    /// `layer_ptr` must point to a valid CoreAnimation layer with a lifetime
    /// that outlives this renderer state.
    pub unsafe fn new(
        layer_ptr: *mut c_void,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Result<Box<Self>, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_ptr))
        }
        .map_err(|error| format!("create_surface_unsafe: {error:?}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "no adapter".to_string())?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(|error| format!("request_device: {error:?}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|candidate| candidate.is_srgb())
            .copied()
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| "surface formats empty".to_string())?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, format);
        let text_system = TextSystem::new(scale_factor.max(0.1));

        Ok(Box::new(Self {
            surface,
            device,
            queue,
            config,
            renderer,
            text_system,
            scale_factor: scale_factor.max(0.1),
            created_at: Instant::now(),
            last_tap_at: None,
            composer_text: String::new(),
            auth_email: String::new(),
            auth_code: String::new(),
            empty_title: "iOS bridge ready".to_string(),
            empty_detail: "Domain lanes were extracted from WGPUI.".to_string(),
            auth_status: String::new(),
            operator_status: String::new(),
            active_input_target: InputTarget::None,
            send_requested: false,
            new_thread_requested: false,
            interrupt_requested: false,
            model_cycle_requested: false,
            reasoning_cycle_requested: false,
            send_code_requested: false,
            verify_code_requested: false,
            sign_out_requested: false,
            refresh_workers_requested: false,
            connect_stream_requested: false,
            disconnect_stream_requested: false,
            send_handshake_requested: false,
            thread_read_requested: false,
            stop_worker_requested: false,
            refresh_snapshot_requested: false,
            mission_retention_cycle_requested: false,
            mission_watch_active_requested: false,
            mission_watchlist_only_toggle_requested: false,
            mission_order_toggle_requested: false,
            mission_alert_errors_toggle_requested: false,
            mission_alert_stuck_turns_toggle_requested: false,
            mission_alert_reconnect_storms_toggle_requested: false,
            submit_requested: false,
            mission_mutations_enabled: true,
            mission_retention_profile: 0,
            mission_watchlist_only: false,
            mission_order_newest_first: true,
            mission_alert_errors_enabled: true,
            mission_alert_stuck_turns_enabled: true,
            mission_alert_reconnect_storms_enabled: true,
            mission_filter: 0,
            mission_pin_critical: false,
        }))
    }

    pub fn render(&mut self) -> Result<(), String> {
        let scale = self.scale_factor.max(0.1);
        let logical = Size::new(
            self.config.width as f32 / scale,
            self.config.height as f32 / scale,
        );

        let mut scene = Scene::new();
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, logical.width, logical.height))
                .with_background(theme::bg::APP),
        );

        {
            let mut paint = PaintContext::new(&mut scene, &mut self.text_system, self.scale_factor);

            let mut dots_grid = DotsGrid::new()
                .color(Hsla::new(0.0, 0.0, 0.30, 0.26))
                .shape(DotShape::Cross)
                .distance(GRID_DOT_DISTANCE)
                .size(5.0)
                .cross_thickness(1.0)
                .origin(DotsOrigin::Center);
            dots_grid.paint(
                Bounds::new(0.0, 0.0, logical.width, logical.height),
                &mut paint,
            );

            let uptime = self.created_at.elapsed().as_secs();
            let header = format!("iOS bridge active ({uptime}s)");
            paint.scene.draw_text(paint.text.layout(
                &header,
                Point::new(16.0, 18.0),
                12.0,
                theme::text::PRIMARY,
            ));
            paint.scene.draw_text(paint.text.layout(
                &self.empty_detail,
                Point::new(16.0, 36.0),
                11.0,
                theme::text::MUTED,
            ));
        }

        self.renderer.resize(&self.queue, logical, scale);
        if self.text_system.is_dirty() {
            self.renderer.update_atlas(
                &self.queue,
                self.text_system.atlas_data(),
                self.text_system.atlas_size(),
            );
            self.text_system.mark_clean();
        }

        let output = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(error) => return Err(format!("surface error: {error:?}")),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("WGPUI iOS Encoder"),
            });

        self.renderer
            .prepare(&self.device, &self.queue, &scene, scale);
        self.renderer.render(&mut encoder, &view);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn handle_tap(&mut self, _x: f32, _y: f32) {
        let now = Instant::now();
        if self
            .last_tap_at
            .is_some_and(|last| now.saturating_duration_since(last) < TAP_DEBOUNCE)
        {
            return;
        }
        self.last_tap_at = Some(now);

        match self.active_input_target {
            InputTarget::Composer => self.send_requested = true,
            InputTarget::AuthEmail => self.send_code_requested = true,
            InputTarget::AuthCode => {
                self.verify_code_requested = true;
                self.submit_requested = true;
            }
            InputTarget::None => self.new_thread_requested = true,
        }
    }

    pub fn clear_codex_messages(&mut self) {}

    pub fn push_codex_message(
        &mut self,
        _role: u8,
        _text_ptr: *const u8,
        _text_len: usize,
        _streaming: bool,
    ) {
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn set_codex_context_utf8(
        &mut self,
        _thread_ptr: *const u8,
        _thread_len: usize,
        _turn_ptr: *const u8,
        _turn_len: usize,
        _model_ptr: *const u8,
        _model_len: usize,
        _reasoning_ptr: *const u8,
        _reasoning_len: usize,
    ) {
    }

    pub fn set_empty_state_utf8(
        &mut self,
        title_ptr: *const u8,
        title_len: usize,
        detail_ptr: *const u8,
        detail_len: usize,
    ) {
        Self::set_utf8_string(&mut self.empty_title, title_ptr, title_len);
        Self::set_utf8_string(&mut self.empty_detail, detail_ptr, detail_len);
    }

    pub fn set_auth_fields_utf8(
        &mut self,
        email_ptr: *const u8,
        email_len: usize,
        code_ptr: *const u8,
        code_len: usize,
        auth_status_ptr: *const u8,
        auth_status_len: usize,
    ) {
        Self::set_utf8_string(&mut self.auth_email, email_ptr, email_len);
        Self::set_utf8_string(&mut self.auth_code, code_ptr, code_len);
        Self::set_utf8_string(&mut self.auth_status, auth_status_ptr, auth_status_len);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn set_operator_status_utf8(
        &mut self,
        worker_status_ptr: *const u8,
        worker_status_len: usize,
        stream_status_ptr: *const u8,
        stream_status_len: usize,
        handshake_status_ptr: *const u8,
        handshake_status_len: usize,
        device_status_ptr: *const u8,
        device_status_len: usize,
        telemetry_ptr: *const u8,
        telemetry_len: usize,
        events_ptr: *const u8,
        events_len: usize,
        control_ptr: *const u8,
        control_len: usize,
    ) {
        let worker = Self::read_utf8_string(worker_status_ptr, worker_status_len);
        let stream = Self::read_utf8_string(stream_status_ptr, stream_status_len);
        let handshake = Self::read_utf8_string(handshake_status_ptr, handshake_status_len);
        let device = Self::read_utf8_string(device_status_ptr, device_status_len);
        let telemetry = Self::read_utf8_string(telemetry_ptr, telemetry_len);
        let events = Self::read_utf8_string(events_ptr, events_len);
        let control = Self::read_utf8_string(control_ptr, control_len);
        self.operator_status = format!(
            "worker={worker} stream={stream} handshake={handshake} device={device} telemetry={telemetry} events={events} control={control}"
        );
    }

    pub fn clear_mission_data(&mut self) {}

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn push_mission_worker(
        &mut self,
        _worker_id_ptr: *const u8,
        _worker_id_len: usize,
        _status_ptr: *const u8,
        _status_len: usize,
        _heartbeat_state_ptr: *const u8,
        _heartbeat_state_len: usize,
        _latest_seq: i64,
        _lag_events: i64,
        _reconnect_state_ptr: *const u8,
        _reconnect_state_len: usize,
        _last_event_at_ptr: *const u8,
        _last_event_at_len: usize,
        _running_turns: u64,
        _queued_requests: u64,
        _failed_requests: u64,
    ) {
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn push_mission_thread(
        &mut self,
        _worker_id_ptr: *const u8,
        _worker_id_len: usize,
        _thread_id_ptr: *const u8,
        _thread_id_len: usize,
        _active_turn_id_ptr: *const u8,
        _active_turn_id_len: usize,
        _last_summary_ptr: *const u8,
        _last_summary_len: usize,
        _last_event_at_ptr: *const u8,
        _last_event_at_len: usize,
        _freshness_seq: i64,
        _unread_count: u64,
        _muted: bool,
    ) {
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn push_mission_timeline_entry(
        &mut self,
        _worker_id_ptr: *const u8,
        _worker_id_len: usize,
        _thread_id_ptr: *const u8,
        _thread_id_len: usize,
        _role_ptr: *const u8,
        _role_len: usize,
        _text_ptr: *const u8,
        _text_len: usize,
        _is_streaming: bool,
        _turn_id_ptr: *const u8,
        _turn_id_len: usize,
        _item_id_ptr: *const u8,
        _item_id_len: usize,
        _occurred_at_ptr: *const u8,
        _occurred_at_len: usize,
    ) {
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn push_mission_event(
        &mut self,
        _id: u64,
        _topic_ptr: *const u8,
        _topic_len: usize,
        _seq: i64,
        _worker_id_ptr: *const u8,
        _worker_id_len: usize,
        _thread_id_ptr: *const u8,
        _thread_id_len: usize,
        _turn_id_ptr: *const u8,
        _turn_id_len: usize,
        _request_id_ptr: *const u8,
        _request_id_len: usize,
        _event_type_ptr: *const u8,
        _event_type_len: usize,
        _method_ptr: *const u8,
        _method_len: usize,
        _summary_ptr: *const u8,
        _summary_len: usize,
        _severity: u8,
        _occurred_at_ptr: *const u8,
        _occurred_at_len: usize,
        _payload_ptr: *const u8,
        _payload_len: usize,
        _resync_marker: bool,
    ) {
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "FFI compatibility for iOS bridge payload contract"
    )]
    pub fn push_mission_request(
        &mut self,
        _request_id_ptr: *const u8,
        _request_id_len: usize,
        _worker_id_ptr: *const u8,
        _worker_id_len: usize,
        _thread_id_ptr: *const u8,
        _thread_id_len: usize,
        _method_ptr: *const u8,
        _method_len: usize,
        _state_ptr: *const u8,
        _state_len: usize,
        _occurred_at_ptr: *const u8,
        _occurred_at_len: usize,
        _error_code_ptr: *const u8,
        _error_code_len: usize,
        _error_message_ptr: *const u8,
        _error_message_len: usize,
        _retryable: bool,
        _response_ptr: *const u8,
        _response_len: usize,
    ) {
    }

    pub fn set_composer_text_utf8(&mut self, ptr: *const u8, len: usize) {
        Self::set_utf8_string(&mut self.composer_text, ptr, len);
    }

    pub fn set_utf8_string(target: &mut String, ptr: *const u8, len: usize) {
        *target = Self::read_utf8_string(ptr, len);
    }

    fn active_input_target(&self) -> InputTarget {
        self.active_input_target
    }

    fn set_active_input_target(&mut self, target: InputTarget) {
        self.active_input_target = target;
    }

    pub fn set_mission_mutations_enabled(&mut self, enabled: bool) {
        self.mission_mutations_enabled = enabled;
    }

    pub fn set_mission_retention_profile(&mut self, profile: u8) {
        self.mission_retention_profile = profile;
    }

    pub fn set_mission_watchlist_only(&mut self, enabled: bool) {
        self.mission_watchlist_only = enabled;
    }

    pub fn set_mission_order_newest_first(&mut self, enabled: bool) {
        self.mission_order_newest_first = enabled;
    }

    pub fn set_mission_alert_rules(
        &mut self,
        errors_enabled: bool,
        stuck_turns_enabled: bool,
        reconnect_storms_enabled: bool,
    ) {
        self.mission_alert_errors_enabled = errors_enabled;
        self.mission_alert_stuck_turns_enabled = stuck_turns_enabled;
        self.mission_alert_reconnect_storms_enabled = reconnect_storms_enabled;
    }

    pub fn set_mission_filter(&mut self, filter: u8) {
        self.mission_filter = filter;
    }

    pub fn mission_filter_u8(&self) -> u8 {
        self.mission_filter
    }

    pub fn set_mission_pin_critical(&mut self, enabled: bool) {
        self.mission_pin_critical = enabled;
    }

    pub fn mission_pin_critical_enabled(&self) -> bool {
        self.mission_pin_critical
    }

    pub fn composer_focused(&self) -> bool {
        self.active_input_target == InputTarget::Composer
    }

    pub fn set_composer_focused(&mut self, focused: bool) {
        self.active_input_target = if focused {
            InputTarget::Composer
        } else {
            InputTarget::None
        };
    }

    pub fn consume_send_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.send_requested)
    }

    pub fn consume_new_thread_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.new_thread_requested)
    }

    pub fn consume_interrupt_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.interrupt_requested)
    }

    pub fn consume_model_cycle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.model_cycle_requested)
    }

    pub fn consume_reasoning_cycle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.reasoning_cycle_requested)
    }

    pub fn consume_send_code_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.send_code_requested)
    }

    pub fn consume_verify_code_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.verify_code_requested)
    }

    pub fn consume_sign_out_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.sign_out_requested)
    }

    pub fn consume_refresh_workers_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.refresh_workers_requested)
    }

    pub fn consume_connect_stream_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.connect_stream_requested)
    }

    pub fn consume_disconnect_stream_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.disconnect_stream_requested)
    }

    pub fn consume_send_handshake_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.send_handshake_requested)
    }

    pub fn consume_thread_read_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.thread_read_requested)
    }

    pub fn consume_stop_worker_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.stop_worker_requested)
    }

    pub fn consume_refresh_snapshot_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.refresh_snapshot_requested)
    }

    pub fn consume_mission_retention_cycle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_retention_cycle_requested)
    }

    pub fn consume_mission_watch_active_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_watch_active_requested)
    }

    pub fn consume_mission_watchlist_only_toggle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_watchlist_only_toggle_requested)
    }

    pub fn consume_mission_order_toggle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_order_toggle_requested)
    }

    pub fn consume_mission_alert_errors_toggle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_alert_errors_toggle_requested)
    }

    pub fn consume_mission_alert_stuck_turns_toggle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_alert_stuck_turns_toggle_requested)
    }

    pub fn consume_mission_alert_reconnect_storms_toggle_requested(&mut self) -> bool {
        Self::consume_flag(&mut self.mission_alert_reconnect_storms_toggle_requested)
    }

    fn consume_flag(flag: &mut bool) -> bool {
        let value = *flag;
        *flag = false;
        value
    }

    fn read_utf8_string(ptr: *const u8, len: usize) -> String {
        if ptr.is_null() || len == 0 {
            return String::new();
        }

        // SAFETY: Caller provides pointer+length pair from FFI boundary.
        // Null/empty is checked above; invalid UTF-8 is handled lossily.
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(bytes).into_owned()
    }
}

unsafe fn ios_state_mut_unchecked<'a>(
    state: *mut IosBackgroundState,
) -> &'a mut IosBackgroundState {
    unsafe { &mut *state }
}

unsafe fn ios_state_ref_unchecked<'a>(state: *const IosBackgroundState) -> &'a IosBackgroundState {
    unsafe { &*state }
}

unsafe fn free_ios_state_unchecked(state: *mut IosBackgroundState) -> Box<IosBackgroundState> {
    unsafe { Box::from_raw(state) }
}

macro_rules! ios_state_mut {
    ($state:expr) => {{
        // SAFETY: all callers perform null checks before invoking this helper
        // and request unique mutable access through a single FFI entrypoint.
        unsafe { ios_state_mut_unchecked($state) }
    }};
}

macro_rules! ios_state_ref {
    ($state:expr) => {{
        // SAFETY: all callers perform null checks before invoking this helper
        // and only request shared access for read-only operations.
        unsafe { ios_state_ref_unchecked($state) }
    }};
}

macro_rules! ios_state_free {
    ($state:expr) => {{
        // SAFETY: all callers pass pointers created by `Box::into_raw`
        // from this module and free each pointer at most once.
        unsafe { free_ios_state_unchecked($state) }
    }};
}

/// C FFI for Swift: create renderer from CAMetalLayer pointer.
/// `width`/`height` are logical points.
/// Returns opaque pointer to IosBackgroundState, or null on error.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_create(
    layer_ptr: *mut c_void,
    width: u32,
    height: u32,
    scale: f32,
) -> *mut IosBackgroundState {
    log::debug!(
        "[WGPUI Rust] wgpui_ios_background_create called width={} height={} scale={}",
        width,
        height,
        scale
    );
    if layer_ptr.is_null() {
        log::error!("[WGPUI Rust] create: layer_ptr is null");
        return std::ptr::null_mut();
    }
    // SAFETY: `layer_ptr` is null-checked above and comes from host-provided
    // CAMetalLayer ownership for the lifetime of the renderer state.
    match unsafe { IosBackgroundState::new(layer_ptr, width, height, scale) } {
        Ok(state) => {
            log::debug!("[WGPUI Rust] create: OK");
            Box::into_raw(state)
        }
        Err(e) => {
            log::error!("[WGPUI Rust] create FAILED: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// C FFI: render one frame. Returns 1 on success, 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_render(state: *mut IosBackgroundState) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    match state.render() {
        Ok(()) => 1,
        Err(e) => {
            log::error!("[WGPUI Rust] render FAILED: {}", e);
            0
        }
    }
}

/// C FFI: resize the surface.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_resize(
    state: *mut IosBackgroundState,
    width: u32,
    height: u32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.resize(width, height);
}

/// C FFI: destroy state and free memory.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_destroy(state: *mut IosBackgroundState) {
    if state.is_null() {
        return;
    }
    let _ = ios_state_free!(state);
}

/// C FFI: handle tap at logical point coordinates (origin top-left). Call from Swift on tap.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_handle_tap(state: *mut IosBackgroundState, x: f32, y: f32) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.handle_tap(x, y);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_clear_codex_messages(state: *mut IosBackgroundState) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.clear_codex_messages();
}

/// role: user=0 assistant=1 reasoning=2 tool=3 system=4 error=5
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_codex_message(
    state: *mut IosBackgroundState,
    role: u8,
    text_ptr: *const c_char,
    text_len: usize,
    streaming: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    let ptr_u8 = if text_ptr.is_null() {
        std::ptr::null()
    } else {
        text_ptr as *const u8
    };
    state.push_codex_message(role, ptr_u8, text_len, streaming != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_codex_context(
    state: *mut IosBackgroundState,
    thread_ptr: *const c_char,
    thread_len: usize,
    turn_ptr: *const c_char,
    turn_len: usize,
    model_ptr: *const c_char,
    model_len: usize,
    reasoning_ptr: *const c_char,
    reasoning_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_codex_context_utf8(
        if thread_ptr.is_null() {
            std::ptr::null()
        } else {
            thread_ptr as *const u8
        },
        thread_len,
        if turn_ptr.is_null() {
            std::ptr::null()
        } else {
            turn_ptr as *const u8
        },
        turn_len,
        if model_ptr.is_null() {
            std::ptr::null()
        } else {
            model_ptr as *const u8
        },
        model_len,
        if reasoning_ptr.is_null() {
            std::ptr::null()
        } else {
            reasoning_ptr as *const u8
        },
        reasoning_len,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_empty_state(
    state: *mut IosBackgroundState,
    title_ptr: *const c_char,
    title_len: usize,
    detail_ptr: *const c_char,
    detail_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_empty_state_utf8(
        if title_ptr.is_null() {
            std::ptr::null()
        } else {
            title_ptr as *const u8
        },
        title_len,
        if detail_ptr.is_null() {
            std::ptr::null()
        } else {
            detail_ptr as *const u8
        },
        detail_len,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_auth_fields(
    state: *mut IosBackgroundState,
    email_ptr: *const c_char,
    email_len: usize,
    code_ptr: *const c_char,
    code_len: usize,
    auth_status_ptr: *const c_char,
    auth_status_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_auth_fields_utf8(
        if email_ptr.is_null() {
            std::ptr::null()
        } else {
            email_ptr as *const u8
        },
        email_len,
        if code_ptr.is_null() {
            std::ptr::null()
        } else {
            code_ptr as *const u8
        },
        code_len,
        if auth_status_ptr.is_null() {
            std::ptr::null()
        } else {
            auth_status_ptr as *const u8
        },
        auth_status_len,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_operator_status(
    state: *mut IosBackgroundState,
    worker_status_ptr: *const c_char,
    worker_status_len: usize,
    stream_status_ptr: *const c_char,
    stream_status_len: usize,
    handshake_status_ptr: *const c_char,
    handshake_status_len: usize,
    device_status_ptr: *const c_char,
    device_status_len: usize,
    telemetry_ptr: *const c_char,
    telemetry_len: usize,
    events_ptr: *const c_char,
    events_len: usize,
    control_ptr: *const c_char,
    control_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_operator_status_utf8(
        if worker_status_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_status_ptr as *const u8
        },
        worker_status_len,
        if stream_status_ptr.is_null() {
            std::ptr::null()
        } else {
            stream_status_ptr as *const u8
        },
        stream_status_len,
        if handshake_status_ptr.is_null() {
            std::ptr::null()
        } else {
            handshake_status_ptr as *const u8
        },
        handshake_status_len,
        if device_status_ptr.is_null() {
            std::ptr::null()
        } else {
            device_status_ptr as *const u8
        },
        device_status_len,
        if telemetry_ptr.is_null() {
            std::ptr::null()
        } else {
            telemetry_ptr as *const u8
        },
        telemetry_len,
        if events_ptr.is_null() {
            std::ptr::null()
        } else {
            events_ptr as *const u8
        },
        events_len,
        if control_ptr.is_null() {
            std::ptr::null()
        } else {
            control_ptr as *const u8
        },
        control_len,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_clear_mission_data(state: *mut IosBackgroundState) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.clear_mission_data();
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_mission_worker(
    state: *mut IosBackgroundState,
    worker_id_ptr: *const c_char,
    worker_id_len: usize,
    status_ptr: *const c_char,
    status_len: usize,
    heartbeat_state_ptr: *const c_char,
    heartbeat_state_len: usize,
    latest_seq: i64,
    lag_events: i64,
    reconnect_state_ptr: *const c_char,
    reconnect_state_len: usize,
    last_event_at_ptr: *const c_char,
    last_event_at_len: usize,
    running_turns: u64,
    queued_requests: u64,
    failed_requests: u64,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.push_mission_worker(
        if worker_id_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_id_ptr as *const u8
        },
        worker_id_len,
        if status_ptr.is_null() {
            std::ptr::null()
        } else {
            status_ptr as *const u8
        },
        status_len,
        if heartbeat_state_ptr.is_null() {
            std::ptr::null()
        } else {
            heartbeat_state_ptr as *const u8
        },
        heartbeat_state_len,
        latest_seq,
        lag_events,
        if reconnect_state_ptr.is_null() {
            std::ptr::null()
        } else {
            reconnect_state_ptr as *const u8
        },
        reconnect_state_len,
        if last_event_at_ptr.is_null() {
            std::ptr::null()
        } else {
            last_event_at_ptr as *const u8
        },
        last_event_at_len,
        running_turns,
        queued_requests,
        failed_requests,
    );
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_mission_thread(
    state: *mut IosBackgroundState,
    worker_id_ptr: *const c_char,
    worker_id_len: usize,
    thread_id_ptr: *const c_char,
    thread_id_len: usize,
    active_turn_id_ptr: *const c_char,
    active_turn_id_len: usize,
    last_summary_ptr: *const c_char,
    last_summary_len: usize,
    last_event_at_ptr: *const c_char,
    last_event_at_len: usize,
    freshness_seq: i64,
    unread_count: u64,
    muted: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.push_mission_thread(
        if worker_id_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_id_ptr as *const u8
        },
        worker_id_len,
        if thread_id_ptr.is_null() {
            std::ptr::null()
        } else {
            thread_id_ptr as *const u8
        },
        thread_id_len,
        if active_turn_id_ptr.is_null() {
            std::ptr::null()
        } else {
            active_turn_id_ptr as *const u8
        },
        active_turn_id_len,
        if last_summary_ptr.is_null() {
            std::ptr::null()
        } else {
            last_summary_ptr as *const u8
        },
        last_summary_len,
        if last_event_at_ptr.is_null() {
            std::ptr::null()
        } else {
            last_event_at_ptr as *const u8
        },
        last_event_at_len,
        freshness_seq,
        unread_count,
        muted != 0,
    );
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_mission_timeline_entry(
    state: *mut IosBackgroundState,
    worker_id_ptr: *const c_char,
    worker_id_len: usize,
    thread_id_ptr: *const c_char,
    thread_id_len: usize,
    role_ptr: *const c_char,
    role_len: usize,
    text_ptr: *const c_char,
    text_len: usize,
    is_streaming: i32,
    turn_id_ptr: *const c_char,
    turn_id_len: usize,
    item_id_ptr: *const c_char,
    item_id_len: usize,
    occurred_at_ptr: *const c_char,
    occurred_at_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.push_mission_timeline_entry(
        if worker_id_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_id_ptr as *const u8
        },
        worker_id_len,
        if thread_id_ptr.is_null() {
            std::ptr::null()
        } else {
            thread_id_ptr as *const u8
        },
        thread_id_len,
        if role_ptr.is_null() {
            std::ptr::null()
        } else {
            role_ptr as *const u8
        },
        role_len,
        if text_ptr.is_null() {
            std::ptr::null()
        } else {
            text_ptr as *const u8
        },
        text_len,
        is_streaming != 0,
        if turn_id_ptr.is_null() {
            std::ptr::null()
        } else {
            turn_id_ptr as *const u8
        },
        turn_id_len,
        if item_id_ptr.is_null() {
            std::ptr::null()
        } else {
            item_id_ptr as *const u8
        },
        item_id_len,
        if occurred_at_ptr.is_null() {
            std::ptr::null()
        } else {
            occurred_at_ptr as *const u8
        },
        occurred_at_len,
    );
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_mission_event(
    state: *mut IosBackgroundState,
    id: u64,
    topic_ptr: *const c_char,
    topic_len: usize,
    seq: i64,
    worker_id_ptr: *const c_char,
    worker_id_len: usize,
    thread_id_ptr: *const c_char,
    thread_id_len: usize,
    turn_id_ptr: *const c_char,
    turn_id_len: usize,
    request_id_ptr: *const c_char,
    request_id_len: usize,
    event_type_ptr: *const c_char,
    event_type_len: usize,
    method_ptr: *const c_char,
    method_len: usize,
    summary_ptr: *const c_char,
    summary_len: usize,
    severity: u8,
    occurred_at_ptr: *const c_char,
    occurred_at_len: usize,
    payload_ptr: *const c_char,
    payload_len: usize,
    resync_marker: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.push_mission_event(
        id,
        if topic_ptr.is_null() {
            std::ptr::null()
        } else {
            topic_ptr as *const u8
        },
        topic_len,
        seq,
        if worker_id_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_id_ptr as *const u8
        },
        worker_id_len,
        if thread_id_ptr.is_null() {
            std::ptr::null()
        } else {
            thread_id_ptr as *const u8
        },
        thread_id_len,
        if turn_id_ptr.is_null() {
            std::ptr::null()
        } else {
            turn_id_ptr as *const u8
        },
        turn_id_len,
        if request_id_ptr.is_null() {
            std::ptr::null()
        } else {
            request_id_ptr as *const u8
        },
        request_id_len,
        if event_type_ptr.is_null() {
            std::ptr::null()
        } else {
            event_type_ptr as *const u8
        },
        event_type_len,
        if method_ptr.is_null() {
            std::ptr::null()
        } else {
            method_ptr as *const u8
        },
        method_len,
        if summary_ptr.is_null() {
            std::ptr::null()
        } else {
            summary_ptr as *const u8
        },
        summary_len,
        severity,
        if occurred_at_ptr.is_null() {
            std::ptr::null()
        } else {
            occurred_at_ptr as *const u8
        },
        occurred_at_len,
        if payload_ptr.is_null() {
            std::ptr::null()
        } else {
            payload_ptr as *const u8
        },
        payload_len,
        resync_marker != 0,
    );
}

#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_push_mission_request(
    state: *mut IosBackgroundState,
    request_id_ptr: *const c_char,
    request_id_len: usize,
    worker_id_ptr: *const c_char,
    worker_id_len: usize,
    thread_id_ptr: *const c_char,
    thread_id_len: usize,
    method_ptr: *const c_char,
    method_len: usize,
    state_ptr: *const c_char,
    state_len: usize,
    occurred_at_ptr: *const c_char,
    occurred_at_len: usize,
    error_code_ptr: *const c_char,
    error_code_len: usize,
    error_message_ptr: *const c_char,
    error_message_len: usize,
    retryable: i32,
    response_ptr: *const c_char,
    response_len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.push_mission_request(
        if request_id_ptr.is_null() {
            std::ptr::null()
        } else {
            request_id_ptr as *const u8
        },
        request_id_len,
        if worker_id_ptr.is_null() {
            std::ptr::null()
        } else {
            worker_id_ptr as *const u8
        },
        worker_id_len,
        if thread_id_ptr.is_null() {
            std::ptr::null()
        } else {
            thread_id_ptr as *const u8
        },
        thread_id_len,
        if method_ptr.is_null() {
            std::ptr::null()
        } else {
            method_ptr as *const u8
        },
        method_len,
        if state_ptr.is_null() {
            std::ptr::null()
        } else {
            state_ptr as *const u8
        },
        state_len,
        if occurred_at_ptr.is_null() {
            std::ptr::null()
        } else {
            occurred_at_ptr as *const u8
        },
        occurred_at_len,
        if error_code_ptr.is_null() {
            std::ptr::null()
        } else {
            error_code_ptr as *const u8
        },
        error_code_len,
        if error_message_ptr.is_null() {
            std::ptr::null()
        } else {
            error_message_ptr as *const u8
        },
        error_message_len,
        retryable != 0,
        if response_ptr.is_null() {
            std::ptr::null()
        } else {
            response_ptr as *const u8
        },
        response_len,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_composer_text(
    state: *mut IosBackgroundState,
    ptr: *const c_char,
    len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    let ptr_u8 = if ptr.is_null() {
        std::ptr::null()
    } else {
        ptr as *const u8
    };
    state.set_composer_text_utf8(ptr_u8, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_auth_email(
    state: *mut IosBackgroundState,
    ptr: *const c_char,
    len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    let ptr_u8 = if ptr.is_null() {
        std::ptr::null()
    } else {
        ptr as *const u8
    };
    IosBackgroundState::set_utf8_string(&mut state.auth_email, ptr_u8, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_auth_code(
    state: *mut IosBackgroundState,
    ptr: *const c_char,
    len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    let ptr_u8 = if ptr.is_null() {
        std::ptr::null()
    } else {
        ptr as *const u8
    };
    IosBackgroundState::set_utf8_string(&mut state.auth_code, ptr_u8, len);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_active_input_target(state: *mut IosBackgroundState) -> u8 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_ref!(state);
    state.active_input_target().as_u8()
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_active_input_target(
    state: *mut IosBackgroundState,
    target: u8,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_active_input_target(InputTarget::from_u8(target));
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_mutations_enabled(
    state: *mut IosBackgroundState,
    enabled: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_mutations_enabled(enabled != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_retention_profile(
    state: *mut IosBackgroundState,
    profile: u8,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_retention_profile(profile);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_watchlist_only(
    state: *mut IosBackgroundState,
    enabled: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_watchlist_only(enabled != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_order_newest_first(
    state: *mut IosBackgroundState,
    enabled: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_order_newest_first(enabled != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_alert_rules(
    state: *mut IosBackgroundState,
    errors_enabled: i32,
    stuck_turns_enabled: i32,
    reconnect_storms_enabled: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_alert_rules(
        errors_enabled != 0,
        stuck_turns_enabled != 0,
        reconnect_storms_enabled != 0,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_filter(
    state: *mut IosBackgroundState,
    filter: u8,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_filter(filter);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_mission_filter(state: *mut IosBackgroundState) -> u8 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_ref!(state);
    state.mission_filter_u8()
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_mission_pin_critical(
    state: *mut IosBackgroundState,
    enabled: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_mission_pin_critical(enabled != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_mission_pin_critical(state: *mut IosBackgroundState) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_ref!(state);
    if state.mission_pin_critical_enabled() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_composer_focused(state: *mut IosBackgroundState) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_ref!(state);
    if state.composer_focused() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_composer_focused(
    state: *mut IosBackgroundState,
    focused: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_composer_focused(focused != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_send_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_send_requested() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_new_thread_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_new_thread_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_interrupt_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_interrupt_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_model_cycle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_model_cycle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_reasoning_cycle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_reasoning_cycle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_send_code_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_send_code_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_verify_code_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_verify_code_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_sign_out_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_sign_out_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_refresh_workers_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_refresh_workers_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_connect_stream_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_connect_stream_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_disconnect_stream_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_disconnect_stream_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_send_handshake_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_send_handshake_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_thread_read_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_thread_read_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_stop_worker_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_stop_worker_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_refresh_snapshot_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_refresh_snapshot_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_retention_cycle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_retention_cycle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_watch_active_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_watch_active_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_watchlist_only_toggle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_watchlist_only_toggle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_order_toggle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_order_toggle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_alert_errors_toggle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_alert_errors_toggle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_alert_stuck_turns_toggle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_alert_stuck_turns_toggle_requested() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_mission_alert_reconnect_storms_toggle_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_mission_alert_reconnect_storms_toggle_requested() {
        1
    } else {
        0
    }
}

/// Backward-compatible alias for older iOS bridge code.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_login_submit_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.send_code_requested { 1 } else { 0 }
}

/// Backward-compatible alias for older iOS bridge code.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_consume_submit_requested(
    state: *mut IosBackgroundState,
) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_mut!(state);
    if state.consume_send_code_requested() {
        1
    } else {
        0
    }
}

/// Backward-compatible alias for older iOS bridge code.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_email_focused(state: *mut IosBackgroundState) -> i32 {
    if state.is_null() {
        return 0;
    }
    let state = ios_state_ref!(state);
    if state.active_input_target() == InputTarget::AuthEmail {
        1
    } else {
        0
    }
}

/// Backward-compatible alias for older iOS bridge code.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_email_focused(
    state: *mut IosBackgroundState,
    focused: i32,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    state.set_active_input_target(if focused != 0 {
        InputTarget::AuthEmail
    } else {
        InputTarget::None
    });
}

/// Backward-compatible alias for older iOS bridge code.
#[unsafe(no_mangle)]
pub extern "C" fn wgpui_ios_background_set_login_email(
    state: *mut IosBackgroundState,
    ptr: *const c_char,
    len: usize,
) {
    if state.is_null() {
        return;
    }
    let state = ios_state_mut!(state);
    if ptr.is_null() || len == 0 {
        IosBackgroundState::set_utf8_string(&mut state.auth_email, std::ptr::null(), 0);
        return;
    }
    let ptr_u8 = ptr as *const u8;
    IosBackgroundState::set_utf8_string(&mut state.auth_email, ptr_u8, len);
}
