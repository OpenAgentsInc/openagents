//! The `everglade/studio` tests that run through the world
//! runtime, kept in `verse` when the zone moved into its own crate.

use crate::zones::everglade::studio::*;
use crate::zones::everglade::{STATIONS, boards, height};
use crate::zones::{Intent, ZoneId};
use coder_access::review::TaskReview;
use coder_access::studio::{Activity, Snapshot, View};
use coder_access::studio::{Role, Seat, Station as At};
use coder_access::{Error as AccessError, Operation, Right};
use glam::Vec3;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn ground(at: [f32; 2]) -> Vec3 {
    Vec3::new(at[0], height(at[0], at[1]), at[1])
}

/// Walks `studio` for `seconds`, a tenth of a second at a time.
fn walk(studio: &mut Studio, seconds: f32) {
    for _ in 0..(seconds * 10.0) as usize {
        studio.tick(0.1);
    }
}

/// A source that counts what the studio asks of it.
#[derive(Clone, Default)]
struct Counting {
    starts: Arc<AtomicUsize>,
    stops: Arc<AtomicUsize>,
    polls: Arc<AtomicUsize>,
}

impl Source for Counting {
    fn start(&mut self) {
        self.starts.fetch_add(1, Ordering::SeqCst);
    }
    fn stop(&mut self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }
    fn poll(&mut self, _: f32) -> Option<Snapshot> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst) as u64;
        Some(snapshot(n + 1, vec![seat("ada", 1, Activity::Waiting)]))
    }
    fn review(&mut self, _: &str) -> Option<TaskReview> {
        None
    }
}

#[test]
fn the_studio_loads_only_while_the_player_is_in_everglade() {
    let source = Counting::default();
    let mut runtime = crate::runtime::WorldRuntime::new();
    runtime.set_studio_source(Box::new(source.clone()));
    // On the plaza, an active surface reads nothing.
    runtime.update_studio(true, 0.1);
    assert_eq!(source.starts.load(Ordering::SeqCst), 0);
    assert_eq!(source.polls.load(Ordering::SeqCst), 0);
    assert!(runtime.studio().view().is_none());

    let mut runtime = crate::zones::everglade_tests::entered();
    runtime.set_studio_source(Box::new(source.clone()));
    runtime.update_studio(true, 0.1);
    assert_eq!(source.starts.load(Ordering::SeqCst), 1);
    assert_eq!(source.polls.load(Ordering::SeqCst), 1);
    assert!(runtime.studio().seat_position("ada").is_some());

    // A suspended surface stops observing and drops what it drew.
    runtime.update_studio(false, 0.1);
    assert_eq!(source.stops.load(Ordering::SeqCst), 1);
    assert_eq!(source.polls.load(Ordering::SeqCst), 1);
    assert!(runtime.studio().view().is_none());
    assert!(runtime.studio().seat_position("ada").is_none());

    // Back, then out through the return intent.
    runtime.update_studio(true, 0.1);
    assert_eq!(source.starts.load(Ordering::SeqCst), 2);
    runtime.zone_intent(Intent::Return).unwrap();
    assert_eq!(runtime.zone, ZoneId::Plaza);
    assert_eq!(source.stops.load(Ordering::SeqCst), 2);
    assert!(runtime.studio().view().is_none());
    runtime.update_studio(true, 0.1);
    assert_eq!(source.starts.load(Ordering::SeqCst), 2);
}

#[test]
fn the_interact_intent_needs_a_station_with_a_panel() {
    let mut runtime = crate::zones::everglade_tests::entered();
    // At the spawn on the approach, no station panel is in reach.
    assert_eq!(runtime.studio_panel_here(), None);
    assert!(runtime.zone_intent(Intent::Interact).is_err());
    assert!(
        !runtime
            .zone_snapshot(1.0)
            .controls
            .iter()
            .any(|c| c.action == Intent::Interact)
    );
    let podium = STATIONS.iter().find(|s| s.id == "podium").unwrap();
    runtime.set_spawn(podium.position(), podium.facing).unwrap();
    assert_eq!(runtime.studio_panel_here(), Some(PanelKind::Decisions));
    runtime.zone_intent(Intent::Interact).unwrap();
    let snapshot = runtime.zone_snapshot(1.0);
    assert!(
        snapshot
            .controls
            .iter()
            .any(|c| c.action == Intent::Interact && c.label == "Decisions")
    );
    assert!(snapshot.caption.contains("F opens the decisions"));
}

