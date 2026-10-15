//! Snap Station's local game area beside Lagrange 1. The host steps the
//! community-game contract; the same scene used in the browser supplies its meshes.
use crate::{
    mesh::Mesh,
    pbr::{GlowVertex, LitVertex, Material},
    render::View,
};
use glam::{Mat4, Vec3};
use snap_rules::{Status, level};
use snap_web::{
    scene,
    zone::{Slot, SnapGame},
};
use verse_game::{CommunityGame, GameInput, Replay};

pub const ORIGIN: Vec3 = Vec3::new(50., 12., 0.);
pub fn arch() -> super::Gate {
    super::Gate {
        at: super::Lagrange::spawn() + Vec3::new(5., 0., -6.),
        yaw: std::f32::consts::PI,
    }
}

pub struct Station {
    pub game: SnapGame,
    pub job: usize,
    pub paused: bool,
    pub replay: Replay,
    pending: Vec<GameInput>,
    carry: f64,
}
impl Default for Station {
    fn default() -> Self {
        Self::new(1)
    }
}
impl Station {
    pub fn new(job: usize) -> Self {
        let job = job.clamp(1, level::COUNT);
        let game = SnapGame::new(level::job(job));
        Self {
            replay: Replay::start(&game),
            game,
            job,
            paused: false,
            pending: Vec::new(),
            carry: 0.,
        }
    }
    pub fn input(&mut self, input: GameInput) {
        if input == GameInput::Pause {
            self.paused = !self.paused;
            self.pending.clear();
            self.carry = 0.;
        } else if !self.paused && self.pending.len() < 32 {
            self.pending.push(input);
        }
    }
    pub fn tick(&mut self, dt: f32) {
        if self.paused || !dt.is_finite() || dt <= 0. {
            return;
        }
        self.carry += f64::from(dt.min(0.25));
        let step = 1. / f64::from(snap_rules::HZ);
        while self.carry + 1e-9 >= step {
            self.carry = (self.carry - step).max(0.);
            let inputs = std::mem::take(&mut self.pending);
            self.replay.step(&mut self.game, &inputs);
        }
    }
    pub fn next(&mut self, direction: isize) {
        let job =
            (self.job as isize - 1 + direction).rem_euclid(level::COUNT as isize) as usize + 1;
        *self = Self::new(job);
    }
    pub fn caption(&self) -> String {
        let game = &self.game.game;
        let status = if self.paused {
            "Paused".to_owned()
        } else {
            match game.status {
                Status::Won => format!("Job done! {} stars", game.stars),
                Status::Failed(_) => "Try another build · X to fix".into(),
                Status::Running => format!("Test {}/{}", game.phase + 1, game.level.tests.len()),
                Status::Building => "Build, then choose Go".into(),
            }
        };
        let slot = self.game.carrying.map_or_else(
            || match self.game.slots().get(self.game.slot) {
                Some(Slot::Piece(kind)) => snap_web::copy::piece(*kind),
                _ => "Go",
            },
            snap_web::copy::piece,
        );
        let mut pieces = 0;
        let mut meters = Vec::new();
        for element in self.game.hud() {
            match element {
                verse_game::HudElement::Counter {
                    icon: verse_game::Icon::Score,
                    value,
                    ..
                } => pieces = value,
                verse_game::HudElement::Ring { left, .. } => {
                    meters.push(format!("Storm: {:.0}%", left * 100.))
                }
                verse_game::HudElement::Pips {
                    label,
                    filled,
                    count,
                    bar,
                    ..
                } => meters.push(format!("{label}: {filled}/{count} · {:.0}%", bar * 100.)),
                _ => {}
            }
        }
        let goal = if matches!(game.status, Status::Won | Status::Failed(_)) {
            status.clone()
        } else {
            snap_web::copy::test_goal(game.level.job).into()
        };
        let last = if meters.is_empty() {
            "Arrows move · E use · R turn · X undo".into()
        } else {
            meters.join(" · ")
        };
        format!(
            "{}. {}\n{}\n{} · {} · Pieces: {}\n{}",
            self.job,
            game.level.name,
            goal,
            if self.paused {
                "Paused"
            } else if game.status == Status::Running {
                "Running"
            } else {
                "Selected"
            },
            slot,
            pieces,
            last
        )
    }

