//! Every number a playtest may change (*tunable* in the design doc), in
//! metres, seconds and health points. The Roblox port reads the same
//! values.

/// The player's body radius and speeds.
pub const PLAYER_RADIUS: f32 = 0.35;
pub const WALK: f32 = 4.5;
pub const SPRINT: f32 = 7.0;
/// A jump's starting speed and gravity: about a metre high.
pub const JUMP_SPEED: f32 = 5.5;
pub const GRAVITY: f32 = 16.0;

pub const MAX_HEALTH: f32 = 100.0;
/// A sprinkle pellet's damage.
pub const SPRINKLE_DAMAGE: f32 = 5.0;
/// A grab's damage and shove.
pub const GRAB_DAMAGE: f32 = 30.0;
pub const GRAB_SHOVE: f32 = 8.0;
/// Health comes back this fast once no hit landed for [`REGEN_AFTER`].
pub const REGEN_AFTER: f32 = 10.0;
pub const REGEN_RATE: f32 = 4.0;

/// The Ice Cream Man.
pub const MAN_RADIUS: f32 = 0.5;
pub const MAN_WANDER: f32 = 2.4;
pub const MAN_INVESTIGATE: f32 = 3.6;
pub const MAN_SEARCH: f32 = 3.2;
/// Faster than a walk, slower than a sprint: a sprint gets you clear,
/// for a while (v5, #11667; v4 had him at 4.48, just under a walk).
pub const MAN_CHASE: f32 = 5.6;
pub const SIGHT: f32 = 22.0;
/// Half his field of view while not chasing, as a cosine (70 degrees).
pub const SIGHT_COS: f32 = 0.342;
/// Up close he notices you whichever way he faces.
pub const NOTICE: f32 = 4.0;
/// How long he searches the last place he saw you.
pub const SEARCH_TIME: f32 = 7.0;
/// How often a wandering Ice Cream Man sniffs out the player's room.
pub const SNIFF_EVERY: f32 = 25.0;
/// Once this many keys are taken he keeps going back to the front door
/// (0: never).
pub const PATROL_KEYS: u32 = 2;
/// Chasing, he cuts corners instead of going by every doorway's middle
/// (1) or not (0).
pub const CORNERS: u32 = 1;

/// Sprinkle volleys: the range, the pellets, their speed and the wait.
pub const VOLLEY_MIN: f32 = 3.0;
pub const VOLLEY_MAX: f32 = 15.0;
pub const VOLLEY_PELLETS: usize = 5;
/// The fan's step between pellets, in degrees.
pub const VOLLEY_SPREAD: f32 = 6.0;
pub const PELLET_SPEED: f32 = 11.0;
pub const VOLLEY_WAIT: f32 = 2.4;
/// A pellet misses a player whose feet are this high (a jump dodges).
pub const PELLET_DODGE: f32 = 0.45;
/// The grab's reach and wait, and how long he gloats after one.
pub const GRAB_REACH: f32 = 1.25;
pub const GRAB_WAIT: f32 = 2.0;
pub const GLOAT: f32 = 1.0;

/// Hot fudge.
pub const FUDGE_START: u32 = 30;
pub const FUDGE_MAX: u32 = 60;
pub const FUDGE_PICKUP: u32 = 15;
pub const FUDGE_RATE: f32 = 6.0;
pub const FUDGE_SPEED: f32 = 16.0;
pub const FUDGE_RANGE: f32 = 13.0;
/// Each hit slows him this long, to this share of his speed.
pub const FUDGE_SLOW: f32 = 2.5;
pub const SLOWED: f32 = 0.5;
/// Hits that freeze him, how long hits keep counting, and the freeze.
pub const FREEZE_HITS: u32 = 6;
pub const FUDGE_MEMORY: f32 = 4.0;
pub const FROZEN: f32 = 3.0;

/// The fart launcher.
pub const FARTS_START: u32 = 3;
pub const FARTS_MAX: u32 = 5;
pub const FART_RANGE: f32 = 4.5;
pub const FART_RADIUS: f32 = 3.5;
pub const FART_SHOVE: f32 = 10.0;
pub const CONFUSED: f32 = 4.0;
pub const FART_WAIT: f32 = 1.0;
pub const CLOUD_LIFE: f32 = 3.0;

/// The key ring's jingle: how far it carries and the wait.
pub const JINGLE_NOISE: f32 = 30.0;
pub const JINGLE_WAIT: f32 = 1.5;

