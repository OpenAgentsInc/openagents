//! A separate, main-thread native render handle. Chat synchronization never
//! shares this handle or its executor. Destroy it before releasing the surface.
use crate::ffi::{CoderMobileBuffer, buffer};
use crate::verse_app::{Config, Request, Scene};
use std::cell::RefCell;
use std::ffi::c_void;
thread_local! { static CREATE_ERROR: RefCell<Option<String>> = const { RefCell::new(None) }; }
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

/// The largest native request: a forwarded Computers view (Rust Native's
/// 512 KiB view bound) with its input request and QR code.
const MAX_REQUEST_BYTES: usize = 640 * 1024;

/// The stack the scene's creation runs on. `Scene::new` builds the world by
/// value through frames that total near a megabyte, which overflowed a
/// phone's main-thread stack when the Verse tab mounted (#10928). The
/// renderer still opens on the calling main thread, which owns the layer.
const CREATE_STACK_BYTES: usize = 64 << 20;

/// Runs `build` on a dedicated `verse-create` thread with a
/// [`CREATE_STACK_BYTES`] stack and hands its boxed result back to the
/// calling thread. A panic in `build` resumes here, so the FFI boundary's
/// `catch_unwind` still reports it.
pub(crate) fn create_scene(
    build: impl FnOnce() -> Result<Box<Scene>, String> + Send,
) -> Result<Box<Scene>, String> {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .name("verse-create".into())
            .stack_size(CREATE_STACK_BYTES)
            .spawn_scoped(scope, build)
        {
            Ok(joined) => joined
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload)),
            Err(error) => Err(format!("Cannot start the world build thread: {error}")),
        }
    })
}

/// Avatar presence for the bare world: the protected world identity and,
/// optionally, a relay other than the public plaza relay.
pub struct BarePresence {
    /// The world identity's secp256k1 secret, as 64 hexadecimal characters.
    /// Keep it separate from any pairing or host-grant key.
    pub secret_hex: String,
    /// A `wss://` relay; the public relay when absent.
    pub relay: Option<String>,
    /// The name shown over this player's head; the short key when absent.
    pub name: Option<String>,
}

/// The bare world's Gym connection: the host's `gym-connect:` code, as in
/// Coder, or the labeled synthetic preview for simulator checks. The default
/// has neither, and its board asks for a connection.
#[derive(Default)]
pub struct BareGym {
    /// A `gym-connect:` code the host saved for this world identity.
    pub code: Option<String>,
    /// Show the labeled synthetic board and start outside the Gym's doorway.
    /// The world then stays offline, as Coder's preview does.
    pub preview: bool,
    /// The host shows the native Gym panel when the board opens. Without it
    /// the Gym stands in the world, but its board shows no tap cue and never
    /// opens.
    pub panel: bool,
    /// The host shows the native results panel for the Gym's RESULTS
    /// board. Without it the board stands with its lettering but no tap
    /// cue, never opens, and loads nothing.
    pub results_panel: bool,
    /// Where the RESULTS board reads the published results; the public
    /// repository's publication when `None`.
    pub results_base: Option<String>,
    /// The app's cache directory for verified results.
    pub results_cache_directory: Option<String>,
    /// Show levels from the labeled tutorial fixture instead of reading the
    /// relay: six throwaway reproductions credited to this player. For
    /// simulator checks only; the world stays offline.
    pub xp_preview: bool,
    /// The host shows the native EVALS panel for the Gym's EVALS board:
    /// published eval results and the agents' notes. Without it the board
    /// stands with its lettering but no tap cue, never opens, and reads
    /// nothing.
    pub evals_panel: bool,
    /// **Compare notes** is on: the player's agent may trade notes about
    /// published results with other trainers' agents in the Gym.
    pub notes: bool,
    /// A `ws://` relay on this machine for the world, for simulator checks
    /// against local fixtures. Honored only in debug builds, and only with
    /// a world identity.
    pub check_relay: Option<String>,
    /// The app's absolute cache directory for zone packs. With it the Grid
    /// shows its walk-in portal to Everglade, whose pinned pack loads into
    /// this directory; without it the Grid has no such portal.
    pub zone_cache_directory: Option<String>,
    /// Where the player's block and mute lists live
    /// ([`verse::blocklist`]); the zone cache directory when absent, and
    /// this mount only when both are.
    pub blocklist_directory: Option<String>,
    /// The plain Grid: no Gym hall, boards, or walls stand on it, so no
    /// board opens, loads, or connects, whatever the fields above say
    /// ([`verse::runtime::WorldRuntime::remove_gym`]).
    pub without_gym: bool,
}

