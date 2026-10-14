//! The documents: `part.v1` and `assembly.v1` (docs/cad-lab/README.md).
//!
//! A **part** is a feature history: an ordered list of features, each one
//! operation with its parameters, replayed from the top on every build. A
//! feature is one of three shapes:
//!
//! - a **sketch** (`op: "sketch"`): a plane and the 2D geometry drawn on it,
//!   which later features extrude, revolve, sweep or loft;
//! - a **modelling operation** (`op: "solid.extrude"`, `"solid.fillet"`, ...):
//!   one step of the history, referring to earlier sketches and features by
//!   their ids;
//! - a **block** (`op: "block"`): direct operations on solids, run together
//!   as one feature of the history and recomputed with it whenever an
//!   earlier feature or a parameter changes.
//!
//! An **assembly** holds its parts (by id), the instances that place them
//! (each relative to a parent instance or the assembly), the joints that
//! move instances relative to their parents, and the contacts the
//! interference check allows.
//!
//! The documents are plain JSON; [`check_assembly`] says, in plain words,
//! everything wrong with one before it is built. Every length is in
//! millimetres and every angle in degrees.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

pub const PART: &str = "part.v1";
pub const ASSEMBLY: &str = "assembly.v1";

/// Materials and their densities (g/cm³). A part names one; the kernel's
/// own materials keep their names, and the rest are made in the session.
pub const MATERIALS: [(&str, f64); 18] = [
    ("Aluminum", 2.70),
    ("Titanium", 4.43),
    ("Steel", 7.85),
    ("Stainless Steel", 8.00),
    ("Brass", 8.50),
    ("Copper", 8.96),
    ("ABS Plastic", 1.06),
    ("PLA", 1.24),
    ("Nylon", 1.14),
    ("Polycarbonate", 1.20),
    ("Glass", 2.50),
    ("Carbon Fibre", 1.60),
    ("Rubber", 1.15),
    ("Circuit Board", 1.85),
    ("Solar Cell", 2.33),
    ("Gold", 19.30),
    ("Foam", 0.05),
    ("Kapton", 1.42),
];

/// The default material.
pub const DEFAULT_MATERIAL: &str = "Aluminum";

/// A material's density in g/cm³, by name (case doesn't matter).
#[must_use]
pub fn density(material: &str) -> Option<f64> {
    MATERIALS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(material.trim()))
        .map(|(_, d)| *d)
}

/// The modelling operations a part's history may use (besides `sketch`
/// and `block`). They are the kernel's own operation names.
pub const OPERATIONS: [&str; 31] = [
    "solid.extrude",
    "solid.revolve",
    "solid.sweep",
    "solid.loft",
    "solid.pipe",
    "solid.box",
    "solid.cylinder",
    "solid.sphere",
    "solid.torus",
    "solid.coil",
    "solid.fillet",
    "solid.chamfer",
    "solid.shell",
    "solid.draft",
    "solid.hole",
    "solid.combine",
    "solid.pattern.rectangular",
    "solid.pattern.circular",
    "solid.pattern.path",
    "solid.mirror",
    "solid.move",
    "solid.scale",
    "solid.offset_face",
    "solid.press_pull",
    "solid.split_body",
    "solid.emboss",
    "solid.rib",
    "solid.web",
    "solid.remove",
    "solid.thread",
    "appearance.assign",
];

/// The direct operations a block may run.
pub const DIRECT: [&str; 12] = [
    "solid.box",
    "solid.cylinder",
    "solid.sphere",
    "solid.torus",
    "solid.combine",
    "solid.move",
    "solid.scale",
    "solid.mirror",
    "solid.fillet",
    "solid.chamfer",
    "solid.remove",
    "solid.split_body",
];

/// Sketch drawing operations a sketch feature may not use: they read or
/// write files on this computer.
const SKETCH_REFUSED: [&str; 3] = [
    "sketch.insert_dxf",
    "sketch.insert_svg",
    "sketch.export_dxf",
];

/// The joint kinds an assembly may declare, and how many values each
/// moves (angles in degrees, distances in mm).
pub const JOINTS: [(&str, usize); 7] = [
    ("fastened", 0),
    ("revolute", 1),
    ("slider", 1),
    ("cylindrical", 2),
    ("planar", 3),
    ("spherical", 3),
    ("gear", 0),
];

