//! The Haunted House as Game Lab's second game
//! (`docs/verse/games/the-haunted-house.md`, "Game Lab: one change per
//! tester").
//!
//! The house is one `haunt.v1` document. The chat's model changes it only
//! through typed tools, each one edit: place a library asset, add or change
//! a trigger, change a light or the look, change the volumes or the music,
//! add or remove an area, or change the words. Every edit runs the house's
//! checks (`haunt-rules`), and a refused edit says why in one sentence and
//! changes nothing. Words of a message are never matched here: the model's
//! choice of tool is the route, as for every Lab.

use haunt_assets::Catalog;
use haunt_rules::doc::{
    Action, AreaKind, EffectAt, House, Intent, LightChange, StateChange, Trigger, Wall, When,
};
use haunt_rules::edit::{NewArea, Placement, Spot};
use lab_kit::{
    Artifact, Card, Change, Draft, JobSpec, JobState, KindManifest, LabKind, Limits, Measurements,
    Refusal, Report, ToolDecl, Version, View, Want,
};
use serde_json::{Value, json};

/// The house's document id in Game Lab.
pub const HAUNTED_HOUSE: &str = "the-haunted-house";
/// Its title.
pub const TITLE: &str = "The Haunted House";
/// The tools that change the house; `haunt_describe` only reads it.
pub const EDITS: [&str; 7] = [
    "haunt_place",
    "haunt_trigger",
    "haunt_light",
    "haunt_audio",
    "haunt_add_area",
    "haunt_remove_area",
    "haunt_words",
];
/// Where the house plays on the website.
pub const PLAY_PATH: &str = "/games/the-haunted-house";

fn catalog() -> &'static Catalog {
    haunt_assets::catalog()
}

/// Version 1: the MVP house.
#[must_use]
pub fn original() -> Value {
    serde_json::from_str(haunt_rules::doc::V1_JSON).unwrap_or(Value::Null)
}

fn refuse(code: &str, message: impl Into<String>) -> Refusal {
    Refusal::new(code, message)
}

/// A house as a document, with its numbers as written (`to_value` would
/// widen 0.15 to 0.15000000596).
#[must_use]
pub fn to_doc(house: &House) -> Value {
    serde_json::to_string(house)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

/// Reads a document as a checked house.
///
/// # Errors
/// It doesn't read, or a check refuses it.
pub fn read(doc: &Value) -> Result<House, Refusal> {
    let house = House::parse(&doc.to_string()).map_err(|e| refuse("haunt_schema", e))?;
    haunt_rules::check(&house, catalog()).map_err(|r| refuse(r.code, r.message))?;
    Ok(house)
}

/// The text an agent reads: the map and every area, or the library.
///
/// # Errors
/// The document doesn't read.
pub fn describe(doc: &Value, library: bool) -> Result<String, Refusal> {
    if library {
        return Ok(catalog().describe());
    }
    let house = House::parse(&doc.to_string()).map_err(|e| refuse("haunt_schema", e))?;
    Ok(haunt_rules::describe::describe(&house, catalog()))
}

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str().map(str::trim).filter(|v| !v.is_empty())
}

fn f(args: &Value, key: &str) -> Option<f32> {
    args[key].as_f64().map(|v| v as f32)
}

fn need<'a>(args: &'a Value, key: &str, what: &str) -> Result<&'a str, Refusal> {
    s(args, key).ok_or_else(|| refuse("missing", format!("Say {what} ({key}).")))
}

fn unit(value: f32, what: &str) -> Result<f32, Refusal> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(refuse("range", format!("{what} must be between 0 and 1.")))
    }
}

/// One edit's result: the new house and the changes in plain words.
#[derive(Clone, Debug, PartialEq)]
pub struct Edited {
    pub house: House,
    pub changes: Vec<String>,
}

