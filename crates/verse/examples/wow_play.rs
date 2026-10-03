//! Native interactive chamber; the action bar unlocks at the cinematic handoff.
use glam::Vec3;
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Instant};
use verse::{
    imported::{
        Renderer, WindowPresenter, chamber,
        controls::{ClassicControls, Held},
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
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
struct App {
    window: Option<Arc<Window>>,
    presenter: Option<WindowPresenter>,
    renderer: Option<Renderer>,
    pack: Pack,
    atlas: Atlas,
    game: Game,
    last: Instant,
    agent_time: f32,
    keys: HashSet<KeyCode>,
    cursor: [f32; 2],
    controls: ClassicControls,
    pointer: [f64; 2],
    captured: bool,
    raw_pointer: bool,
    pending_select: bool,
    dragged: f64,
    heights: std::collections::BTreeMap<String, f32>,
    dir: PathBuf,
    proof: Option<PathBuf>,
}
impl App {
    fn activate(&mut self, ability: Ability) {
        if self.game.agent_controlled {
            return;
        }
        if let Err(e) = self.game.activate(ability) {
            self.game.message = e;
        }
    }
    fn render(&mut self) -> Result<Vec<u8>, String> {
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        let key = |k| self.keys.contains(&k);
        let held = Held {
            forward: key(KeyCode::KeyW) || key(KeyCode::ArrowUp),
            backward: key(KeyCode::KeyS) || key(KeyCode::ArrowDown),
            turn_left: key(KeyCode::KeyA) || key(KeyCode::ArrowLeft),
            turn_right: key(KeyCode::KeyD) || key(KeyCode::ArrowRight),
            strafe_left: key(KeyCode::KeyQ),
            strafe_right: key(KeyCode::KeyE),
        };
        let movement = if self.game.unlocked() && !self.game.agent_controlled {
            self.controls
                .step(held, dt, &mut self.game.yaw, &mut self.game.camera)
        } else {
            [0.0; 2]
        };
        if self.game.agent_controlled {
            self.agent_time += dt;
            while self.agent_time >= 1.0 / 30.0 {
                self.game.tick(1.0 / 30.0, [0.0; 2])?;
                self.agent_time -= 1.0 / 30.0;
            }
        } else {
            self.agent_time = 0.0;
            self.game.tick(dt, movement)?;
        }
        self.draw_frame()
    }
    fn capture_pointer(&mut self) {
        let wanted = self.controls.looking();
        if wanted == self.captured {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        self.captured = wanted;
        if wanted {
            self.raw_pointer = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
            window.set_cursor_visible(false);
            if !self.raw_pointer {
                let size = window.inner_size();
                let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                    size.width as f64 * 0.5,
                    size.height as f64 * 0.5,
                ));
            }
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                self.pointer[0],
                self.pointer[1],
            ));
        }
    }
    fn mouse_motion(&mut self, delta: [f64; 2]) {
        if !self.game.unlocked() {
            return;
        }
        self.dragged += delta[0].abs() + delta[1].abs();
        self.controls
            .motion(delta, &mut self.game.yaw, &mut self.game.camera);
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
        if let Some(position) = self.game.controls.light {
            lighting.lights.push(Light {
                position,
                color: Vec3::new(1.0, 0.85, 0.55),
                intensity: 35.0,
                range: 12.192,
            });
        }
        for p in
            self.game
                .snapshot()
                .projectiles
                .iter()
                .take(if self.game.controls.light.is_some() {
                    1
                } else {
                    2
                })
        {
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
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.captured && self.raw_pointer {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.mouse_motion([delta.0, delta.1]);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.controls.clear();
                self.pending_select = false;
                self.capture_pointer();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        self.keys.insert(key);
                        if !event.repeat {
                            match key {
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::F1 | KeyCode::F2 => {
                                    self.game =
                                        Game::combat(self.game.scene.clone(), key == KeyCode::F2)
                                            .expect("The loaded chamber admits combat");
                                    self.game.time = self.game.scene.cut_at
                                        - if key == KeyCode::F2 { 3.0 } else { 0.0 };
                                    self.agent_time = 0.0;
                                    self.controls.clear();
                                    self.keys.clear();
                                    self.pending_select = false;
                                    self.capture_pointer();
                                }
                                KeyCode::NumLock => {
                                    if self.game.unlocked() {
                                        self.controls.autorun = !self.controls.autorun;
                                    }
                                }
                                KeyCode::Tab if !self.game.agent_controlled => {
                                    self.game.cycle_target()
                                }
                                KeyCode::Digit1 => self.activate(Ability::Bow),
                                KeyCode::Digit2 => self.activate(Ability::FireBolt),
                                KeyCode::Digit3 => self.activate(Ability::MagicMissile),
                                KeyCode::Digit4 => self.activate(Ability::Fireball),
                                KeyCode::Digit5 => self.activate(Ability::MistyStep),
                                KeyCode::Digit6 => self.activate(Ability::Thunderwave),
                                KeyCode::Digit7 => self.activate(Ability::Web),
                                KeyCode::Digit8 => self.activate(Ability::Grease),
                                KeyCode::Digit9 => self.activate(Ability::Light),
                                KeyCode::Digit0 => self.activate(Ability::Shield),
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
                if self.captured {
                    if !self.raw_pointer {
                        let center = [size.width as f64 * 0.5, size.height as f64 * 0.5];
                        let delta = [position.x - center[0], position.y - center[1]];
                        if delta[0].abs() + delta[1].abs() > 0.5 {
                            self.mouse_motion(delta);
                            let _ = self.window.as_ref().unwrap().set_cursor_position(
                                winit::dpi::PhysicalPosition::new(center[0], center[1]),
                            );
                        }
                    }
                } else {
                    self.pointer = [position.x, position.y];
                    self.cursor = [
                        position.x as f32 / size.width.max(1) as f32 * 1280.0,
                        position.y as f32 / size.height.max(1) as f32 * 720.0,
                    ];
                }
            }
            WindowEvent::MouseWheel { delta, .. }
                if self.game.unlocked() && !self.game.agent_controlled =>
            {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.game.camera.zoom(steps);
            }
            WindowEvent::MouseInput { state, button, .. }
                if self.game.unlocked() && !self.game.agent_controlled =>
            {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Right => {
                        if down {
                            self.pending_select = false;
                        }
                        self.controls
                            .button(true, down, &mut self.game.yaw, &self.game.camera);
                    }
                    MouseButton::Left => {
                        if self.game.agent_controlled {
                            return;
                        }
                        if down && !self.controls.looking() {
                            if let Some(ability) =
                                overlay::action_at(self.cursor[0], self.cursor[1], 1280.0, 720.0)
                            {
                                self.activate(ability);
                                return;
                            }
                            if overlay::chrome_at(self.cursor[0], self.cursor[1], 1280.0, 720.0) {
                                return;
                            }
                            self.pending_select = true;
                            self.dragged = 0.0;
                        }
                        if !down && self.pending_select {
                            if self.dragged < 4.0 {
                                self.select();
                            }
                            self.pending_select = false;
                        }
                        self.controls
                            .button(false, down, &mut self.game.yaw, &self.game.camera);
                    }
                    MouseButton::Back if down => self.controls.autorun = !self.controls.autorun,
                    _ => {}
                }
                self.capture_pointer();
            }
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
    let atlas = chamber::portrait_atlas(&dir, &pack)?;
    let mut app = App {
        window: None,
        presenter: None,
        renderer: None,
        pack,
        atlas,
        game,
        last: Instant::now(),
        agent_time: 0.0,
        keys: HashSet::new(),
        cursor: [0.0; 2],
        controls: ClassicControls::default(),
        pointer: [0.0; 2],
        captured: false,
        raw_pointer: false,
        pending_select: false,
        dragged: 0.0,
        heights,
        dir,
        proof: None,
    };
    let mode = args.next();
    if matches!(
        mode.as_deref(),
        Some("--demo" | "--utility-demo" | "--combat-demo")
    ) {
        return demo(
            &mut app,
            PathBuf::from(args.next().ok_or("Expected demo.mp4")?),
            mode.as_deref() == Some("--utility-demo"),
            mode.as_deref() == Some("--combat-demo"),
        );
    }
    if matches!(mode.as_deref(), Some("--agent" | "--combat")) {
        app.game = Game::combat(app.game.scene.clone(), mode.as_deref() == Some("--agent"))?;
        app.game.time = app.game.scene.cut_at - 3.0;
    }
    if mode.as_deref() == Some("--combat-proof") {
        app.proof = Some(PathBuf::from(args.next().ok_or("Expected combat.png")?));
        let time: f32 = args
            .next()
            .ok_or("Expected encounter time")?
            .parse()
            .map_err(|_| "Invalid encounter time")?;
        if !time.is_finite() || !(20.0..=120.0).contains(&time) {
            return Err("Encounter time must be between 20 and 120 seconds".into());
        }
        app.game = Game::combat(app.game.scene.clone(), true)?;
        for _ in 0..180 {
            app.game.tick(0.1, [0.0; 2])?;
        }
        while app.game.time < time {
            app.game.tick(1.0 / 30.0, [0.0; 2])?;
        }
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

fn demo(app: &mut App, output: PathBuf, utility: bool, combat: bool) -> Result<(), String> {
    use std::io::Write;
    if combat {
        app.game = Game::combat(app.game.scene.clone(), true)?;
    }
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
        .arg(&output)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut pipe = encoder.stdin.take().ok_or("Missing encoder input")?;
    for frame in 0..if combat { 3600 } else { 480 } {
        app.game.tick(1.0 / 30.0, [0.0, 0.0])?;
        let sequence = if utility {
            [
                (90, Ability::Light),
                (150, Ability::MistyStep),
                (225, Ability::Web),
                (300, Ability::Grease),
                (390, Ability::Thunderwave),
            ]
        } else {
            [
                (90, Ability::Bow),
                (150, Ability::FireBolt),
                (225, Ability::MagicMissile),
                (330, Ability::Fireball),
                (999, Ability::Bow),
            ]
        };
        for (at, ability) in sequence {
            if !combat && frame == at {
                if ability == Ability::Thunderwave {
                    let target = app
                        .game
                        .frame()
                        .actors
                        .into_iter()
                        .find(|a| a.actor.id == app.game.selected)
                        .ok_or("Missing target")?;
                    app.game.player = target.actor.position - glam::Vec3::Z * 3.0;
                    app.game.yaw = std::f32::consts::PI;
                    app.game.camera.yaw = app.game.yaw;
                }
                app.game.activate(ability)?;
                eprintln!("Activated {:?} at {}", ability, app.game.time);
            }
        }
        let pixels = app.draw_frame()?;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        if frame % 300 == 0 {
            eprintln!(
                "Rendered {} seconds; player {} HP",
                frame as f32 / 30.0,
                app.game.snapshot().player.hp
            );
        }
        if combat {
            let encounter = app.game.encounter.as_ref().unwrap();
            if frame % 300 == 0
                || (app.game.controls.shield > 0
                    && app.game.time < app.game.controls.shield_until
                    && frame % 120 == 0)
            {
                save_png(&output.with_extension("png"), &pixels)?;
            }
            if encounter.ended.is_some_and(|at| app.game.time - at >= 5.0) {
                save_png(&output.with_extension("png"), &pixels)?;
                let evidence = serde_json::json!({"schema":"openagents.wow.agent-combat.v1","controller":"local observation-driven tactical controller","control_mode":"agent","time":app.game.time,"ended_at":encounter.ended,"player":app.game.snapshot().player,"boss_remaining":encounter.boss_remaining,"boss_max":encounter.boss_max,"cultists_defeated":encounter.kills,"damage_taken":encounter.damage,"shield_absorbed":encounter.absorbed,"dodged":encounter.dodged,"enemy_casts":encounter.enemy_casts,"ability_uses":encounter.used,"renderer":"owned native GPU pipeline; no grading; no chat-input automation"});
                std::fs::write(
                    output.with_extension("json"),
                    serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if app.game.snapshot().player.hp != 0
                    || encounter.boss_remaining == 0
                    || encounter.boss_remaining * 5 >= encounter.boss_max
                {
                    return Err("Combat recording did not reach the expected close defeat".into());
                }
                break;
            }
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
