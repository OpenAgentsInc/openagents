//! The checks, the edits and the description, on version 1 and on broken
//! copies of it.

use haunt_assets::catalog;

use crate::check::{check, problems};
use crate::describe::describe;
use crate::doc::{Action, AreaKind, Door, House, Intent, OUTSIDE, Prop, Trigger, Wall, When, v1};
use crate::edit::{NewArea, Placement, Spot, add_area, free_walls, place, remove_area};

fn refused(house: &House) -> (String, String) {
    let r = check(house, catalog()).expect_err("refused");
    (r.code.to_owned(), r.message)
}

fn one_sentence(message: &str) {
    assert!(message.ends_with('.'), "{message}");
    assert_eq!(message.matches(". ").count(), 0, "one sentence: {message}");
    assert!(
        oa_copy::violations(message, &[]).is_empty(),
        "{message}: {:?}",
        oa_copy::violations(message, &[])
    );
}

fn intent(feeling: &str) -> Intent {
    Intent {
        feeling: feeling.into(),
        beat: String::new(),
        intensity: [2, 3],
    }
}

#[test]
fn version_one_passes_every_check() {
    let house = v1();
    assert_eq!(problems(&house, catalog()), vec![]);
    let ids: Vec<&str> = house.areas.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "yard",
            "entry-hall",
            "parlor",
            "long-hall",
            "kitchen",
            "back-hall"
        ]
    );
    // The spec's sizes: w x d x ceiling.
    let size = |id: &str| {
        let a = house.area(id).unwrap();
        (a.width(), a.depth(), a.ceiling)
    };
    assert_eq!(size("yard"), (16.0, 20.0, None));
    assert_eq!(size("entry-hall"), (3.0, 12.0, Some(3.2)));
    assert_eq!(size("parlor"), (8.0, 8.0, Some(3.6)));
    assert_eq!(size("long-hall"), (2.4, 18.0, Some(3.0)));
    assert_eq!(size("kitchen"), (7.0, 9.0, Some(3.4)));
    assert_eq!(size("back-hall"), (3.0, 10.0, Some(3.2)));
    // You arrive facing south, away from the house, with the creak behind.
    assert_eq!(house.arrival.yaw, 180.0);
    assert_eq!(house.arrival.cue, "creak-behind");
    let door = house
        .area("yard")
        .unwrap()
        .door_centre(&house.area("yard").unwrap().doors[0]);
    let at = house.arrival_point().unwrap();
    assert!(door[1] > at[1] + 10.0, "the house is behind you");
    // It round-trips.
    assert_eq!(House::parse(&house.to_json()).unwrap(), house);
}

#[test]
fn overlapping_areas_are_refused() {
    let mut house = v1();
    house.area_mut("kitchen").unwrap().rect[0] = 1.0;
    let (code, message) = refused(&house);
    assert_eq!(code, "overlap");
    assert_eq!(message, "The kitchen overlaps the back hall.");
    one_sentence(&message);
}

#[test]
fn a_door_must_be_on_a_shared_wall_and_listed_on_both_sides() {
    let mut house = v1();
    house.area_mut("parlor").unwrap().doors[1].wall = Wall::East;
    let (code, message) = refused(&house);
    assert_eq!(code, "door");
    one_sentence(&message);

    let mut house = v1();
    house
        .area_mut("long-hall")
        .unwrap()
        .doors
        .retain(|d| d.id != "hall-south");
    let (code, message) = refused(&house);
    assert_eq!(code, "door");
    assert_eq!(
        message,
        "The long hall must list the door parlor-north too, at the same place on its south wall."
    );

    let mut house = v1();
    house.area_mut("parlor").unwrap().doors[1].offset = 3.0;
    assert_eq!(refused(&house).0, "door");

    let mut house = v1();
    house.area_mut("parlor").unwrap().doors[1].width = 0.6;
    assert_eq!(refused(&house).0, "door");
}

