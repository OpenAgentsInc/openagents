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
pub const MATERIALS: [(&str, f64); 19] = [
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
    ("PVC", 1.38),
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

/// The direct operations a block may run: every modelling operation.
pub const DIRECT: [&str; 31] = OPERATIONS;

/// The constraints a sketch may hold between its entities (curve ids such
/// as `l1`, point ids such as `p2`, or `l1.start`, `c1.center`, and `x` or
/// `y` for the sketch's own axes).
pub const CONSTRAINTS: [&str; 14] = [
    "coincident",
    "tangent",
    "equal",
    "parallel",
    "perpendicular",
    "concentric",
    "collinear",
    "fix",
    "midpoint",
    "symmetry",
    "curvature",
    "polygon",
    "horizontal",
    "vertical",
];

/// A sketch constraint `{"type": "tangent", "a": "l1", "b": "a1"}` as the
/// kernel step that adds it.
///
/// # Errors
/// An unknown type.
pub fn constraint_step(c: &Value) -> Result<(String, Value), String> {
    let kind = c["type"].as_str().unwrap_or_default();
    if !CONSTRAINTS.contains(&kind) {
        return Err(format!(
            "a constraint's type is one of {} (got \"{kind}\")",
            CONSTRAINTS.join(", ")
        ));
    }
    let mut params = c.clone();
    if let Some(o) = params.as_object_mut() {
        o.remove("type");
    }
    Ok(match kind {
        "horizontal" | "vertical" => {
            params["mode"] = json!(kind);
            ("sketch.constraint.horizontal_vertical".into(), params)
        }
        other => (format!("sketch.constraint.{other}"), params),
    })
}

/// The most copies one repeated block step may make.
pub const MAX_REPEAT: usize = 500;

/// A block step's repeat: `{"var": "i", "from": 0, "to": 9, "by": 1}` runs
/// the step once for each value, with `{i}` in its parameters replaced by
/// the value (so `"{i} * 12"` or `"5 * sin({i} * 36)"` become expressions
/// the kernel works out). Returns the values, or why it can't.
///
/// # Errors
/// A repeat that isn't well formed or makes too many copies.
pub fn repeat_values(repeat: &Value) -> Result<(String, Vec<f64>), String> {
    let var = repeat["var"].as_str().unwrap_or("i");
    if var.is_empty() || !var.chars().all(|c| c.is_ascii_alphabetic() || c == '_') {
        return Err(format!(
            "a repeat's var is a name of letters (got \"{var}\")"
        ));
    }
    let (Some(from), Some(to)) = (repeat["from"].as_f64(), repeat["to"].as_f64()) else {
        return Err("a repeat needs from and to (numbers; to is included)".into());
    };
    let by = repeat["by"].as_f64().unwrap_or(1.0);
    if !(by.is_finite() && by != 0.0 && from.is_finite() && to.is_finite())
        || (to - from) * by < 0.0
    {
        return Err("a repeat's by must step from toward to".into());
    }
    let count = ((to - from) / by + 1e-9).floor() as usize + 1;
    if count > MAX_REPEAT {
        return Err(format!(
            "a repeat makes {count} copies; {MAX_REPEAT} is the most"
        ));
    }
    Ok((
        var.to_owned(),
        (0..count).map(|k| from + by * k as f64).collect(),
    ))
}

/// `params` with `{var}` replaced by `value` in every string.
#[must_use]
pub fn substitute(params: &Value, var: &str, value: f64) -> Value {
    let token = format!("{{{var}}}");
    let text = if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    };
    match params {
        Value::String(s) if s.contains(&token) => {
            let wrapped = if value < 0.0 {
                format!("({text})")
            } else {
                text.clone()
            };
            let replaced = s.replace(&token, &wrapped);
            // A plain number stays a number (an id or a count, not an expression).
            if s == &token {
                serde_json::Number::from_f64(value).map_or(Value::String(replaced), Value::Number)
            } else {
                Value::String(replaced)
            }
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| substitute(v, var, value)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, var, value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

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

/// Joint kinds this build moves: all of them.
///
/// - `planar`: the child slides in the plane through `origin` normal to
///   `axis` and turns about `axis`; values `[u mm, v mm, degrees]`, where
///   `u` runs along `x_dir` (or a fixed perpendicular to the axis) and `v`
///   along `axis × u`.
/// - `spherical`: the child turns about `origin`; values `[rx, ry, rz]`
///   degrees about the parent's X, then Y, then Z.
/// - `gear`: the child turns about `axis` through `origin` by `ratio` times
///   its `driver` joint's angle (a revolute, cylindrical or gear joint), or
///   by `ratio` degrees per mm of a slider (a rack and pinion).
pub const JOINTS_BUILT: [&str; 7] = [
    "fastened",
    "revolute",
    "slider",
    "cylindrical",
    "planar",
    "spherical",
    "gear",
];

/// The views a render can take.
pub const VIEWS: [&str; 8] = [
    "hero", "rear", "plan", "front", "side", "iso", "under", "detail",
];

/// How a part's surface takes the light: glossy moulded plastic, a satin
/// finish, or matte. Exports carry it (glTF roughness) and the card's view
/// shows the highlight; without one a part keeps the plain look.
pub const FINISHES: [(&str, f64); 3] = [("gloss", 0.18), ("satin", 0.45), ("matte", 0.85)];

/// A finish's glTF roughness, by name.
#[must_use]
pub fn roughness(finish: &str) -> Option<f64> {
    FINISHES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(finish.trim()))
        .map(|(_, r)| *r)
}

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
/// one build path serves both kinds; an assembly with subassembly
/// templates comes back [`flatten`]ed.
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
        flatten(doc).unwrap_or_else(|_| doc.clone())
    }
}

