//! A chamber player without a window: the client worker on its executor,
//! the replica and prediction it feeds, the inputs a stick or a keyboard
//! sends it, and the frame the engine renderer draws from it. The desktop
//! window and the phone's surface both mount this; neither owns the
//! transport, the prediction, or the camera rules.
use super::{chamber, controls, overlay};
use crate::{render, ui::Atlas};
use glam::Vec3;
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use verse_engine::{assets::Pack, core::LifeId, director::Scene};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        view::{Camera, View},
        wire::Reply,
        worker::{self, Input, Update},
    },
};
use web_time::Instant;

/// Time between movement commands while the player is controlled.
const MOVE_INTERVAL: Duration = Duration::from_millis(33);
/// Inputs sent but not yet answered; a full queue refuses new input.
const PENDING_LIMIT: usize = 64;
/// How long a dead player waits before asking again after a respawn that
/// did not return a new life, for example because something stood on the
/// spawn point (#10559).
pub const RESPAWN_RETRY: Duration = Duration::from_secs(2);
/// Notes held for an observer that has not drained them.
const NOTE_LIMIT: usize = 4096;
pub use controls::Held;

/// What one frame draws: the instances, the overlay, the lighting, and the
/// camera, assembled from the replica at one sample time.
pub struct Frame {
    pub view: render::View,
    pub instances: Vec<verse_engine::presentation::Instance>,
    pub ui: crate::ui::UiBatch,
    pub lighting: super::lighting::Lighting,
    /// The overlay's size for `ui`.
    pub overlay: [f32; 2],
    /// Where the player stands, or the scene's focus before control.
    pub focus: Vec3,
    /// The presentation time the frame sampled.
    pub time: f32,
}

impl Frame {
    /// Converts logical overlay coordinates to the renderer's physical pixels.
    pub fn scale_overlay(&mut self, target: [f32; 2]) -> Result<(), String> {
        if self
            .overlay
            .iter()
            .chain(&target)
            .any(|v| !v.is_finite() || *v <= 0. || *v > 8192.)
        {
            return Err("Invalid chamber overlay dimensions".into());
        }
        let scale = [target[0] / self.overlay[0], target[1] / self.overlay[1]];
        for vertex in &mut self.ui.vertices {
            vertex.pos[0] *= scale[0];
            vertex.pos[1] *= scale[1];
        }
        self.overlay = target;
        Ok(())
    }
}

/// Why a session ended, for the caller that leaves the chamber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// The caller asked for it, and the worker closed cleanly.
    Closed,
    /// The transport or the worker failed.
    Failed(String),
}

/// Captured correction context, serialized only when a recorder retains its detail.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CorrectionDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'static str>,
    tick: u64,
    request_id: u64,
    life: LifeId,
    epoch: u64,
    before: Vec3,
    after: Vec3,
    pending: usize,
    baseline: Option<verse_world::movement::Baseline>,
    timing_before: Option<verse_world::prediction::Timing>,
    timing_after: verse_world::prediction::Timing,
    #[serde(flatten)]
    snapshot: Option<SnapshotCorrection>,
}
#[derive(Debug, Clone, serde::Serialize)]
struct SnapshotCorrection {
    previous_life: LifeId,
    previous_epoch: u64,
    reset_reason: Option<&'static str>,
}

