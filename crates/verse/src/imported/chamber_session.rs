//! A chamber player without a window: the client worker on its own thread,
//! the replica and prediction it feeds, the inputs a stick or a keyboard
//! sends it, and the frame the engine renderer draws from it. The desktop
//! window and the phone's surface both mount this; neither owns the
//! transport or the camera rules.
use super::{chamber, controls, overlay};
use crate::{render, ui::Atlas};
use glam::Vec3;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        view::{Camera, View},
        worker::{self, Input, Update},
    },
};

/// Milliseconds between movement commands while the player is controlled.
const MOVE_INTERVAL: Duration = Duration::from_millis(33);
pub use controls::Held;

/// What one frame draws: the instances, the overlay, the lighting, and the
/// camera, assembled from the replica at one sample time.
pub struct Frame {
    pub view: render::View,
    pub instances: Vec<verse_engine::presentation::Instance>,
    pub ui: crate::ui::UiBatch,
    pub lighting: super::lighting::Lighting,
    /// The overlay's size for `ui`: the viewport in pixels.
    pub overlay: [f32; 2],
    /// Where the player stands, or the scene's focus before control.
    pub focus: Vec3,
}

/// Why a session ended, for the caller that leaves the chamber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// The caller asked for it, and the worker closed cleanly.
    Closed,
    /// The transport or the worker failed.
    Failed(String),
}

pub struct Session {
    view: View,
    input: mpsc::Sender<Input>,
    output: mpsc::Receiver<Update>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    pub controls: controls::ClassicControls,
    pub camera: controls::Camera,
    pub yaw: f32,
    last: Instant,
    next_move: Instant,
    received_at: Instant,
    owned_life: Option<verse_engine::core::LifeId>,
    owned_teleport: Option<f32>,
    pub status: String,
    pending: std::collections::VecDeque<(Option<Ability>, Option<u64>)>,
    prediction: verse_world::prediction::Local,
    input_token: u64,
    frame_cursor: Option<(verse_engine::core::LifeId, u64, u64)>,
    frame_bindings: std::collections::BTreeSet<u64>,
    frame_entry: Option<(verse_engine::core::LifeId, u64)>,
    frame_entry_pending: bool,
    respawn_attempts: Vec<verse_engine::core::LifeId>,
    /// Snapshots applied since the session started.
    pub snapshots: u64,
    /// Times the owned life changed while another was held: deaths and
    /// respawns the authority granted.
    pub life_changes: u64,
}

impl Session {
    /// Starts the client worker on its own thread over `runtime`, which must
    /// own the client's transport, and the local replica it fills.
    ///
    /// # Errors
    /// The scene is invalid or the replica cannot be made.
    pub fn start(
        client: Client,
        runtime: tokio::runtime::Runtime,
        scene: &Scene,
    ) -> Result<Self, String> {
        scene.validate()?;
        let instance = client.instance();
        let view = View::new(instance, 10., 0)?;
        let cursor = Cursor::new(instance);
        let (input, inputs, updates, output) = worker::channels();
        let (stop, stopping) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("chamber-worker".into())
            .spawn(move || {
                runtime.block_on(worker::run(
                    client,
                    cursor,
                    worker::NATIVE_CADENCE,
                    inputs,
                    updates,
                    stopping,
                ))
            })
            .map_err(|e| format!("Cannot start the chamber worker: {e}"))?;
        let now = Instant::now();
        Ok(Self {
            view,
            input,
            output,
            stop: Some(stop),
            thread: Some(thread),
            controls: controls::ClassicControls::default(),
            camera: controls::Camera::default(),
            yaw: std::f32::consts::PI,
            last: now,
            next_move: now,
            received_at: now,
            owned_life: None,
            owned_teleport: None,
            status: String::new(),
            pending: std::collections::VecDeque::new(),
            prediction: verse_world::prediction::Local::new(instance),
            input_token: 0,
            frame_cursor: None,
            frame_bindings: Default::default(),
            frame_entry: None,
            frame_entry_pending: false,
            respawn_attempts: vec![],
            snapshots: 0,
            life_changes: 0,
        })
    }

