//! The rooms and objects inside the places agents work: the workshop
//! hall with the Agent Studio's stations and desks, the owner's great room,
//! the Civic Hall's council chamber, and the Agora; and every lit
//! building's fixtures as lamps; and the Pylon Field's Wellspring and
//! pylon sites.

use std::collections::BTreeMap;

use world_tree::{Affordance, Object, slug};

use super::super::layout::estate::{AliceSpot, Fixture, OWNERS_HOUSE};
use super::super::layout::generated::Instance;
use super::super::layout::pylon_field::Field;
use super::super::layout::{self, agora, civic, estate};
use super::super::{HALL, STRONGROOM, YARD};
use super::{Out, Tables, area, cm, heading, rect};

/// A fixture's name in the tree.
fn fixture_name(fixture: Fixture) -> &'static str {
    match fixture {
        Fixture::Candles => "candles",
        Fixture::Sconce => "sconce",
        Fixture::Lamp => "lamp",
        Fixture::Brazier => "brazier",
        Fixture::Lantern => "lantern",
        Fixture::Uplight => "uplight",
    }
}

/// The Agent Studio's object for a station: its kind, what it offers, and
/// which room holds it.
fn station_object(id: &str) -> (Object, &'static [Affordance], &'static str) {
    use Affordance::*;
    match id {
        "desks" => (Object::Station, &[Work], "hall"),
        "library" => (Object::Station, &[Read], "hall"),
        "oracle" => (Object::Station, &[Review], "hall"),
        "merge" => (Object::Station, &[Review, Approve], "strongroom"),
        "task_wall" => (Object::TaskWall, &[Read, Plan], "yard"),
        "proving" => (Object::Station, &[RunCommands], "yard"),
        "podium" => (Object::Lectern, &[Approve], "yard"),
        "lounge" => (Object::Seats, &[Rest], ""),
        "workbench" => (Object::Station, &[Work, RunCommands], ""),
        _ => (Object::Station, &[], ""),
    }
}

/// The workshop hall's rooms: the hall with its desks, the strongroom,
/// and the yard with the Task Wall and the goal board; the stations out in
/// the grounds stand under the building.
pub(super) fn workshop(out: &mut Out, tables: &Tables, id: &str, d: &str) -> Result<(), String> {
    let station = |sid: &str| {
        tables
            .stations
            .iter()
            .find(|s| s.id == sid)
            .ok_or_else(|| format!("no station {sid}"))
    };
    let hall = out.room(
        id,
        d,
        "hall",
        station("desks")?.at,
        rect(HALL),
        "room:workshop hall".into(),
    );
    let strongroom = out.room(
        id,
        d,
        "strongroom",
        station("merge")?.at,
        rect(STRONGROOM),
        "room:strongroom".into(),
    );
    let yard = out.room(
        id,
        d,
        "yard",
        YARD.0,
        rect(YARD),
        "room:workshop yard".into(),
    );
    for s in &tables.stations {
        let (object, offers, room) = station_object(s.id);
        let parent = match room {
            "hall" => &hall,
            "strongroom" => &strongroom,
            "yard" => &yard,
            _ => id,
        };
        let name = if s.id == "approach" {
            "approach".to_owned()
        } else {
            s.studio.to_lowercase()
        };
        out.object(
            parent,
            d,
            &name,
            object,
            (s.at, Some(s.facing)),
            offers,
            false,
            format!("station:{}", s.id),
        );
        if s.id == "desks" {
            for (i, seat) in tables.desk_seats.iter().enumerate() {
                out.object(
                    &hall,
                    d,
                    &format!("desk {}", i + 1),
                    Object::Workstation,
                    (*seat, Some(0.0)),
                    &[Affordance::Work, Affordance::RunCommands],
                    true,
                    format!("desk:{i}"),
                );
            }
        }
    }
    // The goal board in the atrium, read from a step south of it.
    let board = layout::GOAL_BOARD.center;
    out.object(
        &yard,
        d,
        "goal board",
        Object::Board,
        ([board.x, board.z - 1.1], Some(0.0)),
        &[Affordance::Read, Affordance::Plan],
        false,
        "board:goal".into(),
    );
    Ok(())
}

/// How far a standing point keeps from a building's own blocks, m:
/// navigation's clearance (the walker's radius and a little) and a margin,
/// so a point a layout table puts exactly at the walker's clearance is
/// still a destination navigation accepts.
const CLEAR: f32 = crate::controller::RADIUS + 0.1;

/// A spot in `instance`'s frame, where it stands and the point it faces,
/// in the world, stepped back from what it faces until it is clear of the
/// instance's blocks (at most 0.5 m).
fn spot(instance: &Instance, (local, toward): ([f32; 2], [f32; 2])) -> ([f32; 2], Option<f32>) {
    let blocks = instance.blocks();
    let target = instance.world(toward);
    let mut from = instance.world(local);
    let (dx, dz) = (from[0] - target[0], from[1] - target[1]);
    let length = dx.hypot(dz).max(1e-3);
    for _ in 0..10 {
        if !blocks
            .iter()
            .any(|(f, _)| f.contains(from[0], from[1], CLEAR))
        {
            break;
        }
        from = [from[0] + dx / length * 0.05, from[1] + dz / length * 0.05];
    }
    (from, Some(heading(from, target)))
}

