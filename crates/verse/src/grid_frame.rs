//! The Grid's frame as engine presentation: the pinned pack's placements,
//! the arches the runtime stands, and a `grid/figure` instance for the local
//! player and every NIP-MV peer, lit from the zone's atmosphere. The desktop
//! app and the capture example assemble the same frame here and hand it to
//! [`crate::imported::Renderer`], which draws it through
//! [`verse_engine::render_world::RenderWorld`].

use std::time::Duration;
use web_time::Instant;

use glam::{Mat4, Quat, Vec3};
use verse_engine::lighting::{HeightFog, Lighting};
use verse_engine::motion::{Selection, State};
use verse_engine::presentation::Instance;

use crate::avatar::Gait;
use crate::crowd::Figure;
use crate::grid_pack;
use crate::runtime::WorldRuntime;
use crate::zones::Atmosphere;

/// Ground speed at and above which a figure runs rather than walks, meters
/// per second. The player controller's run speed is 7.
pub const RUN_SPEED: f32 = 6.0;
/// Ground speed below which a figure stands still.
pub const STILL_SPEED: f32 = 0.1;
/// Feet height above which a figure is airborne.
pub const AIRBORNE_HEIGHT: f32 = 0.05;

/// Engine lighting for the Grid: no sun, no shadows, and the legacy
/// atmosphere's distance fog as height fog with zero falloff.
#[must_use]
pub fn lighting(atmosphere: &Atmosphere) -> Lighting {
    Lighting {
        ambient: Vec3::ZERO,
        fog: Vec3::from_array(atmosphere.color),
        density: 0.0,
        shadowed: 0,
        height_fog: Some(HeightFog {
            density: std::f32::consts::LN_2 / ((atmosphere.fog_end - atmosphere.fog_start) / 2.0),
            base: 0.0,
            falloff: 0.0,
            start: atmosphere.fog_start,
            max_opacity: 1.0,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }),
        ..Default::default()
    }
}

/// The figure graph state for a character moving at `speed` meters per
/// second with its feet at `height`.
#[must_use]
pub fn state(speed: f32, height: f32) -> State {
    if height > AIRBORNE_HEIGHT {
        State::Airborne
    } else if speed >= RUN_SPEED {
        State::Run
    } else if speed > STILL_SPEED {
        State::Walk
    } else {
        State::Idle
    }
}

/// A `grid/figure` instance with its feet at `pos` facing `rot`, its clip
/// chosen from `speed` and height and its time from the walk cycle.
#[must_use]
pub fn figure(pos: Vec3, rot: Quat, gait: &Gait, speed: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: grid_pack::FIGURE.into(),
        transform: Mat4::from_rotation_translation(rot, pos),
        animation: Selection::Named(state(speed, pos.y)),
        time: gait.cycle(),
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

/// A fixed instance of `model` at `transform`.
#[must_use]
pub fn fixed(model: &str, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

/// The Grid's static instances: the pack's placements. The arches are
/// dynamic, because which ones stand changes as zones load.
#[must_use]
pub fn statics(pack: &verse_engine::assets::Pack) -> Vec<Instance> {
    grid_pack::placements(pack)
}

/// A visitor from the feed: where they stand and which way they face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standing {
    pub pos: Vec3,
    pub yaw: f32,
}

/// The Grid's dynamic instances this frame: the arches the runtime stands,
/// the local player unless the camera is in first person or nobody plays
/// here, every peer the crowd shows (avatars as figures, agents as spades),
/// standing visitors, and the Grid robots: four on their patrols and one
/// dancing by the Everglade arch.
#[must_use]
pub fn dynamic(runtime: &WorldRuntime, crowd: &[Figure], visitors: &[Standing]) -> Vec<Instance> {
    let mut out = grid_pack::gates(runtime);
    out.reserve(1 + crowd.len() + visitors.len());
    if !runtime.first_person() && !runtime.is_unoccupied() {
        out.push(figure(
            runtime.player.pos,
            Quat::from_rotation_y(runtime.player.yaw),
            &runtime.gait,
            runtime.player.speed,
        ));
    }
    for peer in crowd {
        match peer.role.as_str() {
            "avatar" => out.push(figure(peer.pos, peer.rot, &peer.gait, peer.speed)),
            "agent" => out.push(fixed(
                grid_pack::SPADE,
                Mat4::from_rotation_translation(peer.rot, peer.pos),
            )),
            _ => {}
        }
    }
    for visitor in visitors {
        out.push(figure(
            visitor.pos,
            Quat::from_rotation_y(visitor.yaw),
            &Gait::default(),
            0.0,
        ));
    }
    for robot in runtime.robots() {
        out.push(crate::grid_robot::instance(&robot, runtime.framing().eye));
    }
    out
}

/// One second of frame times, as `--frame-times` prints them, one JSON
/// object per line.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct FrameTimes {
    /// Seconds since the window opened.
    pub at_s: f32,
    /// Frames in this second.
    pub frames: usize,
    /// Dynamic instances in the last frame.
    pub instances: usize,
    /// Median frame-to-frame interval, milliseconds.
    pub p50_ms: f32,
    /// 95th percentile frame-to-frame interval, milliseconds.
    pub p95_ms: f32,
    /// Longest frame-to-frame interval, milliseconds.
    pub max_ms: f32,
    /// The renderer's own time for the last frame, milliseconds.
    pub render_ms: f64,
}

