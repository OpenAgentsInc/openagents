use super::controller::Footprint;
use super::everglade::{HALL, height};
use super::solids::Solids;
use super::world::{Profile, World, content_digest};
use crate::{Command, Controller, Intent, Refusal};
use glam::Vec3;

/// The hall's south wall west of its door, as a stand-in building: tall,
/// and across the approach between the yard and the hall.
fn hall_wall() -> Footprint {
    let south = HALL.0[1] - HALL.1[1];
    Footprint {
        min: [-8.0, south - 0.2],
        max: [-1.5, south + 0.2],
    }
}

fn profile() -> Profile {
    let mut solids = Solids::over(height);
    solids.add_block(hall_wall(), 6.0);
    Profile::everglade([7; 32], solids)
}

fn walk(world: &mut World, who: Controller, life: verse_engine::core::LifeId, yaw: f32) {
    let admission = world.admission(life).unwrap().clone();
    let command: Command<()> = admission
        .command(
            world.tick(),
            Intent::Move {
                axes: [0.0, 1.0],
                yaw,
            },
        )
        .unwrap();
    world.command(who, &command).unwrap();
}

#[test]
fn the_social_profile_admits_a_session_without_hostiles_and_walls_stop_it() {
    // Nothing in the profile is an adventurer or a hostile, which the
    // chamber's rules require.
    let mut world = World::new(profile(), 3);
    let who = Controller(11);
    let life = world.join(who).unwrap();
    assert_eq!(life.instance, 3);
    assert!(world.join(who).is_err(), "one avatar per session");
    let spawn = world.avatar(life).unwrap().pos;
    assert_eq!(spawn.y, height(spawn.x, spawn.z));

    // Another controller cannot move this avatar, and a cast finds no combat.
    let admission = world.admission(life).unwrap().clone();
    let cast = admission
        .command(
            0,
            Intent::Cast {
                ability: (),
                target: None,
                aim: [0.0, 0.0, 1.0],
            },
        )
        .unwrap();
    assert_eq!(
        world.command(Controller(12), &cast),
        Err(Refusal::NotController)
    );
    assert_eq!(world.command(who, &cast), Err(Refusal::InvalidIntent));

    // Walking north from the yard toward the wall stops outside it.
    let south = HALL.0[1] - HALL.1[1];
    let mut world = World::new(profile(), 3);
    let life = world.join(who).unwrap();
    for _ in 0..60 * 6 {
        walk(&mut world, who, life, 0.0);
        world.step();
    }
    let at = world.avatar(life).unwrap().pos;
    // The spawn's x is 0, east of this wall: it walks through the doorway.
    assert!(at.z > south, "{at}");

    let mut profile = profile();
    profile.spawn = Vec3::new(-5.0, 0.0, -1.0);
    let mut world = World::new(profile, 3);
    let life = world.join(who).unwrap();
    for _ in 0..60 * 3 {
        walk(&mut world, who, life, 0.0);
        world.step();
    }
    let at = world.avatar(life).unwrap().pos;
    assert!(at.z < south - 0.2, "{at}");
    assert!(at.z > 0.0, "it walked up to the wall: {at}");
}

#[test]
fn walking_climbs_the_heightfield_like_a_local_player() {
    let mut profile = profile();
    profile.spawn = Vec3::new(0.0, 0.0, -142.0);
    let mut world = World::new(profile, 0);
    let who = Controller(1);
    let life = world.join(who).unwrap();
    for _ in 0..60 * 3 {
        // Yaw pi faces -z: out of the clearing, up the rise.
        walk(&mut world, who, life, std::f32::consts::PI);
        world.step();
    }
    let at = world.avatar(life).unwrap().pos;
    assert!(at.z < -151.0, "{at}");
    assert!((at.y - height(at.x, at.z)).abs() < 1e-3, "{at}");
    assert!(at.y > 1.0, "{at}");
}

#[test]
fn a_content_digest_decodes_from_its_pin() {
    let hex = "4bbd3b18ae0f698a37da40e738176b180e3d7b2a4e72944102424a16ce0b598c";
    let digest = content_digest(hex).unwrap();
    assert_eq!(digest[0], 0x4b);
    assert_eq!(digest[31], 0x8c);
    assert!(content_digest("4bbd").is_err());
    assert!(content_digest(&"zz".repeat(32)).is_err());
}

#[cfg(feature = "studio")]
mod studio {
    use super::super::seats::{SeatPose, Seats};
    use super::super::studio::{PanelAccess, SnapshotSource, StudioHost, admit, admit_read, plans};
    use super::*;
    use coder_access::studio::{
        Activity, MergeDecision, Role, Seat, Snapshot, Station, Verdict, View,
    };
    use coder_access::{Code, Operation, Right};
    use std::sync::{Arc, Mutex};

