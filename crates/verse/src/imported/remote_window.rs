//! Native mounting for an already authenticated chamber transport. The
//! shared [`chamber_session::Session`] owns the worker, the replica, and
//! prediction; this window adds the keyboard and mouse, the panels, and the
//! recorder.
use super::{
    Renderer, WindowPresenter, chamber,
    chamber_session::{self, Note, Session, Stopped},
    controls, overlay,
};
use crate::ui::Atlas;
use glam::Vec3;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        worker::{self, Input},
    },
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

/// Runs on the main thread. The supplied runtime must own the client's transport.
/// Callers admit the scene and pack before mounting; this does not fetch assets.
pub fn run(
    client: Client,
    runtime: tokio::runtime::Runtime,
    pack: Pack,
    atlas: Atlas,
    scene: Scene,
    dir: PathBuf,
) -> Result<(), String> {
    run_recorded(client, runtime, pack, atlas, scene, dir, None)
}
pub fn run_recorded(
    client: Client,
    runtime: tokio::runtime::Runtime,
    pack: Pack,
    atlas: Atlas,
    scene: Scene,
    dir: PathBuf,
    record: Option<super::remote_record::Options>,
) -> Result<(), String> {
    if let Some(options) = &record {
        options.validate()?;
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let observer = worker::Observer::default();
    let session = Session::start_observed(
        client,
        runtime,
        &scene,
        record.as_ref().map(|_| observer.clone()),
    )?;
    let mut app = App::new(pack, atlas, scene, dir, session);
    if record.is_some() {
        app.session.observe();
    }
    app.record = record;
    app.observer = observer;
    let result = event_loop.run_app(&mut app).map_err(|e| e.to_string());
    let worker_result = match app.session.close() {
        Stopped::Closed => Ok(()),
        Stopped::Failed(error) => Err(error),
    };
    let recording_result = app.finish_recording();
    result?;
    worker_result?;
    if let Some(error) = app.error {
        return Err(error);
    }
    recording_result
}
struct App {
    pack: Pack,
    atlas: Atlas,
    scene: Scene,
    dir: PathBuf,
    session: Session,
    character_panel: super::character_panel::Panel,
    giver_panel: super::giver_panel::Panel,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    presenter: Option<WindowPresenter>,
    keys: HashSet<KeyCode>,
    pointer: [f32; 2],
    observer: worker::Observer,
    latest_verified_snapshot: Option<Instant>,
    warmup_completed_at: Option<Instant>,
    last_input_handled: Option<Instant>,
    /// The owned life the window last released input for.
    seen_life: Option<verse_engine::core::LifeId>,
    error: Option<String>,
    record: Option<super::remote_record::Options>,
    recorder: Option<super::remote_record::Recorder>,
    record_started: Option<Instant>,
    next_capture: Instant,
    next_demo: f32,
    demo_slot: usize,
    profile: super::remote_record::Profile,
    demo_trace: Vec<serde_json::Value>,
    min_hp: i32,
    recorded_world_start: Option<f32>,
}
impl App {
    fn new(pack: Pack, atlas: Atlas, scene: Scene, dir: PathBuf, session: Session) -> Self {
        Self {
            pack,
            atlas,
            scene,
            dir,
            session,
            character_panel: Default::default(),
            giver_panel: Default::default(),
            window: None,
            renderer: None,
            presenter: None,
            keys: HashSet::new(),
            pointer: [0.; 2],
            observer: worker::Observer::default(),
            latest_verified_snapshot: None,
            warmup_completed_at: None,
            last_input_handled: None,
            seen_life: None,
            error: None,
            record: None,
            recorder: None,
            record_started: None,
            next_capture: Instant::now(),
            next_demo: 0.,
            demo_slot: 0,
            profile: Default::default(),
            demo_trace: vec![],
            min_hp: i32::MAX,
            recorded_world_start: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.error = Some(error);
        event_loop.exit();
    }
    fn unlocked(&self) -> bool {
        self.session.unlocked(&self.scene)
    }
    fn controlled(&self) -> bool {
        self.session.controlled(&self.scene)
    }
    fn in_play(&self) -> bool {
        self.unlocked() && self.session.hud().is_some()
    }
    fn send(&mut self, input: Input) {
        self.session.send(input);
    }
    fn cast(&mut self, ability: Ability) {
        self.session.cast(&self.scene, ability);
    }
    /// Applies the worker's updates, feeds the recorder the session's notes,
    /// and frees the pointer when the owned life changes.
    fn consume(&mut self) -> Result<(), String> {
        let result = self.session.consume(&self.scene);
        for note in self.session.take_notes() {
            self.note(note);
        }
        result?;
        let life = self.session.owned_life();
        if life != self.seen_life {
            self.seen_life = life;
            // The session already stopped the character and cleared its controls.
            self.release_cursor();
        }
        Ok(())
    }
    fn note(&mut self, note: Note) {
        let profile = &mut self.profile;
        match note {
            Note::Reset(reason) => {
                profile.reset_observations += 1;
                *profile.reset_reasons.entry(reason).or_default() += 1;
            }
            Note::Correction {
                distance,
                discontinuity,
                detail,
            } => profile.correction(distance, discontinuity, detail),
            Note::Retirement { distance, detail } => profile.retirement(distance, detail),
            Note::Refusal(detail) => profile.refusal(detail),
            Note::Bound { token, at } => {
                if profile.bindings.len() < 64 {
                    profile.bindings.insert(token, at);
                }
            }
            Note::Unbound(token) => {
                profile.bindings.remove(&token);
            }
            Note::Outcome { token, at } => {
                if let Some(started) = token.and_then(|t| profile.bindings.remove(&t)) {
                    profile
                        .bound_to_outcome_ms
                        .add(at.duration_since(started).as_secs_f64() * 1000.);
                }
            }
        }
    }
    fn redraw(&mut self) -> Result<(), String> {
        let started = Instant::now();
        self.consume()?;
        if !self.controlled()
            && (self.session.controls.looking()
                || self.session.controls.autorun
                || !self.keys.is_empty())
        {
            self.release_pointer();
        }
        let now = Instant::now();
        if let Some(hud) = self.session.hud() {
            self.min_hp = self.min_hp.min(hud.resources.hp);
        }
        self.demo();
        let interval = self.session.tick();
        let frame_interval_ms = interval.as_secs_f64() * 1000.;
        let dt = interval.as_secs_f32().min(0.1);
        if self.record.is_some() {
            self.profile.frame_interval_ms.add(frame_interval_ms);
        }
        let held = controls::Held {
            forward: self.keys.contains(&KeyCode::KeyW),
            backward: self.keys.contains(&KeyCode::KeyS),
            turn_left: self.keys.contains(&KeyCode::KeyA),
            turn_right: self.keys.contains(&KeyCode::KeyD),
            strafe_left: self.keys.contains(&KeyCode::KeyQ),
            strafe_right: self.keys.contains(&KeyCode::KeyE),
        };
        let axes = self.session.turn(held, dt);
        let axes = if self.record.as_ref().is_some_and(|o| o.movement) {
            super::remote_record::movement_axes(
                self.record_started
                    .map_or(0., |s| s.elapsed().as_secs_f64()),
            )
        } else {
            axes
        };
        let movement_ready = !self.record.as_ref().is_some_and(|o| o.movement_frames)
            || self.session.movement_frames();
        self.session.steer(&self.scene, axes, dt, movement_ready)?;
        let size = self.window.as_ref().unwrap().inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let width = 720. * size.width as f32 / size.height as f32;
        let mut frame = self.session.frame_in(
            &self.pack,
            &self.atlas,
            &self.scene,
            [size.width, size.height],
            [width, 720.],
        )?;
        if self.in_play() {
            self.giver_panel
                .draw(&mut frame.ui, &self.atlas, self.session.view());
            self.character_panel.draw(
                &mut frame.ui,
                &self.atlas,
                self.session.view().inventory(),
                width,
                720.,
            );
        }
        let render_started = Instant::now();
        let preparation_ms = started.elapsed().as_secs_f64() * 1000.;
        if self.record.is_some() {
            self.profile.preparation_ms.add(preparation_ms);
        }
        let renderer = self.renderer.as_mut().unwrap();
        if renderer.recover_if_lost(&self.atlas)? {
            self.presenter = Some(renderer.attach_window(self.window.as_ref().unwrap().clone())?);
        }
        renderer.resize(size.width, size.height)?;
        renderer.set_overlay_size(width, 720.);
        renderer.draw_live(frame.view, &frame.instances, &frame.ui, &frame.lighting)?;
        let present_started = Instant::now();
        renderer.present_window(self.presenter.as_mut().unwrap(), [size.width, size.height])?;
        if self.record.is_some() {
            self.profile.render(
                renderer.last_timings,
                present_started.elapsed().as_secs_f64() * 1000.,
            );
            let frame = renderer.last_timings.frame;
            self.profile
                .measurements
                .record(frame, "client_preparation_cpu_ms", preparation_ms);
            self.profile
                .measurements
                .record(frame, "frame_interval_ms", frame_interval_ms);
            if frame > self.profile.measurements.warmup_frames()
                && self.warmup_completed_at.is_none()
            {
                self.warmup_completed_at = Some(render_started);
            }
            let observations = self.observer.drain();
            self.profile.network_observations_omitted = observations.omitted;
            self.latest_verified_snapshot = observations
                .snapshot_verified_at
                .or(self.latest_verified_snapshot);
            for observation in observations.samples {
                let (turnaround, delivery) = match observation.kind {
                    "snapshot" => ("snapshot_turnaround_ms", "snapshot_verified_to_sample_ms"),
                    "inventory" => ("inventory_turnaround_ms", "inventory_verified_to_sample_ms"),
                    "events" => ("events_turnaround_ms", "events_verified_to_sample_ms"),
                    _ => ("command_turnaround_ms", "command_verified_to_sample_ms"),
                };
                let request_phase = if self
                    .warmup_completed_at
                    .is_none_or(|end| observation.started_at < end)
                {
                    0
                } else {
                    frame
                };
                self.profile.measurements.record(
                    request_phase,
                    turnaround,
                    observation.turnaround_ms,
                );
                self.profile.measurements.record(
                    frame,
                    delivery,
                    observation.verified_at.elapsed().as_secs_f64() * 1000.,
                );
                for (name, count) in [
                    ("pending_requests", observation.pending_requests),
                    ("input_channel_depth", observation.queued_inputs),
                    ("update_channel_depth", observation.queued_updates),
                ] {
                    self.profile.measurements.record(frame, name, count as f64);
                }
            }
            if let Some(at) = self.latest_verified_snapshot {
                self.profile.measurements.record(
                    frame,
                    "sdk_verified_snapshot_age_ms",
                    at.elapsed().as_secs_f64() * 1000.,
                );
            }
            if let Some(age) = self.session.snapshot_age() {
                self.profile.measurements.record(
                    frame,
                    "rendered_snapshot_applied_age_ms",
                    age.as_secs_f64() * 1000.,
                );
            }
            if let Some(at) = self.last_input_handled.take() {
                self.profile.measurements.record(
                    frame,
                    "oldest_input_handler_to_next_submit_cpu_ms",
                    at.elapsed().as_secs_f64() * 1000.,
                );
            }
            let presentation = self.presenter.as_ref().unwrap().last_timings;
            for (name, value) in [
                ("surface_acquire_cpu_ms", presentation.acquire_ms),
                ("surface_configure_cpu_ms", Some(presentation.configure_ms)),
                ("presentation_encode_cpu_ms", presentation.encode_ms),
                (
                    "presentation_queue_submit_cpu_ms",
                    presentation.queue_submit_ms,
                ),
                ("present_call_cpu_ms", presentation.present_call_ms),
            ] {
                if let Some(value) = value {
                    self.profile
                        .measurements
                        .record(renderer.last_timings.frame, name, value);
                }
            }
            self.profile
                .render_submission_ms
                .add(render_started.elapsed().as_secs_f64() * 1000.);
        }
        if self.session.view().replica().latest().is_some() && self.record.is_some() {
            if self.record_started.is_none() {
                self.record_started = Some(now);
                self.recorded_world_start = Some(frame.time);
            }
            if now >= self.next_capture {
                self.recorder.as_mut().unwrap().submit(
                    renderer.capture_submitted(),
                    (now.duration_since(self.record_started.unwrap())
                        .as_secs_f64()
                        * 30.)
                        .floor() as u64,
                )?;
                self.next_capture = now + Duration::from_secs_f64(1. / 30.);
            }
        }
        Ok(())
    }
    fn demo(&mut self) {
        if !self.record.as_ref().is_some_and(|o| o.controller) {
            return;
        }
        let Some(hud) = self.session.hud() else {
            return;
        };
        if respawn_ready(
            self.record.as_ref().is_some_and(|o| o.respawn),
            hud.resources.hp,
            hud.life,
            self.session.respawn_attempts(),
            self.session.idle(),
        ) {
            self.session.respawn();
            return;
        }
        if !self.controlled() {
            return;
        }
        let state = self.session.view().replica().latest().unwrap();
        let hud = state.hud.as_ref().unwrap();
        if state.presentation.time < self.next_demo
            || hud.casting.is_some()
            || !self.session.input_idle()
        {
            return;
        }
        let order = [
            Ability::Shield,
            Ability::Fireball,
            Ability::Web,
            Ability::Grease,
            Ability::Light,
            Ability::Thunderwave,
            Ability::MistyStep,
            Ability::Bow,
            Ability::FireBolt,
            Ability::MagicMissile,
        ];
        let ability = order[self.demo_slot % order.len()];
        self.demo_slot += 1;
        self.next_demo = state.presentation.time + 2.;
        if self.demo_trace.len() < 64 {
            self.demo_trace.push(serde_json::json!({"time":state.presentation.time,"ability":ability.label(),"ready":hud.slots.iter().find(|s|s.ability==ability).map(|s|s.ready),"hp":hud.resources.hp}));
        }
        if !hud.slots.iter().any(|s| s.ability == ability && s.ready) {
            return;
        }
        let target = self.session.nearest_hostile();
        match self.session.view_mut().select_target(target) {
            Ok(()) => self.cast(ability),
            Err(error) => {
                if self.demo_trace.len() < 64 {
                    self.demo_trace
                        .push(serde_json::json!({"selection_refused":error}));
                }
            }
        }
    }
    fn finish_recording(&mut self) -> Result<(), String> {
        if let Some(recorder) = self.recorder.take() {
            let dropped = recorder.dropped;
            let stats = recorder.finish()?;
            let options = self.record.as_ref().unwrap();
            let session = &self.session;
            let view = session.view();
            let proof = serde_json::json!({"schema":"verse.remote.capture.v1","profile":self.profile.summary(),"frames":stats.frames,"sampled_frames":stats.sampled,"duplicated_frames":stats.duplicated,"dropped_capture_frames":dropped,"capture_measurements":stats.timings,"encoded_size":[1280,720],"world_start":self.recorded_world_start,"world_end":view.replica().latest().map(|s|s.presentation.time),"accepted_cast_commands":session.accepted_casts,"demo_trace":self.demo_trace,"demo_slot":self.demo_slot,"pending_commands":session.pending(),"final_status":session.status,"window_failure":self.error,"damage_events":session.damage_events,"dialogue_events":session.dialogue_events,"minimum_owned_hp":(self.min_hp != i32::MAX).then_some(self.min_hp),"programmatic_controller":options.controller,"programmatic_respawn":options.respawn,"programmatic_movement":options.movement,"programmatic_movement_frames":options.movement_frames,"respawn_attempts":session.respawn_attempts(),"owned_life_changes":session.life_changes,"native_dimensions":self.renderer.as_ref().map(|r|r.dimensions()),"device_profile":self.renderer.as_ref().map(|r|&r.device_profile),"capture_wall_seconds":self.record_started.map(|s|s.elapsed().as_secs_f64()),"wire_version":verse_world::service::wire::VERSION,"final_inventory":view.inventory(),"final_state":view.replica().latest()});
            std::fs::write(
                options.output.with_extension("json"),
                serde_json::to_vec_pretty(&proof)
                    .map_err(|_| "Cannot encode remote capture proof")?,
            )
            .map_err(|_| "Cannot write remote capture proof")?;
        }
        Ok(())
    }
    fn release_cursor(&mut self) {
        self.keys.clear();
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
    }
    fn release_pointer(&mut self) {
        self.release_cursor();
        self.session.release(&self.scene);
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Verse Engine — Remote chamber")
                            .with_inner_size(winit::dpi::LogicalSize::new(1280., 720.))
                            .with_resizable(self.record.is_none()),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let size = window.inner_size();
            let renderer = Renderer::new(
                self.pack.clone(),
                &self.dir,
                size.width.max(1),
                size.height.max(1),
                &self.atlas,
                &chamber::static_instances(
                    &self.pack,
                    verse_engine::source_position(self.scene.origin),
                ),
            )?;
            let presenter = renderer.attach_window(window.clone())?;
            if let Some(options) = &self.record {
                self.recorder = Some(super::remote_record::Recorder::open(
                    options,
                    renderer.dimensions(),
                )?);
            }
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.presenter = Some(presenter);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.controlled() {
                self.last_input_handled.get_or_insert_with(Instant::now);
                let session = &mut self.session;
                session
                    .controls
                    .motion([delta.0, delta.1], &mut session.yaw, &mut session.camera);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => self.release_pointer(),
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.window.as_ref().unwrap().inner_size();
                self.pointer = [
                    position.x as f32 * 720. / size.height.max(1) as f32,
                    position.y as f32 * 720. / size.height.max(1) as f32,
                ];
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.last_input_handled.get_or_insert_with(Instant::now);
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Released {
                        self.keys.remove(&key);
                    } else if !event.repeat {
                        self.keys.insert(key);
                        match key {
                            KeyCode::Escape => {
                                if self.session.view().interaction().is_some() {
                                    self.session.view_mut().close_giver();
                                } else if !self.character_panel.close() {
                                    event_loop.exit();
                                }
                            }
                            KeyCode::KeyB | KeyCode::KeyI | KeyCode::KeyL if self.in_play() => {
                                self.session.view_mut().close_giver();
                                self.character_panel.toggle(if key == KeyCode::KeyL {
                                    super::character_panel::Kind::Quests
                                } else {
                                    super::character_panel::Kind::Inventory
                                });
                                self.session.controls.left = false;
                                self.session.controls.right = false;
                                let window = self.window.as_ref().unwrap();
                                let _ = window.set_cursor_grab(CursorGrabMode::None);
                                window.set_cursor_visible(true);
                            }
                            KeyCode::PageDown | KeyCode::PageUp
                                if self.character_panel.kind.is_some() =>
                            {
                                let size = self.window.as_ref().unwrap().inner_size();
                                self.character_panel.page(
                                    key == KeyCode::PageDown,
                                    self.session.view().inventory(),
                                    720. * size.width as f32 / size.height.max(1) as f32,
                                    720.,
                                );
                            }
                            KeyCode::KeyF if self.unlocked() => {
                                let givers: Vec<_> = self
                                    .session
                                    .view()
                                    .quest_markers()
                                    .keys()
                                    .copied()
                                    .collect();
                                for giver in givers {
                                    if self.session.view_mut().open_giver(giver).is_ok() {
                                        self.giver_panel.reset();
                                        self.character_panel.close();
                                        self.release_pointer();
                                        break;
                                    }
                                }
                            }
                            KeyCode::Tab => {
                                self.session.view_mut().cycle_target();
                            }
                            KeyCode::Space if self.controlled() => {
                                self.send(Input::Command(Intent::Jump))
                            }
                            KeyCode::NumLock if self.controlled() => {
                                self.session.controls.autorun = !self.session.controls.autorun
                            }
                            _ => {
                                if let Some(ability) = key_ability(key) {
                                    self.cast(ability);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.unlocked() => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.,
                };
                let size = self.window.as_ref().unwrap().inner_size();
                let width = 720. * size.width as f32 / size.height.max(1) as f32;
                if self.character_panel.contains(self.pointer, width, 720.) {
                    if amount != 0. {
                        self.character_panel.page(
                            amount < 0.,
                            self.session.view().inventory(),
                            width,
                            720.,
                        );
                    }
                } else {
                    self.session.camera.zoom(amount);
                }
            }
            WindowEvent::MouseInput { state, button, .. } if self.unlocked() => {
                self.last_input_handled.get_or_insert_with(Instant::now);
                let down = state == ElementState::Pressed;
                let size = self.window.as_ref().unwrap().inner_size();
                let width = 720. * size.width as f32 / size.height.max(1) as f32;
                if down && self.giver_panel.contains(self.session.view(), self.pointer) {
                    if button == MouseButton::Left {
                        match self.giver_panel.click(self.session.view(), self.pointer) {
                            Some(super::giver_panel::Action::Accept(quest, giver)) => {
                                self.send(Input::AcceptQuest(quest, giver))
                            }
                            Some(super::giver_panel::Action::Claim(quest)) => {
                                self.send(Input::ClaimQuest(quest))
                            }
                            Some(super::giver_panel::Action::Close) => {
                                self.session.view_mut().close_giver()
                            }
                            None => {}
                        }
                    }
                    return;
                }
                if down && self.character_panel.contains(self.pointer, width, 720.) {
                    if button == MouseButton::Left {
                        self.character_panel.click(
                            self.pointer,
                            self.session.view().inventory(),
                            width,
                            720.,
                        );
                        if let Some((slot, item)) = self.character_panel.take_gear() {
                            self.send(Input::EquipGear(slot, item));
                        }
                        if let Some(outfit) = self.character_panel.take_equip() {
                            self.send(Input::EquipOutfit(outfit));
                        }
                        if let Some(item) = self.character_panel.take_use() {
                            self.send(Input::UseItem(item));
                        }
                        if let Some((quest, giver)) = self.character_panel.take_accept() {
                            self.send(Input::AcceptQuest(quest, giver));
                        }
                        if let Some(quest) = self.character_panel.take_claim() {
                            self.send(Input::ClaimQuest(quest));
                        }
                    }
                    return;
                }
                if button == MouseButton::Left && down {
                    if let Some(hud) = self.session.hud() {
                        let size = self.window.as_ref().unwrap().inner_size();
                        let width = 720. * size.width as f32 / size.height.max(1) as f32;
                        if overlay::owned_respawn_at(
                            hud,
                            true,
                            self.pointer[0],
                            self.pointer[1],
                            width,
                            720.,
                        ) {
                            self.send(Input::Respawn);
                            return;
                        }
                        if let Some(ability) = overlay::owned_action_at(
                            hud,
                            true,
                            self.pointer[0],
                            self.pointer[1],
                            width,
                            720.,
                        ) {
                            self.cast(ability);
                            return;
                        }
                    }
                }
                if self.controlled() && matches!(button, MouseButton::Left | MouseButton::Right) {
                    let session = &mut self.session;
                    session.controls.button(
                        button == MouseButton::Right,
                        down,
                        &mut session.yaw,
                        &session.camera,
                    );
                    let window = self.window.as_ref().unwrap();
                    if self.session.controls.looking() {
                        let _ = window
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
                        window.set_cursor_visible(false);
                    } else {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    self.fail(event_loop, error);
                } else if self
                    .record
                    .as_ref()
                    .zip(self.record_started)
                    .is_some_and(|(o, start)| start.elapsed().as_secs() >= o.seconds as u64)
                {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
fn key_ability(key: KeyCode) -> Option<Ability> {
    let index = match key {
        KeyCode::Digit1 => 0,
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        KeyCode::Digit6 => 5,
        KeyCode::Digit7 => 6,
        KeyCode::Digit8 => 7,
        KeyCode::Digit9 => 8,
        KeyCode::Digit0 => 9,
        _ => return None,
    };
    Some(Ability::ALL[index])
}
fn respawn_ready(
    enabled: bool,
    hp: i32,
    life: verse_engine::core::LifeId,
    attempted: &[verse_engine::core::LifeId],
    idle: bool,
) -> bool {
    enabled && hp == 0 && idle && attempted.len() < 128 && !attempted.contains(&life)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chamber_session::horizontal_aim;
    use verse_world::service::view::View;
    #[test]
    fn cinematic_camera_converts_scene_radians_to_native_degrees() {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        for time in [0., 40.] {
            let frame = scene.frame(time);
            let camera = chamber_session::authored_camera(&frame);
            assert!((camera.fov - 57.29578).abs() < 0.001);
            assert_eq!(camera.eye, frame.eye);
            assert_eq!(camera.target, frame.target);
        }
    }
    #[test]
    fn automated_respawn_requires_dead_owned_life_and_never_retries_it() {
        let life = verse_engine::core::LifeId {
            instance: 160,
            actor: 1,
            generation: 1,
        };
        assert!(respawn_ready(true, 0, life, &[], true));
        assert!(!respawn_ready(false, 0, life, &[], true));
        assert!(!respawn_ready(true, 1, life, &[], true));
        assert!(!respawn_ready(true, 0, life, &[], false));
        assert!(!respawn_ready(true, 0, life, &[life], true));
        let next = verse_engine::core::LifeId {
            generation: 2,
            ..life
        };
        assert!(respawn_ready(true, 0, next, &[life], true));
        assert!(!respawn_ready(true, 0, next, &vec![life; 128], true));
    }
    #[test]
    fn remote_casts_use_horizontal_directions_instead_of_world_aim_points() {
        let aim = horizontal_aim(
            Vec3::new(4., 3., -20.),
            Some(Vec3::new(7., 8., -16.)),
            Vec3::NEG_Z,
        );
        assert_eq!(aim.y, 0.);
        assert!((aim.length() - 1.).abs() < 0.00001);
        assert!(aim.x > 0. && aim.z > 0.);
        assert_eq!(
            horizontal_aim(Vec3::ZERO, Some(Vec3::ZERO), Vec3::Y),
            Vec3::NEG_Z
        );
        assert_eq!(
            horizontal_aim(Vec3::ZERO, None, Vec3::new(0., -0.5, -0.5)),
            Vec3::NEG_Z
        );
    }
    #[test]
    fn the_window_mounts_the_shared_session_and_its_recorder_reads_notes() {
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let atlas = super::super::original::atlas().unwrap();
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let (input, inputs, updates, output) = worker::channels();
        let session = Session::attached(View::new(160, 10., 0).unwrap(), input, output);
        let mut app = App::new(pack, atlas, scene, dir.path().into(), session);
        app.record = Some(super::super::remote_record::Options {
            output: dir.path().join("capture.mp4"),
            seconds: 30,
            controller: true,
            respawn: true,
            movement: false,
            movement_frames: false,
        });
        assert!(!app.controlled());
        app.demo();
        assert!(app.session.respawn_attempts().is_empty());
        app.note(Note::Bound {
            token: 7,
            at: Instant::now(),
        });
        app.note(Note::Outcome {
            token: Some(7),
            at: Instant::now(),
        });
        app.note(Note::Reset("teleport"));
        assert!(app.profile.bindings.is_empty());
        assert_eq!(app.profile.reset_observations, 1);
        drop(inputs);
        drop(updates);
        assert!(app.consume().is_err());
    }
    #[test]
    fn native_keys_cover_only_the_shared_host_kit() {
        let keys = [
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
        ];
        for (key, ability) in keys.into_iter().zip(Ability::ALL) {
            assert_eq!(key_ability(key), Some(ability));
        }
        assert_eq!(key_ability(KeyCode::F1), None);
    }
}