#[cfg(test)]
pub(crate) fn bare_config(
    width: u32,
    height: u32,
    scale: f32,
    hdr: bool,
    presence: Option<BarePresence>,
) -> Config {
    bare_config_with_gym(width, height, scale, hdr, presence, BareGym::default())
}

pub(crate) fn bare_config_with_gym(
    width: u32,
    height: u32,
    scale: f32,
    hdr: bool,
    presence: Option<BarePresence>,
    gym: BareGym,
) -> Config {
    let (secret_hex, world_relay, display_name, world_offline) = match presence {
        Some(presence) => (presence.secret_hex, presence.relay, presence.name, false),
        None => {
            let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
            let hex = secret
                .secret_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            (hex, None, None, true)
        }
    };
    // A pinned chamber the app keeps beside its zone packs opens the Grid's
    // RITUAL arch, as `~/.verse/ritual.json` does on the desktop.
    let ritual = gym
        .zone_cache_directory
        .as_deref()
        .map(|dir| std::path::Path::new(dir).join(verse::ritual::FILE))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned());
    Config {
        secret_hex,
        width,
        height,
        scale,
        synthetic: gym.preview,
        gym_code: gym.code.filter(|_| !gym.without_gym),
        synthetic_gym: gym.preview,
        world_relay,
        display_name,
        world_offline: world_offline || gym.preview || gym.xp_preview,
        door_preferences: None,
        zone_cache_directory: gym.zone_cache_directory,
        results_base: gym.results_base,
        results_cache_directory: gym.results_cache_directory,
        computer_hud: false,
        hdr,
        bare: true,
        xp_preview: gym.xp_preview,
        gym_notes: gym.notes,
        ritual,
    }
}

/// A mounted Verse world and its renderer.
pub struct VerseHandle {
    /// Boxed so the handle and every frame that carries it stay small: the
    /// scene is tens of kilobytes, and moving it by value through the
    /// creation path overflowed a phone's main-thread stack (#10928).
    pub(crate) scene: Box<Scene>,
    pub(crate) renderer: Option<Surface>,
    pub(crate) rendered_zone_revision: u64,
    /// The chamber content the engine holds; 0 while it holds the Grid.
    pub(crate) rendered_chamber_revision: u64,
    /// The native layer or window the renderer draws on, kept by the host
    /// for as long as this handle is attached; a zone change reopens it.
    pub(crate) layer: *mut c_void,
}

/// The renderer on the native surface: the Grid draws through the engine,
/// every other zone through the legacy renderer until its own migration.
pub(crate) enum Surface {
    Legacy(Box<verse::render::Renderer>),
    Grid(Box<verse::grid_engine::GridEngine>),
}

impl Surface {
    fn is_grid(&self) -> bool {
        matches!(self, Self::Grid(_))
    }

