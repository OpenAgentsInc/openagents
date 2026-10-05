//! A published trace played in the Grid's Gym as a replay.
//!
//! The landmark mapping is the desktop run replay's
//! (`docs/verse/README.md#run-replays`): commands and what the agent said at
//! the workbench, Jev's decision at the oracle, retrieval at the library,
//! tests and the verifier at the proving ground; before its first placed
//! step the ghost waits on the plaza, just inside the doorway. In the Grid the places are stations inside the Gym's hall.
//! The replay reads the trace viewer's rows on the viewer's clock
//! (`gym_leaderboard::view::row_times`), so its visit `n` is the viewer's
//! step `n` at the same time; the ghost goes where the viewer's playhead
//! is.
use crate::mesh::Mesh;
use crate::replay::Place;
use crate::world::GymSite;
use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use gym_leaderboard::contract::{StepKind, TraceBundle};
use gym_leaderboard::view::row_times;

/// One visit: the viewer's row, the bundle step it shows, its time, and
/// where the ghost stands for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceVisit {
    pub step: usize,
    pub at_ms: u64,
    pub place: Place,
}

/// A bundle's rows as visits, in the viewer's order.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceReplay {
    pub visits: Vec<TraceVisit>,
}

impl TraceReplay {
    #[must_use]
    pub fn of(bundle: &TraceBundle) -> Self {
        let mut place = Place::Plaza;
        let visits = row_times(bundle)
            .into_iter()
            .map(|row| {
                place = place_of(&bundle.steps[row.step].kind, bundle.verifier.is_some())
                    .unwrap_or(place);
                TraceVisit {
                    step: row.step,
                    at_ms: row.at_ms,
                    place,
                }
            })
            .collect();
        Self { visits }
    }

    /// The visit for the viewer's row `row`.
    #[must_use]
    pub fn visit(&self, row: usize) -> Option<&TraceVisit> {
        self.visits.get(row)
    }
}

/// Where a step happens, or `None` to stay where the ghost is.
#[must_use]
pub fn place_of(kind: &StepKind, verified: bool) -> Option<Place> {
    Some(match kind {
        StepKind::Command { .. }
        | StepKind::CommandResult { .. }
        | StepKind::Say { .. }
        | StepKind::ModelStep { .. }
        | StepKind::DelegateStarted { .. } => Place::Workbench,
        StepKind::Decision { .. } => Place::Oracle,
        StepKind::Retrieval { .. } => Place::Library,
        StepKind::Tests { .. } => Place::ProvingGround,
        // The verifier grades what the delegate left: the proving ground.
        StepKind::DelegateEnded { .. } | StepKind::Ended { .. } if verified => Place::ProvingGround,
        StepKind::DelegateEnded { .. } | StepKind::Ended { .. } => Place::Plaza,
        // The host's bookkeeping moves no one: before the first placed
        // step the ghost waits on the plaza, after the last it stays.
        StepKind::Host { .. } | StepKind::Usage { .. } | StepKind::Other => return None,
    })
}

/// Where a place's station stands in the Gym's own frame: the plaza just
/// inside the doorway, and the others in a row under the RESULTS board,
/// below its lettering, in view of a player reading the panel there.
#[must_use]
pub fn station(place: Place) -> Vec3 {
    match place {
        Place::Plaza => Vec3::new(39.0, 0.0, 0.0),
        Place::Workbench => Vec3::new(57.2, 0.0, 3.0),
        Place::Oracle => Vec3::new(57.2, 0.0, 4.4),
        Place::Library => Vec3::new(57.2, 0.0, 5.8),
        Place::ProvingGround => Vec3::new(57.2, 0.0, 7.2),
    }
}

/// The ghost at `at` (world coordinates), hovering, with a ring under it,
/// in the neutral palette.
#[must_use]
pub fn ghost_mesh(at: Vec3) -> Mesh {
    let mut mesh = Mesh::default();
    // Below the boards' lower edge, so it never hides their lettering.
    let hover = at + Vec3::Y * 0.7;
    mesh.cube(
        Mat4::from_translation(hover) * Mat4::from_scale(Vec3::splat(0.45)),
        Intensity::ThreeQuarters,
    );
    mesh.ring(at.with_y(0.03), 0.6, 24, Intensity::Full);
    mesh.line(at.with_y(0.03), hover, Intensity::Quarter);
    mesh.neutralize();
    mesh
}

/// The ghost's world position for `place` in the Gym at `site`.
#[must_use]
pub fn ghost_at(site: GymSite, place: Place) -> Vec3 {
    site.point(station(place))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gym_leaderboard::view::{Nav, Page, Tab, render};

    fn published() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/terminal-bench/published")
    }

    /// The replay and the trace viewer agree on every step and time.
    #[test]
    fn the_replay_and_the_viewer_agree_on_every_step_and_time() {
        let leaderboard: gym_leaderboard::contract::Leaderboard = serde_json::from_slice(
            &std::fs::read(published().join(gym_leaderboard::LEADERBOARD_FILE)).unwrap(),
        )
        .unwrap();
        let mut nav = Nav::default();
        nav.select_board(&leaderboard, "tb4-fable-delegate-repro-9776")
            .unwrap();
        nav.select_attempt(&leaderboard, "coq-block-bound.p2")
            .unwrap();
        let trace = nav.open_trace(&leaderboard).unwrap();
        let bundle: TraceBundle =
            serde_json::from_slice(&std::fs::read(published().join(&trace.path)).unwrap()).unwrap();
        nav.set_tab(Tab::Agent).unwrap();
        let replay = TraceReplay::of(&bundle);
        let clock = |nav: &Nav| match render(nav, &leaderboard, None, Some(&bundle)).unwrap() {
            Page::Trace(page) => page.clock,
            _ => panic!(),
        };
        assert_eq!(replay.visits.len(), clock(&nav).steps);
        let mut places = std::collections::BTreeSet::new();
        for row in 0..replay.visits.len() {
            if row > 0 {
                nav.step(&bundle, true).unwrap();
            }
            let viewer = clock(&nav);
            let visit = replay.visit(viewer.step).unwrap();
            assert_eq!(viewer.step, row);
            assert_eq!(visit.at_ms, viewer.playhead_ms, "row {row}");
            if let Some(place) = place_of(&bundle.steps[visit.step].kind, true) {
                assert_eq!(visit.place, place, "row {row}");
            }
            places.insert(visit.place.name());
        }
        // The episode visits the oracle (Jev), the workbench (the
        // delegate's commands), and the proving ground (the verifier).
        for place in ["oracle", "workbench", "proving ground"] {
            assert!(places.contains(place), "{places:?}");
        }
    }

    #[test]
    fn stations_stand_inside_the_hall_clear_of_its_walls() {
        let site = GymSite::GRID;
        for place in [
            Place::Plaza,
            Place::Workbench,
            Place::Oracle,
            Place::Library,
            Place::ProvingGround,
        ] {
            let at = ghost_at(site, place);
            assert!(site.inside(at), "{place:?}");
            assert!(!ghost_mesh(at).faces.is_empty());
        }
    }
}
