//! Edits that keep a house whole: adding or removing an area, placing a
//! library asset, and the free walls an area could grow from. Each returns
//! a new document; the caller runs [`crate::check`] before saving it.

use haunt_assets::{Catalog, Surface};

use crate::check::{Refusal, cap, footprint, reachable, rects_overlap};
use crate::doc::{
    Action, Area, AreaKind, AreaSound, Door, House, Intent, Light, OUTSIDE, Prop, StateChange,
    Surfaces, Trigger, Wall, When, name,
};

/// How far a new area may reach from the wall it grows from, m.
pub const MAX_REACH: f32 = 20.0;
/// The door [`add_area`] cuts, m.
pub const DOOR_WIDTH: f32 = 1.4;

/// A stretch of wall nothing is built against, and the largest area it
/// could take.
#[derive(Clone, Debug, PartialEq)]
pub struct FreeWall {
    pub wall: Wall,
    /// Where it starts and ends along the wall, world m.
    pub from: f32,
    pub to: f32,
    /// How far out an area as long as the stretch could reach, m.
    pub reach: f32,
}

impl FreeWall {
    #[must_use]
    pub fn length(&self) -> f32 {
        self.to - self.from
    }
}

fn subtract(spans: &mut Vec<(f32, f32)>, cut: (f32, f32)) {
    let mut out = Vec::new();
    for (a, b) in spans.drain(..) {
        if cut.1 <= a || cut.0 >= b {
            out.push((a, b));
            continue;
        }
        if cut.0 > a {
            out.push((a, cut.0));
        }
        if cut.1 < b {
            out.push((cut.1, b));
        }
    }
    *spans = out;
}

/// A rectangle against `wall` of `area`, spanning `from..to` along it and
/// reaching `depth` out.
fn rect_out(area: &Area, wall: Wall, from: f32, to: f32, depth: f32) -> [f32; 4] {
    let (fixed, _, _) = area.wall_line(wall);
    match wall {
        Wall::North => [from, fixed, to - from, depth],
        Wall::South => [from, fixed - depth, to - from, depth],
        Wall::East => [fixed, from, depth, to - from],
        Wall::West => [fixed - depth, from, depth, to - from],
    }
}

/// How far out from `wall` a rectangle over `from..to` can reach before it
/// meets another area, up to [`MAX_REACH`].
fn reach(house: &House, area: &Area, wall: Wall, from: f32, to: f32) -> f32 {
    let mut lo = 0.0_f32;
    let mut hi = MAX_REACH;
    if house
        .areas
        .iter()
        .all(|b| !rects_overlap(rect_out(area, wall, from, to, hi), b.rect))
    {
        return hi;
    }
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        let clear = house
            .areas
            .iter()
            .all(|b| !rects_overlap(rect_out(area, wall, from, to, mid), b.rect));
        if clear {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo * 10.0).floor() / 10.0
}

/// The stretches of `area`'s walls with nothing built against them, each
/// at least a door wide, with how far an area could reach from each. The
/// yard has none: it is outdoors.
#[must_use]
pub fn free_walls(house: &House, area: &Area) -> Vec<FreeWall> {
    if area.kind == AreaKind::Yard {
        return Vec::new();
    }
    let mut out = Vec::new();
    for wall in Wall::ALL {
        let (_, start, end) = area.wall_line(wall);
        let mut spans = vec![(start, end)];
        for other in house.areas.iter().filter(|b| b.id != area.id) {
            if let Some(shared) = crate::check::shared(area, wall, other) {
                subtract(&mut spans, shared);
            }
        }
        // The way out keeps its doorstep clear.
        for door in area
            .doors
            .iter()
            .filter(|d| d.wall == wall && d.to == OUTSIDE)
        {
            let (a, b) = area.door_span(door);
            subtract(&mut spans, (a - 0.5, b + 0.5));
        }
        for (from, to) in spans {
            if to - from < DOOR_WIDTH + 0.2 {
                continue;
            }
            let reach = reach(house, area, wall, from, to);
            if reach >= 1.5 {
                out.push(FreeWall {
                    wall,
                    from,
                    to,
                    reach,
                });
            }
        }
    }
    out
}

