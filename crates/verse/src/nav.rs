//! Bounded ground navigation over the same footprints as ordinary walking.
//! Routes use a two-meter grid and exact segment clearance; they never move a player.
//! Planning lives in `verse_world::social::nav`, so a social world's seat
//! actors route as the zones do; this module re-exports it and keeps the
//! player's navigation state.

pub use verse_world::social::nav::{NavError, Route, plan, segment_clear};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationStatus {
    #[default]
    Idle,
    Walking,
    Arrived,
    Cancelled,
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub struct Navigation {
    pub(crate) status: NavigationStatus,
    pub(crate) route: Option<Route>,
    pub(crate) next: usize,
    pub(crate) stalled: f32,
}
impl Navigation {
    #[must_use]
    pub fn status(&self) -> NavigationStatus {
        self.status
    }
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status == NavigationStatus::Walking
    }
    #[must_use]
    pub fn destination(&self) -> Option<[f32; 2]> {
        self.route.as_ref().map(|r| r.destination)
    }
    #[must_use]
    pub fn waypoints(&self) -> &[[f32; 2]] {
        self.route
            .as_ref()
            .map_or(&[], |r| &r.waypoints[self.next..])
    }
    pub(crate) fn start(&mut self, route: Route) {
        self.route = Some(route);
        self.next = 0;
        self.stalled = 0.0;
        self.status = NavigationStatus::Walking;
    }
    pub(crate) fn stop(&mut self, status: NavigationStatus) {
        self.status = status;
        if let Some(route) = &self.route {
            self.next = route.waypoints.len();
        }
        self.stalled = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_map_landmark_has_a_clear_route_from_spawn() {
        let world = crate::world::build();
        for landmark in crate::minimap::LANDMARKS {
            let route = plan(
                [crate::world::SPAWN.x, crate::world::SPAWN.z],
                [landmark.x, landmark.z],
                &world.blockers,
                crate::world::HALF,
            );
            assert!(route.is_ok(), "{}: {route:?}", landmark.label);
        }
    }

    #[test]
    fn seeded_world_gym_is_reachable_through_the_doorway() {
        let world = crate::world::build();
        let route = plan(
            [0.0, -10.0],
            [48.0, 0.0],
            &world.blockers,
            crate::world::HALF,
        )
        .unwrap();
        let mut previous = [0.0, -10.0];
        for next in route.waypoints {
            assert!(segment_clear(
                previous,
                next,
                &world.blockers,
                crate::world::HALF
            ));
            previous = next;
        }
        assert_eq!(previous, [48.0, 0.0]);
    }
}