    fn seat(name: &str, desk: u32, activity: Activity) -> Seat {
        Seat {
            seat: name.into(),
            role: if desk == 0 { Role::Lead } else { Role::Worker },
            route: "codex:studio-sim".into(),
            look: "default".into(),
            desk,
            activity,
            station: activity.station(),
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

    /// A source a test feeds: the next poll returns what it holds.
    #[derive(Clone, Default)]
    struct Fed(Arc<Mutex<Option<Snapshot>>>);
    impl Fed {
        fn feed(&self, snapshot: Snapshot) {
            *self.0.lock().unwrap() = Some(snapshot);
        }
    }
    impl SnapshotSource for Fed {
        fn poll(&mut self) -> Option<Snapshot> {
            self.0.lock().unwrap().take()
        }
    }

    fn walls() -> Vec<Footprint> {
        vec![hall_wall()]
    }

    #[test]
    fn every_viewer_sees_the_authoritys_seat_at_the_same_place() {
        let fed = Fed::default();
        let mut host = StudioHost::new(Box::new(fed.clone()), walls());
        fed.feed(snapshot(
            1,
            vec![
                seat("lead", 0, Activity::Editing),
                seat("w1", 1, Activity::Reading),
            ],
        ));
        host.tick(0.05, &[]);
        // The lead moves from its desk to the podium to wait on a person.
        let mut waiting = seat("lead", 0, Activity::Waiting);
        waiting.station = Station::Podium;
        fed.feed(snapshot(2, vec![waiting, seat("w1", 1, Activity::Reading)]));
        // One viewer stands near the podium; the authority walks for all.
        let players = [Vec3::new(-3.0, 0.0, -6.0), Vec3::new(20.0, 0.0, -20.0)];
        let mut published: Vec<Vec<SeatPose>> = Vec::new();
        for _ in 0..40 {
            host.tick(0.05, &players);
            published.push(host.seats());
        }
        let last = published.last().unwrap();
        let lead = last.iter().find(|s| s.name == "lead").unwrap();
        assert!(lead.pos[2] < 0.0, "the lead left the hall: {lead:?}");

        // Two viewers fed the same publication draw the same seats.
        let viewer_a = last.clone();
        let viewer_b: Vec<SeatPose> =
            serde_json::from_str(&serde_json::to_string(last).unwrap()).unwrap();
        assert_eq!(viewer_a, viewer_b);

        // A second authority fed the same snapshots and players agrees
        // pose for pose: the walk is deterministic.
        let mut again = Seats::new(walls());
        let first = snapshot(
            1,
            vec![
                seat("lead", 0, Activity::Editing),
                seat("w1", 1, Activity::Reading),
            ],
        );
        again.apply(&plans(&first.view));
        again.tick(0.05, &[]);
        let mut waiting = seat("lead", 0, Activity::Waiting);
        waiting.station = Station::Podium;
        again.apply(&plans(
            &snapshot(2, vec![waiting, seat("w1", 1, Activity::Reading)]).view,
        ));
        for published in &published {
            again.tick(0.05, &players);
            assert_eq!(&again.poses(), published);
        }
    }

    #[test]
    fn the_world_right_alone_walks_but_opens_no_studio_panel() {
        let fed = Fed::default();
        let mut host = StudioHost::new(Box::new(fed.clone()), walls());
        fed.feed(snapshot(1, vec![seat("lead", 0, Activity::Editing)]));
        host.tick(0.05, &[]);

        let world = [Right::World];
        assert_eq!(PanelAccess::of(&world), PanelAccess::default());
        assert_eq!(admit_read(&world).unwrap_err().code, Code::MissingRight);
        let refused = host.view(&world).unwrap_err();
        assert_eq!(refused.missing, Some(Right::Observe));
        // The seats are part of the world all the same.
        assert_eq!(host.seats().len(), 1);

        let pause = Operation::PauseSeat {
            seat: "lead".into(),
        };
        let merge = Operation::DecideMerge {
            decision: Box::new(MergeDecision {
                task: "t1".into(),
                base: "a".repeat(40),
                head_commit: "b".repeat(40),
                head: "c".repeat(40),
                verdict: Verdict::Merge,
                text: String::new(),
                command: "d".repeat(64),
                issued_at: 1,
            }),
        };
        assert_eq!(
            host.admit(&world, &pause).unwrap_err().missing,
            Some(Right::Observe)
        );

        let observe = [Right::World, Right::Observe];
        assert!(host.view(&observe).is_ok());
        assert_eq!(
            admit(&observe, &pause).unwrap_err().missing,
            Some(Right::Operate)
        );
        let operate = [Right::World, Right::Observe, Right::Operate];
        assert!(admit(&operate, &pause).is_ok());
        assert_eq!(
            admit(&operate, &merge).unwrap_err().missing,
            Some(Right::Review)
        );
        let review = [Right::Observe, Right::Review];
        assert!(admit(&review, &merge).is_ok());
        assert!(PanelAccess::of(&review).merge);
    }
}