/// How far noises carry.
pub const SPRINT_NOISE: f32 = 14.0;
pub const FUDGE_NOISE: f32 = 12.0;
pub const FRIDGE_NOISE: f32 = 8.0;
pub const DOOR_NOISE: f32 = 25.0;

/// Reach for closets, fridges, the front door and keys.
pub const REACH: f32 = 1.4;
pub const KEY_REACH: f32 = 0.9;
/// Holding E at the front door this long unlocks it.
pub const UNLOCK: f32 = 6.0;
/// The unlock stops and starts over while he is this near (0: never).
pub const UNLOCK_GUARD: f32 = 8.0;
/// One key is locked in the cone cabinet, behind the sprinkle-button
/// puzzle (1), or all three lie on the floor (0).
pub const PUZZLE_KEY: u32 = 1;

/// A shove fades this much each second.
pub const SHOVE_FADE: f32 = 6.0;

/// Hiding spots added to the house beyond its rooms' own closets
/// (`House::tuned`).
pub const EXTRA_CLOSETS: u32 = 2;
/// The most spare closets a house can take: one in each bedroom, kitchen
/// and bathroom, and every house has at least four of those.
pub const MAX_EXTRA_CLOSETS: u32 = 4;
/// The house is this many rooms along each side (v4 had 4 by 4).
pub const HOUSE_SIZE: u32 = 7;
/// The biggest house: [`crate::layout::MAX_SIZE`] rooms along a side.
pub const MAX_HOUSE_SIZE: u32 = crate::layout::MAX_SIZE as u32;
/// The keys go to the house's far ends, far from the bedroom, the front
/// door and each other (1), or into any rooms (0).
pub const KEY_SPREAD: u32 = 1;
/// His field of view while not chasing, in degrees either side of where
/// he faces ([`SIGHT_COS`] is its cosine).
pub const SIGHT_ANGLE: f32 = 70.0;

/// Earned weapons (1: in the game; the original game had none, see
/// [`ORIGINAL`]).
pub const EARNED_WEAPONS: u32 = 1;
/// The Ice Cream Man's health against the earned weapons.
pub const MAN_HEALTH: f32 = 400.0;
pub const SWORD_DAMAGE: f32 = 25.0;
pub const SWORD_REACH: f32 = 1.9;
pub const SWORD_WAIT: f32 = 0.45;
/// Minions to melt before one drops the candy cane sword.
pub const SWORD_TASK: u32 = 1;
pub const SPEAR_DAMAGE: f32 = 35.0;
pub const SPEAR_REACH: f32 = 3.4;
pub const SPEAR_WAIT: f32 = 0.8;
pub const PISTOL_DAMAGE: f32 = 15.0;
pub const PISTOL_RATE: f32 = 3.0;
pub const PISTOL_SPEED: f32 = 22.0;
pub const PISTOL_RANGE: f32 = 18.0;
pub const PISTOL_AMMO: u32 = 12;
pub const PISTOL_MAX: u32 = 40;
pub const PISTOL_PICKUP: u32 = 8;
/// Times you must freeze him with hot fudge to earn the cone pistol.
pub const PISTOL_TASK: u32 = 1;
pub const SUGAR_DAMAGE: f32 = 60.0;
pub const SUGAR_RADIUS: f32 = 2.5;
pub const SUGAR_SPEED: f32 = 12.0;
pub const SUGAR_RANGE: f32 = 14.0;
pub const SUGAR_WAIT: f32 = 1.2;
pub const SUGAR_AMMO: u32 = 4;
pub const SUGAR_MAX: u32 = 10;
pub const SUGAR_PICKUP: u32 = 2;
/// Minions you must melt to earn the sugar launcher.
pub const SUGAR_TASK: u32 = 4;

/// Minions (none in the original game, see [`ORIGINAL`]).
pub const MINION_RADIUS: f32 = 0.3;
pub const MINION_COUNT: u32 = 3;
pub const MINION_FIRST: f32 = 25.0;
pub const MINION_EVERY: f32 = 15.0;
/// One more minion may hunt you every this many seconds of a run (0:
/// never more than [`MINION_COUNT`]).
pub const MINION_GROWTH: f32 = 60.0;
pub const MINION_SPEED: f32 = 3.4;
pub const MINION_HEALTH: f32 = 30.0;
pub const MINION_DAMAGE: f32 = 6.0;
pub const MINION_BITE_WAIT: f32 = 1.0;
pub const MINION_SIGHT: f32 = 14.0;
pub const FUDGE_MINION_DAMAGE: f32 = 10.0;
pub const FART_MINION_DAMAGE: f32 = 40.0;
/// The chance a melted minion drops an edible ice cream cone, and what a
/// cone gives: extra health as a share of full health, for a while.
pub const CONE_DROP: f32 = 0.35;
pub const CONE_BOOST: f32 = 0.5;
pub const CONE_TIME: f32 = 10.0;

