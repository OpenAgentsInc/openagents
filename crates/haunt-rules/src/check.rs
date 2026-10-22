//! The checks every edit runs. A refusal is one plain sentence, as in the
//! Labs `Refusal`, with a stable code.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use haunt_assets::{Catalog, Kind, SoundKind, Surface};

use crate::doc::{Action, Area, AreaKind, House, KIND, OUTSIDE, Prop, Wall, When, name};

/// The most areas a house may have.
pub const MAX_AREAS: usize = 12;
/// The most props an area may have.
pub const MAX_PROPS: usize = 40;
/// The most point lights an area may have.
pub const MAX_LIGHTS: usize = 8;
/// The most sounds that may play at once in an area: the music, the bed,
/// and the effects that can fire together.
pub const MAX_SOUNDS: usize = 6;
/// The clear walking path between doors, m.
pub const PATH_WIDTH: f32 = 1.0;
/// The narrowest door, m.
pub const MIN_DOOR: f32 = 1.0;
/// The widest door, m.
pub const MAX_DOOR: f32 = 3.0;
/// How far a wall prop's middle may sit from its wall, beyond half its
/// depth, m.
const WALL_SLACK: f32 = 0.2;
/// How close two shared walls or doors must line up, m.
const SNAP: f32 = 0.05;
/// The lane grid's cell, m.
const CELL: f32 = 0.1;

/// A refused house: a stable code and one plain sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Runs every check; the first refusal, or none.
///
/// # Errors
/// The house breaks a rule; the refusal says which in one sentence.
pub fn check(house: &House, catalog: &Catalog) -> Result<(), Refusal> {
    match problems(house, catalog).into_iter().next() {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// Every problem, in the order the checks run.
#[must_use]
pub fn problems(house: &House, catalog: &Catalog) -> Vec<Refusal> {
    let mut out = Vec::new();
    basics(house, catalog, &mut out);
    if !out.is_empty() {
        return out;
    }
    for area in &house.areas {
        area_basics(area, catalog, &mut out);
    }
    overlaps(house, &mut out);
    doors(house, &mut out);
    if !out.is_empty() {
        return out;
    }
    reach(house, &mut out);
    for area in &house.areas {
        props(area, catalog, &mut out);
        triggers(area, catalog, &mut out);
        budgets(area, catalog, &mut out);
    }
    if out.is_empty() {
        for area in &house.areas {
            if let Err(refusal) = path(house, area, catalog) {
                out.push(refusal);
            }
        }
    }
    out
}

/// "A" or "An" before a word.
#[must_use]
pub fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "An"
    } else {
        "A"
    }
}

fn unit(x: f32) -> bool {
    x.is_finite() && (0.0..=1.0).contains(&x)
}

