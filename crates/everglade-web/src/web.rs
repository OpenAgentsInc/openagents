//! The page: canvas, pack download, renderer, events, and the frame loop.
use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Reflect, Uint8Array};
use verse::grid_engine::GridEngine;
use verse::grid_frame;
use verse::imported::Gpu;
use verse::render::{DrawStatus, RenderOptions, Renderer};
use verse::runtime::{Action, WorldRuntime};
use verse::ui::Atlas;
use verse::zones::{self, everglade_pack};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent, RequestCredentials,
    RequestInit, RequestRedirect, Response, WheelEvent, Window,
};

use crate::grid::{self, Presence};
use crate::input::Input;
use crate::load_screen::{self, Load, Stage, Way};
use crate::load_view::LoadView;
use crate::zone_ui::{self, Place, Scheme};

/// The page's canvas.
const CANVAS_ID: &str = "everglade-canvas";
/// The element that shows progress and errors. Created when the page has
/// none.
const STATUS_ID: &str = "everglade-status";
/// The largest drawing-buffer side, in device pixels.
const MAX_SIDE: u32 = 4096;
/// The longest frame step; a hidden tab resumes without a jump.
const MAX_STEP: f32 = 0.1;
/// The most landmarker frames held for the Hand Duel between two drawn
/// frames.
const RAW_LIMIT: usize = 8;

/// Keeps one landmarker frame for the Hand Duel.
fn push_raw(data: &[f32], t: f64) {
    RAW.with(|raw| {
        let mut raw = raw.borrow_mut();
        if raw.len() >= RAW_LIMIT {
            raw.pop_front();
        }
        raw.push_back((data.to_vec(), t));
    });
}

thread_local! {
    /// Opt-in hand input on /verse (#12309).
    static HANDS: RefCell<crate::hand_input::HandInput> = RefCell::default();
    /// The landmarker's frames as they came (packed floats, seconds), for
    /// the Hand Duel, which reads both hands itself. The frame loop drains
    /// it; [`RAW_LIMIT`] bounds it while nothing does.
    static RAW: RefCell<std::collections::VecDeque<(Vec<f32>, f64)>> = RefCell::default();
    /// Whether the page's hand tracking is on, for the Hand Duel's screen.
    static RAW_ON: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// `/lander11`: the Grid behind the site's chat. The camera drifts round
    /// the plaza by itself, the page joins no one, and the keyboard is the
    /// chat's until the page hands it over with [`verse_walk`].
    static AMBIENT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WALK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// /home starts orbiting; its first movement key hands over control.
    static HOME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    /// A portal the page asked to go through ([`verse_portal`]).
    static PORTAL: RefCell<Option<zones::ZoneId>> = const { RefCell::new(None) };
    /// Where the page is, for [`verse_where`]: the zone's label, the
    /// renderer's state, and the last error.
    static WHERE: RefCell<String> = const { RefCell::new(String::new()) };
    /// The Grid's players as this page sees them, for [`verse_players`].
    static PLAYERS: RefCell<String> = const { RefCell::new(String::new()) };
    /// What the build billboard shows, for [`verse_build_board`].
    static BOARD: RefCell<String> = const { RefCell::new(String::new()) };
    /// A ground point the player should turn to face, from [`verse_face`].
    static FACE: RefCell<Option<[f32; 2]>> = const { RefCell::new(None) };
    /// What the Grid homepage's visit records of joining (#12439). Zone
    /// trips reach it through `verse-zone-load`.
    static JOURNEY: RefCell<crate::visit::Journey> = RefCell::default();
}

/// Goes through the Grid's portal to `name` (`lagrange1`, `lagrange2`,
/// `everglade`) on the next frame, as walking into it does, or back to the
/// Grid from a zone (`grid`). `false` for a name with no portal.
#[wasm_bindgen]
pub fn verse_portal(name: &str) -> bool {
    let zone = match name {
        "lagrange1" => zones::ZoneId::Lagrange1,
        "lagrange2" => zones::ZoneId::Lagrange2,
        "everglade" => zones::ZoneId::Everglade,
        "grid" => zones::ZoneId::Plaza,
        _ => return false,
    };
    PORTAL.with(|portal| *portal.borrow_mut() = Some(zone));
    true
}

/// Whether the browser opens `zone` from the Grid: the generated stations
/// do; a zone that needs a pack or a renderer this build doesn't carry is
/// only in the desktop app.
fn web_support(zone: zones::ZoneId) -> crate::transit::Support {
    match zone {
        zones::ZoneId::Lagrange1 | zones::ZoneId::Lagrange2 => crate::transit::Support::Web,
        _ => crate::transit::Support::DesktopOnly,
    }
}

/// The page's name for `zone`: what [`verse_portal`] takes, the
/// `data-verse-zone` attribute, and the zone in `verse-zone` and
/// `verse-zone-load` events.
fn zone_name(zone: zones::ZoneId) -> &'static str {
    match zone {
        zones::ZoneId::Plaza => zone_ui::GRID,
        zones::ZoneId::Lagrange1 => "lagrange1",
        zones::ZoneId::Lagrange2 => "lagrange2",
        zones::ZoneId::Everglade => "everglade",
        other => other.world_id(),
    }
}

/// Fires `name` on the window with `detail`, a JSON object, for the page's
/// own layers and analytics.
fn custom_event(name: &str, detail: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&js_sys::JSON::parse(detail).unwrap_or(JsValue::NULL));
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init) {
        let _ = window.dispatch_event(&event);
    }
}

/// Tells the page where the player is (#12438): `data-verse-zone` and
/// `data-verse-place` on the root element, then a `verse-zone` event whose
/// detail is `{zone, place}`. The Grid's layers (the Pylons) hide away from
/// the plaza.
fn place_event(zone: &str, place: Place) {
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let _ = root.set_attribute("data-verse-zone", zone);
        let _ = root.set_attribute("data-verse-place", place.name());
    }
    custom_event(
        "verse-zone",
        &serde_json::json!({"zone": zone, "place": place.name()}).to_string(),
    );
}

/// Runs `f` on the visit's journey and sends what it returns.
fn journey<E: IntoIterator<Item = crate::visit::Event>>(
    f: impl FnOnce(&mut crate::visit::Journey) -> E,
) {
    let events = JOURNEY.with(|j| f(&mut j.borrow_mut()));
    crate::visit::send(events);
}

/// Tells the page Verse stopped (`verse-stopped` on the window), so it can
/// say so and offer a reload instead of leaving a still canvas.
fn stopped_event() {
    if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::Event::new("verse-stopped")) {
        let _ = window.dispatch_event(&event);
    }
}

/// Where the page is now, as JSON: `{zone, draw, stopped}`.
#[wasm_bindgen]
pub fn verse_where() -> String {
    WHERE.with(|w| w.borrow().clone())
}

/// The Grid's players as this page sees them, as JSON, for checks
/// (#12406): `{joined, me: {pubkey, tag, pos}, others: [{pubkey, tag,
/// pos}]}`, where a tag is `[npub prefix, name or null]` and a position is
/// `[x, y, z]`. Refreshed about four times a second; empty before the
/// Grid's presence opens.
#[wasm_bindgen]
pub fn verse_players() -> String {
    PLAYERS.with(|p| p.borrow().clone())
}

/// What the Grid homepage's build billboard shows, as JSON, for checks
/// (#12442): each lane's status word, commit, strip, last result, history
/// length, and flash, plus the events it refused. Refreshed about four
/// times a second; empty on pages without the board.
#[wasm_bindgen]
pub fn verse_build_board() -> String {
    BOARD.with(|b| b.borrow().clone())
}

/// Turns the player to face the ground point (`x`, `z`) on the next frame,
/// for checks that walk one player at another (#12406).
#[wasm_bindgen]
pub fn verse_face(x: f32, z: f32) {
    if x.is_finite() && z.is_finite() {
        FACE.with(|f| *f.borrow_mut() = Some([x, z]));
    }
}

/// How fast the ambient camera circles the plaza, in orbit-drag CSS pixels
/// a second.
const AMBIENT_ORBIT: f32 = 9.0;

/// Project DOM pylons with the camera used to draw this Grid frame. The
/// separately loaded layer owns fixed world anchors, the list, and cards.
fn pylons_camera(view_proj: glam::Mat4, size: [f32; 2], player: glam::Vec3) {
    let Ok(callback) = Reflect::get(&js_sys::global(), &"pylonsCamera".into()) else {
        return;
    };
    let Some(callback) = callback.dyn_ref::<js_sys::Function>() else {
        return;
    };
    let matrix = js_sys::Float32Array::from(view_proj.to_cols_array().as_slice());
    let player = js_sys::Float32Array::from(player.to_array().as_slice());
    let _ = callback.call4(
        &JsValue::UNDEFINED,
        &matrix,
        &JsValue::from_f64(f64::from(size[0])),
        &JsValue::from_f64(f64::from(size[1])),
        &player,
    );
}

/// Whether the page is `/lander11` (or one of its chats).
fn ambient_path(path: &str) -> bool {
    path == "/lander11" || path.starts_with("/lander11/")
}

/// Whether the page draws the Grid behind its own content: `/lander11`,
/// or any page whose canvas carries `data-ambient` (`/home`).
fn ambient_page(window: &Window, canvas: &HtmlCanvasElement) -> bool {
    canvas.has_attribute("data-ambient")
        || window
            .location()
            .pathname()
            .is_ok_and(|path| ambient_path(path.trim_end_matches('/')))
}

/// On `/lander11`, whether Verse takes the keyboard and mouse now.
fn walking() -> bool {
    !AMBIENT.with(std::cell::Cell::get) || WALK.with(std::cell::Cell::get)
}

/// Browser text controls own their keys, including WASD and space.
fn editing() -> bool {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    document
        .get_element_by_id(CANVAS_ID)
        .is_some_and(|c| c.has_attribute("data-composing"))
        || document.active_element().is_some_and(|e| {
            matches!(e.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || e.dyn_ref::<HtmlElement>()
                    .is_some_and(HtmlElement::is_content_editable)
        })
}

/// `/lander11`'s Walk toggle: `true` hands the keyboard and mouse to Verse,
/// `false` gives them back to the chat and lets the camera drift again.
#[wasm_bindgen]
pub fn verse_walk(on: bool) {
    WALK.with(|walk| walk.set(on));
}

/// The page turned hand input on or off.
#[wasm_bindgen]
pub fn verse_hands_on(on: bool) {
    HANDS.with(|h| h.borrow_mut().set_on(on));
    RAW_ON.with(|r| r.set(Some(on)));
    if !on {
        RAW.with(|raw| raw.borrow_mut().clear());
    }
}

/// One frame of the hand landmarker's points, packed as floats, at `t`
/// milliseconds, on a `width` by `height` CSS-pixel page.
#[wasm_bindgen]
pub fn verse_hand_frame(data: &[f32], t: f64, width: f32, height: f32) {
    HANDS.with(|h| h.borrow_mut().frame(data, t / 1000.0, (width, height)));
    push_raw(data, t / 1000.0);
}

/// Made-up hands, for checking without a camera: `pose` at page fractions
/// (`x`, `y`), and a second when `pose2` isn't empty.
#[wasm_bindgen]
pub fn verse_hand_synth(
    pose: &str,
    x: f32,
    y: f32,
    pose2: &str,
    x2: f32,
    y2: f32,
    t: f64,
    width: f32,
    height: f32,
) -> bool {
    let mut hands = vec![(pose, x, y)];
    if !pose2.is_empty() {
        hands.push((pose2, x2, y2));
    }
    if let Some(frame) = crate::hand_input::synth_frame(&hands) {
        push_raw(&frame.encode(), t / 1000.0);
    }
    HANDS.with(|h| h.borrow_mut().synth(&hands, t / 1000.0, (width, height)))
}

/// Moves the hands made since they came on.
#[wasm_bindgen]
pub fn verse_hand_moves() -> u32 {
    HANDS.with(|h| h.borrow().moves)
}

/// Starts Everglade when the module's `init` finishes.
#[wasm_bindgen(start)]
pub fn start() {
    // A panic is logged and the page is told, so it doesn't sit on a dead
    // canvas.
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        stopped_event();
    }));
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        let _ = crate::theme::install(&document);
    }
    if log::set_logger(&CONSOLE).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            web_sys::console::error_1(&JsValue::from_str(&error));
            status(&format!("Everglade could not start: {error}"));
        }
    });
}

