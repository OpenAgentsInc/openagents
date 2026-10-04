//! The spell playground: a lit hall, one scripted scenario per spell, and a
//! headless run that a renderer records frame by frame.
//!
//! A scenario sets up the hall (creatures at spawn points, props, forced
//! saves), then drives the wizard only through admitted commands: turning,
//! moving, jumping, and casting. The run plays the scenario live, then
//! replays its key moment at 0.25× by restoring the checkpoint taken when
//! the moment began and stepping it again, and checks that the replay
//! reaches the same state as the live run.
//!
//! Register a scenario with one line in [`scenarios`].
use crate::play::{Ability, Game};
use crate::spells::{FEET, Target, Track};
use glam::{DVec3, Vec3};
use serde::Deserialize;
use std::collections::BTreeMap;
use verse_engine::director::{Actor, Scene};

/// The hall's collision profile name in its scene.
pub const PROFILE: &str = "spell-playground-v1";
/// Video frames per second and the live step.
pub const FPS: u32 = 30;
/// The key moment replays at this fraction of real time.
pub const REPLAY_SPEED: f32 = 0.25;
/// Longest video a scenario may produce, s.
pub const MAX_VIDEO_SECONDS: f32 = 25.;

#[derive(Clone, Debug, Deserialize)]
pub struct Solid {
    pub name: String,
    pub center: Vec3,
    pub half: Vec3,
    pub color: [f32; 3],
    #[serde(default)]
    pub emissive: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HallLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}

/// `assets/verse/playground/spells.json`: the hall's scene, solids,
/// spawn points, and lights.
#[derive(Clone, Debug, Deserialize)]
pub struct Hall {
    pub schema: String,
    pub scene: Scene,
    pub solids: Vec<Solid>,
    pub spawns: BTreeMap<String, Vec3>,
    pub lights: Vec<HallLight>,
    pub ambient: Vec3,
}

/// The hall, parsed once from the embedded asset.
pub fn hall() -> Result<&'static Hall, String> {
    static HALL: std::sync::OnceLock<Result<Hall, String>> = std::sync::OnceLock::new();
    HALL.get_or_init(|| {
        let hall: Hall = serde_json::from_slice(include_bytes!(
            "../../../assets/verse/playground/spells.json"
        ))
        .map_err(|e| e.to_string())?;
        hall.validate()?;
        Ok(hall)
    })
    .as_ref()
    .map_err(Clone::clone)
}

impl Hall {
    fn validate(&self) -> Result<(), String> {
        self.scene.validate()?;
        if self.schema != "openagents.verse.spell-playground.v1"
            || self.scene.collision_profile.as_deref() != Some(PROFILE)
            || self.solids.is_empty()
            || self.solids.len() > 256
            || self
                .solids
                .iter()
                .any(|s| !s.center.is_finite() || !s.half.is_finite() || s.half.min_element() <= 0.)
            || self.spawns.values().any(|p| !p.is_finite())
            || !self.spawns.contains_key("caster")
        {
            return Err("Invalid spell playground hall".into());
        }
        Ok(())
    }
    /// Collision boxes of the hall's solid (non-emissive) geometry.
    pub fn colliders(&self) -> Vec<physics::kinematic::Aabb> {
        self.solids
            .iter()
            .filter(|s| !s.emissive)
            .map(|s| physics::kinematic::Aabb {
                min: (s.center - s.half).as_dvec3(),
                max: (s.center + s.half).as_dvec3(),
            })
            .collect()
    }
    pub fn spawn(&self, name: &str) -> Result<Vec3, String> {
        self.spawns
            .get(name)
            .copied()
            .ok_or_else(|| format!("The playground hall has no spawn named {name}"))
    }
}

/// A camera keyframe: the view at scene time `at`.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub at: f32,
    pub eye: Vec3,
    pub target: Vec3,
}

/// One admitted command the wizard issues.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Turn to a yaw, radians; facing is `(-sin yaw, 0, -cos yaw)`.
    Face(f32),
    Cast(Ability),
    /// Walk or strafe input, held for one tick.
    Move([f32; 2]),
    Jump,
    /// A scripted hall device, such as an arrow turret or a catapult, acting
    /// through the game's own launch and impulse paths, never on the wizard.
    Device(Device),
}