    /// Stops the worker and waits for it; the transport closes with it.
    pub fn stop(mut self) -> Stopped {
        self.end()
    }

    fn end(&mut self) -> Stopped {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        match self.thread.take().map(std::thread::JoinHandle::join) {
            None | Some(Ok(Ok(()))) => Stopped::Closed,
            Some(Ok(Err(error))) => Stopped::Failed(error),
            Some(Err(_)) => Stopped::Failed("Remote chamber worker panicked".into()),
        }
    }

    /// Whether the worker thread is still running.
    #[must_use]
    pub fn alive(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    #[must_use]
    pub fn view(&self) -> &View {
        &self.view
    }
    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    /// The scene's cut has passed or the authority handed the camera over.
    #[must_use]
    pub fn unlocked(&self, scene: &Scene) -> bool {
        self.view.camera_handoff()
            || self
                .view
                .replica()
                .latest()
                .is_some_and(|s| s.presentation.time >= scene.cut_at)
    }
    /// The player holds a living character.
    #[must_use]
    pub fn controlled(&self, scene: &Scene) -> bool {
        self.unlocked(scene) && self.hud().is_some_and(|h| h.resources.hp > 0)
    }
    /// The player's character is dead and may respawn.
    #[must_use]
    pub fn dead(&self) -> bool {
        self.hud().is_some_and(|h| h.resources.hp == 0)
    }
    #[must_use]
    pub fn hud(&self) -> Option<&verse_world::hud::Own> {
        self.view.replica().latest().and_then(|s| s.hud.as_ref())
    }
    /// The player's current position, predicted when movement is local.
    #[must_use]
    pub fn position(&self) -> Option<Vec3> {
        if let Some(pose) = self.prediction.pose() {
            return Some(pose.position);
        }
        let hud = self.hud()?;
        self.view
            .replica()
            .latest()?
            .presentation
            .actors
            .iter()
            .find(|p| verse_engine::core::LifeId::from(p.life) == hud.life)
            .map(|p| p.actor.position)
    }

    fn send(&mut self, input: Input) {
        if self.pending.len() >= 64 {
            self.status = "Input queue is busy".into();
            return;
        }
        if let Input::Command(intent @ (Intent::Move { .. } | Intent::Jump)) = &input {
            if self.prediction.movement_profile() == Some(verse_world::movement::Profile::Frames) {
                let Some(token) = self.input_token.checked_add(1) else {
                    self.status = "Input token exhausted".into();
                    return;
                };
                self.input_token = token;
                if let Err(message) = self.prediction.queue(token, intent.clone()) {
                    self.prediction.clear();
                    self.status = message;
                }
                let Some((life, epoch)) = self.prediction.context() else {
                    self.prediction.reject(token);
                    return;
                };
                match self.input.try_send(Input::TrackedCommand {
                    token,
                    life,
                    epoch,
                    intent: intent.clone(),
                }) {
                    Ok(()) => self.pending.push_back((None, Some(token))),
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        self.prediction.reject(token);
                        self.status = "Input queue is busy".into();
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        self.prediction.reject(token);
                        self.status = "Chamber connection stopped".into();
                    }
                }
                return;
            }
        }
        let ability = match &input {
            Input::Command(Intent::Cast { ability, .. }) => Some(*ability),
            _ => None,
        };
        match self.input.try_send(input) {
            Ok(()) => self.pending.push_back((ability, None)),
            Err(mpsc::error::TrySendError::Full(_)) => self.status = "Input queue is busy".into(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.status = "Chamber connection stopped".into();
            }
        }
    }

    /// Jumps, when the player is controlled.
    pub fn jump(&mut self, scene: &Scene) {
        if self.controlled(scene) {
            self.send(Input::Command(Intent::Jump));
        }
    }