    fn hdr(&self) -> bool {
        match self {
            Self::Legacy(renderer) => renderer.hdr(),
            Self::Grid(_) => false,
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        match self {
            Self::Legacy(renderer) => renderer.resize(width, height),
            Self::Grid(engine) => engine.resize(width, height),
        }
    }
}

/// Whether the scene's zone is the Grid, which draws through the engine.
fn on_grid(scene: &Scene) -> bool {
    scene.world.is_bare() && scene.world.is_plaza()
}

/// The engine content the scene draws: the chamber's pack while the
/// player is joined to one, the Grid otherwise.
fn engine_content(scene: &Scene) -> Result<verse::grid_engine::Content, String> {
    if let Some(content) = scene.chamber.as_ref().and_then(|c| c.content.as_ref()) {
        return verse::grid_engine::Content::chamber(
            content.pack.clone(),
            &content.dir,
            verse_engine::source_position(content.scene.origin),
        );
    }
    if scene.world.has_gym() {
        verse::grid_engine::Content::grid()
    } else {
        verse::grid_engine::Content::grid_without_gym()
    }
}

/// The chamber content revision the engine was opened with; 0 is the Grid.
fn chamber_revision(scene: &Scene) -> u64 {
    scene
        .chamber
        .as_ref()
        .filter(|c| c.content.is_some())
        .map_or(0, |c| c.content_revision)
}

/// The initial surface projection, before a world is mounted.
#[must_use]
pub fn blueprint_bytes() -> Vec<u8> {
    serde_json::to_vec(&crate::verse_app::blueprint()).unwrap_or_default()
}

fn failure() -> CoderMobileBuffer {
    let mut packet = crate::verse_app::blueprint();
    packet.error = Some("Native Verse request failed".into());
    buffer(serde_json::to_vec(&packet).unwrap_or_default())
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
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        coder_verse_create_inner(layer, bytes, len)
    }));
    match result {
        Ok(Ok(handle)) => Box::into_raw(handle),
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

/// `coder_verse_create`'s body, `#[inline(never)]` so the world build's
/// frames never join the FFI frame (#10928).
#[inline(never)]
unsafe fn coder_verse_create_inner(
    layer: *mut c_void,
    bytes: *const u8,
    len: usize,
) -> Result<Box<VerseHandle>, String> {
    let config: Config = serde_json::from_slice(unsafe { std::slice::from_raw_parts(bytes, len) })
        .map_err(|_| "Invalid native Verse configuration".to_owned())?;
    let scene = create_scene(move || Scene::new(config))?;
    create_renderer(layer, scene)
}

#[cfg(target_os = "ios")]
#[inline(never)]
fn create_renderer(layer: *mut c_void, scene: Box<Scene>) -> Result<Box<VerseHandle>, String> {
    let mut handle = Box::new(VerseHandle {
        scene,
        renderer: None,
        rendered_zone_revision: u64::MAX,
        rendered_chamber_revision: 0,
        layer,
    });
    handle.open_renderer()?;
    Ok(handle)
}

#[cfg(target_os = "ios")]
impl VerseHandle {
    /// Opens the renderer the scene's zone needs on the retained layer.
    #[inline(never)]
    fn open_renderer(&mut self) -> Result<(), String> {
        let viewport = self.scene.lifecycle.viewport();
        let width = viewport.width().max(1);
        let height = viewport.height().max(1);
        self.renderer = None;
        // The FFI contract keeps the native layer alive for this renderer's mount.
        let renderer = if on_grid(&self.scene) {
            let content = engine_content(&self.scene)?;
            self.rendered_chamber_revision = chamber_revision(&self.scene);
            Surface::Grid(unsafe {
                verse::grid_engine::GridEngine::from_metal_layer_with(
                    self.layer,
                    content,
                    &self.scene.atlas,
                    width,
                    height,
                )
            }?)
        } else {
            let mut renderer = unsafe {
                verse::render::Renderer::from_metal_layer(
                    self.layer,
                    width,
                    height,
                    &self.scene.world.world.mesh,
                    &self.scene.atlas,
                    verse::render::RenderOptions {
                        sample_count: 1,
                        max_extent: 4096,
                        // The native host sets an extended linear sRGB color space
                        // on the layer when the screen offers EDR headroom.
                        hdr: self.scene.hdr_requested,
                    },
                )
            }?;
            renderer.set_atmosphere(self.scene.world.atmosphere())?;
            Surface::Legacy(renderer)
        };
        self.renderer = Some(renderer);
        self.rendered_zone_revision = self.scene.world.zone_revision;
        Ok(())
    }
}

/// On Android, `layer` is an acquired `ANativeWindow` that the caller keeps
/// until the handle is dropped or detached.
#[cfg(target_os = "android")]
#[inline(never)]
fn create_renderer(layer: *mut c_void, scene: Box<Scene>) -> Result<Box<VerseHandle>, String> {
    let viewport = scene.lifecycle.viewport();
    let mut handle = Box::new(VerseHandle {
        scene,
        renderer: None,
        rendered_zone_revision: u64::MAX,
        rendered_chamber_revision: 0,
        layer: ptr::null_mut(),
    });
    // SAFETY: the caller's contract is the same as `attach_android`'s.
    unsafe { handle.attach_android(layer, viewport.width(), viewport.height(), viewport.scale()) }?;
    Ok(handle)
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
#[inline(never)]
fn create_renderer(_layer: *mut c_void, _scene: Box<Scene>) -> Result<Box<VerseHandle>, String> {
    Err("Metal native surfaces require an iOS host".into())
}

#[cfg(target_os = "android")]
impl VerseHandle {
    /// Draws the retained scene in a new Android window, after
    /// `detach_android` or at creation. The window is Android's
    /// `ANativeWindow`.
    ///
    /// # Safety
    /// `window` must be an acquired native window on the calling main thread,
    /// kept until `detach_android` returns or this handle is dropped.
    pub unsafe fn attach_android(
        &mut self,
        window: *mut c_void,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<(), String> {
        if self.renderer.is_some() {
            return Err("The native Verse surface is already attached".into());
        }
        let viewport = rust_native::surface::Viewport::new(width, height, scale)
            .map_err(|error| error.to_string())?;
        if !viewport.drawable() || width > 4096 || height > 4096 {
            return Err("Native Verse surface dimensions exceed their bounds".into());
        }
        self.scene.activate(false)?;
        self.layer = window;
        self.open_renderer(width, height)?;
        self.scene
            .lifecycle
            .resize(viewport)
            .map_err(|error| error.to_string())?;
        self.scene.action(Request::ResetMotion)?;
        Ok(())
    }

    /// Opens the renderer the scene's zone needs on the retained window.
    #[inline(never)]
    fn open_renderer(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.renderer = None;
        // SAFETY: the caller keeps the window alive past the renderer.
        let renderer = if on_grid(&self.scene) {
            let content = engine_content(&self.scene)?;
            self.rendered_chamber_revision = chamber_revision(&self.scene);
            Surface::Grid(unsafe {
                verse::grid_engine::GridEngine::from_android_window_with(
                    self.layer,
                    content,
                    &self.scene.atlas,
                    width,
                    height,
                )
            }?)
        } else {
            Surface::Legacy(unsafe {
                verse::render::Renderer::from_android_window(
                    self.layer,
                    width,
                    height,
                    &self.scene.world.world.mesh,
                    &self.scene.atlas,
                    verse::render::RenderOptions {
                        sample_count: 1,
                        max_extent: 4096,
                        hdr: false,
                    },
                )
            }?)
        };
        self.renderer = Some(renderer);
        // The next frame applies the world's mesh and atmosphere.
        self.rendered_zone_revision = u64::MAX;
        Ok(())
    }

    /// Suspends the scene and drops the renderer, so the caller can release
    /// the window Android is taking away. The scene and its pose remain.
    pub fn detach_android(&mut self) -> Result<(), String> {
        self.detach_renderer()
    }
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
    if handle.is_null() || bytes.is_null() || len == 0 || len > MAX_REQUEST_BYTES {
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
    /// Mounts Verse's bare world on a Metal layer: the plaza's ground grid in
    /// the neutral palette, with Coder's player, touch, and motion controls,
    /// the shared ball and blocks, the Gym, and, with a zone cache, the walk-in
    /// portal to Everglade (its portal to Lagrange 1 is hidden for now; see
    /// `verse::zones::gate::GRID_PORTAL_OPEN`). With `presence`, it joins the bare world's own NIP-MV
    /// world for avatar presence alone while active; without it, it joins no
    /// relay and uses a throwaway identity.
    ///
    /// # Safety
    /// As `coder_verse_create`: `layer` must be a live CAMetalLayer owned by
    /// the calling main thread, and the handle must be dropped before it.
    pub unsafe fn create_bare(
        layer: *mut c_void,
        width: u32,
        height: u32,
        scale: f32,
        hdr: bool,
        presence: Option<BarePresence>,
    ) -> Result<Box<Self>, String> {
        unsafe {
            Self::create_bare_with_gym(
                layer,
                width,
                height,
                scale,
                hdr,
                presence,
                BareGym::default(),
            )
        }
    }

    /// As [`Self::create_bare`], with the bare world's Gym connection.
    ///
    /// # Safety
    /// As `coder_verse_create`.
    pub unsafe fn create_bare_with_gym(
        layer: *mut c_void,
        width: u32,
        height: u32,
        scale: f32,
        hdr: bool,
        presence: Option<BarePresence>,
        gym: BareGym,
    ) -> Result<Box<Self>, String> {
        if layer.is_null() {
            return Err("No native layer to draw in".into());
        }
        let (panel, results_panel, evals_panel) = (gym.panel, gym.results_panel, gym.evals_panel);
        let without_gym = gym.without_gym;
        let check_relay = match gym.check_relay.clone().filter(|_| cfg!(debug_assertions)) {
            Some(relay) if presence.is_some() => {
                coder_connect::RelayPolicy::LoopbackTest
                    .validate(&relay)
                    .map_err(|_| "Use a ws:// relay on this machine for checks".to_owned())?;
                Some(relay)
            }
            _ => None,
        };
        let blocklist = gym
            .blocklist_directory
            .clone()
            .or_else(|| gym.zone_cache_directory.clone())
            .filter(|directory| std::path::Path::new(directory).is_absolute())
            .map(std::path::PathBuf::from);
        let scene = create_scene(move || {
            let mut scene = Scene::new(bare_config_with_gym(
                width, height, scale, hdr, presence, gym,
            ))?;
            scene.set_blocklist_directory(blocklist);
            scene.gym_panel = panel;
            scene.results_panel = results_panel;
            scene.evals_panel = evals_panel;
            if without_gym {
                scene.remove_gym();
            }
            if let Some(relay) = check_relay {
                scene.relay = Some(relay);
            }
            Ok(scene)
        })?;
        create_renderer(layer, scene)
    }

    /// Lets the world's Everglade draw the owner's private placements kept
    /// in `directory`, an absolute directory in the app's sandbox that the
    /// app keeps in step with the owner's computer, signing grant requests
    /// with this mount's world key (`docs/verse/private-assets.md`). Call
    /// it only for a mount with the player's world identity: the broker
    /// refuses a throwaway key, so nothing would draw.
    ///
    /// # Errors
    ///
    /// Returns a message when `directory` is not an absolute path.
    pub fn configure_private_assets(&mut self, directory: &str) -> Result<(), String> {
        self.scene.configure_private_assets(directory)
    }

    /// Android can lose its native window while retaining the application scene.
    /// Suspend effects and release the renderer before its window is released.
    #[cfg(any(target_os = "android", test))]
    pub(crate) fn detach_renderer(&mut self) -> Result<(), String> {
        let result = self.scene.activate(false);
        self.renderer = None;
        self.layer = ptr::null_mut();
        result
    }

    /// Reopens the renderer when a zone change moves the scene between the
    /// engine's Grid and the legacy zones; `Ok(false)` when the zone stayed
    /// on the renderer it has.
    fn reopen_for_zone(&mut self) -> Result<bool, String> {
        let Some(renderer) = &self.renderer else {
            return Ok(false);
        };
        if self.layer.is_null()
            || (renderer.is_grid() == on_grid(&self.scene)
                && self.rendered_chamber_revision == chamber_revision(&self.scene))
        {
            return Ok(false);
        }
        #[cfg(target_os = "ios")]
        self.open_renderer()?;
        #[cfg(target_os = "android")]
        {
            let viewport = self.scene.lifecycle.viewport();
            self.open_renderer(viewport.width().max(1), viewport.height().max(1))?;
        }
        Ok(true)
    }

    /// Applies one JSON request and returns the JSON packet that follows it,
    /// as `coder_verse_call` does. Call on the creating main thread.
    pub fn call_bytes(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err("Native Verse request exceeds its size limit".into());
        }
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "Invalid native Verse request".to_owned())?;
        let limit = match &request {
            // A forwarded Computers view is bounded by Rust Native's view
            // limit; the HUD validates it again.
            Request::ComputerFeed { .. } => MAX_REQUEST_BYTES,
            Request::GymConfigure { .. } => 96 * 1024,
            // A plan typed at the podium answers a goal's plan decision.
            Request::StudioText { .. } => 64 * 1024,
            _ => 4096,
        };
        if bytes.len() > limit {
            return Err("Native Verse request exceeds its size limit".into());
        }
        let include_credits = matches!(&request, Request::ZoneCredits);
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
        let include_results = matches!(
            &request,
            Request::ResultsView | Request::InteractResults | Request::Results { .. }
        );
        let include_evals = matches!(
            &request,
            Request::EvalsView | Request::InteractEvals | Request::Evals { .. } | Request::GoEvals
        );
        let include_studio = matches!(
            &request,
            Request::StudioView
                | Request::StudioActivate { .. }
                | Request::StudioText { .. }
                | Request::Zone {
                    intent: verse::zones::Intent::Interact
                }
        );
        let clear_error = !matches!(
            &request,
            Request::Frame { .. }
                | Request::DeviceMotion { .. }
                | Request::Snapshot
                | Request::GymView
                | Request::ResultsView
                | Request::EvalsView
                | Request::StudioView
        );
        match self.call(request) {
            Err(error) => self.scene.error = Some(error),
            Ok(()) if clear_error => self.scene.error = None,
            Ok(()) => {}
        }
        let mut packet = self.scene.packet();
        packet.hdr_output = self.renderer.as_ref().is_some_and(|r| r.hdr());
        packet.computer_commands = self.scene.take_computer_commands();
        if include_credits {
            packet.credits = Some(verse::zones::CREDITS);
        }
        if include_gym {
            packet.gym_board = self.scene.gym_view();
        }
        if include_results {
            packet.results_view = self.scene.results_view();
        }
        if include_evals {
            packet.evals_view = self.scene.evals_view();
        }
        if include_studio {
            packet.studio_view = self.scene.studio_view();
        }
        let bytes = serde_json::to_vec(&packet)
            .map_err(|_| "Cannot encode native Verse state".to_owned())?;
        if bytes.len()
            > if include_gym || include_results || include_evals || include_studio {
                1024 * 1024
            } else {
                64 * 1024
            }
        {
            return Err("Native Verse state exceeds its size limit".into());
        }
        Ok(bytes)
    }

    fn call(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Frame {
                timestamp,
                headroom,
            } => {
                if self.renderer.is_none() {
                    return Ok(());
                }
                let Some(dt) = self.scene.update(timestamp)? else {
                    return Ok(());
                };
                if self.rendered_zone_revision != self.scene.world.zone_revision
                    || self.rendered_chamber_revision != chamber_revision(&self.scene)
                {
                    self.reopen_for_zone()?;
                }
                self.scene.prepare_terminal_glyphs();
                match self.renderer.as_mut() {
                    Some(Surface::Grid(engine)) => {
                        engine.update_atlas(&self.scene.atlas);
                        self.rendered_zone_revision = self.scene.world.zone_revision;
                        if self.scene.in_chamber() {
                            let [w, h] = engine.size();
                            if let Some(frame) = self.scene.chamber_frame([w as u32, h as u32])? {
                                let mut ui = frame.ui;
                                ui.vertices.extend(self.scene.map_ui().vertices);
                                engine.draw(frame.view, &frame.instances, &ui, &frame.lighting)?;
                            } else {
                                // Connecting: the Grid stays up behind the notice.
                                let dynamic =
                                    verse::grid_frame::dynamic(&self.scene.world, &[], &[]);
                                let lighting =
                                    verse::grid_frame::lighting(&self.scene.world.atmosphere());
                                let view = self.scene.world.view(engine.aspect());
                                engine.draw(view, &dynamic, &self.scene.map_ui(), &lighting)?;
                            }
                            self.scene.frames = self.scene.frames.saturating_add(1);
                            return Ok(());
                        }
                        let now = std::time::Instant::now();
                        let peers = self
                            .scene
                            .session
                            .as_mut()
                            .map_or_else(Vec::new, |session| session.crowd.figures(now, dt));
                        let dynamic = verse::grid_frame::dynamic(&self.scene.world, &peers, &[]);
                        let lighting = verse::grid_frame::lighting(&self.scene.world.atmosphere());
                        let view = self.scene.world.view(engine.aspect());
                        engine.draw(view, &dynamic, &self.scene.map_ui(), &lighting)?;
                        self.scene.presented_entities = Box::new(verse::mesh::Mesh::default());
                        self.scene.frames = self.scene.frames.saturating_add(1);
                    }
                    Some(Surface::Legacy(renderer)) => {
                        renderer.update_atlas(&self.scene.atlas);
                        renderer.set_headroom(headroom.unwrap_or(1.0) as f32);
                        if self.rendered_zone_revision != self.scene.world.zone_revision {
                            renderer.replace_world(&self.scene.world.world.mesh)?;
                            renderer.set_atmosphere(self.scene.world.atmosphere())?;
                            self.rendered_zone_revision = self.scene.world.zone_revision;
                        }
                        let mut mesh = self.scene.world.dynamic_mesh_with_boards(
                            true,
                            self.scene.gym_panel,
                            self.scene.results_panel,
                            self.scene.evals_panel,
                        );
                        let entities = self
                            .scene
                            .session
                            .as_mut()
                            .map_or_else(verse::mesh::Mesh::default, |session| {
                                session.crowd.mesh(std::time::Instant::now(), dt)
                            });
                        let entities = if self.scene.world.is_bare() {
                            bare_entities(entities)
                        } else {
                            entities
                        };
                        mesh.extend(&entities);
                        let view = self.scene.world.view(renderer.aspect());
                        match renderer.draw(view, &mesh, &self.scene.map_ui()) {
                            verse::render::DrawStatus::Presented => {
                                self.scene.presented_entities = Box::new(entities);
                                self.scene.frames = self.scene.frames.saturating_add(1);
                            }
                            verse::render::DrawStatus::Skipped(_) => {}
                            verse::render::DrawStatus::Error(error) => return Err(error),
                        }
                    }
                    None => {}
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

/// Other players in the bare world, drawn in its neutral palette: gray and
/// white geometry with no colored light or glow of their own.
pub(crate) fn bare_entities(mut entities: verse::mesh::Mesh) -> verse::mesh::Mesh {
    entities.neutralize();
    entities.lit.clear();
    entities.glow.clear();
    entities.neon = None;
    entities
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
    fn a_ritual_file_beside_the_zone_packs_opens_the_arch() {
        let dir = tempfile::tempdir().unwrap();
        let gym = || BareGym {
            zone_cache_directory: Some(dir.path().to_string_lossy().into_owned()),
            ..BareGym::default()
        };
        assert_eq!(
            bare_config_with_gym(4, 4, 1.0, false, None, gym()).ritual,
            None
        );
        std::fs::write(dir.path().join(verse::ritual::FILE), "{}").unwrap();
        let ritual = bare_config_with_gym(4, 4, 1.0, false, None, gym()).ritual;
        assert_eq!(
            ritual.map(std::path::PathBuf::from),
            Some(dir.path().join(verse::ritual::FILE))
        );
    }

    #[test]
    fn private_assets_need_an_absolute_sandbox_directory() {
        let presence = BarePresence {
            secret_hex: "02".repeat(32),
            relay: None,
            name: None,
        };
        let scene = Scene::new(crate::verse_ffi::bare_config_with_gym(
            4,
            4,
            1.0,
            false,
            Some(presence),
            BareGym::default(),
        ))
        .unwrap();
        let mut handle = VerseHandle {
            scene,
            renderer: None,
            rendered_zone_revision: 0,
            rendered_chamber_revision: 0,
            layer: ptr::null_mut(),
        };
        for bad in ["", "relative/dir"] {
            assert!(handle.configure_private_assets(bad).is_err(), "{bad:?}");
        }
        let dir = tempfile::tempdir().unwrap();
        handle
            .configure_private_assets(&dir.path().to_string_lossy())
            .unwrap();
        // Nothing is read until Everglade is entered.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(handle.scene.world.private_guests(), 0);
    }

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
            display_name: None,
            world_offline: false,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: true,
            hdr: false,
            bare: false,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        })
        .unwrap();
        let mut handle = VerseHandle {
            scene,
            renderer: None,
            rendered_zone_revision: 0,
            rendered_chamber_revision: 0,
            layer: ptr::null_mut(),
        };
        let credits: serde_json::Value =
            serde_json::from_slice(&handle.call_bytes(br#"{"action":"zone_credits"}"#).unwrap())
                .unwrap();
        let text = credits["credits"].as_str().unwrap();
        assert!(text.contains("Wizards of the Coast LLC"));
        assert!(text.contains("Blue Marble") && text.contains("Yale Bright Star"));
        assert!(text.len() <= 32 * 1024);
        let ordinary: serde_json::Value =
            serde_json::from_slice(&handle.call_bytes(br#"{"action":"snapshot"}"#).unwrap())
                .unwrap();
        assert!(ordinary.get("credits").is_none());
        assert_eq!(ordinary["zone"]["id"], "plaza");
        assert_eq!(ordinary["zone"]["progress"], 0.0);
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
        handle
            .call(Request::Frame {
                timestamp: 5000.0,
                headroom: None,
            })
            .unwrap();
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

    /// The world build fits a small stack. Until #10928 it ran on the main
    /// thread — one megabyte on a phone — and overflowed it mounting the
    /// Verse tab. It now runs on the `verse-create` thread
    /// ([`create_scene`]), but its frames must stay well under a megabyte
    /// anyway, which this bounds by building on a small-stack thread. The
    /// bound is on the shipping profile: debug frames run several times
    /// larger, so the debug run keeps a wider margin.
    #[test]
    fn the_world_build_fits_a_small_stack() {
        let stack = if cfg!(debug_assertions) {
            16 << 20
        } else {
            512 << 10
        };
        std::thread::Builder::new()
            .name("verse-create-check".into())
            .stack_size(stack)
            .spawn(|| {
                for bare in [true, false] {
                    let config = Config {
                        bare,
                        synthetic: true,
                        ..bare_config(64, 64, 1.0, false, None)
                    };
                    std::hint::black_box(Scene::new(config)).unwrap();
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