/// Fields a subassembly template may have.
const TEMPLATE_FIELDS: [&str; 6] = ["title", "instances", "joints", "contacts", "source", "note"];

/// An assembly with its subassembly templates (`assemblies`) expanded.
///
/// Parts are part templates and `assemblies` are subassembly templates:
/// each a set of instances, joints and contacts of its own, which may place
/// parts and other subassemblies. An instance with `assembly: "bogie"`
/// places one copy of that template: it becomes a **group** (a frame with
/// no solid of its own, `group: "bogie"`) and the template's instances
/// become its children, named by path (`bogie_l.wheel`). The template's
/// top-level instances hang from the group, its joints and contacts are
/// renamed the same way, and a joint of the outer assembly can reach any
/// of them by path. Nested templates expand the same way, to any depth.
///
/// # Errors
/// A template that doesn't exist, isn't well formed, or contains itself.
pub fn flatten(doc: &Value) -> Result<Value, Vec<String>> {
    let templates = doc["assemblies"].as_object();
    let mut out = doc.clone();
    if let Some(o) = out.as_object_mut() {
        o.remove("assemblies");
    }
    let mut problems = Vec::new();
    for (key, template) in templates.into_iter().flatten() {
        if !is_id(key) {
            problems.push(format!(
                "A subassembly id must be lowercase letters, digits, - and _ (got \"{key}\")."
            ));
        }
        match template.as_object() {
            Some(fields) => {
                for field in fields.keys() {
                    if !TEMPLATE_FIELDS.contains(&field.as_str()) {
                        problems.push(format!("Subassembly {key} has an unknown field {field}."));
                    }
                }
                if !template["instances"].is_array() {
                    problems.push(format!("Subassembly {key} needs a list of instances."));
                }
            }
            None => problems.push(format!("Subassembly {key} isn't an object.")),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut instances = Vec::new();
    let mut joints = doc["joints"].as_array().cloned().unwrap_or_default();
    let mut contacts = doc["contacts"].as_array().cloned().unwrap_or_default();
    let empty = Map::new();
    let templates = templates.unwrap_or(&empty);
    for instance in doc["instances"].as_array().into_iter().flatten() {
        expand(
            instance,
            "",
            None,
            templates,
            &mut Vec::new(),
            &mut instances,
            &mut joints,
            &mut contacts,
            &mut problems,
        );
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    out["instances"] = Value::Array(instances);
    if doc.get("joints").is_some() || !joints.is_empty() {
        out["joints"] = Value::Array(joints);
    }
    if doc.get("contacts").is_some() || !contacts.is_empty() {
        out["contacts"] = Value::Array(contacts);
    }
    Ok(out)
}

/// One instance of a template (or the outer assembly) into the flat list,
/// under `prefix` (`"bogie_l."`), its parent defaulting to `group`.
#[allow(clippy::too_many_arguments)]
fn expand(
    instance: &Value,
    prefix: &str,
    group: Option<&str>,
    templates: &Map<String, Value>,
    stack: &mut Vec<String>,
    instances: &mut Vec<Value>,
    joints: &mut Vec<Value>,
    contacts: &mut Vec<Value>,
    problems: &mut Vec<String>,
) {
    let path = |id: &str| format!("{prefix}{id}");
    let mut flat = instance.clone();
    let id = instance["id"].as_str().unwrap_or_default();
    flat["id"] = json!(path(id));
    match instance["parent"].as_str() {
        Some(parent) => flat["parent"] = json!(path(parent)),
        None => {
            if let Some(group) = group {
                flat["parent"] = json!(group);
            }
        }
    }
    let Some(template_id) = instance["assembly"].as_str() else {
        instances.push(flat);
        return;
    };
    let Some(template) = templates.get(template_id) else {
        problems.push(format!(
            "Instance {} places subassembly \"{template_id}\", which the assembly doesn't have.",
            path(id)
        ));
        return;
    };
    if stack.iter().any(|t| t == template_id) {
        problems.push(format!(
            "Subassembly {template_id} contains itself ({} → {template_id}).",
            stack.join(" → ")
        ));
        return;
    }
    if instance.get("part").is_some() {
        problems.push(format!(
            "Instance {} places both a part and a subassembly; give one.",
            path(id)
        ));
    }
    if let Some(o) = flat.as_object_mut() {
        o.remove("assembly");
    }
    flat["group"] = json!(template_id);
    let me = path(id);
    instances.push(flat);
    stack.push(template_id.to_owned());
    let inner = format!("{me}.");
    for child in template["instances"].as_array().into_iter().flatten() {
        expand(
            child,
            &inner,
            Some(&me),
            templates,
            stack,
            instances,
            joints,
            contacts,
            problems,
        );
    }
    for joint in template["joints"].as_array().into_iter().flatten() {
        let mut j = joint.clone();
        for key in ["id", "child", "driver"] {
            if let Some(s) = joint[key].as_str() {
                j[key] = json!(format!("{inner}{s}"));
            }
        }
        j["parent"] = match joint["parent"].as_str() {
            Some(p) => json!(format!("{inner}{p}")),
            None => json!(me),
        };
        joints.push(j);
    }
    for pair in template["contacts"].as_array().into_iter().flatten() {
        let mapped: Vec<Value> = pair
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| {
                x.as_str()
                    .map_or(x.clone(), |s| json!(format!("{inner}{s}")))
            })
            .collect();
        contacts.push(Value::Array(mapped));
    }
    stack.pop();
}

/// An instance or joint id: an id, or a path of ids through subassemblies
/// (`bogie_l.wheel`).
fn is_path(s: &str) -> bool {
    s.len() <= 200 && s.split('.').all(is_id)
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
            for c in params["constraints"].as_array().into_iter().flatten() {
                if let Err(why) = constraint_step(c) {
                    out.push(format!("Sketch {part}.{id}: {why}."));
                }
            }
            for d in params["dimensions"].as_array().into_iter().flatten() {
                let ok = d["entities"]
                    .as_array()
                    .is_some_and(|e| (1..=2).contains(&e.len()) && e.iter().all(Value::is_string));
                if !ok {
                    out.push(format!(
                        "Sketch {part}.{id}: a dimension names one or two entities, like {{\"entities\": [\"l1\"], \"value\": \"width\"}}."
                    ));
                }
            }
            for key in ["constraints", "dimensions"] {
                if !params[key].is_null() && !params[key].is_array() {
                    out.push(format!("Sketch {part}.{id}'s {key} must be a list."));
                }
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
                    for key in step.as_object().into_iter().flatten().map(|(k, _)| k) {
                        if !matches!(key.as_str(), "op" | "params" | "repeat" | "note") {
                            out.push(format!(
                                "A step of block {part}.{id} has an unknown field {key} (a step is {{op, params, repeat?}})."
                            ));
                        }
                    }
                    if !step["repeat"].is_null()
                        && let Err(why) = repeat_values(&step["repeat"])
                    {
                        out.push(format!("Block {part}.{id}: {why}."));
                    }
                }
                for key in params.as_object().into_iter().flatten().map(|(k, _)| k) {
                    if !matches!(key.as_str(), "ops" | "inputs") {
                        out.push(format!(
                            "Block {part}.{id} has an unknown param {key} (a block has ops and inputs)."
                        ));
                    }
                }
                if params["inputs"]
                    .as_array()
                    .is_some_and(|l| l.iter().any(|i| !i.is_string()))
                    || !(params["inputs"].is_null() || params["inputs"].is_array())
                {
                    out.push(format!(
                        "Block {part}.{id}'s inputs are a list of names: earlier features and parameters."
                    ));
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
                "kind"
                    | "id"
                    | "title"
                    | "material"
                    | "color"
                    | "finish"
                    | "parameters"
                    | "features"
                    | "source"
                    | "note"
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
    if let Some(finish) = part.get("finish").and_then(Value::as_str)
        && roughness(finish).is_none()
    {
        out.push(format!(
            "Part {key}'s finish must be one of {} (got {finish}).",
            FINISHES.map(|(n, _)| n).join(", ")
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
    let flat = match flatten(doc) {
        Ok(flat) => flat,
        Err(problems) => return problems,
    };
    let doc = &flat;
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
                    | "assemblies"
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
    // A block's declared inputs: earlier features of its part, or parameters.
    for (key, part) in parts {
        let mut earlier = BTreeSet::new();
        for f in part["features"].as_array().into_iter().flatten() {
            let fid = f["id"].as_str().unwrap_or_default();
            if f["op"] == "block" {
                for input in f["params"]["inputs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    if !earlier.contains(input) && !names.contains(input) {
                        out.push(format!(
                            "Block {key}.{fid} reads {input}, which isn't an earlier feature of {key} or a parameter."
                        ));
                    }
                }
            }
            earlier.insert(fid);
        }
    }
    let mut instances: BTreeMap<String, Option<String>> = BTreeMap::new();
    for instance in doc["instances"].as_array().into_iter().flatten() {
        let id = instance["id"].as_str().unwrap_or_default();
        if !is_path(id) {
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
                    "id" | "part" | "group" | "parent" | "at" | "rotate" | "note"
                ) {
                    out.push(format!("Instance {id} has an unknown field {field}."));
                }
            }
        }
        let part = instance["part"].as_str().unwrap_or_default();
        if instance.get("group").is_some() {
            if instance.get("part").is_some() {
                out.push(format!(
                    "Instance {id} is a group (a frame for other instances); it places no part."
                ));
            }
        } else if !parts.contains_key(part) {
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
        if !is_path(id) || !joint_ids.insert(id.to_owned()) {
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
        if kind == "spherical" {
            if vec3(&joint["origin"]).is_none() {
                out.push(format!(
                    "Joint {id} needs an origin: the ball's centre, three numbers in its parent's frame."
                ));
            }
        } else if *values > 0 || kind == "gear" {
            if vec3(&joint["origin"]).is_none() || vec3(&joint["axis"]).is_none() {
                out.push(format!(
                    "Joint {id} needs an origin and an axis, each three numbers in its parent's frame."
                ));
            }
            if vec3(&joint["axis"]).is_some_and(|a| a.iter().map(|x| x * x).sum::<f64>() < 1e-12) {
                out.push(format!("Joint {id}'s axis can't be zero."));
            }
        }
        if kind == "planar"
            && !joint["x_dir"].is_null()
            && vec3(&joint["x_dir"]).is_none_or(|x| {
                let a = vec3(&joint["axis"]).unwrap_or([0.0, 0.0, 1.0]);
                let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
                let along = (x[0] * a[0] + x[1] * a[1] + x[2] * a[2]) / n;
                x.iter().map(|c| c * c).sum::<f64>() - along * along < 1e-12
            })
        {
            out.push(format!(
                "Joint {id}'s x_dir must be three numbers not along its axis."
            ));
        }
        if kind == "gear" {
            if !joint["ratio"]
                .as_f64()
                .is_some_and(|r| r.is_finite() && r != 0.0)
            {
                out.push(format!(
                    "Joint {id} (gear) needs a ratio: turns of the child per turn of its driver (negative for meshing gears), or degrees per mm of a slider."
                ));
            }
            if !joint_values(joint).is_empty() {
                out.push(format!(
                    "Joint {id} (gear) takes no value; it follows its driver."
                ));
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
    out.extend(gear_problems(doc));
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

/// What's wrong with the gear joints' drivers: each names another joint
/// that turns or slides, and no gear drives itself round a loop.
fn gear_problems(doc: &Value) -> Vec<String> {
    let joints: BTreeMap<&str, &Value> = doc["joints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|j| Some((j["id"].as_str()?, j)))
        .collect();
    let mut out = Vec::new();
    for (id, joint) in &joints {
        if joint["type"] != "gear" {
            continue;
        }
        let mut at = *id;
        for step in 0..=joints.len() {
            let Some(driver) = joints[at]["driver"].as_str() else {
                out.push(format!(
                    "Joint {at} (gear) needs a driver: the id of the joint that turns it."
                ));
                break;
            };
            let Some(next) = joints.get(driver) else {
                out.push(format!(
                    "Joint {at}'s driver {driver} isn't a joint of the assembly."
                ));
                break;
            };
            match next["type"].as_str().unwrap_or_default() {
                "gear" if driver == *id || step == joints.len() => {
                    out.push(format!("Joint {id} drives itself through a loop of gears."));
                    break;
                }
                "gear" => at = driver,
                "revolute" | "cylindrical" | "slider" => break,
                other => {
                    out.push(format!(
                        "Joint {at}'s driver {driver} is {other}; a gear follows a revolute, cylindrical, slider or gear joint."
                    ));
                    break;
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A joint's angle in degrees (or, for a slider, its travel in mm), with a
/// gear's derived from its driver: what turns a gear it drives.
#[must_use]
pub fn driven_value(joints: &[Value], joint: &Value) -> f64 {
    let mut at = joint;
    let mut ratio = 1.0;
    for _ in 0..=joints.len() {
        if at["type"] != "gear" {
            return ratio * joint_values(at).first().copied().unwrap_or(0.0);
        }
        ratio *= at["ratio"].as_f64().unwrap_or(1.0);
        let Some(next) = at["driver"]
            .as_str()
            .and_then(|d| joints.iter().find(|j| j["id"] == d))
        else {
            return 0.0;
        };
        at = next;
    }
    0.0
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
    let path =
        json!({"type": "string", "pattern": "^[a-z][a-z0-9_-]{0,47}(\\.[a-z][a-z0-9_-]{0,47})*$"});
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
            "finish": {"enum": FINISHES.map(|(n, _)| n)},
            "note": {"type": "string"}, "source": {"type": "object", "description": "the document and version this part template was copied from"},
            "parameters": parameters,
            "features": {"type": "array", "items": {
                "type": "object", "required": ["id", "op"], "additionalProperties": false,
                "properties": {
                    "id": id,
                    "op": {"enum": ops},
                    "params": {"type": "object", "description": "the operation's parameters; a sketch has plane, offset?, draw: [{op: sketch.*, params}], constraints?: [{type, ...entities}] and dimensions?: [{entities, value?, type?}]; a block has ops: [{op, params, repeat?: {var, from, to, by?}}] (direct operations; {var} in a param is replaced by each value) and inputs?: [earlier features and parameters it reads]"},
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
            "assemblies": {"type": "object", "description": "subassembly templates by id; an instance with assembly places a copy, its instances named by path (copy.inner)",
                "additionalProperties": {"type": "object", "required": ["instances"], "additionalProperties": false, "properties": {
                    "title": {"type": "string"}, "note": {"type": "string"}, "source": {"type": "object"},
                    "instances": {"type": "array"}, "joints": {"type": "array"}, "contacts": {"type": "array"}}}},
            "instances": {"type": "array", "items": {
                "type": "object", "required": ["id"], "additionalProperties": false,
                "properties": {"id": path, "part": id, "assembly": id, "group": id, "parent": path, "at": vec3, "rotate": vec3, "note": {"type": "string"}}
            }},
            "joints": {"type": "array", "items": {
                "type": "object", "required": ["id", "type", "child"],
                "properties": {
                    "id": path,
                    "type": {"enum": JOINTS.map(|(k, _)| k)},
                    "parent": path, "child": path,
                    "origin": vec3, "axis": vec3, "x_dir": vec3,
                    "value": {"type": ["number", "array"]},
                    "limits": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}},
                    "ratio": {"type": "number", "description": "gear: turns of the child per turn of the driver"},
                    "driver": path
                }
            }},
            "contacts": {"type": "array", "items": {"type": "array", "items": path, "minItems": 2, "maxItems": 2}}
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