#[test]
fn a_click_on_a_seat_selects_it() {
    let mut runtime = crate::zones::everglade_tests::entered();
    // The counting source's seat waits at the podium.
    let source = Counting::default();
    runtime.set_studio_source(Box::new(source));
    // Stand in the yard south of the podium, facing it.
    runtime.set_spawn(Vec3::new(-3.0, 0.0, -12.0), 0.0).unwrap();
    runtime.update_studio(true, 0.0);
    let podium = STATIONS.iter().find(|s| s.id == "podium").unwrap();
    assert_eq!(
        runtime.studio().seat_position("ada"),
        Some(podium.position())
    );
    let aspect = 1.6;
    let view = runtime.view(aspect);
    let at = runtime.studio().seat_position("ada").unwrap() + Vec3::Y * 1.2;
    let clip = view.view_proj * at.extend(1.0);
    assert!(clip.w > 0.0);
    let x = (clip.x / clip.w + 1.0) / 2.0;
    let y = (1.0 - clip.y / clip.w) / 2.0;
    assert_eq!(
        runtime.studio_pick(aspect, x, y),
        Some(PanelKind::Seat("ada".into()))
    );
    // Far from every target, nothing is selected.
    assert_eq!(runtime.studio_pick(aspect, 0.02, 0.02), None);
}

#[cfg(feature = "model-host")]
mod simulated {
    use super::*;
    use crate::zones::everglade::studio::fixture::{Player, Recording};
    use std::sync::OnceLock;