/// Applies one tool's edit to `before`, then the house's checks.
///
/// # Errors
/// The tool or its arguments don't make an edit, or the checks refuse the
/// result; one plain sentence says why.
pub fn edit(before: &Value, tool: &str, args: &Value) -> Result<Edited, Refusal> {
    let house = House::parse(&before.to_string()).map_err(|e| refuse("haunt_schema", e))?;
    let (after, changes) = match tool {
        "haunt_place" => place(&house, args)?,
        "haunt_trigger" => trigger(&house, args)?,
        "haunt_light" => light(&house, args)?,
        "haunt_audio" => audio(&house, args)?,
        "haunt_add_area" => add_area(&house, args)?,
        "haunt_remove_area" => {
            let id = need(args, "area", "which area to remove")?;
            let after = haunt_rules::edit::remove_area(&house, id)
                .map_err(|r| refuse(r.code, r.message))?;
            (after, vec![format!("removed {}", id)])
        }
        "haunt_words" => words(&house, args)?,
        other => {
            return Err(refuse(
                "unknown_tool",
                format!("There's no tool called {other}."),
            ));
        }
    };
    if after == house {
        return Err(refuse("no_change", "That edit changes nothing."));
    }
    haunt_rules::check(&after, catalog()).map_err(|r| refuse(r.code, r.message))?;
    Ok(Edited {
        house: after,
        changes,
    })
}

fn spot(args: &Value) -> Result<Spot, Refusal> {
    if let Some(at) = args["at"].as_array() {
        let x = at.first().and_then(Value::as_f64);
        let z = at.get(1).and_then(Value::as_f64);
        return match (x, z) {
            (Some(x), Some(z)) => Ok(Spot::At([x as f32, z as f32])),
            _ => Err(refuse(
                "place",
                "Give at as [x, z] in metres from the area's south-west corner.",
            )),
        };
    }
    let near = s(args, "near").unwrap_or("middle");
    if near == "middle" {
        return Ok(Spot::Middle);
    }
    if let Some(wall) = near.strip_prefix("wall:").and_then(Wall::parse) {
        return Ok(Spot::Against(wall));
    }
    if let Some(door) = near.strip_prefix("door:") {
        return Ok(Spot::NearDoor(door.to_owned()));
    }
    Err(refuse(
        "place",
        "Say where: middle, wall:north (or east, south, west), door:ID, or at [x, z].",
    ))
}

