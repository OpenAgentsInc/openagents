//! Grow Little Bunny on the community-game contract (`verse-game`): the
//! generic inputs it takes, the HUD it asks for, and its replays. The
//! browser page drives the game only through this.

use bunny_rules::game::{Game, Input};
use bunny_rules::kinds::PowerKind;
use bunny_rules::receipt;
use bunny_rules::{FarmerState, Status, TIER_NAMES, TIER_QUARTERS, UNIT};
use verse_game::{CommunityGame, GameInput, Hud, HudElement, Icon, MapDot, Outcome};

/// The game's id.
pub const ID: &str = "grow-little-bunny";

/// The game's input for a generic input: Up jumps, Down ducks, Back turns
/// back. Use and Pause are the host's.
#[must_use]
pub fn input(input: GameInput) -> Option<Input> {
    match input {
        GameInput::Left => Some(Input::Left),
        GameInput::Right => Some(Input::Right),
        GameInput::Up => Some(Input::Jump),
        GameInput::Down => Some(Input::Duck),
        GameInput::Back => Some(Input::Back),
        GameInput::Use | GameInput::Pause => None,
    }
}

/// The colours the HUD's map uses.
pub const MAP_FOOD: u32 = 0xF28A1E;
pub const MAP_FARMER: u32 = 0x5C5C59;
pub const MAP_BONUS: u32 = 0x7CC242;

/// A run of the game, as a host sees it.
pub struct BunnyGame(pub Game);

impl BunnyGame {
    /// The farmer's bearing from the bunny, in radians from its heading,
    /// positive to the right, and how close he is from 0 (far) to 1.
    #[must_use]
    pub fn farmer_bearing(&self) -> (f32, f32) {
        let g = &self.0;
        let (bx, bz) = g.bunny_point();
        let (fx, fz) = g.farmer_point();
        let (hx, hz) = g.bunny.heading(&g.garden);
        let (dx, dz) = ((fx - bx) as f32, (fz - bz) as f32);
        let forward = dx * hx as f32 + dz * hz as f32;
        let right = -dx * hz as f32 + dz * hx as f32;
        let distance = (dx * dx + dz * dz).sqrt() / UNIT as f32;
        (
            right.atan2(forward),
            (1.0 - distance / 25.0).clamp(0.0, 1.0),
        )
    }

    fn map(&self) -> HudElement {
        let g = &self.0;
        let garden = &g.garden;
        let xs = garden.nodes.iter().map(|n| n.x);
        let zs = garden.nodes.iter().map(|n| n.z);
        let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(1));
        let (z0, z1) = (zs.clone().min().unwrap_or(0), zs.max().unwrap_or(1));
        let side = (x1 - x0).max(z1 - z0).max(1) as f32 * 1.1;
        let (cx, cz) = ((x0 + x1) as f32 / 2.0, (z0 + z1) as f32 / 2.0);
        let at = |x: i32, z: i32| (0.5 + (x as f32 - cx) / side, 0.5 + (z as f32 - cz) / side);
        let lines = garden
            .edges
            .iter()
            .map(|e| {
                let (a, b) = (garden.nodes[e.a], garden.nodes[e.b]);
                let (ax, ay) = at(a.x, a.z);
                let (bx, by) = at(b.x, b.z);
                [ax, ay, bx, by]
            })
            .collect();
        let mut dots = Vec::new();
        for (index, e) in garden.edibles.iter().enumerate() {
            if !g.eaten[index] {
                let (x, z) = garden.point(e.edge, e.s, i32::from(e.lane) * bunny_rules::LANE_WIDTH);
                let (x, y) = at(x, z);
                dots.push(MapDot {
                    x,
                    y,
                    colour: MAP_FOOD,
                    size: 0.008,
                });
            }
        }
        if g.bonus_out() {
            let spot = garden.bonus;
            let (x, z) = garden.point(spot.edge, spot.s, 0);
            let (x, y) = at(x, z);
            dots.push(MapDot {
                x,
                y,
                colour: MAP_BONUS,
                size: 0.025,
            });
        }
        if g.farmer_on && (g.farmer_sees() || g.golden > 0) {
            let (x, z) = g.farmer_point();
            let (x, y) = at(x, z);
            dots.push(MapDot {
                x,
                y,
                colour: MAP_FARMER,
                size: 0.03,
            });
        }
        let (x, z) = g.bunny_point();
        let (x, y) = at(x, z);
        dots.push(MapDot {
            x,
            y,
            colour: bunny_rules::shade::fur(0),
            size: 0.035,
        });
        HudElement::Map { lines, dots }
    }
}

