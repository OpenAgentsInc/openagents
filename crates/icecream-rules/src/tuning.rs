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
