//! The Grid's surface on the engine renderer: the pinned pack admitted once,
//! a presenter on the host's surface, and one `draw` per frame that hands
//! the assembled [`crate::grid_frame`] to [`crate::imported::Renderer`].
//! The desktop attaches a window; a phone passes its Metal layer or Android
//! window; a browser passes a surface made from its canvas, on a GPU it
//! awaited. A host that composites the world under its own views, as the
//! OpenAgents desktop app does, shares its device and has each frame copied
//! into a texture it owns ([`GridEngine::on_device`], [`GridEngine::encode`]).

#[cfg(feature = "imported-desktop")]
use std::sync::Arc;

use verse_engine::presentation::Instance;
#[cfg(feature = "imported-desktop")]
use winit::window::Window;

use crate::grid_frame;
use crate::grid_pack;
use crate::imported::lighting::Lighting;
use crate::imported::{Gpu, Renderer, TexturePresenter, WindowPresenter};
use crate::render::View;
use crate::ui::{Atlas, UiBatch};

pub struct GridEngine {
    renderer: Box<Renderer>,
    presenter: Presenter,
    size: [u32; 2],
    reattach: Reattach,
    atlas: Atlas,
    content: Kind,
}

/// Which pack the engine holds: the pinned Grid, or a chamber's runtime
/// pack the host admitted the player to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Grid,
    Chamber,
}

/// A pack admitted for the engine, with the static instances drawn under
/// every frame.
pub struct Content {
    pub prepared: verse_engine::loading::Prepared,
    pub statics: Vec<Instance>,
    pub kind: Kind,
}

impl Content {
    /// The built-in Grid pack.
    ///
    /// # Errors
    /// The embedded pack cannot be admitted.
    pub fn grid() -> Result<Self, String> {
        let prepared = grid_pack::prepare_embedded()?;
        let statics = grid_frame::statics(prepared.pack());
        Ok(Self {
            prepared,
            statics,
            kind: Kind::Grid,
        })
    }

    /// The built-in Grid pack without the Gym's hall and boards
    /// ([`crate::runtime::WorldRuntime::remove_gym`]).
    ///
    /// # Errors
    /// The embedded pack cannot be admitted.
    pub fn grid_without_gym() -> Result<Self, String> {
        let mut content = Self::grid()?;
        content
            .statics
            .retain(|i| ![grid_pack::GYM, grid_pack::BOARDS].contains(&i.model.as_str()));
        Ok(content)
    }

    /// A chamber's pack read from `dir`, with its scene's static placements
    /// around `origin`.
    ///
    /// # Errors
    /// The pack's assets cannot be read or admitted.
    #[cfg(feature = "remote-chamber")]
    pub fn chamber(
        pack: verse_engine::assets::Pack,
        dir: &std::path::Path,
        origin: glam::Vec3,
    ) -> Result<Self, String> {
        let statics = crate::imported::chamber::static_instances(&pack, origin);
        let prepared = verse_engine::loading::Prepared::load(pack, dir, Default::default())?;
        Ok(Self {
            prepared,
            statics,
            kind: Kind::Chamber,
        })
    }
}

/// Where the engine's frames go: a surface it presents, or a texture the
/// host owns.
enum Presenter {
    Surface(WindowPresenter),
    Texture(TexturePresenter),
}

/// How the engine gets a presenter back after the GPU device is lost.
enum Reattach {
    /// The desktop window can be attached again here.
    #[cfg(feature = "imported-desktop")]
    Window(Arc<Window>),
    /// The host owns the surface, or the device and the texture; it
    /// detaches and attaches again.
    Host,
}

impl GridEngine {
    /// Admits the pinned Grid pack and attaches the engine renderer to the
    /// window at its current size.
    #[cfg(feature = "imported-desktop")]
    pub fn new(
        window: Arc<Window>,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let pack = grid_pack::load_pinned()?;
        let statics = grid_frame::statics(&pack);
        let size = [width.max(1), height.max(1)];
        let renderer = Box::new(Renderer::new(
            pack,
            &grid_pack::pinned_dir(),
            size[0],
            size[1],
            atlas,
            &statics,
        )?);
        let presenter = renderer.attach_window(window.clone())?;
        Ok(Self {
            renderer,
            presenter: Presenter::Surface(presenter),
            size,
            reattach: Reattach::Window(window),
            atlas: atlas.clone(),
            content: Kind::Grid,
        })
    }

