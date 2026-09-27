//! A separate, main-thread native render handle. Chat synchronization never
//! shares this handle or its executor. Destroy it before releasing the surface.
use crate::ffi::{CoderMobileBuffer, buffer};
use crate::verse_app::{Config, Request, Scene};
use std::cell::RefCell;
use std::ffi::c_void;
thread_local! { static CREATE_ERROR: RefCell<Option<String>> = const { RefCell::new(None) }; }
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

pub struct VerseHandle {
    pub(crate) scene: Scene,
    pub(crate) renderer: Option<verse::render::Renderer>,
}

fn failure() -> CoderMobileBuffer {
    buffer(br#"{"schema":"coder.verse.v1","status":"Verse unavailable","error":"Native Verse request failed","frames_presented":0,"position":[0,0,0],"camera_mode":"touch","camera_yaw":0.0,"camera_pitch":0.28,"camera_distance":6.0,"motion_needed":false,"connection":{"state":"offline","label":"Offline","relay":null,"error":null},"map":{"visible":false,"expanded":false,"state":"","destination":null,"captured_pointers":[],"frame":[0,0,0,0],"plot":[0,0,0,0],"center":[0,0],"half_extent":264,"landmarks":[]},"computer":{"near":false,"visible":false,"screen_x":0.5,"screen_y":0.5,"distance":5.0},"computer_open":false,"gym":{"inside":false,"near":false,"visible":false,"screen_x":0.5,"screen_y":0.5,"distance":60.0},"gym_open":false,"gym_revision":0,"gym_active":false,"view":null}"#.to_vec())
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
    if layer.is_null() || bytes.is_null() || len == 0 || len > 96 * 1024 {
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
    // The FFI contract keeps the native layer alive for this renderer's mount.
    let renderer = unsafe {
        verse::render::Renderer::from_metal_layer(
            layer,
            viewport.width().max(1),
            viewport.height().max(1),
            &scene.world.world.mesh,
            &scene.atlas,
            verse::render::RenderOptions {
                sample_count: 1,
                max_extent: 4096,
            },
        )
    }?;
    Ok(VerseHandle {
        scene,
        renderer: Some(renderer),
    })
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
    if handle.is_null() || bytes.is_null() || len == 0 || len > 96 * 1024 {
        return failure();
    }
    catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
        match unsafe { &mut *handle }.call_bytes(bytes) {
            Ok(bytes) => buffer(bytes),
            Err(_) => failure(),
        }
    }))
    .unwrap_or_else(|_| failure())
}

impl VerseHandle {
    /// Android can lose its native window while retaining the application scene.
    /// Suspend effects and release the renderer before its window is released.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn detach_renderer(&mut self) -> Result<(), String> {
        let result = self.scene.activate(false);
        self.renderer = None;
        result
    }

    pub(crate) fn call_bytes(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.is_empty() || bytes.len() > 96 * 1024 {
            return Err("Native Verse request exceeds its size limit".into());
        }
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "Invalid native Verse request".to_owned())?;
        if bytes.len() > 4096 && !matches!(&request, Request::GymConfigure { .. }) {
            return Err("Native Verse request exceeds its size limit".into());
        }
        let include_gym = matches!(
            &request,
            Request::GymView
                | Request::InteractGym
                | Request::GymConfigure { .. }
                | Request::GymSelectRun { .. }
                | Request::GymSelectRecipe { .. }
                | Request::GymLaunch
                | Request::GymRetry
                | Request::GymCloseDetail
        );
        let clear_error = !matches!(
            &request,
            Request::Frame { .. }
                | Request::DeviceMotion { .. }
                | Request::Snapshot
                | Request::GymView
        );
        match self.call(request) {
            Err(error) => self.scene.error = Some(error),
            Ok(()) if clear_error => self.scene.error = None,
            Ok(()) => {}
        }
        let mut packet = self.scene.packet();
        if include_gym {
            packet.gym_board = self.scene.gym_view();
        }
        let bytes = serde_json::to_vec(&packet)
            .map_err(|_| "Cannot encode native Verse state".to_owned())?;
        if bytes.len() > if include_gym { 1024 * 1024 } else { 64 * 1024 } {
            return Err("Native Verse state exceeds its size limit".into());
        }
        Ok(bytes)
    }

    fn call(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Frame { timestamp } => {
                let Some(renderer) = &mut self.renderer else {
                    return Ok(());
                };
                if let Some(dt) = self.scene.update(timestamp)? {
                    let mut mesh = self.scene.world.dynamic_mesh_with_computer_interaction();
                    let entities = self
                        .scene
                        .session
                        .as_mut()
                        .map_or_else(verse::mesh::Mesh::default, |session| {
                            session.crowd.mesh(std::time::Instant::now(), dt)
                        });
                    mesh.extend(&entities);
                    let view = self.scene.world.view(renderer.aspect());
                    match renderer.draw(view, &mesh, &self.scene.map_ui()) {
                        verse::render::DrawStatus::Presented => {
                            self.scene.presented_entities = entities;
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
                self.renderer
                    .as_mut()
                    .ok_or("The native Verse surface is detached")?
                    .resize(width, height)?;
                self.scene.resize(viewport)?;
                Ok(())
            }
            Request::Active { active: true } if self.renderer.is_none() => {
                Err("The native Verse surface is detached".into())
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
        unsafe { drop(Box::from_raw(handle)) };
    }
}

impl Drop for VerseHandle {
    fn drop(&mut self) {
        let _ = self.scene.activate(false);
        self.scene.lifecycle.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verse_app::CameraMode;

    #[test]
    fn detached_surface_preserves_scene_without_advancing_or_resuming() {
        let scene = Scene::new(Config {
            secret_hex: "01".repeat(32),
            width: 640,
            height: 960,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: false,
            world_relay: None,
        })
        .unwrap();
        let mut handle = VerseHandle {
            scene,
            renderer: None,
        };
        handle.scene.activate(true).unwrap();
        handle
            .scene
            .world
            .set_spawn([3.0, 0.0, -4.0].into(), 0.8)
            .unwrap();
        handle
            .scene
            .action(Request::CameraMode {
                mode: CameraMode::Motion,
            })
            .unwrap();
        let before: serde_json::Value =
            serde_json::from_slice(&handle.call_bytes(br#"{"action":"snapshot"}"#).unwrap())
                .unwrap();

        handle.detach_renderer().unwrap();
        assert!(!handle.scene.lifecycle.active());
        assert!(handle.call(Request::Active { active: true }).is_err());
        handle.call(Request::Frame { timestamp: 5000.0 }).unwrap();
        handle.detach_renderer().unwrap();
        let after: serde_json::Value =
            serde_json::from_slice(&handle.call_bytes(br#"{"action":"snapshot"}"#).unwrap())
                .unwrap();
        assert_eq!(after["position"], before["position"]);
        assert_eq!(after["camera_mode"], "motion");
        assert_eq!(after["camera_yaw"], before["camera_yaw"]);
        assert_eq!(after["frames_presented"], before["frames_presented"]);
        assert_eq!(after["motion_needed"], false);
        assert_eq!(after["gym_active"], false);
    }
}
