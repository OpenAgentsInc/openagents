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

/// The page's canvas.
const CANVAS_ID: &str = "everglade-canvas";
/// The element that shows progress and errors. Created when the page has
/// none.
const STATUS_ID: &str = "everglade-status";
/// The largest drawing-buffer side, in device pixels.
const MAX_SIDE: u32 = 4096;
/// The longest frame step; a hidden tab resumes without a jump.
const MAX_STEP: f32 = 0.1;

/// Starts Everglade when the module's `init` finishes.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
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
) -> Result<GridEngine, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    descriptor.backends = backends;
    let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("cannot draw on the canvas: {e}"))?;
    let gpu = Gpu::open_async(instance, &surface).await?;
    GridEngine::on_surface(gpu, surface, atlas, width, height)
}

/// What draws the page: the Grid through the engine renderer, Everglade and
/// the Grove through the legacy renderer until their own migration.
enum Draw {
    Legacy(Renderer),
    Grid(GridEngine),
}

impl Draw {
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        match self {
            Self::Legacy(renderer) => renderer.resize(width, height),
            Self::Grid(engine) => engine.resize(width, height),
        }
    }

    fn aspect(&self) -> f32 {
        match self {
            Self::Legacy(renderer) => renderer.aspect(),
            Self::Grid(engine) => engine.aspect(),
        }
    }

    fn size(&self) -> [f32; 2] {
        match self {
            Self::Legacy(renderer) => renderer.size(),
            Self::Grid(engine) => engine.size(),
        }
    }
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

/// Everything the frame loop owns.
struct Page {
    canvas: HtmlCanvasElement,
    runtime: WorldRuntime,
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
    /// With `?frames`, the frame times gathered for the console's
    /// once-a-second line.
    frames: Option<Frames>,
    water_dry: bool,
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
    // `?zone=grid`, or the `/grid` page, opens the shared Grid, the spawn
    // plaza, on the engine renderer with the pack built into this module;
    // nothing downloads.
    let grid = window.location().search().is_ok_and(|query| {
        query
            .trim_start_matches('?')
            .split('&')
            .any(|part| part.eq_ignore_ascii_case("zone=grid"))
    }) || window
        .location()
        .pathname()
        .is_ok_and(|path| path.trim_end_matches('/').ends_with("/grid"));
    if grid {
        return run_grid(window, document, canvas).await;
    }
    let bytes = download(&window).await?;
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
    if water {
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
    // it says why instead.
    let unavailable = renderer.physical_error().map(|error| {
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
        frames: query_has(&window, "frames").then(Frames::default),
        water_dry: query_has(&window, "frames") && query_has(&window, "water-dry"),
    }));
    listen(&window, &page)?;
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
    runtime.interact_hint = verse::runtime::InteractHint::None;
    let scale = (window.device_pixel_ratio() as f32).clamp(1.0, 3.0);
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
    let engine = open_grid(&canvas, backends, &atlas, width, height).await?;
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
    let presence = Presence::open(&window, &options).unwrap_or_else(|error| {
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
        frames: None,
        water_dry: false,
    }));
    listen(&window, &page)?;
    animate(window, page);
    Ok(())
}

/// Fetches the pinned pack from this origin with progress, bounded by its
/// pinned length. The runtime checks its length and digest on install.
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

/// Device-pixel drawing size for the canvas's laid-out CSS size.
fn drawing_size(canvas: &HtmlCanvasElement, scale: f32) -> (u32, u32) {
    let side = |css: i32| ((css.max(1) as f32 * scale).round() as u32).clamp(1, MAX_SIDE);
    (side(canvas.client_width()), side(canvas.client_height()))
}

impl Page {
    fn frame(&mut self, now: f64) {
        let gap = self.last.map_or(0.0, |last| now - last);
        let started = web_sys::window()
            .and_then(|w| w.performance())
            .map_or(now, |p| p.now());
        self.draw_frame(now);
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
        if self.runtime.zone_revision != self.rendered_revision {
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
                Draw::Grid(_) => {
                    self.fail(
                        "The browser Grid's arches lead nowhere yet: open Everglade from the \
                         page without ?zone, or the Grove with ?zone=grove",
                    );
                    return;
                }
            }
            self.rendered_revision = self.runtime.zone_revision;
        }
        if super::terminal::active() {
            self.input = super::input::Input::default();
        }
        let input = self.input.take();
        let dt = self
            .runtime
            .tick_with_mode(&input, dt, self.input.orbit, true);
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
            let dynamic = grid_frame::dynamic(&self.runtime, &peers, &[]);
            let lighting = grid_frame::lighting(&self.runtime.atmosphere());
            // Name tags, the connection line, and a player's card, laid
            // out in CSS pixels and drawn in device pixels.
            let mut ui = verse::ui::UiBatch::default();
            if let (Some(presence), Some(layout)) = (&self.presence, &self.layout) {
                presence.draw(&mut ui, layout, css, &self.runtime, view.view_proj);
            }
            if let Some(atlas) = &self.layout {
                super::terminal::draw(&mut ui, atlas, css);
            }
            for vertex in &mut ui.vertices {
                vertex.pos = vertex.pos.map(|v| v * self.scale);
            }
            match engine.draw(view, &dynamic, &ui, &lighting) {
                Ok(render_ms) => {
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
        let size = self.css_size();
        let aspect = self.renderer.aspect();
        let view_proj = self.runtime.view(aspect).view_proj;
        let line = self.layout.as_ref().map_or(14.0, |layout| layout.line);
        let Some(presence) = &mut self.presence else {
            return false;
        };
        presence.press(at, size, line, &self.runtime, view_proj)
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
            let _ = self.runtime.apply(action);
        }
    }

    fn fail(&mut self, error: &str) {
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
            let modifiers = [event.shift_key(), event.ctrl_key(), event.alt_key()];
            let used = page.hotbar_key(&event.code(), down, modifiers);
            if used || (!chord && page.input.key(&event.code(), down)) {
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
            page.borrow_mut().apply(Some(Action::Zoom { lines }));
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
fn status(text: &str) {
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

fn hide_status(document: &Document) {
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