/// Joint kinds this build moves (the rest are declared and checked, and
/// say so plainly when used).
pub const JOINTS_BUILT: [&str; 4] = ["fastened", "revolute", "slider", "cylindrical"];

/// The views a render can take.
pub const VIEWS: [&str; 8] = [
    "hero", "rear", "plan", "front", "side", "iso", "under", "detail",
];

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn vec3(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let mut out = [0.0; 3];
    for (i, x) in a.iter().enumerate() {
        out[i] = x.as_f64().filter(|f| f.is_finite())?;
    }
    Some(out)
}

/// A new empty assembly.
#[must_use]
pub fn new_assembly(id: &str, title: &str) -> Value {
    json!({
        "kind": ASSEMBLY,
        "id": id,
        "title": title,
        "units": "mm",
        "parameters": [],
        "parts": {},
        "instances": [],
        "joints": [],
        "contacts": [],
    })
}

/// A new part with no features.
#[must_use]
pub fn new_part(id: &str, title: &str, material: &str, color: Option<&str>) -> Value {
    let mut part = json!({
        "kind": PART,
        "id": id,
        "title": title,
        "material": material,
        "parameters": [],
        "features": [],
    });
    if let Some(color) = color {
        part["color"] = json!(color);
    }
    part
}

/// A standalone part document as an assembly with one instance of it, so
/// one build path serves both kinds.
#[must_use]
pub fn as_assembly(doc: &Value) -> Value {
    if doc["kind"] == PART {
        let id = doc["id"].as_str().unwrap_or("part");
        let mut assembly = new_assembly(id, doc["title"].as_str().unwrap_or(id));
        assembly["parts"][id] = doc.clone();
        assembly["instances"] =
            json!([{"id": id, "part": id, "at": [0, 0, 0], "rotate": [0, 0, 0]}]);
        assembly
    } else {
        doc.clone()
    }
}

/// Everything wrong with a feature, in plain words.
fn feature_problems(part: &str, feature: &Value, seen: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(object) = feature.as_object() else {
        return vec![format!("A feature of {part} isn't an object.")];
    };
    let id = object.get("id").and_then(Value::as_str).unwrap_or_default();
    if !is_id(id) {
        out.push(format!(
            "A feature of {part} needs an id of lowercase letters, digits, - and _ (got \"{id}\")."
        ));
    } else if seen.contains(id) {
        out.push(format!("{part} has two features called {id}."));
    }
    for key in object.keys() {
        if !matches!(key.as_str(), "id" | "op" | "params" | "suppressed" | "note") {
            out.push(format!("Feature {part}.{id} has an unknown field {key}."));
        }
    }
    let op = object.get("op").and_then(Value::as_str).unwrap_or_default();
    let params = object.get("params").cloned().unwrap_or(json!({}));
    if !params.is_object() {
        out.push(format!("Feature {part}.{id}'s params must be an object."));
        return out;
    }
    if params.get("font").is_some() {
        out.push(format!(
            "Feature {part}.{id}: fonts come from the built-in one; leave font out."
        ));
    }
    match op {
        "sketch" => {
            if params.get("plane").is_none() {
                out.push(format!(
                    "Sketch {part}.{id} needs a plane (XY, XZ, YZ, or {{origin, x_dir, y_dir}})."
                ));
            }
            let draw = params.get("draw").and_then(Value::as_array);
            match draw {
                None => out.push(format!(
                    "Sketch {part}.{id} needs draw: a list of sketch operations like {{\"op\":\"sketch.circle.center\",\"params\":{{...}}}}."
                )),
                Some(steps) => {
                    for step in steps {
                        let sop = step["op"].as_str().unwrap_or_default();
                        if !sop.starts_with("sketch.")
                            || SKETCH_REFUSED.contains(&sop)
                            || matches!(sop, "sketch.create" | "sketch.finish")
                        {
                            out.push(format!(
                                "Sketch {part}.{id} can't use \"{sop}\" (a drawing step is a sketch.* operation such as sketch.rectangle.center, sketch.circle.center, sketch.line, sketch.polygon.inscribed, sketch.slot.overall, sketch.text)."
                            ));
                        }
                    }
                }
            }
        }
        "block" => match params.get("ops").and_then(Value::as_array) {
            Some(ops) if !ops.is_empty() => {
                for step in ops {
                    let sop = step["op"].as_str().unwrap_or_default();
                    if !DIRECT.contains(&sop) {
                        out.push(format!(
                            "Block {part}.{id} can't run \"{sop}\"; a block runs direct operations: {}.",
                            DIRECT.join(", ")
                        ));
                    }
                }
            }
            _ => out.push(format!(
                "Block {part}.{id} needs ops: a list of direct operations."
            )),
        },
        other if OPERATIONS.contains(&other) => {}
        other => out.push(format!(
            "Feature {part}.{id} has an operation this CAD doesn't have: \"{other}\". Use sketch, block, or one of: {}.",
            OPERATIONS.join(", ")
        )),
    }
    out
}