/// Forwards warnings and errors, such as wgpu's validation errors, to the
/// browser console.
struct Console;

static CONSOLE: Console = Console;

impl log::Log for Console {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let text = JsValue::from_str(&format!("{}: {}", record.target(), record.args()));
        if record.level() == log::Level::Error {
            web_sys::console::error_1(&text);
        } else {
            web_sys::console::warn_1(&text);
        }
    }

    fn flush(&self) {}
}

/// A renderer drawing into `canvas` through `backends`: WebGPU when the
/// browser offers an adapter and `backends` admits it, otherwise WebGL2.
async fn open(
    canvas: &HtmlCanvasElement,
    backends: wgpu::Backends,
    runtime: &WorldRuntime,
    atlas: &Atlas,
    width: u32,
    height: u32,
) -> Result<Renderer, String> {
    // WebGL2 creates its canvas surface through the instance's display
    // handle; WebGPU ignores it.
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    descriptor.backends = backends;
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    Renderer::from_surface_async(
        instance,
        surface,
        width,
        height,
        &runtime.world.mesh,
        atlas,
        RenderOptions {
            max_extent: MAX_SIDE,
            ..RenderOptions::default()
        },
    )
    .await
}

/// The Grid on the engine renderer, drawing into `canvas` through
/// `backends`, with the pinned pack built into the module.
async fn open_grid(
    canvas: &HtmlCanvasElement,
    backends: wgpu::Backends,
    atlas: &Atlas,
    width: u32,
    height: u32,
    stage: Option<StageCell>,
) -> Result<GridEngine, String> {
    let reach = |reached: Stage| {
        if let Some(stage) = &stage {
            stage.set(Some(reached));
        }
    };
    // Three tasks, not one (#12319: the start blocked the page for 167 ms):
    // the pack's meshes on the CPU, the GPU's adapter and device, then the
    // upload. The page gets a turn between each.
    let content = if HOME.with(std::cell::Cell::get) {
        crate::home_terminal::content()?
    } else {
        verse::grid_engine::Content::grid()?
    };
    mark("verse-grid-content");
    reach(Stage::World);
    yield_to_page().await;
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    descriptor.backends = backends;
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    let gpu = Gpu::open_async(instance, &surface).await?;
    mark("verse-grid-gpu");
    reach(Stage::Device);
    yield_to_page().await;
    let engine = GridEngine::on_surface_with(gpu, surface, content, atlas, width, height)?;
    mark("verse-grid-engine");
    if HOME.with(std::cell::Cell::get) {
        let _ = canvas.set_attribute("data-pylon-terminal", "world");
        // The pylons layer labels the machines Verse draws instead of
        // drawing its own pillars (#12423).
        let _ = canvas.set_attribute("data-pylon-bay", "world");
    }
    Ok(engine)
}

/// Lets the page run its own tasks (input, the chat, painting) before the
/// next step of the start: a timer task, not a microtask, so it really
/// yields.
async fn yield_to_page() {
    gloo_timers::future::TimeoutFuture::new(0).await;
}

/// A `performance.mark` the page's start timings read.
fn mark(name: &str) {
    if let Some(performance) = web_sys::window().and_then(|window| window.performance()) {
        let _ = performance.mark(name);
    }
}

/// What draws the page: the Grid through the engine renderer, Everglade and
/// the Grove through the legacy renderer until their own migration. A Grid
/// page that walks into a zone (Lagrange 2) swaps to the legacy renderer and
/// back; `Opening` holds the canvas while the next renderer opens.
enum Draw {
    Legacy(Renderer),
    Grid(GridEngine),
    Opening {
        next: Rc<RefCell<Option<Result<Draw, String>>>>,
        size: [f32; 2],
        /// The zone revision whose world the next renderer was opened
        /// with.
        revision: u64,
    },
}

impl Draw {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        match self {
            Self::Legacy(renderer) => renderer.resize(width, height),
            Self::Grid(engine) => engine.resize(width, height),
            Self::Opening { size, .. } => {
                *size = [width as f32, height as f32];
                Ok(())
            }
        }
    }

    fn aspect(&self) -> f32 {
        match self {
            Self::Legacy(renderer) => renderer.aspect(),
            Self::Grid(engine) => engine.aspect(),
            Self::Opening { size, .. } => size[0] / size[1].max(1.0),
        }
    }

    fn size(&self) -> [f32; 2] {
        match self {
            Self::Legacy(renderer) => renderer.size(),
            Self::Grid(engine) => engine.size(),
            Self::Opening { size, .. } => *size,
        }
    }
}

/// The legacy renderer for `world`, for a zone the Grid page walked into.
async fn open_zone(
    canvas: HtmlCanvasElement,
    backends: wgpu::Backends,
    world: verse::mesh::Mesh,
    atlas: Atlas,
    atmosphere: zones::Atmosphere,
    width: u32,
    height: u32,
    stage: StageCell,
) -> Result<Draw, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    descriptor.backends = backends;
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    let mut renderer = Renderer::from_surface_async_with(
        instance,
        surface,
        width,
        height,
        &world,
        &atlas,
        RenderOptions {
            max_extent: MAX_SIDE,
            ..RenderOptions::default()
        },
        || stage.set(Some(Stage::Device)),
    )
    .await?;
    renderer.set_atmosphere(atmosphere)?;
    Ok(Draw::Legacy(renderer))
}

/// The browser's display, for wgpu's WebGL2 backend.
#[derive(Debug)]
pub(crate) struct WebDisplay;

impl raw_window_handle::HasDisplayHandle for WebDisplay {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::web())
    }
}

/// The first line of a possibly long error, such as a shader compiler's,
/// bounded for the page's status line.
fn first_line(error: &str) -> String {
    let line = error.lines().next().unwrap_or(error);
    line.chars().take(240).collect()
}

/// A step the renderer opening on its own task has finished, for the load
/// screen; the frame loop takes it.
type StageCell = Rc<std::cell::Cell<Option<Stage>>>;

/// The way between the Grid and a zone (#12438): the load screen, and the
/// zone panel that carries a zone's own controls.
#[derive(Default)]
struct Trip {
    /// The trip under way, while its load screen is up.
    load: Option<Load>,
    view: Option<LoadView>,
    /// When the load screen began fading out, ms.
    fading: Option<f64>,
    /// The step the opening renderer reached.
    stage: StageCell,
    /// The zone and place the page was last told.
    told: Option<(&'static str, Place)>,
    /// The zone panel: Lagrange's controls, as on the desktop.
    hud: zones::hud::Hud,
    panel: Option<zones::hud::Snapshot>,
}

/// Everything the frame loop owns.
pub(crate) struct Page {
    canvas: HtmlCanvasElement,
    pub(crate) runtime: WorldRuntime,
    renderer: Draw,
    input: Input,
    rendered_revision: u64,
    /// Device pixels per CSS pixel.
    scale: f32,
    /// The previous frame's `performance.now()`, in milliseconds.
    last: Option<f64>,
    stopped: bool,
    /// The HUD atlas at CSS-pixel metrics, for the hotbar.
    layout: Option<Atlas>,
    /// A held descent (-1, the X key) and the pointer holding it, if any.
    climb: Option<(Option<i32>, f32)>,
    /// The pointer holding Levitate down (`None` inside for a key), if any.
    levitate: Option<Option<i32>>,
    /// When the primary button or a touch went down, in milliseconds: in
    /// the demolition yard a quick tap swings the hammer.
    pressed_at: Option<f64>,
    /// The mouse over the canvas, in CSS pixels, for a hotbar slot's card.
    hover: Option<[f32; 2]>,
    /// The hotbar slot the mouse rests on, for its card's delay.
    slot_tip: verse::tooltip::Dwell,
    /// A touch holding a hotbar slot: the pointer, the slot, its intent,
    /// and when it went down, in milliseconds. A long press shows the
    /// slot's card instead of using it.
    slot_touch: Option<(i32, usize, zones::Intent, f64)>,
    /// The latest frame's `performance.now()`, in milliseconds.
    now: f64,
    /// Each held Grove key's code and the slot it pressed, so letting go
    /// releases that slot whatever modifiers are held then.
    grove_keys: Vec<(String, zones::Intent)>,
    /// The row a compact Grove bar shows.
    grove_row: usize,
    /// The Grid's presence session, when the page opened the Grid online.
    presence: Option<Presence>,
    /// The build billboard on the Grid homepage's plaza (#12442).
    board: Option<crate::build_board::BuildBoard>,
    /// With `?frames`, the frame times gathered for the console's
    /// once-a-second line.
    frames: Option<Frames>,
    water_dry: bool,
    /// The backends and glyph atlas the page opened its renderer with, to
    /// open the next one when a Grid page walks into a zone and back.
    backends: wgpu::Backends,
    draw_atlas: Option<Atlas>,
    /// A Grid portal's zone on its way ([`crate::transit`]).
    transit: crate::transit::Transit<zones::ZoneId>,
    /// Zoom from the wheel and the zoom keys, applied over a few frames.
    zoom: crate::overview::SmoothZoom,
    /// `/home` (#12406): where the visitor is between the overview and
    /// their avatar.
    arrival: Arrival,
    /// The load screen and the zone panel (#12438).
    trip: Trip,
}

/// `/home`'s camera as the visitor joins (#12406).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arrival {
    /// The overview, before the visitor joins, and while their spawn is
    /// chosen.
    Watching,
    /// Gliding down behind the avatar, from this shot at these seconds.
    Gliding(crate::overview::Shot, f64),
    /// The third-person camera: the page's own. Every other page starts
    /// here.
    Joined,
}

/// Frame times over the current second, for `?frames`.
#[derive(Default)]
struct Frames {
    /// When the second began, in milliseconds.
    since: f64,
    count: u32,
    /// The time between frames and the time the page spent in a frame, in
    /// milliseconds: their sums and their largest.
    gap: (f64, f64),
    work: (f64, f64),
    water: Vec<verse::render::WaterMeasurements>,
}

impl Frames {
    /// Adds one frame that came `gap` ms after the last and took `work` ms,
    /// and logs the second when it is over.
    fn add(
        &mut self,
        now: f64,
        gap: f64,
        work: f64,
        wreckage: Option<[usize; 3]>,
        offline_light: Option<bool>,
        water: Option<verse::render::WaterMeasurements>,
    ) {
        if self.count == 0 && self.since == 0.0 {
            self.since = now;
        }
        self.count += 1;
        if let Some(water) = water.filter(|w| w.gpu_bytes > 0) {
            self.water.push(water);
        }
        self.gap = (self.gap.0 + gap, self.gap.1.max(gap));
        self.work = (self.work.0 + work, self.work.1.max(work));
        if now - self.since < 1000.0 {
            return;
        }
        let n = f64::from(self.count.max(1));
        let [raised, pieces, chunks] = wreckage.unwrap_or_default();
        let offline_light =
            offline_light.map_or("null", |active| if active { "true" } else { "false" });
        web_sys::console::info_1(&JsValue::from_str(&format!(
            "Everglade frames {{\"frames\":{},\"gap_ms\":{:.1},\"gap_max_ms\":{:.1},\"work_ms\":{:.2},\"work_max_ms\":{:.2},\"raised\":{raised},\"pieces\":{pieces},\"chunks\":{chunks},\"offline_light\":{offline_light}}}",
            self.count,
            self.gap.0 / n,
            self.gap.1,
            self.work.0 / n,
            self.work.1,
        )));
        if !self.water.is_empty() {
            if let Ok(record) = serde_json::to_string(&self.water) {
                web_sys::console::info_1(&JsValue::from_str(&format!("Everglade water {record}")));
            }
        }
        *self = Self {
            since: now,
            ..Self::default()
        };
    }
}

/// Whether the page's query string has `name`, alone or as `name=value`.
fn query_has(window: &Window, name: &str) -> bool {
    query_value(window, name).is_some()
}