    /// The simulated team's whole script, recorded once for every test.
    fn recording() -> &'static Recording {
        static RECORDING: OnceLock<Recording> = OnceLock::new();
        RECORDING.get_or_init(|| {
            let scratch = tempfile::tempdir().unwrap();
            Recording::run(&scratch.path().join("sim")).expect("the simulated team runs")
        })
    }

    #[test]
    fn the_recording_visits_every_working_station() {
        let recording = recording();
        let frames = recording.frames();
        assert!(frames.len() > 20);
        assert_eq!(frames[0].view.seats.len(), 3);
        for (activity, station) in [
            (Activity::Reading, At::Library),
            (Activity::Editing, At::Desk),
            (Activity::Running, At::Workbench),
            (Activity::Testing, At::ProvingGround),
            (Activity::Waiting, At::Podium),
            (Activity::Thinking, At::TaskWall),
        ] {
            assert!(
                recording
                    .find(|v| v
                        .seats
                        .iter()
                        .any(|s| s.activity == activity && s.station == station))
                    .is_some(),
                "{activity:?} at {station:?}"
            );
        }
        // Every frame is a valid studio, and the script ends with its goal
        // done.
        for frame in frames {
            frame.view.validate().expect(&frame.label);
        }
        let last = &frames.last().unwrap().view;
        assert_eq!(last.goals[0].status, coder_access::studio::GoalStatus::Done);
        // The person read each plan task's review.
        for task in last.tasks.iter().filter(|t| t.entry != "lead") {
            assert!(recording.review(&task.task).is_some(), "{}", task.title);
        }
    }

    #[test]
    fn updates_rebuild_every_frame_as_a_fresh_snapshot_would() {
        let recording = recording().clone();
        let frames: Vec<View> = recording.frames().iter().map(|f| f.view.clone()).collect();
        let mut player = Player::new(recording, 0, None);
        player.start();
        let mut shown = player.poll(0.0).expect("a first snapshot").view;
        assert_eq!(shown, frames[0]);
        for (index, frame) in frames.iter().enumerate().skip(1) {
            player.seek(index);
            if let Some(snapshot) = player.poll(0.0) {
                shown = snapshot.view;
            }
            assert_eq!(&shown, frame, "frame {index}");
        }
    }

    #[test]
    fn a_snapshot_fixture_renders_seats_at_their_classified_stations() {
        let recording = recording();
        let mut studio = Studio::default();
        studio.active = true;
        for (index, frame) in recording.frames().iter().enumerate() {
            let snapshot = Snapshot {
                stream: "ab".into(),
                sequence: index as u64 + 1,
                view: frame.view.clone(),
            };
            studio.apply(snapshot, &[]);
            walk(&mut studio, MAX_WALK + 1.0);
            let view = &frame.view;
            for (i, seat) in view.seats.iter().enumerate() {
                let (slot, count) = sharing(view, i);
                let (at, _) = standing(seat.station, seat.desk, slot, count);
                assert_eq!(
                    studio.seat_position(&seat.seat),
                    Some(ground(at)),
                    "{} in frame {index} ({})",
                    seat.seat,
                    frame.label
                );
            }
        }
    }

    #[test]
    fn everglade_plays_the_fixture_and_stations_open_their_panels() {
        let recording = recording().clone();
        let testing = recording
            .find(|v| v.seats.iter().any(|s| s.activity == Activity::Testing))
            .unwrap();
        let view = recording.frames()[testing].view.clone();
        let mut runtime = crate::zones::everglade_tests::entered();
        runtime.set_studio_source(Box::new(Player::new(recording.clone(), testing, None)));
        runtime.update_studio(true, 0.0);
        assert_eq!(runtime.studio().view(), Some(&view));
        let tester = view
            .seats
            .iter()
            .find(|s| s.activity == Activity::Testing)
            .unwrap();
        let proving = STATIONS.iter().find(|s| s.id == "proving").unwrap();
        let at = runtime.studio().seat_position(&tester.seat).unwrap();
        assert!((at.x - proving.at[0]).hypot(at.z - proving.at[1]) < 1.0);
        // The tester's desk monitor and nameplate are drawn.
        assert!(runtime.zone_dynamic_mesh().faces.len() > boards::live(None).faces.len());

        #[cfg(feature = "panels")]
        {
            use crate::panels::studio::{reviewable, rows, title};
            let console = rows(&PanelKind::Console, Some(&view), None);
            assert!(console.iter().any(|row| row.key.starts_with("goal-")));
            assert!(
                console
                    .iter()
                    .any(|row| row.key == format!("roster-{}", tester.seat))
            );
            let desk = PanelKind::Desk(tester.desk);
            assert_eq!(
                title(&desk, Some(&view)),
                format!("{} · {}", tester.seat, tester.route)
            );
            let seat = rows(&desk, Some(&view), None);
            assert!(seat.iter().any(|row| row.key.starts_with("log-")));
            // The merge station shows a done task's real diff.
            let done = recording.frames().last().unwrap().view.clone();
            let task = reviewable(&done)[0].task.clone();
            let review = recording.review(&task).cloned();
            assert!(review.as_ref().is_some_and(|r| !r.diff.is_empty()));
            let panel =
                crate::panels::studio::open(&PanelKind::Review, Some(&done), review.as_ref());
            assert_eq!(panel.tab(), crate::panels::Tab::Changes);
            assert_eq!(
                panel.diff_source(),
                review.as_ref().map(|r| r.diff.as_str())
            );
            // The podium shows the open decision while the lead asks.
            let asking = recording
                .find(|v| {
                    v.decisions
                        .iter()
                        .any(|d| d.kind == coder_access::studio::DecisionKind::Question)
                })
                .unwrap();
            let asking = &recording.frames()[asking].view;
            let decisions = rows(&PanelKind::Decisions, Some(asking), None);
            assert!(decisions.iter().any(|row| row.key.starts_with("decision-")));
        }
    }
}

/// A host connection's source holding every studio right.
struct Granted;

impl Source for Granted {
    fn poll(&mut self, _: f32) -> Option<Snapshot> {
        Some(snapshot(1, vec![seat("ada", 1, Activity::Waiting)]))
    }
    fn review(&mut self, _: &str) -> Option<TaskReview> {
        None
    }
    fn rights(&self) -> &[Right] {
        &[Right::Observe, Right::Operate, Right::Review]
    }
    fn send(&mut self, _: Operation) -> Result<u64, AccessError> {
        Ok(1)
    }
}