/// The sprinkle staff: one super long sprinkle that hurts what it touches
/// and makes friendly minions where it ends.
pub const STAFF_LENGTH: f32 = 10.0;
pub const STAFF_DAMAGE: f32 = 30.0;
pub const STAFF_WAIT: f32 = 2.0;
pub const STAFF_MINIONS: u32 = 3;
pub const STAFF_MINION_LIFE: f32 = 20.0;
/// After a win, play on in free roam with the staff (1) or end (0).
pub const FREE_ROAM: u32 = 1;

/// The house as the Backrooms (0: the Airbnb's rooms), and the doorways
/// added beyond the ones that join every room.
pub const BACKROOMS: u32 = 1;
pub const LOOPS: u32 = 7;
/// The Airbnb's own extra doorways, as the original game built it.
pub const AIRBNB_LOOPS: u32 = 4;
/// The look: how yellowed the rooms are, the fog, the lights' flicker and
/// buzz, and the camera a run starts with.
pub const YELLOW: f32 = 1.0;
pub const FOG_NEAR: f32 = 3.0;
pub const FOG_FAR: f32 = 18.0;
pub const FLICKER: f32 = 0.35;
pub const BUZZ: f32 = 0.8;
pub const FIRST_PERSON: u32 = 0;

/// The game as it played before the designer's revision (#11626), which
/// Game Lab keeps as version 1: the values that differ from today's
/// defaults. A game document saved before a setting existed plays that
/// setting at its value here.
pub const ORIGINAL: &[(&str, f64)] = &[
    ("man_wander", 2.4),
    ("man_investigate", 3.6),
    ("man_search", 3.2),
    ("man_chase", 5.6),
    ("extra_closets", 0.0),
    ("earned_weapons", 0.0),
    ("minion_count", 0.0),
    ("free_roam", 0.0),
    ("backrooms", 0.0),
    ("loops", AIRBNB_LOOPS as f64),
    ("yellow", 0.0),
    ("fog_near", 9.0),
    ("fog_far", 34.0),
    ("flicker", 0.03),
    ("buzz", 0.0),
    ("sprint_noise", 8.0),
    ("unlock", 2.0),
    ("unlock_guard", 0.0),
    ("puzzle_key", 0.0),
    ("house_size", 4.0),
    ("key_spread", 0.0),
    ("patrol_keys", 0.0),
    ("corners", 0.0),
    ("minion_growth", 0.0),
];

/// Version 4 of the game, the designer's revision with the Backrooms, as
/// it played before the difficulty pass (#11667): the values that differ
/// from today's defaults (version 5, Normal). Easy plays it.
pub const V4: &[(&str, f64)] = &[
    ("man_wander", 1.92),
    ("man_investigate", 2.88),
    ("man_search", 2.56),
    ("man_chase", 4.48),
    ("loops", 10.0),
    ("sprint_noise", 8.0),
    ("unlock", 2.0),
    ("unlock_guard", 0.0),
    ("puzzle_key", 0.0),
    ("house_size", 4.0),
    ("key_spread", 0.0),
    ("patrol_keys", 0.0),
    ("corners", 0.0),
    ("minion_growth", 0.0),
];

/// Hard: the values that differ from Normal. For the game's designer.
pub const HARD: &[(&str, f64)] = &[
    ("house_size", 8.0),
    ("man_wander", 2.7),
    ("man_investigate", 4.0),
    ("man_search", 3.6),
    ("man_chase", 6.2),
    ("sight", 26.0),
    ("search_time", 10.0),
    ("sniff_every", 18.0),
    ("patrol_keys", 1.0),
    ("sprint_noise", 18.0),
    ("unlock", 7.0),
    ("unlock_guard", 11.0),
    ("minion_count", 4.0),
    ("minion_growth", 40.0),
    ("minion_speed", 3.8),
    ("extra_closets", 1.0),
];

/// How hard a run is. Normal is the game's defaults (version 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Difficulty {
    /// Version 4: a small house, a slow Ice Cream Man, a quick door. For
    /// younger players.
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    /// Its name in settings and addresses.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.id() == id)
    }

    /// The next one, round and round (the menu's button).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Easy => Self::Normal,
            Self::Normal => Self::Hard,
            Self::Hard => Self::Easy,
        }
    }

    /// The numbers a run at this difficulty plays by.
    #[must_use]
    pub fn tuning(self) -> Tuning {
        let changes = match self {
            Self::Easy => V4,
            Self::Normal => &[],
            Self::Hard => HARD,
        };
        let mut t = Tuning::default();
        for (name, value) in changes {
            t.set(name, *value)
                .expect("the difficulty's values are in bounds");
        }
        t
    }
}

