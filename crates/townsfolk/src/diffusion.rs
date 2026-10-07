//! How a rumor spreads: a pure function of the villagers, the town's seed,
//! and the rumor, so every device agrees who knows it, and since when,
//! with no network message.
//!
//! The source knows the rumor from its start. Each town day it travels,
//! [`crate::sim::meetings`] at a [`STEP`] says who stands together where;
//! at each meeting, each villager who doesn't know it yet hears it from
//! each one there who does with the rumor's repeat probability, by a
//! seeded roll for that rumor, meeting, teller, and listener. A villager
//! who hears it at a meeting can pass it on at the same meeting. The
//! share who know it after a simulated day is the measured number the
//! paper reported.

use serde::Serialize;
use town_clock::{TOWN_DAY_SECONDS, TownTime};
use world_tree::Tree;

use crate::routine::{Roster, Villager, mix};
use crate::rumor::Rumor;
use crate::{clock_text, parse_time, sim};

/// The sampling step of the meetings a rumor travels by, town seconds.
pub const STEP: u32 = 300;
/// The salt of a pass roll.
const SALT: u64 = 0x52_55_4d_4f_52;

/// When one villager learned a rumor, where, and from whom.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Learned {
    pub id: String,
    pub day: i64,
    /// Seconds into that day.
    pub second: u32,
    /// `HH:MM`.
    pub at: String,
    /// Who told it; `None` for the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Where.
    pub node: String,
}

impl Learned {
    fn total(&self) -> i64 {
        self.day * i64::from(TOWN_DAY_SECONDS) + i64::from(self.second)
    }
}

/// Who knows a rumor, and since when, over the days simulated.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diffusion {
    pub rumor: String,
    pub probability: f64,
    /// The villagers in the town.
    pub villagers: usize,
    /// The first day simulated, and the day after the last.
    pub from_day: i64,
    pub to_day: i64,
    /// Each villager who learned it, in the order they did.
    pub learned: Vec<Learned>,
}

impl Diffusion {
    /// Whether `id` knows it at `time`.
    #[must_use]
    pub fn knows(&self, id: &str, time: TownTime) -> bool {
        let now = time.day * i64::from(TOWN_DAY_SECONDS) + time.second as i64;
        self.learned.iter().any(|l| l.id == id && l.total() <= now)
    }

    /// Who knows it at `time`, in the order they learned it.
    #[must_use]
    pub fn known_by(&self, time: TownTime) -> Vec<&str> {
        let now = time.day * i64::from(TOWN_DAY_SECONDS) + time.second as i64;
        self.learned
            .iter()
            .filter(|l| l.total() <= now)
            .map(|l| l.id.as_str())
            .collect()
    }

    /// The share of the town that knows it by the end of the simulation.
    #[must_use]
    pub fn share(&self) -> f64 {
        if self.villagers == 0 {
            0.0
        } else {
            self.learned.len() as f64 / self.villagers as f64
        }
    }

    /// The trace as text: each villager, when, where, and from whom, then
    /// the count at the end of each day.
    #[must_use]
    pub fn render(&self, tree: &Tree, name: impl Fn(&str) -> String) -> Vec<String> {
        let place = |id: &str| tree.node(id).map_or(id.to_owned(), |n| n.name.clone());
        let mut out = vec![format!(
            "rumor {} (pass probability {:.2}), days {} to {}:",
            self.rumor,
            self.probability,
            self.from_day,
            self.to_day - 1
        )];
        for l in &self.learned {
            out.push(match &l.from {
                None => format!(
                    "  day {} {} {} starts it at {}",
                    l.day,
                    l.at,
                    name(&l.id),
                    place(&l.node)
                ),
                Some(from) => format!(
                    "  day {} {} {} hears it from {} at {}",
                    l.day,
                    l.at,
                    name(&l.id),
                    name(from),
                    place(&l.node)
                ),
            });
        }
        for day in self.from_day..self.to_day {
            let end = TownTime {
                day,
                second: f64::from(TOWN_DAY_SECONDS) - 1.0,
            };
            out.push(format!(
                "  end of day {day}: {} of {} villagers know it",
                self.known_by(end).len(),
                self.villagers
            ));
        }
        out
    }
}

/// Whether `teller` passes `rumor` to `listener` at the meeting at `node`
/// that starts at `second` on `day`.
fn passes(seed: u64, rumor: &Rumor, node: &str, day: i64, second: u32, pair: (&str, &str)) -> bool {
    let key = format!("{}|{}|{}|{node}", rumor.id, pair.0, pair.1);
    let roll = mix(seed, &key, day, second as usize, SALT);
    ((roll >> 11) as f64 / (1u64 << 53) as f64) < rumor.probability()
}

/// Who learns `rumor` among `villagers` under the town's `seed`, over
/// `days` town days from its start. An unparsable start, or a source not
/// in `villagers`, spreads to no one.
#[must_use]
pub fn diffusion(villagers: &[Villager], seed: u64, rumor: &Rumor, days: u32) -> Diffusion {
    let mut out = Diffusion {
        rumor: rumor.id.clone(),
        probability: rumor.probability(),
        villagers: villagers.len(),
        from_day: rumor.day,
        to_day: rumor.day + i64::from(days),
        learned: Vec::new(),
    };
    let (Some(start), Some(start_second)) = (rumor.start_seconds(), parse_time(&rumor.at)) else {
        return out;
    };
    if !villagers.iter().any(|v| v.id() == rumor.source) {
        return out;
    }
    out.learned.push(Learned {
        id: rumor.source.clone(),
        day: rumor.day,
        second: start_second,
        at: rumor.at.clone(),
        from: None,
        node: rumor.node.clone(),
    });
    let day_seconds = i64::from(TOWN_DAY_SECONDS);
    for day in out.from_day..out.to_day {
        for m in sim::meetings(villagers, seed, day, STEP) {
            let Some(from) = parse_time(&m.from) else {
                continue;
            };
            let to = if m.to == "24:00" {
                TOWN_DAY_SECONDS
            } else {
                parse_time(&m.to).unwrap_or(TOWN_DAY_SECONDS)
            };
            let (from_total, to_total) = (
                day * day_seconds + i64::from(from),
                day * day_seconds + i64::from(to),
            );
            if to_total <= start {
                continue;
            }
            // Until no one new hears it here: a listener may tell the next.
            loop {
                let mut heard = None;
                'listeners: for listener in &m.ids {
                    if out.learned.iter().any(|l| &l.id == listener) {
                        continue;
                    }
                    for teller in &m.ids {
                        let Some(known) = out
                            .learned
                            .iter()
                            .find(|l| &l.id == teller)
                            .map(Learned::total)
                        else {
                            continue;
                        };
                        if known >= to_total {
                            continue;
                        }
                        if passes(seed, rumor, &m.node, day, from, (teller, listener)) {
                            let when = known.max(from_total).max(start);
                            heard = Some((listener.clone(), teller.clone(), when));
                            break 'listeners;
                        }
                    }
                }
                let Some((listener, teller, when)) = heard else {
                    break;
                };
                let second = (when - day * day_seconds) as u32;
                out.learned.push(Learned {
                    id: listener,
                    day,
                    second,
                    at: clock_text(f64::from(second)),
                    from: Some(teller),
                    node: m.node.clone(),
                });
            }
        }
    }
    out
}

/// [`diffusion`] over the loaded town for the rumor's own days.
#[must_use]
pub fn spread(roster: &Roster, rumor: &Rumor) -> Diffusion {
    diffusion(&roster.villagers, roster.town.seed, rumor, rumor.days)
}