/// The value of `name` in the page's query string: empty for a bare
/// `name`.
fn query_value(window: &Window, name: &str) -> Option<String> {
    let query = window.location().search().ok()?;
    query
        .trim_start_matches('?')
        .split('&')
        .find_map(|part| match part.split_once('=') {
            Some((key, value)) if key == name => Some(value.to_owned()),
            None if part == name => Some(String::new()),
            _ => None,
        })
}

async fn run() -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| format!("the page has no #{CANVAS_ID}"))?
        .dyn_into()
        .map_err(|_| format!("#{CANVAS_ID} is not a canvas"))?;
    if query_value(&window, "zone").as_deref() == Some("chamber") {
        return crate::chamber::run(window, document, canvas).await;
    }
    // The canvas takes drags and pinches instead of the page scrolling.
    let _ = canvas.style().set_property("touch-action", "none");

    // `?zone=grove`, or the `/druid` page, starts in the Grove, the druid
    // training field built on the same pack.
    let grove = window.location().search().is_ok_and(|query| {
        query
            .trim_start_matches('?')
            .split('&')
            .any(|part| part.eq_ignore_ascii_case("zone=grove"))
    }) || window
        .location()
        .pathname()
        .is_ok_and(|path| path.trim_end_matches('/').ends_with("/druid"));
    // `?zone=grid`, or the `/grid` or `/verse` page (#12261), opens the shared Grid, the spawn
    // plaza, on the engine renderer with the pack built into this module;
    // nothing downloads.
    let grid = window.location().search().is_ok_and(|query| {
        query
            .trim_start_matches('?')
            .split('&')
            .any(|part| part.eq_ignore_ascii_case("zone=grid"))
    }) || window.location().pathname().is_ok_and(|path| {
        let path = path.trim_end_matches('/');
        path.ends_with("/grid") || path.ends_with("/verse")
    }) || ambient_page(&window, &canvas);
    if ambient_page(&window, &canvas) {
        AMBIENT.with(|ambient| ambient.set(true));
    }
    HOME.with(|home| {
        home.set(
            window
                .location()
                .pathname()
                // The Grid homepage: `/`, or `/home` before it moved (#12413).
                .is_ok_and(|p| matches!(p.trim_end_matches('/'), "" | "/home")),
        );
    });
    if grid {
        return run_grid(window, document, canvas).await;
    }
    // `?zone=arena`, or the `/arena` page, opens the Grid Arena, the robot
    // duel. Its geometry is generated, so nothing downloads.
    // `?zone=hand-duel`, or the `/hand-duel` page, opens the Hand Duel,
    // played with the camera's hands. Its geometry is generated too.
    let hand_duel = query_value(&window, "zone").as_deref() == Some("hand-duel")
        || window
            .location()
            .pathname()
            .is_ok_and(|path| path.trim_end_matches('/').ends_with("/hand-duel"));
    let arena = hand_duel
        || query_value(&window, "zone").as_deref() == Some("arena")
        || window
            .location()
            .pathname()
            .is_ok_and(|path| path.trim_end_matches('/').ends_with("/arena"));
    let bytes = if arena {
        Vec::new()
    } else {
        download(&window).await?
    };
    let mut runtime = WorldRuntime::new();
    // The town clock runs, as on the desktop; `?town-clock=off` stops it in
    // late-morning daylight and `?town-hour=18.5` pins the hour.
    let setting =
        query_value(&window, "town-clock").and_then(|v| verse::town_clock::Setting::parse(&v).ok());
    let hour =
        query_value(&window, "town-hour").and_then(|v| verse::town_clock::parse_hour(&v).ok());
    runtime.set_town_clock(verse::town_clock::Clock::from_settings(setting, hour));
    // `?demolition` opens the demolition yard: two kit cottages to knock
    // down with a sledgehammer, as `verse --demolition` does.
    runtime.set_demolition(
        window
            .location()
            .search()
            .is_ok_and(|query| query.split(['?', '&']).any(|part| part == "demolition")),
    );
    // `?zone=water` opens the Water Lab, the cove for Verse's water, built
    // on the same pack (`docs/verse/water.md`).
    let water = query_value(&window, "zone").as_deref() == Some("water");
    if hand_duel {
        runtime.enter_hand_duel()?;
    } else if arena {
        runtime.enter_arena()?;
    } else if query_value(&window, "zone").as_deref() == Some("coast") {
        status("Opening the coast…");
        runtime.install_coast_bytes(&bytes)?;
    } else if matches!(
        query_value(&window, "zone").as_deref(),
        Some("meteor-showcase" | "meteor-stress-test")
    ) {
        let pack = everglade_pack::ZonePack::decode_pinned(&bytes)?;
        if query_value(&window, "zone").as_deref() == Some("meteor-showcase") {
            runtime.install_meteor_showcase(&pack);
        } else {
            runtime.install_meteor_stress_test(&pack);
        }
    } else if water {
        status("Opening the Water Lab…");
        runtime.install_water_lab_bytes(&bytes)?;
    } else if grove {
        status("Opening the Grove…");
        runtime.install_grove_bytes(&bytes)?;
    } else {
        let kit = download_kit(&window).await;
        if kit.is_some() {
            download_bake(&window).await;
        }
        status("Opening Everglade…");
        runtime.install_everglade_bytes_with_kit(&bytes, kit.as_deref())?;
    }
    #[cfg(test)]
    if query_has(&window, "light-proof") {
        crate::light_proof::export(&runtime)?;
    }
    // The page opens no studio panel, on a keyboard or a touchscreen.
    if !arena && let Err(error) = load_cad_models(&window, &mut runtime).await {
        web_sys::console::warn_1(&JsValue::from_str(&error));
    }
    runtime.interact_hint = verse::runtime::InteractHint::None;
    drop(bytes);
    // `?at=X,Z` or `?at=X,Z,YAW` starts the player there, for captures of
    // one place in the town.
    if let Some(at) = query_value(&window, "at") {
        let numbers: Vec<f32> = at.split(',').filter_map(|n| n.parse().ok()).collect();
        if let [x, z, ref rest @ ..] = numbers[..] {
            let yaw = rest.first().copied().unwrap_or(0.0);
            let _ = runtime.set_spawn(glam::Vec3::new(x, 0.0, z), yaw);
        }
    }

    let scale = (window.device_pixel_ratio() as f32).clamp(1.0, 3.0);
    let (width, height) = drawing_size(&canvas, scale);
    canvas.set_width(width);
    canvas.set_height(height);
    // `?gl` forces WebGL2, the path browsers without WebGPU take, so it can
    // be checked from any browser.
    let force_gl = window
        .location()
        .search()
        .is_ok_and(|query| query.split(['?', '&']).any(|part| part == "gl"));
    let backends = if force_gl {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    let mut atlas = Atlas::new((14.0 * scale).round());
    zones::everglade::hotbar::add_sprites(&mut atlas)?;
    zones::grove::hotbar::add_sprites(&mut atlas)?;
    zones::everglade::demolition::hotbar::add_sprites(&mut atlas)?;
    let mut canvas = canvas;
    let mut renderer = open(&canvas, backends, &runtime, &atlas, width, height).await?;
    // Everglade's textured world draws only on the physical renderer. A
    // browser whose WebGPU rejects it (WebKit's, on some devices) may still
    // run it on WebGL2, so try that on a fresh canvas: a canvas that has a
    // WebGPU context cannot open a WebGL2 one.
    if let Some(error) = renderer.physical_error().map(str::to_owned)
        && !force_gl
    {
        web_sys::console::warn_1(&JsValue::from_str(&format!(
            "Everglade: WebGPU cannot run the physical renderer, trying WebGL2: {error}"
        )));
        if let Ok(fresh) = canvas
            .clone_node()
            .map(|node| node.unchecked_into::<HtmlCanvasElement>())
        {
            fresh.set_width(width);
            fresh.set_height(height);
            if let Ok(fallback) =
                open(&fresh, wgpu::Backends::GL, &runtime, &atlas, width, height).await
                && fallback.physical_error().is_none()
                && canvas.replace_with_with_node_1(&fresh).is_ok()
            {
                canvas = fresh;
                renderer = fallback;
            }
        }
    }
    if query_has(&window, "frames") {
        renderer.enable_water_timing();
        web_sys::console::info_1(&JsValue::from_str(&format!(
            "Everglade water renderer {}",
            serde_json::json!({
                "tier":renderer.quality().tier.name(),
                "physical":renderer.water_measurements().is_some(),
            })
        )));
    }
    // Without the physical renderer the page would show an empty field, so
    // it says why instead. The arena draws only lines and faces.
    let unavailable = renderer.physical_error().filter(|_| !arena).map(|error| {
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "Everglade: the physical renderer is unavailable: {error}"
        )));
        format!(
            "This browser's graphics cannot draw Everglade's world yet: {}",
            first_line(error)
        )
    });
    renderer.set_atmosphere(zones::atmosphere(runtime.zone))?;
    match &unavailable {
        Some(reason) => status(reason),
        None => hide_status(&document),
    }

    let page = Rc::new(RefCell::new(Page {
        canvas,
        rendered_revision: runtime.zone_revision,
        runtime,
        renderer: Draw::Legacy(renderer),
        input: Input::default(),
        scale,
        last: None,
        stopped: false,
        layout: atlas.layout_at_scale(scale),
        climb: None,
        levitate: None,
        pressed_at: None,
        hover: None,
        slot_tip: verse::tooltip::Dwell::default(),
        slot_touch: None,
        now: 0.0,
        grove_keys: Vec::new(),
        grove_row: 0,
        presence: None,
        board: None,
        frames: query_has(&window, "frames").then(Frames::default),
        water_dry: query_has(&window, "frames") && query_has(&window, "water-dry"),
        backends,
        draw_atlas: None,
        transit: Default::default(),
        zoom: Default::default(),
        trip: Trip::default(),
        arrival: Arrival::Joined,
    }));
    listen(&window, &page)?;
    crate::cad_sync::start(window.clone(), page.clone());
    animate(window, page);
    Ok(())
}

/// Opens the Grid: the bare plaza drawn through the engine renderer.
async fn run_grid(
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
) -> Result<(), String> {
    status("Opening the Grid…");
    let mut runtime = WorldRuntime::bare();
    if query_value(&window, "lab").as_deref() == Some("game") {
        let _ = runtime.set_spawn(
            zones::game_place::POSITION + glam::Vec3::new(0.0, 0.0, -2.0),
            0.0,
        );
    }
    // The page opens a portal's zone itself: it says what loads first, and
    // refuses a zone this build can't open ([`crate::transit`]).
    runtime.defer_grid_portals(true);
    // `?hosted=TENANT[,TENANT...]`: arches to zones hosted on this origin.
    let hosted = crate::hosted::from_query(&window);
    if !hosted.is_empty() {
        runtime.set_hosted_portals(hosted);
    }
    runtime.interact_hint = verse::runtime::InteractHint::None;
    // `?at=X,Z` or `?at=X,Z,YAW` stands the player there, as on the
    // Everglade page, for captures (an arch's approach).
    if let Some(at) = query_value(&window, "at") {
        let numbers: Vec<f32> = at.split(',').filter_map(|n| n.parse().ok()).collect();
        if let [x, z, ref rest @ ..] = numbers[..] {
            let yaw = rest.first().copied().unwrap_or(0.0);
            let _ = runtime.set_spawn(glam::Vec3::new(x, 0.0, z), yaw);
        }
    }
    let ambient = AMBIENT.with(std::cell::Cell::get);
    // Behind the chat the Grid draws at no more than 1.5 device pixels a
    // CSS pixel, leaving the GPU room for the chat's own canvas.
    let most = if ambient { 1.5 } else { 3.0 };
    let scale = (window.device_pixel_ratio() as f32).clamp(1.0, most);
    let (width, height) = drawing_size(&canvas, scale);
    canvas.set_width(width);
    canvas.set_height(height);
    let force_gl = window
        .location()
        .search()
        .is_ok_and(|query| query.split(['?', '&']).any(|part| part == "gl"));
    let backends = if force_gl {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
    };
    let atlas = Atlas::new((14.0 * scale).round());
    mark("verse-grid-atlas");
    yield_to_page().await;
    let engine = open_grid(&canvas, backends, &atlas, width, height, None).await?;
    web_sys::console::info_1(&JsValue::from_str(&format!(
        "Grid: engine renderer on {}",
        engine.adapter_name()
    )));
    // The admitted shadow and pose layout, for a downlevel (WebGL2) report.
    web_sys::console::info_1(&JsValue::from_str(&format!(
        "Grid: device profile {}",
        engine.device_profile()
    )));
    hide_status(&document);
    // Presence over the browser's WebSocket; a page that cannot join still
    // shows the Grid.
    let options = grid::Options::parse(&window.location().search().unwrap_or_default());
    // `/home` watches the Grid's players from the overview until the
    // visitor joins (#12406); `/lander11`'s backdrop stays offline.
    let home = HOME.with(std::cell::Cell::get);
    let presence = if ambient && !home {
        Ok(None)
    } else {
        Presence::open(&window, &options, home)
    }
    .unwrap_or_else(|error| {
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "Grid: presence is off: {error}"
        )));
        None
    });
    let page = Rc::new(RefCell::new(Page {
        canvas,
        rendered_revision: runtime.zone_revision,
        runtime,
        renderer: Draw::Grid(engine),
        input: Input::default(),
        scale,
        last: None,
        stopped: false,
        layout: atlas.layout_at_scale(scale),
        climb: None,
        levitate: None,
        pressed_at: None,
        hover: None,
        slot_tip: verse::tooltip::Dwell::default(),
        slot_touch: None,
        now: 0.0,
        grove_keys: Vec::new(),
        grove_row: 0,
        presence,
        board: home.then(|| {
            crate::build_board::BuildBoard::new(crate::build_board::BuildBoard::wants_test(&window))
        }),
        frames: None,
        water_dry: false,
        backends,
        draw_atlas: Some(atlas.clone()),
        transit: Default::default(),
        zoom: Default::default(),
        trip: Trip::default(),
        arrival: if HOME.with(std::cell::Cell::get) {
            Arrival::Watching
        } else {
            Arrival::Joined
        },
    }));
    listen(&window, &page)?;
    animate(window, page);
    Ok(())
}