/// What prediction and transport did, for a recorder that profiles them.
/// A session collects notes only after [`Session::observe`].
#[derive(Debug, Clone)]
pub enum Note {
    /// Prediction restarted on a snapshot, for the stated reason.
    Reset(&'static str),
    /// A snapshot moved the predicted pose by `distance` metres.
    Correction {
        distance: f64,
        discontinuity: bool,
        detail: Box<CorrectionDetail>,
    },
    /// A retired input moved the predicted pose by `distance` metres.
    Retirement {
        distance: f64,
        detail: serde_json::Value,
    },
    /// The worker or the authority refused an input.
    Refusal(serde_json::Value),
    /// The worker bound tracked input `token` to the transport at `at`.
    Bound { token: u64, at: Instant },
    /// Tracked input `token` will never reach an outcome.
    Unbound(u64),
    /// The authority answered the oldest pending input at `at`.
    Outcome { token: Option<u64>, at: Instant },
}

pub struct Session {
    view: View,
    input: mpsc::Sender<Input>,
    output: mpsc::Receiver<Update>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    #[cfg(target_arch = "wasm32")]
    task: Option<verse_world::service::client_runtime::Task>,
    pub controls: controls::ClassicControls,
    pub camera: controls::Camera,
    pub yaw: f32,
    last: Instant,
    next_move: Instant,
    received_at: Instant,
    owned_life: Option<LifeId>,
    owned_teleport: Option<f32>,
    pub status: String,
    pending: VecDeque<(Option<Ability>, Option<u64>)>,
    prediction: verse_world::prediction::Local,
    prediction_failures: u64,
    last_prediction_failure: Option<String>,
    input_token: u64,
    frame_cursor: Option<(LifeId, u64, u64)>,
    frame_bindings: BTreeMap<u64, (LifeId, u64)>,
    frame_entry: Option<(LifeId, u64)>,
    frame_entry_pending: bool,
    respawn_attempts: Vec<LifeId>,
    respawn_asked: Option<Instant>,
    notes: Option<Vec<Note>>,
    /// Snapshots applied since the session started.
    pub snapshots: u64,
    /// Times the owned life changed while another was held: deaths and
    /// respawns the authority granted.
    pub life_changes: u64,
    /// Damage and dialogue events delivered.
    pub damage_events: u64,
    pub dialogue_events: u64,
    /// Accepted casts by ability label.
    pub accepted_casts: BTreeMap<String, u64>,
}

impl Session {
    /// Starts the client worker on its executor over `runtime`, which must
    /// own the client's transport, and the local replica it fills.
    ///
    /// # Errors
    /// The scene is invalid or the replica cannot be made.
    #[cfg(feature = "remote-chamber")]
    pub fn start(
        client: Client,
        runtime: tokio::runtime::Runtime,
        scene: &Scene,
    ) -> Result<Self, String> {
        Self::start_observed(client, runtime, scene, None)
    }

