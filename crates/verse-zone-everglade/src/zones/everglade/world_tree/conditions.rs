//! Object states from what the zone knows now (the specification's state
//! table): lamps from the town clock's light ([`Light::lamps_lit`]), doors
//! from the layout, studio desks and stations busy from the studio
//! snapshot's seats, and the Task Wall's columns from its tasks. Callers
//! add occupants the snapshot doesn't carry, such as Alice at her
//! workstation.

use coder_access::studio::{Activity, Station, View};
use town_clock::TownTime;
use world_tree::{Column, Conditions, State, States, Tree};

use super::super::boards;
use super::super::layout::TASK_COLUMNS;
use super::super::time_of_day::Light;

/// The node a studio seat occupies: its own desk at the desks station, or
/// the station it is drawn at.
#[must_use]
pub fn seat_node<'t>(tree: &'t Tree, station: Station, desk: u32) -> Option<&'t str> {
    let source = if station == Station::Desk {
        format!("desk:{desk}")
    } else {
        format!("station:{}", verse_world::social::studio::place_id(station))
    };
    tree.by_source(&source).map(|n| n.id.as_str())
}

/// The conditions at `time` with the studio `view`, when there is one.
/// A seat that is idle, paused, or finished occupies nothing; one at a
/// shared station leaves it free for others.
#[must_use]
pub fn at(tree: &Tree, time: TownTime, view: Option<&View>) -> Conditions {
    let mut now = Conditions {
        lamps_lit: Light::at(time).lamps_lit(),
        ..Conditions::default()
    };
    let Some(view) = view else {
        return now;
    };
    for seat in &view.seats {
        let working = !matches!(
            seat.activity,
            Activity::Idle | Activity::Paused | Activity::Done | Activity::Failed
        );
        if !working {
            continue;
        }
        if let Some(id) = seat_node(tree, seat.station, seat.desk)
            && tree.node(id).is_some_and(|n| n.exclusive)
        {
            now.occupants.insert(id.to_owned(), seat.seat.clone());
        }
    }
    let mut counts = [0_u32; TASK_COLUMNS.len()];
    for task in &view.tasks {
        counts[boards::column(task.status)] += 1;
    }
    now.task_columns = TASK_COLUMNS
        .iter()
        .zip(counts)
        .map(|(name, count)| Column {
            name: (*name).to_owned(),
            count,
        })
        .collect();
    now
}

/// `now` with the Pylon Field's pylon and Wellspring states from
/// `compute`'s newest sample.
#[must_use]
pub fn with_compute(
    mut now: Conditions,
    tree: &Tree,
    compute: &super::super::compute::Compute,
) -> Conditions {
    now.compute = compute.conditions(tree);
    now
}

/// Every stateful object's state at `time` with the studio `view`.
#[must_use]
pub fn states(tree: &Tree, time: TownTime, view: Option<&View>) -> States {
    world_tree::state::derive(tree, &at(tree, time, view))
}

/// Publishing dynamic object state: a hosted world authority sends only
/// what changed since `before`, each as one NIP-MV `33301` object entity
/// (`verse_net::mv::object`).
#[must_use]
pub fn changed<'a>(before: &States, now: &'a States) -> Vec<(&'a str, &'a State)> {
    now.iter()
        .filter(|(id, state)| before.get(*id) != Some(state))
        .map(|(id, state)| (id.as_str(), state))
        .collect()
}