/// One value a game document may change: its section, name, default,
/// bounds, and what it does, in plain words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    pub section: &'static str,
    pub name: &'static str,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    /// A whole number (ammo, hits, closets).
    pub whole: bool,
    pub about: &'static str,
}

macro_rules! tunables {
    ($( $section:literal $field:ident : $ty:ident = $default:expr, $min:expr, $max:expr, $about:literal; )*) => {
        /// Every number a run reads, at run time: the constants above are
        /// its defaults. Game Lab (`docs/game-lab/README.md`) changes
        /// these as data, without building the game again.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Tuning {
            $( pub $field: $ty, )*
        }

        impl Default for Tuning {
            fn default() -> Self {
                Self { $( $field: $default, )* }
            }
        }

        /// The values a document may change, in order.
        pub const FIELDS: &[Field] = &[
            $( Field {
                section: $section,
                name: stringify!($field),
                default: $default as f64,
                min: $min as f64,
                max: $max as f64,
                whole: whole(stringify!($ty)),
                about: $about,
            }, )*
        ];

        impl Tuning {
            /// A value by name.
            #[must_use]
            pub fn get(&self, name: &str) -> Option<f64> {
                match name {
                    $( stringify!($field) => Some(f64::from(self.$field)), )*
                    _ => None,
                }
            }

            /// Sets a value by name, within its bounds.
            ///
            /// # Errors
            /// The name isn't a value, or the value is out of bounds or
            /// not whole where it must be; the reason in plain words.
            pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
                let field = field(name).ok_or_else(|| format!("the game has no setting called {name}"))?;
                if !value.is_finite() || value < field.min || value > field.max {
                    return Err(format!(
                        "{name} must be between {} and {} ({})",
                        trim(field.min),
                        trim(field.max),
                        field.about
                    ));
                }
                if field.whole && value.fract() != 0.0 {
                    return Err(format!("{name} must be a whole number ({})", field.about));
                }
                match name {
                    $( stringify!($field) => self.$field = value as $ty, )*
                    _ => {}
                }
                Ok(())
            }
        }
    };
}

const fn whole(ty: &str) -> bool {
    matches!(ty.as_bytes(), b"u32")
}

fn trim(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A value's description, by name.
#[must_use]
pub fn field(name: &str) -> Option<&'static Field> {
    FIELDS.iter().find(|f| f.name == name)
}

