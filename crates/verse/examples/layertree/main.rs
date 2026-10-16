//! Replay a LayerTree event file in Verse or capture it without a window.

#[path = "mod.rs"]
mod layertree;

use layertree::{Set, scene};
use std::{path::PathBuf, sync::Arc, time::Instant};
use verse::imported::{Renderer, WindowPresenter};
use verse::ui::Atlas;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

#[derive(Clone)]
struct Options {
    events: PathBuf,
    at: Option<u64>,
    play: bool,
    compare_progress: bool,
    from: u64,
    to: Option<u64>,
    rate: f32,
    capture_every: Option<f32>,
    capture: Option<PathBuf>,
    size: [u32; 2],
    scale: f32,
}

fn options() -> Result<Options, String> {
    let mut out = Options {
        events: PathBuf::new(),
        at: None,
        play: false,
        compare_progress: false,
        from: 0,
        to: None,
        rate: 0.6,
        capture_every: None,
        capture: None,
        size: [1920, 1080],
        scale: 1.5,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value."));
        match arg.as_str() {
            "--events" => out.events = value()?.into(),
            "--at" => out.at = Some(value()?.parse().map_err(|_| "Choose an event number.")?),
            "--capture" => out.capture = Some(value()?.into()),
            "--scale" => {
                out.scale = value()?
                    .parse()
                    .map_err(|_| "Choose a text scale from 1 to 4.")?
            }
            "--size" => {
                let size = value()?;
                let (w, h) = size.split_once('x').ok_or("Use --size WIDTHxHEIGHT.")?;
                out.size = [
                    w.parse().map_err(|_| "Choose a pixel width.")?,
                    h.parse().map_err(|_| "Choose a pixel height.")?,
                ];
            }
            "--play" => out.play = true,
            "--compare-progress" => out.compare_progress = true,
            "--from" => out.from = value()?.parse().map_err(|_| "Choose a start event.")?,
            "--to" => out.to = Some(value()?.parse().map_err(|_| "Choose an end event.")?),
            "--rate" => out.rate = value()?.parse().map_err(|_| "Choose events per second.")?,
            "--capture-every" => {
                out.capture_every = Some(
                    value()?
                        .parse()
                        .map_err(|_| "Choose seconds between frames.")?,
                )
            }
            "--visual" => {}
            "--help" | "-h" => {
                println!(
                    "layertree --events FILE [--at EVENT] [--play --from N --to N --rate EVENTS_PER_SECOND] [--visual] [--capture FILE.png] [--capture-every SECONDS] [--size WIDTHxHEIGHT] [--scale 1..4]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown option: {arg}.")),
        }
    }
    if out.events.as_os_str().is_empty() {
        return Err("Choose an event file with --events FILE.".into());
    }
    if !out.scale.is_finite() || !(1.0..=4.0).contains(&out.scale) {
        return Err("Choose a text scale from 1 to 4.".into());
    }
    if out.size.iter().any(|n| !(64..=8192).contains(n)) {
        return Err("Choose image dimensions from 64 to 8192 pixels.".into());
    }
    if out
        .capture_every
        .is_some_and(|n| !n.is_finite() || n < 0.05)
    {
        return Err("Choose at least 0.05 seconds between frames.".into());
    }
    if out.capture_every.is_some() && (!out.play || out.capture.is_none()) {
        return Err("Frame capture needs --play and --capture DIRECTORY.".into());
    }
    Ok(out)
}

struct Fonts {
    raster: Atlas,
    layout: Atlas,
    scale: f32,
}
impl Fonts {
    fn new(scale: f32) -> Self {
        let raster = Atlas::new((14.0 * scale).round());
        let layout = raster
            .layout_at_scale(scale)
            .unwrap_or_else(|| Atlas::new(14.0));
        Self {
            raster,
            layout,
            scale,
        }
    }
}

struct App {
    set: Set,
    options: Options,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    presenter: Option<WindowPresenter>,
    fonts: Fonts,
    started: Instant,
    yaw: f32,
    zoom: f32,
    cursor: Option<[f32; 2]>,
    error: Option<String>,
}