    pub fn view(&self, aspect: f32) -> View {
        let (low, high) = scene::bounds(&self.game.game);
        let size = (high - low).max(glam::Vec2::new(5., 4.));
        let centre = (low + high) * 0.5;
        let aspect = if aspect.is_finite() {
            aspect.clamp(0.25, 4.)
        } else {
            1.
        };
        let distance =
            (size.y * 1.8).max(size.x / aspect * 1.2) * 0.5 / (20_f32.to_radians().tan());
        let target = ORIGIN + Vec3::new(centre.x, centre.y - size.y * 0.25, 0.);
        let eye = target + Vec3::Z * distance;
        View {
            eye,
            view_proj: Mat4::perspective_rh(40_f32.to_radians(), aspect, 0.1, 2000.)
                * Mat4::look_at_rh(eye, target, Vec3::Y),
        }
    }
    pub fn mesh(&self, mut sky: crate::pbr::Sky) -> Mesh {
        let held = self.game.carrying.map(|kind| scene::Held {
            kind,
            at: self.game.cursor,
            turn: self.game.best_turn(kind),
        });
        let drawn = scene::draw(&scene::Frame {
            game: &self.game.game,
            time: self.game.tick() as f32 / snap_rules::HZ as f32,
            held,
            contrast: false,
            marks: false,
            sparks: &[],
        });
        let mut mesh = Mesh::default();
        sky.sun_dir = Vec3::new(0.35, 0.55, 0.85).normalize();
        sky.shadow_center = ORIGIN;
        sky.shadow_half = 30.;
        sky.probes = None;
        sky.camera = crate::pbr::Camera::art();
        mesh.sky = Some(sky);
        for source in [&drawn.lit, &drawn.ghost] {
            for v in source.data.chunks_exact(snap_web::mesh::STRIDE) {
                let local = Vec3::from_slice(&v[..3]);
                mesh.lit.push(LitVertex {
                    pos: (ORIGIN + local).to_array(),
                    local: local.to_array(),
                    normal: [v[3], v[4], v[5]],
                    tangent: [1., 0., 0.],
                    color: [v[6].powf(2.2), v[7].powf(2.2), v[8].powf(2.2)],
                    params: [0., 0.65, Material::Lacquer.code(), 1.],
                });
            }
        }
        for v in drawn.glow.data.chunks_exact(snap_web::mesh::STRIDE) {
            mesh.glow.push(GlowVertex {
                pos: (ORIGIN + Vec3::from_slice(&v[..3])).to_array(),
                radiance: [v[6] * 25_000., v[7] * 25_000., v[8] * 25_000.],
                uv: [0., 0.],
            });
        }
        // The keyboard's cursor is visible even with no piece held.
        let p = ORIGIN + Vec3::new(self.game.cursor.x as f32, self.game.cursor.y as f32, 0.5);
        for axis in [Vec3::X, Vec3::Y] {
            for point in [p - axis * 0.18, p + axis * 0.18] {
                mesh.lines.push(crate::mesh::Vertex {
                    pos: point.to_array(),
                    color: [1.; 3],
                    fog: 0.,
                });
            }
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controller::InputState,
        runtime::WorldRuntime,
        zones::{Intent, ZoneId},
    };
    fn press(station: &mut Station, input: GameInput) {
        station.input(input);
        station.tick(1. / 60.);
    }
    #[test]
    fn a_hosted_cursor_run_and_rotation_replay_through_the_contract() {
        use GameInput::{Left, Right, Turn, Use};
        let mut station = Station::default();
        for input in [
            Use, Turn, Turn, Turn, Use, Use, Right, Right, Use, Use, Right, Right, Use, Left, Use,
        ] {
            press(&mut station, input);
        }
        for _ in 0..10_000 {
            if station.game.outcome().is_some() {
                break;
            }
            station.tick(1. / 60.);
        }
        assert_eq!(station.game.game.status, Status::Won);
        station
            .replay
            .verify(&mut SnapGame::new(level::job(1)))
            .unwrap();
        let tick = station.game.tick();
        station.input(GameInput::Pause);
        station.tick(1.);
        assert_eq!(station.game.tick(), tick);
        station.input(GameInput::Pause);
        station.tick(1. / 60.);
        assert!(station.game.tick() > tick);
        station.next(1);
        assert_eq!(station.job, 2);
        assert_eq!(station.game.tick(), 0);
        station.next(-1);
        assert_eq!(station.job, 1);
    }
    #[test]
    fn the_arch_opens_a_local_game_and_returns_to_the_same_station() {
        let mut runtime = WorldRuntime::new();
        assert!(runtime.enter_snap_station().is_err());
        runtime.install_lagrange();
        assert!(runtime.enter_snap_station().is_err());
        let gate = arch();
        runtime.set_spawn(gate.at, 0.).unwrap();
        runtime.walk_through_portals(gate.at - Vec3::Z * 2., 1. / 60.);
        assert_eq!(runtime.zone_label(), "Snap Station");
        assert_eq!(runtime.zone, ZoneId::Lagrange1);
        let pos = runtime.player.pos;
        runtime.tick(
            &InputState {
                forward: true,
                ..Default::default()
            },
            0.05,
        );
        assert_eq!(
            runtime.player.pos, pos,
            "game controls must not move the astronaut"
        );
        assert_eq!(runtime.zone_snapshot(1.5).controls.len(), 12);
        assert!(!runtime.zone_dynamic_mesh().lit.is_empty());
        runtime.zone_intent(Intent::Grab).unwrap();
        runtime.tick(&InputState::default(), 1. / 60.);
        assert!(runtime.snap_station().unwrap().game.carrying.is_some());
        runtime.zone_intent(Intent::Return).unwrap();
        assert!(runtime.snap_station().is_none());
        assert!(runtime.zone_state.lagrange.is_some());
        runtime.walk_through_portals(pos, 1. / 60.);
        assert!(runtime.snap_station().is_none());
        runtime.zone_intent(Intent::Return).unwrap();
        assert_eq!(runtime.zone, ZoneId::Plaza);
    }
    #[test]
    fn all_jobs_fit_the_physical_mesh_and_camera_budgets() {
        let mut runtime = WorldRuntime::new();
        runtime.install_lagrange();
        let sky = runtime
            .zone_state
            .lagrange
            .as_ref()
            .unwrap()
            .dynamic()
            .sky
            .clone()
            .unwrap();
        for job in 1..=level::COUNT {
            let station = Station::new(job);
            let mesh = station.mesh(sky.clone());
            assert!(!mesh.lit.is_empty());
            assert!(mesh.lit.len() / 3 < 40_000);
            assert!(
                mesh.lit
                    .iter()
                    .all(|v| v.pos.into_iter().chain(v.normal).all(f32::is_finite))
            );
            assert!(station.view(1.5).view_proj.is_finite());
        }
    }
}