/// Everything wrong with a part, in plain words.
#[must_use]
pub fn part_problems(key: &str, part: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if part["kind"] != PART {
        out.push(format!("Part {key} must have kind \"{PART}\"."));
    }
    if part["id"].as_str() != Some(key) {
        out.push(format!("Part {key}'s id must be \"{key}\"."));
    }
    if !is_id(key) {
        out.push(format!(
            "A part id must be lowercase letters, digits, - and _ (got \"{key}\")."
        ));
    }
    if let Some(object) = part.as_object() {
        for field in object.keys() {
            if !matches!(
                field.as_str(),
                "kind" | "id" | "title" | "material" | "color" | "parameters" | "features" | "note"
            ) {
                out.push(format!("Part {key} has an unknown field {field}."));
            }
        }
    }
    let material = part["material"].as_str().unwrap_or(DEFAULT_MATERIAL);
    if density(material).is_none() {
        out.push(format!(
            "Part {key}'s material \"{material}\" isn't one I know; use one of: {}.",
            MATERIALS.map(|(n, _)| n).join(", ")
        ));
    }
    if let Some(color) = part.get("color").and_then(Value::as_str)
        && !is_color(color)
    {
        out.push(format!(
            "Part {key}'s color must look like #a1b2c3 (got {color})."
        ));
    }
    let mut seen = BTreeSet::new();
    match part["features"].as_array() {
        Some(features) => {
            for feature in features {
                out.extend(feature_problems(key, feature, &seen));
                if let Some(id) = feature["id"].as_str() {
                    seen.insert(id.to_owned());
                }
            }
        }
        None => out.push(format!("Part {key} needs a features list.")),
    }
    out
}

fn parameter_problems(owner: &str, params: &Value, names: &mut BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(list) = params.as_array() else {
        if !params.is_null() {
            out.push(format!("{owner}'s parameters must be a list."));
        }
        return out;
    };
    for p in list {
        let name = p["name"].as_str().unwrap_or_default();
        let valid = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        if !valid {
            out.push(format!(
                "A parameter of {owner} needs a name of letters, digits and _ (got \"{name}\")."
            ));
        } else if !names.insert(name.to_owned()) {
            out.push(format!(
                "Two parameters are called {name}; parameter names are shared by the whole assembly."
            ));
        }
        if !(p["expression"].is_string() || p["expression"].is_number()) {
            out.push(format!(
                "Parameter {name} needs an expression such as \"60 mm\"."
            ));
        }
    }
    out
}