/// Loads only the signed-in person's CAD copies, with same-origin credentials.
pub(crate) async fn fetch_cad_models(
    window: &Window,
    zone: &str,
) -> Result<Vec<(zones::cad_place::Placement, Vec<u8>)>, String> {
    let init = RequestInit::new();
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_redirect(RequestRedirect::Error);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init("/cad-world", &init))
        .await
        .map_err(|_| "Your CAD models couldn't be loaded.")?
        .dyn_into()
        .map_err(|_| "Your CAD models couldn't be loaded.")?;
    if !response.ok() {
        return Ok(Vec::new());
    }
    let text = JsFuture::from(
        response
            .text()
            .map_err(|_| "Your CAD list couldn't be read.")?,
    )
    .await
    .map_err(|_| "Your CAD list couldn't be read.")?
    .as_string()
    .ok_or("Your CAD list couldn't be read.")?;
    if text.len() > 32 * 1024 {
        return Err("Your CAD list is too large.".into());
    }
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&text).map_err(|_| "Your CAD list couldn't be read.")?;
    if list.len() > 20 {
        return Err("Your world has too many CAD models.".into());
    }
    let mut models = Vec::new();
    let mut total = 0;
    for entry in list {
        if entry["zone"].as_str() != Some(zone) {
            continue;
        }
        let url = entry["glb"]
            .as_str()
            .filter(|s| s.starts_with("/cad-world/") && !s.contains(['?', '#']))
            .ok_or("This CAD model has an invalid address.")?;
        let response: Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
            .await
            .map_err(|_| "A CAD model couldn't be loaded.")?
            .dyn_into()
            .map_err(|_| "A CAD model couldn't be loaded.")?;
        if !response.ok() {
            return Err("A CAD model couldn't be loaded. Reload to try again.".into());
        }
        let bytes = Uint8Array::new(
            &JsFuture::from(
                response
                    .array_buffer()
                    .map_err(|_| "A CAD model couldn't be read.")?,
            )
            .await
            .map_err(|_| "A CAD model couldn't be read.")?,
        )
        .to_vec();
        total += bytes.len();
        if total > 32 << 20 {
            return Err("Your CAD models are too large for this view.".into());
        }
        let placement =
            serde_json::from_value(entry).map_err(|_| "A CAD placement couldn't be read.")?;
        models.push((placement, bytes));
    }
    Ok(models)
}
async fn load_cad_models(window: &Window, runtime: &mut WorldRuntime) -> Result<(), String> {
    let models = fetch_cad_models(window, runtime.zone.world_id()).await?;
    let count = runtime.install_cad_models(models)?;
    if count > 0 {
        runtime.enable_cad_sync(String::new());
    }
    Ok(())
}

async fn download(window: &Window) -> Result<Vec<u8>, String> {
    let url = format!(
        "/everglade/pack/{}.{}",
        everglade_pack::PACK_SHA256,
        everglade_pack::PACK_EXTENSION
    );
    fetch_pinned(window, &url, everglade_pack::PACK_BYTES).await
}

/// Fetches the pinned medieval kit pack from this origin,
/// `/everglade/kit/<KIT_SHA256>.vtp`, when one is pinned. The runtime
/// checks its length and digest; without it the town draws the kit's
/// committed proxies, so a failure here only logs.
async fn download_kit(window: &Window) -> Option<Vec<u8>> {
    use everglade_pack::kit::{KIT_BYTES, KIT_SHA256};
    if KIT_BYTES == 0 {
        return None;
    }
    let url = format!("/everglade/kit/{KIT_SHA256}.vtp");
    match fetch_pinned(window, &url, KIT_BYTES).await {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "Everglade draws the kit's proxies: {error}"
            )));
            None
        }
    }
}

/// Offers the verified offline layers before the town creates its light job.
async fn download_bake(window: &Window) {
    use everglade_pack::kit_bake;
    let file = kit_bake::pinned();
    if file.bytes == 0 {
        return;
    }
    let url = format!("/everglade/kit/bake/{}.vlay", file.sha256);
    match fetch_pinned(window, &url, file.bytes)
        .await
        .and_then(|bytes| kit_bake::decode_pinned(&bytes))
    {
        Ok(layers) => kit_bake::offer(layers),
        Err(error) => web_sys::console::warn_1(&JsValue::from_str(&format!(
            "Everglade uses load-time light: {error}"
        ))),
    }
}

/// Fetches `url` from this origin with progress, without credentials or
/// redirects, bounded by its pinned length `total`.
async fn fetch_pinned(window: &Window, url: &str, total: u64) -> Result<Vec<u8>, String> {
    progress(0, total);
    let init = RequestInit::new();
    init.set_redirect(RequestRedirect::Error);
    init.set_credentials(RequestCredentials::Omit);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|_| "the pack download could not connect".to_owned())?
        .dyn_into()
        .map_err(|_| "the pack download returned no response".to_owned())?;
    if !response.ok() {
        return Err(format!(
            "the pack download failed with HTTP {}",
            response.status()
        ));
    }
    let reader: web_sys::ReadableStreamDefaultReader = response
        .body()
        .ok_or("the pack download has no body")?
        .get_reader()
        .dyn_into()
        .map_err(|_| "the pack download cannot be read".to_owned())?;
    let mut bytes = Vec::with_capacity(total as usize);
    let mut reported = 0;
    loop {
        let chunk = JsFuture::from(reader.read())
            .await
            .map_err(|_| "the pack download was interrupted".to_owned())?;
        let done = Reflect::get(&chunk, &JsValue::from_str("done"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if done {
            break;
        }
        let value: Uint8Array = Reflect::get(&chunk, &JsValue::from_str("value"))
            .ok()
            .and_then(|v| v.dyn_into().ok())
            .ok_or("the pack download sent an unreadable chunk")?;
        let start = bytes.len();
        let end = start + value.length() as usize;
        if end as u64 > total {
            let _ = reader.cancel();
            return Err("the pack download exceeds its pinned size".into());
        }
        bytes.resize(end, 0);
        value.copy_to(&mut bytes[start..]);
        // About a hundred updates per download.
        if end - reported >= (total / 100) as usize || end as u64 == total {
            progress(end as u64, total);
            reported = end;
        }
    }
    if bytes.len() as u64 != total {
        return Err("the pack download ended early".into());
    }
    Ok(bytes)
}

/// The space sky's files (`verse::pbr::gpu::SPACE_FILES`), fetched from
/// `/everglade/space/` the first time a space frame asks for them: they
/// are 5.7 MB of PNGs the module no longer carries, and the Grid on /verse
/// and /lander11 never draws space (#12319). The frame draws its
/// stand-ins until they arrive.
fn fetch_space_when_wanted() {
    thread_local! {
        static ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if !verse::pbr::gpu::space_files_wanted() || ASKED.with(|asked| asked.replace(true)) {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        let Some(window) = web_sys::window() else {
            return;
        };
        let mut files: Vec<Vec<u8>> = Vec::new();
        for name in verse::pbr::gpu::SPACE_FILES {
            match fetch_bytes(&window, &format!("/everglade/space/{name}")).await {
                Ok(bytes) => files.push(bytes),
                Err(error) => {
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "Space draws without its sky: {error}"
                    )));
                    return;
                }
            }
        }
        if let Ok(files) = <[Vec<u8>; 6]>::try_from(files) {
            verse::pbr::gpu::provide_space_files(files);
        }
    });
}

