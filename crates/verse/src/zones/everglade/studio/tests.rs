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

#[test]
fn a_seat_stands_at_its_own_desk_and_seats_share_other_stations() {
    for (i, desk) in DESKS.iter().enumerate() {
        assert_eq!(standing(At::Desk, i as u32, 0, 1), (at_desk(desk), 0.0));
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
    assert_eq!(Studio::panel_at(at("library")), Some(PanelKind::Library));
    assert_eq!(Studio::panel_at(at("oracle")), None);
    assert_eq!(Studio::panel_at(Vec3::new(30.0, 0.0, 30.0)), None);
}

#[test]
fn a_seat_at_its_desk_stands_out_of_the_players_camera_path() {
    for (i, desk) in DESKS.iter().enumerate() {
        let figure = at_desk(desk);
        // Beside the desk's standing point, at the same depth, nearer the
        // hall's middle.
        assert!((figure[1] - desk.seat[1]).abs() < 1e-6);
        assert!(((figure[0] - desk.seat[0]).abs() - DESK_ASIDE).abs() < 1e-6);
        assert!(figure[0].abs() < desk.seat[0].abs());
        // A player using the desk stands at its standing point with the
        // camera behind them, facing the bench (+z): the seat stays clear
        // of the line from the camera to the player.
        let [px, pz] = desk.seat;
        for back in [2.5_f32, 9.0] {
            let camera = [px, pz - back];
            let t = ((figure[1] - camera[1]) / back).clamp(0.0, 1.0);
            let nearest = [camera[0], camera[1] + t * back];
            let clear = (figure[0] - nearest[0]).hypot(figure[1] - nearest[1]);
            assert!(clear >= 0.6, "{clear}");
        }
        // The seat's own spot still opens its desk.
        assert_eq!(
            Studio::panel_at(ground(figure)),
            Some(PanelKind::Desk(i as u32))
        );
    }
}

/// The highest and lowest elevation, radians, at which `eye` sees the
/// nameplate of a seat at `feet`, and the plate's height in the glade.
fn plate_seen(plate: &Mesh, feet: Vec3, eye: Vec3) -> Option<(f32, f32, f32)> {
    let transform = plate_transform(feet, eye)?;
    let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
    let (mut bottom, mut top) = (f32::INFINITY, f32::NEG_INFINITY);
    for v in &plate.faces {
        let p = transform.transform_point3(Vec3::from(v.pos));
        let d = p - eye;
        let elevation = d.y.atan2(d.x.hypot(d.z));
        low = low.min(elevation);
        high = high.max(elevation);
        bottom = bottom.min(p.y);
        top = top.max(p.y);
    }
    Some((low, high, top - bottom))
}

#[test]
fn a_nameplate_never_grows_past_its_screen_bound() {
    let plate = plate(
        &nameplate(&seat("grace", 2, Activity::Editing)),
        Attention::Working,
    );
    // The plate's rows reach exactly its full height.
    let top = plate
        .faces
        .iter()
        .map(|v| v.pos[1])
        .fold(f32::MIN, f32::max);
    let bottom = plate
        .faces
        .iter()
        .map(|v| v.pos[1])
        .fold(f32::MAX, f32::min);
    assert!((top - bottom - PLATE_TALL).abs() < 1e-4, "{}", top - bottom);

    let feet = ground(at_desk(&DESKS[2]));
    // Far away, the plate draws at full size over the seat's head.
    let far = feet + Vec3::new(0.0, 6.0, -30.0);
    let (_, _, tall) = plate_seen(&plate, feet, far).unwrap();
    assert!((tall - PLATE_TALL).abs() < 1e-3, "{tall}");
    // However near the eye comes, from level or above, the plate
    // subtends at most about PLATE_ANGLE.
    for back in [1.2_f32, 1.6, 2.5, 4.0, 6.0, 10.0] {
        for rise in [1.6_f32, 2.2, 3.0, 5.0] {
            let eye = feet + Vec3::new(0.3, rise, -back);
            let (low, high, _) = plate_seen(&plate, feet, eye).unwrap();
            assert!(
                high - low <= PLATE_ANGLE * 1.1,
                "{back} m back, {rise} m up: {}",
                high - low
            );
        }
    }
    // An eye at the seat draws no plate.
    assert!(plate_transform(feet, feet + Vec3::Y * 2.0).is_none());
}

#[test]
fn the_hall_camera_sees_desk_nameplates_whole() {
    // The `studio-hall` capture: the player at the desks station facing the
    // desks, the camera pulled in under the hall's ceiling and pitched down
    // 20.6 degrees with a vertical field of view of one radian, so the top
    // of the view is about 8 degrees over level.
    let eye = Vec3::new(0.0, 2.2, 3.4);
    let top_of_view = 0.5 - 0.36;
    let plate = plate(
        &nameplate(&seat("grace", 2, Activity::Editing)),
        Attention::Working,
    );
    for desk in &DESKS {
        let feet = ground(at_desk(desk));
        let (low, high, _) = plate_seen(&plate, feet, eye).unwrap();
        assert!(high < top_of_view - 0.02, "{:?}: {high}", desk.seat);
        assert!(high - low <= PLATE_ANGLE * 1.1);
    }
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
    let desk = ground(at_desk(&DESKS[1]));
    assert_eq!(studio.seat_position("ada"), Some(desk));
    assert!(!studio.seat_walking("ada"));

    // A nearby station: the seat walks there.
    studio.apply(snapshot(2, vec![seat("ada", 1, Activity::Reading)]), &[]);
    let (library, _) = standing(At::Library, 1, 0, 1);
    assert!(studio.seat_walking("ada"));
    studio.tick(0.1);
    let moved = studio.seat_position("ada").unwrap();
    assert!(moved != desk);
    assert!(moved.distance(desk) <= WALK_SPEED * 0.1 + 1e-3);

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
        spend: Default::default(),
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

#[test]
fn each_activity_has_its_posture() {
    use Activity as A;
    // At its own desk a seat sits, typing while it works.
    for activity in [A::Editing, A::Thinking, A::Reading, A::Judging] {
        assert_eq!(Posture::of(activity, At::Desk, true, false), Posture::Type);
    }
    for activity in [A::Idle, A::Paused, A::Done] {
        assert_eq!(Posture::of(activity, At::Desk, true, false), Posture::Sit);
    }
    // A seat with no desk of its own stands at the desks station.
    assert_eq!(
        Posture::of(A::Editing, At::Desk, false, false),
        Posture::Stand
    );
    // Every other activity takes its station's posture.
    for (activity, posture) in [
        (A::Reading, Posture::Read),
        (A::Running, Posture::Work),
        (A::Testing, Posture::Lean),
        (A::Judging, Posture::Think),
        (A::Waiting, Posture::Wait),
        (A::Blocked, Posture::Stand),
        (A::Paused, Posture::Stand),
        (A::Done, Posture::Stand),
        (A::Failed, Posture::Stand),
    ] {
        assert_eq!(
            Posture::of(activity, activity.station(), false, false),
            posture,
            "{activity:?}"
        );
    }
    // A standing seat gestures while it speaks; a seated or busy one keeps
    // its posture.
    assert_eq!(
        Posture::of(A::Waiting, At::Podium, false, true),
        Posture::Talk
    );
    assert_eq!(
        Posture::of(A::Done, At::TaskWall, false, true),
        Posture::Talk
    );
    assert_eq!(Posture::of(A::Editing, At::Desk, true, true), Posture::Type);
    assert_eq!(
        Posture::of(A::Reading, At::Library, false, true),
        Posture::Read
    );
    // Each state has its particles; a seat that only waits has none.
    assert_eq!(Particles::of(A::Thinking), Some(Particles::Thinking));
    assert_eq!(Particles::of(A::Editing), Some(Particles::Working));
    assert_eq!(Particles::of(A::Failed), Some(Particles::Error));
    assert_eq!(Particles::of(A::Done), Some(Particles::Done));
    assert_eq!(Particles::of(A::Waiting), None);
}

#[test]
fn seats_hold_their_posture_where_they_stand_and_walk_between() {
    let mut studio = Studio::default();
    studio.active = true;
    let team = |ada: Activity| vec![seat("ada", 1, ada), seat("lead", 0, Activity::Testing)];
    studio.apply(snapshot(1, team(Activity::Editing)), &[]);
    let find = |studio: &Studio, name: &str| {
        studio
            .figures()
            .into_iter()
            .find(|f| f.name == name)
            .unwrap()
    };
    let ada = find(&studio, "ada");
    assert_eq!(ada.posture, Posture::Type);
    assert_eq!(ada.speed, 0.0);
    // At its desk it looks at its monitor.
    assert_eq!(ada.look, Some(DESKS[1].monitor.center));
    let lead = find(&studio, "lead");
    assert_eq!(lead.posture, Posture::Lean);
    assert_ne!(ada.tint, lead.tint);
    // On its way to the library it stands and moves.
    studio.apply(snapshot(2, team(Activity::Reading)), &[]);
    studio.tick(0.1);
    let ada = find(&studio, "ada");
    assert_eq!(ada.posture, Posture::Stand);
    assert!((ada.speed - WALK_SPEED).abs() < 1e-3, "{}", ada.speed);
    walk(&mut studio, MAX_WALK);
    let ada = find(&studio, "ada");
    assert_eq!(ada.posture, Posture::Read);
    assert_eq!(ada.speed, 0.0);
    // A seat's look names its color.
    let mut violet = seat("grace", 2, Activity::Idle);
    violet.look = "violet".into();
    studio.apply(snapshot(3, vec![violet]), &[]);
    assert_eq!(find(&studio, "grace").tint, [0.8, 0.6, 1.0]);
}

#[test]
fn a_waiting_seat_walks_over_to_a_player_near_the_podium_and_back() {
    let mut studio = Studio::default();
    studio.active = true;
    studio.apply(snapshot(1, vec![seat("ada", 1, Activity::Waiting)]), &[]);
    let podium = STATIONS.iter().find(|s| s.id == "podium").unwrap();
    let home = ground(podium.at);
    assert_eq!(studio.seat_position("ada"), Some(home));
    // A player far away: the seat stays.
    let away = Vec3::new(-3.0, 0.0, -20.0);
    studio.set_player(Some(away));
    walk(&mut studio, 2.0);
    assert_eq!(studio.seat_position("ada"), Some(home));
    // The player comes near: the seat walks over and stands facing them.
    let player = Vec3::new(1.0, 0.0, -5.0);
    studio.set_player(Some(player));
    walk(&mut studio, MAX_WALK);
    let at = studio.seat_position("ada").unwrap();
    let gap = (at.x - player.x).hypot(at.z - player.z);
    assert!((gap - APPROACH_GAP).abs() < 0.1, "{gap}");
    let figure = &studio.figures()[0];
    assert_eq!(figure.posture, Posture::Wait);
    assert_eq!(figure.look, Some(player + Vec3::Y * 1.6));
    let toward = (player.x - at.x).atan2(player.z - at.z);
    assert!((figure.yaw - toward).abs() < 1e-3, "{}", figure.yaw);
    // The player leaves: it goes back to the podium.
    studio.set_player(Some(away));
    walk(&mut studio, MAX_WALK);
    assert_eq!(studio.seat_position("ada"), Some(home));
    assert_eq!(studio.figures()[0].yaw, podium.facing);
}

fn goal(id: &str, submitted_at: u64) -> coder_access::studio::Goal {
    coder_access::studio::Goal {
        goal: id.into(),
        text: "Greet the visitor".into(),
        workspace: "site".into(),
        lead: "lead".into(),
        status: coder_access::studio::GoalStatus::Decision,
        final_tasks: 0,
        total_tasks: 1,
        submitted_at,
    }
}

fn decision(
    id: &str,
    goal: &str,
    seat: Option<&str>,
    text: &str,
) -> coder_access::studio::Decision {
    coder_access::studio::Decision {
        decision: id.into(),
        goal: goal.into(),
        task: None,
        seat: seat.map(str::to_owned),
        kind: if seat.is_some() {
            coder_access::studio::DecisionKind::Question
        } else {
            coder_access::studio::DecisionKind::NoPlan
        },
        text: text.into(),
        based_on: 1,
    }
}

#[test]
fn the_mark_stands_over_the_seat_that_owns_the_oldest_decision() {
    let mut view = View {
        goals: vec![goal("g1", 10), goal("g2", 5)],
        seats: vec![
            seat("ada", 1, Activity::Waiting),
            seat("grace", 2, Activity::Waiting),
            seat("lead", 0, Activity::Idle),
        ],
        decisions: vec![
            decision("d1", "g1", Some("ada"), "Which greeting?"),
            decision("d2", "g2", Some("grace"), "May I run the tests?"),
        ],
        ..View::default()
    };
    view.canonicalize();
    // The second goal is older, so its decision is the oldest.
    assert_eq!(marked(&view), Some("grace"));
    // A decision about the goal itself belongs to the goal's lead.
    let mut goal_only = view.clone();
    goal_only.decisions[1] = decision("d2", "g2", None, "Plan the greeting.");
    assert_eq!(marked(&goal_only), Some("lead"));
    // No open decision, no mark.
    let mut none = view.clone();
    none.decisions.clear();
    assert_eq!(marked(&none), None);

    // The studio marks that seat, and each asking seat says its question.
    let mut studio = Studio::default();
    studio.active = true;
    studio.apply(
        Snapshot {
            stream: "ab".into(),
            sequence: 1,
            view,
        },
        &[],
    );
    assert_eq!(studio.marked_seat(), Some("grace"));
    let asks = studio.speech("grace").unwrap();
    assert_eq!(asks.to, Addressee::Person);
    assert_eq!(asks.text, "May I run the tests?");
    // The mark and the bubbles draw over the seats; the bubbles go once
    // said.
    let eye = Vec3::new(-3.0, 2.5, -14.0);
    let speaking = studio.draw(eye, false).faces.len();
    studio.tick(SPEECH_SECONDS + 0.1);
    assert_eq!(studio.speech("grace"), None);
    let said = studio.draw(eye, false).faces.len();
    assert!(said < speaking, "{said} {speaking}");
    assert_eq!(studio.marked_seat(), Some("grace"));
    // Without boxes, no seat is drawn as a boxy figure.
    assert!(studio.draw(eye, true).faces.len() > said);
}

#[test]
fn the_lead_asks_the_person_then_hands_a_worker_its_task() {
    let task = |id: &str, entry: &str, seat: &str, title: &str| Task {
        task: id.into(),
        goal: "g1".into(),
        entry: entry.into(),
        position: u32::from(entry != "lead"),
        title: title.into(),
        seat: seat.into(),
        depends_on: Vec::new(),
        status: TaskStatus::Running,
    };
    let mut before = View {
        goals: vec![goal("g1", 1)],
        seats: vec![seat("ada", 1, Activity::Editing), {
            let mut lead = seat("lead", 0, Activity::Waiting);
            lead.station = At::Podium;
            lead
        }],
        tasks: vec![task("t1", "lead", "lead", "Plan the greeting")],
        ..View::default()
    };
    before.canonicalize();
    let mut after = before.clone();
    after
        .tasks
        .push(task("t2", "e1", "ada", "Write the greeting"));
    after
        .decisions
        .push(decision("d1", "g1", Some("lead"), "Which greeting?"));
    after.canonicalize();
    let said = speeches(Some(&before), &after);
    assert_eq!(
        said,
        vec![
            Speech {
                speaker: "lead".into(),
                to: Addressee::Person,
                text: "Which greeting?".into(),
            },
            Speech {
                speaker: "lead".into(),
                to: Addressee::Seat("ada".into()),
                text: "Write the greeting".into(),
            },
        ]
    );
    // Said once: the same view again says nothing, and a first view says
    // only its open decisions.
    assert!(speeches(Some(&after), &after).is_empty());
    assert_eq!(speeches(None, &after).len(), 1);

    // The studio says them in turn, the speaker gesturing toward whom it
    // speaks to, and the addressee looking back.
    let mut studio = Studio::default();
    studio.active = true;
    let mut first = snapshot(1, Vec::new());
    first.view = before;
    studio.apply(first, &[]);
    let mut next = snapshot(2, Vec::new());
    next.view = after;
    studio.apply(next, &[]);
    assert_eq!(studio.speech("lead").unwrap().to, Addressee::Person);
    let lead = studio
        .figures()
        .into_iter()
        .find(|f| f.name == "lead")
        .unwrap();
    assert_eq!(lead.posture, Posture::Talk);
    studio.tick(SPEECH_SECONDS + 0.01);
    let hands = studio.speech("lead").unwrap();
    assert_eq!(hands.to, Addressee::Seat("ada".into()));
    let figures = studio.figures();
    let at = |name: &str| studio.seat_position(name).unwrap() + Vec3::Y * 1.6;
    let lead = figures.iter().find(|f| f.name == "lead").unwrap();
    assert_eq!(lead.look, Some(at("ada")));
    let ada = figures.iter().find(|f| f.name == "ada").unwrap();
    assert_eq!(ada.look, Some(at("lead")));
    // Ada keeps typing while she listens.
    assert_eq!(ada.posture, Posture::Type);
}

#[test]
fn a_bubble_wraps_its_text_between_words() {
    assert_eq!(
        wrap("Which greeting should the page use?", 12, 3),
        ["WHICH", "GREETING", "SHOULD TH..."]
    );
    assert_eq!(wrap("ok", 12, 3), ["OK"]);
    assert_eq!(wrap("abcdefghijklmnop", 5, 2), ["AB..."]);
    assert!(wrap("", 12, 3).is_empty());
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
