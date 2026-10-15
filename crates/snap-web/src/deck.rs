//! The Crew Deck, its four job boards, and builds kept on this device.

use glam::{Vec2, Vec3};
use snap_rules::{Game, Input, Status, level, progress::Progress, receipt};

use crate::{
    kit::{self, Crew, Mood, Style},
    mesh::{Mesh, rgb},
    scene::Drawn,
    zone::SnapGame,
};

pub const TITLE: &str = "Crew Deck";
pub const SAVED: &str = "Your builds";
pub const STARS: &str = "Star board";
pub const HINT: &str = "Tap to float. Visit a board or use the buttons.";
pub const EMPTY: &str = "Build something, then come back to see it here.";
pub const SAVE_KEY: &str = "snap.deck-builds.v1";
const MAX_INPUTS: usize = 512;

#[derive(Clone, Copy, Debug)]
pub struct Board {
    pub title: &'static str,
    pub crew: Crew,
    pub at: Vec2,
}

pub const BOARDS: [Board; 4] = [
    Board {
        title: "Dock Yard",
        crew: Crew::Cog,
        at: Vec2::new(-5., 2.),
    },
    Board {
        title: "Garden Ring",
        crew: Crew::Rivet,
        at: Vec2::new(5., 2.),
    },
    Board {
        title: "Storm Watch",
        crew: Crew::Nova,
        at: Vec2::new(-5., -2.),
    },
    Board {
        title: "Deep Space",
        crew: Crew::Tock,
        at: Vec2::new(5., -2.),
    },
];

/// The introductory jobs keep their original order; later jobs come in sets.
pub fn set_for(job: usize) -> usize {
    match job {
        1 | 5 | 10 | 12 | 15 => 0,
        4 | 7 | 9 | 14 | 19 => 1,
        3 | 8 | 13 | 16 | 18 => 2,
        _ => 3,
    }
}

pub fn jobs(board: usize) -> Vec<usize> {
    (1..=level::COUNT)
        .filter(|n| set_for(*n) == board)
        .collect()
}

#[derive(Clone, Debug, Default)]
pub struct Deck {
    pub at: Vec2,
    pub target: Vec2,
}

impl Deck {
    pub fn go(&mut self, at: Vec2) {
        self.target = at.clamp(Vec2::new(-7., -4.), Vec2::new(7., 4.));
    }
    pub fn step(&mut self, dt: f32) {
        let d = self.target - self.at;
        self.at += d.normalize_or_zero() * d.length().min(dt.max(0.).min(0.25) * 4.);
    }
    pub fn board_at(at: Vec2) -> Option<usize> {
        BOARDS.iter().position(|b| b.at.distance(at) < 1.4)
    }
}

/// One build per job. Only construction inputs are kept; a run starts fresh.
#[derive(Clone, Debug)]
pub struct Saved {
    pub job: usize,
    pub inputs: Vec<Input>,
    preview: Mesh,
}

impl Saved {
    pub fn game(&self) -> Option<SnapGame> {
        if !(1..=level::COUNT).contains(&self.job) || self.inputs.len() > MAX_INPUTS {
            return None;
        }
        let mut game = SnapGame::new(level::job(self.job));
        for i in &self.inputs {
            if !matches!(i, Input::Place { .. } | Input::Remove { .. }) {
                return None;
            }
            game.step_rules(&[*i]);
        }
        if game.game.status != Status::Building {
            return None;
        }
        if game.game.level.storm.is_some() {
            game.step_rules(&[Input::Go, Input::Fix]);
        }
        Some(game)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    pub builds: Vec<Saved>,
}

impl Library {
    pub fn exhibit_at(&self, at: Vec2) -> Option<usize> {
        self.builds
            .iter()
            .take(20)
            .enumerate()
            .find(|(i, _)| at.distance(exhibit_spot(*i)) < 0.8)
            .map(|(_, b)| b.job)
    }