impl App {
    fn open(&mut self, scale: f32) -> Result<(), String> {
        let window = self.window.clone().ok_or("The window is closed.")?;
        let size = window.inner_size();
        self.fonts = Fonts::new(scale);
        self.renderer = None;
        self.presenter = None;
        let renderer = scene::renderer(
            [size.width.max(64), size.height.max(64)],
            &self.fonts.raster,
        )?;
        self.presenter = Some(renderer.attach_window(window)?);
        self.renderer = Some(renderer);
        Ok(())
    }

    fn draw(&mut self) -> Result<(), String> {
        let window = self.window.as_ref().ok_or("The window is closed.")?;
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        self.set
            .advance(self.started.elapsed().as_secs_f32() - self.set.time);
        let renderer = self.renderer.as_mut().ok_or("The scene has not opened.")?;
        if renderer.recover_if_lost(&self.fonts.raster)? {
            self.presenter = Some(renderer.attach_window(window.clone())?);
        }
        renderer.resize(size.width, size.height)?;
        let view = scene::camera(
            &self.set,
            size.width as f32 / size.height as f32,
            self.yaw,
            self.zoom,
        );
        let overlay = [
            size.width as f32 / self.fonts.scale,
            size.height as f32 / self.fonts.scale,
        ];
        let ui = scene::overlay(
            &self.set,
            view,
            overlay,
            &self.fonts.layout,
            self.cursor,
            true,
        );
        renderer.set_overlay_size(overlay[0], overlay[1]);
        renderer.draw_live(view, &scene::frame(&self.set), &ui, &scene::lighting())?;
        renderer.present_window(
            self.presenter.as_mut().ok_or("The window is closed.")?,
            [size.width, size.height],
        )
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let opened = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Verse - LayerTree")
                            .with_inner_size(winit::dpi::PhysicalSize::new(
                                self.options.size[0],
                                self.options.size[1],
                            )),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let scale = window.scale_factor() as f32;
            self.window = Some(window);
            self.open(scale)
        })();
        if let Err(error) = opened {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Err(error) = self.open(scale_factor as f32) {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some([
                    position.x as f32 / self.fonts.scale,
                    position.y as f32 / self.fonts.scale,
                ])
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    match key {
                        KeyCode::Escape => event_loop.exit(),
                        KeyCode::ArrowLeft => self.yaw -= 0.08,
                        KeyCode::ArrowRight => self.yaw += 0.08,
                        KeyCode::ArrowUp => self.zoom = (self.zoom * 0.9).max(0.25),
                        KeyCode::ArrowDown => self.zoom = (self.zoom * 1.1).min(4.0),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn run() -> Result<(), String> {
    let options = options()?;
    let mut set = Set::load(&options.events, options.at)?;
    set.compare_progress = options.compare_progress;
    if options.play {
        set.play(options.from, options.to, options.rate)?;
    }
    if let Some(path) = &options.capture {
        let fonts = Fonts::new(options.scale);
        let mut renderer = scene::renderer(options.size, &fonts.raster)?;
        let duration = set.duration();
        let step = options.capture_every.unwrap_or(duration.max(1.0));
        let frames = if options.capture_every.is_some() {
            (duration / step).ceil() as usize + 1
        } else {
            1
        };
        for frame in 0..frames {
            if frame > 0 {
                set.advance(step);
            } else if options.play && options.capture_every.is_none() {
                set.advance(duration);
            }
            let view = scene::camera(
                &set,
                options.size[0] as f32 / options.size[1] as f32,
                0.18,
                1.0,
            );
            let size = options.size.map(|v| v as f32 / fonts.scale);
            renderer.set_overlay_size(size[0], size[1]);
            let ui = scene::overlay(&set, view, size, &fonts.layout, None, false);
            let pixels = renderer.draw(view, &scene::frame(&set), &ui, &scene::lighting())?;
            let output = if options.capture_every.is_some() {
                path.join(format!("{frame:05}.png"))
            } else {
                path.clone()
            };
            scene::write_png(&output, options.size, &pixels)?;
        }
        println!("Wrote {}", path.display());
        return Ok(());
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        set,
        options,
        window: None,
        renderer: None,
        presenter: None,
        fonts: Fonts::new(1.0),
        started: Instant::now(),
        yaw: 0.18,
        zoom: 1.0,
        cursor: None,
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