#[test]
fn every_area_and_the_way_out_must_be_reachable() {
    let mut house = v1();
    // Brick up the kitchen's door to the back hall on both sides.
    house
        .area_mut("kitchen")
        .unwrap()
        .doors
        .retain(|d| d.to != "back-hall");
    house
        .area_mut("back-hall")
        .unwrap()
        .doors
        .retain(|d| d.to != "kitchen");
    let (code, message) = refused(&house);
    assert_eq!(code, "unreachable");
    assert_eq!(
        message,
        "You can't get to the back hall from where you arrive."
    );
    one_sentence(&message);

    let mut house = v1();
    house.exit.door = "kitchen-east".into();
    assert_eq!(refused(&house).0, "exit");
}

#[test]
fn a_prop_in_the_path_is_refused_and_named() {
    let mut house = v1();
    house.area_mut("long-hall").unwrap().props.push(Prop {
        id: "coffin-1".into(),
        asset: "coffin".into(),
        at: [1.2, 9.0],
        yaw: 0.0,
        y: 0.0,
        state: None,
        note: String::new(),
        text: None,
    });
    let (code, message) = refused(&house);
    assert_eq!(code, "path");
    assert!(
        message.starts_with("coffin-1 blocks the way to the door"),
        "{message}"
    );
    one_sentence(&message);
}

#[test]
fn props_must_fit_inside_on_their_surface_and_under_the_ceiling() {
    let prop = |asset: &str, at: [f32; 2], y: f32| Prop {
        id: "x-1".into(),
        asset: asset.into(),
        at,
        yaw: 0.0,
        y,
        state: None,
        note: String::new(),
        text: None,
    };
    for (area, p, code) in [
        // Outside the parlor's walls.
        ("parlor", prop("armchair", [7.9, 4.0], 0.0), "place"),
        // A portrait in the middle of the room, on no wall.
        ("parlor", prop("portrait", [4.0, 4.0], 1.4), "place"),
        // A clock too tall for the long hall at that height.
        (
            "long-hall",
            prop("grandfather-clock", [0.4, 2.0], 1.0),
            "place",
        ),
        // Chains need a ceiling; the yard has none.
        ("yard", prop("chains", [3.0, 3.0], 1.0), "place"),
        // The moon goes in the sky.
        ("parlor", prop("moon", [4.0, 4.0], 0.0), "place"),
        // Not in the library.
        ("parlor", prop("dragon", [4.0, 4.0], 0.0), "library"),
    ] {
        let mut house = v1();
        house.area_mut(area).unwrap().props.push(p.clone());
        let (got, message) = refused(&house);
        assert_eq!(got, code, "{} in {area}: {message}", p.asset);
        one_sentence(&message);
    }
    // A state the asset doesn't have.
    let mut house = v1();
    house.area_mut("parlor").unwrap().props[0].state = Some("flying".into());
    let (code, message) = refused(&house);
    assert_eq!(code, "library");
    assert_eq!(message, "An armchair can be still, not flying.");
}

#[test]
fn library_ids_must_exist() {
    let cases: [(&str, fn(&mut House)); 7] = [
        ("surface", |h| h.areas[1].surfaces.walls = "gingham".into()),
        ("preset", |h| h.areas[1].light.preset = "disco".into()),
        ("bed", |h| h.areas[1].sound.bed = Some("howl-wolf".into())),
        ("music", |h| h.audio.music_track = "wind".into()),
        ("cue", |h| h.arrival.cue = "boom".into()),
        ("sky", |h| h.look.sky = "plaid".into()),
        ("sfx", |h| {
            h.areas[2].triggers[0].actions[1] = Action::Sfx("kazoo".into());
        }),
    ];
    for (what, break_it) in cases {
        let mut house = v1();
        break_it(&mut house);
        let (code, message) = refused(&house);
        assert_eq!(code, "library", "{what}: {message}");
        one_sentence(&message);
    }
}

