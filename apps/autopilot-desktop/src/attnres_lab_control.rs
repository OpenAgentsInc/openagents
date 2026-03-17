use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::app_state::{
    AttnResLabBlockSummary, AttnResLabInferenceSummary, AttnResLabMetricPoint, AttnResLabPaneState,
    AttnResLabPlaybackState, AttnResLabSnapshot, AttnResLabSublayerSnapshot, AttnResLabViewMode,
    PaneLoadState, PaneStatusAccess,
};
use psionic_models::{
    AttnResConfig, AttnResCpuReferenceModel, AttnResDiagnosticsSnapshot, AttnResSublayerKind,
};
use psionic_runtime::{
    AttnResHiddenParityReport, AttnResLogitParityReport, AttnResTwoPhaseParityBudget,
    AttnResTwoPhaseParityStatus, compare_attnres_hidden_two_phase_parity,
    compare_attnres_logit_two_phase_parity,
};
use psionic_serve::{
    AttnResTextGenerationOutcome, AttnResTextGenerationRequest, AttnResTextGenerationResponse,
    LocalAttnResTextGenerationService,
};
use psionic_train::{
    AttnResTinyTrainingConfig, AttnResTinyTrainingCorpus, AttnResTinyTrainingLifecycleStatus,
    AttnResTinyTrainingRunner,
};
use serde::{Deserialize, Serialize};

const LIVE_SOURCE_BADGE: &str = "psionic.attnres";
const ATTNRES_LAB_SCHEMA_VERSION: u16 = 2;
const ATTNRES_LAB_STATE_FILENAME: &str = "attnres-lab.json";
const ATTNRES_EVENT_LIMIT: usize = 12;
const DEFAULT_SPEED_MULTIPLIER: usize = 3;
const MIN_SPEED_MULTIPLIER: usize = 1;
const MAX_SPEED_MULTIPLIER: usize = 5;

static DESKTOP_ATTNRES_LAB_CONTROLLER: OnceLock<Mutex<DesktopAttnResLabController>> =
    OnceLock::new();

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedAttnResLabState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    playback_state: AttnResLabPlaybackState,
    selected_view: AttnResLabViewMode,
    selected_sublayer: usize,
    show_help: bool,
    current_step: u64,
    speed_multiplier: usize,
    events: Vec<String>,
    last_action: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DesktopAttnResLabStatus {
    pub(crate) playback_state: AttnResLabPlaybackState,
    pub(crate) selected_view: AttnResLabViewMode,
    pub(crate) selected_sublayer: usize,
    pub(crate) show_help: bool,
    pub(crate) snapshot: AttnResLabSnapshot,
    pub(crate) last_action: Option<String>,
    pub(crate) last_error: Option<String>,
}

struct DesktopAttnResLabController {
    storage_path: PathBuf,
    state: PersistedAttnResLabState,
    runner: Option<AttnResTinyTrainingRunner>,
    snapshot: AttnResLabSnapshot,
    runtime_telemetry: AttnResLabRunTelemetry,
    last_tick_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug, Default)]
struct AttnResLabRunTelemetry {
    last_train_ms: f64,
    last_diag_ms: f64,
    avg_loop_ms: f64,
}

impl AttnResLabRunTelemetry {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn record_step(&mut self, train_ms: f64, diag_ms: f64) {
        self.last_train_ms = train_ms;
        self.last_diag_ms = diag_ms;
        let loop_ms = train_ms + diag_ms;
        self.avg_loop_ms = if self.avg_loop_ms <= 0.0 {
            loop_ms
        } else {
            self.avg_loop_ms * 0.82 + loop_ms * 0.18
        };
    }

    fn effective_loop_ms(&self, speed_multiplier: usize) -> f64 {
        let cadence_ms = speed_poll_interval(speed_multiplier).as_secs_f64() * 1000.0;
        self.avg_loop_ms.max(cadence_ms)
    }

    fn steps_per_second(&self, speed_multiplier: usize) -> f64 {
        let effective_loop_ms = self.effective_loop_ms(speed_multiplier);
        if effective_loop_ms <= 0.0 {
            0.0
        } else {
            1000.0 / effective_loop_ms
        }
    }

    fn eta_seconds(&self, speed_multiplier: usize, step: u64, max_steps: u64) -> f64 {
        let remaining = max_steps.saturating_sub(step) as f64;
        let steps_per_second = self.steps_per_second(speed_multiplier);
        if steps_per_second <= 0.0 {
            0.0
        } else {
            remaining / steps_per_second
        }
    }
}

pub(crate) fn ensure_live_snapshot_loaded(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| Ok(controller.status())) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn current_status() -> Result<DesktopAttnResLabStatus, String> {
    with_controller(|controller| Ok(controller.status()))
}

