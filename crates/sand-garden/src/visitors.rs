//! Visitors favor the habitat the person makes, and stay on sand or in water.
use crate::{HEIGHT, Point, WIDTH};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Crab,
    Hermit,
    Sandpiper,
    Turtle,
    Minnow,
    Shrimp,
    SeaStar,
}
impl Kind {
    pub fn aquatic(self) -> bool {
        matches!(self, Self::Minnow | Self::Shrimp | Self::SeaStar)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Crab => "Crab",
            Self::Hermit => "Hermit crab",
            Self::Sandpiper => "Sandpiper",
            Self::Turtle => "Sea turtle",
            Self::Minnow => "Little fish",
            Self::Shrimp => "Shrimp",
            Self::SeaStar => "Sea star",
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Habitat {
    pub water: f64,
    pub anemones: usize,
    pub shells: usize,
}
impl Habitat {
    pub fn weights(self) -> Vec<(Kind, f64)> {
        let water = self.water.clamp(0., 1.);
        let sand = 1. - water;
        let mut out = vec![];
        if sand > 0.04 {
            out.extend([
                (Kind::Crab, sand * 4.),
                (Kind::Hermit, sand * (2. + self.shells.min(5) as f64)),
                (Kind::Sandpiper, sand * 2.),
                (Kind::Turtle, sand),
            ]);
        }
        if water > 0.015 {
            out.extend([
                (Kind::Minnow, water * 7.),
                (Kind::Shrimp, water * (2. + self.anemones.min(6) as f64)),
                (Kind::SeaStar, water * (1. + self.anemones.min(6) as f64)),
            ]);
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct Guest {
    pub kind: Kind,
    pub position: Point,
    pub heading: f64,
    pub phase: f64,
    pub alpha: f64,
    age: f64,
    life: f64,
}
pub struct Visitors {
    pub guests: Vec<Guest>,
    seed: u32,
    last: Option<f64>,
    next: f64,
}
impl Visitors {
    pub fn new(seed: u32) -> Self {
        Self {
            guests: vec![],
            seed: seed.max(1),
            last: None,
            next: 1.5,
        }
    }
    fn random(&mut self) -> f64 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        self.seed as f64 / u32::MAX as f64
    }
    pub fn clear(&mut self) {
        self.guests.clear();
        self.last = None;
        self.next = 1.5;
    }
    pub fn step(
        &mut self,
        time: f64,
        habitat: Habitat,
        water: impl Fn(Point) -> bool,
        motion: bool,
    ) {
        let dt = self.last.map_or(0., |last| (time - last).clamp(0., 0.08));
        self.last = Some(time);
        self.next -= dt;
        for g in &mut self.guests {
            g.age += dt;
            if motion {
                g.phase += dt * 5.;
            }
            g.alpha = (g.age / 1.5).min((g.life - g.age) / 1.5).clamp(0., 1.);
            if !motion {
                g.alpha = 1.;
                continue;
            }
            let speed = match g.kind {
                Kind::Sandpiper => 19.,
                Kind::Minnow => 18.,
                Kind::Shrimp => 10.,
                Kind::SeaStar => 0.7,
                Kind::Turtle => 5.,
                _ => 9.,
            };
            let x = g.position.x + g.heading.cos() * dt * speed;
            let y = g.position.y + g.heading.sin() * dt * speed;
            let p = Point::new(x, y);
            if x < 20.
                || x > WIDTH - 20.
                || y < 20.
                || y > HEIGHT - 20.
                || water(p) != g.kind.aquatic()
            {
                g.heading += 1.8;
            } else {
                g.position = p;
            }
        }
        self.guests
            .retain(|g| g.age < g.life && water(g.position) == g.kind.aquatic());
        if self.next > 0. || self.guests.len() >= 3 {
            return;
        }
        self.next = 7. + self.random() * 6.;
        let weights = habitat.weights();
        let total: f64 = weights.iter().map(|(_, w)| w).sum();
        let mut pick = self.random() * total;
        let mut kind = Kind::Crab;
        for (k, w) in weights {
            kind = k;
            pick -= w;
            if pick <= 0. {
                break;
            }
        }
        for _ in 0..80 {
            let position = Point::new(
                30. + self.random() * (WIDTH - 60.),
                30. + self.random() * (HEIGHT - 60.),
            );
            if water(position) != kind.aquatic() {
                continue;
            }
            let heading = self.random() * std::f64::consts::TAU;
            let life = 22. + self.random() * 12.;
            self.guests.push(Guest {
                kind,
                position,
                heading,
                phase: 0.,
                alpha: if motion { 0. } else { 1. },
                age: 0.,
                life,
            });
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sand_and_water_choose_different_visitors_and_anemones_favor_pool_life() {
        assert!(
            Habitat::default()
                .weights()
                .iter()
                .all(|(k, _)| !k.aquatic())
        );
        let wet = Habitat {
            water: 1.,
            ..Habitat::default()
        };
        assert!(wet.weights().iter().all(|(k, _)| k.aquatic()));
        let rich = Habitat { anemones: 4, ..wet };
        let weight = |h: Habitat| {
            h.weights()
                .into_iter()
                .find(|(k, _)| *k == Kind::Shrimp)
                .unwrap()
                .1
        };
        assert!(weight(rich) > weight(wet));
    }
    #[test]
    fn reduced_motion_guests_start_visible_and_stay_still() {
        let mut v = Visitors::new(19);
        let h = Habitat::default();
        for i in 0..50 {
            v.step(i as f64 / 25., h, |_| false, false);
            if !v.guests.is_empty() {
                break;
            }
        }
        assert!(!v.guests.is_empty());
        let g = v.guests[0].clone();
        assert_eq!(g.alpha, 1.);
        v.step(2.1, h, |_| false, false);
        assert_eq!(v.guests[0].position.x, g.position.x);
        assert_eq!(v.guests[0].position.y, g.position.y);
        assert_eq!(v.guests[0].phase, 0.);
        assert_eq!(v.guests[0].alpha, 1.);
    }
    #[test]
    fn swimmers_stay_in_water_and_visitors_expire() {
        let mut v = Visitors::new(7);
        let h = Habitat {
            water: 1.,
            ..Habitat::default()
        };
        for i in 0..1000 {
            v.step(i as f64 / 20., h, |_| true, true);
            assert!(v.guests.iter().all(|g| g.kind.aquatic() && g.age < g.life));
        }
        assert!(!v.guests.is_empty());
        v.clear();
        assert!(v.guests.is_empty());
    }
}