    /// Admits the built-in Grid pack onto `gpu`, opened against `surface`,
    /// and presents on that surface. The host made both: see [`Gpu::open`]
    /// and [`Gpu::open_async`].
    pub fn on_surface(
        gpu: Gpu,
        surface: wgpu::Surface<'static>,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        Self::on_surface_with(gpu, surface, Content::grid()?, atlas, width, height)
    }

    /// Admits `content` onto `gpu`, opened against `surface`, and presents
    /// on that surface.
    pub fn on_surface_with(
        gpu: Gpu,
        surface: wgpu::Surface<'static>,
        content: Content,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let size = [width.max(1), height.max(1)];
        let renderer = Renderer::from_prepared_on(
            gpu,
            content.prepared,
            size[0],
            size[1],
            atlas,
            &content.statics,
        )?;
        let presenter = renderer.attach_surface(surface, size)?;
        Ok(Self {
            renderer,
            presenter: Presenter::Surface(presenter),
            size,
            reattach: Reattach::Host,
            atlas: atlas.clone(),
            content: content.kind,
        })
    }

    /// Admits `content` onto the host's own device and copies each frame
    /// into a texture of `format` the host owns ([`Self::encode`]). The
    /// host keeps its device: when that device is lost, it drops this
    /// engine and opens another on the new one.
    #[allow(clippy::too_many_arguments)]
    pub fn on_device(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        content: Content,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let size = [width.max(1), height.max(1)];
        // The engine presents on no surface of its own, so the instance it
        // keeps for one is never used to make one.
        let gpu = Gpu {
            instance: wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            ),
            adapter: adapter.clone(),
            device: device.clone(),
            queue: queue.clone(),
        };
        let renderer = Renderer::from_prepared_on(
            gpu,
            content.prepared,
            size[0],
            size[1],
            atlas,
            &content.statics,
        )?;
        let presenter = renderer.attach_texture(format);
        Ok(Self {
            renderer,
            presenter: Presenter::Texture(presenter),
            size,
            reattach: Reattach::Host,
            atlas: atlas.clone(),
            content: content.kind,
        })
    }

    /// Which pack this engine holds.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.content
    }

    /// Opens the engine renderer on an Apple host's `CAMetalLayer`.
    ///
    /// # Safety
    /// `layer` must point to a valid CAMetalLayer on its owning UI thread,
    /// kept alive and attached until this engine is dropped.
    #[cfg(target_vendor = "apple")]
    pub unsafe fn from_metal_layer(
        layer: *mut core::ffi::c_void,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Box<Self>, String> {
        // SAFETY: the caller's contract is this function's.
        unsafe { Self::from_metal_layer_with(layer, Content::grid()?, atlas, width, height) }
    }

    /// Opens the engine renderer with `content` on an Apple host's
    /// `CAMetalLayer`.
    ///
    /// # Safety
    /// As [`Self::from_metal_layer`].
    #[cfg(target_vendor = "apple")]
    pub unsafe fn from_metal_layer_with(
        layer: *mut core::ffi::c_void,
        content: Content,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Box<Self>, String> {
        if layer.is_null() {
            return Err("native Metal layer is null".into());
        }
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        // SAFETY: the caller owns the layer lifetime and UI-thread confinement.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer))
        }
        .map_err(|e| format!("cannot create a Metal surface: {e}"))?;
        let gpu = Gpu::open(instance, &surface)?;
        Self::on_surface_with(gpu, surface, content, atlas, width, height).map(Box::new)
    }

    /// Opens the engine renderer on an acquired Android `ANativeWindow`,
    /// trying the graphics APIs the legacy renderer tries, in its order.
    ///
    /// # Safety
    /// `window` must point to a valid ANativeWindow on its owning UI thread,
    /// retained until after this engine is dropped.
    #[cfg(target_os = "android")]
    pub unsafe fn from_android_window(
        window: *mut core::ffi::c_void,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Box<Self>, String> {
        // SAFETY: the caller's contract is this function's.
        unsafe { Self::from_android_window_with(window, Content::grid()?, atlas, width, height) }
    }

    /// Opens the engine renderer with `content` on an acquired Android
    /// `ANativeWindow`.
    ///
    /// # Safety
    /// As [`Self::from_android_window`].
    #[cfg(target_os = "android")]
    pub unsafe fn from_android_window_with(
        window: *mut core::ffi::c_void,
        content: Content,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Box<Self>, String> {
        let window = std::ptr::NonNull::new(window).ok_or("native Android window is null")?;
        let mut failures = Vec::new();
        let mut content = Some(content);
        for backends in crate::render::android::backends() {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            descriptor.backends = backends;
            let instance = wgpu::Instance::new(descriptor);
            let handle = wgpu::rwh::AndroidNdkWindowHandle::new(window);
            // SAFETY: the caller retains the window through this engine's
            // drop. A failed attempt drops its surface before the next one.
            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(wgpu::rwh::AndroidDisplayHandle::new().into()),
                    raw_window_handle: handle.into(),
                })
            };
            let result = surface
                .map_err(|error| format!("cannot create an Android surface: {error}"))
                .and_then(|surface| Gpu::open(instance, &surface).map(|gpu| (gpu, surface)))
                .and_then(|(gpu, surface)| {
                    let content = content.take().ok_or("content was consumed")?;
                    Self::on_surface_with(gpu, surface, content, atlas, width, height)
                });
            match result {
                Ok(engine) => return Ok(Box::new(engine)),
                Err(error) => {
                    failures.push(format!("{backends:?}: {error}"));
                    if content.is_none() {
                        break;
                    }
                }
            }
        }
        Err(failures.join("; "))
    }

    /// The GPU adapter drawing the Grid.
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.renderer.adapter_name
    }

    /// What the renderer admitted on this device: quality tier, shadow
    /// layout, pose block, and budgets.
    #[must_use]
    pub fn device_profile(&self) -> &serde_json::Value {
        &self.renderer.device_profile
    }

    /// The viewport in pixels.
    #[must_use]
    pub fn size(&self) -> [f32; 2] {
        [self.size[0] as f32, self.size[1] as f32]
    }

    #[must_use]
    pub fn aspect(&self) -> f32 {
        self.size[0] as f32 / self.size[1] as f32
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.recover()?;
        self.renderer.resize(width, height)?;
        self.size = [width, height];
        Ok(())
    }

    /// Uploads `atlas` again after glyphs were added to it. Returns false
    /// when its size changed, which only a new engine can take.
    pub fn update_atlas(&mut self, atlas: &Atlas) -> bool {
        if !self.renderer.update_atlas(atlas) {
            return false;
        }
        self.atlas = atlas.clone();
        true
    }

    /// Draws and presents one frame; returns the renderer's own time for it
    /// in milliseconds.
    pub fn draw(
        &mut self,
        view: View,
        dynamic: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<f64, String> {
        self.recover()?;
        let Presenter::Surface(presenter) = &mut self.presenter else {
            return Err("this engine draws into the host's texture; use encode".into());
        };
        let [width, height] = [self.size[0] as f32, self.size[1] as f32];
        self.renderer.set_overlay_size(width, height);
        self.renderer.draw_live(view, dynamic, ui, lighting)?;
        self.renderer.present_window(presenter, self.size)?;
        Ok(self.renderer.last_timings.total_ms)
    }

    /// Draws one frame and records its copy into `target`, the host's
    /// texture at this engine's size, on `encoder`. The frame's own work is
    /// already submitted when this returns, so the host submits `encoder`
    /// on the shared queue after it. Returns the renderer's own time for
    /// the frame in milliseconds.
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        view: View,
        dynamic: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<f64, String> {
        self.recover()?;
        let Presenter::Texture(presenter) = &mut self.presenter else {
            return Err("this engine presents on its own surface; use draw".into());
        };
        let [width, height] = [self.size[0] as f32, self.size[1] as f32];
        self.renderer.set_overlay_size(width, height);
        self.renderer.draw_live(view, dynamic, ui, lighting)?;
        self.renderer.encode_into(presenter, encoder, target)?;
        Ok(self.renderer.last_timings.total_ms)
    }

    fn recover(&mut self) -> Result<(), String> {
        if matches!(self.reattach, Reattach::Host) {
            // The host's surface or texture belongs to the lost device:
            // opening a private device here would only be dropped.
            return match self.renderer.lost() {
                Some(reason) => Err(format!(
                    "the GPU device was lost ({reason}); detach and attach again"
                )),
                None => Ok(()),
            };
        }
        if self.renderer.recover_if_lost(&self.atlas)? {
            match &self.reattach {
                #[cfg(feature = "imported-desktop")]
                Reattach::Window(window) => {
                    self.presenter =
                        Presenter::Surface(self.renderer.attach_window(window.clone())?);
                }
                Reattach::Host => {}
            }
        }
        Ok(())
    }
}
