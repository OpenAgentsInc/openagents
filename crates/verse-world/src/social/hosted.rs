//! Everglade as a hosted social instance (`docs/verse/networking.md`,
//! "Shared Everglade"): the closed social profile a chamber host serves
//! for it, and, with the `studio` feature, the feed that publishes the
//! Agent Studio's seats into that instance.
//!
//! The profile carries Everglade's heightfield as one bounded triangle
//! mesh and one seat object per studio slot. The chamber's social rules
//! ([`crate::play::social`]) then admit walking and sitting, and its wire
//! serves every viewer the same seat poses (`State::social`). A seat actor
//! holds its slot's seat object, so a person cannot sit in it, and a slot a
//! person already sits in shows no seat actor until they stand.

use super::everglade::{DESK_SEATS, HALF_EXTENT, STATIONS, height};
use crate::play::social::{Kind, Object, PROFILE_REVISION, Profile, Zone};
#[cfg(feature = "studio")]
use crate::play::social::{SeatActor, State};
use glam::DVec3;
use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Triangle, Usage};

/// Cells along each side of the terrain mesh. Two triangles per cell keep
/// the mesh at the social profile's 512-triangle budget.
pub const GRID: usize = 16;

/// The studio slots, in the order a snapshot's seats fill them: one at
/// each desk, then one at each station's standing point. Each is a seat
/// object of the profile, numbered from 1.
#[must_use]
pub fn slots() -> Vec<([f32; 3], f32)> {
    let desks = DESK_SEATS.iter().map(|&[x, z]| ([x, height(x, z), z], 0.0));
    let stations = STATIONS.iter().map(|station| {
        let [x, z] = station.at;
        ([x, height(x, z), z], station.facing)
    });
    desks.chain(stations).collect()
}

/// Everglade's ground as a [`GRID`] by [`GRID`] mesh sampled from the
/// shared heightfield, wound so every face points up.
#[must_use]
pub fn terrain() -> Vec<Triangle> {
    let step = 2.0 * HALF_EXTENT / GRID as f32;
    let point = |i: usize, j: usize| {
        let x = -HALF_EXTENT + i as f32 * step;
        let z = -HALF_EXTENT + j as f32 * step;
        DVec3::new(f64::from(x), f64::from(height(x, z)), f64::from(z))
    };
    let mut triangles = Vec::with_capacity(GRID * GRID * 2);
    for i in 0..GRID {
        for j in 0..GRID {
            let (a, b, c, d) = (
                point(i, j),
                point(i, j + 1),
                point(i + 1, j + 1),
                point(i + 1, j),
            );
            triangles.push(Triangle([a, b, c]));
            triangles.push(Triangle([a, c, d]));
        }
    }
    triangles
}

/// The closed social profile a chamber host serves Everglade under
/// (`"profile": "everglade"` in its configuration).
///
/// # Errors
/// Returns a message when the terrain or the slots exceed the profile's
/// bounds.
pub fn everglade_profile() -> Result<Profile, String> {
    let mut geometry = Scene::default();
    geometry.insert(MeshCollider {
        key: ColliderKey {
            life: Life {
                instance: 0,
                entity: 0,
                generation: 0,
            },
            shape: 0,
        },
        layers: 1,
        usage: Usage::Blocking,
        mesh: Mesh::compile(terrain())?,
    })?;
    let objects = slots()
        .into_iter()
        .zip(1..)
        .map(|((feet, yaw), id)| Object {
            id,
            feet,
            yaw,
            kind: Kind::Seat,
        })
        .collect();
    let profile = Profile {
        revision: PROFILE_REVISION,
        zone: Zone::Everglade,
        geometry: geometry.snapshot(0)?,
        objects,
    };
    profile.validate()?;
    Ok(profile)
}