fn defaults(kind: AreaKind) -> (Surfaces, f32, Light) {
    match kind {
        AreaKind::Hallway => (
            Surfaces {
                floor: "planks".into(),
                walls: "wallpaper-plum".into(),
                trim: Some("wood".into()),
            },
            3.2,
            Light {
                preset: "lantern-only".into(),
                dim: 0.0,
                ambient: None,
                lights: Vec::new(),
            },
        ),
        _ => (
            Surfaces {
                floor: "planks".into(),
                walls: "wallpaper-stripe".into(),
                trim: Some("wood".into()),
            },
            3.4,
            Light {
                preset: "candle".into(),
                dim: 0.0,
                ambient: None,
                lights: Vec::new(),
            },
        ),
    }
}

/// What [`add_area`] makes.
#[derive(Clone, Debug, PartialEq)]
pub struct NewArea {
    /// The new area's id (taken from the request, or made up).
    pub id: Option<String>,
    pub kind: AreaKind,
    /// Width (x) and depth (z), m.
    pub size: [f32; 2],
    pub intent: Intent,
    pub ceiling: Option<f32>,
}

fn fresh_id(house: &House, kind: AreaKind) -> String {
    let base = kind.word();
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| house.area(id).is_none())
        .unwrap_or_else(|| base.into())
}

/// Adds an area against `wall` of `next_to`, with a door through the wall
/// they share, listed on both sides. The new area lines up with the middle
/// of the wall's free stretch where it can, and slides along it otherwise.
///
/// # Errors
/// The area or wall doesn't exist, the area is the yard, the size is out
/// of range, or there's no room; one sentence says which.
pub fn add_area(
    house: &House,
    catalog: &Catalog,
    next_to: &str,
    wall: Wall,
    new: &NewArea,
) -> Result<(House, String), Refusal> {
    let Some(base) = house.area(next_to) else {
        return Err(Refusal::new(
            "unknown_area",
            format!("There's no area called {next_to}."),
        ));
    };
    if base.kind == AreaKind::Yard || new.kind == AreaKind::Yard {
        return Err(Refusal::new(
            "yard",
            "The yard is outdoors; add hallways and rooms to the house instead.",
        ));
    }
    let [w, d] = new.size;
    if !(1.5..=20.0).contains(&w) || !(1.5..=20.0).contains(&d) {
        return Err(Refusal::new(
            "size",
            "A new area must be 1.5 to 20 m on each side.",
        ));
    }
    let id = new.id.clone().unwrap_or_else(|| fresh_id(house, new.kind));
    if house.area(&id).is_some() || id == OUTSIDE {
        return Err(Refusal::new(
            "id",
            format!("There's already an area called {id}."),
        ));
    }
    // Along the wall, the new area is as long as its size on that axis,
    // and reaches out by the other.
    let (along, out) = if wall.along_x() { (w, d) } else { (d, w) };
    let free = free_walls(house, base);
    let stretches: Vec<&FreeWall> = free.iter().filter(|f| f.wall == wall).collect();
    if stretches.is_empty() {
        return Err(Refusal::new(
            "no_room",
            format!(
                "{} has no free stretch of {} wall to build against.",
                cap(&name(next_to)),
                wall.word()
            ),
        ));
    }
    let (_, wall_start, wall_end) = base.wall_line(wall);
    let middle = (wall_start + wall_end) / 2.0;
    // Every place along the free stretches it fits, nearest the wall's
    // middle first.
    let mut fits: Vec<([f32; 4], f32, (f32, f32))> = Vec::new();
    for stretch in &stretches {
        let centre = middle.clamp(stretch.from, stretch.to);
        let mut starts = vec![
            centre - along / 2.0,
            stretch.from,
            stretch.to - along,
            stretch.from + (stretch.length() - along) / 2.0,
        ];
        let mut x = stretch.from - along + DOOR_WIDTH + 0.2;
        while x <= stretch.to - DOOR_WIDTH - 0.2 {
            starts.push(x);
            x += 0.5;
        }
        for start in starts {
            let end = start + along;
            let shared_from = start.max(stretch.from);
            let shared_to = end.min(stretch.to);
            if shared_to - shared_from < DOOR_WIDTH + 0.2 {
                continue;
            }
            let rect = rect_out(base, wall, start, end, out);
            if house.areas.iter().any(|b| rects_overlap(rect, b.rect)) {
                continue;
            }
            let score = ((start + end) / 2.0 - middle).abs();
            fits.push((rect, score, (shared_from, shared_to)));
        }
    }
    fits.sort_by(|a, b| a.1.total_cmp(&b.1));
    if fits.is_empty() {
        return Err(Refusal::new(
            "no_room",
            format!(
                "A {w} x {d} m {} doesn't fit against {}'s {} wall without overlapping another area.",
                new.kind.word(),
                name(next_to),
                wall.word()
            ),
        ));
    }
    let mut first = None;
    for (rect, _, shared) in fits {
        match with_door(house, catalog, next_to, wall, new, &id, rect, shared) {
            Ok(after) => return Ok((after, id)),
            Err(after) => {
                first.get_or_insert(after);
            }
        }
    }
    // Nothing clear: hand back the nearest, and the checks say why.
    Ok((first.expect("at least one place"), id))
}