/// Collects frame intervals and emits [`FrameTimes`] once a second.
#[derive(Debug)]
pub struct Timing {
    started: Instant,
    window: Instant,
    intervals: Vec<f32>,
}

impl Timing {
    #[must_use]
    pub fn start(now: Instant) -> Self {
        Self {
            started: now,
            window: now,
            intervals: Vec::new(),
        }
    }

    /// Records a frame that took `dt` seconds since the last one; returns the
    /// second's summary when one has passed.
    pub fn frame(
        &mut self,
        now: Instant,
        dt: f32,
        instances: usize,
        render_ms: f64,
    ) -> Option<FrameTimes> {
        self.intervals.push(dt * 1000.0);
        if now.duration_since(self.window) < Duration::from_secs(1) {
            return None;
        }
        let mut sorted = std::mem::take(&mut self.intervals);
        sorted.sort_by(f32::total_cmp);
        let at = |q: f32| sorted[((sorted.len() - 1) as f32 * q).round() as usize];
        let out = FrameTimes {
            at_s: now.duration_since(self.started).as_secs_f32(),
            frames: sorted.len(),
            instances,
            p50_ms: at(0.5),
            p95_ms: at(0.95),
            max_ms: *sorted.last().unwrap_or(&0.0),
            render_ms,
        };
        self.window = now;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_follow_speed_and_height() {
        assert_eq!(state(0.0, 0.0), State::Idle);
        assert_eq!(state(2.0, 0.0), State::Walk);
        assert_eq!(state(7.0, 0.0), State::Run);
        assert_eq!(state(7.0, 1.0), State::Airborne);
    }

    #[test]
    fn the_bare_grid_frame_has_one_figure_per_player_and_its_arches() {
        let runtime = WorldRuntime::bare();
        let pack = grid_pack::load_pinned().unwrap();
        let placed = statics(&pack);
        assert!(placed.iter().any(|i| i.model.as_str() == grid_pack::FLOOR));
        assert!(
            placed
                .iter()
                .all(|i| i.model != grid_pack::GYM && i.model != grid_pack::BOARDS)
        );
        let arches = grid_pack::gates(&runtime).len();
        let mut gait = Gait::default();
        gait.advance(3.0, false, 0.5);
        let peers = vec![
            Figure {
                pubkey: "a".into(),
                id: "me".into(),
                role: "avatar".into(),
                pos: Vec3::new(2.0, 0.0, 2.0),
                rot: Quat::IDENTITY,
                online: true,
                speed: 3.0,
                gait,
            },
            Figure {
                pubkey: "b".into(),
                id: "spade".into(),
                role: "agent".into(),
                pos: Vec3::new(4.0, 0.0, 2.0),
                rot: Quat::IDENTITY,
                online: true,
                speed: 0.0,
                gait: Gait::default(),
            },
        ];
        let dynamic = dynamic(
            &runtime,
            &peers,
            &[Standing {
                pos: Vec3::X,
                yaw: 0.0,
            }],
        );
        let figures = &dynamic[arches..];
        // The player, the avatar, the agent's spade, the visitor, the four
        // Grid robots on their patrols, and the one dancing by the Everglade
        // arch.
        assert_eq!(figures.len(), 9);
        assert!(
            figures[4..8]
                .iter()
                .all(|f| f.model.as_str().starts_with(crate::grid_robot::ROBOT))
        );
        assert_eq!(
            figures[8].animation,
            Selection::Legacy(crate::grid_robot::CLIP_DANCE)
        );
        assert_eq!(figures[0].animation, Selection::Named(State::Idle));
        assert_eq!(figures[1].animation, Selection::Named(State::Walk));
        assert!((figures[1].time - gait.cycle()).abs() < 1e-6);
        assert_eq!(figures[2].model.as_str(), grid_pack::SPADE);
        let spectator = WorldRuntime::unoccupied();
        assert_eq!(
            self::dynamic(&spectator, &[], &[]).len(),
            grid_pack::gates(&spectator).len() + 5,
            "a spectator sees the arches and the five robots"
        );
    }

    #[test]
    fn timing_summarizes_each_second() {
        let start = Instant::now();
        let mut timing = Timing::start(start);
        let mut summaries = Vec::new();
        for i in 1..=60u64 {
            let now = start + Duration::from_millis(i * 17);
            summaries.extend(timing.frame(now, 0.017, 21, 2.5));
        }
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].frames, 59);
        assert!((summaries[0].p50_ms - 17.0).abs() < 1e-3);
        assert_eq!(summaries[0].instances, 21);
    }
}