/// One same-origin file's bytes, without credentials or redirects.
async fn fetch_bytes(window: &Window, url: &str) -> Result<Vec<u8>, String> {
    let init = RequestInit::new();
    init.set_redirect(RequestRedirect::Error);
    init.set_credentials(RequestCredentials::Omit);
    let response: Response = JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|_| format!("{url} could not connect"))?
        .dyn_into()
        .map_err(|_| format!("{url} returned no response"))?;
    if !response.ok() {
        return Err(format!("{url} failed with HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|_| format!("{url} has no body"))?,
    )
    .await
    .map_err(|_| format!("{url} was interrupted"))?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

/// Device-pixel drawing size for the canvas's laid-out CSS size.
fn drawing_size(canvas: &HtmlCanvasElement, scale: f32) -> (u32, u32) {
    let side = |css: i32| ((css.max(1) as f32 * scale).round() as u32).clamp(1, MAX_SIDE);
    (side(canvas.client_width()), side(canvas.client_height()))
}

impl Page {
    fn frame(&mut self, now: f64) {
        fetch_space_when_wanted();
        let gap = self.last.map_or(0.0, |last| now - last);
        let started = web_sys::window()
            .and_then(|w| w.performance())
            .map_or(now, |p| p.now());
        self.draw_frame(now);
        if HOME.with(|home| home.get()) {
            crate::help_desk::frame(self.runtime.player.pos, self.runtime.is_plaza());
            crate::foreman::frame(now);
            crate::foreman::talk_frame(self.runtime.player.pos, self.runtime.is_plaza());
            // Game Lab opens with E or a tap under its arch, on the Grid
            // plaza only, never in a zone (#12438, #12459).
            crate::game_lab::frame(self.runtime.player.pos, self.runtime.is_plaza());
            crate::controls_hint::frame(self.runtime.player.pos, self.runtime.is_plaza());
        }
        self.advance_trip(now);
        if let Some(frames) = &mut self.frames {
            let ended = web_sys::window()
                .and_then(|w| w.performance())
                .map_or(started, |p| p.now());
            let offline_light = self
                .runtime
                .everglade_zone_mut()
                .map(|zone| zone.uses_baked_light());
            let water = match &self.renderer {
                Draw::Legacy(renderer) => renderer.water_measurements(),
                _ => None,
            };
            frames.add(
                now,
                gap,
                ended - started,
                self.runtime.everglade_wreckage(),
                offline_light,
                water,
            );
            if frames.count == 0 && !self.runtime.cad_placed().is_empty() {
                let models:Vec<_>=self.runtime.cad_placed().iter().map(|p|serde_json::json!({"id":p.placement.key(),"doc":p.placement.doc,"version":p.placement.version,"state":self.runtime.cad_report(p.placement.key())})).collect();
                web_sys::console::info_1(&JsValue::from_str(&format!(
                    "Everglade CAD physics {}",
                    serde_json::json!({"models":models})
                )));
            }
        }
    }

    fn draw_frame(&mut self, now: f64) {
        let dt = self
            .last
            .map_or(0.0, |last| ((now - last) / 1000.0) as f32)
            .clamp(0.0, MAX_STEP);
        self.last = Some(now);
        self.now = now;

        let (width, height) = drawing_size(&self.canvas, self.scale);
        if (width, height) != (self.canvas.width(), self.canvas.height()) {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            if let Err(error) = self.renderer.resize(width, height) {
                self.fail(&error);
                return;
            }
        }
        self.runtime.zone_tick();
        let first_person = self.runtime.first_person();
        journey(|j| j.first_person(first_person));
        if self.runtime.zone_revision != self.rendered_revision {
            let on_grid = self.runtime.is_bare() && self.runtime.is_plaza();
            let walks = self.draw_atlas.is_some();
            let swap = match &self.renderer {
                // Back through the zone's arch onto the Grid.
                Draw::Legacy(_) if walks && on_grid => Some(true),
                // Walked through a Grid arch into a zone the engine renderer
                // does not draw: open the legacy renderer on the canvas.
                Draw::Grid(_) if walks && !on_grid => Some(false),
                _ => None,
            };
            if let Some(grid) = swap {
                if grid && self.trip.load.is_none() {
                    self.begin_load(Way::Back, zone_ui::GRID, "the Grid", now);
                }
                self.reopen(grid);
            } else {
                match &mut self.renderer {
                    Draw::Legacy(renderer) => {
                        let replaced =
                            renderer
                                .replace_world(&self.runtime.world.mesh)
                                .and_then(|()| {
                                    renderer.set_atmosphere(zones::atmosphere(self.runtime.zone))
                                });
                        if let Err(error) = replaced {
                            self.fail(&error);
                            return;
                        }
                    }
                    // The engine draws the Grid from its pack.
                    Draw::Grid(_) if on_grid => {}
                    Draw::Grid(_) => {
                        self.fail(
                            "The browser Grid's arches lead nowhere yet: open Everglade from the \
                             page without ?zone, or the Grove with ?zone=grove",
                        );
                        return;
                    }
                    Draw::Opening { .. } => {}
                }
            }
            self.rendered_revision = self.runtime.zone_revision;
        }
        if let Draw::Opening { next, revision, .. } = &self.renderer {
            let revision = *revision;
            let opened = next.borrow_mut().take();
            match opened {
                Some(Ok(draw)) => {
                    self.renderer = draw;
                    self.reach(Stage::Renderer);
                    // It already holds that revision's world; a zone that
                    // changed again while it opened differs from it. (A
                    // second upload of the same world was refused as too
                    // big for Lagrange 2's factory, #11651.)
                    self.rendered_revision = revision;
                }
                Some(Err(error)) if !self.runtime.is_plaza() && self.draw_atlas.is_some() => {
                    // A zone's renderer didn't open: back to the Grid, with a
                    // line saying so, rather than a stopped page.
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "Verse: {} did not open: {error}",
                        self.runtime.zone_label()
                    )));
                    let label = self.runtime.zone_label();
                    let step = self.transit.failed(label, now);
                    self.show(step);
                    // The load screen stays up for the way back, and says
                    // why.
                    self.load_failed(&error, now);
                    self.begin_load(Way::Back, zone_ui::GRID, "the Grid", now);
                    if let Some(load) = &mut self.trip.load {
                        load.note = Some(crate::transit::failed_text(label));
                        if let Some(view) = &self.trip.view {
                            view.update(load);
                        }
                    }
                    let _ = self.runtime.zone_intent(zones::Intent::Return);
                    self.rendered_revision = self.runtime.zone_revision;
                    self.reopen(true);
                    return;
                }
                Some(Err(error)) => {
                    self.fail(&error);
                    return;
                }
                None => {}
            }
        }
        if super::terminal::active() || editing() {
            self.input = super::input::Input::default();
        }
        let mut input = self.input.take();
        let (mut moves, mut walking) = HANDS.with(|h| h.borrow_mut().take());
        let raw: Vec<(Vec<f32>, f64)> = RAW.with(|r| r.borrow_mut().drain(..).collect());
        if let Some(duel) = self.runtime.hand_duel_mut() {
            // The Hand Duel reads both hands itself; they don't steer the
            // camera or walk.
            if let Some(on) = RAW_ON.with(std::cell::Cell::take) {
                duel.set_tracking(on);
            }
            for (data, t) in &raw {
                duel.hand_frame(data, *t);
            }
            moves.clear();
            walking = false;
        }
        for hand in moves {
            let _ = self.runtime.apply(match hand {
                crate::hand_input::Move::Look { dx, dy } => Action::Look { dx, dy },
                crate::hand_input::Move::Zoom(scale) => Action::PinchZoom { scale },
            });
        }
        input.forward |= walking;
        if editing() {
            input = verse::controller::InputState::default();
        }
        // A portal walked through, or asked for by the page: say what
        // loads, then build it a couple of frames later.
        let page_asked = PORTAL.with(|portal| portal.borrow_mut().take());
        if page_asked == Some(zones::ZoneId::Plaza) && !self.runtime.is_plaza() {
            let _ = self.runtime.zone_intent(zones::Intent::Return);
        }
        let asked = self
            .runtime
            .take_portal_asked()
            .or(page_asked)
            .filter(|zone| self.runtime.grid_portal_zones().contains(zone));
        if let Some(zone) = asked {
            let step = self.transit.ask(zone, web_support(zone), zone.label(), now);
            if crate::transit::refused(&step) {
                // A portal this build can't open, for analytics (#12439).
                custom_event(
                    "verse-zone-unavailable",
                    &serde_json::json!({"zone": zone_name(zone)}).to_string(),
                );
            }
            if self.transit.busy() {
                // The load screen says what loads, rather than a line.
                self.begin_load(Way::In, zone_name(zone), zone.label(), now);
            } else {
                self.show(step);
            }
        }
        let opening = matches!(self.renderer, Draw::Opening { .. });
        let step = self.transit.frame(now, opening);
        if let crate::transit::Step::Build(zone) = step {
            let clock = web_sys::window().and_then(|w| w.performance());
            let began = clock.as_ref().map_or(0.0, |p| p.now());
            let entered = self.runtime.enter_grid_portal(zone);
            let took = clock.as_ref().map_or(0.0, |p| p.now()) - began;
            web_sys::console::info_1(&JsValue::from_str(&format!(
                "Verse: built {zone:?} in {took:.0} ms"
            )));
            if entered {
                self.reach(Stage::World);
            } else {
                let step = self.transit.failed(zone.label(), now);
                self.show(step);
                self.load_failed("the zone did not open", now);
            }
        } else {
            self.show(step);
        }
        // A packed zone this build can't load: refused, back to idle.
        if self.runtime.zone_load_state() == zones::LoadState::Failed {
            self.runtime.zone_cancel_loading();
            let step = self.transit.ask(
                self.runtime.zone,
                crate::transit::Support::DesktopOnly,
                "",
                now,
            );
            self.show(step);
        }
        // The hosted zone's arch: prefetch, open, and come back.
        let hosted_open = crate::hosted::frame(&mut self.runtime);
        if hosted_open || self.transit.busy() || self.trip.load.is_some() {
            input = verse::controller::InputState::default();
        }
        let place = self.tell_place(hosted_open);
        let state = serde_json::json!({
            "zone": self.runtime.zone_label(),
            "place": place.name(),
            "scheme": format!("{:?}", self.scheme()),
            "trip": self.trip.load.as_ref().map(|load| format!("{:?}", load.stage)),
            "panel": self
                .trip
                .panel
                .as_ref()
                .filter(|panel| panel.visible)
                .map(|panel| {
                    panel
                        .buttons
                        .iter()
                        .map(|b| serde_json::json!({"label": b.label, "frame": b.frame}))
                        .collect::<Vec<_>>()
                }),
            "player": [
                self.runtime.player.pos.x,
                self.runtime.player.pos.y,
                self.runtime.player.pos.z,
                self.runtime.player.yaw,
            ],
            "draw": match &self.renderer {
                Draw::Legacy(_) => "legacy",
                Draw::Grid(_) => "grid",
                Draw::Opening { .. } => "opening",
            },
            "load": format!("{:?}", self.runtime.zone_load_state()),
            "stopped": self.stopped,
        })
        .to_string();
        WHERE.with(|w| *w.borrow_mut() = state);
        let home = HOME.with(std::cell::Cell::get);
        if !self::walking() && !editing() {
            // Behind the chat: no keys or drags, and the camera circles
            // (on `/home`, the overview does).
            input = verse::controller::InputState::default();
            if !home {
                let _ = self.runtime.apply(Action::Orbit {
                    dx: AMBIENT_ORBIT * dt,
                    dy: 0.0,
                });
            }
        }
        let lines = self.zoom.take(dt);
        if lines != 0.0 {
            let _ = self.runtime.apply(Action::Zoom { lines });
        }
        if home && self.arrival_camera(now) {
            input = verse::controller::InputState::default();
        }
        if let Some([x, z]) = FACE.with(|f| f.borrow_mut().take()) {
            let to = glam::Vec3::new(x, 0.0, z) - self.runtime.player.pos;
            if to.x.hypot(to.z) > 1e-3 {
                self.runtime.player.yaw = to.x.atan2(to.z);
            }
        }
        let dt = self
            .runtime
            .tick_with_mode(&input, dt, self.input.orbit, true);
        // Nobody walks through the Grid Foreman (#12444).
        if home && self.runtime.is_bare() && self.runtime.is_plaza() {
            crate::foreman::keep_clear(&mut self.runtime.player.pos);
        }
        if let Some((_, direction)) = self.climb {
            if self.runtime.everglade_levitating() {
                self.runtime.everglade_climb(direction, dt);
            } else {
                self.climb = None;
            }
        }

        let aspect = self.renderer.aspect();
        let view = self.runtime.view(aspect);
        let css = self.css_size();
        if let Draw::Grid(engine) = &mut self.renderer {
            let peers = self
                .presence
                .as_mut()
                .map_or_else(Vec::new, |presence| presence.tick(&mut self.runtime, dt));
            if let Some(presence) = &self.presence
                && (self.now / 250.0).floor()
                    != ((self.now - f64::from(dt) * 1000.0) / 250.0).floor()
            {
                let players = presence.players(&self.runtime).to_string();
                PLAYERS.with(|p| *p.borrow_mut() = players);
            }
            let mut peers = peers;
            if HOME.with(std::cell::Cell::get) && self.runtime.is_plaza() {
                crate::foreman::take(&mut peers, self.presence.as_mut());
            }
            let mut dynamic = grid_frame::dynamic(&self.runtime, &peers, &[]);
            if HOME.with(std::cell::Cell::get) && self.runtime.is_plaza() {
                crate::pylon_bay::instances(self.now, &mut dynamic);
                crate::arch_marks::web::instances(&self.runtime, &mut dynamic);
                crate::foreman::instances(self.now, view.eye, &mut dynamic);
            }
            if let Some(board) = &mut self.board {
                board.tick(self.presence.as_mut());
                if self.runtime.is_plaza() {
                    board.screen_instances(
                        crate::foreman::screen_task().as_deref(),
                        kiln_screen().as_deref(),
                        &mut dynamic,
                    );
                    board.instances(dynamic.len(), &mut dynamic);
                }
                if (self.now / 250.0).floor()
                    != ((self.now - f64::from(dt) * 1000.0) / 250.0).floor()
                {
                    let report = board.report().to_string();
                    BOARD.with(|b| *b.borrow_mut() = report);
                }
            }
            let lighting = grid_frame::lighting(&self.runtime.atmosphere());
            // Name tags, the connection line, and a player's card, laid
            // out in CSS pixels and drawn in device pixels.
            let mut ui = verse::ui::UiBatch::default();
            if let (Some(presence), Some(layout)) = (&self.presence, &self.layout) {
                presence.draw(
                    &mut ui,
                    layout,
                    css,
                    &self.runtime,
                    view.view_proj,
                    view.eye,
                );
            }
            if let Some(atlas) = &self.layout
                && HOME.with(std::cell::Cell::get)
            {
                crate::help_desk::draw(&mut ui, atlas, css, view.view_proj);
                if self.runtime.is_plaza() {
                    crate::foreman::draw(&mut ui, atlas, css, view.view_proj, view.eye, self.now);
                    crate::foreman::draw_prompt(&mut ui, atlas, css, view.view_proj);
                }
                if self.runtime.is_plaza() {
                    crate::arch_marks::web::draw(
                        &mut ui,
                        atlas,
                        css,
                        view.view_proj,
                        &self.runtime,
                    );
                }
                crate::game_lab::draw(&mut ui, atlas, css, view.view_proj);
                crate::controls_hint::draw(&mut ui, atlas, css);
            }
            // A model group's details over its sign, when it is near or
            // pointed at (#12403).
            if HOME.with(std::cell::Cell::get)
                && self.runtime.is_plaza()
                && let Some(atlas) = &self.layout
            {
                crate::pylon_bay::draw(
                    &mut ui,
                    atlas,
                    css,
                    view.view_proj,
                    self.runtime.player.pos,
                    self.now,
                );
                // Every other machine's label, drawn here rather than in
                // HTML over the world (#12459).
                crate::pylon_bay::draw_labels(&mut ui, atlas, css, view.view_proj);
            }
            if let Some(atlas) = &self.layout {
                super::terminal::draw(&mut ui, atlas, css);
            }
            for vertex in &mut ui.vertices {
                vertex.pos = vertex.pos.map(|v| v * self.scale);
            }
            let view_proj = view.view_proj;
            match engine.draw(view, &dynamic, &ui, &lighting) {
                Ok(render_ms) => {
                    pylons_camera(view_proj, css, self.runtime.player.pos);
                    if let Some(presence) = &mut self.presence {
                        presence.frame_times(dt, dynamic.len(), render_ms, peers.len());
                    }
                }
                Err(error) => self.fail(&error),
            }
            return;
        }
        let mut dynamic = self.runtime.dynamic_mesh();
        if self.water_dry
            && let Some(neon) = &mut dynamic.neon
        {
            neon.water = None;
        }
        // No zone panel over the world (owner, 2026-10-04): the glade and
        // its hotbar, laid out in CSS pixels and drawn in device pixels.
        let mut ui = verse::ui::UiBatch::default();
        let tip = self.slot_tip();
        if let (Some(layout), Some(slots)) = (&self.layout, self.runtime.everglade_hotbar()) {
            zones::everglade::hotbar::draw(&mut ui, layout, self.css_size(), 0.0, &slots);
            // The breath bar over the tray under water.
            if let Some(breath) = self.runtime.everglade_breath() {
                zones::everglade::water::draw_breath(
                    &mut ui,
                    layout,
                    self.css_size(),
                    0.0,
                    &breath,
                );
            }
            if let Some(index) = tip {
                zones::everglade::hotbar::draw_tip(
                    &mut ui,
                    layout,
                    self.css_size(),
                    0.0,
                    slots.len(),
                    index,
                );
            }
        }
        if let (Some(layout), Some(bar)) = (&self.layout, self.runtime.demolition_bar()) {
            zones::everglade::demolition::hotbar::draw(&mut ui, layout, self.css_size(), 0.0, &bar);
            if let Some(index) = tip {
                zones::everglade::demolition::hotbar::draw_tip(
                    &mut ui,
                    layout,
                    self.css_size(),
                    0.0,
                    index,
                );
            }
        }
        if let (Some(atlas), Some(bar)) = (&self.layout, self.runtime.grove_bar()) {
            let size = self.css_size();
            let layout = self.grove_layout();
            zones::grove::hotbar::draw(&mut ui, atlas, size, 0.0, layout, &bar);
            if let Some((status, lines)) = self.runtime.grove_log() {
                zones::grove::hotbar::draw_log(&mut ui, atlas, size, 0.0, layout, &status, &lines);
            }
            // Meteor Swarm's or the Thunderbolt's help and cast bar.
            if let Some(swarm) = self.runtime.grove_swarm() {
                zones::everglade::demolition::hotbar::draw_aim(&mut ui, atlas, size, &swarm);
            }
            if let Some(index) = tip {
                zones::grove::hotbar::draw_tip(&mut ui, atlas, size, 0.0, layout, &bar, index);
            }
        }
        // The Hand Duel's bars, belt, hands, and numbers.
        if let (Some(atlas), Some(duel)) = (&self.layout, self.runtime.hand_duel()) {
            zones::hand_duel::hud::draw(&mut ui, atlas, css, view.view_proj, duel);
        }
        // The Grid Arena's bars, numbers, and action bar.
        if let (Some(atlas), Some(arena)) = (&self.layout, self.runtime.arena()) {
            zones::arena::hud::draw(
                &mut ui,
                atlas,
                css,
                view.view_proj,
                arena,
                zones::arena::hud::CONTROLS_WEB,
            );
        }
        // Lagrange's zone panel: the zone's caption and buttons, as on the
        // desktop (#12438).
        self.trip.panel = None;
        if self.scheme().zone_panel()
            && self.trip.load.is_none()
            && let Some(atlas) = &self.layout
        {
            let snapshot = self.runtime.zone_snapshot(aspect);
            let _ = self.trip.hud.set_bottom_clearance(16.0);
            let panel = self.trip.hud.snapshot(css, &snapshot, true);
            ui.vertices
                .extend(self.trip.hud.draw(atlas, &panel, 1.0).vertices);
            self.trip.panel = Some(panel);
        }
        if let Some(atlas) = &self.layout {
            super::terminal::draw(&mut ui, atlas, css);
        }
        for vertex in &mut ui.vertices {
            vertex.pos = vertex.pos.map(|v| v * self.scale);
        }
        if let Draw::Legacy(renderer) = &mut self.renderer
            && let DrawStatus::Error(error) = renderer.draw(view, &dynamic, &ui)
        {
            self.fail(&error);
        }
    }

    /// A press at `at` (CSS pixels) on the Grid's name tags or open card.
    /// Returns whether the Grid used it.
    fn press_grid(&mut self, at: [f32; 2]) -> bool {
        // The help desk's prompt opens the chat (#12443).
        if HOME.with(std::cell::Cell::get)
            && (crate::help_desk::press(at) || crate::foreman::press(at))
        {
            return true;
        }
        // So does Game Lab's, under its arch (#12459).
        if HOME.with(std::cell::Cell::get) && crate::game_lab::press(at) {
            return true;
        }
        let size = self.css_size();
        let aspect = self.renderer.aspect();
        let view = self.runtime.view(aspect);
        let line = self.layout.as_ref().map_or(14.0, |layout| layout.line);
        let Some(presence) = &mut self.presence else {
            return false;
        };
        presence.press(at, size, line, view.view_proj, view.eye)
    }

    /// `/home`'s camera at `now` (ms, #12406): the overview until the
    /// visitor joins and their spawn is chosen, then a glide down behind
    /// their avatar into the page's third-person camera. Returns whether
    /// the visitor is still on their way in, when they don't walk yet.
    fn arrival_camera(&mut self, now: f64) -> bool {
        use crate::overview;
        let seconds = now / 1000.0;
        if self::walking() && self.arrival == Arrival::Watching {
            if let (Some(presence), Some(window)) = (&mut self.presence, web_sys::window()) {
                presence.join(&window);
            }
        }
        let placed = self
            .presence
            .as_ref()
            .is_none_or(|presence| presence.joined() && !presence.placing());
        match self.arrival {
            Arrival::Watching if self::walking() && placed => {
                self.runtime.set_occupied(true);
                self.arrival = Arrival::Gliding(overview::shot(seconds), seconds);
                mark("home-joined");
                journey(|j| j.joined(now));
            }
            Arrival::Watching => {
                journey(|j| {
                    j.watching(now);
                    None::<crate::visit::Event>
                });
                self.runtime.set_occupied(false);
                self.runtime.set_shot(Some(overview::shot(seconds)));
                return true;
            }
            _ => {}
        }
        if let Arrival::Gliding(from, started) = self.arrival {
            let t = overview::glide(started, seconds);
            self.runtime.set_shot(None);
            if t >= 1.0 {
                self.arrival = Arrival::Joined;
                mark("home-third-person");
                return false;
            }
            let eye = self.runtime.view(self.renderer.aspect()).eye;
            let focus = self.runtime.player.pos + glam::Vec3::Y * verse::camera::FOCUS_HEIGHT;
            self.runtime
                .set_shot(Some(overview::between(from, (eye, focus), t)));
        }
        false
    }

    /// Puts Meteor Swarm's circle on the ground under `at` (CSS pixels)
    /// while the demolition yard aims it.
    fn aim_meteor_swarm(&mut self, at: [f32; 2]) {
        let size = self.css_size();
        if size[0] > 0.0 && size[1] > 0.0 {
            let aspect = self.renderer.aspect();
            self.runtime.demolition_aim(
                aspect,
                (at[0] / size[0]).clamp(0.0, 1.0),
                (at[1] / size[1]).clamp(0.0, 1.0),
            );
        }
    }

    /// How the Grove's bar lays out on this canvas.
    fn grove_layout(&self) -> zones::grove::hotbar::Layout {
        zones::grove::hotbar::Layout::for_screen(self.css_size(), self.grove_row)
    }

    /// The canvas's size in CSS pixels.
    fn css_size(&self) -> [f32; 2] {
        let size = self.renderer.size();
        [size[0] / self.scale, size[1] / self.scale]
    }

    /// The shown hotbar slot under `at` (CSS pixels), with its intent.
    fn slot_at(&self, at: [f32; 2]) -> Option<(usize, zones::Intent)> {
        use zones::everglade::demolition::hotbar as yard;
        let size = self.css_size();
        if self.runtime.in_demolition() {
            let index = yard::slot_under(at, size, 0.0)?;
            Some((index, yard::SLOTS.get(index)?.0))
        } else if self.runtime.grove_bar().is_some() {
            let index = zones::grove::hotbar::slot_under(at, size, 0.0, self.grove_layout())?;
            Some((index, zones::grove::hotbar::intent(index)?))
        } else if let Some(slots) = self.runtime.everglade_hotbar() {
            let index = zones::everglade::hotbar::slot_under(at, size, 0.0, slots.len())?;
            Some((index, zones::everglade::hotbar::SLOTS.get(index)?.0))
        } else {
            None
        }
    }

    /// The slot whose card shows this frame: one a touch has held past a
    /// long press, or one the mouse has rested on.
    fn slot_tip(&mut self) -> Option<usize> {
        let now = self.now;
        if let Some((_, index, _, at)) = self.slot_touch {
            return verse::tooltip::long_press(((now - at) / 1000.0) as f32).then_some(index);
        }
        let slot = self.hover.and_then(|at| self.slot_at(at)).map(|(i, _)| i);
        self.slot_tip.update(slot, (now / 1000.0) as f32)
    }

    /// Presses the hotbar slot under `at` (CSS pixels) for `pointer`;
    /// returns whether one was there. A touch on a slot other than
    /// Levitate acts when it lifts, so a long press can show the card
    /// instead; Levitate rises while held.
    fn press_hotbar(&mut self, at: [f32; 2], pointer: Option<i32>, touch: bool, now: f64) -> bool {
        // A compact Grove bar's switcher shows the next row.
        if self.runtime.grove_bar().is_some()
            && zones::grove::hotbar::hit(at, self.css_size(), 0.0, self.grove_layout())
                == Some(zones::grove::hotbar::Hit::Switch)
        {
            self.grove_row = (self.grove_row + 1) % zones::grove::slots::ROWS;
            return true;
        }
        let Some((index, intent)) = self.slot_at(at) else {
            return false;
        };
        if let (true, Some(id)) = (touch, pointer) {
            self.slot_touch = Some((id, index, intent, now));
            if intent == zones::Intent::Levitate {
                self.press(intent, pointer);
            }
            return true;
        }
        self.press(intent, pointer);
        true
    }

    /// Lifts a touch from the hotbar: a short tap on a slot other than
    /// Levitate uses it; a long press or a cancel only hides the card.
    fn lift_hotbar(&mut self, pointer: i32, cancelled: bool, now: f64) {
        let Some((id, _, intent, at)) = self.slot_touch else {
            return;
        };
        if id != pointer {
            return;
        }
        self.slot_touch = None;
        let long = verse::tooltip::long_press(((now - at) / 1000.0) as f32);
        if !cancelled && !long && intent != zones::Intent::Levitate {
            self.press(intent, None);
        }
    }

    /// Levitate rises while held; any other slot acts once.
    fn press(&mut self, intent: zones::Intent, pointer: Option<i32>) {
        if intent == zones::Intent::Levitate {
            if self.levitate.is_none() && self.runtime.everglade_levitate(true).is_ok() {
                self.levitate = Some(pointer);
            }
        } else {
            let _ = self.runtime.zone_intent(intent);
        }
    }

    /// Lets go of a held descent or Levitate by `pointer` (`None` for
    /// keys).
    fn release_climb(&mut self, pointer: Option<i32>, direction: Option<f32>) {
        if let Some((held, held_direction)) = self.climb
            && held == pointer
            && direction.is_none_or(|d| d == held_direction)
        {
            self.climb = None;
        }
        if direction.is_none() && self.levitate == Some(pointer) {
            self.levitate = None;
            let _ = self.runtime.everglade_levitate(false);
        }
    }

    /// The hotbar's keys: 1 to 5 are its slots, 1 and L hold Levitate, and
    /// while levitating X holds a descent. In the Grove, 1 to 9, 0, -, and
    /// = cast row 1's slots, and with Shift, Ctrl, or Alt (`modifiers`, in
    /// that order) rows 2, 3, and 4. Returns whether it used the key.
    fn hotbar_key(&mut self, code: &str, down: bool, modifiers: [bool; 3]) -> bool {
        // The Grid Arena: 1 to 4 fire the abilities and R starts the fight
        // again.
        if self.runtime.arena().is_some() {
            let slot = match code {
                "Digit1" => Some(0),
                "Digit2" => Some(1),
                "Digit3" => Some(2),
                "Digit4" => Some(3),
                _ => None,
            };
            if let Some(slot) = slot {
                if down {
                    let _ = self.runtime.arena_press(slot);
                }
                return true;
            }
            if code == "KeyR" {
                if down {
                    self.runtime.arena_restart();
                }
                return true;
            }
            return false;
        }
        if self.runtime.grove_bar().is_some() {
            // As the Giant Eagle, X holds a descent.
            if code == "KeyX" {
                if down && self.runtime.everglade_levitating() {
                    self.climb = Some((None, -1.0));
                    return true;
                }
                if !down && self.climb.is_some() {
                    self.release_climb(None, Some(-1.0));
                    return true;
                }
                return false;
            }
            let key = match code {
                "Minus" => Some('-'),
                "Equal" => Some('='),
                _ => code
                    .strip_prefix("Digit")
                    .and_then(|d| d.chars().next())
                    .filter(|_| code.len() == 6),
            };
            let intent = if down {
                let [shift, ctrl, alt] = modifiers;
                let row = zones::grove::hotbar::row_of(shift, ctrl, alt);
                let intent = key.and_then(|k| zones::grove::hotbar::key(k, row));
                if let Some(intent) = intent {
                    self.grove_keys.retain(|(c, _)| c != code);
                    self.grove_keys.push((code.to_owned(), intent));
                }
                intent
            } else {
                self.grove_keys
                    .iter()
                    .position(|(c, _)| c == code)
                    .map(|i| self.grove_keys.remove(i).1)
            };
            let Some(intent) = intent else {
                return false;
            };
            // Every press casts, and a held key recasts until it is let go.
            let _ = self.runtime.grove_key(intent, down);
            return true;
        }
        if self.runtime.everglade_hotbar().is_none() {
            return false;
        }
        let order = self.runtime.everglade_hotbar_order();
        let slot = code
            .strip_prefix("Digit")
            .and_then(|d| d.parse::<usize>().ok())
            .and_then(|n| zones::everglade::hotbar::key_intent(n, &order));
        let intent = match (code, slot) {
            (_, Some(intent)) => intent,
            ("KeyL", _) => zones::Intent::Levitate,
            ("KeyX", _) => {
                if down && self.runtime.everglade_levitating() {
                    self.climb = Some((None, -1.0));
                    return true;
                }
                if !down && self.climb.is_some() {
                    self.release_climb(None, Some(-1.0));
                    return true;
                }
                return false;
            }
            _ => return false,
        };
        if intent == zones::Intent::Levitate {
            if down {
                self.press(intent, None);
            } else {
                self.release_climb(None, None);
            }
        } else if down {
            self.press(intent, None);
        }
        true
    }

    fn apply(&mut self, action: Option<Action>) {
        if let Some(action) = action {
            if matches!(action, Action::PinchZoom { .. }) {
                journey(|j| j.zoomed("pinch"));
            }
            let _ = self.runtime.apply(action);
        }
    }

    /// Drops the renderer and opens the next one on the same canvas: the
    /// engine for the Grid, or the legacy renderer for the zone the player
    /// walked into. The page keeps ticking while it opens.
    fn reopen(&mut self, grid: bool) {
        let Some(atlas) = self.draw_atlas.clone() else {
            return;
        };
        let size = self.renderer.size();
        let next = Rc::new(RefCell::new(None));
        // The old renderer lets go of the canvas before the next one takes it.
        self.renderer = Draw::Opening {
            next: next.clone(),
            size,
            revision: self.runtime.zone_revision,
        };
        let canvas = self.canvas.clone();
        let backends = self.backends;
        let (width, height) = (canvas.width(), canvas.height());
        let world = self.runtime.world.mesh.clone();
        let atmosphere = zones::atmosphere(self.runtime.zone);
        let stage = self.trip.stage.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let opened = if grid {
                open_grid(&canvas, backends, &atlas, width, height, Some(stage))
                    .await
                    .map(Draw::Grid)
            } else {
                open_zone(
                    canvas, backends, world, atlas, atmosphere, width, height, stage,
                )
                .await
            };
            *next.borrow_mut() = Some(opened);
        });
    }

    /// Puts a portal's line on screen, or takes it down.
    fn show(&self, step: crate::transit::Step<zones::ZoneId>) {
        match step {
            crate::transit::Step::Say(text) => status(&text),
            crate::transit::Step::Clear => {
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    hide_status(&document);
                }
            }
            _ => {}
        }
    }

    /// The zone's page name, `grid` on the Grid plaza.
    fn zone_name(&self) -> &'static str {
        if self.runtime.is_plaza() {
            zone_ui::GRID
        } else {
            zone_name(self.runtime.zone)
        }
    }

    /// How the keys, mouse, and zone panel steer here (#12438).
    fn scheme(&self) -> Scheme {
        zone_ui::scheme(self.zone_name(), HOME.with(std::cell::Cell::get))
    }

    /// Tells the page where the player is when that changed; returns it.
    /// A hosted zone open over the Grid (`hosted`) counts as a zone.
    fn tell_place(&mut self, hosted: bool) -> Place {
        let zone = if hosted { "hosted" } else { self.zone_name() };
        let place = Place::of(zone, self.transit.busy() || self.trip.load.is_some());
        if self.trip.told != Some((zone, place)) {
            self.trip.told = Some((zone, place));
            place_event(zone, place);
        }
        place
    }

    /// Puts the load screen up for a trip `way` to `zone`, labeled
    /// `label`, and fires `verse-zone-load`'s `start`.
    fn begin_load(&mut self, way: Way, zone: &'static str, label: &str, now: f64) {
        let load = Load::new(way, zone, label, now);
        if self.trip.view.is_none() {
            self.trip.view = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| LoadView::new(&d));
        }
        if let Some(view) = &self.trip.view {
            view.show(&load);
        }
        custom_event(
            "verse-zone-load",
            &load.event(load_screen::Event::Start, now, None),
        );
        self.trip.stage.set(None);
        self.trip.fading = None;
        self.trip.load = Some(load);
        mark("verse-zone-load-start");
    }

    /// The trip under way finished `stage`.
    fn reach(&mut self, stage: Stage) {
        if let Some(load) = &mut self.trip.load
            && load.reach(stage)
            && let Some(view) = &self.trip.view
        {
            view.update(load);
        }
    }

    /// The trip under way failed: `verse-zone-load`'s `failed`, and the
    /// screen lifts.
    fn load_failed(&mut self, error: &str, now: f64) {
        if let Some(load) = self.trip.load.take() {
            custom_event(
                "verse-zone-load",
                &load.event(load_screen::Event::Failed, now, Some(error)),
            );
            if let Some(view) = &self.trip.view {
                view.fade();
            }
            self.trip.fading = Some(now);
        }
    }

    /// After a frame: takes the opening renderer's steps, notes the first
    /// frame drawn by the new place's renderer, and lifts the screen.
    fn advance_trip(&mut self, now: f64) {
        if let Some(stage) = self.trip.stage.take() {
            self.reach(stage);
        }
        let drawn = !self.stopped && !matches!(self.renderer, Draw::Opening { .. });
        if drawn
            && self
                .trip
                .load
                .as_ref()
                .is_some_and(|load| load.stage >= Stage::Renderer)
        {
            self.reach(Stage::Drawn);
        }
        if let Some(load) = self.trip.load.take_if(|load| load.done()) {
            custom_event(
                "verse-zone-load",
                &load.event(load_screen::Event::Ready, now, None),
            );
            mark("verse-zone-load-ready");
            if let Some(view) = &self.trip.view {
                view.fade();
            }
            self.trip.fading = Some(now);
        }
        if self.stopped && self.trip.load.is_some() {
            self.load_failed("Verse stopped", now);
        }
        if let Some(since) = self.trip.fading
            && now - since >= load_screen::FADE_MS
        {
            self.trip.fading = None;
            if let Some(view) = &self.trip.view {
                view.hide();
            }
        }
    }

    fn fail(&mut self, error: &str) {
        stopped_event();
        self.stopped = true;
        web_sys::console::error_1(&JsValue::from_str(error));
        status(&format!("Everglade stopped: {error}"));
    }
}