pub(crate) fn refresh_live_snapshot(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| {
        controller.state.last_action = Some(String::from("Rebuilt live AttnRes snapshot"));
        controller.state.last_error = None;
        controller.refresh_runtime()?;
        controller.persist()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn cycle_view(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| {
        controller.state.selected_view = next_view(controller.state.selected_view);
        controller.state.last_action = Some(format!(
            "Selected {} view",
            controller.state.selected_view.label()
        ));
        controller.state.last_error = None;
        controller.persist()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn select_view(pane_state: &mut AttnResLabPaneState, view: AttnResLabViewMode) {
    match with_controller(|controller| {
        controller.state.selected_view = view;
        controller.state.last_action = Some(format!("Selected {} view", view.label()));
        controller.state.last_error = None;
        controller.persist()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn start_or_resume(pane_state: &mut AttnResLabPaneState) {
    let now = Instant::now();
    match with_controller(|controller| {
        controller.start_or_resume(now)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn pause(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| {
        controller.pause()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn move_selected_sublayer(pane_state: &mut AttnResLabPaneState, delta: isize) {
    match with_controller(|controller| {
        controller.move_selected_sublayer(delta)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn set_selected_sublayer(pane_state: &mut AttnResLabPaneState, index: usize) {
    match with_controller(|controller| {
        controller.set_selected_sublayer(index)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn adjust_speed(pane_state: &mut AttnResLabPaneState, delta: isize) {
    match with_controller(|controller| {
        controller.adjust_speed(delta)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn set_speed_multiplier(pane_state: &mut AttnResLabPaneState, speed_multiplier: usize) {
    match with_controller(|controller| {
        controller.set_speed_multiplier(speed_multiplier)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn toggle_help(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| {
        controller.state.show_help = !controller.state.show_help;
        controller.state.last_action = Some(if controller.state.show_help {
            String::from("AttnRes help overlay shown")
        } else {
            String::from("AttnRes help overlay hidden")
        });
        controller.state.last_error = None;
        controller.persist()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn toggle_playback(pane_state: &mut AttnResLabPaneState) {
    let now = Instant::now();
    match with_controller(|controller| {
        controller.toggle_playback(now)?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn reset_training(pane_state: &mut AttnResLabPaneState) {
    match with_controller(|controller| {
        controller.reset_training()?;
        Ok(controller.status())
    }) {
        Ok(status) => sync_pane_state(pane_state, status),
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn background_tick(pane_state: &mut AttnResLabPaneState) -> bool {
    let Some(controller) = DESKTOP_ATTNRES_LAB_CONTROLLER.get() else {
        return false;
    };
    let mut controller = match controller.lock() {
        Ok(controller) => controller,
        Err(_) => {
            let _ = pane_state.pane_set_error("AttnRes lab controller lock poisoned");
            return true;
        }
    };
    match controller.tick(Instant::now()) {
        Ok(changed) => {
            if changed {
                sync_pane_state(pane_state, controller.status());
            }
            changed
        }
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
            true
        }
    }
}

pub(crate) fn running_poll_interval(pane_state: &AttnResLabPaneState) -> Option<Duration> {
    pane_state
        .playback_state
        .is_running()
        .then_some(speed_poll_interval(pane_state.snapshot.speed_multiplier))
}

fn with_controller<T>(
    f: impl FnOnce(&mut DesktopAttnResLabController) -> Result<T, String>,
) -> Result<T, String> {
    let controller = DESKTOP_ATTNRES_LAB_CONTROLLER
        .get_or_init(|| Mutex::new(DesktopAttnResLabController::load(attnres_lab_state_path())));
    let mut controller = controller
        .lock()
        .map_err(|_| String::from("AttnRes lab controller lock poisoned"))?;
    f(&mut controller)
}

fn sync_pane_state(pane_state: &mut AttnResLabPaneState, status: DesktopAttnResLabStatus) {
    pane_state.playback_state = status.playback_state;
    pane_state.show_help = status.show_help;
    pane_state.selected_view = status.selected_view;
    pane_state.selected_sublayer = status.selected_sublayer;
    pane_state.snapshot = status.snapshot;
    pane_state.clamp_selected_sublayer();
    pane_state.load_state = if status.last_error.is_some() {
        PaneLoadState::Error
    } else {
        PaneLoadState::Ready
    };
    pane_state.last_action = status.last_action;
    pane_state.last_error = status.last_error;
}

impl DesktopAttnResLabController {
    fn load(storage_path: PathBuf) -> Self {
        let mut state = fs::read(storage_path.as_path())
            .ok()
            .and_then(|raw| serde_json::from_slice::<PersistedAttnResLabState>(&raw).ok())
            .unwrap_or_else(default_persisted_state);
        normalize_persisted_state(&mut state);

        let mut controller = Self {
            storage_path,
            state,
            runner: None,
            snapshot: crate::app_state::replay_attnres_lab_snapshot(),
            runtime_telemetry: AttnResLabRunTelemetry::default(),
            last_tick_at: None,
        };
        if let Err(error) = controller.refresh_runtime() {
            controller.state.last_error = Some(format!(
                "Failed to hydrate persisted AttnRes state: {error}"
            ));
            controller.state.last_action = Some(String::from(
                "Showing replay fallback until live Psionic state is available",
            ));
        }
        controller
    }

    fn status(&self) -> DesktopAttnResLabStatus {
        DesktopAttnResLabStatus {
            playback_state: self.state.playback_state,
            selected_view: self.state.selected_view,
            selected_sublayer: self.state.selected_sublayer,
            show_help: self.state.show_help,
            snapshot: self.snapshot.clone(),
            last_action: self.state.last_action.clone(),
            last_error: self.state.last_error.clone(),
        }
    }

    fn persist(&mut self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create AttnRes lab state dir: {error}"))?;
        }
        self.state.schema_version = ATTNRES_LAB_SCHEMA_VERSION;
        self.state.updated_at_epoch_ms = current_epoch_ms();
        let raw = serde_json::to_vec_pretty(&self.state)
            .map_err(|error| format!("Failed to encode AttnRes lab state: {error}"))?;
        fs::write(self.storage_path.as_path(), raw)
            .map_err(|error| format!("Failed to write AttnRes lab state: {error}"))
    }

    fn refresh_runtime(&mut self) -> Result<(), String> {
        let runner = build_runner_to_step(self.state.current_step)?;
        let snapshot = build_snapshot_from_runner(
            &runner,
            self.state.speed_multiplier,
            self.state.playback_state,
            self.state.events.as_slice(),
            self.runtime_telemetry,
        )?;
        self.runner = Some(runner);
        self.snapshot = snapshot;
        self.sync_selection_bounds();
        Ok(())
    }

    fn move_selected_sublayer(&mut self, delta: isize) -> Result<(), String> {
        let len = self.snapshot.sublayers.len();
        if len == 0 {
            self.state.selected_sublayer = 0;
        } else {
            self.state.selected_sublayer =
                (self.state.selected_sublayer as isize + delta).rem_euclid(len as isize) as usize;
        }
        let label = self
            .snapshot
            .sublayer(self.state.selected_sublayer)
            .map(|sublayer| sublayer.label.as_str())
            .unwrap_or("none");
        self.state.last_action = Some(format!("Selected sublayer {label}"));
        self.state.last_error = None;
        self.persist()
    }

    fn set_selected_sublayer(&mut self, index: usize) -> Result<(), String> {
        self.state.selected_sublayer = index.min(self.snapshot.sublayers.len().saturating_sub(1));
        let label = self
            .snapshot
            .sublayer(self.state.selected_sublayer)
            .map(|sublayer| sublayer.label.as_str())
            .unwrap_or("none");
        self.state.last_action = Some(format!("Selected sublayer {label}"));
        self.state.last_error = None;
        self.persist()
    }

    fn adjust_speed(&mut self, delta: isize) -> Result<(), String> {
        let next = ((self.state.speed_multiplier as isize) + delta)
            .clamp(MIN_SPEED_MULTIPLIER as isize, MAX_SPEED_MULTIPLIER as isize)
            as usize;
        if next == self.state.speed_multiplier {
            return Ok(());
        }
        self.state.speed_multiplier = next;
        self.snapshot.speed_multiplier = next;
        self.snapshot.avg_loop_ms = self.runtime_telemetry.effective_loop_ms(next);
        self.snapshot.steps_per_second = self.runtime_telemetry.steps_per_second(next);
        self.snapshot.eta_seconds =
            self.runtime_telemetry
                .eta_seconds(next, self.snapshot.step, self.snapshot.max_steps);
        self.push_event(format!("speed -> {}x", next));
        self.state.last_action = Some(format!("AttnRes speed set to {}x", next));
        self.state.last_error = None;
        self.persist()
    }

    fn set_speed_multiplier(&mut self, speed_multiplier: usize) -> Result<(), String> {
        let next = speed_multiplier.clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
        if next == self.state.speed_multiplier {
            self.state.last_action = Some(format!("AttnRes speed already {}x", next));
            self.state.last_error = None;
            return self.persist();
        }
        self.state.speed_multiplier = next;
        self.snapshot.speed_multiplier = next;
        self.snapshot.avg_loop_ms = self.runtime_telemetry.effective_loop_ms(next);
        self.snapshot.steps_per_second = self.runtime_telemetry.steps_per_second(next);
        self.snapshot.eta_seconds =
            self.runtime_telemetry
                .eta_seconds(next, self.snapshot.step, self.snapshot.max_steps);
        self.push_event(format!("speed -> {}x", next));
        self.state.last_action = Some(format!("AttnRes speed set to {}x", next));
        self.state.last_error = None;
        self.persist()
    }

    fn reset_training(&mut self) -> Result<(), String> {
        self.reset_training_internal()?;
        self.state.last_action = Some(String::from("AttnRes lab reset to seeded state"));
        self.persist()
    }

    fn reset_training_internal(&mut self) -> Result<(), String> {
        self.state.playback_state = AttnResLabPlaybackState::Armed;
        self.state.current_step = 0;
        self.state.speed_multiplier = DEFAULT_SPEED_MULTIPLIER;
        self.state.last_error = None;
        self.state.events = bootstrap_events();
        self.runtime_telemetry.reset();
        self.last_tick_at = None;
        self.refresh_runtime()
    }

    fn start_or_resume(&mut self, now: Instant) -> Result<(), String> {
        match self.state.playback_state {
            AttnResLabPlaybackState::Running => {
                self.state.last_action = Some(String::from("AttnRes training already running"));
                self.state.last_error = None;
                self.last_tick_at = Some(now);
                self.persist()
            }
            AttnResLabPlaybackState::Armed
            | AttnResLabPlaybackState::Paused
            | AttnResLabPlaybackState::Completed => self.toggle_playback(now),
        }
    }

    fn pause(&mut self) -> Result<(), String> {
        match self.state.playback_state {
            AttnResLabPlaybackState::Running => self.toggle_playback(Instant::now()),
            AttnResLabPlaybackState::Paused => {
                self.state.last_action = Some(String::from("AttnRes training already paused"));
                self.state.last_error = None;
                self.persist()
            }
            AttnResLabPlaybackState::Armed => {
                self.state.last_action = Some(String::from("AttnRes training has not started yet"));
                self.state.last_error = None;
                self.persist()
            }
            AttnResLabPlaybackState::Completed => {
                self.state.last_action = Some(String::from("AttnRes run already completed"));
                self.state.last_error = None;
                self.persist()
            }
        }
    }

    fn toggle_playback(&mut self, now: Instant) -> Result<(), String> {
        match self.state.playback_state {
            AttnResLabPlaybackState::Armed => {
                self.state.playback_state = AttnResLabPlaybackState::Running;
                self.state.last_action = Some(String::from("Started AttnRes training"));
                self.state.last_error = None;
                self.push_event("new training run started");
                self.snapshot.run_status = self.state.playback_state.status_label().to_string();
                self.last_tick_at = Some(now);
            }
            AttnResLabPlaybackState::Running => {
                self.state.playback_state = AttnResLabPlaybackState::Paused;
                self.state.last_action = Some(String::from("Paused AttnRes training"));
                self.state.last_error = None;
                self.push_event("training paused");
                self.snapshot.run_status = self.state.playback_state.status_label().to_string();
                self.last_tick_at = None;
            }
            AttnResLabPlaybackState::Paused => {
                self.state.playback_state = AttnResLabPlaybackState::Running;
                self.state.last_action = Some(String::from("Resumed AttnRes training"));
                self.state.last_error = None;
                self.push_event("training resumed");
                self.snapshot.run_status = self.state.playback_state.status_label().to_string();
                self.last_tick_at = Some(now);
            }
            AttnResLabPlaybackState::Completed => {
                self.reset_training_internal()?;
                self.state.playback_state = AttnResLabPlaybackState::Running;
                self.state.last_action = Some(String::from("Started a fresh AttnRes run"));
                self.state.last_error = None;
                self.push_event("new training run started");
                self.snapshot.run_status = self.state.playback_state.status_label().to_string();
                self.last_tick_at = Some(now);
            }
        }
        self.persist()
    }

    fn tick(&mut self, now: Instant) -> Result<bool, String> {
        if !self.state.playback_state.is_running() {
            return Ok(false);
        }
        let interval = speed_poll_interval(self.state.speed_multiplier);
        if self
            .last_tick_at
            .is_some_and(|last_tick| now.saturating_duration_since(last_tick) < interval)
        {
            return Ok(false);
        }

        let previous_snapshot = self.snapshot.clone();
        let train_started = Instant::now();
        let update = {
            let runner = self.ensure_runner()?;
            runner.step().map_err(|error| error.to_string())?
        };
        let train_ms = train_started.elapsed().as_secs_f64() * 1000.0;

        self.state.current_step = update.current_global_step;
        self.last_tick_at = Some(now);
        let diagnostics_started = Instant::now();
        self.snapshot = build_snapshot_from_runner(
            self.runner
                .as_ref()
                .ok_or_else(|| String::from("AttnRes runner is unavailable"))?,
            self.state.speed_multiplier,
            self.state.playback_state,
            self.state.events.as_slice(),
            self.runtime_telemetry,
        )?;
        let diag_ms = diagnostics_started.elapsed().as_secs_f64() * 1000.0;
        self.runtime_telemetry.record_step(train_ms, diag_ms);
        self.snapshot.last_train_ms = self.runtime_telemetry.last_train_ms;
        self.snapshot.last_diag_ms = self.runtime_telemetry.last_diag_ms;
        self.snapshot.avg_loop_ms = self
            .runtime_telemetry
            .effective_loop_ms(self.snapshot.speed_multiplier);
        self.snapshot.steps_per_second = self
            .runtime_telemetry
            .steps_per_second(self.snapshot.speed_multiplier);
        self.snapshot.eta_seconds = self.runtime_telemetry.eta_seconds(
            self.snapshot.speed_multiplier,
            self.snapshot.step,
            self.snapshot.max_steps,
        );
        self.sync_selection_bounds();
        self.note_training_step(&previous_snapshot);

        if update.lifecycle == AttnResTinyTrainingLifecycleStatus::Completed {
            self.state.playback_state = AttnResLabPlaybackState::Completed;
            self.snapshot.run_status = self.state.playback_state.status_label().to_string();
            self.last_tick_at = None;
            self.push_event("run complete; press Space to restart or r to reset");
            self.state.last_action = Some(String::from("AttnRes local reference run completed"));
        } else {
            self.snapshot.run_status = self.state.playback_state.status_label().to_string();
            self.state.last_action = Some(format!(
                "Applied AttnRes training step {} of {}",
                update.current_global_step, update.max_steps
            ));
        }
        self.state.last_error = None;
        self.snapshot.events = self.state.events.clone();
        self.persist()?;
        Ok(true)
    }

    fn ensure_runner(&mut self) -> Result<&mut AttnResTinyTrainingRunner, String> {
        let needs_refresh = self.runner.as_ref().is_none_or(|runner| {
            runner.current_update().current_global_step != self.state.current_step
        });
        if needs_refresh {
            self.refresh_runtime()?;
        }
        self.runner
            .as_mut()
            .ok_or_else(|| String::from("AttnRes runner is unavailable"))
    }

    fn note_training_step(&mut self, previous_snapshot: &AttnResLabSnapshot) {
        self.push_event(format!(
            "step {}/{} loss {:.3} selectivity {:.0}%",
            self.snapshot.step,
            self.snapshot.max_steps,
            self.snapshot.training_loss,
            self.snapshot.avg_selectivity * 100.0
        ));

        if self.snapshot.step == 1 {
            self.push_event("loss stream live; zero-init routing starts near-uniform");
        }

        let previous_band = routing_band(previous_snapshot.avg_selectivity);
        let next_band = routing_band(self.snapshot.avg_selectivity);
        if previous_band != next_band {
            self.push_event(format!("routing regime -> {}", band_description(next_band)));
        }

        if let (Some(previous), Some(current)) = (
            previous_snapshot.sublayer(self.state.selected_sublayer),
            self.snapshot.sublayer(self.state.selected_sublayer),
        ) {
            if previous.dominant_source_label != current.dominant_source_label {
                self.push_event(format!(
                    "{} now favors {} ({:.0}%)",
                    current.label,
                    current.dominant_source_label,
                    current.dominant_weight * 100.0
                ));
            }
        }
    }

    fn push_event(&mut self, message: impl Into<String>) {
        self.state.events.insert(
            0,
            format!("s{:03}  {}", self.state.current_step, message.into()),
        );
        if self.state.events.len() > ATTNRES_EVENT_LIMIT {
            self.state.events.truncate(ATTNRES_EVENT_LIMIT);
        }
        self.snapshot.events = self.state.events.clone();
    }

    fn sync_selection_bounds(&mut self) {
        self.state.selected_sublayer = self
            .state
            .selected_sublayer
            .min(self.snapshot.sublayers.len().saturating_sub(1));
    }
}

fn default_persisted_state() -> PersistedAttnResLabState {
    PersistedAttnResLabState {
        schema_version: ATTNRES_LAB_SCHEMA_VERSION,
        updated_at_epoch_ms: current_epoch_ms(),
        playback_state: AttnResLabPlaybackState::Armed,
        selected_view: AttnResLabViewMode::Overview,
        selected_sublayer: 0,
        show_help: false,
        current_step: 0,
        speed_multiplier: DEFAULT_SPEED_MULTIPLIER,
        events: bootstrap_events(),
        last_action: Some(String::from("AttnRes lab armed and ready")),
        last_error: None,
    }
}

fn normalize_persisted_state(state: &mut PersistedAttnResLabState) {
    let max_steps = lab_training_config()
        .map(|config| config.budget.max_steps)
        .unwrap_or(1);
    if state.schema_version < ATTNRES_LAB_SCHEMA_VERSION {
        if state.playback_state == AttnResLabPlaybackState::Completed
            && state.current_step < max_steps
        {
            let selected_view = state.selected_view;
            let selected_sublayer = state.selected_sublayer;
            let show_help = state.show_help;
            let speed_multiplier = state
                .speed_multiplier
                .clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
            *state = default_persisted_state();
            state.selected_view = selected_view;
            state.selected_sublayer = selected_sublayer;
            state.show_help = show_help;
            state.speed_multiplier = speed_multiplier;
            state.last_action = Some(String::from(
                "Reset AttnRes lab state after migrating to the full local reference run",
            ));
        }
        state.schema_version = ATTNRES_LAB_SCHEMA_VERSION;
    }
    state.speed_multiplier = state
        .speed_multiplier
        .clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
    state.current_step = state.current_step.min(max_steps);
    if state.playback_state == AttnResLabPlaybackState::Completed && state.current_step < max_steps
    {
        state.playback_state = if state.current_step == 0 {
            AttnResLabPlaybackState::Armed
        } else {
            AttnResLabPlaybackState::Paused
        };
        state.last_action = Some(String::from(
            "Recovered AttnRes lab state after detecting an incomplete completed snapshot",
        ));
    } else if state.current_step >= max_steps {
        state.playback_state = AttnResLabPlaybackState::Completed;
    } else if state.playback_state.is_running() {
        state.playback_state = AttnResLabPlaybackState::Paused;
        state.last_action = Some(String::from(
            "Restored AttnRes run in paused state after app restart",
        ));
    }
    if state.events.is_empty() {
        state.events = bootstrap_events();
    }
}

fn attnres_lab_state_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(ATTNRES_LAB_STATE_FILENAME)
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn speed_poll_interval(speed: usize) -> Duration {
    match speed.clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER) {
        1 => Duration::from_millis(260),
        2 => Duration::from_millis(150),
        3 => Duration::from_millis(85),
        4 => Duration::from_millis(45),
        _ => Duration::from_millis(20),
    }
}

fn next_view(view: AttnResLabViewMode) -> AttnResLabViewMode {
    let index = AttnResLabViewMode::ALL
        .iter()
        .position(|candidate| *candidate == view)
        .unwrap_or_default();
    AttnResLabViewMode::ALL[(index + 1) % AttnResLabViewMode::ALL.len()]
}

fn bootstrap_events() -> Vec<String> {
    let corpus = lab_corpus().ok();
    let block_line = corpus
        .as_ref()
        .and_then(|corpus| corpus.config.block_size().ok())
        .map(|block_size| {
            format!(
                "s000  block size {} over {} residual blocks",
                block_size,
                corpus
                    .as_ref()
                    .map(|corpus| corpus.config.num_blocks)
                    .unwrap_or(0)
            )
        })
        .unwrap_or_else(|| String::from("s000  block schedule unavailable"));
    vec![
        block_line,
        String::from(
            "s000  inspect sublayers with Left/Right and switch views with Tab or 1/2/3/4",
        ),
        String::from("s000  dashboard armed; press Space to start training"),
    ]
}

fn build_runner_to_step(step: u64) -> Result<AttnResTinyTrainingRunner, String> {
    let corpus = lab_corpus()?;
    let config = lab_training_config()?;
    let mut runner = AttnResTinyTrainingRunner::new(&corpus, &config)
        .map_err(|error| format!("Failed to seed AttnRes runner: {error}"))?;
    let target_step = step.min(config.budget.max_steps);
    while runner.current_update().current_global_step < target_step {
        runner.step().map_err(|error| error.to_string())?;
    }
    Ok(runner)
}

fn build_snapshot_from_runner(
    runner: &AttnResTinyTrainingRunner,
    speed_multiplier: usize,
    playback_state: AttnResLabPlaybackState,
    events: &[String],
    runtime_telemetry: AttnResLabRunTelemetry,
) -> Result<AttnResLabSnapshot, String> {
    let update = runner.current_update();
    let corpus = lab_corpus()?;
    let config = lab_training_config()?;
    let current_model = AttnResCpuReferenceModel::seeded(
        config.model_id.clone(),
        config.model_revision.clone(),
        corpus.config.clone(),
    )
    .map_err(|error| error.to_string())?;
    let generation_response =
        generate_preview_response(&corpus, &current_model, update.current_global_step)?;
    let inspection_sequence = generation_response.full_sequence.clone();
    let standard_hidden = current_model
        .forward_hidden(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let standard_logits = current_model
        .forward(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let two_phase_hidden = current_model
        .forward_two_phase_hidden(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let two_phase_logits = current_model
        .forward_two_phase(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;

    let parity_budget = AttnResTwoPhaseParityBudget::default();
    let hidden_parity = compare_attnres_hidden_two_phase_parity(
        standard_hidden.values(),
        two_phase_hidden.values(),
        parity_budget.hidden,
    )
    .map_err(|error| error.to_string())?;
    let logit_parity = compare_attnres_logit_two_phase_parity(
        standard_logits.values(),
        two_phase_logits.values(),
        parity_budget.logits,
    )
    .map_err(|error| error.to_string())?;

    let (metrics, final_ema_loss) = collect_metric_points(runner)?;
    let sublayers = map_sublayers(&update.diagnostics, &corpus.config);
    let avg_selectivity = mean_selectivity(sublayers.as_slice());
    let block_summaries = build_block_summaries(sublayers.as_slice(), corpus.config.num_blocks);
    let final_block_index = block_summaries
        .last()
        .map(|block| block.block_index)
        .unwrap_or_default();
    let current_block_fill = sublayers
        .iter()
        .filter(|sublayer| sublayer.target_block == final_block_index)
        .count();
    let completed_blocks = update.diagnostics.final_completed_blocks;
    let active_block = completed_blocks
        + if update.diagnostics.final_partial_block_present {
            1
        } else {
            0
        };
    let active_block = active_block.max(1);
    let inference = build_inference_summary(
        &corpus.config,
        &update.diagnostics,
        &generation_response,
        &hidden_parity,
        &logit_parity,
    );

    Ok(AttnResLabSnapshot {
        source_badge: LIVE_SOURCE_BADGE.to_string(),
        model_label: format!("{} // {}", config.model_id, config.model_revision),
        architecture_label: format!(
            "{} sublayers // {} residual blocks // {} heads",
            corpus.config.num_layers, corpus.config.num_blocks, corpus.config.num_heads
        ),
        run_label: format!(
            "{} // {}",
            update.run_id,
            update
                .checkpoint
                .checkpoint_ref
                .clone()
                .unwrap_or_else(|| String::from("seed"))
        ),
        run_status: playback_state.status_label().to_string(),
        num_transformer_layers: corpus.config.num_transformer_layers(),
        num_residual_blocks: corpus.config.num_blocks,
        block_size: corpus.config.block_size().unwrap_or(1),
        num_heads: corpus.config.num_heads,
        batch_size: update.diagnostics.batch_size,
        sequence_length: update.diagnostics.sequence_length,
        hidden_size: update.diagnostics.hidden_size,
        step: update.current_global_step,
        max_steps: update.max_steps,
        speed_multiplier,
        last_train_ms: runtime_telemetry.last_train_ms,
        last_diag_ms: runtime_telemetry.last_diag_ms,
        avg_loop_ms: runtime_telemetry.effective_loop_ms(speed_multiplier),
        steps_per_second: runtime_telemetry.steps_per_second(speed_multiplier),
        eta_seconds: runtime_telemetry.eta_seconds(
            speed_multiplier,
            update.current_global_step,
            update.max_steps,
        ),
        training_loss: update.current_training_mean_loss,
        ema_loss: final_ema_loss,
        avg_selectivity,
        mean_query_norm: mean_query_norm(sublayers.as_slice()),
        max_query_norm: max_query_norm(sublayers.as_slice()),
        active_block,
        current_block_fill,
        completed_blocks,
        final_partial_block_present: update.diagnostics.final_partial_block_present,
        metrics,
        sublayers,
        block_summaries,
        inference,
        events: events.to_vec(),
    })
}

fn collect_metric_points(
    runner: &AttnResTinyTrainingRunner,
) -> Result<(Vec<AttnResLabMetricPoint>, f32), String> {
    let seeded_runner = build_runner_to_step(0)?;
    let initial = seeded_runner.current_update();
    let mut metrics = vec![AttnResLabMetricPoint {
        global_step: 0,
        training_loss: initial.current_training_mean_loss,
        ema_loss: initial.current_training_mean_loss,
        selectivity: mean_selectivity_from_diagnostics(&initial.diagnostics),
    }];
    let mut ema_loss = initial.current_training_mean_loss;
    let target_step = runner.current_update().current_global_step;
    let mut replay_runner = build_runner_to_step(0)?;
    while replay_runner.current_update().current_global_step < target_step {
        let update = replay_runner.step().map_err(|error| error.to_string())?;
        ema_loss = if metrics.len() == 1 {
            update.current_training_mean_loss
        } else {
            (ema_loss * 0.6) + (update.current_training_mean_loss * 0.4)
        };
        metrics.push(AttnResLabMetricPoint {
            global_step: update.current_global_step,
            training_loss: update.current_training_mean_loss,
            ema_loss,
            selectivity: mean_selectivity_from_diagnostics(&update.diagnostics),
        });
    }
    Ok((metrics, ema_loss))
}

fn generate_preview_response(
    corpus: &AttnResTinyTrainingCorpus,
    model: &AttnResCpuReferenceModel,
    step: u64,
) -> Result<AttnResTextGenerationResponse, String> {
    let prompt_sample = corpus
        .held_out_samples
        .first()
        .cloned()
        .or_else(|| corpus.training_samples.first().cloned())
        .ok_or_else(|| String::from("AttnRes lab corpus is empty"))?;
    let request = AttnResTextGenerationRequest::new(
        format!("attnres-lab-preview-step-{step}"),
        prompt_sample.input_tokens.clone(),
        2,
    )
    .with_requested_model_id(model.descriptor().model.model_id.clone());
    let generation_service = LocalAttnResTextGenerationService::new().with_model(model.clone());
    match generation_service
        .execute(&request)
        .map_err(|error| error.to_string())?
    {
        AttnResTextGenerationOutcome::Completed { response } => Ok(response),
        AttnResTextGenerationOutcome::Refused { refusal } => {
            Err(format!("AttnRes generation refused: {}", refusal.detail))
        }
    }
}

fn lab_training_config() -> Result<AttnResTinyTrainingConfig, String> {
    AttnResTinyTrainingConfig::reference().map_err(|error| error.to_string())
}

fn lab_corpus() -> Result<AttnResTinyTrainingCorpus, String> {
    AttnResTinyTrainingCorpus::reference().map_err(|error| error.to_string())
}

fn build_inference_summary(
    config: &AttnResConfig,
    diagnostics: &AttnResDiagnosticsSnapshot,
    generation_response: &AttnResTextGenerationResponse,
    hidden_parity: &AttnResHiddenParityReport,
    logit_parity: &AttnResLogitParityReport,
) -> AttnResLabInferenceSummary {
    let sublayers = map_sublayers(diagnostics, config);
    let partial_merge_share = sublayers
        .iter()
        .map(|sublayer| sublayer.partial_mass)
        .sum::<f32>()
        / sublayers.len().max(1) as f32;
    let cache_merge_share = sublayers
        .iter()
        .map(|sublayer| sublayer.cache_mass)
        .sum::<f32>()
        / sublayers.len().max(1) as f32;
    let block_cache_fill_share =
        diagnostics.final_completed_blocks as f32 / config.num_blocks.max(1) as f32;
    let boundary_layers = config
        .boundary_transformer_layers()
        .unwrap_or_default()
        .into_iter()
        .map(|layer| format!("L{layer}"))
        .collect::<Vec<_>>();

    AttnResLabInferenceSummary {
        hidden_parity_label: parity_status_label(hidden_parity.status).to_string(),
        logit_parity_label: parity_status_label(logit_parity.status).to_string(),
        hidden_max_abs_diff: hidden_parity.summary.max_abs_delta,
        logit_max_abs_diff: logit_parity.summary.max_abs_delta,
        prompt_token_count: generation_response.prompt_tokens.len(),
        generated_token_count: generation_response.generated_tokens.len(),
        decoded_token_count: generation_response.full_sequence.len(),
        inspected_sublayers: diagnostics.sublayers.len(),
        partial_merge_share,
        cache_merge_share,
        block_cache_fill_share,
        cached_blocks: diagnostics.final_completed_blocks,
        partial_block_present: diagnostics.final_partial_block_present,
        boundary_layers: boundary_layers.clone(),
        schedule_note: format!(
            "decoded {} tokens over {} sublayers; block boundaries at {}",
            generation_response.full_sequence.len(),
            diagnostics.sublayers.len(),
            join_or_dash(boundary_layers.as_slice())
        ),
        merge_note: format!(
            "average routing mass stayed {:.0}% partial vs {:.0}% cached across the inspected decode path",
            partial_merge_share * 100.0,
            cache_merge_share * 100.0
        ),
        cache_note: format!(
            "{} completed blocks cached with partial block still present={}",
            diagnostics.final_completed_blocks, diagnostics.final_partial_block_present
        ),
    }
}

fn map_sublayers(
    diagnostics: &AttnResDiagnosticsSnapshot,
    config: &AttnResConfig,
) -> Vec<AttnResLabSublayerSnapshot> {
    let block_size = config.block_size().unwrap_or(1).max(1);
    diagnostics
        .sublayers
        .iter()
        .map(|sublayer| {
            let source_labels = source_labels(sublayer.completed_blocks_before, sublayer);
            let source_logits =
                aggregate_source_values(&sublayer.source_logits, sublayer.source_shape);
            let routing_weights =
                aggregate_source_values(&sublayer.routing_weights, sublayer.source_shape);
            let (dominant_source_label, dominant_weight) =
                dominant_source(source_labels.as_slice(), routing_weights.as_slice());
            let partial_mass = source_labels
                .iter()
                .enumerate()
                .find(|(_, label)| label.as_str() == "partial")
                .and_then(|(index, _)| routing_weights.get(index).copied())
                .unwrap_or(0.0);
            let cache_mass = (routing_weights.iter().sum::<f32>() - partial_mass).clamp(0.0, 1.0);
            let selectivity = selectivity_from_weights(routing_weights.as_slice());
            let entropy = entropy_from_weights(routing_weights.as_slice());
            let target_block = sublayer.sublayer_index / block_size;
            AttnResLabSublayerSnapshot {
                sublayer_index: sublayer.sublayer_index,
                transformer_layer_index: sublayer.transformer_layer_index,
                slot_in_block: sublayer.sublayer_index % block_size,
                label: format!(
                    "L{} {}",
                    sublayer.transformer_layer_index,
                    kind_label(sublayer.kind)
                ),
                kind_label: kind_label(sublayer.kind).to_lowercase(),
                target_block,
                dominant_source_label: dominant_source_label.clone(),
                dominant_weight,
                selectivity,
                entropy,
                query_norm: sublayer.query_norm,
                partial_mass,
                cache_mass,
                source_labels,
                source_logits,
                routing_weights,
                route_note: route_note(
                    sublayer,
                    target_block,
                    dominant_weight,
                    cache_mass,
                    partial_mass,
                    dominant_source_label.as_str(),
                ),
                starts_new_block_before: sublayer.starts_new_block_before,
                completed_blocks_before: sublayer.completed_blocks_before,
                completed_blocks_after: sublayer.completed_blocks_after,
                partial_block_present_before: sublayer.partial_block_present_before,
                partial_block_present_after: sublayer.partial_block_present_after,
            }
        })
        .collect()
}

fn source_labels(
    completed_blocks_before: usize,
    sublayer: &psionic_models::AttnResSublayerSnapshot,
) -> Vec<String> {
    let source_count = sublayer.source_shape[0];
    if completed_blocks_before == 0 && source_count == 1 {
        return vec![String::from("seed")];
    }
    let mut labels = (0..completed_blocks_before.min(source_count))
        .map(|index| format!("block-{index}"))
        .collect::<Vec<_>>();
    while labels.len() < source_count {
        if sublayer.partial_block_present_before && labels.len() + 1 == source_count {
            labels.push(String::from("partial"));
        } else {
            labels.push(format!("source-{}", labels.len()));
        }
    }
    labels
}

fn aggregate_source_values(values: &[f32], source_shape: [usize; 3]) -> Vec<f32> {
    let source_count = source_shape[0];
    if source_count == 0 {
        return Vec::new();
    }
    let per_source = source_shape[1].saturating_mul(source_shape[2]).max(1);
    (0..source_count)
        .map(|source_index| {
            let start = source_index.saturating_mul(per_source);
            if start >= values.len() {
                return 0.0;
            }
            let end = (start + per_source).min(values.len());
            let slice = &values[start..end];
            slice.iter().sum::<f32>() / slice.len().max(1) as f32
        })
        .collect()
}

fn dominant_source(labels: &[String], weights: &[f32]) -> (String, f32) {
    weights
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, weight)| {
            (
                labels
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| String::from("source")),
                weight,
            )
        })
        .unwrap_or_else(|| (String::from("source"), 0.0))
}

fn selectivity_from_weights(weights: &[f32]) -> f32 {
    if weights.len() <= 1 {
        0.0
    } else {
        1.0 - entropy_from_weights(weights)
    }
}

fn entropy_from_weights(weights: &[f32]) -> f32 {
    if weights.len() <= 1 {
        return 0.0;
    }
    let sum = weights.iter().copied().sum::<f32>().max(f32::EPSILON);
    let entropy = weights.iter().fold(0.0, |acc, weight| {
        let normalized = (*weight / sum).clamp(0.0, 1.0);
        if normalized <= f32::EPSILON {
            acc
        } else {
            acc - normalized * normalized.ln()
        }
    });
    let max_entropy = (weights.len() as f32).ln().max(f32::EPSILON);
    (entropy / max_entropy).clamp(0.0, 1.0)
}

fn mean_selectivity_from_diagnostics(diagnostics: &AttnResDiagnosticsSnapshot) -> f32 {
    let values = diagnostics
        .sublayers
        .iter()
        .map(|sublayer| {
            let weights = aggregate_source_values(&sublayer.routing_weights, sublayer.source_shape);
            selectivity_from_weights(weights.as_slice())
        })
        .collect::<Vec<_>>();
    mean_selectivity_values(values.as_slice())
}

fn mean_selectivity(sublayers: &[AttnResLabSublayerSnapshot]) -> f32 {
    let values = sublayers
        .iter()
        .map(|sublayer| sublayer.selectivity)
        .collect::<Vec<_>>();
    mean_selectivity_values(values.as_slice())
}

fn mean_selectivity_values(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn mean_query_norm(sublayers: &[AttnResLabSublayerSnapshot]) -> f32 {
    if sublayers.is_empty() {
        0.0
    } else {
        sublayers
            .iter()
            .map(|sublayer| sublayer.query_norm)
            .sum::<f32>()
            / sublayers.len() as f32
    }
}

fn max_query_norm(sublayers: &[AttnResLabSublayerSnapshot]) -> f32 {
    sublayers
        .iter()
        .map(|sublayer| sublayer.query_norm)
        .fold(0.0_f32, f32::max)
}

fn build_block_summaries(
    sublayers: &[AttnResLabSublayerSnapshot],
    block_count: usize,
) -> Vec<AttnResLabBlockSummary> {
    (0..block_count)
        .filter_map(|block_index| {
            let block_sublayers = sublayers
                .iter()
                .filter(|sublayer| sublayer.target_block == block_index)
                .collect::<Vec<_>>();
            if block_sublayers.is_empty() {
                return None;
            }
            Some(AttnResLabBlockSummary {
                block_index,
                avg_selectivity: block_sublayers
                    .iter()
                    .map(|sublayer| sublayer.selectivity)
                    .sum::<f32>()
                    / block_sublayers.len() as f32,
                avg_query_norm: block_sublayers
                    .iter()
                    .map(|sublayer| sublayer.query_norm)
                    .sum::<f32>()
                    / block_sublayers.len() as f32,
                sublayers: block_sublayers.len(),
            })
        })
        .collect()
}

fn route_note(
    sublayer: &psionic_models::AttnResSublayerSnapshot,
    target_block: usize,
    dominant_weight: f32,
    cache_mass: f32,
    partial_mass: f32,
    dominant_source_label: &str,
) -> String {
    let boundary_note = if sublayer.starts_new_block_before {
        format!("Boundary opened block {}. ", target_block + 1)
    } else {
        String::new()
    };
    let routing_mode = if partial_mass > cache_mass {
        "partial lane led"
    } else if cache_mass > 0.0 {
        "cache lanes led"
    } else {
        "single source routed"
    };
    format!(
        "{boundary_note}{routing_mode}; {dominant_source_label} carried {:.0}% of the averaged routing mass.",
        dominant_weight * 100.0
    )
}

fn kind_label(kind: AttnResSublayerKind) -> &'static str {
    match kind {
        AttnResSublayerKind::Attention => "Attention",
        AttnResSublayerKind::FeedForward => "MLP",
    }
}

fn parity_status_label(status: AttnResTwoPhaseParityStatus) -> &'static str {
    match status {
        AttnResTwoPhaseParityStatus::Exact => "exact",
        AttnResTwoPhaseParityStatus::WithinBudget => "within budget",
        AttnResTwoPhaseParityStatus::OutsideBudget => "outside budget",
    }
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        String::from("-")
    } else {
        values.join(", ")
    }
}

fn band_description(band: &'static str) -> &'static str {
    match band {
        "uniform" => "uniform averaging",
        "forming" => "emerging preferences",
        _ => "selective routing",
    }
}

fn routing_band(selectivity: f32) -> &'static str {
    if selectivity < 0.18 {
        "uniform"
    } else if selectivity < 0.36 {
        "forming"
    } else {
        "selective"
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use tempfile::tempdir;

    use super::{
        ATTNRES_LAB_SCHEMA_VERSION, DEFAULT_SPEED_MULTIPLIER, DesktopAttnResLabController,
        aggregate_source_values, build_runner_to_step, collect_metric_points, kind_label,
        parity_status_label, routing_band, selectivity_from_weights, speed_poll_interval,
    };
    use crate::app_state::{AttnResLabPlaybackState, AttnResLabViewMode};
    use psionic_models::{
        AttnResDiagnosticsSnapshot, AttnResSublayerKind, AttnResSublayerSnapshot,
    };
    use psionic_runtime::AttnResTwoPhaseParityStatus;

    #[test]
    fn controller_persists_view_speed_help_and_selection() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("attnres-state.json");
        let mut controller = DesktopAttnResLabController::load(path.clone());
        controller.state.selected_view = AttnResLabViewMode::Inference;
        controller.state.show_help = true;
        controller.adjust_speed(2).expect("speed change");
        controller
            .move_selected_sublayer(1)
            .expect("move selection");
        controller.persist().expect("persist");

        let reloaded = DesktopAttnResLabController::load(path);
        assert_eq!(reloaded.state.selected_view, AttnResLabViewMode::Inference);
        assert!(reloaded.state.show_help);
        assert_eq!(
            reloaded.state.speed_multiplier,
            DEFAULT_SPEED_MULTIPLIER + 2
        );
        assert_eq!(reloaded.state.selected_sublayer, 1);
    }

    #[test]
    fn controller_ticks_to_completion_and_space_restarts() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("attnres-state.json");
        let mut controller = DesktopAttnResLabController::load(path);
        let mut now = Instant::now();
        controller.toggle_playback(now).expect("start run");
        while controller.state.playback_state != AttnResLabPlaybackState::Completed {
            now +=
                speed_poll_interval(controller.state.speed_multiplier) + Duration::from_millis(5);
            assert!(controller.tick(now).expect("tick should succeed"));
        }
        assert_eq!(
            controller.snapshot.step, controller.snapshot.max_steps,
            "run should complete its full bounded budget"
        );
        now += Duration::from_millis(25);
        controller.toggle_playback(now).expect("restart run");
        assert_eq!(
            controller.state.playback_state,
            AttnResLabPlaybackState::Running
        );
        assert_eq!(controller.snapshot.step, 0);
    }

    #[test]
    fn legacy_completed_state_is_reset_for_full_run_migration() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("attnres-state.json");
        let legacy = serde_json::json!({
            "schema_version": 1,
            "updated_at_epoch_ms": 0u64,
            "playback_state": "completed",
            "selected_view": "inference",
            "selected_sublayer": 1usize,
            "show_help": true,
            "current_step": 6u64,
            "speed_multiplier": 5usize,
            "events": ["s006  legacy run complete"],
            "last_action": "AttnRes training run completed",
            "last_error": null
        });
        fs::write(
            &path,
            serde_json::to_vec_pretty(&legacy).expect("legacy json"),
        )
        .expect("write legacy state");

        let reloaded = DesktopAttnResLabController::load(path);
        assert_eq!(reloaded.state.schema_version, ATTNRES_LAB_SCHEMA_VERSION);
        assert_eq!(
            reloaded.state.playback_state,
            AttnResLabPlaybackState::Armed
        );
        assert_eq!(reloaded.state.current_step, 0);
        assert_eq!(reloaded.state.selected_view, AttnResLabViewMode::Inference);
        assert_eq!(reloaded.state.selected_sublayer, 1);
        assert!(reloaded.state.show_help);
        assert_eq!(reloaded.state.speed_multiplier, 5);
        assert_eq!(reloaded.snapshot.step, 0);
        assert_eq!(reloaded.snapshot.max_steps, 320);
    }

    #[test]
    fn valid_full_run_completed_state_survives_schema_upgrade() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("attnres-state.json");
        let persisted = serde_json::json!({
            "schema_version": 1,
            "updated_at_epoch_ms": 0u64,
            "playback_state": "completed",
            "selected_view": "inference",
            "selected_sublayer": 0usize,
            "show_help": false,
            "current_step": 320u64,
            "speed_multiplier": 5usize,
            "events": ["s320  run complete; press Space to restart or r to reset"],
            "last_action": "AttnRes local reference run completed",
            "last_error": null
        });
        fs::write(
            &path,
            serde_json::to_vec_pretty(&persisted).expect("persisted json"),
        )
        .expect("write persisted state");

        let reloaded = DesktopAttnResLabController::load(path);
        assert_eq!(reloaded.state.schema_version, ATTNRES_LAB_SCHEMA_VERSION);
        assert_eq!(
            reloaded.state.playback_state,
            AttnResLabPlaybackState::Completed
        );
        assert_eq!(reloaded.state.current_step, 320);
        assert_eq!(reloaded.snapshot.step, 320);
        assert_eq!(reloaded.snapshot.max_steps, 320);
    }

    #[test]
    fn aggregated_source_values_average_each_source_plane() {
        let values = aggregate_source_values(&[0.2, 0.4, 0.6, 0.8], [2, 1, 2]);
        assert!((values[0] - 0.3).abs() < 1.0e-6);
        assert!((values[1] - 0.7).abs() < 1.0e-6);
    }

    #[test]
    fn selectivity_is_zero_for_single_source_and_positive_for_peaked_routes() {
        assert_eq!(selectivity_from_weights(&[1.0]), 0.0);
        assert!(selectivity_from_weights(&[0.95, 0.05]) > 0.5);
    }

    #[test]
    fn helper_labels_match_runtime_contract() {
        assert_eq!(kind_label(AttnResSublayerKind::Attention), "Attention");
        assert_eq!(kind_label(AttnResSublayerKind::FeedForward), "MLP");
        assert_eq!(
            parity_status_label(AttnResTwoPhaseParityStatus::OutsideBudget),
            "outside budget"
        );
        assert_eq!(routing_band(0.05), "uniform");
        assert_eq!(routing_band(0.25), "forming");
        assert_eq!(routing_band(0.55), "selective");
    }

    #[test]
    fn explicit_controls_are_idempotent_and_allow_direct_selection() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("attnres-state.json");
        let mut controller = DesktopAttnResLabController::load(path);
        let last_index = controller.snapshot.sublayers.len().saturating_sub(1);

        controller
            .set_selected_sublayer(last_index)
            .expect("set sublayer should succeed");
        assert_eq!(controller.state.selected_sublayer, last_index);

        controller
            .set_speed_multiplier(5)
            .expect("set speed should succeed");
        assert_eq!(controller.state.speed_multiplier, 5);

        let now = Instant::now();
        controller
            .start_or_resume(now)
            .expect("start should succeed");
        assert_eq!(
            controller.state.playback_state,
            AttnResLabPlaybackState::Running
        );

        controller
            .start_or_resume(now + Duration::from_millis(5))
            .expect("repeated start should succeed");
        assert_eq!(
            controller.state.playback_state,
            AttnResLabPlaybackState::Running
        );

        controller.pause().expect("pause should succeed");
        assert_eq!(
            controller.state.playback_state,
            AttnResLabPlaybackState::Paused
        );

        controller.pause().expect("repeated pause should succeed");
        assert_eq!(
            controller.state.playback_state,
            AttnResLabPlaybackState::Paused
        );
    }

    #[test]
    fn collect_metric_points_preserves_seeded_step_zero_loss() {
        let mut runner = build_runner_to_step(0).expect("seed runner");
        runner.step().expect("step one");
        runner.step().expect("step two");

        let progressed_loss = runner.current_update().current_training_mean_loss;
        let seeded_loss = build_runner_to_step(0)
            .expect("seed runner for comparison")
            .current_update()
            .current_training_mean_loss;

        let (metrics, _) = collect_metric_points(&runner).expect("collect metric points");
        assert_eq!(metrics[0].global_step, 0);
        assert_eq!(metrics[1].global_step, 1);
        assert_eq!(metrics[2].global_step, 2);
        assert!((metrics[0].training_loss - seeded_loss).abs() < 1.0e-6);
        assert!((metrics[0].training_loss - progressed_loss).abs() > 1.0e-4);
    }

    #[test]
    fn diagnostics_snapshot_shape_matches_adapter_expectations() {
        let diagnostics = AttnResDiagnosticsSnapshot {
            batch_size: 1,
            sequence_length: 2,
            hidden_size: 8,
            final_completed_blocks: 1,
            final_partial_block_present: true,
            sublayers: vec![AttnResSublayerSnapshot {
                sublayer_index: 0,
                transformer_layer_index: 0,
                kind: AttnResSublayerKind::Attention,
                starts_new_block_before: false,
                completed_blocks_before: 0,
                completed_blocks_after: 0,
                partial_block_present_before: true,
                partial_block_present_after: true,
                source_shape: [2, 1, 2],
                source_logits: vec![0.1, 0.3, 0.6, 0.9],
                routing_weights: vec![0.2, 0.4, 0.6, 0.8],
                query_norm: 0.5,
            }],
        };
        assert_eq!(diagnostics.sublayers[0].source_shape, [2, 1, 2]);
    }
}