    /// [`Session::start`], with the worker reporting network telemetry to
    /// `observer` when one is given.
    ///
    /// # Errors
    /// The scene is invalid or the replica cannot be made.
    #[cfg(feature = "remote-chamber")]
    pub fn start_observed(
        client: Client,
        runtime: tokio::runtime::Runtime,
        scene: &Scene,
        observer: Option<worker::Observer>,
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
                runtime.block_on(async move {
                    match observer {
                        Some(observer) => {
                            worker::run_profiled(
                                client,
                                cursor,
                                worker::NATIVE_CADENCE,
                                inputs,
                                updates,
                                stopping,
                                observer,
                            )
                            .await
                        }
                        None => {
                            worker::run(
                                client,
                                cursor,
                                worker::NATIVE_CADENCE,
                                inputs,
                                updates,
                                stopping,
                            )
                            .await
                        }
                    }
                })
            })
            .map_err(|e| format!("Cannot start the chamber worker: {e}"))?;
        let mut session = Self::attached(view, input, output);
        session.stop = Some(stop);
        session.thread = Some(thread);
        Ok(session)
    }

    /// Starts the same worker on the browser's local executor.
    #[cfg(target_arch = "wasm32")]
    pub fn start_browser(client: Client, scene: &Scene) -> Result<Self, String> {
        scene.validate()?;
        let instance = client.instance();
        let view = View::new(instance, 10., 0)?;
        let cursor = Cursor::new(instance);
        let (input, inputs, updates, output) = worker::channels();
        let (stop, stopping) = oneshot::channel();
        let task = verse_world::service::client_runtime::spawn(async move {
            let _ = worker::run(
                client,
                cursor,
                worker::NATIVE_CADENCE,
                inputs,
                updates,
                stopping,
            )
            .await;
        });
        let mut session = Self::attached(view, input, output);
        session.stop = Some(stop);
        session.task = Some(task);
        Ok(session)
    }

    /// A session over channels a worker the caller runs already serves.
    #[must_use]
    pub fn attached(
        view: View,
        input: mpsc::Sender<Input>,
        output: mpsc::Receiver<Update>,
    ) -> Self {
        let now = Instant::now();
        let instance = view.instance();
        Self {
            view,
            input,
            output,
            stop: None,
            thread: None,
            #[cfg(target_arch = "wasm32")]
            task: None,
            controls: controls::ClassicControls::default(),
            camera: controls::Camera::default(),
            yaw: std::f32::consts::PI,
            last: now,
            next_move: now,
            received_at: now,
            owned_life: None,
            owned_teleport: None,
            status: String::new(),
            pending: VecDeque::new(),
            prediction: verse_world::prediction::Local::new(instance),
            prediction_failures: 0,
            last_prediction_failure: None,
            input_token: 0,
            frame_cursor: None,
            frame_bindings: BTreeMap::new(),
            frame_entry: None,
            frame_entry_pending: false,
            respawn_attempts: vec![],
            respawn_asked: None,
            notes: None,
            snapshots: 0,
            life_changes: 0,
            damage_events: 0,
            dialogue_events: 0,
            accepted_casts: BTreeMap::new(),
        }
    }

    /// Starts collecting [`Note`]s for [`Session::take_notes`].
    pub fn observe(&mut self) {
        self.notes.get_or_insert_with(Vec::new);
    }

    /// The notes collected since the last call.
    pub fn take_notes(&mut self) -> Vec<Note> {
        self.notes.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn note(&mut self, note: Note) {
        if let Some(notes) = &mut self.notes {
            if notes.len() < NOTE_LIMIT {
                notes.push(note);
            }
        }
    }

    /// Local input-to-prediction clock delay; this excludes display latency.
    pub fn prediction_delay_steps(&self) -> u64 {
        self.prediction.prediction_delay_steps()
    }
    pub fn prediction_embedding_deferrals(&self) -> u64 {
        self.prediction.embedding_deferrals()
    }
    pub fn prediction_separating_steps(&self) -> u64 {
        self.prediction.separating_steps()
    }
    pub fn prediction_embedding_diagnostic(&self) -> Option<&str> {
        self.prediction.last_embedding()
    }

    /// Counts local frames held at the bounded prediction horizon.
    pub fn prediction_horizon_pauses(&self) -> u64 {
        self.prediction.horizon_pauses()
    }
    /// Returns local prediction errors that discarded a clock or pending input.
    pub fn prediction_failures(&self) -> u64 {
        self.prediction_failures
    }
    /// Retains the most recent local prediction error independently of UI status.
    pub fn last_prediction_failure(&self) -> Option<&str> {
        self.last_prediction_failure.as_deref()
    }
    fn discard_failed_prediction(&mut self, message: String) {
        self.prediction_failures = self.prediction_failures.saturating_add(1);
        self.last_prediction_failure = Some(message.clone());
        self.prediction.clear();
        self.status = message;
    }

    /// Stops the worker and waits for it; the transport closes with it.
    pub fn stop(mut self) -> Stopped {
        self.close()
    }

    /// [`Session::stop`] for an owner that keeps the session to read its
    /// final state. A second call reports [`Stopped::Closed`].
    pub fn close(&mut self) -> Stopped {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(task) = self.task.take() {
            task.abort();
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
        self.thread
            .as_ref()
            .map_or_else(|| !self.output.is_closed(), |t| !t.is_finished())
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
    /// The life the player holds, as of the last snapshot.
    #[must_use]
    pub fn owned_life(&self) -> Option<LifeId> {
        self.owned_life
    }
    /// The lives a respawn was already asked for.
    #[must_use]
    pub fn respawn_attempts(&self) -> &[LifeId] {
        &self.respawn_attempts
    }
    /// No input waits for the worker or the authority.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.pending.is_empty() && self.input_idle()
    }
    /// The worker has taken every input sent to it.
    #[must_use]
    pub fn input_idle(&self) -> bool {
        self.input.capacity() == worker::INPUT_CAPACITY
    }
    /// Inputs sent but not yet answered.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    /// Movement goes to the authority as frame intervals.
    #[must_use]
    pub fn movement_frames(&self) -> bool {
        self.prediction.movement_profile() == Some(verse_world::movement::Profile::Frames)
    }
    /// The time since the last snapshot applied, once there is one.
    #[must_use]
    pub fn snapshot_age(&self) -> Option<Duration> {
        self.view
            .replica()
            .latest()
            .map(|_| self.received_at.elapsed())
    }
    /// The player's current position, predicted when movement is local.
    #[must_use]
    pub fn position(&self) -> Option<Vec3> {
        if let Some(pose) = self.prediction.pose() {
            return Some(pose.position);
        }
        let hud = self.hud()?;
        self.actor(hud.life)
    }

    pub fn animation_support(&self) -> verse_world::animation_support::Queries<'_> {
        self.prediction.animation_support()
    }
    pub fn animation_controls(
        &self,
        instances: &[verse_engine::presentation::Instance],
    ) -> Vec<verse_engine::locomotion::Control> {
        let Some(life) = self.owned_life else {
            return Vec::new();
        };
        let Some(body) = instances.iter().find(|i| i.actor == Some(life)) else {
            return Vec::new();
        };
        let direction = self
            .view
            .target()
            .and_then(|target| self.actor(target))
            .map_or(self.camera.direction(), |target| {
                target - body.transform.w_axis.truncate()
            });
        let horizontal = Vec3::new(direction.x, 0., direction.z).normalize_or_zero();
        let forward = body.transform.transform_vector3(Vec3::X);
        let forward = Vec3::new(forward.x, 0., forward.z).normalize_or_zero();
        let yaw = forward
            .cross(horizontal)
            .y
            .atan2(forward.dot(horizontal))
            .clamp(-1.2, 1.2);
        let pitch = direction
            .y
            .atan2(Vec3::new(direction.x, 0., direction.z).length())
            .clamp(-1.2, 1.2);
        vec![verse_engine::locomotion::Control {
            life,
            aim: verse_engine::locomotion::Aim { yaw, pitch },
        }]
    }

    fn actor(&self, life: LifeId) -> Option<Vec3> {
        self.view
            .replica()
            .latest()?
            .presentation
            .actors
            .iter()
            .find(|p| LifeId::from(p.life) == life)
            .map(|p| p.actor.position)
    }

    /// Sends `input` to the worker. Movement and jumps under a control
    /// context are tracked so prediction can retire them; under movement
    /// frames they stay local until an interval carries them.
    pub fn send(&mut self, input: Input) {
        if self.pending.len() >= PENDING_LIMIT {
            self.status = "Input queue is busy".into();
            return;
        }
        if let Input::Command(intent @ (Intent::Move { .. } | Intent::Jump)) = &input {
            if self.prediction.movement_profile() == Some(verse_world::movement::Profile::Frames) {
                if self.prediction.pending() >= verse_world::prediction::CAPACITY {
                    self.status = "Input queue is busy".into();
                    return;
                }
                let Some(token) = self.input_token.checked_add(1) else {
                    self.status = "Input token exhausted".into();
                    return;
                };
                self.input_token = token;
                if let Err(message) = self.prediction.queue(token, intent.clone()) {
                    self.discard_failed_prediction(message);
                }
                return;
            }
            if self.frame_entry.is_some() && self.frame_entry == self.prediction.context() {
                return;
            }
        }
        let (input, predicted) = match input {
            Input::Command(intent @ (Intent::Move { .. } | Intent::Jump))
                if self.prediction.context().is_some()
                    && self.view.replica().control().is_some() =>
            {
                let control = self.view.replica().control().unwrap();
                let Some(token) = self.input_token.checked_add(1) else {
                    self.status = "Input token exhausted".into();
                    return;
                };
                self.input_token = token;
                (
                    Input::TrackedCommand {
                        token,
                        life: control.life.into(),
                        epoch: control.epoch,
                        intent: intent.clone(),
                    },
                    Some((token, intent)),
                )
            }
            input => (input, None),
        };
        let ability = match &input {
            Input::Command(Intent::Cast { ability, .. }) => Some(*ability),
            _ => None,
        };
        match self.input.try_send(input) {
            Ok(()) => {
                self.status.clear();
                let token = predicted.as_ref().map(|p| p.0);
                if let Some((token, intent)) = predicted {
                    if let Err(message) = self.prediction.queue(token, intent) {
                        self.discard_failed_prediction(message);
                    }
                }
                self.pending.push_back((ability, token));
            }
            Err(mpsc::error::TrySendError::Full(_)) => self.status = "Input queue is busy".into(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.status = "Chamber connection stopped".into();
            }
        }
    }

    /// Clears held controls and, while controlled, stops the character.
    pub fn release(&mut self, scene: &Scene) {
        self.controls.clear();
        if self.controlled(scene) {
            self.send(Input::Command(Intent::Move {
                axes: [0.; 2],
                yaw: self.yaw,
            }));
        }
    }

    /// Jumps, when the player is controlled.
    pub fn jump(&mut self, scene: &Scene) {
        if self.controlled(scene) {
            self.send(Input::Command(Intent::Jump));
        }
    }

    /// Asks for a new character once per death, and again every
    /// [`RESPAWN_RETRY`] while that life stays dead: the authority refuses a
    /// respawn whose spawn point is obstructed.
    pub fn respawn(&mut self) {
        let Some(hud) = self.hud() else {
            return;
        };
        if hud.resources.hp != 0 {
            return;
        }
        let life = hud.life;
        let retry = self.respawn_attempts.last() == Some(&life)
            && self
                .respawn_asked
                .is_some_and(|at| at.elapsed() >= RESPAWN_RETRY);
        if !retry && (self.respawn_attempts.len() >= 128 || self.respawn_attempts.contains(&life)) {
            return;
        }
        let before = self.pending.len();
        self.send(Input::Respawn);
        if self.pending.len() > before {
            if !retry {
                self.respawn_attempts.push(life);
            }
            self.respawn_asked = Some(Instant::now());
        }
    }

    /// Casts `ability` at the selected target, or ahead of the camera.
    pub fn cast(&mut self, scene: &Scene, ability: Ability) {
        if !self.controlled(scene) {
            return;
        }
        let target = self.view.target();
        let Some(life) = self.hud().map(|h| h.life) else {
            return;
        };
        let player = self.actor(life).unwrap_or(Vec3::ZERO);
        let target_position = target.and_then(|life| self.actor(life));
        let aim = horizontal_aim(player, target_position, self.camera.direction());
        self.send(Input::Command(Intent::Cast {
            ability,
            target,
            aim: aim.to_array(),
        }));
    }

    /// The nearest living, visible hostile to the player.
    #[must_use]
    pub fn nearest_hostile(&self) -> Option<LifeId> {
        let state = self.view.replica().latest()?;
        let hud = state.hud.as_ref()?;
        let player = self.actor(hud.life).unwrap_or(Vec3::ZERO);
        state
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
            .map(|p| LifeId::from(p.life))
    }

    /// Selects the nearest living hostile as the target.
    pub fn target_nearest(&mut self) {
        let target = self.nearest_hostile();
        if let Err(error) = self.view.select_target(target) {
            self.status = error;
        }
    }

    fn observe_movement_credit(
        &mut self,
        control: Option<&verse_world::service::wire::Control>,
    ) -> Result<(), String> {
        if let Some(control) = control.filter(|c| {
            self.prediction.context() == Some((c.life.into(), c.epoch))
                && self.prediction.movement_profile()
                    == Some(verse_world::movement::Profile::Frames)
        }) {
            self.prediction.movement_credit(
                control.life.into(),
                control.epoch,
                control.credit_step,
            )?;
        }
        Ok(())
    }

    /// Applies every update the worker delivered.
    ///
    /// # Errors
    /// The update stream stopped or a snapshot is inconsistent.
    pub fn consume(&mut self, scene: &Scene) -> Result<(), String> {
        for _ in 0..worker::UPDATE_CAPACITY {
            match self.output.try_recv() {
                Ok(Update::Snapshot(r)) => self.snapshot(scene, &r)?,
                Ok(Update::Events {
                    delivery, control, ..
                }) => {
                    self.view.push_events(&delivery)?;
                    self.observe_movement_credit(control.as_ref())?;
                    for event in &delivery.events {
                        match event.kind {
                            verse_world::events::Kind::Damage { .. } => self.damage_events += 1,
                            verse_world::events::Kind::Dialogue { .. } => self.dialogue_events += 1,
                            _ => {}
                        }
                    }
                }
                Ok(Update::MovementCredit(r)) => {
                    self.observe_movement_credit(r.control.as_ref())?;
                }
                Ok(Update::Inventory(r)) => {
                    self.view.push_inventory(&r)?;
                    self.observe_movement_credit(r.control.as_ref())?;
                }
                Ok(Update::MovementSuperseded { token, replacement }) => {
                    let before = self.prediction.pose();
                    self.pending.retain(|(_, pending)| *pending != Some(token));
                    if self.prediction.contains(token) {
                        if let Err(message) = self.prediction.supersede(token, replacement) {
                            self.discard_failed_prediction(message);
                        }
                    }
                    if self.notes.is_some() {
                        if let (Some(before), Some(after)) = (before, self.prediction.pose()) {
                            if before.life == after.life && before.epoch == after.epoch {
                                let detail = serde_json::json!({"token":token,"replacement":replacement,
                                    "reason":"Unsent movement superseded","pending":self.prediction.pending()});
                                self.note(Note::Retirement {
                                    distance: f64::from(before.position.distance(after.position)),
                                    detail,
                                });
                            }
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
                            self.note(Note::Bound {
                                token,
                                at: Instant::now(),
                            });
                        }
                        Err(message) => {
                            self.pending.retain(|(_, pending)| *pending != Some(token));
                            let detail = serde_json::json!({"stage":"frame_binding",
                                "token":token,"message":message,"submitted_context":submitted,
                                "context":self.prediction.context()});
                            self.note(Note::Refusal(detail));
                            if submitted.is_none() || submitted == self.prediction.context() {
                                self.prediction.clear();
                                self.frame_cursor = None;
                            }
                            self.status = message;
                        }
                    }
                }
                Ok(Update::CommandBound { token, binding }) => match binding {
                    Ok(command) => {
                        self.note(Note::Bound {
                            token,
                            at: Instant::now(),
                        });
                        if self.prediction.context() == Some((command.actor, command.epoch))
                            && self.prediction.contains(token)
                        {
                            if let Err(message) = self.prediction.bind(token, &command) {
                                self.discard_failed_prediction(message);
                            }
                        }
                    }
                    Err(message) => {
                        let before = self.prediction.pose();
                        self.pending.retain(|(_, pending)| *pending != Some(token));
                        self.prediction.reject(token);
                        if self.notes.is_some() {
                            self.prediction.advance(0.)?;
                            if let (Some(before), Some(after)) = (before, self.prediction.pose()) {
                                if before.life == after.life && before.epoch == after.epoch {
                                    let detail = serde_json::json!({"token":token,"reason":message,
                                        "life":after.life,"epoch":after.epoch,
                                        "before":before.position,"after":after.position,
                                        "pending":self.prediction.pending()});
                                    self.note(Note::Retirement {
                                        distance: f64::from(
                                            before.position.distance(after.position),
                                        ),
                                        detail,
                                    });
                                }
                            }
                        }
                        self.note(Note::Unbound(token));
                        self.status = message;
                    }
                },
                Ok(Update::Outcome(r)) => {
                    self.confirm_applied_movement(&r)?;
                    self.observe_movement_credit(r.control.as_ref())?;
                    if self.frame_entry_pending {
                        self.frame_entry_pending = false;
                        if matches!(r.body, Reply::Refused { .. }) {
                            self.frame_entry = None;
                        }
                    }
                    let (ability, token) = self.pending.pop_front().unwrap_or_default();
                    self.note(Note::Outcome {
                        token,
                        at: Instant::now(),
                    });
                    match r.body {
                        Reply::Refused { message, .. } => {
                            if let Some(token) = token {
                                self.prediction.reject(token);
                            }
                            let detail = serde_json::json!({"stage":"authority_outcome",
                                "token":token,"ability":ability.map(|a|a.label()),"message":message,
                                "request_id":r.request_id,"tick":r.tick,"control":r.control,
                                "context":self.prediction.context()});
                            self.note(Note::Refusal(detail));
                            self.status = message;
                        }
                        Reply::Accepted => {
                            if let Some(ability) = ability {
                                *self
                                    .accepted_casts
                                    .entry(ability.label().into())
                                    .or_default() += 1;
                            }
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

    fn confirm_applied_movement(
        &mut self,
        r: &verse_world::service::wire::Response,
    ) -> Result<(), String> {
        let Some(baseline) = r.control.as_ref().and_then(|c| c.applied_movement) else {
            return Ok(());
        };
        if let Some(control) = &r.control {
            self.prediction.observe_dynamic_poses(&control.dynamic)?;
        }
        let before = self.prediction.pose();
        let timing = self.notes.as_ref().map(|_| self.prediction.timing());
        if !self
            .prediction
            .observe_applied(baseline, r.tick, r.request_id)?
        {
            return Ok(());
        }
        if self.notes.is_some() {
            self.prediction.advance(0.)?;
            if let (Some(before), Some(after)) = (before, self.prediction.pose()) {
                let detail = Box::new(CorrectionDetail {
                    stage: Some("applied_movement_confirmation"),
                    tick: r.tick,
                    request_id: r.request_id,
                    life: after.life,
                    epoch: after.epoch,
                    before: before.position,
                    after: after.position,
                    pending: self.prediction.pending(),
                    baseline: Some(baseline),
                    timing_before: timing,
                    timing_after: self.prediction.timing(),
                    snapshot: None,
                });
                self.note(Note::Correction {
                    distance: f64::from(before.position.distance(after.position)),
                    discontinuity: false,
                    detail,
                });
            }
        }
        Ok(())
    }

    fn snapshot(
        &mut self,
        scene: &Scene,
        r: &verse_world::service::wire::Response,
    ) -> Result<(), String> {
        let previous_pose = self.prediction.pose();
        let previous_timing = self.notes.as_ref().map(|_| self.prediction.timing());
        let previous_control = self
            .view
            .replica()
            .control()
            .map(|c| (LifeId::from(c.life), c.epoch));
        self.view.push_snapshot(r)?;
        self.snapshots += 1;
        self.received_at = Instant::now();
        let state = self.view.replica().latest().unwrap();
        let context = self
            .view
            .replica()
            .control()
            .map(|c| (c.life.into(), c.epoch));
        if previous_control != context {
            // A verified handoff retires messages about the previous control.
            self.status.clear();
        }
        let teleport = context.and_then(|(life, _)| {
            state
                .presentation
                .actors
                .iter()
                .find(|p| LifeId::from(p.life) == life)
                .and_then(|p| p.teleport_stamp)
        });
        let controlled = self.controlled(scene);
        // Body projection retains its admission prefix. Completed travel can be
        // newer than that body; an older body must not undo a verified confirmation.
        let movement = state
            .movement
            .into_iter()
            .chain(r.control.as_ref().and_then(|c| c.applied_movement))
            .chain(self.prediction.confirmed().filter(|b| {
                controlled && teleport == self.owned_teleport && context == Some((b.life, b.epoch))
            }))
            .max_by_key(|b| (b.physics_step, b.applied_sequence));
        let collision = state.collision.clone();
        let reset_reason = if !controlled {
            "unavailable_or_dead"
        } else if context.map(|c| c.0) != self.prediction.context().map(|c| c.0) {
            "life_or_reconnect"
        } else if teleport != self.owned_teleport {
            "teleport"
        } else {
            "control_epoch"
        };
        let discontinuity =
            teleport != self.owned_teleport || context != self.prediction.context() || !controlled;
        if discontinuity && previous_pose.is_some() {
            self.note(Note::Reset(reset_reason));
        }
        if teleport != self.owned_teleport {
            self.prediction.clear();
        }
        self.owned_teleport = teleport;
        if context != self.prediction.context() || !controlled {
            self.prediction.clear();
        }
        if !controlled {
            if let Some(geometry) = collision.as_ref() {
                self.prediction.observe_animation_geometry(geometry)?;
            }
        }
        if controlled {
            if let (Some(baseline), Some(geometry)) = (movement, collision.as_ref()) {
                self.prediction
                    .observe(baseline, geometry, r.tick, r.request_id)?;
                if baseline.profile == verse_world::movement::Profile::Frames {
                    let control = r
                        .control
                        .as_ref()
                        .ok_or("Movement snapshot has no control")?;
                    self.prediction.grant_world_credit(
                        baseline.life,
                        baseline.epoch,
                        control.credit_step,
                    )?;
                }
                if baseline.profile == verse_world::movement::Profile::Frames
                    && self.frame_cursor.is_none_or(|(life, epoch, _)| {
                        life != baseline.life || epoch != baseline.epoch
                    })
                {
                    self.frame_cursor =
                        Some((baseline.life, baseline.epoch, baseline.physics_step));
                }
            } else if self.prediction.context().is_some() {
                if let Some(geometry) = collision.as_ref() {
                    self.prediction
                        .update_geometry(geometry, r.tick, r.request_id)?;
                }
            }
        }
        if self.notes.is_some() {
            self.prediction.advance(0.)?;
            if let (Some(before), Some(after)) = (previous_pose, self.prediction.pose()) {
                let detail = Box::new(CorrectionDetail {
                    stage: None,
                    tick: r.tick,
                    request_id: r.request_id,
                    life: after.life,
                    epoch: after.epoch,
                    before: before.position,
                    after: after.position,
                    pending: self.prediction.pending(),
                    baseline: movement,
                    timing_before: previous_timing,
                    timing_after: self.prediction.timing(),
                    snapshot: Some(SnapshotCorrection {
                        previous_life: before.life,
                        previous_epoch: before.epoch,
                        reset_reason: if discontinuity {
                            Some(reset_reason)
                        } else {
                            None
                        },
                    }),
                });
                self.note(Note::Correction {
                    distance: f64::from(before.position.distance(after.position)),
                    discontinuity,
                    detail,
                });
            }
        }
        let life = self.hud().map(|h| h.life);
        if life != self.owned_life {
            if self.owned_life.is_some() && life.is_some() {
                self.life_changes += 1;
            }
            self.owned_life = life;
            if let Some(yaw) = self.view.replica().latest().and_then(|s| {
                s.presentation
                    .actors
                    .iter()
                    .find(|p| Some(LifeId::from(p.life)) == life)
                    .map(|p| p.actor.yaw)
            }) {
                self.yaw = yaw;
                self.camera.yaw = yaw;
            }
            self.release(scene);
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
        let mut start = match self.frame_cursor {
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
        // One request carries the complete available history within one authority work budget.
        let steps = end
            .saturating_sub(start)
            .min(u64::from(verse_world::movement::frames::MAX_STEPS)) as u32;
        // Keep local prediction immediate while amortizing durable ordered requests.
        // The complete history retains direction changes, lease expiries, and jump edges.
        // Flush a credit-limited remainder; local-only time still waits for a full batch.
        if steps == 0
            || (steps < verse_world::movement::frames::SEND_STEPS
                && end == self.prediction.physics_step())
            || self.input.capacity() == 0
            || self.pending.len() >= PENDING_LIMIT
        {
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
                start += u64::from(steps);
                self.frame_cursor = Some((life, epoch, start));
                self.frame_bindings.insert(token, (life, epoch));
                self.pending.push_back((None, Some(token)));
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.status = "Input queue is busy".into();
            }
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
    pub fn step(&mut self, scene: &Scene, held: Held) -> Result<f32, String> {
        self.consume(scene)?;
        let dt = self.tick().as_secs_f32().min(0.1);
        if !self.controlled(scene) {
            self.controls.clear();
        }
        let axes = self.turn(held, dt);
        self.steer(scene, axes, dt, true)?;
        Ok(dt)
    }

    /// The time since the previous call, which starts this frame.
    pub fn tick(&mut self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last);
        self.last = now;
        elapsed
    }

    /// Applies `held` to the controls for `dt` seconds and returns the
    /// movement axes.
    pub fn turn(&mut self, held: Held, dt: f32) -> [f32; 2] {
        self.controls
            .step(held, dt, &mut self.yaw, &mut self.camera)
    }

    /// Sends `axes` as this frame's movement when the player is controlled,
    /// `movement` allows it, and the command cadence allows one; then
    /// advances prediction by `dt` and sends any complete movement interval.
    ///
    /// # Errors
    /// The connection stopped.
    pub fn steer(
        &mut self,
        scene: &Scene,
        axes: [f32; 2],
        dt: f32,
        movement: bool,
    ) -> Result<(), String> {
        let now = Instant::now();
        let controlled = self.controlled(scene);
        // Recover elapsed history before recording this frame's new controls.
        self.prediction.recover_world_credit()?;
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
        if controlled && movement && now >= self.next_move && self.input.capacity() > 0 {
            self.send(Input::Command(Intent::Move {
                axes,
                yaw: self.yaw,
            }));
            self.next_move = now + MOVE_INTERVAL;
        }
        if let Err(message) = self.prediction.advance(f64::from(dt).min(0.1)) {
            self.discard_failed_prediction(message);
        }
        self.send_movement_interval()
    }

    /// Assembles the frame for a viewport of `size` pixels, with the
    /// overlay in pixels.
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
        let overlay = [size[0].max(1) as f32, size[1].max(1) as f32];
        self.frame_in(pack, atlas, scene, size, overlay)
    }

    /// Assembles the frame for a viewport of `size` pixels, with the
    /// overlay laid out in an `overlay`-sized space of the same aspect.
    ///
    /// # Errors
    /// The replica cannot be sampled.
    pub fn frame_in(
        &self,
        pack: &Pack,
        atlas: &Atlas,
        scene: &Scene,
        size: [u32; 2],
        overlay: [f32; 2],
    ) -> Result<Frame, String> {
        let aspect = size[0].max(1) as f32 / size[1].max(1) as f32;
        let [width, h] = overlay;
        let origin = verse_engine::source_position(scene.origin);
        let alpha = (self.received_at.elapsed().as_secs_f32()
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
        let mut camera = authored_camera(&cinematic);
        let mut focus = self
            .hud()
            .and_then(|h| {
                sampled
                    .as_ref()?
                    .actors
                    .iter()
                    .find(|p| LifeId::from(p.life) == h.life)
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
        let projection = glam::Mat4::perspective_rh(camera.fov.to_radians(), aspect, 0.1, 1000.)
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
            overlay,
            focus,
            time,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// The scene director's camera, with the field of view in degrees.
#[must_use]
pub fn authored_camera(frame: &verse_engine::director::Frame) -> Camera {
    Camera {
        eye: frame.eye,
        target: frame.target,
        fov: frame.fov.to_degrees(),
    }
}

/// The horizontal direction from `player` to `target`, or the camera's.
#[must_use]
pub fn horizontal_aim(player: Vec3, target: Option<Vec3>, camera: Vec3) -> Vec3 {
    let mut direction = target.map_or(camera, |target| target - player);
    direction.y = 0.;
    direction.normalize_or(Vec3::NEG_Z)
}

#[cfg(test)]
#[path = "chamber_session_tests.rs"]
mod tests;