/// `house` with the new area at `rect` and a door through `shared`: the
/// first door position whose path in the base area is clear (Ok), or the
/// middle one (Err).
#[allow(clippy::too_many_arguments)]
fn with_door(
    house: &House,
    catalog: &Catalog,
    next_to: &str,
    wall: Wall,
    new: &NewArea,
    id: &str,
    rect: [f32; 4],
    (shared_from, shared_to): (f32, f32),
) -> Result<House, House> {
    let (_, wall_start, _) = house.area(next_to).expect("the base area").wall_line(wall);
    let id = id.to_owned();
    let (surfaces, ceiling, light) = defaults(new.kind);
    let area = Area {
        id: id.clone(),
        kind: new.kind,
        rect,
        ceiling: Some(new.ceiling.unwrap_or(ceiling)),
        surfaces,
        light,
        sound: AreaSound {
            bed: None,
            music_gain: 1.0,
        },
        intent: new.intent.clone(),
        doors: Vec::new(),
        props: Vec::new(),
        triggers: Vec::new(),
    };
    // The door goes in the middle of the shared stretch, or slides along
    // it until the base area's path to it is clear.
    let middle_of = (shared_from + shared_to) / 2.0;
    let half = DOOR_WIDTH / 2.0;
    let mut candidates = vec![middle_of];
    for k in 1..40 {
        let step = k as f32 * 0.25;
        for m in [middle_of - step, middle_of + step] {
            if m - half >= shared_from + 0.1 && m + half <= shared_to - 0.1 {
                candidates.push(m);
            }
        }
    }
    let mut first = None;
    for door_middle in candidates {
        let door_from = door_middle - half;
        let mut area = area.clone();
        let (_, new_start, _) = area.wall_line(wall.opposite());
        area.doors.push(Door {
            id: format!("{id}-{}", wall.opposite().word()),
            to: next_to.into(),
            wall: wall.opposite(),
            offset: round(door_from - new_start),
            width: DOOR_WIDTH,
        });
        let mut after = house.clone();
        let base = after.area_mut(next_to).expect("the base area");
        base.doors.push(Door {
            id: format!("{next_to}-to-{id}"),
            to: id.clone(),
            wall,
            offset: round(door_from - wall_start),
            width: DOOR_WIDTH,
        });
        after.areas.push(area);
        let clear = after
            .area(next_to)
            .is_some_and(|b| crate::check::path(&after, b, catalog).is_ok());
        if clear {
            return Ok(after);
        }
        first.get_or_insert(after);
    }
    Err(first.expect("a door position"))
}

fn round(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}