#[test]
fn budgets_are_kept() {
    let mut house = v1();
    let light = house.areas[2].light.lights[0].clone();
    house.areas[2].light.lights = vec![light; 9];
    let (code, message) = refused(&house);
    assert_eq!(code, "budget");
    assert_eq!(
        message,
        "The parlor has 9 lights; an area can have at most 8."
    );

    let mut house = v1();
    for _ in 0..4 {
        house.areas[3].triggers.push(Trigger {
            when: When::Enter(true),
            actions: vec![Action::Sfx("thunder".into())],
            every_visit: true,
        });
    }
    let (code, message) = refused(&house);
    assert_eq!(code, "budget");
    one_sentence(&message);

    let mut house = v1();
    let spare = house.areas[3].clone();
    for i in 0..7 {
        let mut a = spare.clone();
        a.id = format!("spare-{i}");
        a.rect[0] = 100.0 + 10.0 * i as f32;
        a.doors.clear();
        a.triggers.clear();
        a.props.clear();
        house.areas.push(a);
    }
    let (code, message) = refused(&house);
    assert_eq!(code, "budget");
    assert_eq!(message, "The house has 13 areas; it can have at most 12.");
}

#[test]
fn every_area_needs_an_intent() {
    let mut house = v1();
    house.areas[4].intent.feeling = " ".into();
    let (code, message) = refused(&house);
    assert_eq!(code, "intent");
    assert_eq!(
        message,
        "The kitchen needs an intent: the feeling it should give, and an intensity from 0 to 5."
    );
    let mut house = v1();
    house.areas[4].intent.intensity = [2, 9];
    assert_eq!(refused(&house).0, "intent");
}

#[test]
fn a_fourth_hallway_and_a_third_room_pass_the_checks() {
    let house = v1();
    let (house, hall) = add_area(
        &house,
        catalog(),
        "parlor",
        Wall::West,
        &NewArea {
            id: None,
            kind: AreaKind::Hallway,
            size: [8.0, 2.4],
            intent: intent("dread: a corridor that seems longer than it is"),
            ceiling: None,
        },
    )
    .unwrap();
    assert_eq!(hall, "hallway-2");
    assert_eq!(problems(&house, catalog()), vec![]);
    let (house, room) = add_area(
        &house,
        catalog(),
        &hall,
        Wall::West,
        &NewArea {
            id: Some("library".into()),
            kind: AreaKind::Room,
            size: [6.0, 6.0],
            intent: intent("curiosity, then a book falls"),
            ceiling: Some(3.6),
        },
    )
    .unwrap();
    assert_eq!(room, "library");
    assert_eq!(problems(&house, catalog()), vec![]);
    assert_eq!(house.areas.len(), 8);
    // Both are walkable: the new doors line up and a walker gets through.
    let walls = crate::walk::Walls::of(&house, catalog());
    // From just inside the parlor's new door, walking west.
    let parlor = house.area("parlor").unwrap();
    let door = parlor.doors.iter().find(|d| d.to == hall).unwrap();
    let mut p = parlor.door_inside(door, 0.6);
    for _ in 0..200 {
        p = walls.slide(p, [-0.1, 0.0], crate::walk::RADIUS);
    }
    assert_eq!(
        crate::walk::area_at(&house, p).map(|a| a.id.as_str()),
        Some("library"),
        "{p:?}"
    );
    // The new areas show in the description.
    let text = describe(&house, catalog());
    assert!(text.contains("[G] hallway-2 (hallway): 8 x 2.4 m"));
    assert!(text.contains("[H] library (room): 6 x 6 m"));
}

#[test]
fn add_area_refuses_with_one_sentence() {
    let house = v1();
    let new = NewArea {
        id: None,
        kind: AreaKind::Room,
        size: [6.0, 6.0],
        intent: intent("x"),
        ceiling: None,
    };
    for (next_to, wall, code) in [
        ("yard", Wall::East, "yard"),
        ("attic", Wall::North, "unknown_area"),
        // The long hall's east side has 0.6 m before the back hall.
        ("kitchen", Wall::North, "ok"),
    ] {
        match add_area(&house, catalog(), next_to, wall, &new) {
            Ok((after, _)) => {
                assert_eq!(code, "ok");
                assert!(check(&after, catalog()).is_ok());
            }
            Err(r) => {
                assert_eq!(r.code, code);
                one_sentence(&r.message);
            }
        }
    }
    // Too big for the gap between the long hall and the back hall's side.
    let narrow = NewArea {
        size: [0.6, 4.0],
        ..new.clone()
    };
    assert_eq!(
        add_area(&house, catalog(), "long-hall", Wall::East, &narrow)
            .unwrap_err()
            .code,
        "size"
    );
}