    /// Asks for a new character once per death.
    pub fn respawn(&mut self) {
        let Some(hud) = self.hud() else {
            return;
        };
        if hud.resources.hp != 0
            || self.respawn_attempts.len() >= 128
            || self.respawn_attempts.contains(&hud.life)
        {
            return;
        }
        let life = hud.life;
        let before = self.pending.len();
        self.send(Input::Respawn);
        if self.pending.len() > before {
            self.respawn_attempts.push(life);
        }
    }

    /// Casts `ability` at the selected target, or ahead of the camera.
    pub fn cast(&mut self, scene: &Scene, ability: Ability) {
        if !self.controlled(scene) {
            return;
        }
        let target = self.view.target();
        let state = self.view.replica().latest().unwrap();
        let hud = state.hud.as_ref().unwrap();
        let player = state
            .presentation
            .actors
            .iter()
            .find(|p| verse_engine::core::LifeId::from(p.life) == hud.life)
            .map_or(Vec3::ZERO, |p| p.actor.position);
        let target_position = target.and_then(|life| {
            state
                .presentation
                .actors
                .iter()
                .find(|p| verse_engine::core::LifeId::from(p.life) == life)
                .map(|p| p.actor.position)
        });
        let aim = horizontal_aim(player, target_position, self.camera.direction());
        self.send(Input::Command(Intent::Cast {
            ability,
            target,
            aim: aim.to_array(),
        }));
    }

    /// Selects the nearest living hostile as the target.
    pub fn target_nearest(&mut self) {
        let Some(state) = self.view.replica().latest() else {
            return;
        };
        let Some(hud) = state.hud.as_ref() else {
            return;
        };
        let player = state
            .presentation
            .actors
            .iter()
            .find(|p| verse_engine::core::LifeId::from(p.life) == hud.life)
            .map_or(Vec3::ZERO, |p| p.actor.position);
        let target = state
            .presentation
            .actors
            .iter()
            .filter(|p| {
                p.health > 0
                    && p.visible
                    && p.actor.nameplate
                    && !p.actor.friendly
                    && p.actor.model != "adventurer"
            })
            .min_by(|a, b| {
                a.actor
                    .position
                    .distance_squared(player)
                    .total_cmp(&b.actor.position.distance_squared(player))
            })
            .map(|p| verse_engine::core::LifeId::from(p.life));
        if let Err(error) = self.view.select_target(target) {
            self.status = error;
        }
    }

