//! A separate, main-thread native render handle. Chat synchronization never
//! shares this handle or its executor. Destroy it before releasing the layer.
use crate::ffi::{CoderMobileBuffer, buffer};
use crate::verse_app::{Config, Request, Scene};
use std::cell::RefCell;
use std::ffi::c_void;
thread_local! { static CREATE_ERROR: RefCell<Option<String>> = const { RefCell::new(None) }; }
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

pub struct VerseHandle {
    scene: Scene,
    renderer: verse::render::Renderer,
}

fn failure() -> CoderMobileBuffer {
    buffer(br#"{"schema":"coder.verse.v1","status":"Verse unavailable","error":"Native Verse request failed","frames_presented":0,"position":[0,0,0],"computer":{"near":false,"visible":false,"screen_x":0.5,"screen_y":0.5,"distance":5.0},"computer_open":false,"view":null}"#.to_vec())
}

/// Returns the initial Rust-owned surface projection. Release the result with
/// `coder_mobile_buffer_free`. Creating a projection does not start a renderer.
#[unsafe(no_mangle)]
pub extern "C" fn coder_verse_blueprint() -> CoderMobileBuffer {
    let mut packet = crate::verse_app::blueprint();
    packet.error = CREATE_ERROR.with(|error| error.borrow().clone());
    match serde_json::to_vec(&packet) {
        Ok(bytes) => buffer(bytes),
        Err(_) => failure(),
    }
}

/// # Safety
/// `layer` must be a live CAMetalLayer owned by the calling main thread.
/// `bytes` must contain `len` readable bytes. Serialize all operations on the
/// main thread, and destroy the returned handle before releasing the layer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_verse_create(
    layer: *mut c_void,
    bytes: *const u8,
    len: usize,
) -> *mut VerseHandle {
    if layer.is_null() || bytes.is_null() || len == 0 || len > 16 * 1024 {
        return ptr::null_mut();
    }
    CREATE_ERROR.with(|error| *error.borrow_mut() = None);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let config: Config =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(bytes, len) })
                .map_err(|_| "Invalid native Verse configuration".to_owned())?;
        let scene = Scene::new(config)?;
        create_renderer(layer, scene)
    }));
    match result {
        Ok(Ok(handle)) => Box::into_raw(Box::new(handle)),
        error => {
            let message = match error {
                Ok(Err(message)) => message,
                _ => "The native renderer could not initialize".into(),
            };
            CREATE_ERROR.with(|error| *error.borrow_mut() = Some(message));
            ptr::null_mut()
        }
    }
}

#[cfg(target_os = "ios")]
fn create_renderer(layer: *mut c_void, scene: Scene) -> Result<VerseHandle, String> {
    let viewport = scene.lifecycle.viewport();
    let atlas = verse::ui::Atlas::new(18.0);
    // The FFI contract keeps the native layer alive for this renderer's mount.
    let renderer = unsafe {
        verse::render::Renderer::from_metal_layer(
            layer,
            viewport.width().max(1),
            viewport.height().max(1),
            &scene.world.world.mesh,
            &atlas,
            verse::render::RenderOptions {
                sample_count: 1,
                max_extent: 4096,
            },
        )
    }?;
    Ok(VerseHandle { scene, renderer })
}

#[cfg(not(target_os = "ios"))]
fn create_renderer(_layer: *mut c_void, _scene: Scene) -> Result<VerseHandle, String> {
    Err("Metal native surfaces require an iOS host".into())
}

/// # Safety
/// Pass a live, exclusively borrowed Verse handle and `len` readable bytes.
/// Calls must run on the main thread. Free each result exactly once with
/// `coder_mobile_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_verse_call(
    handle: *mut VerseHandle,
    bytes: *const u8,
    len: usize,
) -> CoderMobileBuffer {
    if handle.is_null() || bytes.is_null() || len == 0 || len > 4096 {
        return failure();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let request: Request =
            match serde_json::from_slice(unsafe { std::slice::from_raw_parts(bytes, len) }) {
                Ok(request) => request,
                Err(_) => return failure(),
            };
        let handle = unsafe { &mut *handle };
        let clear_error = !matches!(&request, Request::Frame { .. } | Request::Snapshot);
        match handle.call(request) {
            Err(error) => handle.scene.error = Some(error),
            Ok(()) if clear_error => handle.scene.error = None,
            Ok(()) => {}
        }
        match serde_json::to_vec(&handle.scene.packet()) {
            Ok(bytes) if bytes.len() <= 64 * 1024 => buffer(bytes),
            _ => failure(),
        }
    }))
    .unwrap_or_else(|_| failure())
}

impl VerseHandle {
    fn call(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Frame { timestamp } => {
                if let Some(dt) = self.scene.update(timestamp)? {
                    let mut mesh = self.scene.world.dynamic_mesh();
                    if let Some(session) = &mut self.scene.session {
                        mesh.extend(&session.crowd.mesh(std::time::Instant::now(), dt));
                    }
                    let view = self.scene.world.view(self.renderer.aspect());
                    match self
                        .renderer
                        .draw(view, &mesh, &verse::ui::UiBatch::default())
                    {
                        verse::render::DrawStatus::Presented => {
                            self.scene.frames = self.scene.frames.saturating_add(1);
                        }
                        verse::render::DrawStatus::Skipped(_) => {}
                        verse::render::DrawStatus::Error(error) => return Err(error),
                    }
                }
                Ok(())
            }
            Request::Resize {
                width,
                height,
                scale,
            } => {
                let viewport = rust_native::surface::Viewport::new(width, height, scale)
                    .map_err(|e| e.to_string())?;
                self.renderer.resize(width, height)?;
                self.scene
                    .lifecycle
                    .resize(viewport)
                    .map_err(|e| e.to_string())
            }
            request => self.scene.action(request),
        }
    }
}

/// # Safety
/// The handle must be live with no pending native callbacks, and used on its
/// creating main thread. The native layer must remain alive until this returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn coder_verse_destroy(handle: *mut VerseHandle) {
    if !handle.is_null() {
        let mut handle = unsafe { Box::from_raw(handle) };
        let _ = handle.scene.activate(false);
        handle.scene.lifecycle.destroy();
        drop(handle);
    }
}