#[test]
fn remove_area_refuses_to_cut_off_the_way_out() {
    let house = v1();
    let r = remove_area(&house, "long-hall").unwrap_err();
    assert_eq!(r.code, "cuts_off_exit");
    assert_eq!(
        r.message,
        "Removing the long hall would cut off the way out."
    );
    assert_eq!(remove_area(&house, "back-hall").unwrap_err().code, "exit");
    assert_eq!(remove_area(&house, "yard").unwrap_err().code, "arrival");
    // A dead-end room comes out cleanly with its door.
    let (with_room, id) = add_area(
        &house,
        catalog(),
        "parlor",
        Wall::West,
        &NewArea {
            id: Some("study".into()),
            kind: AreaKind::Room,
            size: [5.0, 6.0],
            intent: intent("quiet"),
            ceiling: None,
        },
    )
    .unwrap();
    let removed = remove_area(&with_room, &id).unwrap();
    assert_eq!(removed, house);
}

#[test]
fn placing_a_werewolf_in_the_long_hall_brings_its_howl() {
    let house = v1();
    let (after, id) = place(
        &house,
        catalog(),
        &Placement {
            area: "long-hall".into(),
            asset: "wolf man".into(),
            spot: Spot::Middle,
            yaw: None,
            id: None,
            state: None,
            note: "howls when you get near".into(),
            with_trigger: true,
        },
    )
    .unwrap();
    assert_eq!(id, "werewolf-1");
    assert!(check(&after, catalog()).is_ok());
    let hall = after.area("long-hall").unwrap();
    let prop = hall.props.iter().find(|p| p.id == id).unwrap();
    assert_eq!(prop.asset, "werewolf");
    let trigger = hall.triggers.last().unwrap();
    assert!(matches!(&trigger.when, When::Near { prop, .. } if prop == &id));
    assert!(trigger.actions.contains(&Action::Sfx("howl-wolf".into())));
    // A dragon isn't in the library.
    let r = place(
        &house,
        catalog(),
        &Placement {
            area: "long-hall".into(),
            asset: "dragon".into(),
            spot: Spot::Middle,
            yaw: None,
            id: None,
            state: None,
            note: String::new(),
            with_trigger: true,
        },
    )
    .unwrap_err();
    assert_eq!(r.code, "library");
}

#[test]
fn free_walls_skip_shared_walls_and_the_yard() {
    let house = v1();
    assert!(free_walls(&house, house.area("yard").unwrap()).is_empty());
    let parlor = free_walls(&house, house.area("parlor").unwrap());
    // North: the long hall takes 4 to 6.4 of 3 to 11.
    let north: Vec<_> = parlor.iter().filter(|f| f.wall == Wall::North).collect();
    assert_eq!(north.len(), 1, "{north:?}");
    assert_eq!((north[0].from, north[0].to), (6.4, 11.0));
    assert!(
        parlor
            .iter()
            .any(|f| f.wall == Wall::West && f.reach == 20.0)
    );
}

#[test]
fn an_outside_door_must_lead_outside() {
    let mut house = v1();
    house.area_mut("kitchen").unwrap().doors.push(Door {
        id: "kitchen-out".into(),
        to: OUTSIDE.into(),
        wall: Wall::East,
        offset: 3.0,
        width: 1.4,
    });
    let (code, message) = refused(&house);
    assert_eq!(code, "door");
    one_sentence(&message);
}

#[test]
fn describe_matches_the_golden_file() {
    let text = describe(&v1(), catalog());
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/describe-v1.txt");
    if std::env::var_os("HAUNT_GOLDEN").is_some() {
        std::fs::write(path, &text).unwrap();
    }
    let golden = std::fs::read_to_string(path).unwrap_or_default();
    assert_eq!(text, golden, "set HAUNT_GOLDEN=1 to rewrite {path}");
    // Every area with its size, position, facing, contents and intent.
    for area in &v1().areas {
        assert!(text.contains(&format!("] {} (", area.id)), "{}", area.id);
        for p in &area.props {
            assert!(text.contains(&format!("- {} (", p.id)), "{}", p.id);
        }
        assert!(text.contains(&area.intent.feeling));
    }
    assert!(text.contains("you arrive facing south"));
    let _ = (Prop::clone, AreaKind::Room);
}