/// A hall device's action.
#[derive(Clone, Copy)]
pub struct Device(pub fn(&mut Game) -> Result<(), String>);

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Device")
    }
}

impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cue {
    pub at: f32,
    pub step: Step,
}

/// One spell's playground recording.
pub struct Scenario {
    /// Lowercase name, as on the `--spell-playground` command line.
    pub key: &'static str,
    pub title: &'static str,
    /// The SRD line: level, range, area, save.
    pub srd: &'static str,
    pub seed: u64,
    /// Seconds of live action.
    pub live: f32,
    /// The key moment, scene seconds, replayed at [`REPLAY_SPEED`].
    pub replay: (f32, f32),
    /// Adds creatures to the hall's scene, usually at spawn points.
    pub setup: fn(&mut Scene, &Hall) -> Result<(), String>,
    /// Adds props and forced saves to the started game.
    pub populate: fn(&mut Game, &Hall) -> Result<(), String>,
    pub script: fn() -> Vec<Cue>,
    /// Camera keyframes over the live run.
    pub camera: fn() -> Vec<Shot>,
    /// Eye and target during the replay.
    pub replay_camera: (Vec3, Vec3),
    /// Checks the live result; the error explains what went wrong.
    pub check: fn(&Game) -> Result<(), String>,
}

/// Every registered scenario, one line per spell.
pub fn scenarios() -> Vec<Scenario> {
    vec![
        crate::spells::thunderwave::scenario(),
        crate::telekinesis::scenario(),
        crate::spells::wind_wall::scenario(),
        crate::spells::levitate::scenario(),
        crate::spells::feather_fall::scenario(),
        bow_stance(),
        crate::reverse_gravity::game::scenario(),
    ]
}

pub fn scenario(key: &str) -> Option<Scenario> {
    scenarios().into_iter().find(|s| s.key == key)
}

/// A creature for a scenario's scene: a training dummy unless `model` says
/// otherwise.
pub fn creature(id: u64, name: &str, model: &str, position: Vec3, yaw: f32, health: u32) -> Actor {
    Actor {
        id,
        name: name.into(),
        model: model.into(),
        position,
        yaw,
        scale: 1.,
        health,
        nameplate: true,
        friendly: false,
    }
}

/// Where a tracked target is now, and how far it has moved horizontally, m.
pub fn measure(game: &Game, track: &Track) -> Option<(DVec3, f64)> {
    let now = match track.target {
        Target::Actor(actor) => game.actor_position(actor)?.as_dvec3(),
        Target::Prop(index) => {
            if game.spells.props.get(index)?.removed {
                return None;
            }
            game.spells.prop_center(index)
        }
    };
    let d = now - track.start;
    Some((now, DVec3::new(d.x, 0., d.z).length()))
}

/// A scenario played live and then replayed, one video frame per call.
pub struct Run {
    pub scenario: Scenario,
    pub game: Game,
    live: Option<Game>,
    script: Vec<Cue>,
    next: usize,
    frame: u32,
    start: Option<(Vec<u8>, usize)>,
    end: Option<Vec<u8>>,
    pub replay_identical: Option<bool>,
    /// Interpolation between the last two ticks for the current frame.
    pub alpha: f32,
}