fn slug(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn basics(house: &House, catalog: &Catalog, out: &mut Vec<Refusal>) {
    if house.kind != KIND {
        out.push(Refusal::new(
            "kind",
            format!("The document must be a {KIND} house."),
        ));
    }
    let title = house.title.trim();
    if title.is_empty() || title.chars().count() > 60 {
        out.push(Refusal::new(
            "words",
            "The title must be 1 to 60 characters.",
        ));
    }
    if house.areas.is_empty() {
        out.push(Refusal::new("budget", "The house needs at least one area."));
    }
    if house.areas.len() > MAX_AREAS {
        out.push(Refusal::new(
            "budget",
            format!(
                "The house has {} areas; it can have at most {MAX_AREAS}.",
                house.areas.len()
            ),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut props = BTreeSet::new();
    let mut doors = BTreeSet::new();
    for area in &house.areas {
        if !slug(&area.id) || area.id == OUTSIDE {
            out.push(Refusal::new(
                "id",
                format!(
                    "{:?} can't be an area id; use lowercase letters, digits and dashes.",
                    area.id
                ),
            ));
        }
        if !ids.insert(area.id.as_str()) {
            out.push(Refusal::new(
                "id",
                format!("Two areas are called {}.", area.id),
            ));
        }
        for prop in &area.props {
            if !slug(&prop.id) {
                out.push(Refusal::new(
                    "id",
                    format!(
                        "{:?} can't be a prop id; use lowercase letters, digits and dashes.",
                        prop.id
                    ),
                ));
            }
            if !props.insert(prop.id.as_str()) {
                out.push(Refusal::new(
                    "id",
                    format!("Two props are called {}.", prop.id),
                ));
            }
        }
        for door in &area.doors {
            if !doors.insert(door.id.as_str()) {
                out.push(Refusal::new(
                    "id",
                    format!("Two doors are called {}.", door.id),
                ));
            }
        }
    }
    let look = &house.look;
    if !unit(look.vignette) || !unit(look.grain) {
        out.push(Refusal::new(
            "look",
            "The vignette and the grain must each be between 0 and 1.",
        ));
    }
    if catalog.sky(&look.sky).is_none() {
        out.push(Refusal::new(
            "library",
            format!("There's no sky called {} in the library.", look.sky),
        ));
    }
    let a = &house.audio;
    if ![a.master, a.music, a.ambience, a.sfx, a.voice]
        .into_iter()
        .all(unit)
    {
        out.push(Refusal::new(
            "audio",
            "Each volume must be between 0 and 1.",
        ));
    }
    if catalog
        .sound(&a.music_track)
        .is_none_or(|s| s.kind != SoundKind::Music)
    {
        out.push(Refusal::new(
            "library",
            format!("There's no music called {} in the library.", a.music_track),
        ));
    }
    if catalog.cue(&house.arrival.cue).is_none() {
        out.push(Refusal::new(
            "library",
            format!(
                "There's no arrival cue called {} in the library.",
                house.arrival.cue
            ),
        ));
    }
    match house.area(&house.arrival.area) {
        None => out.push(Refusal::new(
            "arrival",
            format!(
                "The arrival names {}, which isn't an area.",
                house.arrival.area
            ),
        )),
        Some(area) => {
            let p = area.world(house.arrival.at);
            if !area.contains(p, 0.5) || !house.arrival.yaw.is_finite() {
                out.push(Refusal::new(
                    "arrival",
                    format!(
                        "The arrival point must be inside {}, at least 0.5 m from its walls.",
                        name(&area.id)
                    ),
                ));
            }
        }
    }
    match house.area(&house.exit.area) {
        None => out.push(Refusal::new(
            "exit",
            format!("The exit names {}, which isn't an area.", house.exit.area),
        )),
        Some(area) => {
            if !area
                .doors
                .iter()
                .any(|d| d.id == house.exit.door && d.to == OUTSIDE)
            {
                out.push(Refusal::new(
                    "exit",
                    format!(
                        "The exit must be a door from {} to the outside called {}.",
                        name(&area.id),
                        house.exit.door
                    ),
                ));
            }
        }
    }
}

fn area_basics(area: &Area, catalog: &Catalog, out: &mut Vec<Refusal>) {
    let n = name(&area.id);
    let [x, z, w, d] = area.rect;
    if ![x, z, w, d].iter().all(|v| v.is_finite()) || w < 1.5 || d < 1.5 || w > 40.0 || d > 40.0 {
        out.push(Refusal::new(
            "size",
            format!("{} must be between 1.5 and 40 m on each side.", cap(&n)),
        ));
    }
    match (area.kind, area.ceiling) {
        (AreaKind::Yard, Some(_)) => out.push(Refusal::new(
            "size",
            format!("{} is outdoors, so it has no ceiling.", cap(&n)),
        )),
        (AreaKind::Hallway | AreaKind::Room, None) => out.push(Refusal::new(
            "size",
            format!("{} needs a ceiling height.", cap(&n)),
        )),
        (_, Some(c)) if !(2.4..=8.0).contains(&c) => out.push(Refusal::new(
            "size",
            format!("{}'s ceiling must be between 2.4 and 8 m high.", cap(&n)),
        )),
        _ => {}
    }
    for id in [
        Some(&area.surfaces.floor),
        Some(&area.surfaces.walls),
        area.surfaces.trim.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if catalog.surface(id).is_none() {
            out.push(Refusal::new(
                "library",
                format!("There's no surface called {id} in the library."),
            ));
        }
    }
    let light = &area.light;
    if catalog.preset(&light.preset).is_none() {
        out.push(Refusal::new(
            "library",
            format!(
                "There's no light preset called {} in the library.",
                light.preset
            ),
        ));
    }
    if !unit(light.dim) {
        out.push(Refusal::new(
            "light",
            format!("{}'s dimming must be between 0 and 1.", cap(&n)),
        ));
    }
    if let Some(ambient) = &light.ambient
        && (haunt_assets::colour(&ambient.color).is_none() || !unit(ambient.level))
    {
        out.push(Refusal::new(
            "light",
            format!(
                "{}'s ambient light needs a palette colour and a level between 0 and 1.",
                cap(&n)
            ),
        ));
    }
    for l in &light.lights {
        let inside = (0.0..=w).contains(&l.at[0]) && (0.0..=d).contains(&l.at[1]);
        let fits = area.ceiling.is_none_or(|c| l.height <= c) && l.height >= 0.0;
        if haunt_assets::colour(&l.color).is_none()
            || !inside
            || !fits
            || !(0.0..=2.0).contains(&l.intensity)
            || !(0.5..=20.0).contains(&l.radius)
            || !unit(l.flicker)
        {
            out.push(Refusal::new(
                "light",
                format!(
                    "Each light in {n} needs a palette colour, a place inside it under the ceiling, an intensity from 0 to 2, a radius from 0.5 to 20 m, and a flicker from 0 to 1."
                ),
            ));
            break;
        }
    }
    if let Some(bed) = &area.sound.bed
        && catalog.sound(bed).is_none_or(|s| s.kind != SoundKind::Bed)
    {
        out.push(Refusal::new(
            "library",
            format!("There's no ambience called {bed} in the library."),
        ));
    }
    if !(0.0..=2.0).contains(&area.sound.music_gain) {
        out.push(Refusal::new(
            "audio",
            format!("{}'s music level must be between 0 and 2.", cap(&n)),
        ));
    }
    let intent = &area.intent;
    if intent.feeling.trim().is_empty() || intent.intensity.iter().any(|i| *i > 5) {
        out.push(Refusal::new(
            "intent",
            format!(
                "{} needs an intent: the feeling it should give, and an intensity from 0 to 5.",
                cap(&n)
            ),
        ));
    }
}

fn overlap_1d(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Whether two rectangles share area (not just an edge).
#[must_use]
pub fn rects_overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    overlap_1d(a[0], a[0] + a[2], b[0], b[0] + b[2]) > 1e-3
        && overlap_1d(a[1], a[1] + a[3], b[1], b[1] + b[3]) > 1e-3
}

fn overlaps(house: &House, out: &mut Vec<Refusal>) {
    for (i, a) in house.areas.iter().enumerate() {
        for b in &house.areas[i + 1..] {
            if rects_overlap(a.rect, b.rect) {
                out.push(Refusal::new(
                    "overlap",
                    format!("{} overlaps {}.", cap(&name(&a.id)), name(&b.id)),
                ));
            }
        }
    }
}

/// The first letter in capitals.
#[must_use]
pub fn cap(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The length two areas share along `wall` of `a` (the matching wall of
/// `b`), and where it starts and ends.
#[must_use]
pub fn shared(a: &Area, wall: Wall, b: &Area) -> Option<(f32, f32)> {
    let (fixed, a0, a1) = a.wall_line(wall);
    let (other, b0, b1) = b.wall_line(wall.opposite());
    if (fixed - other).abs() > SNAP {
        return None;
    }
    let (s, e) = (a0.max(b0), a1.min(b1));
    (e - s > 1e-3).then_some((s, e))
}

fn doors(house: &House, out: &mut Vec<Refusal>) {
    for area in &house.areas {
        let n = name(&area.id);
        for door in &area.doors {
            let len = area.wall_length(door.wall);
            if !(MIN_DOOR..=MAX_DOOR).contains(&door.width)
                || door.offset < 0.0
                || door.offset + door.width > len + 1e-3
            {
                out.push(Refusal::new(
                    "door",
                    format!(
                        "The door {} must be {MIN_DOOR} to {MAX_DOOR} m wide and fit in {n}'s {} wall.",
                        door.id,
                        door.wall.word()
                    ),
                ));
                continue;
            }
            let (s, e) = area.door_span(door);
            if door.to == OUTSIDE {
                // An outside door may not open into another area.
                if house.areas.iter().any(|b| {
                    b.id != area.id
                        && shared(area, door.wall, b)
                            .is_some_and(|(bs, be)| overlap_1d(s, e, bs, be) > 1e-3)
                }) {
                    out.push(Refusal::new(
                        "door",
                        format!("The door {} opens into another area, not outside.", door.id),
                    ));
                }
                continue;
            }
            let Some(other) = house.area(&door.to) else {
                out.push(Refusal::new(
                    "door",
                    format!(
                        "The door {} leads to {}, which isn't an area.",
                        door.id, door.to
                    ),
                ));
                continue;
            };
            match shared(area, door.wall, other) {
                Some((ss, se)) if s >= ss - SNAP && e <= se + SNAP => {}
                _ => {
                    out.push(Refusal::new(
                        "door",
                        format!(
                            "The door {} isn't on a wall {} shares with {}.",
                            door.id,
                            n,
                            name(&other.id)
                        ),
                    ));
                    continue;
                }
            }
            let matched = other.doors.iter().any(|d| {
                d.to == area.id && d.wall == door.wall.opposite() && {
                    let (os, oe) = other.door_span(d);
                    (os - s).abs() <= SNAP && (oe - e).abs() <= SNAP
                }
            });
            if !matched {
                out.push(Refusal::new(
                    "door",
                    format!(
                        "{} must list the door {} too, at the same place on its {} wall.",
                        cap(&name(&other.id)),
                        door.id,
                        door.wall.opposite().word()
                    ),
                ));
            }
        }
    }
}

/// The areas reachable from the arrival through doors, and whether the
/// way out is among them.
#[must_use]
pub fn reachable(house: &House) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([house.arrival.area.clone()]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(area) = house.area(&id) {
            for door in &area.doors {
                if door.to != OUTSIDE && house.area(&door.to).is_some() {
                    queue.push_back(door.to.clone());
                }
            }
        }
    }
    seen
}

fn reach(house: &House, out: &mut Vec<Refusal>) {
    let seen = reachable(house);
    for area in &house.areas {
        if !seen.contains(&area.id) {
            out.push(Refusal::new(
                "unreachable",
                format!("You can't get to {} from where you arrive.", name(&area.id)),
            ));
        }
    }
    if !seen.contains(&house.exit.area) {
        out.push(Refusal::new(
            "unreachable",
            "You can't get to the way out from where you arrive.",
        ));
    }
}

/// A prop's footprint in world coordinates, turned by its yaw: `[x0, z0,
/// x1, z1]`.
#[must_use]
pub fn footprint(area: &Area, prop: &Prop, size: [f32; 3]) -> [f32; 4] {
    let [cx, cz] = area.world(prop.at);
    let (s, c) = prop.yaw.to_radians().sin_cos();
    let (hw, hd) = (size[0] / 2.0, size[1] / 2.0);
    let ex = (hw * c).abs() + (hd * s).abs();
    let ez = (hw * s).abs() + (hd * c).abs();
    [cx - ex, cz - ez, cx + ex, cz + ez]
}

/// Whether a prop stands on the floor in the way of a walker.
#[must_use]
pub fn blocks_floor(prop: &Prop, surface: Surface) -> bool {
    surface == Surface::Floor && prop.y < 0.3
}

fn props(area: &Area, catalog: &Catalog, out: &mut Vec<Refusal>) {
    let n = name(&area.id);
    for prop in &area.props {
        let Some(asset) = catalog.asset(&prop.asset) else {
            out.push(Refusal::new(
                "library",
                format!("There's no {} in the library.", prop.asset),
            ));
            continue;
        };
        if let Some(state) = &prop.state
            && !asset.states.contains(state)
        {
            out.push(Refusal::new(
                "library",
                format!(
                    "{} {} can be {}, not {state}.",
                    article(&asset.id),
                    asset.id,
                    asset.states.join(" or ")
                ),
            ));
        }
        if prop.text.as_ref().is_some_and(|t| t.chars().count() > 120) {
            out.push(Refusal::new(
                "words",
                format!("The words on {} must be 120 characters or fewer.", prop.id),
            ));
        }
        if ![prop.at[0], prop.at[1], prop.yaw, prop.y]
            .iter()
            .all(|v| v.is_finite())
            || prop.y < 0.0
        {
            out.push(Refusal::new(
                "place",
                format!("{} needs a place with real numbers.", prop.id),
            ));
            continue;
        }
        let [x0, z0, x1, z1] = footprint(area, prop, asset.footprint);
        let inside = x0 >= area.x0() - 1e-3
            && z0 >= area.z0() - 1e-3
            && x1 <= area.x1() + 1e-3
            && z1 <= area.z1() + 1e-3;
        let height = asset.footprint[2];
        match asset.surface {
            Surface::Sky => {
                if area.ceiling.is_some() {
                    out.push(Refusal::new(
                        "place",
                        format!("The {} goes in the sky, so only outdoors.", asset.id),
                    ));
                }
                continue;
            }
            Surface::Ceiling if area.ceiling.is_none() => {
                out.push(Refusal::new(
                    "place",
                    format!(
                        "The {} hangs from a ceiling, and {n} is outdoors.",
                        asset.id
                    ),
                ));
                continue;
            }
            _ => {}
        }
        if !inside {
            out.push(Refusal::new(
                "place",
                format!("{} doesn't fit inside {n} where it is.", prop.id),
            ));
            continue;
        }
        if let Some(ceiling) = area.ceiling
            && prop.y + height > ceiling + 1e-3
        {
            out.push(Refusal::new(
                "place",
                format!(
                    "{} is {height} m tall and doesn't fit under {n}'s {ceiling} m ceiling there.",
                    prop.id
                ),
            ));
            continue;
        }
        if asset.surface == Surface::Wall {
            let [cx, cz] = area.world(prop.at);
            let reach = asset.footprint[1] / 2.0 + WALL_SLACK;
            let near = (cx - area.x0()).abs() <= reach
                || (area.x1() - cx).abs() <= reach
                || (cz - area.z0()).abs() <= reach
                || (area.z1() - cz).abs() <= reach;
            if !near {
                out.push(Refusal::new(
                    "place",
                    format!(
                        "The {} hangs on a wall; move {} against one.",
                        asset.id, prop.id
                    ),
                ));
            }
        }
    }
}

fn triggers(area: &Area, catalog: &Catalog, out: &mut Vec<Refusal>) {
    let n = name(&area.id);
    let prop_asset: BTreeMap<&str, &str> = area
        .props
        .iter()
        .map(|p| (p.id.as_str(), p.asset.as_str()))
        .collect();
    for trigger in &area.triggers {
        match &trigger.when {
            When::Near { prop, within } => {
                if !prop_asset.contains_key(prop.as_str()) {
                    out.push(Refusal::new(
                        "trigger",
                        format!("A trigger in {n} watches {prop}, which isn't in {n}."),
                    ));
                }
                if !(0.5..=12.0).contains(within) {
                    out.push(Refusal::new(
                        "trigger",
                        format!("A near trigger's distance must be 0.5 to 12 m."),
                    ));
                }
            }
            When::Look { prop } if !prop_asset.contains_key(prop.as_str()) => {
                out.push(Refusal::new(
                    "trigger",
                    format!("A trigger in {n} watches {prop}, which isn't in {n}."),
                ));
            }
            _ => {}
        }
        if trigger.actions.is_empty() {
            out.push(Refusal::new(
                "trigger",
                format!("A trigger in {n} does nothing; give it something to do."),
            ));
        }
        for action in &trigger.actions {
            match action {
                Action::Sfx(id) => {
                    if catalog
                        .sound(id)
                        .is_none_or(|s| s.kind != SoundKind::Effect)
                    {
                        out.push(Refusal::new(
                            "library",
                            format!("There's no sound effect called {id} in the library."),
                        ));
                    }
                }
                Action::Light(change) => {
                    if change
                        .preset
                        .as_ref()
                        .is_some_and(|p| catalog.preset(p).is_none())
                    {
                        out.push(Refusal::new(
                            "library",
                            format!(
                                "There's no light preset called {} in the library.",
                                change.preset.as_deref().unwrap_or_default()
                            ),
                        ));
                    }
                    if change.dim.is_some_and(|d| !unit(d))
                        || change.seconds.is_some_and(|s| !(0.1..=60.0).contains(&s))
                    {
                        out.push(Refusal::new(
                            "light",
                            "A light change dims from 0 to 1 for 0.1 to 60 seconds.",
                        ));
                    }
                }
                Action::State(change) => match prop_asset.get(change.prop.as_str()) {
                    None => out.push(Refusal::new(
                        "trigger",
                        format!(
                            "A trigger in {n} changes {}, which isn't in {n}.",
                            change.prop
                        ),
                    )),
                    Some(asset) => {
                        if catalog
                            .asset(asset)
                            .is_some_and(|a| !a.states.contains(&change.to))
                        {
                            out.push(Refusal::new(
                                "library",
                                format!("{} can't be {}.", change.prop, change.to),
                            ));
                        }
                    }
                },
                Action::Effect(effect) => {
                    if catalog
                        .asset(&effect.id)
                        .is_none_or(|a| a.kind != Kind::Effect)
                    {
                        out.push(Refusal::new(
                            "library",
                            format!("There's no effect called {} in the library.", effect.id),
                        ));
                    }
                    if let Some(at) = &effect.at
                        && !prop_asset.contains_key(at.as_str())
                    {
                        out.push(Refusal::new(
                            "trigger",
                            format!("An effect in {n} is at {at}, which isn't in {n}."),
                        ));
                    }
                }
            }
        }
    }
}

/// The most sounds that can play at once in an area: the music, its bed,
/// the effects of everything that fires on entering and at the middle, and
/// the largest single prop trigger.
#[must_use]
pub fn sounds_at_once(area: &Area) -> usize {
    let sfx = |t: &crate::doc::Trigger| {
        t.actions
            .iter()
            .filter(|a| matches!(a, Action::Sfx(_)))
            .count()
    };
    let together: usize = area
        .triggers
        .iter()
        .filter(|t| matches!(t.when, When::Enter(_) | When::EnterMiddle(_)))
        .map(sfx)
        .sum();
    let single = area
        .triggers
        .iter()
        .filter(|t| t.prop().is_some())
        .map(sfx)
        .max()
        .unwrap_or(0);
    1 + usize::from(area.sound.bed.is_some()) + together + single
}

fn budgets(area: &Area, _catalog: &Catalog, out: &mut Vec<Refusal>) {
    let n = name(&area.id);
    if area.props.len() > MAX_PROPS {
        out.push(Refusal::new(
            "budget",
            format!(
                "{} has {} props; an area can have at most {MAX_PROPS}.",
                cap(&n),
                area.props.len()
            ),
        ));
    }
    if area.light.lights.len() > MAX_LIGHTS {
        out.push(Refusal::new(
            "budget",
            format!(
                "{} has {} lights; an area can have at most {MAX_LIGHTS}.",
                cap(&n),
                area.light.lights.len()
            ),
        ));
    }
    let sounds = sounds_at_once(area);
    if sounds > MAX_SOUNDS {
        out.push(Refusal::new(
            "budget",
            format!("Up to {sounds} sounds could play at once in {n}; the most is {MAX_SOUNDS}."),
        ));
    }
}

/// The free cells of an area's walking grid: each cell's middle is at
/// least half the path's width from every wall and floor prop.
struct Grid {
    x0: f32,
    z0: f32,
    nx: usize,
    nz: usize,
    free: Vec<bool>,
}

impl Grid {
    fn of(area: &Area, catalog: &Catalog) -> Self {
        let half = PATH_WIDTH / 2.0;
        let nx = (area.width() / CELL).round().max(1.0) as usize;
        let nz = (area.depth() / CELL).round().max(1.0) as usize;
        let boxes: Vec<[f32; 4]> = area
            .props
            .iter()
            .filter_map(|p| {
                let asset = catalog.asset(&p.asset)?;
                blocks_floor(p, asset.surface).then(|| footprint(area, p, asset.footprint))
            })
            .collect();
        let mut free = vec![false; nx * nz];
        for j in 0..nz {
            for i in 0..nx {
                let x = area.x0() + (i as f32 + 0.5) * CELL;
                let z = area.z0() + (j as f32 + 0.5) * CELL;
                let walls = x - area.x0() >= half - 1e-3
                    && area.x1() - x >= half - 1e-3
                    && z - area.z0() >= half - 1e-3
                    && area.z1() - z >= half - 1e-3;
                let clear = boxes.iter().all(|b| {
                    let dx = (b[0] - x).max(x - b[2]).max(0.0);
                    let dz = (b[1] - z).max(z - b[3]).max(0.0);
                    dx.hypot(dz) >= half - 1e-3
                });
                free[j * nx + i] = walls && clear;
            }
        }
        Self {
            x0: area.x0(),
            z0: area.z0(),
            nx,
            nz,
            free,
        }
    }

    fn cell(&self, p: [f32; 2]) -> Option<usize> {
        let i = ((p[0] - self.x0) / CELL).floor();
        let j = ((p[1] - self.z0) / CELL).floor();
        if i < 0.0 || j < 0.0 {
            return None;
        }
        let (i, j) = (i as usize, j as usize);
        (i < self.nx && j < self.nz).then_some(j * self.nx + i)
    }

    /// The free cell nearest `p` within a cell or two.
    fn start(&self, p: [f32; 2]) -> Option<usize> {
        let mut best = None;
        for dj in -2..=2 {
            for di in -2..=2 {
                let q = [p[0] + di as f32 * CELL, p[1] + dj as f32 * CELL];
                if let Some(c) = self.cell(q)
                    && self.free[c]
                {
                    let d = di * di + dj * dj;
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, c));
                    }
                }
            }
        }
        best.map(|(_, c)| c)
    }

    fn connected(&self, from: usize) -> Vec<bool> {
        let mut seen = vec![false; self.free.len()];
        let mut queue = VecDeque::from([from]);
        seen[from] = true;
        while let Some(c) = queue.pop_front() {
            let (i, j) = (c % self.nx, c / self.nx);
            let mut next = Vec::with_capacity(4);
            if i > 0 {
                next.push(c - 1);
            }
            if i + 1 < self.nx {
                next.push(c + 1);
            }
            if j > 0 {
                next.push(c - self.nx);
            }
            if j + 1 < self.nz {
                next.push(c + self.nx);
            }
            for n in next {
                if self.free[n] && !seen[n] {
                    seen[n] = true;
                    queue.push_back(n);
                }
            }
        }
        seen
    }
}

/// The points a walker must be able to reach in an area: just inside each
/// door, and the arrival point.
fn waypoints(house: &House, area: &Area) -> Vec<(String, [f32; 2])> {
    let half = PATH_WIDTH / 2.0;
    let mut points: Vec<(String, [f32; 2])> = area
        .doors
        .iter()
        .map(|d| {
            (
                format!("the door {}", d.id),
                area.door_inside(d, half + CELL),
            )
        })
        .collect();
    if house.arrival.area == area.id {
        points.push(("where you arrive".into(), area.world(house.arrival.at)));
    }
    points
}

/// What a walker can't reach in an area, skipping prop `skip`.
fn unreached(house: &House, area: &Area, catalog: &Catalog, skip: Option<usize>) -> Option<String> {
    let points = waypoints(house, area);
    let mut trimmed;
    let area = match skip {
        Some(i) => {
            trimmed = area.clone();
            trimmed.props.remove(i);
            &trimmed
        }
        None => area,
    };
    let grid = Grid::of(area, catalog);
    let mut starts = Vec::new();
    for (what, p) in &points {
        match grid.start(*p) {
            Some(c) => starts.push((what, c)),
            None => return Some(what.clone()),
        }
    }
    let Some((_, first)) = starts.first() else {
        return None;
    };
    let seen = grid.connected(*first);
    starts
        .iter()
        .find(|(_, c)| !seen[*c])
        .map(|(what, _)| (*what).clone())
}

/// Checks that a clear path [`PATH_WIDTH`] wide joins every door of an
/// area (and the arrival point).
///
/// # Errors
/// A prop or a corner blocks it; the refusal names what can't be reached
/// and the prop in the way.
pub fn path(house: &House, area: &Area, catalog: &Catalog) -> Result<(), Refusal> {
    let Some(what) = unreached(house, area, catalog, None) else {
        return Ok(());
    };
    let culprit = area.props.iter().enumerate().find(|(i, p)| {
        catalog
            .asset(&p.asset)
            .is_some_and(|a| blocks_floor(p, a.surface))
            && unreached(house, area, catalog, Some(*i)).is_none()
    });
    let message = match culprit {
        Some((_, p)) => format!(
            "{} blocks the way to {what} in {}: keep a clear path {PATH_WIDTH} m wide between its doors.",
            p.id,
            name(&area.id)
        ),
        None => format!(
            "There's no clear path {PATH_WIDTH} m wide to {what} in {}.",
            name(&area.id)
        ),
    };
    Err(Refusal::new("path", message))
}
