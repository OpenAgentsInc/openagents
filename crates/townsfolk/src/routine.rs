//! Where a villager is at a town time: a pure function of its definition,
//! the world tree, the town's seed, and the [`TownTime`], so every device
//! derives the same town with no network message.
//!
//! Each row's departure is its time plus a seeded delay of up to
//! [`JITTER_SECONDS`], different each day, so villagers don't move in
//! lockstep. A villager walks from the previous row's node to the row's
//! node in [`walk_seconds`] of town time, from the straight distance
//! between the two standing points times [`DETOUR`]; the zone draws the
//! walk along its real route at that progress. At a node it stands at the
//! node's standing point plus a seeded offset ([`SCATTER`]), except at an
//! exclusive object, where it stands on the point.

use std::collections::BTreeMap;

use town_clock::{DAY_REAL_SECONDS, TOWN_DAY_SECONDS, TownTime};
use world_tree::Tree;

use crate::validate::{self, Checks, NoScreen};
use crate::{Activity, Npc, Problem, Town};

/// The walking speed a walk's duration assumes, m per real second.
pub const WALK_SPEED: f32 = 1.4;
/// How much longer than the straight line a walk is taken to be.
pub const DETOUR: f32 = 1.4;
/// Town seconds per real second.
pub const TOWN_PER_REAL: f64 = TOWN_DAY_SECONDS as f64 / DAY_REAL_SECONDS as f64;
/// The longest seeded delay of a departure, town seconds.
pub const JITTER_SECONDS: u32 = 600;
/// How far from a node's standing point a villager stands there, m:
/// at least the first and at most the second.
pub const SCATTER: [f32; 2] = [1.2, 2.6];

/// Town seconds a walk of `meters` takes.
#[must_use]
pub fn walk_seconds(meters: f32) -> f64 {
    f64::from(meters) / f64::from(WALK_SPEED) * TOWN_PER_REAL
}

/// The straight distance between two nodes' standing points times
/// [`DETOUR`], m: the length a walk's duration assumes.
#[must_use]
pub fn leg_meters(tree: &Tree, from: &str, to: &str) -> Option<f32> {
    let a = tree.node(from)?.stand;
    let b = tree.node(to)?.stand;
    Some((a[0] - b[0]).hypot(a[1] - b[1]) * DETOUR)
}

/// Where a villager is.
#[derive(Clone, Debug, PartialEq)]
pub enum Placement<'a> {
    /// Standing at `node` for row `row`, `offset` m from its standing
    /// point.
    At {
        row: usize,
        node: &'a str,
        activity: Activity,
        offset: [f32; 2],
    },
    /// Walking from `from` to row `row`'s node, `progress` of the way
    /// (0 to 1), to do `activity` there.
    Walking {
        row: usize,
        from: &'a str,
        to: &'a str,
        activity: Activity,
        progress: f32,
    },
}

impl Placement<'_> {
    /// The node it stands at, when it isn't walking.
    #[must_use]
    pub fn node(&self) -> Option<&str> {
        match self {
            Self::At { node, .. } => Some(node),
            Self::Walking { .. } => None,
        }
    }

    #[must_use]
    pub fn activity(&self) -> Activity {
        match self {
            Self::At { activity, .. } | Self::Walking { activity, .. } => *activity,
        }
    }

    #[must_use]
    pub fn row(&self) -> usize {
        match self {
            Self::At { row, .. } | Self::Walking { row, .. } => *row,
        }
    }
}

/// A definition compiled against a tree: its rows' times and walks.
#[derive(Clone, Debug, PartialEq)]
pub struct Villager {
    pub npc: Npc,
    pub digest: String,
    /// Each row's time, seconds into the day.
    starts: Vec<u32>,
    /// The walk into each row from the row before it (the last row's, for
    /// the first), town seconds.
    walks: Vec<f64>,
    /// Whether each row's node is an exclusive object.
    exclusive: Vec<bool>,
}

impl Villager {
    /// Compiles `npc` against `tree` after checking it with
    /// [`validate::npc`] without routes or a screen.
    ///
    /// # Errors
    ///
    /// The problems the check found.
    pub fn compile(npc: Npc, tree: &Tree) -> Result<Self, Vec<Problem>> {
        let checks = Checks::new(tree, &NoScreen);
        let problems = validate::npc(&npc, &checks);
        if !problems.is_empty() {
            return Err(problems);
        }
        Ok(Self::compiled(npc, tree))
    }

