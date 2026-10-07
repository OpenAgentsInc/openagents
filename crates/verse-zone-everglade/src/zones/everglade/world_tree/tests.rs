use std::collections::BTreeSet;
use std::path::PathBuf;

use coder_access::studio::{
    Activity, Role, Seat, Station as StudioStation, Task, TaskStatus, View,
};
use town_clock::TownTime;
use verse_world::social::nav;
use verse_world::social::sight::Footprints;
use world_tree::{Affordance, Kind, Known, Object, State};

use super::super::HALF_EXTENT;
use super::super::tests::world;
use super::*;

/// The checked-in snapshot `coder` and the web build read.
fn snapshot_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../world-tree/data/everglade.json")
}

#[test]
fn the_world_tree_snapshot_is_current() {
    let json = everglade().to_json();
    let path = snapshot_path();
    if std::env::var_os("WORLD_TREE_WRITE").is_some() {
        std::fs::write(&path, &json).unwrap();
    }
    let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        checked_in == json,
        "crates/world-tree/data/everglade.json is stale; regenerate it with \
         `WORLD_TREE_WRITE=1 cargo test -p verse-zone-everglade world_tree_snapshot`"
    );
}

#[test]
fn every_doorway_front_station_and_desk_maps_to_a_node() {
    let tree = everglade();
    for (name, outside, _) in layout::doors() {
        let building = tree
            .by_source(&format!("door:{name}"))
            .unwrap_or_else(|| panic!("no node for {name}'s doorway"));
        assert_eq!(building.kind, Kind::Building);
        assert_eq!(building.stand, at(outside), "{name}");
        let door = tree.node(&format!("{}/door", building.id)).unwrap();
        assert_eq!((door.object, door.open), (Some(Object::Door), Some(true)));
    }
    for (name, _) in layout::fronts() {
        let building = tree
            .by_source(&format!("front:{name}"))
            .unwrap_or_else(|| panic!("no node for {name}'s front"));
        let door = tree.node(&format!("{}/door", building.id)).unwrap();
        assert_eq!(door.open, Some(false), "{name}");
    }
    for station in &STATIONS {
        let node = tree
            .by_source(&format!("station:{}", station.id))
            .unwrap_or_else(|| panic!("no node for station {}", station.id));
        assert_eq!(node.stand, at(station.at));
    }
    for i in 0..layout::DESKS.len() {
        let desk = tree.by_source(&format!("desk:{i}")).unwrap();
        assert!(desk.exclusive && desk.offers(Affordance::RunCommands));
    }
    // The places the next phases name.
    let alice = "everglade/knowledge-district/owners-house/great-room/workstation";
    assert!(tree.node(alice).unwrap().exclusive);
    assert!(
        tree.node("everglade/commons/workshop-hall/yard/task-wall")
            .is_some()
    );
    assert!(
        tree.node("everglade/fountain-plaza/agora/trading-floor/desk-18")
            .is_some()
    );
    assert!(tree.with_affordance(Affordance::BuyBread).count() >= 2);
    assert!(tree.with_affordance(Affordance::Sleep).count() >= 20);
    // Every building is in a district the layout names.
    for building in tree.of_kind(Kind::Building) {
        let district = tree.parent(&building.id).unwrap();
        assert_eq!(district.kind, Kind::District);
        assert_eq!(building.district, district.district);
    }
    assert_eq!(
        tree.objects(Object::Lamp).count(),
        estate::LIGHTS.len() + civic::LIGHTS.len() + agora::LIGHTS.len()
    );
}

