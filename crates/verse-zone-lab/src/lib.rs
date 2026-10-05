//! Physics Lab: a sandbox that runs each mechanism of the shared `physics`
//! crate live, with knobs to choose a scenario and change its parameters.
//!
//! [`scenes`] builds and steps the scenarios without drawing; this module
//! owns the knobs, the fixed-step clock, and the HUD text; [`draw`] turns a
//! scenario into meshes. The player walks around a stage on flat ground;
//! physics coordinates are stage coordinates offset by [`STAGE`].
//!
//! `verse` re-exports this crate as `zones::lab`, so an edit here recompiles
//! this crate and what depends on it rather than all of Verse.

mod draw;
mod scenes;
#[cfg(test)]
mod tests;

use crate::{mesh::Mesh, world::World};

// The paths this zone was written against inside `crates/verse`.
use glam::Vec3;
use physics::FixedStep;
use serde::Serialize;
use verse_core::world;
use verse_pbr::mesh;
use verse_world::social::controller;

pub use scenes::Kind;
pub use scenes::{DT, KnobDef, Scene};

/// Scene position of the physics origin: the center of the stage floor.
pub const STAGE: Vec3 = Vec3::new(0.0, 0.0, 3.0);
/// The return portal, to the front left of the stage.
pub const RETURN_PORTAL: Vec3 = Vec3::new(-9.0, 0.0, -8.0);
/// Half the walkable square, m.
pub const HALF_EXTENT: f32 = 40.0;
/// Where the player arrives: off center, so the character does not hide
/// the scenario, and turned toward the stage.
const SPAWN: Vec3 = Vec3::new(1.6, 0.0, -2.2);
const SPAWN_YAW: f32 = -0.3;
/// Frame time multipliers for the time-scale knob.
const TIME_SCALES: [f64; 6] = [0.1, 0.25, 0.5, 1.0, 2.0, 4.0];
/// Most fixed steps one frame may run, enough for 4× at 20 frames per second.
const MAX_STEPS: u32 = 48;
/// Knobs every scenario shares, before its own.
const SHARED: [&str; 4] = ["scene", "time", "gravity", "overlay"];

/// One knob as the HUD and native accessibility see it.
#[derive(Clone, Debug, Serialize)]
pub struct KnobView {
    pub id: &'static str,
    pub label: &'static str,
    pub value: String,
    pub selected: bool,
    /// Changes apply on the next step instead of rebuilding the scenario.
    pub live: bool,
}

/// The lab's state for hosts and tests.
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub scenario: Kind,
    pub name: &'static str,
    /// 1-based position in the scenario list.
    pub number: usize,
    pub count: usize,
    pub paused: bool,
    pub time_scale: f64,
    pub gravity: bool,
    pub overlay: bool,
    /// Simulated seconds since the scenario was built.
    pub time: f64,
    pub knobs: Vec<KnobView>,
    pub readout: Vec<String>,
}

pub struct Lab {
    pub scene: Scene,
    scenario: usize,
    /// Option index of every knob, per scenario, kept across switches.
    params: Vec<Vec<usize>>,
    /// Index into the shared knobs followed by the scenario's.
    selected: usize,
    time_scale: usize,
    pub gravity: bool,
    pub overlay: bool,
    pub paused: bool,
    clock: FixedStep,
    rendered: Mesh,
}

impl Lab {
    pub fn new() -> Self {
        let kind = Kind::ALL[0];
        let params: Vec<Vec<usize>> = Kind::ALL.iter().map(|k| k.defaults()).collect();
        let mut lab = Self {
            scene: Scene::new(kind, &params[0]),
            scenario: 0,
            params,
            selected: 0,
            time_scale: 3,
            gravity: kind.gravity(),
            overlay: true,
            paused: false,
            clock: FixedStep::new(DT, MAX_STEPS),
            rendered: Mesh::default(),
        };
        lab.redraw();
        lab
    }

    pub fn spawn() -> Vec3 {
        SPAWN
    }

    pub fn spawn_yaw() -> f32 {
        SPAWN_YAW
    }

    pub fn kind(&self) -> Kind {
        Kind::ALL[self.scenario]
    }