fn place(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let area = need(args, "area", "which area")?;
    let asset = need(args, "asset", "which library asset")?;
    let placement = Placement {
        area: area.into(),
        asset: asset.into(),
        spot: spot(args)?,
        yaw: f(args, "facing"),
        id: s(args, "id").map(str::to_owned),
        state: s(args, "state").map(str::to_owned),
        note: s(args, "note").unwrap_or_default().to_owned(),
        with_trigger: args["trigger"].as_bool().unwrap_or(true),
    };
    let (after, id) = haunt_rules::edit::place(house, catalog(), &placement)
        .map_err(|r| refuse(r.code, r.message))?;
    let (_, prop) = after
        .prop(&id)
        .ok_or_else(|| refuse("place", "The prop didn't land."))?;
    let mut changes = vec![format!(
        "placed {} ({}) in {} at [{:.1}, {:.1}]",
        id, prop.asset, area, prop.at[0], prop.at[1]
    )];
    if let Some(t) = after
        .area(area)
        .and_then(|a| a.triggers.iter().find(|t| t.prop() == Some(id.as_str())))
    {
        let sounds: Vec<&str> = t
            .actions
            .iter()
            .filter_map(|a| match a {
                Action::Sfx(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        let when = match &t.when {
            When::Near { within, .. } => format!("when you come within {within} m"),
            When::Look { .. } => "when you look at it".into(),
            When::Enter(_) => "when you enter".into(),
            When::EnterMiddle(_) => "at the middle".into(),
        };
        changes.push(format!("it plays {} {when}", sounds.join(" and ")));
    }
    Ok((after, changes))
}

fn trigger(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let area_id = need(args, "area", "which area")?;
    let mut after = house.clone();
    let area = after
        .area_mut(area_id)
        .ok_or_else(|| refuse("unknown_area", format!("There's no area called {area_id}.")))?;
    if let Some(index) = args["remove"].as_u64() {
        let i = index as usize;
        if i >= area.triggers.len() {
            return Err(refuse("trigger", format!("{area_id} has no trigger {i}.")));
        }
        area.triggers.remove(i);
        return Ok((after, vec![format!("removed trigger {i} in {area_id}")]));
    }
    let prop = s(args, "prop").map(str::to_owned);
    let when = match need(args, "when", "when it fires (enter, middle, near or look)")? {
        "enter" => When::Enter(true),
        "middle" => When::EnterMiddle(true),
        "near" => When::Near {
            prop: prop
                .clone()
                .ok_or_else(|| refuse("trigger", "A near trigger needs the prop it watches."))?,
            within: f(args, "within").unwrap_or(2.5),
        },
        "look" => When::Look {
            prop: prop
                .clone()
                .ok_or_else(|| refuse("trigger", "A look trigger needs the prop it watches."))?,
        },
        other => {
            return Err(refuse(
                "trigger",
                format!("A trigger fires on enter, middle, near or look, not {other}."),
            ));
        }
    };
    let mut actions = Vec::new();
    if let Some(id) = s(args, "sfx") {
        actions.push(Action::Sfx(id.into()));
    }
    if s(args, "light_preset").is_some() || f(args, "dim").is_some() {
        actions.push(Action::Light(LightChange {
            preset: s(args, "light_preset").map(str::to_owned),
            dim: f(args, "dim"),
            seconds: f(args, "seconds"),
        }));
    }
    if let Some(to) = s(args, "prop_state") {
        let target = s(args, "state_prop")
            .map(str::to_owned)
            .or_else(|| prop.clone())
            .ok_or_else(|| refuse("trigger", "Say which prop changes state."))?;
        actions.push(Action::State(StateChange {
            prop: target,
            to: to.into(),
        }));
    }
    if let Some(id) = s(args, "effect") {
        actions.push(Action::Effect(EffectAt {
            id: id.into(),
            at: prop.clone(),
        }));
    }
    if actions.is_empty() {
        return Err(refuse(
            "trigger",
            "Give the trigger something to do: a sound (sfx), a light change, a prop state, or an effect.",
        ));
    }
    let new = Trigger {
        when,
        actions,
        every_visit: args["first_visit_only"].as_bool() != Some(true),
    };
    let what = match args["replace"].as_u64() {
        Some(i) if (i as usize) < area.triggers.len() => {
            area.triggers[i as usize] = new;
            format!("changed trigger {i} in {area_id}")
        }
        Some(i) => return Err(refuse("trigger", format!("{area_id} has no trigger {i}."))),
        None => {
            area.triggers.push(new);
            format!("added a trigger in {area_id}")
        }
    };
    Ok((after, vec![what]))
}

fn light(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let mut after = house.clone();
    let mut changes = Vec::new();
    if let Some(area_id) = s(args, "area") {
        let area = after
            .area_mut(area_id)
            .ok_or_else(|| refuse("unknown_area", format!("There's no area called {area_id}.")))?;
        if let Some(preset) = s(args, "preset") {
            area.light.preset = preset.into();
            changes.push(format!("{area_id}'s light is now {preset}"));
        }
        if let Some(dim) = f(args, "dim") {
            area.light.dim = unit(dim, "The dimming")?;
            changes.push(format!("{area_id} dimmed to {dim}"));
        }
        if let Some(token) = s(args, "ambient") {
            let level = f(args, "ambient_level").unwrap_or(0.3);
            area.light.ambient = Some(haunt_rules::doc::Ambient {
                color: token.into(),
                level: unit(level, "The ambient level")?,
            });
            changes.push(format!("{area_id}'s ambient light is {token} at {level}"));
        }
    }
    let look = &mut after.look;
    if let Some(v) = f(args, "vignette") {
        look.vignette = unit(v, "The vignette")?;
        changes.push(format!("vignette {v}"));
    }
    if let Some(g) = f(args, "grain") {
        look.grain = unit(g, "The grain")?;
        changes.push(format!("grain {g}"));
    }
    if let Some(on) = args["lantern"].as_bool() {
        look.lantern = on;
        changes.push(format!("lantern {}", if on { "on" } else { "off" }));
    }
    if let Some(sky) = s(args, "sky") {
        look.sky = sky.into();
        changes.push(format!("sky {sky}"));
    }
    if changes.is_empty() {
        return Err(refuse(
            "light",
            "Say what to change: an area's preset, dim or ambient, or the vignette, grain, lantern or sky.",
        ));
    }
    Ok((after, changes))
}

/// The buses `haunt_audio` changes.
pub const BUSES: [&str; 5] = ["master", "music", "ambience", "sfx", "voice"];

fn audio(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let mut after = house.clone();
    let mut changes = Vec::new();
    for bus in BUSES {
        let current = match bus {
            "master" => after.audio.master,
            "music" => after.audio.music,
            "ambience" => after.audio.ambience,
            "sfx" => after.audio.sfx,
            _ => after.audio.voice,
        };
        let to = if let Some(level) = f(&args[bus], "to").or_else(|| f(args, bus)) {
            Some(level)
        } else {
            f(&args[bus], "by").map(|by| (current + by).clamp(0.0, 1.0))
        };
        if let Some(level) = to {
            let level = (unit(level, &format!("The {bus} volume"))? * 100.0).round() / 100.0;
            match bus {
                "master" => after.audio.master = level,
                "music" => after.audio.music = level,
                "ambience" => after.audio.ambience = level,
                "sfx" => after.audio.sfx = level,
                _ => after.audio.voice = level,
            }
            changes.push(format!("{bus} {current:.2} → {level:.2}"));
        }
    }
    if let Some(track) = s(args, "music_track") {
        after.audio.music_track = track.into();
        changes.push(format!("music track {track}"));
    }
    if changes.is_empty() {
        return Err(refuse(
            "audio",
            "Say which volume to change (master, music, ambience, sfx or voice) and to what, or by how much.",
        ));
    }
    Ok((after, changes))
}

fn add_area(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let next_to = need(args, "next_to", "the area to build against")?;
    let wall = need(args, "wall", "which of its walls")?;
    let wall =
        Wall::parse(wall).ok_or_else(|| refuse("wall", "A wall is north, east, south or west."))?;
    let kind = match s(args, "kind").unwrap_or("room") {
        "room" => AreaKind::Room,
        "hallway" => AreaKind::Hallway,
        other => {
            return Err(refuse(
                "kind",
                format!("An area is a room or a hallway, not {other}."),
            ));
        }
    };
    let width = f(args, "width").ok_or_else(|| refuse("size", "Give the width in metres."))?;
    let depth = f(args, "depth").ok_or_else(|| refuse("size", "Give the depth in metres."))?;
    let feeling = need(args, "feeling", "the feeling it should give")?;
    let intensity = args["intensity"]
        .as_array()
        .map(|v| {
            let at = |i: usize| v.get(i).and_then(Value::as_u64).unwrap_or(2).min(5) as u8;
            [at(0), at(1)]
        })
        .unwrap_or([2, 3]);
    let new = NewArea {
        id: s(args, "id").map(str::to_owned),
        kind,
        size: [width, depth],
        intent: Intent {
            feeling: feeling.into(),
            beat: s(args, "beat").unwrap_or_default().into(),
            intensity,
        },
        ceiling: f(args, "ceiling"),
    };
    let (after, id) = haunt_rules::edit::add_area(house, catalog(), next_to, wall, &new)
        .map_err(|r| refuse(r.code, r.message))?;
    let rect = after.area(&id).map(|a| a.rect).unwrap_or_default();
    Ok((
        after,
        vec![format!(
            "added the {} {id}, {width} x {depth} m, at [{}, {}], through a door in {next_to}'s {} wall",
            kind.word(),
            rect[0],
            rect[1],
            wall.word()
        )],
    ))
}

fn words(house: &House, args: &Value) -> Result<(House, Vec<String>), Refusal> {
    let mut after = house.clone();
    let mut changes = Vec::new();
    if let Some(title) = s(args, "title") {
        after.title = title.into();
        changes.push(format!("the title is now \"{title}\""));
    }
    if let Some(prop) = s(args, "prop") {
        let text = need(args, "text", "the words")?;
        let found = after
            .areas
            .iter_mut()
            .flat_map(|a| a.props.iter_mut())
            .find(|p| p.id == prop)
            .ok_or_else(|| refuse("unknown_prop", format!("There's no prop called {prop}.")))?;
        found.text = Some(text.into());
        changes.push(format!("{prop} now reads \"{text}\""));
    }
    if changes.is_empty() {
        return Err(refuse(
            "words",
            "Give a new title, or a prop (a sign) and its words.",
        ));
    }
    Ok((after, changes))
}

/// The tools' declarations for the chat's model.
#[must_use]
pub fn tools() -> Value {
    let summary = json!({"type": "string", "description": "one line in plain words saying what the edit changes"});
    let area =
        json!({"type": "string", "description": "an area id from haunt_describe, e.g. long-hall"});
    let level = json!({"type": "object", "description": "a level: to (0 to 1) or by (a change, e.g. -0.2)",
        "properties": {"to": {"type": "number"}, "by": {"type": "number"}}});
    json!({"tools": [
        {"name": "haunt_describe", "description": "Read The Haunted House before changing it: a map, then every area with its size, place beside its neighbours, the way you face coming in, its light, sound, intended feeling, props and triggers, its free walls, and the budgets left; with library true, every prepared asset (cards, meshes, effects), sound, surface, light preset and colour instead.",
         "parameters": {"type": "object", "properties": {"library": {"type": "boolean"}}}},
        {"name": "haunt_place", "description": "Place a prepared library asset (by id or alias, e.g. werewolf, black cat, coffin) in an area, with its default trigger (a werewolf howls when you come near). near: middle, wall:north|east|south|west, door:DOOR_ID; or at [x, z] in metres from the area's south-west corner. It slides to the nearest spot that keeps the doors' path clear.",
         "parameters": {"type": "object", "required": ["area", "asset", "summary"], "properties": {
            "area": area, "asset": {"type": "string"},
            "near": {"type": "string"}, "at": {"type": "array", "items": {"type": "number"}},
            "facing": {"type": "number", "description": "degrees; 0 faces north"},
            "state": {"type": "string"}, "note": {"type": "string"},
            "trigger": {"type": "boolean", "description": "bring the asset's default trigger (default true)"},
            "summary": summary}}},
        {"name": "haunt_trigger", "description": "Add (or replace, or remove) a trigger in an area: when (enter, middle, near, look; near and look watch a prop) and what it does: sfx (a sound id), a light change (light_preset, dim, seconds), a prop's state (prop_state, of prop or state_prop), or an effect (lightning-flash, light-flicker, door-slam).",
         "parameters": {"type": "object", "required": ["area", "summary"], "properties": {
            "area": area, "when": {"type": "string"}, "prop": {"type": "string"},
            "within": {"type": "number"}, "sfx": {"type": "string"},
            "light_preset": {"type": "string"}, "dim": {"type": "number"}, "seconds": {"type": "number"},
            "prop_state": {"type": "string"}, "state_prop": {"type": "string"}, "effect": {"type": "string"},
            "first_visit_only": {"type": "boolean"},
            "replace": {"type": "integer", "description": "the index of the trigger to replace"},
            "remove": {"type": "integer", "description": "the index of the trigger to remove"},
            "summary": summary}}},
        {"name": "haunt_light", "description": "Change an area's light (preset: moonlight, candle, lantern-only, lightning, jack-o-glow, blackout; dim 0 to 1; ambient colour and level) or the whole house's look (vignette 0 to 1, grain 0 to 1, lantern on or off, sky).",
         "parameters": {"type": "object", "required": ["summary"], "properties": {
            "area": area, "preset": {"type": "string"}, "dim": {"type": "number"},
            "ambient": {"type": "string"}, "ambient_level": {"type": "number"},
            "vignette": {"type": "number"}, "grain": {"type": "number"},
            "lantern": {"type": "boolean"}, "sky": {"type": "string"}, "summary": summary}}},
        {"name": "haunt_audio", "description": "Change the house's volumes, live in a running house: master, music, ambience, sfx and voice, each to a level from 0 to 1 or by an amount (turn the music down: music by -0.25), or the music track. Read the current levels with haunt_describe.",
         "parameters": {"type": "object", "required": ["summary"], "properties": {
            "master": level, "music": level, "ambience": level, "sfx": level, "voice": level,
            "music_track": {"type": "string"}, "summary": summary}}},
        {"name": "haunt_add_area", "description": "Add a hallway or room against a free wall of an existing area (haunt_describe lists the free walls and how big an area each can take), with a door through, and the feeling it should give. width is east-west and depth north-south, in metres.",
         "parameters": {"type": "object", "required": ["next_to", "wall", "width", "depth", "feeling", "summary"], "properties": {
            "next_to": area, "wall": {"type": "string"}, "kind": {"type": "string", "description": "room or hallway"},
            "width": {"type": "number"}, "depth": {"type": "number"},
            "feeling": {"type": "string"}, "beat": {"type": "string"},
            "intensity": {"type": "array", "items": {"type": "integer"}, "description": "on entering and at its peak, 0 to 5"},
            "id": {"type": "string"}, "ceiling": {"type": "number"}, "summary": summary}}},
        {"name": "haunt_remove_area", "description": "Remove an area and its doors. Refused when it would cut off the way out or another area.",
         "parameters": {"type": "object", "required": ["area", "summary"], "properties": {"area": area, "summary": summary}}},
        {"name": "haunt_words", "description": "Change the house's title, or the words on a sign (a prop id).",
         "parameters": {"type": "object", "required": ["summary"], "properties": {
            "title": {"type": "string"}, "prop": {"type": "string"}, "text": {"type": "string"}, "summary": summary}}}
    ]})
}

/// What the chat's model is told about the house.
#[must_use]
pub fn guidance() -> &'static str {
    "\nThe Haunted House (the-haunted-house) is changed with the haunt_* tools, one edit a call. Read it with \
haunt_describe first (library true lists every prepared asset and sound). To add a creature or decoration, \
call haunt_place with a library asset; it brings its default trigger and sound. Volume requests (\"turn the \
music down\", \"louder\") are haunt_audio: lower or raise the bus by about a quarter unless a level is given. \
A light or mood request is haunt_light. Every edit is checked; if it's refused, say why in plain words. The \
house the person has open shows the change within a second or two.\n"
}

/// Game Lab's Haunted House as a Labs platform kind.
pub struct Haunt;

/// The intent a haunt tool's edit travels in.
#[must_use]
pub fn intent(tool: &str, args: &Value) -> Change {
    Change::Intent {
        intent: json!({"intent": "haunt", "tool": tool, "args": args}),
    }
}

impl LabKind for Haunt {
    const KIND: &'static str = "haunt";

    fn manifest() -> KindManifest {
        let tools = tools()["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| ToolDecl {
                name: t["name"].as_str().unwrap_or_default().into(),
                description: t["description"].as_str().unwrap_or_default().into(),
                parameters: t["parameters"].clone(),
            })
            .collect();
        KindManifest {
            kind: Self::KIND.into(),
            name: "Game Lab".into(),
            schemas: vec![haunt_rules::KIND.into()],
            tool_prefix: "haunt_".into(),
            doc_arg: "house".into(),
            tools,
            job_types: Vec::new(),
            job_requirements: Default::default(),
            views: vec![View::Playable],
            limits: Limits::default(),
        }
    }

    fn check(content: &Value, _: &Measurements) -> Result<Report, Refusal> {
        let house = read(content)?;
        Ok(Report {
            warnings: Vec::new(),
            facts: json!({"areas": house.areas.len(), "props": house.areas.iter().map(|a| a.props.len()).sum::<usize>()}),
        })
    }

    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        _: &Measurements,
    ) -> Result<Draft, Refusal> {
        let Change::Intent { intent } = change else {
            return Err(refuse(
                "unknown_intent",
                "Change the house with a haunt tool.",
            ));
        };
        if intent["intent"] != "haunt" {
            return Err(refuse(
                "unknown_intent",
                "Change the house with a haunt tool.",
            ));
        }
        let tool = intent["tool"].as_str().unwrap_or_default();
        let edited = edit(&base.content, tool, &intent["args"])?;
        Ok(Draft {
            content: to_doc(&edited.house),
            report: Report {
                warnings: Vec::new(),
                facts: json!({"changes": edited.changes}),
            },
            summary: summary.into(),
            assets: base.assets.clone(),
        })
    }

    fn jobs_for(_: &Version, _: &Want) -> Vec<JobSpec> {
        Vec::new()
    }

    fn card(version: &Version, _: &[JobState], _: &[Artifact]) -> Card {
        Card {
            schema: lab_kit::card::SCHEMA.into(),
            kind: Self::KIND.into(),
            document: version.document.clone(),
            version: version.number,
            title: version.content["title"].as_str().unwrap_or(TITLE).into(),
            summary: version.summary.clone(),
            state: None,
            state_text: None,
            refusal: None,
            view: View::Playable,
            primary: None,
            poster: None,
            strip: Vec::new(),
            compare: None,
            notes: Vec::new(),
            page: Some(format!("{PLAY_PATH}?v={}", version.number)),
            private_local: version.has_private_local(),
            detail: version.report.facts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v1() -> Value {
        original()
    }

    fn version(n: u64, content: Value) -> Version {
        Version {
            document: HAUNTED_HOUSE.into(),
            number: n,
            parent: n.checked_sub(1).filter(|p| *p > 0),
            author: "test".into(),
            summary: String::new(),
            created_unix_ms: 0,
            schema: haunt_rules::KIND.into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: Report {
                warnings: Vec::new(),
                facts: Value::Null,
            },
            assets: Vec::new(),
        }
    }

    #[test]
    fn a_werewolf_in_the_long_hall_that_howls_when_you_get_near() {
        let edited = edit(
            &v1(),
            "haunt_place",
            &json!({"area": "long-hall", "asset": "werewolf", "note": "howls when you get near", "summary": "A werewolf in the long hall"}),
        )
        .unwrap();
        let hall = edited.house.area("long-hall").unwrap();
        let wolf = hall.props.iter().find(|p| p.asset == "werewolf").unwrap();
        let t = hall.triggers.last().unwrap();
        assert!(matches!(&t.when, When::Near { prop, .. } if *prop == wolf.id));
        assert!(t.actions.contains(&Action::Sfx("howl-wolf".into())));
        assert!(
            edited.changes[1].contains("howl-wolf"),
            "{:?}",
            edited.changes
        );
        // Through the platform kind it's the same draft.
        let draft = Haunt::apply(
            &version(1, v1()),
            &intent(
                "haunt_place",
                &json!({"area": "long-hall", "asset": "werewolf", "note": "howls when you get near"}),
            ),
            "x",
            &Value::Null,
        )
        .unwrap();
        assert_eq!(draft.content, to_doc(&edited.house));
        assert_eq!(draft.content["look"]["grain"], json!(0.15));
        assert!(Haunt::check(&draft.content, &Value::Null).is_ok());
    }

    #[test]
    fn turning_the_music_down_lowers_the_music_bus() {
        let edited = edit(
            &v1(),
            "haunt_audio",
            &json!({"music": {"by": -0.25}, "summary": "Music down"}),
        )
        .unwrap();
        assert_eq!(edited.house.audio.music, 0.35);
        assert_eq!(edited.house.audio.master, 0.8);
        let louder = edit(&v1(), "haunt_audio", &json!({"master": {"to": 1.0}})).unwrap();
        assert_eq!(louder.house.audio.master, 1.0);
        assert_eq!(
            edit(&v1(), "haunt_audio", &json!({"music": {"to": 3.0}}))
                .unwrap_err()
                .code,
            "range"
        );
    }

    #[test]
    fn refused_edits_say_why_in_one_sentence() {
        for (tool, args, code) in [
            (
                "haunt_place",
                json!({"area": "long-hall", "asset": "dragon"}),
                "library",
            ),
            (
                "haunt_place",
                json!({"area": "attic", "asset": "werewolf"}),
                "unknown_area",
            ),
            (
                "haunt_remove_area",
                json!({"area": "long-hall"}),
                "cuts_off_exit",
            ),
            (
                "haunt_add_area",
                json!({"next_to": "yard", "wall": "east", "width": 5, "depth": 5, "feeling": "x"}),
                "yard",
            ),
            (
                "haunt_light",
                json!({"area": "parlor", "preset": "disco"}),
                "library",
            ),
            ("haunt_audio", json!({}), "audio"),
            (
                "haunt_light",
                json!({"area": "parlor", "preset": "candle"}),
                "no_change",
            ),
            ("haunt_fly", json!({}), "unknown_tool"),
        ] {
            let r = edit(&v1(), tool, &args).unwrap_err();
            assert_eq!(r.code, code, "{tool} {args}: {}", r.message);
            assert!(r.message.ends_with('.'), "{}", r.message);
            assert!(
                oa_copy::violations(&r.message, &[]).is_empty(),
                "{}",
                r.message
            );
        }
    }

    #[test]
    fn the_light_add_area_and_words_tools_make_checked_houses() {
        let lit = edit(
            &v1(),
            "haunt_light",
            &json!({"area": "parlor", "preset": "jack-o-glow"}),
        )
        .unwrap();
        assert_eq!(
            lit.house.area("parlor").unwrap().light.preset,
            "jack-o-glow"
        );
        let room = edit(
            &v1(),
            "haunt_add_area",
            &json!({"next_to": "parlor", "wall": "west", "kind": "room", "width": 6, "depth": 6, "feeling": "a quiet library, then a book falls"}),
        )
        .unwrap();
        assert_eq!(room.house.areas.len(), 7);
        let titled = edit(&v1(), "haunt_words", &json!({"title": "The Hollow House"})).unwrap();
        assert_eq!(titled.house.title, "The Hollow House");
        let card = Haunt::card(&version(2, to_doc(&titled.house)), &[], &[]);
        assert_eq!(card.page.as_deref(), Some("/games/the-haunted-house?v=2"));
        assert_eq!(card.title, "The Hollow House");
    }

    #[test]
    fn every_tool_is_declared_and_describe_reads() {
        let names: Vec<String> = tools()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names.len(), 8);
        for edit in EDITS {
            assert!(names.iter().any(|n| n == edit), "{edit}");
        }
        assert!(describe(&v1(), false).unwrap().contains("[A] yard"));
        assert!(describe(&v1(), true).unwrap().contains("werewolf"));
    }
}