/// The seat actors to publish for `poses`, a studio's seats in its
/// snapshot's order: the first seat holds slot 1, and so on. A seat past
/// the last slot, in a slot a person sits in, or at a nonfinite point is
/// left out.
#[cfg(feature = "studio")]
#[must_use]
pub fn seat_actors(poses: &[super::seats::SeatPose], state: &State) -> Vec<SeatActor> {
    let seats: Vec<u64> = state
        .profile
        .objects
        .iter()
        .filter(|object| object.kind == Kind::Seat)
        .map(|object| object.id)
        .collect();
    poses
        .iter()
        .zip(seats)
        .filter(|(pose, seat)| {
            state.occupant(*seat).is_none()
                && pose.pos.iter().all(|v| v.is_finite())
                && pose.yaw.is_finite()
        })
        .map(|(pose, seat)| SeatActor {
            seat,
            feet: pose.pos,
            yaw: pose.yaw,
        })
        .collect()
}

/// Publishes the Agent Studio's seats into a hosted social instance: reads
/// a [`super::studio::SnapshotSource`], walks each seat toward its station
/// on the authority's tick, and hands the poses to the gateway as the
/// trusted host's projection. Every viewer then sees the same seat at the
/// same place. Run it from the serve loop's tick
/// ([`crate::service::net::Tick`]).
#[cfg(all(feature = "studio", feature = "service-auth"))]
pub struct StudioFeed {
    host: super::studio::StudioHost,
    since: f32,
    published: Option<Vec<SeatActor>>,
}

#[cfg(all(feature = "studio", feature = "service-auth"))]
impl StudioFeed {
    /// The most often the feed publishes, s. Seats walk at the authority's
    /// rate between publications.
    pub const PERIOD: f32 = 0.1;

    /// A feed reading `source`.
    #[must_use]
    pub fn new(source: Box<dyn super::studio::SnapshotSource>) -> Self {
        Self {
            host: super::studio::StudioHost::new(source, Vec::new()),
            since: Self::PERIOD,
            published: None,
        }
    }

    /// Walks the seats `dt` seconds and publishes them when they changed
    /// and [`Self::PERIOD`] has passed. A waiting seat meets the nearest
    /// living avatar.
    ///
    /// # Errors
    /// Returns a message when the gateway hosts no social profile or
    /// refuses the poses.
    pub fn tick(
        &mut self,
        gateway: &mut crate::service::auth::Gateway,
        dt: f32,
    ) -> Result<(), String> {
        let players: Vec<glam::Vec3> = gateway
            .game()
            .snapshot()
            .actors
            .iter()
            .filter(|actor| actor.alive)
            .map(|actor| glam::Vec3::from_array(actor.pos))
            .collect();
        self.host.tick(dt, &players);
        self.since += dt;
        if self.since < Self::PERIOD {
            return Ok(());
        }
        self.since = 0.0;
        let state = gateway
            .game()
            .social_state()
            .ok_or("The chamber hosts no social profile")?;
        let actors = seat_actors(&self.host.seats(), state);
        if self.published.as_ref() == Some(&actors) {
            return Ok(());
        }
        gateway.publish_social_studio(actors.clone())?;
        self.published = Some(actors);
        Ok(())
    }