    /// Compiles a definition already checked.
    pub(crate) fn compiled(npc: Npc, tree: &Tree) -> Self {
        let n = npc.routine.len();
        let starts = npc
            .routine
            .iter()
            .map(|r| r.second().unwrap_or(0))
            .collect();
        let walks = (0..n)
            .map(|i| {
                let from = &npc.routine[(i + n - 1) % n].node;
                let to = &npc.routine[i].node;
                if from == to {
                    0.0
                } else {
                    walk_seconds(leg_meters(tree, from, to).unwrap_or(0.0))
                }
            })
            .collect();
        let exclusive = npc
            .routine
            .iter()
            .map(|r| tree.node(&r.node).is_some_and(|n| n.exclusive))
            .collect();
        Self {
            digest: npc.digest(),
            npc,
            starts,
            walks,
            exclusive,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.npc.id
    }

    /// Row `row`'s time, seconds into the day.
    #[must_use]
    pub fn start(&self, row: usize) -> u32 {
        self.starts[row]
    }

    /// Whether row `row`'s node is an exclusive object.
    #[must_use]
    pub fn exclusive(&self, row: usize) -> bool {
        self.exclusive[row]
    }

    /// The walk into row `row`, town seconds.
    #[must_use]
    pub fn walk(&self, row: usize) -> f64 {
        self.walks[row]
    }

    /// The town's seed with this villager's own mixed in.
    fn seed(&self, town: u64) -> u64 {
        town ^ self.npc.seed.rotate_left(17)
    }

    /// When the villager leaves for row `row` on `day`: the row's time
    /// plus its seeded delay, seconds into the day.
    #[must_use]
    pub fn departure(&self, seed: u64, day: i64, row: usize) -> f64 {
        let delay = mix(self.seed(seed), &self.npc.id, day, row, 0) % u64::from(JITTER_SECONDS + 1);
        f64::from(self.starts[row]) + delay as f64
    }

    /// Where it stands off row `row`'s node's standing point on `day`, m.
    #[must_use]
    pub fn offset(&self, seed: u64, day: i64, row: usize) -> [f32; 2] {
        if self.exclusive[row] {
            return [0.0, 0.0];
        }
        let h = mix(self.seed(seed), &self.npc.id, day, row, 1);
        let angle = (h & 0xFFFF) as f32 / 65_536.0 * std::f32::consts::TAU;
        let along = ((h >> 16) & 0xFFFF) as f32 / 65_536.0;
        let r = SCATTER[0] + (SCATTER[1] - SCATTER[0]) * along;
        let (sin, cos) = angle.sin_cos();
        [sin * r, cos * r]
    }

    /// Where it is at `time`.
    #[must_use]
    pub fn at(&self, seed: u64, time: TownTime) -> Placement<'_> {
        let n = self.npc.routine.len();
        let t = time.second;
        // The row it last left for today, or yesterday's last row before
        // today's first departure.
        let (row, day) = match (0..n)
            .rev()
            .find(|&i| self.departure(seed, time.day, i) <= t)
        {
            Some(i) => (i, time.day),
            None => (n - 1, time.day - 1),
        };
        let row_def = &self.npc.routine[row];
        let walk = self.walks[row];
        let since = if day == time.day {
            t - self.departure(seed, day, row)
        } else {
            t + f64::from(TOWN_DAY_SECONDS) - self.departure(seed, day, row)
        };
        if walk > 0.0 && since < walk {
            return Placement::Walking {
                row,
                from: &self.npc.routine[(row + n - 1) % n].node,
                to: &row_def.node,
                activity: row_def.activity,
                progress: (since / walk).clamp(0.0, 1.0) as f32,
            };
        }
        Placement::At {
            row,
            node: &row_def.node,
            activity: row_def.activity,
            offset: self.offset(seed, day, row),
        }
    }
}