#[test]
fn a_viewer_with_only_the_world_right_opens_no_studio_panel() {
    let mut runtime = crate::zones::everglade_tests::entered();
    runtime.set_studio_source(Box::new(Granted));
    runtime.update_studio(true, 0.1);
    let podium = STATIONS.iter().find(|s| s.id == "podium").unwrap();
    runtime.set_spawn(podium.position(), podium.facing).unwrap();
    let pause = || Operation::PauseSeat { seat: "ada".into() };

    // A lone viewer reads and acts under its source's rights, as before.
    assert_eq!(runtime.studio_panel_here(), Some(PanelKind::Decisions));
    assert!(runtime.studio().access().act);

    // In a hosted instance the world grant admits walking only.
    runtime.set_studio_grant(Some(vec![Right::World]));
    assert_eq!(runtime.studio_panel_here(), None);
    assert_eq!(runtime.studio().access(), PanelAccess::default());
    assert!(runtime.studio().rights().is_empty());
    let refused = runtime.studio_send(pause()).unwrap_err();
    assert_eq!(refused.missing, Some(Right::Observe));
    // The seats still stand in the world.
    assert!(runtime.studio().seat_position("ada").is_some());

    // `observe` opens the panel without its actions; `operate` acts.
    runtime.set_studio_grant(Some(vec![Right::World, Right::Observe]));
    assert_eq!(runtime.studio_panel_here(), Some(PanelKind::Decisions));
    let access = runtime.studio().access();
    assert!(access.read && !access.act && !access.merge);
    let refused = runtime.studio_send(pause()).unwrap_err();
    assert_eq!(refused.missing, Some(Right::Operate));
    runtime.set_studio_grant(Some(vec![Right::World, Right::Observe, Right::Operate]));
    assert_eq!(runtime.studio_send(pause()), Ok(1));
}

#[test]
fn private_sales_boards_clear_on_inactive_surface_and_rebuild_after_reconnect() {
    use crate::zones::everglade::sales_floor::{Read, Snapshot, Source};
    struct PrivateOwner;
    impl Source for PrivateOwner {
        fn read(&mut self) -> Read {
            Read::Ready(
                Snapshot {
                    pipeline: [1, 0, 0, 0, 0],
                    pending_drafts: 1,
                    certificate_records: [0; 3],
                    practice_records: 0,
                    proposals: vec!["a".repeat(64)],
                    outbox_live: vec![],
                    outbox_fixture: vec![],
                    outbox_unknown: 0,
                    idle: false,
                    model_available: false,
                },
                0.0,
            )
        }
    }
    let mut runtime = crate::zones::everglade_tests::entered();
    runtime.update_studio(true, 0.1);
    assert!(runtime.private_sales_snapshot().is_none());
    let missing_faces = runtime.zone_dynamic_mesh().faces.len();
    runtime.set_sales_source(Some(Box::new(PrivateOwner)));
    runtime.update_studio(true, 0.1);
    assert_eq!(runtime.private_sales_snapshot().unwrap().pending_drafts, 1);
    assert!(runtime.zone_dynamic_mesh().faces.len() > missing_faces);
    runtime.update_studio(false, 0.1);
    assert!(runtime.private_sales_snapshot().is_none());
    runtime.update_studio(true, 0.1);
    assert_eq!(runtime.private_sales_snapshot().unwrap().pipeline[0], 1);
    runtime.set_sales_source(None);
    assert!(runtime.private_sales_snapshot().is_none());
}

#[cfg(feature = "hosted-social")]
#[test]
fn shared_world_transition_removes_private_sales_observations_and_reader() {
    use crate::zones::everglade::sales_floor::{Read, Snapshot, Source};
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
    use verse_world::play::social::{Profile, Zone};
    struct PrivateOwner;
    impl Source for PrivateOwner {
        fn read(&mut self) -> Read {
            Read::Ready(
                Snapshot {
                    pipeline: [1, 0, 0, 0, 0],
                    pending_drafts: 1,
                    certificate_records: [0; 3],
                    practice_records: 0,
                    proposals: vec!["a".repeat(64)],
                    outbox_live: vec![],
                    outbox_fixture: vec![],
                    outbox_unknown: 0,
                    idle: false,
                    model_available: false,
                },
                0.0,
            )
        }
    }
    let mut runtime = crate::zones::everglade_tests::entered();
    runtime.set_sales_source(Some(Box::new(PrivateOwner)));
    runtime.update_studio(true, 0.1);
    assert!(runtime.private_sales_snapshot().is_some());
    let mut geometry = Scene::default();
    geometry
        .insert(MeshCollider {
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
            mesh: Mesh::from_box(
                glam::DVec3::new(-12., -1., -12.),
                glam::DVec3::new(12., 0., 12.),
            )
            .unwrap(),
        })
        .unwrap();
    let profile = Profile {
        revision: 1,
        zone: Zone::Everglade,
        geometry: geometry.snapshot(0).unwrap(),
        objects: vec![],
    };
    runtime.enter_hosted_social(7, &profile).unwrap();
    assert!(runtime.private_sales_snapshot().is_none());
    runtime.update_studio(true, 0.1);
    assert!(runtime.private_sales_snapshot().is_none());
    assert!(runtime.zone_state.sales_floor.snapshot().is_none());
}