/// Everything wrong with an assembly, in plain words: an empty list when
/// it may be built.
#[must_use]
pub fn check_assembly(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if doc["kind"] != ASSEMBLY {
        return vec![format!("The document must have kind \"{ASSEMBLY}\".")];
    }
    if let Some(object) = doc.as_object() {
        for field in object.keys() {
            if !matches!(
                field.as_str(),
                "kind"
                    | "id"
                    | "title"
                    | "units"
                    | "parameters"
                    | "parts"
                    | "instances"
                    | "joints"
                    | "contacts"
                    | "note"
            ) {
                out.push(format!("The assembly has an unknown field {field}."));
            }
        }
    }
    if doc["units"].as_str().is_some_and(|u| u != "mm") {
        out.push("The assembly's units are mm.".into());
    }
    let mut names = BTreeSet::new();
    out.extend(parameter_problems(
        "The assembly",
        &doc["parameters"],
        &mut names,
    ));
    let empty = Map::new();
    let parts = doc["parts"].as_object().unwrap_or(&empty);
    for (key, part) in parts {
        out.extend(part_problems(key, part));
        out.extend(parameter_problems(
            &format!("Part {key}"),
            &part["parameters"],
            &mut names,
        ));
    }
    let mut instances: BTreeMap<String, Option<String>> = BTreeMap::new();
    for instance in doc["instances"].as_array().into_iter().flatten() {
        let id = instance["id"].as_str().unwrap_or_default();
        if !is_id(id) {
            out.push(format!(
                "An instance needs an id of lowercase letters, digits, - and _ (got \"{id}\")."
            ));
            continue;
        }
        if instances.contains_key(id) {
            out.push(format!("Two instances are called {id}."));
        }
        if let Some(object) = instance.as_object() {
            for field in object.keys() {
                if !matches!(
                    field.as_str(),
                    "id" | "part" | "parent" | "at" | "rotate" | "note"
                ) {
                    out.push(format!("Instance {id} has an unknown field {field}."));
                }
            }
        }
        let part = instance["part"].as_str().unwrap_or_default();
        if !parts.contains_key(part) {
            out.push(format!(
                "Instance {id} places part \"{part}\", which the assembly doesn't have."
            ));
        }
        for key in ["at", "rotate"] {
            if !instance[key].is_null() && vec3(&instance[key]).is_none() {
                out.push(format!(
                    "Instance {id}'s {key} must be three numbers, like [0, 0, 0]."
                ));
            }
        }
        instances.insert(
            id.to_owned(),
            instance["parent"].as_str().map(str::to_owned),
        );
    }
    for (id, parent) in &instances {
        if let Some(parent) = parent {
            if !instances.contains_key(parent) {
                out.push(format!(
                    "Instance {id}'s parent {parent} isn't an instance of the assembly."
                ));
                continue;
            }
            // No loops.
            let mut at = Some(parent.clone());
            let mut steps = 0;
            while let Some(p) = at {
                if &p == id || steps > instances.len() {
                    out.push(format!("Instance {id} is its own ancestor."));
                    break;
                }
                at = instances.get(&p).cloned().flatten();
                steps += 1;
            }
        }
    }
    let mut joint_ids = BTreeSet::new();
    let mut moved = BTreeSet::new();
    for joint in doc["joints"].as_array().into_iter().flatten() {
        let id = joint["id"].as_str().unwrap_or_default();
        if !is_id(id) || !joint_ids.insert(id.to_owned()) {
            out.push(format!(
                "Each joint needs its own id of lowercase letters, digits, - and _ (got \"{id}\")."
            ));
        }
        let kind = joint["type"].as_str().unwrap_or_default();
        let Some((_, values)) = JOINTS.iter().find(|(k, _)| *k == kind) else {
            out.push(format!(
                "Joint {id}'s type \"{kind}\" isn't one of: {}.",
                JOINTS.map(|(k, _)| k).join(", ")
            ));
            continue;
        };
        if !JOINTS_BUILT.contains(&kind) {
            out.push(format!(
                "Joint {id} is {kind}; this CAD Lab moves fastened, revolute, slider and cylindrical joints so far."
            ));
        }
        let child = joint["child"].as_str().unwrap_or_default();
        match instances.get(child) {
            None => out.push(format!(
                "Joint {id}'s child \"{child}\" isn't an instance of the assembly."
            )),
            Some(parent) => {
                if joint["parent"].as_str().map(str::to_owned) != *parent {
                    out.push(format!(
                        "Joint {id}'s parent must be the parent of {child} ({}).",
                        parent.as_deref().unwrap_or("none: the assembly")
                    ));
                }
            }
        }
        if !moved.insert(child.to_owned()) {
            out.push(format!("{child} is the child of two joints."));
        }
        if *values > 0 {
            if vec3(&joint["origin"]).is_none() || vec3(&joint["axis"]).is_none() {
                out.push(format!(
                    "Joint {id} needs an origin and an axis, each three numbers in its parent's frame."
                ));
            }
            if vec3(&joint["axis"]).is_some_and(|a| a.iter().map(|x| x * x).sum::<f64>() < 1e-12) {
                out.push(format!("Joint {id}'s axis can't be zero."));
            }
        }
        let value_list: Vec<f64> = match &joint["value"] {
            Value::Number(n) => n.as_f64().into_iter().collect(),
            Value::Array(a) => a.iter().filter_map(Value::as_f64).collect(),
            _ => Vec::new(),
        };
        if value_list.len() > *values {
            out.push(format!(
                "Joint {id} ({kind}) takes {values} value(s), not {}.",
                value_list.len()
            ));
        }
        if let Some(limits) = joint["limits"].as_array() {
            for (i, limit) in limits.iter().enumerate() {
                let (Some(lo), Some(hi)) = (limit[0].as_f64(), limit[1].as_f64()) else {
                    out.push(format!(
                        "Joint {id}'s limits are [[min, max], ...], one pair per value."
                    ));
                    break;
                };
                if lo > hi {
                    out.push(format!("Joint {id}'s limit {i} has min above max."));
                }
                if let Some(v) = value_list.get(i)
                    && (*v < lo - 1e-9 || *v > hi + 1e-9)
                {
                    out.push(format!(
                        "Joint {id}'s value {v} is outside its limits [{lo}, {hi}]."
                    ));
                }
            }
        }
    }
    for pair in doc["contacts"].as_array().into_iter().flatten() {
        let ok = pair.as_array().is_some_and(|p| {
            p.len() == 2
                && p.iter()
                    .all(|x| x.as_str().is_some_and(|s| instances.contains_key(s)))
        });
        if !ok {
            out.push(format!(
                "A contact is a pair of instance ids, like [\"wheel_fl\", \"rocker_l\"] (got {pair})."
            ));
        }
    }
    out
}