/// The owner's great room, with Alice's three spots.
pub(super) fn owners_house(out: &mut Out, id: &str, d: &str) {
    let ([x0, x1], [z0, z1], _) = estate::ROOM;
    let middle = OWNERS_HOUSE.world(estate::GRECO_HOUSE.inside.unwrap_or([0.0, -18.0]));
    let room = out.room(
        id,
        d,
        "great room",
        middle,
        area(&OWNERS_HOUSE, [x0, x1], [z0, z1]),
        "room:great room".into(),
    );
    use Affordance::*;
    let spots: [(&str, AliceSpot, Object, &[Affordance]); 3] = [
        (
            "workstation",
            AliceSpot::Desk,
            Object::Workstation,
            &[Work, RunCommands],
        ),
        (
            "console",
            AliceSpot::Workbench,
            Object::Console,
            &[RunCommands],
        ),
        (
            "lectern",
            AliceSpot::Podium,
            Object::Lectern,
            &[Approve, Review],
        ),
    ];
    for (name, place, object, offers) in spots {
        out.object(
            &room,
            d,
            name,
            object,
            spot(&OWNERS_HOUSE, place.local()),
            offers,
            true,
            format!("estate:{name}"),
        );
    }
}

/// The Civic Hall's council chamber: its benches round the well, and the
/// speaker's place before the lectern on the dais, facing the well.
pub(super) fn civic_hall(out: &mut Out, id: &str, d: &str) {
    let ([x0, x1], [z0, z1], _) = civic::CHAMBER;
    let middle = civic::CIVIC.world(civic::MIDDLE);
    let room = out.room(
        id,
        d,
        "council chamber",
        middle,
        area(&civic::CIVIC, [x0, x1], [z0, z1]),
        "room:council chamber".into(),
    );
    out.object(
        &room,
        d,
        "council benches",
        Object::Seats,
        (middle, None),
        &[Affordance::Gather],
        false,
        "civic:benches".into(),
    );
    out.object(
        &room,
        d,
        "speaker's lectern",
        Object::Lectern,
        spot(&civic::CIVIC, ([0.0, -23.9], civic::MIDDLE)),
        &[Affordance::Gather, Affordance::Teach],
        true,
        "civic:lectern".into(),
    );
}

/// The doorways from the Agora's trading floor into Paul's office and the
/// training room, in the hall's frame: a point on the floor and one inside.
const WING_DOORS: [[[f32; 2]; 2]; 2] =
    [[[-7.0, -11.5], [-9.4, -11.5]], [[7.0, -11.5], [9.4, -11.5]]];