    fn knobs(&self) -> &'static [KnobDef] {
        self.kind().knobs()
    }

    fn knob_count(&self) -> usize {
        SHARED.len() + self.knobs().len()
    }

    /// Switch scenarios; gravity returns to the scenario's default.
    pub fn select(&mut self, kind: Kind) {
        self.scenario = Kind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        self.gravity = kind.gravity();
        self.selected = self.selected.min(self.knob_count() - 1);
        self.reset();
    }

    /// Rebuild the scenario from its knobs, discarding accumulated time.
    pub fn reset(&mut self) {
        self.scene = Scene::new(self.kind(), &self.params[self.scenario]);
        self.clock = FixedStep::new(DT, MAX_STEPS);
        self.redraw();
    }

    /// Set a scenario knob to the option nearest `value`. Returns false for
    /// an unknown knob.
    #[cfg(test)]
    pub fn set(&mut self, id: &str, value: f64) -> bool {
        let Some(index) = self.knobs().iter().position(|k| k.id == id) else {
            return false;
        };
        let def = self.knobs()[index];
        let option = def
            .options
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - value).abs().total_cmp(&(b.1 - value).abs()))
            .map_or(0, |(i, _)| i);
        self.set_option(index, option);
        true
    }

    fn set_option(&mut self, index: usize, option: usize) {
        let def = self.knobs()[index];
        let params = &mut self.params[self.scenario];
        if params[index] == option {
            return;
        }
        params[index] = option;
        if !def.live {
            self.reset();
        }
    }

    /// Select another knob; wraps at either end.
    pub fn cycle_knob(&mut self, forward: bool) {
        let n = self.knob_count();
        self.selected = if forward {
            (self.selected + 1) % n
        } else {
            (self.selected + n - 1) % n
        };
    }

    /// Move the selected knob one option; the scenario list wraps, the
    /// others stop at their ends.
    pub fn adjust(&mut self, up: bool) {
        match self.selected {
            0 => {
                let n = Kind::ALL.len();
                let next = if up {
                    (self.scenario + 1) % n
                } else {
                    (self.scenario + n - 1) % n
                };
                self.select(Kind::ALL[next]);
            }
            1 => {
                self.time_scale = if up {
                    (self.time_scale + 1).min(TIME_SCALES.len() - 1)
                } else {
                    self.time_scale.saturating_sub(1)
                };
            }
            2 => self.gravity = up,
            3 => self.overlay = up,
            i => {
                let index = i - SHARED.len();
                let len = self.knobs()[index].options.len();
                let current = self.params[self.scenario][index];
                let next = if up {
                    (current + 1).min(len - 1)
                } else {
                    current.saturating_sub(1)
                };
                self.set_option(index, next);
            }
        }
        self.redraw();
    }

    /// Advance the clock by frame time `dt`, s, unless paused.
    pub fn tick(&mut self, dt: f32) {
        if !self.paused {
            let steps = self
                .clock
                .advance(f64::from(dt) * TIME_SCALES[self.time_scale]);
            for _ in 0..steps {
                self.step_once();
            }
        }
        self.redraw();
    }

    /// Pause and advance exactly one fixed step.
    pub fn single_step(&mut self) {
        self.paused = true;
        self.clock.accumulator = 0.0;
        self.step_once();
        self.redraw();
    }

    fn step_once(&mut self) {
        let params = &self.params[self.scenario];
        self.scene.step(self.gravity, params);
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.redraw();
    }

    fn redraw(&mut self) {
        let alpha = if self.paused { 1.0 } else { self.clock.alpha() };
        self.rendered = draw::scene(&self.scene, alpha, self.overlay);
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// Floor grid, stage, rails, and stage footprint.
    pub fn world() -> World {
        draw::world()
    }

    pub fn snapshot(&self) -> Snapshot {
        let kind = self.kind();
        let params = &self.params[self.scenario];
        let shared = [
            ("scene", "Scene", kind.name().to_owned(), false),
            (
                "time",
                "Time scale",
                format!("{}×", TIME_SCALES[self.time_scale]),
                true,
            ),
            (
                "gravity",
                "Gravity",
                if self.gravity { "uniform" } else { "zero-g" }.to_owned(),
                true,
            ),
            (
                "overlay",
                "Overlay",
                if self.overlay { "on" } else { "off" }.to_owned(),
                true,
            ),
        ];
        let knobs = shared
            .into_iter()
            .chain(kind.knobs().iter().zip(params).map(|(def, index)| {
                (
                    def.id,
                    def.label,
                    (def.format)(def.options[*index]),
                    def.live,
                )
            }))
            .enumerate()
            .map(|(i, (id, label, value, live))| KnobView {
                id,
                label,
                value,
                selected: i == self.selected,
                live,
            })
            .collect();
        Snapshot {
            scenario: kind,
            name: kind.name(),
            number: self.scenario + 1,
            count: Kind::ALL.len(),
            paused: self.paused,
            time_scale: TIME_SCALES[self.time_scale],
            gravity: self.gravity,
            overlay: self.overlay,
            time: self.scene.time,
            knobs,
            readout: self.scene.readout(params, self.gravity),
        }
    }

    /// Up to four short lines: scenario and clock, the selected knob, and
    /// what the mechanism is doing.
    pub fn caption(snapshot: &Snapshot) -> String {
        let clock = if snapshot.paused {
            "paused".to_owned()
        } else {
            format!("{}×", snapshot.time_scale)
        };
        let gravity = if snapshot.gravity { "1 g" } else { "0 g" };
        let mut lines = vec![format!(
            "{}/{} {} · {clock} · {gravity}",
            snapshot.number, snapshot.count, snapshot.name
        )];
        let index = snapshot.knobs.iter().position(|k| k.selected).unwrap_or(0);
        if let Some(knob) = snapshot.knobs.get(index) {
            lines.push(format!(
                "[{}/{}] {} = {}",
                index + 1,
                snapshot.knobs.len(),
                knob.label,
                knob.value
            ));
        }
        lines.extend(snapshot.readout.iter().take(2).cloned());
        lines.join("\n")
    }
}