    /// Wraps the feed as the serve loop's tick. A refused publication
    /// leaves the previous poses in place; the next change tries again.
    #[cfg(feature = "service-net")]
    #[must_use]
    pub fn into_tick(mut self) -> crate::service::net::Tick {
        Box::new(move |gateway, dt| {
            if self.tick(gateway, dt).is_err() {
                self.published = None;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_everglade_profile_is_closed_and_follows_the_heightfield() {
        let profile = everglade_profile().unwrap();
        assert_eq!(profile.zone, Zone::Everglade);
        assert_eq!(profile.objects.len(), slots().len());
        assert!(profile.objects.iter().all(|o| o.kind == Kind::Seat));
        // The same profile every time: its digest is the content identity.
        assert_eq!(
            profile.digest().unwrap(),
            everglade_profile().unwrap().digest().unwrap()
        );
        for triangle in terrain() {
            for p in triangle.0 {
                let ground = f64::from(height(p.x as f32, p.z as f32));
                assert!((p.y - ground).abs() < 1e-4);
            }
            let [a, b, c] = triangle.0;
            assert!((b - a).cross(c - a).y > 0.0, "every face points up");
        }
    }

    #[cfg(all(feature = "studio", feature = "service-auth"))]
    mod feed {
        use super::super::*;
        use crate::play::Game;
        use crate::service::{Chamber, auth::Gateway};
        use crate::social::seats::SeatPose;
        use crate::social::studio::SnapshotSource;
        use coder_access::studio::{Activity, Role, Seat, Snapshot, View};
        use std::sync::{Arc, Mutex};

        #[derive(Clone, Default)]
        struct Fed(Arc<Mutex<Option<Snapshot>>>);
        impl SnapshotSource for Fed {
            fn poll(&mut self) -> Option<Snapshot> {
                self.0.lock().unwrap().take()
            }
        }

        fn seat(name: &str, desk: u32) -> Seat {
            Seat {
                seat: name.into(),
                role: if desk == 0 { Role::Lead } else { Role::Worker },
                route: "codex:studio-sim".into(),
                look: "default".into(),
                desk,
                activity: Activity::Editing,
                station: Activity::Editing.station(),
                task: None,
                paused: false,
                spend: Default::default(),
            }
        }

        fn snapshot(sequence: u64, seats: Vec<Seat>) -> Snapshot {
            let mut view = View {
                seats,
                ..View::default()
            };
            view.canonicalize();
            Snapshot {
                stream: "ab".into(),
                sequence,
                view,
            }
        }

        fn pose(name: &str, x: f32) -> SeatPose {
            SeatPose {
                name: name.into(),
                pos: [x, 0.0, 0.0],
                yaw: 0.0,
                speed: 0.0,
                walking: false,
            }
        }

        fn gateway() -> Gateway {
            let mut scene = verse_engine::director::Scene::from_json(include_bytes!(
                "../../../../assets/verse/original/ritual.json"
            ))
            .unwrap();
            scene.actors.retain(|a| a.model == "adventurer");
            scene.actors[0].position = glam::Vec3::new(0.0, 0.0, -20.0);
            scene.cut_at = 0.;
            scene.cues.clear();
            scene.collision_profile = None;
            let game = Game::social_in(scene, 41, everglade_profile().unwrap()).unwrap();
            Gateway::new(Chamber::new(game).unwrap()).unwrap()
        }

        #[test]
        fn the_feed_publishes_the_studios_seats_and_skips_an_occupied_slot() {
            let mut gateway = gateway();
            let fed = Fed::default();
            *fed.0.lock().unwrap() = Some(snapshot(1, vec![seat("lead", 0), seat("ada", 1)]));
            let mut feed = StudioFeed::new(Box::new(fed.clone()));
            feed.tick(&mut gateway, 1.0 / 30.0).unwrap();
            let studio = gateway.game().social_state().unwrap().studio.clone();
            assert_eq!(studio.len(), 2);
            assert_eq!((studio[0].seat, studio[1].seat), (1, 2));
            // Each seat at its desk stands beside its bench, where every
            // viewer draws it.
            for actor in &studio {
                assert!(
                    DESK_SEATS[..2].iter().any(|desk| {
                        let home = crate::social::studio::at_desk(*desk);
                        (actor.feet[0] - home[0]).abs() < 0.01
                            && (actor.feet[2] - home[1]).abs() < 0.01
                    }),
                    "{actor:?}"
                );
            }

            // An unchanged studio publishes nothing new.
            let revision = gateway.game().social_state().unwrap().revision;
            feed.tick(&mut gateway, StudioFeed::PERIOD).unwrap();
            assert_eq!(gateway.game().social_state().unwrap().revision, revision);

            // A seat that leaves the snapshot leaves the world.
            *fed.0.lock().unwrap() = Some(snapshot(2, vec![seat("lead", 0)]));
            feed.tick(&mut gateway, StudioFeed::PERIOD).unwrap();
            assert_eq!(gateway.game().social_state().unwrap().studio.len(), 1);

            // A slot a person sits in shows no seat actor.
            let mut occupied = gateway.game().social_state().unwrap().clone();
            occupied.occupants.push(crate::play::social::Occupant {
                seat: 2,
                life: gateway.game().player_life(),
            });
            let actors = seat_actors(&[pose("lead", 0.0), pose("ada", 1.0)], &occupied);
            assert_eq!(actors.len(), 1);
            assert_eq!(actors[0].seat, 1);
        }
    }
}