/// The Agora's trading floor, Paul's office, and training room, with the
/// places [`agora`] keeps for an agent. Each booth has its own node in
/// the clear side aisle; its two lecterns remain written-practice props.
pub(super) fn agora_hall(out: &mut Out, id: &str, d: &str) {
    use Affordance::*;
    let hall = &agora::AGORA;
    let rooms = ["trading floor", "Paul's office", "training room"];
    let stands = [
        hall.world(agora::MIDDLE),
        spot(hall, agora::PAUL).0,
        spot(hall, agora::TEACHER).0,
    ];
    let ids: Vec<String> = rooms
        .iter()
        .zip(agora::ROOMS)
        .zip(stands)
        .enumerate()
        .map(|(i, ((name, (x, z, _)), stand))| {
            let id = out.room(id, d, name, stand, area(hall, x, z), format!("room:{name}"));
            if i > 0 {
                let [outside, inside] = WING_DOORS[i - 1];
                let entry = [hall.world(outside), hall.world(inside)];
                if let Some(room) = out.nodes.last_mut() {
                    room.entry = Some(entry.map(|p| [cm(p[0]), cm(p[1])]));
                }
            }
            id
        })
        .collect();
    let place = |s: agora::Station| spot(hall, s);
    for (index, room) in ids.iter().enumerate().skip(1) {
        let [outside, inside] = WING_DOORS[index - 1];
        out.object(
            room,
            d,
            "door",
            Object::Door,
            place((inside, outside)),
            &[],
            false,
            format!("agora:wing-door:{index}"),
        );
        if let Some(door) = out.nodes.last_mut() {
            door.open = Some(true);
        }
    }
    for (i, desk) in agora::DESKS.into_iter().enumerate() {
        out.object(
            &ids[0],
            d,
            &format!("desk {}", i + 1),
            Object::Workstation,
            place(desk),
            &[Work, Sell],
            true,
            format!("agora:desk:{i}"),
        );
    }
    out.object(
        &ids[0],
        d,
        "stand-up spot",
        Object::Seats,
        place(agora::STANDUP),
        &[Gather, Plan],
        false,
        "agora:standup".into(),
    );
    out.object(
        &ids[0],
        d,
        "bell",
        Object::Station,
        place((agora::STANDUP.0, agora::BELL_PIVOT.0)),
        &[Read],
        false,
        "agora:bell".into(),
    );
    out.object(
        &ids[0],
        d,
        "leaderboard",
        Object::Board,
        place((agora::MIDDLE, agora::LEADERBOARD.0)),
        &[Read],
        false,
        "agora:leaderboard".into(),
    );
    out.object(
        &ids[1],
        d,
        "Paul's desk",
        Object::Workstation,
        place(agora::PAUL),
        &[Work, Review],
        true,
        "agora:paul".into(),
    );
    out.object(
        &ids[1],
        d,
        "owner's lectern",
        Object::Lectern,
        place(agora::OWNER),
        &[Approve],
        true,
        "agora:owner".into(),
    );
    out.object(
        &ids[2],
        d,
        "whiteboard",
        Object::Board,
        place(agora::TEACHER),
        &[Teach],
        true,
        "agora:teacher".into(),
    );
    out.object(
        &ids[2],
        d,
        "role-play booths",
        Object::Seats,
        place((WING_DOORS[1][1], agora::BOOTHS[0][0].0)),
        &[Practice],
        false,
        "agora:booths".into(),
    );
    // Preserve the group's existing ID for retained plans. Individual
    // destinations stand beside the lecterns, outside their collision boxes.
    for (index, booth) in agora::BOOTHS.iter().enumerate() {
        let x = if index % 2 == 0 { 9.0 } else { 14.2 };
        let z = (booth[0].0[1] + booth[1].0[1]) * 0.5;
        out.object(
            &ids[2],
            d,
            &format!("role-play booth {}", index + 1),
            Object::Seats,
            place(([x, z], [booth[0].0[0], z])),
            &[Practice],
            false,
            format!("agora:booth:{index}"),
        );
    }
}

/// The building's light fixtures as lamps: those indoors in the room that
/// holds them, the rest on the building. A lamp's standing point is its
/// parent's, since nobody walks to a candle.
pub(super) fn lamps(out: &mut Out, tables: &Tables, name: &str, id: &str, d: &str) {
    let Some((instance, lights)) = tables.lights.iter().find(|(i, _)| i.name == name) else {
        return;
    };
    let rooms: Vec<(String, [f32; 2], [f32; 2])> = match name {
        "owner's house" => {
            let (x, z, _) = estate::ROOM;
            vec![(format!("{id}/great-room"), x, z)]
        }
        "civic hall" => {
            let (x, z, _) = civic::CHAMBER;
            vec![(format!("{id}/council-chamber"), x, z)]
        }
        "agora" => ["trading-floor", "pauls-office", "training-room"]
            .into_iter()
            .zip(agora::ROOMS)
            .map(|(room, (x, z, _))| (format!("{id}/{room}"), x, z))
            .collect(),
        _ => vec![],
    };
    let source = slug(name);
    let mut counts: BTreeMap<(String, &str), u32> = BTreeMap::new();
    for (i, &(fixture, [x, _, z])) in lights.iter().enumerate() {
        let parent = rooms
            .iter()
            .find(|(_, rx, rz)| {
                fixture.indoors() && (rx[0]..=rx[1]).contains(&x) && (rz[0]..=rz[1]).contains(&z)
            })
            .map_or(id.to_owned(), |r| r.0.clone());
        let kind = fixture_name(fixture);
        let n = counts.entry((parent.clone(), kind)).or_default();
        *n += 1;
        let stand = out
            .nodes
            .iter()
            .find(|node| node.id == parent)
            .map_or(instance.world([x, z]), |node| node.stand);
        out.object(
            &parent,
            d,
            &format!("{kind} {n}"),
            Object::Lamp,
            (stand, None),
            &[],
            false,
            format!("light:{source}:{i}"),
        );
    }
}

/// The Pylon Field's objects: the Wellspring, and one pylon per site,
/// numbered from the southernmost. Their states come from the compute
/// source; an empty site has none.
pub(super) fn pylon_field(out: &mut Out, field: &Field, id: &str, d: &str) {
    let (stand, facing) = field.basin_stand();
    out.object(
        id,
        d,
        "wellspring",
        Object::Wellspring,
        (stand, Some(facing)),
        &[],
        false,
        super::super::compute::WELLSPRING_SOURCE.into(),
    );
    for i in 0..field.sites.len() {
        let Some((stand, facing)) = field.site_stand(i) else {
            continue;
        };
        out.object(
            id,
            d,
            &format!("pylon {}", i + 1),
            Object::Pylon,
            (stand, Some(facing)),
            &[],
            false,
            super::super::compute::site_source(i),
        );
    }
}
