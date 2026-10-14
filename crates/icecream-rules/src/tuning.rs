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
/// Between a walk and a sprint, so sprinting matters.
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
pub const SPRINT_NOISE: f32 = 8.0;
pub const FUDGE_NOISE: f32 = 12.0;
pub const FRIDGE_NOISE: f32 = 8.0;
pub const DOOR_NOISE: f32 = 25.0;

/// Reach for closets, fridges, the front door and keys.
pub const REACH: f32 = 1.4;
pub const KEY_REACH: f32 = 0.9;
/// Holding E at the front door this long unlocks it.
pub const UNLOCK: f32 = 2.0;

/// A shove fades this much each second.
pub const SHOVE_FADE: f32 = 6.0;

/// Hiding spots added to the house beyond its rooms' own closets
/// (`House::tuned`).
pub const EXTRA_CLOSETS: u32 = 0;
/// The most spare closets a house can take: one in each bedroom, kitchen
/// and bathroom, and every house has at least four of those.
pub const MAX_EXTRA_CLOSETS: u32 = 4;
/// His field of view while not chasing, in degrees either side of where
/// he faces ([`SIGHT_COS`] is its cosine).
pub const SIGHT_ANGLE: f32 = 70.0;

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
    "rules" reach: f32 = REACH, 0.5, 4.0, "how close you must be to use a closet, fridge or the front door";
    "rules" key_reach: f32 = KEY_REACH, 0.3, 4.0, "how close you must walk to pick up a key";
}

impl Tuning {
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
        for f in FIELDS {
            assert_eq!(t.get(f.name), Some(f.default), "{}", f.name);
            assert!(f.min <= f.default && f.default <= f.max, "{}", f.name);
        }
    }

    #[test]
    fn values_are_set_by_name_within_bounds() {
        let mut t = Tuning::default();
        t.set("man_chase", 4.48).unwrap();
        assert!((t.man_chase - 4.48).abs() < 1e-6);
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