    /// Applies every update the worker delivered.
    ///
    /// # Errors
    /// The update stream stopped or a snapshot is inconsistent.
    pub fn consume(&mut self, scene: &Scene) -> Result<(), String> {
        for _ in 0..worker::UPDATE_CAPACITY {
            match self.output.try_recv() {
                Ok(Update::Snapshot(r)) => {
                    self.view.push_snapshot(&r)?;
                    self.snapshots += 1;
                    self.received_at = Instant::now();
                    let state = self.view.replica().latest().unwrap();
                    let context = self
                        .view
                        .replica()
                        .control()
                        .map(|c| (c.life.into(), c.epoch));
                    let teleport = context.and_then(|(life, _)| {
                        state
                            .presentation
                            .actors
                            .iter()
                            .find(|p| verse_engine::core::LifeId::from(p.life) == life)
                            .and_then(|p| p.teleport_stamp)
                    });
                    let controlled = self.controlled(scene);
                    if teleport != self.owned_teleport {
                        self.prediction.clear();
                    }
                    self.owned_teleport = teleport;
                    if context != self.prediction.context() || !controlled {
                        self.prediction.clear();
                    }
                    if controlled {
                        if let (Some(baseline), Some(geometry)) =
                            (state.movement, state.collision.as_ref())
                        {
                            self.prediction
                                .observe(baseline, geometry, r.tick, r.request_id)?;
                            if baseline.profile == verse_world::movement::Profile::Frames
                                && self.frame_cursor.is_none_or(|(life, epoch, _)| {
                                    life != baseline.life || epoch != baseline.epoch
                                })
                            {
                                self.frame_cursor =
                                    Some((baseline.life, baseline.epoch, baseline.physics_step));
                            }
                        } else if self.prediction.context().is_some() {
                            if let Some(geometry) = state.collision.as_ref() {
                                self.prediction
                                    .update_geometry(geometry, r.tick, r.request_id)?;
                            }
                        }
                    }
                    let life = self.hud().map(|h| h.life);
                    if life != self.owned_life {
                        if self.owned_life.is_some() && life.is_some() {
                            self.life_changes += 1;
                        }
                        self.owned_life = life;
                        if let Some(pose) = self.view.replica().latest().and_then(|s| {
                            s.presentation
                                .actors
                                .iter()
                                .find(|p| Some(verse_engine::core::LifeId::from(p.life)) == life)
                        }) {
                            self.yaw = pose.actor.yaw;
                            self.camera.yaw = self.yaw;
                        }
                        self.controls.clear();
                    }
                }
                Ok(Update::Events { delivery, .. }) => self.view.push_events(&delivery)?,
                Ok(Update::Inventory(r)) => self.view.push_inventory(&r)?,
                Ok(Update::MovementSuperseded { token, replacement }) => {
                    self.pending.retain(|(_, pending)| *pending != Some(token));
                    if self.prediction.contains(token) {
                        if let Err(message) = self.prediction.supersede(token, replacement) {
                            self.prediction.clear();
                            self.status = message;
                        }
                    }
                }
                Ok(Update::FrameBound { token, binding }) => {
                    let submitted = self.frame_bindings.remove(&token);
                    match binding {
                        Ok(frame) => {
                            if self.prediction.context() == Some((frame.life, frame.epoch)) {
                                self.prediction.bind_movement_frame(&frame)?;
                            }
                        }
                        Err(message) => {
                            self.pending.retain(|(_, pending)| *pending != Some(token));
                            if submitted {
                                self.prediction.clear();
                                self.frame_cursor = None;
                            }
                            self.status = message;
                        }
                    }
                }
                Ok(Update::CommandBound { token, binding }) => match binding {
                    Ok(command) => {
                        if self.prediction.context() == Some((command.actor, command.epoch))
                            && self.prediction.contains(token)
                        {
                            if let Err(message) = self.prediction.bind(token, &command) {
                                self.prediction.clear();
                                self.status = message;
                            }
                        }
                    }
                    Err(message) => {
                        self.pending.retain(|(_, pending)| *pending != Some(token));
                        self.prediction.reject(token);
                        self.status = message;
                    }
                },
                Ok(Update::Outcome(r)) => {
                    use verse_world::service::wire::Reply;
                    if self.frame_entry_pending {
                        self.frame_entry_pending = false;
                        if matches!(r.body, Reply::Refused { .. }) {
                            self.frame_entry = None;
                        }
                    }
                    let (_, token) = self.pending.pop_front().unwrap_or_default();
                    match r.body {
                        Reply::Refused { message, .. } => {
                            if let Some(token) = token {
                                self.prediction.reject(token);
                            }
                            self.status = message;
                        }
                        Reply::QuestClaimed { .. } => self.status = "Quest completed".into(),
                        Reply::OutfitEquipped { .. } => self.status = "Outfit updated".into(),
                        Reply::GearEquipped { .. } => self.status = "Equipment updated".into(),
                        Reply::ItemUsed { .. } => self.status = "Item used".into(),
                        _ => {}
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.prediction.clear();
                    return Err("Chamber update stream stopped".into());
                }
            }
        }
        Ok(())
    }

    fn send_movement_interval(&mut self) -> Result<(), String> {
        let Some((life, epoch)) = self.prediction.context() else {
            self.frame_cursor = None;
            return Ok(());
        };
        if self.prediction.movement_profile() != Some(verse_world::movement::Profile::Frames) {
            self.frame_cursor = None;
            return Ok(());
        }
        let start = match self.frame_cursor {
            Some((old_life, old_epoch, start)) if life == old_life && epoch == old_epoch => start,
            _ => {
                let start = self.prediction.physics_step();
                self.frame_cursor = Some((life, epoch, start));
                start
            }
        };
        let end = self
            .prediction
            .movement_frame_limit()
            .ok_or("Movement interval time credit is unavailable")?;
        let steps = end
            .saturating_sub(start)
            .min(u64::from(verse_world::movement::frames::MAX_STEPS)) as u32;
        if steps < 4 || self.input.capacity() == 0 || self.pending.len() >= 64 {
            return Ok(());
        }
        let frame = self.prediction.movement_frame(start, steps)?;
        let token = self
            .input_token
            .checked_add(1)
            .ok_or("Input token exhausted")?;
        match self.input.try_send(Input::MovementFrame { token, frame }) {
            Ok(()) => {
                self.input_token = token;
                self.frame_cursor = Some((life, epoch, start + u64::from(steps)));
                self.frame_bindings.insert(token);
                self.pending.push_back((None, Some(token)));
            }
            Err(mpsc::error::TrySendError::Full(_)) => self.status = "Input queue is busy".into(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                return Err("Chamber connection stopped".into());
            }
        }
        Ok(())
    }

    /// Advances one frame: consumes updates, turns `held` into movement at
    /// the command cadence, and advances the local prediction. Returns the
    /// frame's duration in seconds.
    ///
    /// # Errors
    /// The connection stopped.
    pub fn step(&mut self, scene: &Scene, held: controls::Held) -> Result<f32, String> {
        self.consume(scene)?;
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        if !self.controlled(scene) {
            self.controls.clear();
        }
        let axes = self
            .controls
            .step(held, dt, &mut self.yaw, &mut self.camera);
        self.steer(scene, axes, now)?;
        Ok(dt)
    }

    /// Sends `axes` as the movement of this frame, when the player is
    /// controlled and the command cadence allows one.
    fn steer(&mut self, scene: &Scene, axes: [f32; 2], now: Instant) -> Result<(), String> {
        let controlled = self.controlled(scene);
        if controlled && self.pending.is_empty() {
            if let Some(baseline) = self.view.replica().latest().and_then(|s| s.movement) {
                let context = (baseline.life, baseline.epoch);
                if baseline.profile == verse_world::movement::Profile::Arrival
                    && baseline.character.support.is_some()
                    && baseline.held.axes(baseline.physics_step) == [0.; 2]
                    && self.frame_entry != Some(context)
                    && self
                        .input
                        .try_send(Input::BeginMovementFrames {
                            life: baseline.life,
                            epoch: baseline.epoch,
                        })
                        .is_ok()
                {
                    self.frame_entry = Some(context);
                    self.frame_entry_pending = true;
                    self.pending.push_back((None, None));
                }
            }
        }
        if controlled && now >= self.next_move && self.input.capacity() > 0 {
            self.send(Input::Command(Intent::Move {
                axes,
                yaw: self.yaw,
            }));
            self.next_move = now + MOVE_INTERVAL;
        }
        let dt = now.duration_since(self.last).as_secs_f64().min(0.1);
        if let Err(message) = self.prediction.advance(dt) {
            self.prediction.clear();
            self.status = message;
        }
        self.send_movement_interval()
    }

    /// Assembles the frame for a viewport of `size` pixels.
    ///
    /// # Errors
    /// The replica cannot be sampled.
    pub fn frame(
        &self,
        pack: &Pack,
        atlas: &Atlas,
        scene: &Scene,
        size: [u32; 2],
    ) -> Result<Frame, String> {
        let [w, h] = [size[0].max(1) as f32, size[1].max(1) as f32];
        let width = w;
        let origin = verse_engine::source_position(scene.origin);
        let now = Instant::now();
        let alpha = (now.duration_since(self.received_at).as_secs_f32()
            / worker::NATIVE_CADENCE.as_secs_f32())
        .clamp(0., 1.);
        let mut predicted = self.prediction.pose();
        if let Some(pose) = &mut predicted {
            pose.yaw = self.yaw;
        }
        let time = self
            .view
            .replica()
            .latest()
            .map_or(0., |s| s.presentation.time);
        let sampled = self.view.replica().sample(alpha)?;
        let time = sampled.as_ref().map_or(time, |s| s.time);
        let cinematic = scene.frame(time);
        let mut camera = Camera {
            eye: cinematic.eye,
            target: cinematic.target,
            fov: cinematic.fov.to_degrees(),
        };
        let mut focus = self
            .hud()
            .and_then(|h| {
                sampled
                    .as_ref()?
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == h.life)
            })
            .map_or(cinematic.target, |p| p.actor.position);
        if let Some(pose) = predicted {
            focus = pose.position;
        }
        let in_play = self.unlocked(scene) && self.hud().is_some();
        if in_play {
            camera.target = focus + Vec3::Y * 1.5;
            camera.eye = camera.target - self.camera.direction() * self.camera.distance.max(0.01);
            camera.fov = 60.;
        }
        let rendered = chamber::remote_scene_predicted(
            pack,
            &self.view,
            alpha,
            camera,
            origin,
            scene.collision_profile.as_deref() == Some(verse_world::playground::PROFILE),
            focus,
            predicted,
        )?;
        let projection = glam::Mat4::perspective_rh(camera.fov.to_radians(), w / h, 0.1, 1000.)
            * glam::Mat4::look_at_rh(camera.eye, camera.target, Vec3::Y);
        let mut ui = crate::ui::UiBatch::default();
        let mut instances = vec![];
        let mut lighting = chamber::lighting(origin);
        if let Some(rendered) = rendered {
            let heights = pack
                .models
                .iter()
                .map(|(id, m)| (id.clone(), m.height))
                .collect();
            ui = overlay::cinematic_with_focus(
                atlas,
                &rendered.frame,
                &heights,
                projection,
                width,
                h,
                &self.view.quest_markers(),
                focus,
                self.view.target(),
            );
            overlay::damage_numbers_from_values(
                &mut ui,
                atlas,
                &self.view.damage_numbers(alpha)?,
                rendered.frame.time,
                &rendered.frame,
                &heights,
                projection,
                width,
                h,
            );
            if let Some(hud) = self.hud() {
                overlay::owned_hud(
                    &mut ui,
                    atlas,
                    hud,
                    &rendered.frame,
                    self.unlocked(scene),
                    width,
                    h,
                )?;
            }
            if let Some(target) = self.view.target() {
                overlay::target_hud(&mut ui, atlas, &rendered.frame, target, width, h)?;
            }
            instances = rendered.instances;
            lighting = rendered.lighting;
        } else {
            ui.text(atlas, 20., 20., "Waiting for chamber state", [1.; 4]);
        }
        if !self.status.is_empty() {
            ui.text(atlas, 20., 126., &self.status, [1., 0.8, 0.4, 1.]);
        }
        Ok(Frame {
            view: render::View {
                view_proj: projection,
                eye: camera.eye,
            },
            instances,
            ui,
            lighting,
            overlay: [width, h],
            focus,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.end();
    }
}

/// The horizontal direction from `player` to `target`, or the camera's.
#[must_use]
pub fn horizontal_aim(player: Vec3, target: Option<Vec3>, camera: Vec3) -> Vec3 {
    let mut direction = target.map_or(camera, |target| target - player);
    direction.y = 0.;
    direction.normalize_or(Vec3::NEG_Z)
}
