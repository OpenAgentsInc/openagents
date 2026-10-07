//! A town day as text: where each villager is at given hours, who stands
//! together, and the meetings over a whole day. `openagents verse town
//! preview` prints it, and a proposal records it.

use serde::Serialize;
use town_clock::TownTime;
use world_tree::Tree;

use crate::routine::{Placement, Villager, gatherings};
use crate::{Activity, clock_text};

/// One villager at one moment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Where {
    pub id: String,
    pub name: String,
    pub activity: Activity,
    /// The node it stands at, or walks to.
    pub node: String,
    /// The node it walks from, while walking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// How far along the walk, 0 to 1, while walking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
    /// Where it stands, x and z, m: the standing point and its offset, or
    /// the straight line's point at its progress (the zone's route bends).
    pub at: [f32; 2],
}

/// Every villager at one moment, and who stands together.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Moment {
    /// `HH:MM`.
    pub time: String,
    pub day: i64,
    pub villagers: Vec<Where>,
    /// Each node where two or more villagers stand, and who.
    pub together: Vec<(String, Vec<String>)>,
}

/// The villagers at `time`.
#[must_use]
pub fn moment(villagers: &[Villager], tree: &Tree, seed: u64, time: TownTime) -> Moment {
    let stand = |id: &str| tree.node(id).map_or([0.0, 0.0], |n| n.stand);
    let out = villagers
        .iter()
        .map(|v| {
            let placement = v.at(seed, time);
            let (node, from, progress, at) = match &placement {
                Placement::At { node, offset, .. } => {
                    let s = stand(node);
                    (*node, None, None, [s[0] + offset[0], s[1] + offset[1]])
                }
                Placement::Walking {
                    from, to, progress, ..
                } => {
                    let (a, b) = (stand(from), stand(to));
                    let p = *progress;
                    (
                        *to,
                        Some((*from).to_owned()),
                        Some(p),
                        [a[0] + (b[0] - a[0]) * p, a[1] + (b[1] - a[1]) * p],
                    )
                }
            };
            Where {
                id: v.id().to_owned(),
                name: v.npc.name.clone(),
                activity: placement.activity(),
                node: node.to_owned(),
                from,
                progress,
                at,
            }
        })
        .collect();
    let together = gatherings(villagers, seed, time)
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(node, ids)| {
            (
                node.to_owned(),
                ids.into_iter().map(str::to_owned).collect(),
            )
        })
        .collect();
    Moment {
        time: clock_text(time.second),
        day: time.day,
        villagers: out,
        together,
    }
}

fn place_name<'t>(tree: &'t Tree, id: &'t str) -> &'t str {
    tree.node(id).map_or(id, |n| n.name.as_str())
}

/// `moment` as lines of text.
#[must_use]
pub fn render(moment: &Moment, tree: &Tree) -> Vec<String> {
    let mut out = vec![format!("{} (day {})", moment.time, moment.day)];
    for w in &moment.villagers {
        let line = match (&w.from, w.progress) {
            (Some(from), Some(p)) => format!(
                "  {:<12} walking {} -> {}, {:.0}%, then {} ({:.1}, {:.1})",
                w.name,
                place_name(tree, from),
                place_name(tree, &w.node),
                p * 100.0,
                w.activity.doing(),
                w.at[0],
                w.at[1]
            ),
            _ => format!(
                "  {:<12} {} at {} ({:.1}, {:.1})",
                w.name,
                w.activity.doing(),
                place_name(tree, &w.node),
                w.at[0],
                w.at[1]
            ),
        };
        out.push(line);
    }
    for (node, ids) in &moment.together {
        out.push(format!(
            "  together at {}: {}",
            place_name(tree, node),
            ids.join(", ")
        ));
    }
    out
}

/// A villager's routine as text, one row a line.
#[must_use]
pub fn schedule(v: &Villager, tree: &Tree) -> Vec<String> {
    let mut out = vec![format!(
        "{} ({}, a character): {}",
        v.npc.name,
        v.npc.card.role,
        v.id()
    )];
    for (i, row) in v.npc.routine.iter().enumerate() {
        let walk = v.walk(i);
        let walk = if walk > 0.0 {
            format!(", a {:.0} min walk", (walk / 60.0).ceil())
        } else {
            String::new()
        };
        out.push(format!(
            "  {} {:<16} {}{walk}",
            row.at,
            row.activity.doing(),
            place_name(tree, &row.node)
        ));
    }
    out
}

/// A span of a day when two or more villagers stand at one node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Meeting {
    pub node: String,
    pub ids: Vec<String>,
    /// `HH:MM`, sampled every step.
    pub from: String,
    pub to: String,
}

/// The meetings on `day`, sampled every `step` town seconds: who stands
/// together where and when. Rumors (phase E2) pass at a meeting.
#[must_use]
pub fn meetings(villagers: &[Villager], seed: u64, day: i64, step: u32) -> Vec<Meeting> {
    let step = step.max(60);
    let mut open: Vec<(String, Vec<String>, u32, u32)> = Vec::new();
    let mut done = Vec::new();
    let mut second = 0;
    while second < 86_400 {
        let time = TownTime {
            day,
            second: f64::from(second),
        };
        let now: Vec<(String, Vec<String>)> = gatherings(villagers, seed, time)
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .map(|(n, ids)| (n.to_owned(), ids.into_iter().map(str::to_owned).collect()))
            .collect();
        let mut still = Vec::new();
        for (node, ids, start, _) in open.drain(..) {
            if now.iter().any(|(n, i)| *n == node && *i == ids) {
                still.push((node, ids, start, second));
            } else {
                done.push((node, ids, start, second));
            }
        }
        for (node, ids) in now {
            if !still.iter().any(|(n, i, _, _)| *n == node && *i == ids) {
                still.push((node, ids, second, second));
            }
        }
        open = still;
        second += step;
    }
    done.extend(open.into_iter().map(|(n, i, s, _)| (n, i, s, 86_400)));
    done.sort_by_key(|(_, _, s, _)| *s);
    done.into_iter()
        .map(|(node, ids, s, e)| Meeting {
            node,
            ids,
            from: clock_text(f64::from(s)),
            to: if e >= 86_400 {
                "24:00".to_owned()
            } else {
                clock_text(f64::from(e))
            },
        })
        .collect()
}