impl Run {
    pub fn new(scenario: Scenario) -> Result<Self, String> {
        let hall = hall()?;
        let mut scene = hall.scene.clone();
        (scenario.setup)(&mut scene, hall)?;
        if !scene.actors.iter().any(|a| a.nameplate) {
            // The chamber authority needs one creature; park it out of shot.
            scene.actors.push(creature(
                99,
                "Spare Dummy",
                "dummy",
                hall.spawn("spare")?,
                0.,
                100,
            ));
        }
        scene.validate()?;
        let mut game = Game::new(scene)?;
        game.spells.dice = crate::spells::Dice::new(scenario.seed);
        (scenario.populate)(&mut game, hall)?;
        let mut script = (scenario.script)();
        script.sort_by(|a, b| a.at.total_cmp(&b.at));
        let video = scenario.live + (scenario.replay.1 - scenario.replay.0) / REPLAY_SPEED;
        if !(0. ..scenario.live).contains(&scenario.replay.0)
            || scenario.replay.1 <= scenario.replay.0
            || scenario.replay.1 > scenario.live
            || video > MAX_VIDEO_SECONDS
        {
            return Err(format!(
                "Scenario {} runs {video:.1} s; the limit is {MAX_VIDEO_SECONDS} s",
                scenario.key
            ));
        }
        Ok(Self {
            scenario,
            game,
            live: None,
            script,
            next: 0,
            frame: 0,
            start: None,
            end: None,
            replay_identical: None,
            alpha: 1.,
        })
    }
    pub fn live_frames(&self) -> u32 {
        (self.scenario.live * FPS as f32).round() as u32
    }
    pub fn replay_ticks(&self) -> u32 {
        ((self.scenario.replay.1 - self.scenario.replay.0) * FPS as f32).round() as u32
    }
    pub fn frames(&self) -> u32 {
        self.live_frames() + self.replay_ticks() * (1. / REPLAY_SPEED) as u32
    }
    pub fn frame(&self) -> u32 {
        self.frame
    }
    pub fn replaying(&self) -> bool {
        self.frame > self.live_frames()
    }
    pub fn done(&self) -> bool {
        self.frame >= self.frames()
    }
    fn dispatch(&mut self) -> Result<(), String> {
        let mut movement = [0.; 2];
        while self
            .script
            .get(self.next)
            .is_some_and(|cue| cue.at <= self.game.time + 1e-4)
        {
            let cue = self.script[self.next];
            self.next += 1;
            match cue.step {
                Step::Face(yaw) => self.game.face(yaw)?,
                Step::Cast(ability) => self
                    .game
                    .activate(ability)
                    .map_err(|e| format!("{} at {:.2} s: {e}", ability.label(), cue.at))?,
                Step::Move(axes) => movement = axes,
                Step::Jump => self.game.jump()?,
                Step::Device(device) => (device.0)(&mut self.game)
                    .map_err(|e| format!("Device at {:.2} s: {e}", cue.at))?,
            }
        }
        self.game.tick(1. / FPS as f32, movement)
    }
    /// Advances one video frame.
    pub fn advance(&mut self) -> Result<(), String> {
        if self.done() {
            return Err("The playground run is complete".into());
        }
        let live = self.live_frames();
        if self.frame < live {
            if self.start.is_none() && self.game.time >= self.scenario.replay.0 - 1e-4 {
                self.start = Some((self.game.checkpoint()?, self.next));
            }
            self.dispatch()?;
            if self.end.is_none() && self.game.time >= self.scenario.replay.1 - 1e-4 {
                self.end = Some(self.game.checkpoint()?);
            }
            self.alpha = 1.;
        } else {
            let sub = (self.frame - live) % (1. / REPLAY_SPEED) as u32;
            if self.frame == live {
                let (bytes, next) = self.start.clone().ok_or("The key moment never began")?;
                (self.scenario.check)(&self.game)?;
                self.live = Some(std::mem::replace(&mut self.game, Game::restore(&bytes)?));
                self.next = next;
            }
            if sub == 0 {
                self.dispatch()?;
                if self.replay_identical.is_none()
                    && self.game.time >= self.scenario.replay.1 - 1e-4
                {
                    self.replay_identical = Some(Some(self.game.checkpoint()?) == self.end);
                }
            }
            self.alpha = (sub + 1) as f32 * REPLAY_SPEED;
        }
        self.frame += 1;
        Ok(())
    }
    /// The game the evidence describes: the live run once it has ended.
    pub fn result(&self) -> &Game {
        self.live.as_ref().unwrap_or(&self.game)
    }
    /// Eye and target for the current frame.
    pub fn camera(&self) -> (Vec3, Vec3) {
        if self.replaying() {
            return self.scenario.replay_camera;
        }
        let shots = (self.scenario.camera)();
        let t = self.game.time;
        let Some(first) = shots.first() else {
            return (Vec3::new(0., 6., -12.), Vec3::ZERO);
        };
        let mut view = (first.eye, first.target);
        for pair in shots.windows(2) {
            if t >= pair[0].at {
                let span = (pair[1].at - pair[0].at).max(1e-3);
                let f = ((t - pair[0].at) / span).clamp(0., 1.);
                let f = f * f * (3. - 2. * f);
                view = (
                    pair[0].eye.lerp(pair[1].eye, f),
                    pair[0].target.lerp(pair[1].target, f),
                );
            }
        }
        view
    }
    /// Text for the overlay: title lines, then log and measurements.
    pub fn overlay(&self) -> Vec<String> {
        let game = &self.game;
        let mut lines = vec![
            format!("{} - spell playground", self.scenario.title),
            self.scenario.srd.to_string(),
        ];
        lines.push(if self.replaying() {
            format!(
                "REPLAY {:.2}x from checkpoint  t = {:.2} s",
                REPLAY_SPEED, game.time
            )
        } else {
            format!("t = {:.2} s", game.time)
        });
        lines.push(String::new());
        let recent: Vec<_> = game.spells.log.iter().rev().take(7).collect();
        for record in recent.into_iter().rev() {
            lines.push(format!("[{:5.2}] {}", record.at, record.text));
        }
        lines.extend(crate::spells::feather_fall::overlay(game));
        lines.push(String::new());
        for track in game.spells.tracks.iter().rev().take(10).rev() {
            if let Some((_, moved)) = measure(game, track) {
                lines.push(format!(
                    "{:<18} moved {:5.1} ft{}",
                    track.label,
                    moved / FEET,
                    if track.requested > 0. {
                        format!(" (pushed {:.0} ft)", track.requested / FEET)
                    } else {
                        " (no push)".into()
                    }
                ));
            }
        }
        lines
    }
    /// The evidence JSON for `spell-<key>.json`.
    pub fn evidence(&self) -> Result<serde_json::Value, String> {
        let game = self.result();
        let error = game.spells.ledger_error();
        let tracks: Vec<_> = game
            .spells
            .tracks
            .iter()
            .map(|t| {
                let measured = measure(game, t);
                serde_json::json!({
                    "label": t.label, "spell": t.spell, "at": t.at,
                    "start": t.start.to_array(),
                    "final": measured.map(|(p, _)| p.to_array()),
                    "requested_ft": t.requested / FEET,
                    "measured_ft": measured.map(|(_, d)| d / FEET),
                })
            })
            .collect();
        let props: Vec<_> = (0..game.spells.props.len())
            .map(|i| {
                let p = &game.spells.props[i];
                let (center, rotation) = game.spells.prop_pose(i, 1.);
                serde_json::json!({
                    "name": p.name, "kind": p.spec.kind, "mass_kg": p.spec.mass,
                    "secured": p.spec.secured, "removed": p.removed,
                    "center": center.to_array(), "rotation": rotation.to_array(),
                })
            })
            .collect();
        let actors: Vec<_> = game
            .scene
            .actors
            .iter()
            .filter_map(|a| {
                game.actor_position(a.id).map(|p| {
                    serde_json::json!({"id": a.id, "name": a.name, "position": p.to_array(),
                        "health": game.frame().actors.iter().find(|f| f.actor.id == a.id).map(|f| f.health)})
                })
            })
            .collect();
        let saves: Vec<_> = game
            .spells
            .log
            .iter()
            .filter_map(|r| r.save.as_ref().map(|s| (r.at, s)))
            .map(|(at, s)| serde_json::json!({"at": at, "save": s}))
            .collect();
        Ok(serde_json::json!({
            "schema": "openagents.verse.spell-playground.v1",
            "spell": self.scenario.key,
            "title": self.scenario.title,
            "srd": self.scenario.srd,
            "srd_source": "SRD 5.2.1 (SRD_CC_v5.2.1.pdf)",
            "rules_revision": crate::play::RULES_REVISION,
            "seed": self.scenario.seed,
            "dice_rolled": game.spells.dice.rolled,
            "saves": saves,
            "log": game.spells.log.iter().map(|r| format!("[{:.2}] {}", r.at, r.text)).collect::<Vec<_>>(),
            "tracks": tracks,
            "props": props,
            "actors": actors,
            "ledger": {
                "terms": game.spells.ledger.external.iter().map(|(k, m)| (k.clone(), serde_json::json!({"linear": m.linear.to_array(), "angular": m.angular.to_array()}))).collect::<BTreeMap<_, _>>(),
                "residual_relative": {"linear": error.linear, "angular": error.angular},
                "tolerance": crate::spells::LEDGER_TOLERANCE,
            },
            "replay": {
                "window_s": [self.scenario.replay.0, self.scenario.replay.1],
                "speed": REPLAY_SPEED,
                "source": "checkpoint at the window start, stepped again with the same admitted commands",
                "identical_to_live": self.replay_identical,
            },
            "live_seconds": self.scenario.live,
            "video_seconds": self.frames() as f32 / FPS as f32,
            "authority_tick": game.authority_tick,
            "physics_steps": game.physics_steps,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_scenario_plays_replays_identically_and_passes_its_check() {
        for scenario in scenarios() {
            let key = scenario.key;
            let mut run = Run::new(scenario).unwrap();
            while !run.done() {
                run.advance().unwrap();
                assert!(!run.overlay().is_empty());
            }
            (run.scenario.check)(run.result()).unwrap();
            assert_eq!(run.replay_identical, Some(true), "{key}");
            let evidence = run.evidence().unwrap();
            assert!(evidence["video_seconds"].as_f64().unwrap() <= MAX_VIDEO_SECONDS as f64);
            let residual = &evidence["ledger"]["residual_relative"];
            assert!(residual["linear"].as_f64().unwrap() < crate::spells::LEDGER_TOLERANCE);
        }
    }
    #[test]
    fn the_hall_loads_with_its_named_profile() {
        let hall = hall().unwrap();
        assert!(hall.colliders().len() >= 8);
        let game = Game::new({
            let mut scene = hall.scene.clone();
            scene.actors.push(creature(
                2,
                "Dummy",
                "dummy",
                hall.spawn("open").unwrap(),
                0.,
                10,
            ));
            scene
        })
        .unwrap();
        let restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
        assert_eq!(restored.player, game.player);
    }
}

/// Not a spell: the archer stands with the bow stowed on his back, shoots a
/// dummy, keeps the bow drawn for [`crate::play::BOW_STANCE`], then stows it.
fn bow_stance() -> Scenario {
    use std::f32::consts::FRAC_PI_2;
    Scenario {
        key: "bow",
        title: "Bow stance",
        srd: "Stowed on the back; drawn in the left hand while shooting",
        seed: 1,
        live: 9.,
        replay: (2.3, 3.6),
        setup: |scene, hall| {
            scene.actors.push(creature(
                101,
                "Dummy",
                "dummy",
                hall.spawn("wall_station")?,
                FRAC_PI_2,
                100,
            ));
            Ok(())
        },
        populate: |_, _| Ok(()),
        script: || {
            vec![
                Cue {
                    at: 0.3,
                    step: Step::Face(-FRAC_PI_2),
                },
                Cue {
                    at: 2.5,
                    step: Step::Cast(Ability::Bow),
                },
            ]
        },
        camera: || {
            let back = (Vec3::new(3.9, 2.0, 1.5), Vec3::new(5.75, 1.2, 0.));
            let front = (Vec3::new(7.4, 1.7, 1.7), Vec3::new(5.75, 1.2, 0.));
            [
                (0., back),
                (2.2, back),
                (2.4, front),
                (6.4, front),
                (6.9, back),
                (9., back),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        replay_camera: (Vec3::new(7.4, 1.7, 1.7), Vec3::new(5.75, 1.2, 0.)),
        check: |_| Ok(()),
    }
}
