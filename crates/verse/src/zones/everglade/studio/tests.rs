use super::*;
use crate::zones::{Intent, ZoneId};
use coder_access::studio::{Log, LogLine, Role, Seat, Station as At, Task, TaskStatus};
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

#[test]
fn a_seat_stands_at_its_own_desk_and_seats_share_other_stations() {
    for (i, desk) in DESKS.iter().enumerate() {
        assert_eq!(standing(At::Desk, i as u32, 0, 1), (desk.seat, 0.0));
    }
    // A seat with no desk of its own stands at the desks station.
    let desks = STATIONS.iter().find(|s| s.id == "desks").unwrap();
    assert_eq!(standing(At::Desk, 99, 0, 1).0, desks.at);
    // Two seats at the library stand apart, either side of its point.
    let library = STATIONS.iter().find(|s| s.id == "library").unwrap();
    let (a, _) = standing(At::Library, 0, 0, 2);
    let (b, facing) = standing(At::Library, 0, 1, 2);
    assert_eq!(facing, library.facing);
    let gap = (a[0] - b[0]).hypot(a[1] - b[1]);
    assert!((gap - SLOT).abs() < 1e-4, "{gap}");
    let middle = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    assert!((middle[0] - library.at[0]).hypot(middle[1] - library.at[1]) < 1e-4);
    // Every snapshot station has a place in the layout.
    for station in [
        At::Desk,
        At::Library,
        At::Workbench,
        At::ProvingGround,
        At::Oracle,
        At::Podium,
        At::Lounge,
        At::TaskWall,
    ] {
        assert!(STATIONS.iter().any(|s| s.id == place_id(station)));
    }
}

#[test]
fn stations_open_their_panels() {
    let at = |id: &str| STATIONS.iter().find(|s| s.id == id).unwrap().position();
    assert_eq!(Studio::panel_at(at("task_wall")), Some(PanelKind::Console));
    assert_eq!(Studio::panel_at(at("podium")), Some(PanelKind::Decisions));
    assert_eq!(Studio::panel_at(at("merge")), Some(PanelKind::Review));
    // The desks station opens the nearest desk's seat, and each desk its own.
    assert!(matches!(
        Studio::panel_at(at("desks")),
        Some(PanelKind::Desk(_))
    ));
    for (i, desk) in DESKS.iter().enumerate() {
        assert_eq!(
            Studio::panel_at(ground(desk.seat)),
            Some(PanelKind::Desk(i as u32))
        );
    }
    // Stations without a panel, and open ground, open nothing.
    assert_eq!(Studio::panel_at(at("library")), None);
    assert_eq!(Studio::panel_at(Vec3::new(30.0, 0.0, 30.0)), None);
}

#[test]
fn lettering_keeps_the_board_alphabet() {
    assert_eq!(lettering("codex:studio-sim", 24), "CODEX STUDIO SIM");
    assert_eq!(lettering("Plan: a/b.c", 24), "PLAN  A/B.C");
    assert_eq!(lettering("abcdef", 3), "ABC");
}

#[test]
fn seats_walk_to_their_station_and_skip_ahead_when_activity_outruns_them() {
    let mut studio = Studio::default();
    studio.active = true;
    studio.apply(snapshot(1, vec![seat("ada", 1, Activity::Editing)]), &[]);
    // A seat seen first stands at its station at once.
    assert_eq!(studio.seat_position("ada"), Some(ground(DESKS[1].seat)));
    assert!(!studio.seat_walking("ada"));

    // A nearby station: the seat walks there.
    studio.apply(snapshot(2, vec![seat("ada", 1, Activity::Reading)]), &[]);
    let (library, _) = standing(At::Library, 1, 0, 1);
    assert!(studio.seat_walking("ada"));
    studio.tick(0.1);
    let moved = studio.seat_position("ada").unwrap();
    assert!(moved != ground(DESKS[1].seat));
    assert!(moved.distance(ground(DESKS[1].seat)) <= WALK_SPEED * 0.1 + 1e-3);

    // The activity changes again before it arrives: it skips ahead.
    studio.apply(snapshot(3, vec![seat("ada", 1, Activity::Judging)]), &[]);
    let (oracle, _) = standing(At::Oracle, 1, 0, 1);
    assert!(!studio.seat_walking("ada"));
    assert_eq!(studio.seat_position("ada"), Some(ground(oracle)));

    // Back to the library, then all the way to arrival.
    studio.apply(snapshot(4, vec![seat("ada", 1, Activity::Reading)]), &[]);
    walk(&mut studio, MAX_WALK);
    assert!(!studio.seat_walking("ada"));
    assert_eq!(studio.seat_position("ada"), Some(ground(library)));

    // The lounge is farther than a walk takes: it skips ahead.
    studio.apply(snapshot(5, vec![seat("ada", 1, Activity::Blocked)]), &[]);
    let (lounge, _) = standing(At::Lounge, 1, 0, 1);
    assert!(!studio.seat_walking("ada"));
    assert_eq!(studio.seat_position("ada"), Some(ground(lounge)));

    // A seat that leaves the snapshot leaves the glade.
    studio.apply(snapshot(6, vec![seat("grace", 2, Activity::Idle)]), &[]);
    assert_eq!(studio.seat_position("ada"), None);
    assert!(studio.seat_position("grace").is_some());
}

#[test]
fn the_world_draws_seats_lamps_beacons_and_live_boards() {
    let idle = boards::live(None);
    let mut studio = Studio::default();
    assert_eq!(studio.mesh(Vec3::ZERO).faces.len(), idle.faces.len());
    studio.active = true;
    let mut waiting = seat("lead", 0, Activity::Waiting);
    waiting.task = Some("t1".into());
    let mut snap = snapshot(1, vec![waiting, seat("ada", 1, Activity::Idle)]);
    snap.view.tasks.push(Task {
        task: "t1".into(),
        goal: "g1".into(),
        entry: "lead".into(),
        position: 0,
        title: "Plan the greeting".into(),
        seat: "lead".into(),
        depends_on: Vec::new(),
        status: TaskStatus::Waiting,
    });
    snap.view.logs.push(Log {
        seat: "lead".into(),
        task: Some("t1".into()),
        lines: vec![LogLine {
            at: 1,
            activity: Activity::Waiting,
            text: "waiting: ask_user — Which greeting?".into(),
        }],
    });
    studio.apply(snap, &[]);
    let mesh = studio.mesh(Vec3::new(0.0, 2.0, -20.0));
    assert!(mesh.faces.len() > idle.faces.len());
    // The waiting seat raises a beacon in the lamp's color; the idle seat
    // has no lamp.
    let lamp = Attention::AwaitingInput.lamp().unwrap();
    assert!(mesh.lines.iter().any(|v| v.color == lamp && v.pos[1] > 6.0));
    assert_eq!(Attention::of(Activity::Idle).lamp(), None);
    // The card and the monitor's line add lettering to the boards.
    assert!(boards::live(studio.view()).faces.len() > idle.faces.len());
    assert_eq!(boards::column(TaskStatus::Waiting), 2);
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

    let mut runtime = super::super::tests::entered();
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
    let mut runtime = super::super::tests::entered();
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
    let mut runtime = super::super::tests::entered();
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
        let mut runtime = super::super::super::tests::entered();
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
