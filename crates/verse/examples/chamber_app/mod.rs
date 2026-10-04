// Native interactive chamber; the action bar unlocks at the cinematic handoff.
mod audio;
mod profile;
mod reload;
use glam::Vec3;
use std::{collections::HashSet, io::Write, path::PathBuf, sync::Arc, time::Instant};
use verse::{
    imported::{
        Renderer, WindowPresenter, chamber,
        controls::{ClassicControls, Held},
        overlay,
        play::{Ability, Game},
    },
    render::View,
    ui::Atlas,
};
use verse_engine::{assets::Pack, director::Scene, source_position as position_from_wow};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
struct App {
    audio: Option<audio::Audio>,
    audio_error: String,
    window: Option<Arc<Window>>,
    presenter: Option<WindowPresenter>,
    renderer: Option<Renderer>,
    pack: Arc<Pack>,
    atlas: Arc<Atlas>,
    reload_path: Option<PathBuf>,
    reload_contract: Arc<reload::Contract>,
    reload_pending: Option<reload::Pending>,
    reload_status: String,
    reload_window_proof: Option<reload::WindowProof>,
    game: Game,
    last: Instant,
    first_redraw: bool,
    schedule: verse_engine::core::FixedSchedule,
    interpolation: f32,
    capture_view: Option<(Vec3, Vec3, f32)>,
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
    profile: Option<std::io::BufWriter<std::fs::File>>,
    world_ms: f64,
    frame_interval_ms: f64,
    stress: Option<Stress>,
}
struct Stress {
    duration: f64,
    started: Option<Instant>,
    next_cast: f32,
    next_shield: Option<f32>,
    casts: u64,
    move_forward: bool,
}
impl App {
    fn start_reload(&mut self) -> Result<(), String> {
        if self.reload_pending.is_some() {
            return Err("Asset reload is already running".into());
        }
        let path = self
            .reload_path
            .clone()
            .ok_or("Asset reload requires an original scene")?;
        let source = self
            .renderer
            .as_ref()
            .ok_or("Renderer is not ready")?
            .reload_source();
        self.reload_pending = Some(reload::start(
            path,
            source,
            self.atlas.clone(),
            self.reload_contract.clone(),
            chamber::static_instances(&self.pack, position_from_wow(self.game.scene.origin_wow)),
        )?);
        self.reload_status = "Preparing renderer assets".into();
        Ok(())
    }
    fn poll_reload(&mut self) {
        let Some(pending) = &self.reload_pending else {
            return;
        };
        let result = match pending.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("Asset reload worker stopped".into())
            }
        };
        self.reload_pending = None;
        match result {
            Ok(ready) => {
                let before = self
                    .reload_window_proof
                    .as_ref()
                    .map(|_| self.game.checkpoint());
                let started = Instant::now();
                match self
                    .renderer
                    .as_mut()
                    .unwrap()
                    .commit_reload(ready.candidate)
                {
                    Ok(retired) => {
                        let commit_ms = started.elapsed().as_secs_f64() * 1000.;
                        self.pack = ready.pack;
                        self.heights = ready.heights;
                        if let Some(proof) = &mut self.reload_window_proof {
                            let unchanged = before
                                .and_then(Result::ok)
                                .is_some_and(|b| self.game.checkpoint().is_ok_and(|a| a == b));
                            proof.commit = Some(
                                serde_json::json!({"prepare_ms":ready.prepare_ms, "commit_ms":commit_ms, "world_checkpoint_unchanged":unchanged}),
                            );
                        }
                        // Resource disposal can be large; keep it outside the event thread too.
                        std::thread::spawn(move || drop(retired));
                        self.reload_status = format!(
                            "Renderer assets reloaded ({:.0} ms preparation)",
                            ready.prepare_ms
                        );
                    }
                    Err(error) => self.reload_status = error,
                }
            }
            Err(error) => self.reload_status = error,
        }
    }
    fn activate(&mut self, ability: Ability) {
        if self.game.agent_controlled {
            return;
        }
        if let Err(e) = self.game.activate(ability) {
            self.game.message = e;
        }
    }
    fn render(&mut self) -> Result<Vec<u8>, String> {
        let started = Instant::now();
        let elapsed = self.last.elapsed().as_secs_f64();
        self.last = Instant::now();
        self.frame_interval_ms = if self.presenter.is_some() {
            elapsed * 1000.
        } else {
            0.
        };
        if let Some(stress) = &mut self.stress {
            self.keys.clear();
            if stress
                .next_shield
                .is_some_and(|time| self.game.time >= time)
                && self.game.casting.is_none()
                && self.game.activate(Ability::Shield).is_ok()
            {
                stress.next_shield = Some(self.game.time + 15.);
            }
            if self.game.casting.is_none() && self.game.time >= stress.next_cast {
                self.game.selected = 1;
                if self.game.activate(Ability::Fireball).is_ok() {
                    stress.casts += 1;
                }
                stress.next_cast = self.game.time + 3.;
            }
            if self.game.player.z > -16. {
                stress.move_forward = false;
            }
            if self.game.player.z < -24. {
                stress.move_forward = true;
            }
            if self.game.casting.is_none() {
                self.keys.insert(if stress.move_forward {
                    KeyCode::KeyW
                } else {
                    KeyCode::KeyS
                });
                if self.game.player.x > 2. {
                    self.keys.insert(KeyCode::KeyE);
                }
                if self.game.player.x < -2. {
                    self.keys.insert(KeyCode::KeyQ);
                }
            }
            let age = stress
                .started
                .map_or((self.game.time - self.game.scene.cut_at) as f64, |at| {
                    at.elapsed().as_secs_f64()
                });
            let look = age % 15. < 1. && self.game.casting.is_none();
            self.controls
                .button(true, look, &mut self.game.yaw, &self.game.camera);
            if look {
                self.controls.motion(
                    [
                        -elapsed * 24. * if (age / 15.) as u64 % 2 == 0 { 1. } else { -1. },
                        elapsed * 10. * age.sin(),
                    ],
                    &mut self.game.yaw,
                    &mut self.game.camera,
                );
            }
        }
        let key = |k| self.keys.contains(&k);
        let held = Held {
            forward: key(KeyCode::KeyW) || key(KeyCode::ArrowUp),
            backward: key(KeyCode::KeyS) || key(KeyCode::ArrowDown),
            turn_left: key(KeyCode::KeyA) || key(KeyCode::ArrowLeft),
            turn_right: key(KeyCode::KeyD) || key(KeyCode::ArrowRight),
            strafe_left: key(KeyCode::KeyQ),
            strafe_right: key(KeyCode::KeyE),
        };
        let batch = self.schedule.advance(elapsed)?;
        self.interpolation = batch.interpolation;
        for _ in 0..batch.steps {
            let movement = if self.game.unlocked() && !self.game.agent_controlled {
                self.controls.step(
                    held,
                    batch.seconds,
                    &mut self.game.yaw,
                    &mut self.game.camera,
                )
            } else {
                [0.0; 2]
            };
            self.game.tick(batch.seconds, movement)?;
        }
        if self.game.snapshot().player.hp == 0 {
            self.controls = Default::default();
            self.capture_pointer();
        }
        self.world_ms = started.elapsed().as_secs_f64() * 1000.;
        self.draw_frame()
    }
    fn capture_pointer(&mut self) {
        let wanted = self.game.snapshot().player.hp > 0 && self.controls.looking();
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
    fn capture_frame(&mut self) -> Result<Vec<u8>, String> {
        let proof = self.proof.take();
        self.proof = Some(PathBuf::new());
        let result = self.draw_frame();
        self.proof = proof;
        result
    }
    fn overlay_size(&self) -> [f32; 2] {
        let size = self
            .window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                [s.width, s.height]
            })
            .unwrap_or([1280, 720]);
        [720. * size[0].max(1) as f32 / size[1].max(1) as f32, 720.]
    }
    fn draw_frame(&mut self) -> Result<Vec<u8>, String> {
        let started = Instant::now();
        let [width, height] = self.overlay_size();
        if self.presenter.is_some() {
            let size = self.window.as_ref().unwrap().inner_size();
            self.renderer
                .as_mut()
                .unwrap()
                .resize(size.width, size.height)?;
        }
        let dimensions = self.renderer.as_ref().unwrap().dimensions();
        let mut frame = self.game.interpolated_frame(self.interpolation)?;
        if self.game.scene.collision_profile.as_deref() == Some("original-chamber-v1")
            && frame
                .actors
                .iter()
                .any(|a| matches!(a.animation, verse_engine::motion::Selection::Legacy(_)))
        {
            return Err("Original scene selected a legacy animation".into());
        }
        if let Some((eye, target, fov)) = self.capture_view {
            frame.eye = eye;
            frame.target = target;
            frame.fov = fov;
        }
        let view = View {
            view_proj: frame.view_projection(dimensions[0] as f32 / dimensions[1] as f32),
            eye: frame.eye,
        };
        let mut ui = overlay::cinematic(
            &self.atlas,
            &frame,
            &self.heights,
            view.view_proj,
            width,
            height,
        );
        overlay::damage_numbers(
            &mut ui,
            &self.atlas,
            &self.game,
            &frame,
            &self.heights,
            view.view_proj,
            width,
            height,
        );
        let hover = overlay::action_at(self.cursor[0], self.cursor[1], width, height);
        overlay::action_bar(&mut ui, &self.atlas, &self.game, width, height, hover);
        if !self.reload_status.is_empty() {
            ui.text(
                &self.atlas,
                16.,
                126.,
                &self.reload_status,
                [1., 0.8, 0.45, 1.],
            );
        }
        let mut actors = chamber::instances(&self.pack, &frame)?;
        let combat_visuals = verse_world::visuals::Combat::extract(&self.game);
        actors.extend(chamber::spell_instances_from_visuals(&combat_visuals));
        actors.extend(chamber::blocker_instances(&self.pack, &self.game));
        actors.extend(chamber::prop_instances(
            &self.pack,
            &self.game,
            self.interpolation,
        ));
        let lighting = chamber::lighting_from_visuals(
            &combat_visuals,
            position_from_wow(self.game.scene.origin_wow),
            self.game.scene.collision_profile.as_deref() == Some(verse_world::playground::PROFILE),
            self.game.player,
        );
        let renderer = self.renderer.as_mut().unwrap();
        renderer.set_overlay_size(width, height);
        let pixels = if self.presenter.is_none() || self.proof.is_some() {
            renderer.draw(view, &actors, &ui, &lighting)?
        } else {
            renderer.draw_live(view, &actors, &ui, &lighting)?;
            vec![]
        };
        let animation_markers = renderer.take_marker_events();
        let audio_started = Instant::now();
        if let Some(audio) = &mut self.audio {
            if let Err(error) = audio.update(&self.game, view, &animation_markers) {
                self.audio_error = error;
            }
        }

        let audio_update_ms = audio_started.elapsed().as_secs_f64() * 1000.;
        if let Some(profile) = &mut self.profile {
            let renderer = self.renderer.as_ref().unwrap();
            let timing = renderer.last_timings;
            let projection_ms = (started.elapsed().as_secs_f64() * 1000. - timing.total_ms).max(0.);
            let snapshot = self.game.snapshot();
            let row = serde_json::json!({
                "schema":"openagents.verse.frame-profile.v1",
                "adapter":renderer.adapter_name,
                "render_dimensions":renderer.dimensions(),
                "samples":4,
                "scene_time":self.game.time,
                "player_hp":snapshot.player.hp,
                "player_position":self.game.player.to_array(),
                "spell_casts":snapshot.counters.casts,
                "living_actors":snapshot.actors.iter().filter(|a| a.alive).count(),
                "respawn_generation_max":self.game.scene.actors.iter().filter_map(|a| self.game.actor_life(a.id)).map(|life| life.generation).max().unwrap_or(0),
                "world_ms":self.world_ms,
                "projection_ms":projection_ms,
                "render":timing,
                "animation_markers":animation_markers,
                "audio":self.audio.as_ref().map(|audio| audio.stats()),
                "audio_error":self.audio_error,
                "audio_update_ms":audio_update_ms,
                "frame_work_ms":self.world_ms + projection_ms + timing.total_ms,
                "navigation_plans":self.game.navigation_plans,
                "physics_steps":self.game.physics_steps,
                "frame_interval_ms":self.frame_interval_ms,
                "schedule_dropped_seconds":self.schedule.dropped_seconds,
                "stress_casts":self.stress.as_ref().map(|s| s.casts),
                "asset_reload_pending":self.reload_pending.is_some(),
                "asset_reload_committed":self.reload_window_proof.as_ref().is_some_and(|p| p.commit.is_some()),
                "proof_capture":self.proof.is_some(),
            });
            serde_json::to_writer(&mut *profile, &row).map_err(|e| e.to_string())?;
            profile.write_all(b"\n").map_err(|e| e.to_string())?;
        }
        Ok(pixels)
    }
    fn select(&mut self) {
        let frame = self.game.frame();
        let size = self.renderer.as_ref().unwrap().dimensions();
        let vp = frame.view_projection(size[0] as f32 / size[1] as f32);
        let [width, height] = self.overlay_size();
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
            let dx = (ndc.x + 1.0) * width * 0.5 - self.cursor[0];
            let dy = (1.0 - ndc.y) * height * 0.5 - self.cursor[1];
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
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.audio = None;
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.game.scene.collision_profile.is_some() && self.audio.is_none() {
            match audio::Audio::open(&self.game) {
                Ok(audio) => self.audio = Some(audio),
                Err(error) => self.audio_error = error,
            }
        }
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(if self.game.scene.collision_profile.is_some() {
                                "Verse Engine — Original ritual chamber"
                            } else {
                                "The Verse — Scholomance"
                            })
                            .with_inner_size(if self.reload_window_proof.is_some() {
                                winit::dpi::LogicalSize::new(1728.0, 1084.0)
                            } else {
                                winit::dpi::LogicalSize::new(1280.0, 720.0)
                            }),
                    )
                    .map_err(|e| e.to_string())?,
            );
            if self.reload_window_proof.is_some() {
                window.focus_window();
            }
            let renderer = Renderer::new(
                (*self.pack).clone(),
                &self.dir,
                window.inner_size().width.max(1),
                window.inner_size().height.max(1),
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
            let profile = self.profile.take();
            let loading = self
                .draw_frame()
                .and_then(|_| self.renderer.as_ref().unwrap().finish_loading_frame());
            self.profile = profile;
            loading?;
            self.last = Instant::now();
            self.first_redraw = true;
            if let Some(stress) = &mut self.stress {
                stress.started = Some(Instant::now());
            }
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
                        let shift = self.keys.contains(&KeyCode::ShiftLeft)
                            || self.keys.contains(&KeyCode::ShiftRight);
                        if !event.repeat {
                            match key {
                                // Shift+1 through Shift+0 cast the second row.
                                key if shift && row_two_slot(key).is_some() => {
                                    self.activate(Ability::Spell(row_two_slot(key).unwrap()))
                                }
                                KeyCode::Escape => event_loop.exit(),
                                KeyCode::F1 | KeyCode::F2 => {
                                    self.game
                                        .restart_combat(key == KeyCode::F2)
                                        .expect("The loaded chamber admits combat");
                                    verse::imported::props::admit_collision(
                                        &self.pack,
                                        &mut self.game,
                                    )
                                    .expect("The loaded furniture admits collision");
                                    self.schedule = verse_engine::core::FixedSchedule::new(30, 3)
                                        .expect("The chamber uses a valid fixed schedule");
                                    self.controls.clear();
                                    self.keys.clear();
                                    self.pending_select = false;
                                    self.capture_pointer();
                                }
                                KeyCode::Space if !self.game.agent_controlled => {
                                    if let Err(error) = self.game.jump() {
                                        self.game.message = error;
                                    }
                                }
                                KeyCode::F5 => {
                                    if let Err(error) = self.start_reload() {
                                        self.reload_status = error;
                                    }
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
                        position.x as f32 / size.width.max(1) as f32 * self.overlay_size()[0],
                        position.y as f32 / size.height.max(1) as f32 * self.overlay_size()[1],
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
                let [width, height] = self.overlay_size();
                match button {
                    MouseButton::Right => {
                        if down {
                            self.pending_select = false;
                        }
                        self.controls
                            .button(true, down, &mut self.game.yaw, &self.game.camera);
                    }
                    MouseButton::Left => {
                        if down
                            && self.game.snapshot().player.hp == 0
                            && overlay::respawn_at(self.cursor[0], self.cursor[1], width, height)
                        {
                            match self.game.respawn_player() {
                                Ok(()) => {
                                    self.controls = Default::default();
                                    self.keys.clear();
                                    self.pointer = [0.; 2];
                                    self.pending_select = false;
                                    self.dragged = 0.;
                                }
                                Err(error) => self.game.message = error,
                            }
                            return;
                        }
                        if self.game.agent_controlled {
                            return;
                        }
                        if down && !self.controls.looking() {
                            if let Some(ability) =
                                overlay::action_at(self.cursor[0], self.cursor[1], width, height)
                            {
                                self.activate(ability);
                                return;
                            }
                            if overlay::chrome_at(self.cursor[0], self.cursor[1], width, height) {
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
                if event_loop.exiting() {
                    return;
                }
                if self.first_redraw {
                    self.last = Instant::now();
                    if let Some(stress) = &mut self.stress {
                        stress.started = Some(Instant::now());
                    }
                    self.first_redraw = false;
                }
                if let Some(proof) = &mut self.reload_window_proof {
                    proof.frames += 1;
                    if proof.resize && proof.frames == 240 {
                        let window = self.window.as_ref().unwrap();
                        let size = window.inner_size();
                        proof.original_size = Some([size.width, size.height]);
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                            size.width * 2 / 3,
                            size.height * 3 / 4,
                        ));
                    }
                    if proof.resize && proof.frames == 720 {
                        let size = proof.original_size.unwrap();
                        let _ =
                            self.window.as_ref().unwrap().request_inner_size(
                                winit::dpi::PhysicalSize::new(size[0], size[1]),
                            );
                    }
                    if proof.frames == 90 {
                        let output = proof.output.clone();
                        let capture = self.renderer.as_ref().unwrap().capture_submitted();
                        let dimensions = self.renderer.as_ref().unwrap().dimensions();
                        let writer = std::thread::spawn(move || {
                            let pixels = capture.finish()?;
                            save_png_size(&output.join("before.png"), &pixels, dimensions)
                        });
                        self.reload_window_proof.as_mut().unwrap().before = Some(writer);
                        let result = self.start_reload();
                        if let Err(error) = result {
                            eprintln!("{error}");
                            event_loop.exit();
                            return;
                        }
                    }
                }
                self.poll_reload();
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
                        if let Some(proof) = &mut self.reload_window_proof {
                            let dimensions = self.renderer.as_ref().unwrap().dimensions();
                            if proof.viewport_sizes.last() != Some(&dimensions) {
                                proof.viewport_sizes.push(dimensions);
                            }
                        }
                        if let Some(path) = self.proof.take() {
                            if let Err(e) = save_png(&path, &pixels) {
                                eprintln!("{e}");
                            }
                            event_loop.exit();
                        }
                        if self.stress.as_ref().is_some_and(|s| {
                            s.started
                                .is_some_and(|at| at.elapsed().as_secs_f64() >= s.duration)
                        }) {
                            if let Some(profile) = &mut self.profile {
                                if let Err(e) = profile.flush() {
                                    eprintln!("{e}");
                                }
                            }
                            if let Some(proof) = self.reload_window_proof.take() {
                                let output = proof.output.clone();
                                let result = proof
                                    .finish(self)
                                    .and_then(|_| self.capture_frame())
                                    .and_then(|pixels| {
                                        save_png_size(
                                            &output.join("after.png"),
                                            &pixels,
                                            self.renderer.as_ref().unwrap().dimensions(),
                                        )
                                    });
                                if let Err(error) = result {
                                    eprintln!("{error}");
                                    event_loop.exit();
                                    return;
                                }
                            }
                            eprintln!(
                                "Stress check completed: {} fireball casts",
                                self.stress.as_ref().unwrap().casts
                            );
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
/// Second action-bar row slot for a digit key: 1 is slot 0 and 0 is slot 9.
fn row_two_slot(key: KeyCode) -> Option<u8> {
    [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
    ]
    .iter()
    .position(|k| *k == key)
    .map(|slot| slot as u8)
}
fn save_png(path: &std::path::Path, pixels: &[u8]) -> Result<(), String> {
    save_png_size(path, pixels, [1280, 720])
}
fn save_png_size(
    path: &std::path::Path,
    pixels: &[u8],
    dimensions: [u32; 2],
) -> Result<(), String> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).map_err(|e| e.to_string())?,
        dimensions[0],
        dimensions[1],
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}
pub fn run(original_default: bool) -> Result<(), String> {
    let mut inputs: Vec<String> = std::env::args().skip(1).collect();
    if inputs.first().is_some_and(|a| a == "--check-profile") {
        return profile::check(std::path::Path::new(
            inputs.get(1).ok_or("Expected frame profile path")?,
        ));
    }
    if inputs.first().is_some_and(|a| a == "--refresh-inventory") {
        let path = PathBuf::from(inputs.get(1).ok_or("Expected runtime pack path")?);
        let pack = Pack::read(&path)?;
        if pack.inventory.is_none() {
            return Err("Pack has no original inventory to refresh".into());
        }
        reload::write_manifest(&path, &pack)?;
        return Ok(());
    }
    // `verse_play` implies `--original`; accept it when also given.
    if original_default && inputs.first().is_none_or(|a| a != "--original") {
        inputs.insert(0, "--original".into());
    }
    let quest_giver = inputs
        .iter()
        .position(|a| a == "--quest-giver")
        .map(|i| inputs.remove(i))
        .is_some();
    let greybox = inputs
        .iter()
        .position(|a| a == "--greybox")
        .map(|i| inputs.remove(i))
        .is_some();
    let appearance = if let Some(i) = inputs.iter().position(|a| a == "--appearance") {
        inputs.remove(i);
        if i >= inputs.len() {
            return Err("Expected character appearance".into());
        }
        inputs.remove(i)
    } else {
        "male-ranger".into()
    };
    let bestiary = if let Some(i) = inputs.iter().position(|a| a == "--bestiary") {
        inputs.remove(i);
        if i >= inputs.len() {
            return Err("Expected monster.glb".into());
        }
        Some(PathBuf::from(inputs.remove(i)))
    } else {
        std::env::var_os("HOME").map(PathBuf::from).map(|home| home.join("Downloads/Bestiary - Dungeon Monsters Kit[Standard]/Bestiary - Dungeon Monsters Kit[Standard]/Exports/GLB (Godot-Unreal)/Puglin.glb")).filter(|p| p.exists())
    };
    let mut args = inputs.into_iter();
    let input = args.next().ok_or("Expected pack.json or --original")?;
    let original = input == "--original";
    let path = if original {
        std::env::temp_dir().join(format!(
            "verse-original-ritual-{}/pack.json",
            std::process::id()
        ))
    } else {
        PathBuf::from(input)
    };
    let dir = path
        .parent()
        .ok_or("Expected pack directory")?
        .to_path_buf();
    let mut pack = if original {
        verse::imported::original::generate(&dir)?
    } else {
        Pack::read(&path)?
    };
    let character_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/characters/quaternius");
    let mut admitted_bestiary = None;
    if original && !greybox {
        let snapshot = dir.join("source-quaternius");
        verse::imported::inventory::snapshot_characters(&character_root, &snapshot)?;
        verse::imported::characters::install(&mut pack, &dir, &snapshot, &appearance)?;
        pack.source_revision = "verse-universal-ritual-v1".into();
        if let Some(path) = &bestiary {
            let frozen = dir.join("source-bestiary/Puglin.glb");
            verse::imported::inventory::snapshot_bestiary(path, &frozen)?;
            verse::imported::characters::install_bestiary(
                &mut pack,
                &dir,
                &frozen,
                &snapshot.join("animations.glb"),
            )?;
            admitted_bestiary = Some(frozen);
        }
    }
    if original && !greybox {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/props/quaternius");
        let snapshot = dir.join("source-fantasy-props");
        verse::imported::inventory::snapshot_props(&root, &snapshot)?;
        verse::imported::props::install(&mut pack, &dir, &snapshot)?;
    }
    if !original {
        verse_wow::motion::bind(&mut pack)?;
        chamber::add_effect_models(&mut pack, &dir)?;
    }
    let mut scene = Scene::from_json(if original && quest_giver {
        include_bytes!("../../../../assets/verse/original/ritual-quests.json").as_slice()
    } else if original {
        include_bytes!("../../../../assets/verse/original/ritual.json").as_slice()
    } else {
        include_bytes!("../../../../assets/verse/wow/anthropic.json").as_slice()
    })?;
    if original && !greybox {
        let mut index = 0;
        for actor in &mut scene.actors {
            if actor.model == "cultist" {
                actor.model = [
                    "cultist",
                    "cultist-female",
                    "cultist-peasant",
                    "cultist-peasant-female",
                ][index % 4]
                    .into();
                index += 1;
            }
        }
    }
    if pack.source_revision == "verse-bestiary-ritual-v1" {
        if let Some(claude) = scene.actors.iter_mut().find(|a| a.model == "claude") {
            claude.scale = 6.0 / (pack.models["claude"].height * 0.9144);
        }
    }
    let mut game = Game::new(scene)?;
    verse::imported::props::admit_collision(&pack, &mut game)?;
    let heights = pack
        .models
        .iter()
        .map(|(id, m)| (id.clone(), m.height))
        .collect();
    if original {
        verse::imported::inventory::compile(&mut pack, &dir, admitted_bestiary.as_deref())?;
    }
    let atlas = if original {
        chamber::original_portrait_atlas(&dir, &pack)?
    } else {
        chamber::portrait_atlas(&dir, &pack)?
    };
    let reload_path = original.then(|| dir.join("runtime-pack.json"));
    if let Some(path) = &reload_path {
        reload::write_manifest(path, &pack)?;
        eprintln!("F5 reloads renderer assets from {}", path.display());
    }
    let reload_contract = Arc::new(reload::Contract::new(
        &pack,
        &chamber::static_instances(&pack, position_from_wow(game.scene.origin_wow)),
    )?);
    let mut app = App {
        audio: None,
        audio_error: String::new(),
        window: None,
        presenter: None,
        renderer: None,
        pack: Arc::new(pack),
        atlas: Arc::new(atlas),
        reload_path,
        reload_contract,
        reload_pending: None,
        reload_status: String::new(),
        reload_window_proof: None,
        game,
        last: Instant::now(),
        first_redraw: true,
        schedule: verse_engine::core::FixedSchedule::new(30, 3)?,
        interpolation: 1.,
        capture_view: None,
        keys: HashSet::new(),
        cursor: [0.0; 2],
        controls: ClassicControls::default(),
        pointer: [0.0; 2],
        captured: false,
        profile: match std::env::var_os("VERSE_FRAME_PROFILE") {
            Some(path) => Some(std::io::BufWriter::new(
                std::fs::File::create(path).map_err(|e| e.to_string())?,
            )),
            None => None,
        },
        world_ms: 0.,
        frame_interval_ms: 0.,
        stress: None,
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
        Some("--reload-window-proof" | "--resize-window-proof" | "--audio-window-proof")
    ) {
        let output = PathBuf::from(
            args.next()
                .ok_or("Expected window reload proof directory")?,
        );
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        let mut changed = (*app.pack).clone();
        changed
            .models
            .get_mut("claude")
            .ok_or("Missing Claude reload model")?
            .surfaces[0]
            .tint = [0.7, 0.12, 0.08];
        reload::write_manifest(
            app.reload_path
                .as_ref()
                .ok_or("Window reload proof requires an original scene")?,
            &changed,
        )?;
        let duration: f64 = args
            .next()
            .map(|v| v.parse().map_err(|_| "Invalid reload proof duration"))
            .transpose()?
            .unwrap_or(15.);
        if !duration.is_finite() || !(15. ..=600.).contains(&duration) {
            return Err("Reload proof duration must be between 15 and 600 seconds".into());
        }
        app.reload_window_proof = Some(reload::WindowProof {
            output: output.clone(),
            frames: 0,
            commit: None,
            before: None,
            duration,
            resize: mode.as_deref() == Some("--resize-window-proof"),
            original_size: None,
            viewport_sizes: vec![],
        });
        app.profile = Some(std::io::BufWriter::new(
            std::fs::File::create(output.join("frames.ndjson")).map_err(|e| e.to_string())?,
        ));
        app.game = combat_game(&app.pack, app.game.scene.clone(), false)?;
        app.game.time = app.game.scene.cut_at;
        app.game
            .encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)?;
        app.stress = Some(Stress {
            duration,
            started: None,
            next_cast: app.game.time + 1.,
            next_shield: (mode.as_deref() == Some("--audio-window-proof"))
                .then_some(app.game.time + 5.),
            casts: 0,
            move_forward: true,
        });
    }
    if mode.as_deref() == Some("--stress-demo") {
        let profile = args.next().ok_or("Expected frame profile path")?;
        let duration: f64 = args
            .next()
            .ok_or("Expected stress duration in seconds")?
            .parse()
            .map_err(|_| "Invalid stress duration")?;
        if !duration.is_finite() || !(10. ..=600.).contains(&duration) {
            return Err("Stress duration must be between 10 and 600 seconds".into());
        }
        app.profile = Some(std::io::BufWriter::new(
            std::fs::File::create(profile).map_err(|e| e.to_string())?,
        ));
        app.game = combat_game(&app.pack, app.game.scene.clone(), false)?;
        app.game.time = app.game.scene.cut_at;
        app.game
            .encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)?;
        app.stress = Some(Stress {
            duration,
            started: None,
            next_cast: app.game.time + 1.,
            next_shield: None,
            casts: 0,
            move_forward: true,
        });
    }
    if original && mode.is_none() {
        app.game = combat_game(&app.pack, app.game.scene.clone(), false)?;
    }
    if matches!(
        mode.as_deref(),
        Some("--respawn-proof" | "--residency-proof" | "--reload-proof" | "--inventory-proof")
    ) {
        let output = PathBuf::from(args.next().ok_or("Expected proof directory")?);
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        app.game = combat_game(&app.pack, app.game.scene.clone(), false)?;
        while app.game.time < 150. && app.game.snapshot().player.hp > 0 {
            app.game.tick(1. / 30., [0.; 2])?;
        }
        if app.game.snapshot().player.hp != 0 {
            return Err("Proof player did not die".into());
        }
        app.renderer = Some(Renderer::new(
            (*app.pack).clone(),
            &app.dir,
            1280,
            720,
            &app.atlas,
            &chamber::static_instances(&app.pack, position_from_wow(app.game.scene.origin_wow)),
        )?);
        app.interpolation = 1.;
        save_png(&output.join("player-dead.png"), &app.draw_frame()?)?;
        if mode.as_deref() == Some("--residency-proof") {
            let cold = std::fs::read(output.join("player-dead.png")).map_err(|e| e.to_string())?;
            app.draw_frame()?;
            let cached = app.draw_frame()?;
            save_png(&output.join("cached-dead.png"), &cached)?;
            let timing = app.renderer.as_ref().unwrap().last_timings;
            if cold != std::fs::read(output.join("cached-dead.png")).map_err(|e| e.to_string())?
                || timing.cached_shadow_casters == 0
                || timing.static_shadow_refreshes != 0
            {
                return Err("Cached corpse shadows changed pixels or failed to reuse depth".into());
            }
            std::fs::write(output.join("shadow-cache.json"), serde_json::to_vec_pretty(
                &serde_json::json!({"schema":"openagents.verse.shadow-cache-proof.v1",
                    "cold_cached_pixels_identical":true, "cached_shadow_casters":timing.cached_shadow_casters,
                    "static_shadow_refreshes":timing.static_shadow_refreshes,
                    "dynamic_shadow_draws":timing.shadow_draws})
            ).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let frame = app.game.frame();
            let actors = chamber::instances(&app.pack, &frame)?;
            let prior = app.renderer.as_ref().unwrap().resolve_instances(&actors)?;
            let empty = app.renderer.as_ref().unwrap().resolve_instances(&[])?;
            // This proof rebuilds offscreen residency; it does not exercise a live surface reload.
            app.renderer = Some(Renderer::new(
                (*app.pack).clone(),
                &app.dir,
                1280,
                720,
                &app.atlas,
                &chamber::static_instances(&app.pack, position_from_wow(app.game.scene.origin_wow)),
            )?);
            let view = View {
                view_proj: frame.view_projection(1280. / 720.),
                eye: frame.eye,
            };
            let ui = verse::ui::UiBatch::default();
            let lighting = chamber::combat_lighting(&app.game);
            let renderer = app.renderer.as_mut().unwrap();
            let stale = renderer
                .draw_resolved(view, &prior, &ui, &lighting)
                .unwrap_err();
            let stale_empty = renderer
                .draw_live_resolved(view, &empty, &ui, &lighting)
                .unwrap_err();
            if !stale.contains("foreign asset catalog")
                || !stale_empty.contains("foreign asset catalog")
            {
                return Err("Residency proof did not reject stale frames".into());
            }
            save_png(&output.join("rebuilt-dead.png"), &app.draw_frame()?)?;
            let identical = std::fs::read(output.join("player-dead.png"))
                .map_err(|e| e.to_string())?
                == std::fs::read(output.join("rebuilt-dead.png")).map_err(|e| e.to_string())?;
            if !identical {
                return Err("Rebuilt residency changed the captured scene".into());
            }
            std::fs::write(
                output.join("residency.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "schema":"openagents.verse.residency-proof.v1", "stale_frame":stale,
                    "stale_empty_frame":stale_empty, "rebuild_pixels_identical":identical,
                    "live_surface_reload":false,
                }))
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        if mode.as_deref() == Some("--reload-proof") {
            reload::prove(&mut app, &output)?;
        }
        if mode.as_deref() == Some("--inventory-proof") {
            reload::prove_inventory(&mut app, &output)?;
        }
        let old = app.game.player_life();
        if !overlay::respawn_at(640., 328., 1280., 720.) {
            return Err("Proof click missed Respawn".into());
        }
        app.game.respawn_player()?;
        save_png(&output.join("player-respawned.png"), &app.draw_frame()?)?;
        app.game.activate(Ability::Shield)?;
        for _ in 0..30 {
            app.game.tick(1. / 30., [0., 1.])?;
        }
        let evidence = serde_json::json!({"schema":"openagents.verse.player-respawn.v1",
            "rules_revision":verse::imported::play::RULES_REVISION, "prepared_pack":app.renderer.as_ref().unwrap().pack_receipt, "old_life":old,
            "new_life":app.game.player_life(), "player":app.game.snapshot().player,
            "player_position":app.game.player, "control_mode":"human", "shield_cast":true,
            "world_time":app.game.time, "npc_lives":app.game.frame().actors.iter()
                .filter(|a| a.actor.nameplate).map(|a| (a.actor.id, a.health)).collect::<Vec<_>>()});
        std::fs::write(
            output.join("player-respawn.json"),
            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if mode.as_deref() == Some("--lair-proof") {
        let output = PathBuf::from(args.next().ok_or("Expected lair proof directory")?);
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        app.renderer = Some(Renderer::new(
            (*app.pack).clone(),
            &app.dir,
            1920,
            1080,
            &app.atlas,
            &chamber::static_instances(&app.pack, position_from_wow(app.game.scene.origin_wow)),
        )?);
        for (name, time) in [
            ("ritual-wide", 4.),
            ("summoning", 12.),
            ("player-handoff", 20.8),
        ] {
            while app.game.time < time {
                app.game.tick(1. / 30., [0.; 2])?;
            }
            app.interpolation = 1.;
            save_png_size(
                &output.join(format!("{name}.png")),
                &app.draw_frame()?,
                [1920, 1080],
            )?;
        }
        let evidence = serde_json::json!({"schema":"openagents.verse.lair-proof.v1", "render_dimensions":[1920,1080], "samples":4, "props":app.pack.placements.len()-1,
            "collision_props":app.game.navigation_blockers().active_bounds().count(), "source_admission":app.renderer.as_ref().unwrap().pack_receipt,
            "lights":chamber::combat_lighting(&app.game).lights.len(), "lighting":"warm torches and candles, green cauldrons, violet summoning light"});
        std::fs::write(
            output.join("scene.json"),
            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if mode.as_deref() == Some("--spell-playground") {
        let spell = args.next().ok_or("Expected a spell name or all")?;
        let output = PathBuf::from(args.next().ok_or("Expected OUT.mp4 or OUT_DIR")?);
        return spell_playground(&mut app, &spell, output);
    }
    let demonstration = match mode.as_deref() {
        Some("--demo") => Some(Demo::Spells),
        Some("--utility-demo") => Some(Demo::Utilities),
        Some("--combat-demo") => Some(Demo::Combat),
        Some("--stress-capture") => Some(Demo::Stress),
        Some("--navigation-demo") => Some(Demo::Navigation),
        Some("--movement-demo") => Some(Demo::Movement),
        Some("--stair-navigation-demo") => Some(Demo::StairNavigation),
        _ => None,
    };
    if let Some(demonstration) = demonstration {
        return demo(
            &mut app,
            PathBuf::from(args.next().ok_or("Expected demo.mp4")?),
            demonstration,
        );
    }
    if matches!(mode.as_deref(), Some("--agent" | "--combat")) {
        app.game = combat_game(
            &app.pack,
            app.game.scene.clone(),
            mode.as_deref() == Some("--agent"),
        )?;
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
        app.game = combat_game(&app.pack, app.game.scene.clone(), true)?;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Demo {
    Spells,
    Utilities,
    Combat,
    Stress,
    Navigation,
    Movement,
    StairNavigation,
}
fn demo(app: &mut App, output: PathBuf, mode: Demo) -> Result<(), String> {
    let utility = mode == Demo::Utilities;
    let combat = mode == Demo::Combat;
    let stress = mode == Demo::Stress;
    let navigation = mode == Demo::Navigation;
    let movement_demo = mode == Demo::Movement;
    let stair_navigation = mode == Demo::StairNavigation;
    use std::io::Write;
    if combat {
        app.game = combat_game(&app.pack, app.game.scene.clone(), true)?;
    }
    if stress {
        app.game = combat_game(&app.pack, app.game.scene.clone(), false)?;
        app.game.time = app.game.scene.cut_at;
        app.game
            .encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(600.)?;
        app.stress = Some(Stress {
            duration: 100.,
            started: None,
            next_cast: app.game.time + 1.,
            next_shield: None,
            casts: 0,
            move_forward: true,
        });
    }
    if movement_demo {
        let mut scene = app.game.scene.clone();
        scene
            .actors
            .iter_mut()
            .find(|a| a.model == "adventurer")
            .unwrap()
            .position = Vec3::new(18., 0., -32.);
        app.game = Game::new(scene)?;
        verse::imported::props::admit_collision(&app.pack, &mut app.game)?;
        app.game.time = 20.;
        app.game.yaw = std::f32::consts::PI;
        app.game.camera.yaw = app.game.yaw;
        app.game.camera.pitch = 0.25;
        app.game.camera.distance = 7.;
    }
    if stair_navigation {
        app.capture_view = Some((Vec3::new(20.5, 6., -35.), Vec3::new(18., 2., -28.5), 1.05));
        let mut scene = app.game.scene.clone();
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 14)
            .unwrap()
            .position = Vec3::new(18., 1.5, -25.);
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 2)
            .unwrap()
            .position = Vec3::new(18., 0., -32.);
        app.game = combat_game(&app.pack, scene, false)?;
        app.game.time = 20.;
        app.game
            .encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(50.)?;
        app.game.direct_npc_navigation(
            app.game.actor_life(2).unwrap(),
            Vec3::new(18.8, 1.5, -25.),
            1.2,
        )?;
        app.game.set_navigation_blocker(
            physics::queries::Life {
                instance: 0,
                entity: 9001,
                generation: 0,
            },
            glam::DVec3::new(17.6, 0., -31.2),
            glam::DVec3::new(18.4, 1.2, -30.6),
        )?;
        app.game.yaw = std::f32::consts::PI;
        app.game.camera.yaw = std::f32::consts::PI - 0.5;
        app.game.camera.pitch = 0.3;
        app.game.camera.distance = 7.;
    }
    if navigation {
        if app.game.scene.collision_profile.is_none() {
            return Err("Navigation capture requires original geometry".into());
        }
        let mut scene = app.game.scene.clone();
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 14)
            .unwrap()
            .position = Vec3::new(13., 0., -13.);
        scene
            .actors
            .iter_mut()
            .find(|a| a.id == 2)
            .unwrap()
            .position = Vec3::new(17., 0., -13.);
        scene.validate()?;
        app.game = combat_game(&app.pack, scene, false)?;
        app.game.time = 20.;
        app.game
            .encounter
            .as_mut()
            .unwrap()
            .postpone_casts_until(32.)?;
        app.game.yaw = -std::f32::consts::FRAC_PI_2;
        app.game.camera.yaw = app.game.yaw;
        app.game.camera.pitch = 0.15;
        app.game.camera.distance = 7.;
    }
    for _ in 0..if app.game.scene.collision_profile.is_some() {
        0
    } else {
        180
    } {
        app.game.tick(0.1, [0.0, 0.0])?;
    }
    app.renderer = Some(Renderer::new(
        (*app.pack).clone(),
        &app.dir,
        1280,
        720,
        &app.atlas,
        &chamber::static_instances(&app.pack, position_from_wow(app.game.scene.origin_wow)),
    )?);
    let mut encoder = encoder(&output)?;
    let mut pipe = encoder.stdin.take().ok_or("Missing encoder input")?;
    let mut movement_max_height: f32 = 0.;
    app.interpolation = 1.;
    for frame in 0..if stress {
        3000
    } else if combat {
        3600
    } else if stair_navigation {
        360
    } else if navigation || movement_demo {
        300
    } else {
        480
    } {
        if stress {
            app.last = Instant::now() - std::time::Duration::from_secs_f64(1. / 30.);
            let pixels = app.render()?;
            pipe.write_all(&pixels).map_err(|e| e.to_string())?;
            if frame % 300 == 0 {
                eprintln!(
                    "Stress capture: {} seconds, {} completed spells",
                    frame / 30,
                    app.game.snapshot().counters.casts
                );
            }
            if frame == 2999 {
                save_png(&output.with_extension("png"), &pixels)?;
            }
            continue;
        }
        if stair_navigation && frame == 120 {
            app.game.remove_navigation_blocker(physics::queries::Life {
                instance: 0,
                entity: 9001,
                generation: 0,
            })?;
        }
        if movement_demo && frame == 110 {
            app.game.jump()?;
        }
        let world_started = Instant::now();
        app.game.tick(
            1.0 / 30.0,
            if movement_demo && frame < 180 {
                [0., 0.25]
            } else {
                [0.; 2]
            },
        )?;
        app.world_ms = world_started.elapsed().as_secs_f64() * 1000.;
        movement_max_height = movement_max_height.max(app.game.player.y);
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
            if !combat && !navigation && !movement_demo && !stair_navigation && frame == at {
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
        if movement_demo && frame == 120 {
            save_png(&output.with_extension("png"), &pixels)?;
        }
        if movement_demo && frame == 299 {
            if movement_max_height < 2. || app.game.player.y > 0.001 {
                return Err("Movement capture did not climb, jump, and land".into());
            }
            std::fs::write(output.with_extension("json"),serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"openagents.verse.grounded-movement.v1", "rules_revision":verse::imported::play::RULES_REVISION,
                "authority_tick":app.game.authority_tick, "physics_steps":app.game.physics_steps,
                "physics_dropped_seconds":app.game.physics_clock.dropped,"max_height_m":movement_max_height,
                "final_feet":app.game.player.to_array(),"jump_command_frame":110,
                "renderer":"native GPU frames; programmatic admitted movement and jump; no grading"
            })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        }
        if stair_navigation && frame == 100 {
            save_png(&output.with_extension("png"), &pixels)?;
        }
        if stair_navigation && frame == 359 {
            let position = app
                .game
                .frame()
                .actors
                .iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .actor
                .position;
            let target = Vec3::new(18.8, 1.5, -25.);
            if position.distance(target) > 0.1 || app.game.navigation_budget_refusals > 0 {
                return Err(format!(
                    "Cultist did not reach the stair goal: {position:?}"
                ));
            }
            std::fs::write(output.with_extension("json"),serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"openagents.verse.navigation.v2","rules_revision":verse::imported::play::RULES_REVISION,
                "start":[18.,0.,-32.],"target":target.to_array(),"cultist_final":position.to_array(),
                "authority_tick":app.game.authority_tick,"physics_steps":app.game.physics_steps,
                "navigation_plans":app.game.navigation_plans,"navigation_budget_refusals":app.game.navigation_budget_refusals,
                "blocker_revision":app.game.navigation_blockers().revision,"blocker_removed_at_frame":120,
                "renderer":"native GPU frames; admitted NPC intent; no teleport or grading"
            })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        }
        if navigation && frame == 299 {
            save_png(&output.with_extension("png"), &pixels)?;
            let position = app
                .game
                .frame()
                .actors
                .into_iter()
                .find(|a| a.actor.id == 2)
                .unwrap()
                .actor
                .position;
            if (position.z + 13.).abs() < 1. {
                return Err("Navigation capture did not show a column detour".into());
            }
            std::fs::write(output.with_extension("json"),serde_json::to_vec_pretty(&serde_json::json!({"schema":"openagents.verse.navigation.v1","asset_pack":app.pack.source_revision,"start":[17.,0.,-13.],"target":[13.,0.,-13.],"cultist_final":position,"player_hp":app.game.snapshot().player.hp,"renderer":"native GPU frames"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
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
                let evidence = serde_json::json!({"schema":"openagents.verse.agent-combat.v1","rules_revision":verse::imported::play::RULES_REVISION,"authority_tick":app.game.authority_tick,"physics_steps":app.game.physics_steps,"physics_dropped_seconds":app.game.physics_clock.dropped,"committed_event_count":app.game.events.len(),"asset_pack":app.pack.source_revision,"controller":"local observation-driven tactical controller","control_mode":"agent","time":app.game.time,"ended_at":encounter.ended,"player":app.game.snapshot().player,"boss_remaining":encounter.boss_remaining,"boss_max":encounter.boss_max,"cultists_defeated":encounter.kills,"damage_taken":encounter.damage,"shield_absorbed":encounter.absorbed,"dodged":encounter.dodged,"enemy_casts":encounter.enemy_casts,"ability_uses":encounter.used,"boss_model":{"source":app.pack.models["claude"].source,"sha256":app.pack.models["claude"].source_sha256,"height_m":app.game.scene.actors.iter().find(|a|a.model=="claude").unwrap().scale * app.pack.models["claude"].height * 0.9144},"prepared_pack":app.renderer.as_ref().unwrap().pack_receipt,"animation_contract":"named states and life-aware local-space transitions", "animation_bindings":app.pack.models.iter().filter(|(_,m)|!m.states.is_empty()).map(|(name,m)|(name, &m.states)).collect::<std::collections::BTreeMap<_,_>>(), "renderer":"owned native GPU pipeline; no grading; no chat-input automation"});
                std::fs::write(
                    output.with_extension("json"),
                    serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if app.game.snapshot().player.hp != 0
                    || encounter.boss_remaining == 0
                    || encounter.boss_remaining >= encounter.boss_max
                {
                    return Err(
                        "Combat recording did not reach the expected adventurer defeat".into(),
                    );
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
        "Owned combat: {} spell casts, {} projectiles, {} impacts, {} mana",
        snapshot.counters.casts,
        snapshot.counters.projectiles,
        snapshot.counters.hits,
        snapshot.player.mana
    );
    Ok(())
}

fn combat_game(pack: &Pack, scene: Scene, agent: bool) -> Result<Game, String> {
    let mut game = Game::combat(scene, agent)?;
    // The live agent locks groups of cultists in place with Black Tentacles.
    game.spells.agent_tentacles = true;
    verse::imported::props::admit_collision(pack, &mut game)?;
    Ok(game)
}

/// Spell playground recordings render the scene at 2560x1440 with the
/// renderer's 4x multisampling, like the native window on a Retina display.
/// The HUD keeps its 1280x720 logical layout and scales with the frame.
const RECORD: [u32; 2] = [2560, 1440];

/// Starts an H.264 encoder that reads 1280x720 RGBA frames at 30 fps.
fn encoder(output: &std::path::Path) -> Result<std::process::Child, String> {
    encoder_sized(output, [1280, 720])
}

/// Starts an H.264 encoder that reads RGBA frames of `size` at 30 fps.
fn encoder_sized(output: &std::path::Path, size: [u32; 2]) -> Result<std::process::Child, String> {
    std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{}x{}", size[0], size[1]),
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
        .map_err(|e| e.to_string())
}

/// `--spell-playground SPELL OUT.mp4` records one scenario;
/// `--spell-playground all OUT_DIR` records every registered one as
/// `OUT_DIR/spell-<name>.mp4`. Each writes its evidence beside the video.
fn spell_playground(app: &mut App, spell: &str, output: PathBuf) -> Result<(), String> {
    use verse_world::playground::scenarios;
    if spell == "all" {
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        for scenario in scenarios() {
            let path = output.join(format!("spell-{}.mp4", scenario.key));
            record_spell(app, scenario, &path)?;
        }
        return Ok(());
    }
    let scenario = scenarios()
        .into_iter()
        .find(|s| s.key == spell)
        .ok_or_else(|| {
            format!(
                "Unknown spell {spell}; registered: {}",
                scenarios()
                    .iter()
                    .map(|s| s.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    record_spell(app, scenario, &output)
}

fn record_spell(
    app: &mut App,
    scenario: verse_world::playground::Scenario,
    output: &std::path::Path,
) -> Result<(), String> {
    let mut run = verse_world::playground::Run::new(scenario)?;
    app.renderer = Some(Renderer::new(
        (*app.pack).clone(),
        &app.dir,
        RECORD[0],
        RECORD[1],
        &app.atlas,
        &chamber::playground_static_instances(),
    )?);
    app.renderer
        .as_ref()
        .unwrap()
        .set_overlay_size(1280.0, 720.0);
    let mut encoder = encoder_sized(output, RECORD)?;
    let mut pipe = encoder.stdin.take().ok_or("Missing encoder input")?;
    let poster = run.live_frames() + run.frames().saturating_sub(run.live_frames()) / 2;
    while !run.done() {
        run.advance()?;
        let game = &run.game;
        let mut frame = game.interpolated_frame(run.alpha)?;
        let (eye, target) = run.camera();
        frame.eye = eye;
        frame.target = target;
        frame.fov = 1.0;
        let view = View {
            view_proj: frame.view_projection(1280.0 / 720.0),
            eye: frame.eye,
        };
        let mut ui = overlay::cinematic(
            &app.atlas,
            &frame,
            &app.heights,
            view.view_proj,
            1280.0,
            720.0,
        );
        overlay::damage_numbers(
            &mut ui,
            &app.atlas,
            game,
            &frame,
            &app.heights,
            view.view_proj,
            1280.0,
            720.0,
        );
        overlay::action_bar(&mut ui, &app.atlas, game, 1280.0, 720.0, None);
        overlay::spell_panel(
            &mut ui,
            &app.atlas,
            &run.overlay(),
            game,
            view.view_proj,
            1280.0,
            720.0,
            run.replaying(),
        );
        let mut actors = chamber::instances(&app.pack, &frame)?;
        actors.extend(chamber::spell_instances(game));
        actors.extend(chamber::prop_instances(&app.pack, game, run.alpha));
        let lighting = chamber::combat_lighting(game);
        let pixels = app
            .renderer
            .as_mut()
            .unwrap()
            .draw(view, &actors, &ui, &lighting)?;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        if run.frame() == poster {
            save_png_size(&output.with_extension("png"), &pixels, RECORD)?;
        }
    }
    drop(pipe);
    if !encoder.wait().map_err(|e| e.to_string())?.success() {
        return Err("Spell playground encoder failed".into());
    }
    if run.replay_identical != Some(true) {
        return Err("The slow-motion replay diverged from the live run".into());
    }
    let mut evidence = run.evidence()?;
    evidence["renderer"] =
        "owned native GPU pipeline; scripted admitted commands; no grading".into();
    evidence["prepared_pack"] =
        serde_json::to_value(app.renderer.as_ref().unwrap().pack_receipt.clone())
            .map_err(|e| e.to_string())?;
    std::fs::write(
        output.with_extension("json"),
        serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    eprintln!(
        "Recorded {} ({} frames) to {}",
        run.scenario.title,
        run.frames(),
        output.display()
    );
    Ok(())
}