#[test]
fn every_standing_point_routes_from_the_approach() {
    let tree = everglade();
    let blockers = &world().blockers;
    let approach = tree.root().stand;
    // Nodes that share a standing point, such as a room's lamps, share a
    // route: one route per point, to the first node standing there.
    let mut seen = BTreeSet::new();
    let mut failed = Vec::new();
    let mut routed = 0;
    for node in tree.nodes() {
        let stand = node.stand;
        if !seen.insert(stand.map(f32::to_bits)) {
            continue;
        }
        routed += 1;
        if let Err(e) = perceive::route(&tree, blockers, approach, &node.id) {
            let names: Vec<_> = tree
                .nodes()
                .iter()
                .filter(|n| n.stand == stand)
                .map(|n| n.id.as_str())
                .collect();
            failed.push(format!("{stand:?} ({e:?}): {names:?}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
    assert!(routed > 150, "{routed}");
    // A doorway too narrow for the grid is crossed straight: the direct
    // plan into the Agora fails, and the grounded route passes the door.
    let desk = "everglade/fountain-plaza/agora/trading-floor/desk-1";
    let stand = tree.node(desk).unwrap().stand;
    assert!(nav::plan(approach, stand, blockers, HALF_EXTENT).is_err());
    let agora = tree.node("everglade/fountain-plaza/agora").unwrap();
    let (route, _) = perceive::route(&tree, blockers, approach, desk).unwrap();
    assert!(route.waypoints.contains(&agora.entry.unwrap()[1]));
    // Grounding a node is a route to it and the heading to face.
    let alice = "everglade/knowledge-district/owners-house/great-room/workstation";
    let (route, facing) = perceive::route(&tree, blockers, approach, alice).unwrap();
    assert_eq!(route.destination, tree.node(alice).unwrap().stand);
    assert!(facing.is_some());
    assert!(matches!(
        perceive::route(&tree, blockers, approach, "everglade/nowhere"),
        Err(perceive::GroundError::Unknown(_))
    ));
}

#[test]
fn the_digest_is_stable_and_changes_with_a_layout_table() {
    let tables = Tables::everglade();
    let a = generate(&tables).unwrap();
    assert_eq!(a.digest(), generate(&tables).unwrap().digest());
    assert_eq!(a.digest(), everglade().digest());
    let mut moved = tables.clone();
    moved.doors[0].1[0] += 1.0;
    assert_ne!(generate(&moved).unwrap().digest(), a.digest());
    let mut station = tables.clone();
    station.stations[1].at[1] -= 0.5;
    assert_ne!(generate(&station).unwrap().digest(), a.digest());
    let mut lights = tables.clone();
    lights.lights[0].1.pop();
    assert_ne!(generate(&lights).unwrap().digest(), a.digest());
    let mut fewer = tables;
    fewer.fronts.pop();
    assert_ne!(generate(&fewer).unwrap().digest(), a.digest());
}

#[test]
fn an_agent_that_never_entered_the_great_room_does_not_know_its_objects() {
    let tree = everglade();
    let blockers = &world().blockers;
    let sight = Footprints {
        blocks: blockers,
        tops: &[],
        default_top: 12.0,
        floor: 0.0,
    };
    let house = "everglade/knowledge-district/owners-house";
    let workstation = format!("{house}/great-room/workstation");
    let mut known = Known::new("bram", &tree);
    // At the end of Library Way, facing the house's stair: the house is
    // in sight, its great room's objects are behind its walls.
    let street = layout::estate::WALK.0;
    assert!(perceive::perceive(&mut known, &tree, &sight, street) > 0);
    assert!(known.knows(house), "{:?}", known.nodes);
    assert!(!known.knows(&workstation));
    // Seeing the workshop from the approach path shows it
    // and nothing inside the owner's house.
    let mut fresh = Known::new("ada", &tree);
    let path = [super::super::SPAWN.x, super::super::SPAWN.z];
    perceive::perceive(&mut fresh, &tree, &sight, path);
    assert!(fresh.knows("everglade/commons/workshop-hall"));
    for node in tree.descendants(&format!("{house}/great-room")) {
        assert!(!fresh.knows(&node.id), "{}", node.id);
    }
    // Walking into the great room enters it.
    let inside = tree.node(&format!("{house}/great-room")).unwrap().stand;
    perceive::perceive(&mut known, &tree, &sight, inside);
    assert!(known.knows(&workstation));
    assert!(known.knows(&format!("{house}/great-room/console")));
}

fn seat(name: &str, desk: u32, activity: Activity, station: StudioStation) -> Seat {
    Seat {
        seat: name.into(),
        role: Role::Worker,
        route: "codex:gpt".into(),
        look: "default".into(),
        desk,
        activity,
        station,
        task: None,
        paused: false,
        spend: Default::default(),
    }
}

fn task(id: &str, status: TaskStatus) -> Task {
    Task {
        task: id.into(),
        goal: "g".into(),
        entry: "lead".into(),
        position: 0,
        title: id.into(),
        seat: "ada".into(),
        depends_on: vec![],
        status,
        spend: Default::default(),
    }
}

#[test]
fn object_states_follow_the_clock_and_the_studio() {
    let tree = everglade();
    let view = View {
        seats: vec![
            seat("ada", 1, Activity::Editing, StudioStation::Desk),
            seat("bo", 2, Activity::Idle, StudioStation::Desk),
            seat("cy", 0, Activity::Testing, StudioStation::ProvingGround),
        ],
        tasks: vec![
            task("t1", TaskStatus::Running),
            task("t2", TaskStatus::Queued),
            task("t3", TaskStatus::Held),
            task("t4", TaskStatus::Done),
        ],
        ..View::default()
    };
    let night = conditions::states(&tree, TownTime::at_hour(3, 22.0), Some(&view));
    let noon = conditions::states(&tree, TownTime::at_hour(3, 12.0), None);
    let lamp = tree.objects(Object::Lamp).next().unwrap();
    assert_eq!(night[&lamp.id], State::Lamp { lit: true });
    assert_eq!(noon[&lamp.id], State::Lamp { lit: false });
    let desk = |i: u32| tree.by_source(&format!("desk:{i}")).unwrap().id.clone();
    assert_eq!(night[&desk(1)].describe(), "busy (ada)");
    assert_eq!(night[&desk(2)].describe(), "free");
    let wall = "everglade/commons/workshop-hall/yard/task-wall";
    assert_eq!(
        night[wall].describe(),
        "planned 2, running 1, review 0, done 1, blocked 0"
    );
    let door = tree.by_source("door:bakery:door").unwrap();
    assert_eq!(night[&door.id], State::Door { open: true });
    let closed = tree.by_source("front:the stacks:door").unwrap();
    assert_eq!(night[&closed.id], State::Door { open: false });
    // A hosted authority publishes only what changed.
    let changed = conditions::changed(&noon, &night);
    assert!(changed.iter().any(|(id, _)| *id == lamp.id));
    assert!(changed.iter().any(|(id, _)| *id == desk(1)));
    assert!(!changed.iter().any(|(id, _)| *id == door.id));
    // The publish encoding carries the state the derivation made.
    let object = verse_net::mv::object::Object::new(
        &desk(1),
        tree.digest(),
        night[&desk(1)].clone(),
        [0.0; 3],
        1,
    );
    assert_eq!(object.state, night[&desk(1)]);
}

#[test]
fn a_choice_walks_down_to_a_free_desk_the_agent_knows() {
    let tree = everglade();
    let mut known = Known::new("ada", &tree);
    let hall = "everglade/commons/workshop-hall/hall";
    known.enter(&tree, hall);
    let occupied = BTreeSet::from([format!("{hall}/desk-1")]);
    let mut chooser = world_tree::choose::Scripted::new([
        Some("commons"),
        Some("workshop-hall"),
        Some("hall"),
        Some("desk-2"),
    ]);
    let walk = world_tree::descend(
        &tree,
        Some(&known),
        &mut chooser,
        "ada",
        "run the tests",
        Some(Affordance::RunCommands),
        &occupied,
        ZONE,
    )
    .unwrap();
    assert_eq!(walk.chosen(), Some(format!("{hall}/desk-2").as_str()));
    // Only the commons was known and served commands at the top.
    assert_eq!(chooser.asked[0], ["commons"]);
    assert!(!chooser.asked[3].contains(&"desk-1".to_owned()));
}