tunables! {
    "player" walk: f32 = WALK, 1.0, 12.0, "the player's walking speed, metres a second";
    "player" sprint: f32 = SPRINT, 1.0, 16.0, "the player's sprinting speed, metres a second";
    "player" jump_speed: f32 = JUMP_SPEED, 2.0, 10.0, "a jump's starting speed";
    "player" gravity: f32 = GRAVITY, 4.0, 40.0, "how fast a jump comes down";
    "player" max_health: f32 = MAX_HEALTH, 10.0, 1000.0, "the player's health";
    "player" regen_after: f32 = REGEN_AFTER, 0.0, 120.0, "seconds without a hit before health comes back";
    "player" regen_rate: f32 = REGEN_RATE, 0.0, 50.0, "health back a second";
    "man" man_wander: f32 = MAN_WANDER, 0.5, 12.0, "his speed wandering the rooms";
    "man" man_investigate: f32 = MAN_INVESTIGATE, 0.5, 12.0, "his speed going to look at a noise";
    "man" man_search: f32 = MAN_SEARCH, 0.5, 12.0, "his speed searching where he last saw you";
    "man" man_chase: f32 = MAN_CHASE, 0.5, 14.0, "his speed chasing you";
    "man" sight: f32 = SIGHT, 2.0, 60.0, "how far he sees, metres";
    "man" sight_angle: f32 = SIGHT_ANGLE, 10.0, 180.0, "his field of view either side of where he faces, degrees";
    "man" notice: f32 = NOTICE, 0.0, 20.0, "how close you can be before he notices you whichever way he faces";
    "man" search_time: f32 = SEARCH_TIME, 0.0, 60.0, "seconds he searches the last place he saw you";
    "man" sniff_every: f32 = SNIFF_EVERY, 3.0, 600.0, "how often a wandering Ice Cream Man sniffs out your room, seconds";
    "man" patrol_keys: u32 = PATROL_KEYS, 0, 3, "once you have this many keys he keeps going back to guard the front door; 0 for never";
    "man" corners: u32 = CORNERS, 0, 1, "chasing, he cuts corners (1) or goes by the middle of every doorway (0)";
    "man" sprinkle_damage: f32 = SPRINKLE_DAMAGE, 0.0, 100.0, "a sprinkle pellet's damage";
    "man" grab_damage: f32 = GRAB_DAMAGE, 0.0, 200.0, "a grab's damage";
    "man" grab_shove: f32 = GRAB_SHOVE, 0.0, 30.0, "how hard a grab shoves you";
    "man" grab_reach: f32 = GRAB_REACH, 0.5, 4.0, "how far he reaches to grab";
    "man" grab_wait: f32 = GRAB_WAIT, 0.2, 20.0, "seconds between grabs";
    "man" gloat: f32 = GLOAT, 0.0, 10.0, "seconds he stands gloating after a grab";
    "man" volley_min: f32 = VOLLEY_MIN, 0.0, 30.0, "the nearest he fires sprinkles from";
    "man" volley_max: f32 = VOLLEY_MAX, 1.0, 40.0, "the farthest he fires sprinkles from";
    "man" volley_spread: f32 = VOLLEY_SPREAD, 0.0, 30.0, "degrees between a volley's pellets";
    "man" pellet_speed: f32 = PELLET_SPEED, 1.0, 40.0, "how fast sprinkles fly";
    "man" volley_wait: f32 = VOLLEY_WAIT, 0.3, 30.0, "seconds between volleys";
    "man" pellet_dodge: f32 = PELLET_DODGE, 0.0, 1.0, "how high a jump clears sprinkles, metres";
    "items" fudge_start: u32 = FUDGE_START, 0, 500, "hot fudge at the start";
    "items" fudge_max: u32 = FUDGE_MAX, 1, 999, "the most hot fudge you can carry";
    "items" fudge_pickup: u32 = FUDGE_PICKUP, 0, 500, "hot fudge in a fridge";
    "items" fudge_rate: f32 = FUDGE_RATE, 0.5, 20.0, "squirts a second";
    "items" fudge_speed: f32 = FUDGE_SPEED, 2.0, 40.0, "how fast fudge flies";
    "items" fudge_range: f32 = FUDGE_RANGE, 1.0, 40.0, "how far fudge flies, metres";
    "items" fudge_slow: f32 = FUDGE_SLOW, 0.0, 20.0, "seconds a fudge hit slows him";
    "items" slowed: f32 = SLOWED, 0.05, 1.0, "his share of his speed while slowed";
    "items" freeze_hits: u32 = FREEZE_HITS, 1, 50, "fudge hits that freeze him";
    "items" fudge_memory: f32 = FUDGE_MEMORY, 0.5, 30.0, "seconds fudge hits keep counting toward a freeze";
    "items" frozen: f32 = FROZEN, 0.0, 60.0, "seconds he stays frozen";
    "items" farts_start: u32 = FARTS_START, 0, 50, "fart clouds at the start";
    "items" farts_max: u32 = FARTS_MAX, 1, 99, "the most fart clouds you can carry";
    "items" fart_range: f32 = FART_RANGE, 1.0, 20.0, "how far the fart launcher throws";
    "items" fart_radius: f32 = FART_RADIUS, 0.5, 15.0, "how big a fart cloud is";
    "items" fart_shove: f32 = FART_SHOVE, 0.0, 40.0, "how hard a fart cloud shoves him";
    "items" confused: f32 = CONFUSED, 0.0, 60.0, "seconds a fart cloud confuses him";
    "items" fart_wait: f32 = FART_WAIT, 0.1, 20.0, "seconds between fart clouds";
    "items" cloud_life: f32 = CLOUD_LIFE, 0.2, 20.0, "seconds a fart cloud hangs about";
    "items" jingle_noise: f32 = JINGLE_NOISE, 0.0, 80.0, "how far the key ring's jingle carries";
    "items" jingle_wait: f32 = JINGLE_WAIT, 0.2, 20.0, "seconds between jingles";
    "noise" sprint_noise: f32 = SPRINT_NOISE, 0.0, 80.0, "how far sprinting is heard";
    "noise" fudge_noise: f32 = FUDGE_NOISE, 0.0, 80.0, "how far a squirt of fudge is heard";
    "noise" fridge_noise: f32 = FRIDGE_NOISE, 0.0, 80.0, "how far opening a fridge is heard";
    "noise" door_noise: f32 = DOOR_NOISE, 0.0, 80.0, "how far rattling the front door is heard";
    "layout" extra_closets: u32 = EXTRA_CLOSETS, 0, MAX_EXTRA_CLOSETS, "hiding spots (closets) added to the house";
    "rules" unlock: f32 = UNLOCK, 0.0, 600.0, "seconds holding E at the front door with every key to unlock it";
    "rules" unlock_guard: f32 = UNLOCK_GUARD, 0.0, 60.0, "the unlock stops and starts over while he is this near, metres; 0 for never";
    "rules" puzzle_key: u32 = PUZZLE_KEY, 0, 1, "one key is locked in the cone cabinet until the sprinkle buttons are pressed in order (1), or all keys lie on the floor (0)";
    "rules" reach: f32 = REACH, 0.5, 4.0, "how close you must be to use a closet, fridge or the front door";
    "rules" key_reach: f32 = KEY_REACH, 0.3, 4.0, "how close you must walk to pick up a key";
    "layout" backrooms: u32 = BACKROOMS, 0, 1, "build the house as the Backrooms (1): wide empty yellow rooms with wall stubs among the Airbnb's own; 0 keeps the Airbnb's rooms";
    "layout" loops: u32 = LOOPS, 0, 24, "extra doorways beyond the ones that join every room; more make the house loop like the Backrooms";
    "layout" house_size: u32 = HOUSE_SIZE, 3, MAX_HOUSE_SIZE, "rooms along each side of the house (4 is 4 by 4, 16 rooms)";
    "layout" key_spread: u32 = KEY_SPREAD, 0, 1, "the keys go to the far ends of the house, far from the bedroom, the front door and each other (1), or into any rooms (0)";
    "weapons" earned_weapons: u32 = EARNED_WEAPONS, 0, 1, "whether the four earned weapons are in the game (1) or not (0): the candy cane sword, the waffle cone spear, the cone pistol and the sugar launcher";
    "weapons" man_health: f32 = MAN_HEALTH, 10.0, 10000.0, "the Ice Cream Man's health against the earned weapons; at none he melts and you win";
    "weapons" sword_damage: f32 = SWORD_DAMAGE, 0.0, 500.0, "a candy cane sword swing's damage";
    "weapons" sword_reach: f32 = SWORD_REACH, 0.5, 6.0, "how far the candy cane sword reaches, metres";
    "weapons" sword_wait: f32 = SWORD_WAIT, 0.1, 10.0, "seconds between candy cane sword swings";
    "weapons" sword_task: u32 = SWORD_TASK, 1, 50, "minions you melt before one drops the candy cane sword";
    "weapons" spear_damage: f32 = SPEAR_DAMAGE, 0.0, 500.0, "a waffle cone spear jab's damage";
    "weapons" spear_reach: f32 = SPEAR_REACH, 1.0, 10.0, "how far the waffle cone spear reaches, metres";
    "weapons" spear_wait: f32 = SPEAR_WAIT, 0.1, 10.0, "seconds between waffle cone spear jabs";
    "weapons" pistol_damage: f32 = PISTOL_DAMAGE, 0.0, 500.0, "a cone from the cone pistol's damage";
    "weapons" pistol_rate: f32 = PISTOL_RATE, 0.2, 20.0, "cones the cone pistol fires a second";
    "weapons" pistol_speed: f32 = PISTOL_SPEED, 2.0, 60.0, "how fast the cone pistol's cones fly";
    "weapons" pistol_range: f32 = PISTOL_RANGE, 1.0, 60.0, "how far the cone pistol's cones fly, metres";
    "weapons" pistol_ammo: u32 = PISTOL_AMMO, 0, 500, "cones the cone pistol comes with";
    "weapons" pistol_max: u32 = PISTOL_MAX, 1, 999, "the most cones you can carry for the cone pistol";
    "weapons" pistol_pickup: u32 = PISTOL_PICKUP, 0, 500, "cones a fridge gives the cone pistol";
    "weapons" pistol_task: u32 = PISTOL_TASK, 1, 50, "times you freeze him with hot fudge to earn the cone pistol";
    "weapons" sugar_damage: f32 = SUGAR_DAMAGE, 0.0, 1000.0, "a sugar burst's damage to everything in it";
    "weapons" sugar_radius: f32 = SUGAR_RADIUS, 0.5, 10.0, "how big a sugar burst is, metres";
    "weapons" sugar_speed: f32 = SUGAR_SPEED, 2.0, 60.0, "how fast the sugar launcher's sugar flies";
    "weapons" sugar_range: f32 = SUGAR_RANGE, 1.0, 60.0, "how far the sugar launcher throws, metres";
    "weapons" sugar_wait: f32 = SUGAR_WAIT, 0.1, 20.0, "seconds between sugar launches";
    "weapons" sugar_ammo: u32 = SUGAR_AMMO, 0, 200, "sugar the sugar launcher comes with";
    "weapons" sugar_max: u32 = SUGAR_MAX, 1, 999, "the most sugar you can carry for the sugar launcher";
    "weapons" sugar_pickup: u32 = SUGAR_PICKUP, 0, 200, "sugar a fridge gives the sugar launcher";
    "weapons" sugar_task: u32 = SUGAR_TASK, 1, 100, "minions you melt to earn the sugar launcher";
    "minions" minion_count: u32 = MINION_COUNT, 0, 12, "how many minions (mini Ice Cream Men) can hunt you at once; 0 for none";
    "minions" minion_first: f32 = MINION_FIRST, 0.0, 600.0, "seconds before the first minion comes";
    "minions" minion_every: f32 = MINION_EVERY, 1.0, 600.0, "seconds between new minions";
    "minions" minion_growth: f32 = MINION_GROWTH, 0.0, 600.0, "one more minion may hunt you every this many seconds of a run, up to 12; 0 for never more";
    "minions" minion_speed: f32 = MINION_SPEED, 0.5, 12.0, "a minion's speed, metres a second";
    "minions" minion_health: f32 = MINION_HEALTH, 1.0, 1000.0, "a minion's health";
    "minions" minion_damage: f32 = MINION_DAMAGE, 0.0, 100.0, "a minion's bite's damage (your own minions bite as hard)";
    "minions" minion_bite_wait: f32 = MINION_BITE_WAIT, 0.1, 20.0, "seconds between a minion's bites";
    "minions" minion_sight: f32 = MINION_SIGHT, 1.0, 80.0, "how near you must be before a minion hunts you, metres";
    "minions" fudge_minion_damage: f32 = FUDGE_MINION_DAMAGE, 0.0, 500.0, "a squirt of hot fudge's damage to a minion";
    "minions" fart_minion_damage: f32 = FART_MINION_DAMAGE, 0.0, 500.0, "a fart cloud's damage to the minions in it";
    "minions" cone_drop: f32 = CONE_DROP, 0.0, 1.0, "the chance a melted minion drops an edible ice cream cone, from 0 to 1";
    "minions" cone_boost: f32 = CONE_BOOST, 0.0, 3.0, "extra health an eaten ice cream cone gives, as a share of full health (0.5 is 50% more)";
    "minions" cone_time: f32 = CONE_TIME, 0.0, 120.0, "seconds an ice cream cone's extra health lasts";
    "minions" staff_length: f32 = STAFF_LENGTH, 1.0, 40.0, "how long the sprinkle staff's one super long sprinkle reaches, metres";
    "minions" staff_damage: f32 = STAFF_DAMAGE, 0.0, 1000.0, "the sprinkle staff's damage to what its sprinkle touches";
    "minions" staff_wait: f32 = STAFF_WAIT, 0.2, 30.0, "seconds between sprinkle staff waves";
    "minions" staff_minions: u32 = STAFF_MINIONS, 1, 12, "your own minions one wave of the sprinkle staff makes";
    "minions" staff_minion_life: f32 = STAFF_MINION_LIFE, 1.0, 600.0, "seconds one of your own minions lasts";
    "rules" free_roam: u32 = FREE_ROAM, 0, 1, "after you win (escape, or melt him), keep playing in free roam with the sprinkle staff (1), or end the run (0)";
    "look" yellow: f32 = YELLOW, 0.0, 1.0, "how yellowed the rooms look, from 0 (pastel ice cream) to 1 (Backrooms: monotone yellow wallpaper, damp carpet, fluorescent panels)";
    "look" fog_near: f32 = FOG_NEAR, 0.0, 100.0, "where the fog starts, metres from the camera";
    "look" fog_far: f32 = FOG_FAR, 1.0, 120.0, "where the fog is thickest, metres from the camera";
    "look" flicker: f32 = FLICKER, 0.0, 1.0, "how often the lights flicker, from 0 (never) to 1 (often): above 0.05 they dip for half a second at a time, two a second at most, so they never strobe";
    "look" buzz: f32 = BUZZ, 0.0, 1.0, "how loud the fluorescent lights buzz, from 0 to 1";
    "look" first_person: u32 = FIRST_PERSON, 0, 1, "the camera a run starts with: 1 first person, 0 third person (V switches)";
}