/// The joint values as a list (a single number is one value).
#[must_use]
pub fn joint_values(joint: &Value) -> Vec<f64> {
    match &joint["value"] {
        Value::Number(n) => n.as_f64().into_iter().collect(),
        Value::Array(a) => a.iter().filter_map(Value::as_f64).collect(),
        _ => Vec::new(),
    }
}

/// An instance's placement: `at` (mm) and `rotate` (degrees about X, then
/// Y, then Z, fixed axes).
#[must_use]
pub fn placement(instance: &Value) -> ([f64; 3], [f64; 3]) {
    (
        vec3(&instance["at"]).unwrap_or([0.0; 3]),
        vec3(&instance["rotate"]).unwrap_or([0.0; 3]),
    )
}

/// A three-number field.
#[must_use]
pub fn triple(v: &Value) -> Option<[f64; 3]> {
    vec3(v)
}

/// The JSON Schemas of both kinds.
#[must_use]
pub fn schema() -> Value {
    let vec3 = json!({"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3});
    let id = json!({"type": "string", "pattern": "^[a-z][a-z0-9_-]{0,47}$"});
    let parameters = json!({"type": "array", "items": {"type": "object", "required": ["name", "expression"],
        "properties": {"name": {"type": "string"}, "expression": {"type": ["string", "number"]}, "description": {"type": "string"}}}});
    let mut ops: Vec<&str> = vec!["sketch", "block"];
    ops.extend(OPERATIONS);
    let part = json!({
        "$id": "https://openagents.com/schemas/cad/part.v1.json",
        "title": "part.v1: a part as a feature history",
        "type": "object",
        "required": ["kind", "id", "features"],
        "additionalProperties": false,
        "properties": {
            "kind": {"const": PART},
            "id": id,
            "title": {"type": "string"},
            "material": {"enum": MATERIALS.map(|(n, _)| n)},
            "color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$"},
            "note": {"type": "string"},
            "parameters": parameters,
            "features": {"type": "array", "items": {
                "type": "object", "required": ["id", "op"], "additionalProperties": false,
                "properties": {
                    "id": id,
                    "op": {"enum": ops},
                    "params": {"type": "object", "description": "the operation's parameters; a sketch has plane, offset? and draw: [{op: sketch.*, params}]; a block has ops: [{op, params}] (direct operations)"},
                    "suppressed": {"type": "boolean"},
                    "note": {"type": "string"}
                }
            }}
        }
    });
    let assembly = json!({
        "$id": "https://openagents.com/schemas/cad/assembly.v1.json",
        "title": "assembly.v1: parts, their instances, joints and contacts",
        "type": "object",
        "required": ["kind", "id", "parts", "instances"],
        "additionalProperties": false,
        "properties": {
            "kind": {"const": ASSEMBLY},
            "id": id,
            "title": {"type": "string"},
            "units": {"const": "mm"},
            "note": {"type": "string"},
            "parameters": parameters,
            "parts": {"type": "object", "additionalProperties": {"$ref": "part.v1.json"}},
            "instances": {"type": "array", "items": {
                "type": "object", "required": ["id", "part"], "additionalProperties": false,
                "properties": {"id": id, "part": id, "parent": id, "at": vec3, "rotate": vec3, "note": {"type": "string"}}
            }},
            "joints": {"type": "array", "items": {
                "type": "object", "required": ["id", "type", "child"],
                "properties": {
                    "id": id,
                    "type": {"enum": JOINTS.map(|(k, _)| k)},
                    "parent": id, "child": id,
                    "origin": vec3, "axis": vec3,
                    "value": {"type": ["number", "array"]},
                    "limits": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}},
                    "ratio": {"type": "number", "description": "gear: turns of the child per turn of the driver"},
                    "driver": id
                }
            }},
            "contacts": {"type": "array", "items": {"type": "array", "items": id, "minItems": 2, "maxItems": 2}}
        }
    });
    json!({"part.v1": part, "assembly.v1": assembly})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rover() -> Value {
        let mut a = new_assembly("rover", "Rover");
        a["parts"]["wheel"] = new_part("wheel", "Wheel", "Aluminum", Some("#333333"));
        a["parts"]["wheel"]["features"] = json!([
            {"id": "tyre", "op": "solid.cylinder", "params": {"radius": 60, "height": 40, "axis": "Y"}},
            {"id": "hub", "op": "sketch", "params": {"plane": "XZ", "draw": [{"op": "sketch.circle.center", "params": {"center": [0, 0], "radius": 10}}]}}
        ]);
        a["instances"] = json!([
            {"id": "body", "part": "wheel"},
            {"id": "wheel_fl", "part": "wheel", "parent": "body", "at": [200, 150, 0]}
        ]);
        a["joints"] = json!([{"id": "spin", "type": "revolute", "parent": "body", "child": "wheel_fl",
            "origin": [200, 150, 0], "axis": [0, 1, 0], "value": 30, "limits": [[-360, 360]]}]);
        a
    }

    #[test]
    fn a_good_assembly_has_no_problems() {
        assert_eq!(check_assembly(&rover()), Vec::<String>::new());
    }

    #[test]
    fn problems_are_named_plainly() {
        let mut a = rover();
        a["instances"][1]["part"] = json!("tyre");
        a["parts"]["wheel"]["features"][0]["op"] = json!("solid.explode");
        a["parts"]["wheel"]["material"] = json!("Unobtainium");
        a["joints"][0]["value"] = json!(400);
        a["contacts"] = json!([["body", "nobody"]]);
        let text = check_assembly(&a).join("\n");
        for says in [
            "doesn't have",
            "solid.explode",
            "Unobtainium",
            "outside its limits",
            "pair of instance ids",
        ] {
            assert!(text.contains(says), "{says}: {text}");
        }
    }

    #[test]
    fn sketches_cant_touch_files_and_loops_are_refused() {
        let mut a = rover();
        a["parts"]["wheel"]["features"][1]["params"]["draw"][0]["op"] = json!("sketch.insert_svg");
        a["instances"][0]["parent"] = json!("wheel_fl");
        let text = check_assembly(&a).join("\n");
        assert!(text.contains("sketch.insert_svg"), "{text}");
        assert!(text.contains("own ancestor"), "{text}");
    }

    #[test]
    fn a_part_document_builds_as_a_one_instance_assembly() {
        let part = new_part("bracket", "Bracket", "Steel", None);
        let a = as_assembly(&part);
        assert_eq!(a["instances"][0]["part"], "bracket");
        assert!(check_assembly(&a).is_empty());
        assert!(schema()["assembly.v1"]["properties"]["joints"].is_object());
    }
}