/// Runs [`Page::frame`] on every animation frame until it stops.
fn animate(window: Window, page: Rc<RefCell<Page>>) {
    type Tick = Closure<dyn FnMut(f64)>;
    let tick: Rc<RefCell<Option<Tick>>> = Rc::new(RefCell::new(None));
    let next = tick.clone();
    let scheduler = window.clone();
    *tick.borrow_mut() = Some(Closure::new(move |now: f64| {
        let mut page = page.borrow_mut();
        page.frame(now);
        if page.stopped {
            // Not rescheduling ends the loop.
            return;
        }
        if let Some(callback) = next.borrow().as_ref() {
            let _ = scheduler.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    if let Some(callback) = tick.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    }
}

/// Wires keyboard, mouse, wheel, and touch events for the page's lifetime.
fn listen(window: &Window, page: &Rc<RefCell<Page>>) -> Result<(), String> {
    let canvas = page.borrow().canvas.clone();
    let key = |down: bool| {
        let page = page.clone();
        move |event: KeyboardEvent| {
            if editing() && !super::terminal::active() {
                page.borrow_mut().input.release();
                return;
            }
            let home = HOME.with(std::cell::Cell::get);
            if home
                && down
                && !event.ctrl_key()
                && !event.alt_key()
                && !event.meta_key()
                && matches!(
                    event.code().as_str(),
                    "KeyW"
                        | "KeyA"
                        | "KeyS"
                        | "KeyD"
                        | "KeyQ"
                        | "KeyE"
                        | "ArrowUp"
                        | "ArrowDown"
                        | "ArrowLeft"
                        | "ArrowRight"
                )
            {
                if !walking() {
                    journey(|j| {
                        j.join_by(crate::visit::JoinBy::Keys);
                        None::<crate::visit::Event>
                    });
                }
                WALK.with(|walk| walk.set(true));
            }
            if !walking() {
                return;
            }
            // E at the help desk opens the chat there (#12443), and under
            // Game Lab's arch opens Game Lab (#12459).
            if home
                && down
                && !event.repeat()
                && !event.ctrl_key()
                && !event.alt_key()
                && !event.meta_key()
                && (crate::help_desk::key(&event.code())
                    || crate::foreman::key(&event.code())
                    || crate::game_lab::key(&event.code()))
            {
                event.prevent_default();
                return;
            }
            if super::terminal::active() {
                if super::terminal::key(&event, down) {
                    event.prevent_default();
                }
                return;
            }
            let mut page = page.borrow_mut();
            // The Grove's rows 3 and 4 take Ctrl and Alt; elsewhere they
            // stay the browser's.
            let grove = page.runtime.grove_bar().is_some();
            let chord = event.ctrl_key() || event.alt_key();
            if event.meta_key() || (chord && !grove) {
                return;
            }
            if event.repeat() {
                event.prevent_default();
                return;
            }
            // The demolition yard's hotbar: 1 swings the sledgehammer, 2
            // aims Meteor Swarm, and R rebuilds. Escape leaves the aim or
            // stops the cast, there and in the Grove, where R restores the
            // tower. Everglade's town has no offensive spell on the web.
            if down && event.code() == "Escape" && page.runtime.demolition_cancel() {
                event.prevent_default();
                return;
            }
            if down
                && event.code() == "KeyR"
                && (page.runtime.everglade_swarm().is_some()
                    || page.runtime.grove_swarm().is_some())
            {
                let _ = page.runtime.zone_intent(zones::Intent::Rebuild);
                event.prevent_default();
                return;
            }
            if down
                && page.runtime.in_demolition()
                && let Some(intent) = zones::everglade::demolition::hotbar::key(&event.code())
            {
                let _ = page.runtime.zone_intent(intent);
                event.prevent_default();
                return;
            }
            // `=` and `-` zoom while held, in and out of first person.
            let zoom = match event.code().as_str() {
                "Equal" | "NumpadAdd" => 1.0,
                "Minus" | "NumpadSubtract" => -1.0,
                _ => 0.0,
            };
            if zoom != 0.0 && !chord && (page.runtime.is_plaza() || page.scheme().zone_panel()) {
                if down {
                    journey(|j| j.zoomed("keys"));
                }
                page.zoom.held = if down { zoom } else { 0.0 };
                event.prevent_default();
                return;
            }
            // Lagrange's F goes through a nearby arch, or into Snap
            // Station, as on the desktop (#12438).
            let scheme = page.scheme();
            if down && !chord && scheme.arch_key() && event.code() == "KeyF" {
                if page.runtime.snap_arch_near() {
                    let _ = page.runtime.zone_intent(zones::Intent::Interact);
                } else {
                    let aspect = page.renderer.aspect();
                    let portal = page.runtime.zone_snapshot(aspect).portal;
                    if portal.near && portal.visible {
                        let _ = page.runtime.zone_intent(zones::Intent::Return);
                    }
                }
                event.prevent_default();
                return;
            }
            let modifiers = [event.shift_key(), event.ctrl_key(), event.alt_key()];
            let used = page.hotbar_key(&event.code(), down, modifiers);
            let code = event.code();
            // `/` strafes with A and D (#12353); a zone keeps its own keys.
            let code = scheme.key(&code);
            if used || (!chord && page.input.key(code, down)) {
                event.prevent_default();
            }
        }
    };
    on(window, "keydown", key(true))?;
    on(window, "keyup", key(false))?;
    {
        let page = page.clone();
        on(window, "blur", move |_: web_sys::Event| {
            let mut page = page.borrow_mut();
            page.input.release();
            page.runtime.grove_release();
        })?;
    }
    {
        let page = page.clone();
        let target = canvas.clone();
        on(&canvas, "pointerdown", move |event: PointerEvent| {
            if super::terminal::pointer(&event, Some(true)) {
                event.prevent_default();
                return;
            }
            event.prevent_default();
            let _ = target.focus();
            let _ = target.set_pointer_capture(event.pointer_id());
            let mut page = page.borrow_mut();
            let on_canvas = [event.offset_x() as f32, event.offset_y() as f32];
            let touch = event.pointer_type() == "touch";
            let now = event.time_stamp();
            if event.is_primary() && event.button() == 0 && page.press_grid(on_canvas) {
                return;
            }
            // On `/home`, a click on the overview joins the Grid (#12406).
            if HOME.with(std::cell::Cell::get) && !walking() && event.is_primary() {
                let by = if touch {
                    crate::visit::JoinBy::Touch
                } else {
                    crate::visit::JoinBy::Click
                };
                journey(|j| {
                    j.join_by(by);
                    None::<crate::visit::Event>
                });
                WALK.with(|walk| walk.set(true));
                return;
            }
            // Lagrange's zone panel takes a press on it (#12438).
            let id = u64::from(event.pointer_id() as u32);
            if let Some(panel) = page.trip.panel.clone()
                && page.trip.hud.down(id, on_canvas, &panel)
            {
                return;
            }
            if page.press_hotbar(on_canvas, Some(event.pointer_id()), touch, now) {
                return;
            }
            // Aiming Meteor Swarm: a click casts it at the circle and a
            // right click leaves the aim; a touch moves the circle, and a
            // quick tap casts it there.
            if page.runtime.demolition_targeting() {
                page.aim_meteor_swarm(on_canvas);
                if event.pointer_type() != "touch" {
                    match event.button() {
                        0 => {
                            page.runtime.demolition_confirm();
                        }
                        2 => {
                            page.runtime.demolition_cancel();
                        }
                        _ => {}
                    }
                    return;
                }
            }
            if event.is_primary() && event.button() == 0 {
                page.pressed_at = Some(event.time_stamp());
            }
            if event.pointer_type() == "touch" {
                let at = [event.client_x() as f32, event.client_y() as f32];
                page.input.touch_start(event.pointer_id(), at);
            } else {
                match event.button() {
                    0 => page.input.orbit = true,
                    2 => page.input.look = true,
                    _ => {}
                }
            }
        })?;
    }
    {
        let page = page.clone();
        on(&canvas, "pointermove", move |event: PointerEvent| {
            if super::terminal::pointer(&event, None) {
                event.prevent_default();
                return;
            }
            let mut page = page.borrow_mut();
            let id = u64::from(event.pointer_id() as u32);
            if page.trip.hud.captured(id) {
                page.trip
                    .hud
                    .moved(id, [event.offset_x() as f32, event.offset_y() as f32]);
                return;
            }
            // A card follows a resting mouse, not a drag that turns the view.
            if event.pointer_type() == "mouse" {
                page.hover = (event.buttons() == 0)
                    .then(|| [event.offset_x() as f32, event.offset_y() as f32]);
            }
            if event.pointer_type() != "touch" && page.runtime.demolition_targeting() {
                page.aim_meteor_swarm([event.offset_x() as f32, event.offset_y() as f32]);
            }
            let action = if event.pointer_type() == "touch" {
                let at = [event.client_x() as f32, event.client_y() as f32];
                page.input.touch_move(event.pointer_id(), at)
            } else {
                page.input
                    .drag(event.movement_x() as f32, event.movement_y() as f32)
            };
            page.apply(action);
        })?;
    }
    for name in ["pointerup", "pointercancel"] {
        let page = page.clone();
        on(&canvas, name, move |event: PointerEvent| {
            if super::terminal::pointer(&event, Some(false)) {
                event.prevent_default();
                return;
            }
            let mut page = page.borrow_mut();
            // A button on Lagrange's zone panel acts when it lifts.
            let id = u64::from(event.pointer_id() as u32);
            if page.trip.hud.captured(id) {
                let at = [event.offset_x() as f32, event.offset_y() as f32];
                let cancelled = event.type_() == "pointercancel";
                if let Some(intent) = page.trip.hud.up(id, at, cancelled) {
                    let _ = page.runtime.zone_intent(intent);
                }
                return;
            }
            page.release_climb(Some(event.pointer_id()), None);
            let now = event.time_stamp();
            page.lift_hotbar(event.pointer_id(), event.type_() == "pointercancel", now);
            if let Some(at) = page.pressed_at.take()
                && event.is_primary()
                && event.time_stamp() - at <= 300.0
                && (page.runtime.in_demolition() || page.runtime.demolition_targeting())
            {
                if page.runtime.demolition_targeting() {
                    page.aim_meteor_swarm([event.offset_x() as f32, event.offset_y() as f32]);
                    page.runtime.demolition_confirm();
                } else {
                    let _ = page.runtime.zone_intent(zones::Intent::Swing);
                }
            }
            if event.pointer_type() == "touch" {
                page.input.touch_end(event.pointer_id());
            } else {
                // `buttons` is what stays held after this release.
                page.input.orbit = event.buttons() & 1 != 0;
                page.input.look = event.buttons() & 2 != 0;
            }
        })?;
    }
    {
        let page = page.clone();
        on(&canvas, "wheel", move |event: WheelEvent| {
            if super::terminal::wheel(event.delta_y() as f32) {
                event.prevent_default();
                return;
            }
            event.prevent_default();
            // Positive lines move the camera closer; a wheel line is about
            // 100 pixels.
            let pixels = match event.delta_mode() {
                WheelEvent::DOM_DELTA_LINE => event.delta_y() * 100.0,
                WheelEvent::DOM_DELTA_PAGE => event.delta_y() * 800.0,
                _ => event.delta_y(),
            };
            let lines = (-pixels / 100.0).clamp(-10.0, 10.0) as f32;
            // Applied over the next few frames, so a wheel notch glides.
            page.borrow_mut().zoom.add(lines);
            journey(|j| j.zoomed("wheel"));
        })?;
    }
    {
        let page = page.clone();
        on(&canvas, "pointerleave", move |_: PointerEvent| {
            page.borrow_mut().hover = None;
        })?;
    }
    on(&canvas, "contextmenu", |event: web_sys::Event| {
        event.prevent_default()
    })?;
    Ok(())
}

/// Adds a listener that lives as long as the page. Wheel and touch
/// listeners are not passive, so they may prevent scrolling.
fn on<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    name: &str,
    handler: impl FnMut(E) + 'static,
) -> Result<(), String> {
    let mut handler = handler;
    let callback = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        if let Ok(event) = event.dyn_into::<E>() {
            handler(event);
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            name,
            callback.as_ref().unchecked_ref(),
            &options,
        )
        .map_err(|_| format!("cannot listen for {name}"))?;
    callback.forget();
    Ok(())
}

/// Shows `text` in the status element, creating it over the canvas when the
/// page has none.
pub(crate) fn status(text: &str) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(element) = status_element(&document) {
        element.set_text_content(Some(text));
        let _ = element.style().set_property("display", "block");
    }
}

fn progress(received: u64, total: u64) {
    let mb = |bytes: u64| bytes as f64 / 1_000_000.0;
    let percent = (received * 100).checked_div(total).unwrap_or(0);
    status(&format!(
        "Loading Everglade… {percent}% ({:.1} of {:.1} MB)",
        mb(received),
        mb(total)
    ));
}

pub(crate) fn hide_status(document: &Document) {
    if let Some(element) = document
        .get_element_by_id(STATUS_ID)
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    {
        let _ = element.style().set_property("display", "none");
    }
}

fn status_element(document: &Document) -> Option<HtmlElement> {
    if let Some(element) = document.get_element_by_id(STATUS_ID) {
        return element.dyn_into().ok();
    }
    let element: HtmlElement = document.create_element("div").ok()?.dyn_into().ok()?;
    element.set_id(STATUS_ID);
    let style = element.style();
    for (name, value) in [
        ("position", "fixed"),
        ("left", "16px"),
        ("bottom", "16px"),
        ("max-width", "calc(100vw - 32px)"),
        ("padding", "8px 12px"),
        ("border-radius", "6px"),
        ("font", "14px/1.4 \"Paper Mono\", monospace"),
        ("pointer-events", "none"),
    ] {
        let _ = style.set_property(name, value);
    }
    let background = coder_ui::coder_noir::SURFACE_SUBTLE;
    let _ = style.set_property(
        "background",
        &format!(
            "rgba({},{},{},0.8)",
            background >> 16,
            (background >> 8) & 255,
            background & 255
        ),
    );
    let _ = style.set_property("color", &format!("#{:06x}", coder_ui::coder_noir::CONTENT));
    document.body()?.append_child(&element).ok()?;
    Some(element)
}

/// Kiln's task while it works for this visitor at the terminal (#12456),
/// which the page's chat sets as `openagentsKilnScreen`.
fn kiln_screen() -> Option<String> {
    js_sys::Reflect::get(&js_sys::global(), &"openagentsKilnScreen".into())
        .ok()?
        .as_string()
        .filter(|task| !task.trim().is_empty())
}
