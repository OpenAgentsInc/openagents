//! What can be in a garden: edibles, obstacles, and power-ups, with the
//! spec's tables (`docs/verse/games/grow-little-bunny.md`, Growth, Edibles,
//! Power-ups) as code.

use crate::ticks;

/// Something the bunny eats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdibleKind {
    Seedling,
    Carrot,
    Radish,
    Lettuce,
    Strawberry,
    Pumpkin,
    /// The golden carrot: the farmer is spooked while it lasts.
    Golden,
    /// The bonus vegetable that appears in the middle of the garden for a
    /// while; optional for a win.
    Bonus,
}

impl EdibleKind {
    pub const ALL: [Self; 8] = [
        Self::Seedling,
        Self::Carrot,
        Self::Radish,
        Self::Lettuce,
        Self::Strawberry,
        Self::Pumpkin,
        Self::Golden,
        Self::Bonus,
    ];

    /// Points for eating it (the bonus vegetable's depend on the set).
    #[must_use]
    pub fn points(self) -> u32 {
        match self {
            Self::Seedling => 10,
            Self::Carrot => 50,
            Self::Radish => 80,
            Self::Lettuce => 100,
            Self::Strawberry => 150,
            Self::Pumpkin => 300,
            Self::Golden => 200,
            Self::Bonus => 0,
        }
    }

    /// Growth it gives, in quarter growth points.
    #[must_use]
    pub fn quarters(self) -> u32 {
        match self {
            Self::Seedling => 1,
            Self::Carrot | Self::Golden => 8,
            Self::Radish | Self::Strawberry => 4,
            Self::Lettuce => 12,
            Self::Pumpkin => 20,
            Self::Bonus => 0,
        }
    }

    /// The smallest tier (0-based) that can eat it; smaller bunnies bump
    /// into it as if it were an obstacle.
    #[must_use]
    pub fn min_tier(self) -> u8 {
        match self {
            Self::Lettuce => 2,
            Self::Pumpkin => 4,
            _ => 0,
        }
    }

    /// Its name in level files.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Seedling => "seedling",
            Self::Carrot => "carrot",
            Self::Radish => "radish",
            Self::Lettuce => "lettuce",
            Self::Strawberry => "strawberry",
            Self::Pumpkin => "pumpkin",
            Self::Golden => "golden",
            Self::Bonus => "bonus",
        }
    }
}

/// The bonus vegetable's points for each garden set (1 to 4).
#[must_use]
pub fn bonus_points(set: u8) -> u32 {
    match set {
        0 | 1 => 500,
        2 => 800,
        3 => 1_200,
        _ => 2_000,
    }
}

/// What happens when a bunny of some size meets an obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// Runs past or steps over it.
    Pass,
    /// Runs through it at this share of its speed, in percent, for a moment.
    Slow(u8),
    /// Passable only in a jump; running into it is a tumble.
    Jump,
    /// Passable only ducking; running into it is a tumble.
    Duck,
    /// Smashes, breaks, tears or knocks it over at full speed, for points.
    Smash,
    /// A tumble.
    Block,
}

impl Contact {
    /// Whether some action gets the bunny past.
    #[must_use]
    pub fn passable(self) -> bool {
        self != Self::Block
    }
}

/// An obstacle placed in a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObstacleKind {
    /// A fence panel: a wall for every size.
    Fence,
    /// A gap under a fence: only a Kit or a Bunny fits.
    Gap,
    /// A bean-pole tunnel: only a Kit or a Bunny fits.
    Tunnel,
    Hose,
    Puddle,
    Tray,
    Pot,
    Can,
    Gnome,
    Wire,
    Barrow,
    Scarecrow,
    BirdNet,
}

impl ObstacleKind {
    pub const ALL: [Self; 13] = [
        Self::Fence,
        Self::Gap,
        Self::Tunnel,
        Self::Hose,
        Self::Puddle,
        Self::Tray,
        Self::Pot,
        Self::Can,
        Self::Gnome,
        Self::Wire,
        Self::Barrow,
        Self::Scarecrow,
        Self::BirdNet,
    ];

    /// The spec's "What each tier can do" table: what a bunny of `tier`
    /// (0-based: Kit, Bunny, Jack, Big Bun, Giant) does to it.
    #[must_use]
    pub fn contact(self, tier: u8) -> Contact {
        use Contact::{Block, Duck, Jump, Pass, Slow, Smash};
        let row: [Contact; 5] = match self {
            Self::Fence => [Block; 5],
            Self::Gap | Self::Tunnel => [Pass, Pass, Block, Block, Block],
            Self::Hose => [Jump, Jump, Pass, Pass, Pass],
            Self::Puddle => [Slow(60), Slow(80), Pass, Pass, Pass],
            Self::Tray => [Block, Jump, Jump, Smash, Smash],
            Self::Pot => [Block, Block, Smash, Smash, Smash],
            Self::Can => [Block, Block, Jump, Smash, Smash],
            Self::Gnome => [Block, Block, Block, Smash, Smash],
            Self::Wire => [Block, Block, Block, Smash, Smash],
            Self::Barrow => [Block, Block, Block, Jump, Smash],
            Self::Scarecrow => [Block, Block, Block, Block, Smash],
            Self::BirdNet => [Duck, Duck, Duck, Duck, Smash],
        };
        row[usize::from(tier.min(4))]
    }