/// A 64-bit mix of the seed, the villager, the day, the row, and a salt:
/// integer only, so every platform agrees.
pub(crate) fn mix(seed: u64, id: &str, day: i64, row: usize, salt: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    let mut x = seed
        ^ h
        ^ (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (row as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    // splitmix64's finalizer.
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The villagers standing at each node at `time`, by node ID, each list in
/// roster order: who is together, which rumors (phase E2) pass between.
#[must_use]
pub fn gatherings<'a>(
    villagers: &'a [Villager],
    seed: u64,
    time: TownTime,
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for v in villagers {
        if let Placement::At { node, .. } = v.at(seed, time) {
            out.entry(node).or_default().push(v.id());
        }
    }
    out
}

/// The admitted town as a client loads it.
#[derive(Clone, Debug)]
pub struct Roster {
    pub town: Town,
    pub villagers: Vec<Villager>,
}

impl Roster {
    /// Loads the roster `town_json` and the definition files `files`
    /// against `tree`. A definition loads only when the roster admits its
    /// ID with its exact digest and it passes [`validate::npc`] (without
    /// routes, which the zone's tests check). Every one that doesn't is
    /// left out and reported.
    ///
    /// # Errors
    ///
    /// When the roster doesn't parse, names another zone, or is over a
    /// budget or a ceiling.
    pub fn load(
        town_json: &str,
        files: &[&str],
        tree: &Tree,
    ) -> Result<(Self, Vec<Problem>), String> {
        let town = Town::parse(town_json)?;
        let problems = town.problems();
        if let Some(p) = problems.first() {
            return Err(format!("the town roster: {p}"));
        }
        if town.zone != tree.zone() {
            return Err(format!(
                "the town roster is for {}, not {}",
                town.zone,
                tree.zone()
            ));
        }
        let mut left_out = Vec::new();
        let mut loaded: Vec<Villager> = Vec::new();
        for (i, json) in files.iter().enumerate() {
            let npc = match Npc::parse(json) {
                Ok(npc) => npc,
                Err(e) => {
                    left_out.push(Problem::new(format!("file {i}"), crate::Code::Schema, e));
                    continue;
                }
            };
            let digest = npc.digest();
            match town.entry(&npc.id) {
                None => {
                    left_out.push(
                        Problem::new(
                            "id",
                            crate::Code::NotAdmitted,
                            "the roster doesn't admit it",
                        )
                        .of(&npc.id),
                    );
                    continue;
                }
                Some(a) if a.digest != digest => {
                    left_out.push(
                        Problem::new(
                            "digest",
                            crate::Code::Digest,
                            format!("is {digest}, but the roster admits {}", a.digest),
                        )
                        .of(&npc.id),
                    );
                    continue;
                }
                Some(_) => {}
            }
            if loaded.iter().any(|v| v.id() == npc.id) {
                left_out.push(
                    Problem::new("id", crate::Code::Duplicate, "two files define it").of(&npc.id),
                );
                continue;
            }
            let checks = Checks::new(tree, &NoScreen).with_budgets(town.budgets);
            let found = validate::npc(&npc, &checks);
            if found.is_empty() {
                loaded.push(Villager::compiled(npc, tree));
            } else {
                let id = npc.id.clone();
                left_out.extend(found.into_iter().map(|p| p.of(&id)));
            }
        }
        for a in &town.admitted {
            if !loaded.iter().any(|v| v.id() == a.id)
                && !left_out
                    .iter()
                    .any(|p| p.field.starts_with(&format!("{}:", a.id)))
            {
                left_out.push(
                    Problem::new("file", crate::Code::Missing, "no definition file holds it")
                        .of(&a.id),
                );
            }
        }
        // Town-wide checks leave out the later of two clashing villagers.
        let clashes = validate::exclusive(&loaded);
        if !clashes.is_empty() {
            let drop: Vec<String> = clashes
                .iter()
                .filter_map(|p| p.field.split_once(':').map(|(id, _)| id.to_owned()))
                .collect();
            loaded.retain(|v| !drop.iter().any(|d| d == v.id()));
            left_out.extend(clashes);
        }
        // Keep the roster's order.
        loaded.sort_by_key(|v| town.admitted.iter().position(|a| a.id == v.id()));
        Ok((
            Self {
                town,
                villagers: loaded,
            },
            left_out,
        ))
    }

    /// Every villager and where it is at `time`.
    #[must_use]
    pub fn at(&self, time: TownTime) -> Vec<(&Villager, Placement<'_>)> {
        self.villagers
            .iter()
            .map(|v| (v, v.at(self.town.seed, time)))
            .collect()
    }

    /// [`gatherings`] for this town.
    #[must_use]
    pub fn gatherings(&self, time: TownTime) -> BTreeMap<&str, Vec<&str>> {
        gatherings(&self.villagers, self.town.seed, time)
    }

    #[must_use]
    pub fn villager(&self, id: &str) -> Option<&Villager> {
        self.villagers.iter().find(|v| v.id() == id)
    }
}