/// Removes an area and the doors into it.
///
/// # Errors
/// It doesn't exist, it is where you arrive or the way out, or removing it
/// would cut off the way out or another area.
pub fn remove_area(house: &House, id: &str) -> Result<House, Refusal> {
    if house.area(id).is_none() {
        return Err(Refusal::new(
            "unknown_area",
            format!("There's no area called {id}."),
        ));
    }
    if id == house.arrival.area {
        return Err(Refusal::new(
            "arrival",
            format!("You arrive in {}, so it has to stay.", name(id)),
        ));
    }
    if id == house.exit.area {
        return Err(Refusal::new(
            "exit",
            format!("{} has the way out, so it has to stay.", cap(&name(id))),
        ));
    }
    let mut after = house.clone();
    after.areas.retain(|a| a.id != id);
    for area in &mut after.areas {
        area.doors.retain(|d| d.to != id);
    }
    let seen = reachable(&after);
    if !seen.contains(&after.exit.area) {
        return Err(Refusal::new(
            "cuts_off_exit",
            format!("Removing {} would cut off the way out.", name(id)),
        ));
    }
    if let Some(lost) = after.areas.iter().find(|a| !seen.contains(&a.id)) {
        return Err(Refusal::new(
            "cuts_off",
            format!("Removing {} would cut off {}.", name(id), name(&lost.id)),
        ));
    }
    Ok(after)
}

/// Where to put a placed asset.
#[derive(Clone, Debug, PartialEq)]
pub enum Spot {
    /// Relative to the area's south-west corner, m.
    At([f32; 2]),
    /// A little inside a door, to one side of the way through.
    NearDoor(String),
    /// The middle of the area, beside the way through.
    Middle,
    /// Against a wall, in the middle of it.
    Against(Wall),
}

/// What [`place`] puts in.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub area: String,
    pub asset: String,
    pub spot: Spot,
    /// Degrees; by default it faces the middle of the area.
    pub yaw: Option<f32>,
    pub id: Option<String>,
    pub state: Option<String>,
    pub note: String,
    /// Bring the asset's default trigger (default yes).
    pub with_trigger: bool,
}

fn fresh_prop_id(house: &House, asset: &str) -> String {
    (1..)
        .map(|n| format!("{asset}-{n}"))
        .find(|id| house.prop(id).is_none())
        .unwrap_or_else(|| asset.into())
}

/// Candidate spots near `p`, nearest first, on a small spiral.
fn around(p: [f32; 2]) -> Vec<[f32; 2]> {
    let mut out = vec![p];
    for ring in 1..=12 {
        let r = ring as f32 * 0.35;
        for k in 0..16 {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            out.push([p[0] + r * a.cos(), p[1] + r * a.sin()]);
        }
    }
    out
}