    pub fn keep(&mut self, game: &SnapGame) {
        if game.game.build.used() == 0 || game.construction.len() > MAX_INPUTS {
            return;
        }
        let job = usize::from(game.game.level.number);
        self.builds.retain(|b| b.job != job);
        let mut preview = Mesh::new();
        miniature(&mut preview, &game.game, Vec2::ZERO);
        self.builds.push(Saved {
            job,
            inputs: game.construction.clone(),
            preview,
        });
        self.builds.sort_by_key(|b| b.job);
    }
    pub fn encode(&self) -> String {
        self.builds
            .iter()
            .map(|b| {
                format!(
                    "{} {}",
                    b.job,
                    b.inputs
                        .iter()
                        .map(|i| receipt::encode_input(*i))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn decode(text: &str) -> Self {
        let mut out = Self::default();
        if text.len() > 32_768 * level::COUNT {
            return out;
        }
        for line in text.lines().take(level::COUNT) {
            if line.len() > 32_768 {
                continue;
            }
            let mut words = line.split_whitespace();
            let Some(job) = words.next().and_then(|w| w.parse().ok()) else {
                continue;
            };
            let inputs: Result<Vec<_>, _> = words
                .take(MAX_INPUTS + 1)
                .map(receipt::decode_input)
                .collect();
            let Ok(inputs) = inputs else {
                continue;
            };
            let saved = Saved {
                job,
                inputs,
                preview: Mesh::new(),
            };
            if let Some(game) = saved.game() {
                out.keep(&game);
            }
        }
        out
    }
}

fn exhibit_spot(i: usize) -> Vec2 {
    Vec2::new(-3.6 + (i % 5) as f32 * 1.8, -3.5 - (i / 5) as f32 * 0.95)
}

fn pad(mesh: &mut Mesh, at: Vec2, radius: f32, colour: u32) {
    mesh.disc(at, radius, 12, 0., -0.5, -0.35, rgb(colour));
}

/// A station with the crew, boards, stars, miniature builds, and a little pod.
pub fn draw(deck: &Deck, progress: &Progress, library: &Library, time: f32) -> Drawn {
    let mut out = Drawn::default();
    out.lit
        .disc(Vec2::ZERO, 7.8, 12, 0., -0.85, -0.65, rgb(0x263653));
    for b in BOARDS {
        out.lit
            .rod(Vec2::ZERO, b.at, 0.55, -0.64, -0.55, rgb(0x7585A8));
        pad(&mut out.lit, b.at, 1.5, 0x425572);
        kit::robot(
            &mut out.lit,
            b.crew,
            b.at + Vec2::new(0., 0.35),
            time,
            Mood::Waiting,
        );
        out.glow.rod(
            b.at + Vec2::new(-0.65, -0.8),
            b.at + Vec2::new(0.65, -0.8),
            0.16,
            0.,
            0.08,
            rgb(b.crew.colour()),
        );
    }
    kit::robot(
        &mut out.lit,
        Crew::Pip,
        Vec2::new(0., 3.4),
        time,
        Mood::Cheering,
    );
    // Each earned star lights one window on the central board.
    for n in 0..(level::COUNT * 3) {
        let at = Vec2::new(-1.5 + (n % 10) as f32 * 0.33, 2.6 - (n / 10) as f32 * 0.33);
        let colour = if n < progress.stars() as usize {
            0xFFD166
        } else {
            0x435370
        };
        for i in 0..10 {
            let corner = |n: u32| {
                let a = std::f32::consts::FRAC_PI_2 + n as f32 * std::f32::consts::PI / 5.;
                let r = if n % 2 == 0 { 0.13 } else { 0.06 };
                Vec3::new(at.x + a.cos() * r, at.y + a.sin() * r, 0.04)
            };
            out.glow
                .tri(at.extend(0.04), corner(i), corner(i + 1), rgb(colour));
        }
    }
    for (i, saved) in library.builds.iter().take(20).enumerate() {
        let at = exhibit_spot(i);
        pad(&mut out.lit, at, 0.55, 0x506583);
        let start = out.lit.data.len();
        out.lit.data.extend_from_slice(&saved.preview.data);
        for v in out.lit.data[start..].chunks_exact_mut(crate::mesh::STRIDE) {
            v[0] += at.x;
            v[1] += at.y;
        }
    }
    out.lit.sphere(
        Vec3::new(deck.at.x, deck.at.y, 0.65),
        0.28,
        6,
        10,
        rgb(0xE5F5FF),
    );
    out.glow.disc(
        deck.at - Vec2::Y * 0.28,
        0.12,
        6,
        0.,
        0.5,
        0.65,
        rgb(0x64DFFF),
    );
    out
}

fn miniature(mesh: &mut Mesh, game: &Game, at: Vec2) {
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for p in &game.build.pieces {
        for q in &p.at {
            let q = Vec2::new(q.x as f32, q.y as f32);
            low = low.min(q);
            high = high.max(q);
        }
    }
    let scale = 1.2 / (high - low).max_element().max(1.);
    let mid = (low + high) * 0.5;
    for p in &game.build.pieces {
        let corners: Vec<_> =
            p.at.iter()
                .map(|q| at + (Vec2::new(q.x as f32, q.y as f32) - mid) * scale)
                .collect();
        kit::tile(mesh, p.kind, &corners, Style::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_saved_builds_fit_the_decks_mesh_budget() {
        let mut library = Library::default();
        for n in 1..=level::COUNT {
            let level = level::job(n);
            let plan = snap_rules::bot::solve(&level).unwrap();
            let mut game = SnapGame::new(level);
            for input in plan.into_iter().filter(|i| !matches!(i, Input::Go)) {
                game.step_rules(&[input]);
            }
            library.keep(&game);
        }
        assert_eq!(library.builds.len(), 20);
        let drawn = draw(&Deck::default(), &Progress::default(), &library, 0.);
        let triangles = drawn.lit.triangles() + drawn.glow.triangles();
        assert!(triangles < 40_000, "{triangles} triangles");
    }
    #[test]
    fn all_jobs_have_one_board_and_the_deck_stays_bounded() {
        let all: Vec<_> = (0..4).flat_map(jobs).collect();
        for board in 0..4 {
            assert_eq!(jobs(board).len(), 5);
        }
        assert_eq!(all.len(), level::COUNT);
        for n in 1..=level::COUNT {
            assert_eq!(all.iter().filter(|j| **j == n).count(), 1);
        }
        let mut d = Deck::default();
        d.go(Vec2::splat(100.));
        for _ in 0..100 {
            d.step(0.1);
        }
        assert_eq!(d.at, Vec2::new(7., 4.));
        assert_eq!(Deck::board_at(BOARDS[2].at), Some(2));
    }
    #[test]
    fn saved_builds_reopen_with_their_inventory_and_draw_from_the_save() {
        let mut g = SnapGame::new(level::job(1));
        for i in snap_rules::bot::solve(&level::job(1))
            .unwrap()
            .into_iter()
            .filter(|i| !matches!(i, Input::Go))
        {
            g.step_rules(&[i]);
        }
        let mut l = Library::default();
        l.keep(&g);
        let read = Library::decode(&l.encode());
        assert_eq!(read.builds.len(), 1);
        let mut restored = read.builds[0].game().unwrap();
        assert_eq!(restored.game.build.used(), g.game.build.used());
        assert_eq!(restored.game.tray, g.game.tray);
        restored.step_rules(&[Input::Go]);
        while restored.game.status == Status::Running {
            restored.step_rules(&[]);
        }
        assert_eq!(restored.game.status, Status::Won);
        assert_eq!(read.exhibit_at(Vec2::new(-3.6, -3.5)), Some(1));
        let plain = draw(
            &Deck::default(),
            &Progress::default(),
            &Library::default(),
            0.,
        );
        let full = draw(&Deck::default(), &Progress::default(), &read, 0.);
        assert!(full.lit.vertices() > plain.lit.vertices());
        assert!(full.lit.data.iter().all(|v| v.is_finite()));
        assert!(Library::decode("999 G\n1 G\n1 nonsense").builds.is_empty());
    }
}