impl CommunityGame for BunnyGame {
    fn id(&self) -> &'static str {
        ID
    }

    fn hz(&self) -> u32 {
        bunny_rules::HZ
    }

    fn step(&mut self, inputs: &[GameInput]) {
        let inputs: Vec<Input> = inputs.iter().filter_map(|i| input(*i)).collect();
        self.0.step(&inputs);
    }

    fn tick(&self) -> u32 {
        self.0.tick
    }

    fn outcome(&self) -> Option<Outcome> {
        match self.0.status {
            Status::Playing => None,
            Status::Won => Some(Outcome::Won),
            Status::Caught => Some(Outcome::Lost),
            Status::Left => Some(Outcome::Left),
        }
    }

    fn hud(&self) -> Hud {
        let g = &self.0;
        let food = g.garden.food().max(1);
        let tier = usize::from(g.bunny.tier);
        let bar = if tier + 1 < TIER_QUARTERS.len() {
            let (from, to) = (TIER_QUARTERS[tier], TIER_QUARTERS[tier + 1]);
            (g.bunny.quarters.saturating_sub(from)) as f32 / (to - from) as f32
        } else {
            1.0
        };
        let multiplier = if g.chain >= 8 {
            Some(2.0)
        } else if g.chain >= 4 {
            Some(1.5)
        } else {
            None
        };
        let mut hud = vec![
            HudElement::Counter {
                icon: Icon::Food,
                value: g.food_left as u32,
                ring: Some(1.0 - g.food_left as f32 / food as f32),
            },
            HudElement::Pips {
                icon: Icon::Size,
                filled: g.bunny.tier + 1,
                count: TIER_NAMES.len() as u8,
                bar: bar.clamp(0.0, 1.0),
                label: TIER_NAMES[tier],
            },
            HudElement::Score {
                value: g.score,
                multiplier,
            },
        ];
        if g.golden > 0 {
            hud.push(HudElement::Ring {
                icon: Icon::Power(0),
                left: g.golden as f32 / g.garden.farmer.spook.max(1) as f32,
            });
        }
        if let Some((kind, left)) = g.power {
            let index = PowerKind::ALL.iter().position(|k| *k == kind).unwrap_or(0) as u8;
            hud.push(HudElement::Ring {
                icon: Icon::Power(index + 1),
                left: left as f32 / kind.duration().max(1) as f32,
            });
        }
        if g.farmer_on && !matches!(g.farmer.state, FarmerState::Shed { .. }) {
            let (angle, weight) = self.farmer_bearing();
            let mark = match g.farmer.state {
                FarmerState::Chase { .. } => Some(Icon::Danger),
                FarmerState::Search { .. } => Some(Icon::Search),
                _ if g.farmer.windup > 0 => Some(Icon::Danger),
                _ => None,
            };
            hud.push(HudElement::EdgeArrow {
                angle,
                weight,
                mark,
            });
        }
        hud.push(self.map());
        hud
    }

    fn state_digest(&self) -> [u8; 32] {
        self.0.state_digest()
    }

    fn rules_digest(&self) -> [u8; 32] {
        receipt::rules_digest()
    }

    fn level_digest(&self) -> [u8; 32] {
        self.0.garden.digest
    }

    fn seed(&self) -> u64 {
        self.0.seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunny_rules::level;
    use verse_game::Replay;

    fn fresh() -> BunnyGame {
        BunnyGame(Game::with_seed(level::garden(2), 4, false))
    }

    #[test]
    fn the_bot_plays_through_the_contract_and_its_replay_verifies() {
        let mut game = fresh();
        let mut replay = Replay::start(&game);
        for _ in 0..bunny_rules::HZ * 120 {
            let generic: Vec<GameInput> = bunny_rules::bot::input(&game.0)
                .map(|i| match i {
                    Input::Left => GameInput::Left,
                    Input::Right => GameInput::Right,
                    Input::Jump => GameInput::Up,
                    Input::Duck => GameInput::Down,
                    Input::Back | Input::Leave => GameInput::Back,
                })
                .into_iter()
                .collect();
            replay.step(&mut game, &generic);
            if game.outcome().is_some() {
                break;
            }
        }
        assert!(!replay.inputs.is_empty());
        let text = replay.encode();
        let read = Replay::decode(&text).unwrap();
        read.verify(&mut fresh()).unwrap();
        // A replay from another garden is refused.
        let mut other = BunnyGame(Game::with_seed(level::garden(1), 4, false));
        assert!(read.verify(&mut other).is_err());
    }

    #[test]
    fn the_hud_shows_food_size_score_and_the_map() {
        let game = fresh();
        let hud = game.hud();
        assert!(matches!(
            hud[0],
            HudElement::Counter {
                icon: Icon::Food,
                ..
            }
        ));
        assert!(matches!(
            hud[1],
            HudElement::Pips {
                filled: 1,
                count: 5,
                ..
            }
        ));
        assert!(matches!(hud[2], HudElement::Score { value: 0, .. }));
        let HudElement::Map { lines, dots } = hud.last().unwrap() else {
            panic!("a map last");
        };
        assert_eq!(lines.len(), game.0.garden.edges.len());
        assert!(dots.len() > game.0.garden.food());
        for dot in dots {
            assert!((0.0..=1.0).contains(&dot.x) && (0.0..=1.0).contains(&dot.y));
        }
        assert_eq!(input(GameInput::Up), Some(Input::Jump));
        assert_eq!(input(GameInput::Pause), None);
    }
}
