//! Native interactive chamber; the action bar unlocks at the cinematic handoff.
use glam::Vec3;
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Instant};
use verse::{
    imported::{
        Renderer, WindowPresenter, chamber,
        lighting::Light,
        overlay,
        play::{Ability, Game},
    },
    render::View,
    ui::Atlas,
};
use verse_wow::{assets::Pack, director::Scene, position_from_wow};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
struct App {
    window: Option<Arc<Window>>,
    presenter: Option<WindowPresenter>,
    renderer: Option<Renderer>,
    pack: Pack,
    atlas: Atlas,
    game: Game,
    last: Instant,
    keys: HashSet<KeyCode>,
    cursor: [f32; 2],
    right: bool,
    heights: std::collections::BTreeMap<String, f32>,
    dir: PathBuf,
    proof: Option<PathBuf>,
}
impl App {
    fn activate(&mut self, ability: Ability) {
        if let Err(e) = self.game.activate(ability) {
            self.game.message = e;
        }
    }
    fn render(&mut self) -> Result<Vec<u8>, String> {
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        let key = |k| if self.keys.contains(&k) { 1.0 } else { 0.0 };
        self.game.tick(
            dt,
            [
                key(KeyCode::KeyD) - key(KeyCode::KeyA),
                key(KeyCode::KeyW) - key(KeyCode::KeyS),
            ],
        )?;
        self.draw_frame()
    }
    fn draw_frame(&mut self) -> Result<Vec<u8>, String> {
        let frame = self.game.frame();
        let view = View {
            view_proj: frame.view_projection(1280.0 / 720.0),
            eye: frame.eye,
        };
        let mut ui = overlay::cinematic(
            &self.atlas,
            &frame,
            &self.heights,
            view.view_proj,
            1280.0,
            720.0,
        );
        let hover = overlay::action_at(self.cursor[0], self.cursor[1], 1280.0, 720.0);
        overlay::action_bar(&mut ui, &self.atlas, &self.game, 1280.0, 720.0, hover);
        let mut actors = chamber::instances(&self.pack, &frame);
        actors.extend(chamber::spell_instances(&self.game));
        let mut lighting = chamber::lighting(position_from_wow(self.game.scene.origin_wow));
        lighting.time = self.game.time;
        for p in self.game.snapshot().projectiles.iter().take(2) {
            lighting.lights.push(Light {
                position: p.pos.into(),
                color: if p.kind == verse_ruins::Spell::MagicMissile {
                    Vec3::new(0.3, 0.15, 1.0)
                } else {
                    Vec3::new(1.0, 0.22, 0.03)
                },
                intensity: 12.0,
                range: 7.0,
            });
        }
        self.renderer
            .as_mut()
            .unwrap()
            .draw(view, &actors, &ui, &lighting)
    }
    fn select(&mut self) {
        let frame = self.game.frame();
        let vp = frame.view_projection(1280.0 / 720.0);
        let mut closest = None;
        for a in frame
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && a.health > 0)
        {
            let clip = vp * (a.actor.position + Vec3::Y * 1.2).extend(1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            let dx = (ndc.x + 1.0) * 640.0 - self.cursor[0];
            let dy = (1.0 - ndc.y) * 360.0 - self.cursor[1];
            let distance = dx * dx + dy * dy;
            if distance < 40.0 * 40.0 && closest.is_none_or(|(_, d)| distance < d) {
                closest = Some((a.actor.id, distance));
            }
        }
        if let Some((id, _)) = closest {
            self.game.selected = id;
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("The Verse — Scholomance")
                            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let renderer = Renderer::new(
                self.pack.clone(),
                &self.dir,
                1280,
                720,
                &self.atlas,
                &chamber::static_instances(
                    &self.pack,
                    position_from_wow(self.game.scene.origin_wow),
                ),
            )?;
            let presenter = renderer.attach_window(window.clone())?;
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.presenter = Some(presenter);
            self.last = Instant::now();
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.right = false;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        self.keys.insert(key);
                        if !event.repeat {
                            match key {
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::Tab => self.game.cycle_target(),
                                KeyCode::Digit1 => self.activate(Ability::Bow),
                                KeyCode::Digit2 => self.activate(Ability::FireBolt),
                                KeyCode::Digit3 => self.activate(Ability::MagicMissile),
                                KeyCode::Digit4 => self.activate(Ability::Fireball),
                                _ => {}
                            }
                        }
                    } else {
                        self.keys.remove(&key);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.window.as_ref().unwrap().inner_size();
                let next = [
                    position.x as f32 / size.width.max(1) as f32 * 1280.0,
                    position.y as f32 / size.height.max(1) as f32 * 720.0,
                ];
                if self.right && self.game.unlocked() {
                    self.game.yaw -= (next[0] - self.cursor[0]) * 0.006;
                }
                self.cursor = next;
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Right => self.right = state == ElementState::Pressed,
                MouseButton::Left if state == ElementState::Pressed => {
                    if let Some(ability) =
                        overlay::action_at(self.cursor[0], self.cursor[1], 1280.0, 720.0)
                    {
                        self.activate(ability);
                    } else if self.game.unlocked() {
                        self.select();
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                match self.render() {
                    Ok(pixels) => {
                        let size = self.window.as_ref().unwrap().inner_size();
                        if let Err(e) = self.renderer.as_ref().unwrap().present_window(
                            self.presenter.as_mut().unwrap(),
                            [size.width, size.height],
                        ) {
                            eprintln!("{e}");
                            event_loop.exit();
                        }
                        if let Some(path) = self.proof.take() {
                            if let Err(e) = save_png(&path, &pixels) {
                                eprintln!("{e}");
                            }
                            event_loop.exit();
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
fn save_png(path: &std::path::Path, pixels: &[u8]) -> Result<(), String> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).map_err(|e| e.to_string())?,
        1280,
        720,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().ok_or("Expected private pack.json")?);
    let dir = path
        .parent()
        .ok_or("Expected pack directory")?
        .to_path_buf();
    let mut pack = Pack::read(&path)?;
    chamber::add_effect_models(&mut pack, &dir)?;
    let scene = Scene::from_json(include_bytes!("../../../assets/verse/wow/anthropic.json"))?;
    let game = Game::new(scene)?;
    let heights = pack
        .models
        .iter()
        .map(|(id, m)| (id.clone(), m.height))
        .collect();
    let atlas = chamber::classic_atlas(&dir)?;
    let mut app = App {
        window: None,
        presenter: None,
        renderer: None,
        pack,
        atlas,
        game,
        last: Instant::now(),
        keys: HashSet::new(),
        cursor: [0.0; 2],
        right: false,
        heights,
        dir,
        proof: None,
    };
    let mode = args.next();
    if mode.as_deref() == Some("--demo") {
        return demo(
            &mut app,
            PathBuf::from(args.next().ok_or("Expected demo.mp4")?),
        );
    }
    if mode.as_deref() == Some("--proof") {
        app.proof = Some(PathBuf::from(args.next().ok_or("Expected proof.png")?));
        for _ in 0..215 {
            app.game.tick(0.1, [0.0, 0.0])?;
        }
        app.game.activate(Ability::Fireball)?;
        for _ in 0..12 {
            app.game.tick(0.1, [0.0, 0.0])?;
        }
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

fn demo(app: &mut App, output: PathBuf) -> Result<(), String> {
    use std::io::Write;
    for _ in 0..180 {
        app.game.tick(0.1, [0.0, 0.0])?;
    }
    app.renderer = Some(Renderer::new(
        app.pack.clone(),
        &app.dir,
        1280,
        720,
        &app.atlas,
        &chamber::static_instances(&app.pack, position_from_wow(app.game.scene.origin_wow)),
    )?);
    let mut encoder = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            "1280x720",
            "-framerate",
            "30",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(output)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut pipe = encoder.stdin.take().ok_or("Missing encoder input")?;
    for frame in 0..480 {
        app.game.tick(1.0 / 30.0, [0.0, 0.0])?;
        for (at, ability) in [
            (90, Ability::Bow),
            (150, Ability::FireBolt),
            (225, Ability::MagicMissile),
            (330, Ability::Fireball),
        ] {
            if frame == at {
                app.game.activate(ability)?;
                eprintln!("Activated {:?} at {}", ability, app.game.time);
            }
        }
        let pixels = app.draw_frame()?;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        if frame % 120 == 0 {
            eprintln!("Rendered {} / 16 seconds", frame as f32 / 30.0);
        }
    }
    drop(pipe);
    if !encoder.wait().map_err(|e| e.to_string())?.success() {
        return Err("Demo encoder failed".into());
    }
    let snapshot = app.game.snapshot();
    eprintln!(
        "Retained combat: {} spell casts, {} projectiles, {} impacts, {} mana",
        snapshot.counters.casts,
        snapshot.counters.projectiles,
        snapshot.counters.hits,
        snapshot.player.mana
    );
    Ok(())
}