/// Places a library asset (by id or alias) in an area, with its default
/// trigger. A spot near a door or in the middle slides to the nearest
/// place that keeps the doors' path clear.
///
/// # Errors
/// The area or asset doesn't exist, or no nearby place fits.
pub fn place(
    house: &House,
    catalog: &Catalog,
    placement: &Placement,
) -> Result<(House, String), Refusal> {
    let Some(area) = house.area(&placement.area) else {
        return Err(Refusal::new(
            "unknown_area",
            format!("There's no area called {}.", placement.area),
        ));
    };
    let Some(asset) = catalog.find(&placement.asset) else {
        return Err(Refusal::new(
            "library",
            format!(
                "There's no {} in the library; haunt_describe with the library lists what there is.",
                placement.asset
            ),
        ));
    };
    let id = placement
        .id
        .clone()
        .unwrap_or_else(|| fresh_prop_id(house, &asset.id));
    if house.prop(&id).is_some() {
        return Err(Refusal::new(
            "id",
            format!("There's already a prop called {id}."),
        ));
    }
    let [w, d] = [area.width(), area.depth()];
    let centre = [w / 2.0, d / 2.0];
    let (target, wall_yaw) = match &placement.spot {
        Spot::At(p) => (*p, None),
        Spot::Middle => (centre, None),
        Spot::NearDoor(door) => {
            let Some(door) = area.doors.iter().find(|x| &x.id == door || x.to == *door) else {
                return Err(Refusal::new(
                    "unknown_door",
                    format!("{} has no door {door}.", cap(&name(&area.id))),
                ));
            };
            let p = area.door_inside(door, 1.6);
            ([p[0] - area.x0(), p[1] - area.z0()], None)
        }
        Spot::Against(wall) => {
            let depth = asset.footprint[1] / 2.0 + 0.02;
            match wall {
                Wall::North => ([w / 2.0, d - depth], Some(180.0)),
                Wall::South => ([w / 2.0, depth], Some(0.0)),
                Wall::East => ([w - depth, d / 2.0], Some(270.0)),
                Wall::West => ([depth, d / 2.0], Some(90.0)),
            }
        }
    };
    let yaw_for = |p: [f32; 2]| {
        placement.yaw.or(wall_yaw).unwrap_or_else(|| {
            // Face the middle (or, at the middle, face the arrival side).
            let (dx, dz) = (centre[0] - p[0], centre[1] - p[1]);
            if dx.hypot(dz) < 0.5 {
                180.0
            } else {
                (dx.atan2(dz).to_degrees() + 360.0) % 360.0
            }
        })
    };
    let y = if asset.surface == Surface::Wall {
        1.2_f32
            .min(area.ceiling.unwrap_or(3.0) - asset.footprint[2])
            .max(0.0)
    } else if asset.surface == Surface::Ceiling {
        (area.ceiling.unwrap_or(3.0) - asset.footprint[2]).max(0.0)
    } else {
        0.0
    };
    let mut trigger = None;
    if placement.with_trigger
        && let Some(t) = &asset.trigger
    {
        let when = match t.when {
            haunt_assets::When::Near => When::Near {
                prop: id.clone(),
                within: t.within.unwrap_or(2.5),
            },
            haunt_assets::When::Enter => When::Enter(true),
            haunt_assets::When::EnterMiddle => When::EnterMiddle(true),
            haunt_assets::When::Look => When::Look { prop: id.clone() },
        };
        let actions = t
            .actions
            .iter()
            .filter_map(|a| {
                if let Some(s) = &a.sfx {
                    Some(Action::Sfx(s.clone()))
                } else if let Some(s) = &a.state {
                    Some(Action::State(StateChange {
                        prop: id.clone(),
                        to: s.clone(),
                    }))
                } else {
                    a.effect.as_ref().map(|e| {
                        Action::Effect(crate::doc::EffectAt {
                            id: e.clone(),
                            at: Some(id.clone()),
                        })
                    })
                }
            })
            .collect();
        trigger = Some(Trigger {
            when,
            actions,
            every_visit: true,
        });
    }
    let fixed = matches!(placement.spot, Spot::At(_));
    let spots = if fixed { vec![target] } else { around(target) };
    let mut last = None;
    for p in spots {
        let prop = Prop {
            id: id.clone(),
            asset: asset.id.clone(),
            at: [round(p[0]), round(p[1])],
            yaw: round(yaw_for(p)),
            y,
            state: placement.state.clone(),
            note: placement.note.clone(),
            text: None,
        };
        let mut after = house.clone();
        let a = after.area_mut(&placement.area).expect("the area");
        let [x0, z0, x1, z1] = footprint(a, &prop, asset.footprint);
        if asset.surface != Surface::Sky
            && (x0 < a.x0() || z0 < a.z0() || x1 > a.x1() || z1 > a.z1())
        {
            continue;
        }
        a.props.push(prop);
        if let Some(t) = &trigger {
            a.triggers.push(t.clone());
        }
        match crate::check::check(&after, catalog) {
            Ok(()) => return Ok((after, id)),
            Err(refusal) => last = Some(refusal),
        }
    }
    Err(last.unwrap_or_else(|| {
        Refusal::new(
            "place",
            format!("A {} doesn't fit in {} there.", asset.id, name(&area.id)),
        )
    }))
}