impl Tuning {
    /// The game as it played before the designer's revision ([`ORIGINAL`]).
    #[must_use]
    pub fn original() -> Self {
        let mut t = Self::default();
        for (name, value) in ORIGINAL {
            t.set(name, *value)
                .expect("the original values are in bounds");
        }
        t
    }

    /// The cosine of [`Tuning::sight_angle`].
    #[must_use]
    pub fn sight_cos(&self) -> f32 {
        self.sight_angle.to_radians().cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constants() {
        let t = Tuning::default();
        assert_eq!(t.man_chase, MAN_CHASE);
        assert_eq!(t.fudge_start, FUDGE_START);
        assert!((t.sight_cos() - SIGHT_COS).abs() < 0.001);
        // Version 5 of the-ice-cream-man (#11667), on top of the
        // designer's revision Game Lab measured as version 4.
        assert_eq!((t.earned_weapons, t.minion_count, t.free_roam), (1, 3, 1));
        assert_eq!((t.backrooms, t.loops, t.extra_closets), (1, 7, 2));
        assert_eq!((t.house_size, t.key_spread, t.puzzle_key), (7, 1, 1));
        assert_eq!((t.man_chase, t.unlock, t.unlock_guard), (5.6, 6.0, 8.0));
        assert_eq!((t.patrol_keys, t.corners, t.minion_growth), (2, 1, 60.0));
        assert_eq!((t.yellow, t.fog_near, t.fog_far), (1.0, 3.0, 18.0));
        assert_eq!((t.flicker, t.buzz, t.first_person), (0.35, 0.8, 0));
        let original = Tuning::original();
        assert_eq!((original.man_chase, original.minion_count), (5.6, 0));
        assert_eq!((original.backrooms, original.yellow), (0, 0.0));
        for f in FIELDS {
            assert_eq!(t.get(f.name), Some(f.default), "{}", f.name);
            assert!(f.min <= f.default && f.default <= f.max, "{}", f.name);
        }
    }

    #[test]
    fn easy_is_version_4_and_hard_is_harder() {
        let easy = Difficulty::Easy.tuning();
        assert_eq!((easy.man_chase, easy.man_wander), (4.48, 1.92));
        assert_eq!((easy.house_size, easy.loops, easy.unlock), (4, 10, 2.0));
        assert_eq!(
            (easy.puzzle_key, easy.key_spread, easy.unlock_guard),
            (0, 0, 0.0)
        );
        assert_eq!(
            (easy.patrol_keys, easy.corners, easy.minion_growth),
            (0, 0, 0.0)
        );
        assert_eq!(easy.sprint_noise, 8.0);
        assert_eq!(Difficulty::default(), Difficulty::Normal);
        assert_eq!(Difficulty::Normal.tuning(), Tuning::default());
        let (normal, hard) = (Tuning::default(), Difficulty::Hard.tuning());
        assert!(hard.house_size > normal.house_size && normal.house_size > easy.house_size);
        assert!(hard.man_chase > normal.man_chase && normal.man_chase > easy.man_chase);
        assert!(hard.unlock > normal.unlock && normal.unlock > easy.unlock);
        // A sprint still gets you clear of him.
        assert!(hard.man_chase < hard.sprint);
        for d in Difficulty::ALL {
            assert_eq!(Difficulty::from_id(d.id()), Some(d));
            assert_ne!(d.next(), d);
        }
        assert_eq!(Difficulty::from_id("impossible"), None);
        // The original game knows nothing of the difficulty pass.
        let original = Tuning::original();
        assert_eq!(
            (original.house_size, original.unlock, original.puzzle_key),
            (4, 2.0, 0)
        );
    }

    #[test]
    fn values_are_set_by_name_within_bounds() {
        let mut t = Tuning::default();
        t.set("man_chase", 4.48).unwrap();
        assert!((t.man_chase - 4.48).abs() < 1e-6);
        assert!(
            t.set("house_size", 11.0)
                .unwrap_err()
                .contains("between 3 and 10")
        );
        t.set("extra_closets", 2.0).unwrap();
        assert_eq!(t.extra_closets, 2);
        assert!(t.set("extra_closets", 1.5).unwrap_err().contains("whole"));
        assert!(
            t.set("unlock", 1e9)
                .unwrap_err()
                .contains("between 0 and 600")
        );
        assert!(
            t.set("door_locked", 1.0)
                .unwrap_err()
                .contains("no setting")
        );
        assert!(t.set("walk", f64::NAN).is_err());
    }
}