    /// Points for smashing, breaking or knocking it over.
    #[must_use]
    pub fn smash_points(self) -> u32 {
        match self {
            Self::Tray => 25,
            Self::Pot | Self::Can | Self::BirdNet => 50,
            Self::Gnome | Self::Wire => 75,
            Self::Barrow | Self::Scarecrow => 100,
            _ => 0,
        }
    }

    /// Whether the farmer can't walk a corridor with this obstacle in it:
    /// he never climbs fences or crawls through the bunny's gaps.
    #[must_use]
    pub fn bars_farmer(self) -> bool {
        matches!(self, Self::Fence | Self::Gap | Self::Tunnel)
    }

    /// Its name in level files.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Fence => "fence",
            Self::Gap => "gap",
            Self::Tunnel => "tunnel",
            Self::Hose => "hose",
            Self::Puddle => "puddle",
            Self::Tray => "tray",
            Self::Pot => "pot",
            Self::Can => "can",
            Self::Gnome => "gnome",
            Self::Wire => "wire",
            Self::Barrow => "barrow",
            Self::Scarecrow => "scarecrow",
            Self::BirdNet => "birdnet",
        }
    }
}

/// A power-up's kind. The golden carrot is an edible ([`EdibleKind::Golden`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PowerKind {
    /// Runs 25% faster; tumbles cost no growth.
    Clover,
    /// The next jumps float and clear any obstacle.
    Dandelion,
    /// The farmer's sight halves.
    SunHat,
    /// Edibles in the neighbouring lanes are pulled into the bunny's.
    Magnet,
}

impl PowerKind {
    pub const ALL: [Self; 4] = [Self::Clover, Self::Dandelion, Self::SunHat, Self::Magnet];

    /// How long it lasts, in ticks (the dandelion counts jumps instead).
    #[must_use]
    pub fn duration(self) -> u32 {
        match self {
            Self::Clover | Self::Magnet => ticks(6_000),
            Self::Dandelion => ticks(30_000),
            Self::SunHat => ticks(10_000),
        }
    }

    /// Its name in level files.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Clover => "clover",
            Self::Dandelion => "dandelion",
            Self::SunHat => "sunhat",
            Self::Magnet => "magnet",
        }
    }
}

/// How many floating jumps a dandelion puff gives.
pub const DANDELION_JUMPS: u8 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pass_table_matches_the_spec() {
        use Contact::{Block, Duck, Jump, Pass, Slow, Smash};
        let table = |kind: ObstacleKind| (0..5).map(|t| kind.contact(t)).collect::<Vec<_>>();
        assert_eq!(table(ObstacleKind::Gap), [Pass, Pass, Block, Block, Block]);
        assert_eq!(table(ObstacleKind::Hose), [Jump, Jump, Pass, Pass, Pass]);
        assert_eq!(
            table(ObstacleKind::Puddle),
            [Slow(60), Slow(80), Pass, Pass, Pass]
        );
        assert_eq!(table(ObstacleKind::Tray), [Block, Jump, Jump, Smash, Smash]);
        assert_eq!(
            table(ObstacleKind::Pot),
            [Block, Block, Smash, Smash, Smash]
        );
        assert_eq!(table(ObstacleKind::Can), [Block, Block, Jump, Smash, Smash]);
        assert_eq!(
            table(ObstacleKind::Gnome),
            [Block, Block, Block, Smash, Smash]
        );
        assert_eq!(
            table(ObstacleKind::Wire),
            [Block, Block, Block, Smash, Smash]
        );
        assert_eq!(
            table(ObstacleKind::Barrow),
            [Block, Block, Block, Jump, Smash]
        );
        assert_eq!(
            table(ObstacleKind::Scarecrow),
            [Block, Block, Block, Block, Smash]
        );
        assert_eq!(
            table(ObstacleKind::BirdNet),
            [Duck, Duck, Duck, Duck, Smash]
        );
        assert_eq!(table(ObstacleKind::Fence), [Block; 5]);
        for kind in ObstacleKind::ALL {
            let smashes = (0..5).any(|t| kind.contact(t) == Smash);
            assert_eq!(smashes, kind.smash_points() > 0, "{kind:?}");
        }
    }

    #[test]
    fn edibles_match_the_spec() {
        let gp = |kind: EdibleKind| kind.quarters() as f32 / 4.0;
        assert_eq!(gp(EdibleKind::Seedling), 0.25);
        assert_eq!(gp(EdibleKind::Carrot), 2.0);
        assert_eq!(gp(EdibleKind::Radish), 1.0);
        assert_eq!(gp(EdibleKind::Lettuce), 3.0);
        assert_eq!(gp(EdibleKind::Strawberry), 1.0);
        assert_eq!(gp(EdibleKind::Pumpkin), 5.0);
        assert_eq!(gp(EdibleKind::Golden), 2.0);
        assert_eq!(EdibleKind::Lettuce.min_tier(), 2);
        assert_eq!(EdibleKind::Pumpkin.min_tier(), 4);
        assert_eq!(bonus_points(1), 500);
        assert_eq!(bonus_points(4), 2_000);
    }
}
