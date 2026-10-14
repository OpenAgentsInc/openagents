//! Building a document: replay every part's history in the kernel, place
//! the instances, move the joints, and read back the solids.
//!
//! The kernel is SolidCraft's (on Truck), pinned in `Cargo.toml`; this is
//! the only module that talks to it, so callers see our documents and
//! [`Built`], never its types. Each part is built once, in a component of
//! its own at the origin; each instance is a placement of that component,
//! moved to where the assembly puts it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use solidcraft_engine::Session;
use solidcraft_engine::geom::{Mesh, Vec3};

use crate::doc;
use crate::xf::Xf;

/// One solid of the built assembly, where it is in the world.
#[derive(Clone)]
pub struct Solid {
    /// The instance it belongs to.
    pub instance: String,
    pub part: String,
    /// The kernel's name for it (unique in the build).
    pub body: String,
    pub color: [u8; 3],
    /// Its part's finish (`gloss`, `satin`, `matte`), when it has one.
    pub finish: Option<String>,
    /// Triangles in world millimetres.
    pub mesh: Arc<Mesh>,
    /// Whether every shell of it closes.
    pub closed: bool,
}

/// A built document.
pub struct Built {
    pub session: Session,
    pub solids: Vec<Solid>,
    /// Instance id → its world placement.
    pub placements: BTreeMap<String, Xf>,
    /// Instance id → its kernel placement (occurrence) id.
    pub occurrences: BTreeMap<String, u64>,
    /// Things worth saying that don't stop the build (a kernel retry, a
    /// part nothing places).
    pub warnings: Vec<String>,
}

fn run(s: &mut Session, op: &str, params: &Value) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let result = s.execute(op, params).map_err(|e| e.to_string());
    // `CAD_TRACE=1` prints each kernel step and how long it took.
    if std::env::var_os("CAD_TRACE").is_some() {
        eprintln!(
            "cad: {op} {} ms {}",
            started.elapsed().as_millis(),
            params.get("name").and_then(Value::as_str).unwrap_or("")
        );
    }
    result
}

/// The kernel name of a feature, sketch or body of `part`.
#[must_use]
pub fn scoped(part: &str, name: &str) -> String {
    if name.starts_with(&format!("{part}.")) {
        name.to_owned()
    } else {
        format!("{part}.{name}")
    }
}

/// A feature's parameters with its references to sketches, features and
/// bodies of the same part turned into kernel names.
fn scope_params(part: &str, params: &Value) -> Value {
    let mut out = params.clone();
    let Some(object) = out.as_object_mut() else {
        return out;
    };
    for key in [
        "sketch",
        "path_sketch",
        "body",
        "target",
        "body_name",
        "face_body",
    ] {
        if let Some(Value::String(name)) = object.get(key) {
            let scoped = scoped(part, name);
            object.insert(key.into(), json!(scoped));
        }
    }
    for key in ["features", "bodies", "targets", "tools"] {
        if let Some(Value::Array(names)) = object.get(key) {
            let scoped: Vec<Value> = names
                .iter()
                .map(|n| match n {
                    Value::String(s) => json!(scoped(part, s)),
                    other => other.clone(),
                })
                .collect();
            object.insert(key.into(), Value::Array(scoped));
        }
    }
    if let Some(Value::Array(sections)) = object.get_mut("sections") {
        for section in sections {
            if let Some(Value::String(name)) = section.get("sketch").cloned() {
                section["sketch"] = json!(scoped(part, &name));
            }
        }
    }
    if let Some(plane) = object.get_mut("plane")
        && let Some(Value::String(body)) = plane.get("body").cloned()
    {
        plane["body"] = json!(scoped(part, &body));
    }
    // Edge and face picks with a body name.
    for key in ["edges", "faces"] {
        if let Some(Value::Array(picks)) = object.get_mut(key) {
            for pick in picks {
                if let Some(Value::String(body)) = pick.get("body").cloned() {
                    pick["body"] = json!(scoped(part, &body));
                }
            }
        }
    }
    out
}

/// Operations that make a new body unless told to join or cut.
const MAKERS: [&str; 10] = [
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
];

/// Runs one feature of `part`; the error names the feature.
fn feature(s: &mut Session, part: &str, f: &Value) -> Result<(), String> {
    let id = f["id"].as_str().unwrap_or_default();
    let op = f["op"].as_str().unwrap_or_default();
    let params = f.get("params").cloned().unwrap_or(json!({}));
    let fail = |e: String| format!("{part}.{id} ({op}) failed: {e}");
    match op {
        "sketch" => {
            let mut create = json!({"plane": params["plane"].clone(), "name": scoped(part, id)});
            if let Some(offset) = params.get("offset") {
                create["offset"] = offset.clone();
            }
            let create = scope_params(part, &create);
            run(s, "sketch.create", &create).map_err(&fail)?;
            for (i, step) in params["draw"].as_array().into_iter().flatten().enumerate() {
                let sop = step["op"].as_str().unwrap_or_default();
                let sparams = step.get("params").cloned().unwrap_or(json!({}));
                if let Err(e) = run(s, sop, &sparams) {
                    let _ = run(s, "sketch.finish", &json!({}));
                    return Err(fail(format!("drawing step {} ({sop}): {e}", i + 1)));
                }
            }
            run(s, "sketch.finish", &json!({})).map_err(&fail)?;
        }
        "block" => {
            for (i, step) in params["ops"].as_array().into_iter().flatten().enumerate() {
                let sop = step["op"].as_str().unwrap_or_default();
                let mut sparams =
                    scope_params(part, &step.get("params").cloned().unwrap_or(json!({})));
                if sparams.get("name").is_none() {
                    sparams["name"] = json!(scoped(part, &format!("{id}-{}", i + 1)));
                }
                run(s, sop, &sparams)
                    .map_err(|e| fail(format!("direct step {} ({sop}): {e}", i + 1)))?;
            }
        }
        _ => {
            let mut p = scope_params(part, &params);
            if p.get("name").is_none() {
                p["name"] = json!(scoped(part, id));
            }
            let makes = MAKERS.contains(&op)
                && matches!(
                    p.get("operation").and_then(Value::as_str),
                    None | Some("new")
                );
            if makes && p.get("body_name").is_none() {
                p["body_name"] = json!(scoped(part, id));
            }
            if op == "appearance.assign" && p.get("bodies").is_none() && p.get("faces").is_none() {
                return Err(fail("name the bodies or faces to colour".into()));
            }
            run(s, op, &p).map_err(&fail)?;
        }
    }
    Ok(())
}

/// Operations whose new solids are copies of others.
const COPIES: [&str; 4] = [
    "solid.pattern.circular",
    "solid.pattern.rectangular",
    "solid.pattern.path",
    "solid.mirror",
];

/// Every solid's volume (mm³), by name.
fn volumes(s: &Session) -> BTreeMap<String, f64> {
    let state = s.world_state();
    state
        .bodies
        .iter()
        .map(|b| (b.name.clone(), crate::mass::integrals(&b.mesh()).volume))
        .collect()
}

/// The solids a pattern or mirror feature made, each with the solid it
/// copies: the kernel names copies anew ("Body7") and doesn't carry their
/// source's colour, so a patterned blue strut would come out the part's
/// red. A copy is matched to the source (the feature's `bodies`, or the
/// solids its `features` made) whose volume is nearest to its own: a copy
/// is congruent to its source.
fn copies_made(
    part: &str,
    params: &Value,
    before: &BTreeMap<String, f64>,
    after: &BTreeMap<String, f64>,
) -> Vec<(String, String)> {
    let named = |key: &str| -> Vec<String> {
        params[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|n| scoped(part, n))
            .collect()
    };
    let sources: Vec<(String, f64)> = named("bodies")
        .into_iter()
        .chain(named("features"))
        .filter_map(|n| before.get(&n).map(|v| (n, *v)))
        .collect();
    after
        .iter()
        .filter(|(name, _)| !before.contains_key(*name))
        .filter_map(|(name, v)| {
            sources
                .iter()
                .min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs()))
                .map(|(source, _)| (name.clone(), source.clone()))
        })
        .collect()
}

/// A solid's own colour, or, for a copy without one, its source's.
fn inherited(
    name: &str,
    own: &BTreeMap<String, Option<String>>,
    copy_of: &BTreeMap<String, String>,
) -> Option<String> {
    let mut at = name;
    // A chain of copies is at most as long as the history.
    for _ in 0..=copy_of.len() {
        if let Some(color) = own.get(at).cloned().flatten() {
            return Some(color);
        }
        at = copy_of.get(at)?;
    }
    None
}

fn hex(color: &str) -> Option<[u8; 3]> {
    let c = color.strip_prefix('#')?;
    if c.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(c, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

/// The kernel's name for a material.
fn material_name(material: &str) -> &'static str {
    doc::MATERIALS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(material.trim()))
        .map_or(doc::DEFAULT_MATERIAL, |(n, _)| n)
}

/// World placements of every instance: its parent's, then its joint's
/// motion, then its own `at` and `rotate`.
pub fn placements(assembly: &Value) -> BTreeMap<String, Xf> {
    let instances: Vec<&Value> = assembly["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let joints: BTreeMap<String, &Value> = assembly["joints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|j| Some((j["child"].as_str()?.to_owned(), j)))
        .collect();
    let mut out: BTreeMap<String, Xf> = BTreeMap::new();
    // Parents first: repeat until every instance is placed (loops were refused).
    for _ in 0..=instances.len() {
        for instance in &instances {
            let id = instance["id"].as_str().unwrap_or_default();
            if out.contains_key(id) {
                continue;
            }
            let parent = match instance["parent"].as_str() {
                Some(p) => match out.get(p) {
                    Some(xf) => *xf,
                    None => continue,
                },
                None => Xf::IDENTITY,
            };
            let motion = joints.get(id).map_or(Xf::IDENTITY, |j| joint_motion(j));
            let (at, rotate) = doc::placement(instance);
            let local = Xf::translate(at).mul(&Xf::rotate_xyz(rotate));
            out.insert(id.to_owned(), parent.mul(&motion).mul(&local));
        }
    }
    out
}

/// The motion of a joint at its value, in its parent's frame.
#[must_use]
pub fn joint_motion(joint: &Value) -> Xf {
    let values = doc::joint_values(joint);
    let origin = doc::triple(&joint["origin"]).unwrap_or([0.0; 3]);
    let axis = doc::triple(&joint["axis"]).unwrap_or([0.0, 0.0, 1.0]);
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2])
        .sqrt()
        .max(1e-12);
    let unit = [axis[0] / len, axis[1] / len, axis[2] / len];
    let turn = |degrees: f64| {
        Xf::translate(origin)
            .mul(&Xf::axis_angle(unit, degrees.to_radians()))
            .mul(&Xf::translate([-origin[0], -origin[1], -origin[2]]))
    };
    let slide = |mm: f64| Xf::translate([unit[0] * mm, unit[1] * mm, unit[2] * mm]);
    match joint["type"].as_str().unwrap_or_default() {
        "revolute" => turn(values.first().copied().unwrap_or(0.0)),
        "slider" => slide(values.first().copied().unwrap_or(0.0)),
        "cylindrical" => slide(values.get(1).copied().unwrap_or(0.0))
            .mul(&turn(values.first().copied().unwrap_or(0.0))),
        _ => Xf::IDENTITY,
    }
}

/// Where built parts are kept between builds: `$CAD_PART_CACHE`, else a
/// folder in the system's temporary directory.
#[must_use]
pub fn default_cache() -> PathBuf {
    std::env::var_os("CAD_PART_CACHE").map_or_else(
        || std::env::temp_dir().join("openagents-cad-parts"),
        PathBuf::from,
    )
}

/// The kernel session with the materials and parameters every build uses.
fn session(parameters: &[Value]) -> Result<Session, Vec<String>> {
    let mut s = Session::default();
    let fail = |e: String| vec![e];
    let known = run(&mut s, "material.list", &json!({})).map_err(fail)?;
    let has = |name: &str| {
        known["materials"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|m| m["name"] == name)
    };
    for (name, density) in doc::MATERIALS {
        if !has(name) {
            run(
                &mut s,
                "material.create",
                &json!({"name": name, "density_g_cm3": density}),
            )
            .map_err(fail)?;
        }
    }
    for p in parameters {
        let expression = match &p["expression"] {
            Value::Number(n) => n.to_string(),
            other => other.as_str().unwrap_or_default().to_owned(),
        };
        run(
            &mut s,
            "parameters.add",
            &json!({"name": p["name"], "expression": expression}),
        )
        .map_err(|e| vec![format!("Parameter {}: {e}", p["name"])])?;
    }
    Ok(s)
}

/// A part built on its own: its solids as a STEP file, each solid's
/// colour, and the kernel's warnings.
struct PartBuild {
    step: PathBuf,
    meta: Value,
}

/// The digest a built part is kept under: the part, the parameters it may
/// use, and the kernel it was built with.
fn part_digest(part: &Value, parameters: &[Value]) -> String {
    let key = json!({"part": part, "parameters": parameters, "kernel": KERNEL, "build": BUILD});
    let hash = Sha256::digest(key.to_string().as_bytes());
    hash.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// The kernel build this crate pins (part caches don't cross it).
pub const KERNEL: &str = "solidcraft@229de148";

/// What a cached part records, versioned: 2 added the colours pattern and
/// mirror copies take from their sources.
const BUILD: u32 = 2;

/// Builds one part from its history, or takes it from the cache.
fn build_part(
    key: &str,
    part: &Value,
    parameters: &[Value],
    cache: &Path,
) -> Result<PartBuild, Vec<String>> {
    let digest = part_digest(part, parameters);
    let step = cache.join(format!("{digest}.step"));
    let meta_path = cache.join(format!("{digest}.json"));
    if step.is_file()
        && let Some(meta) = std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    {
        return Ok(PartBuild { step, meta });
    }
    let mut s = session(parameters)?;
    // Each copy a pattern or mirror made → the solid it copies.
    let mut copy_of: BTreeMap<String, String> = BTreeMap::new();
    for f in part["features"].as_array().into_iter().flatten() {
        if f["suppressed"] == true {
            continue;
        }
        let copies = COPIES.contains(&f["op"].as_str().unwrap_or_default());
        let before = copies.then(|| volumes(&s));
        feature(&mut s, key, f).map_err(|e| vec![e])?;
        if let Some(before) = before {
            copy_of.extend(copies_made(key, &f["params"], &before, &volumes(&s)));
        }
    }
    let mut warnings = Vec::new();
    if let Ok(inspect) = run(&mut s, "document.inspect", &json!({"measure": false})) {
        for entry in inspect["timeline"].as_array().into_iter().flatten() {
            if let Some(error) = entry["error"].as_str() {
                return Err(vec![format!(
                    "{} failed: {error}",
                    entry["name"].as_str().unwrap_or("A feature")
                )]);
            }
            // The kernel's retries of near-coincident booleans are routine
            // (the checks after the build catch a wrong result).
            if let Some(warning) = entry["warning"]
                .as_str()
                .filter(|w| !w.starts_with("Boolean retry"))
            {
                warnings.push(format!(
                    "{}: {warning}",
                    entry["name"].as_str().unwrap_or("A feature")
                ));
            }
        }
    }
    let state = s.world_state();
    let mut own: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut order = Vec::new();
    for body in state.bodies.iter().filter(|b| s.body_visible(&b.name)) {
        if body.body.is_surface() {
            return Err(vec![format!(
                "Part {key}: {} isn't a closed solid.",
                body.name
            )]);
        }
        let color = s.doc.body_color(body).map(|[r, g, b]| {
            format!(
                "#{:02x}{:02x}{:02x}",
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8
            )
        });
        own.insert(body.name.clone(), color);
        order.push(body.name.clone());
    }
    drop(state);
    let bodies: Vec<Value> = order
        .iter()
        .map(|name| json!({"name": name, "color": inherited(name, &own, &copy_of)}))
        .collect();
    if bodies.is_empty() {
        return Err(vec![format!(
            "Part {key} has no solid yet: add a feature that makes one (an extrude, a revolve, a box or a cylinder)."
        )]);
    }
    std::fs::create_dir_all(cache).map_err(|e| vec![format!("The part cache: {e}")])?;
    let names: Vec<&Value> = bodies.iter().map(|b| &b["name"]).collect();
    let temporary = cache.join(format!("{digest}.{}.step", std::process::id()));
    run(
        &mut s,
        "file.export",
        &json!({"path": temporary.to_string_lossy(), "format": "step", "bodies": names}),
    )
    .map_err(|e| vec![format!("Part {key} couldn't be written: {e}")])?;
    std::fs::rename(&temporary, &step).map_err(|e| vec![e.to_string()])?;
    let meta = json!({"bodies": bodies, "warnings": warnings});
    let _ = std::fs::write(&meta_path, meta.to_string());
    Ok(PartBuild { step, meta })
}

/// Builds a document (a part or an assembly), keeping built parts in the
/// default cache. The error lists everything that stopped it.
pub fn build(document: &Value) -> Result<Built, Vec<String>> {
    build_with(document, &default_cache())
}

/// Builds a document, keeping built parts in `cache`: a part whose history
/// and parameters haven't changed is read back rather than rebuilt, and
/// the parts that changed build side by side.
pub fn build_with(document: &Value, cache: &Path) -> Result<Built, Vec<String>> {
    let assembly = doc::as_assembly(document);
    let problems = doc::check_assembly(&assembly);
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut warnings = Vec::new();
    let fail = |e: String| vec![e];
    let empty = Map::new();
    let parts = assembly["parts"].as_object().unwrap_or(&empty);
    let mut parameters: Vec<Value> = assembly["parameters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for part in parts.values() {
        parameters.extend(part["parameters"].as_array().cloned().unwrap_or_default());
    }
    let instances: Vec<&Value> = assembly["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    // Every part builds (or comes from the cache), each in a session of
    // its own, side by side; only placed ones go into the assembly.
    let all: Vec<(&String, &Value)> = parts.iter().collect();
    let results: Vec<Result<PartBuild, Vec<String>>> = std::thread::scope(|scope| {
        let jobs: Vec<_> = all
            .iter()
            .map(|(key, part)| {
                let parameters = &parameters;
                scope.spawn(move || build_part(key, part, parameters, cache))
            })
            .collect();
        jobs.into_iter()
            .map(|job| {
                job.join()
                    .unwrap_or_else(|_| Err(vec!["A part's build stopped unexpectedly.".into()]))
            })
            .collect()
    });
    let mut built_parts = Vec::new();
    let mut errors = Vec::new();
    for ((key, part), result) in all.iter().zip(results) {
        match result {
            Ok(b) if instances.iter().any(|i| i["part"] == key.as_str()) => {
                built_parts.push((key.to_string(), *part, b))
            }
            Ok(_) => warnings.push(format!("Part {key} isn't placed yet.")),
            Err(e) => errors.extend(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut s = session(&parameters)?;
    // Each part, read into its own component at the origin.
    let mut component_of: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut colors: BTreeMap<String, [u8; 3]> = BTreeMap::new();
    let mut finishes: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (key, part, b) in &built_parts {
        for w in b.meta["warnings"].as_array().into_iter().flatten() {
            warnings.push(w.as_str().unwrap_or_default().to_owned());
        }
        let made = run(
            &mut s,
            "component.create",
            &json!({"name": key, "activate": true}),
        )
        .map_err(fail)?;
        let component = made["component"].as_u64().unwrap_or(0);
        let occurrence = made["occurrence"].as_u64().unwrap_or(0);
        let inserted = run(
            &mut s,
            "file.insert_step",
            &json!({"path": b.step.to_string_lossy(), "name": key}),
        )
        .map_err(|e| vec![format!("Part {key} couldn't be read back: {e}")])?;
        let bodies: Vec<String> = inserted["created_bodies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        // The reader puts solids in the design's root: move them into the part.
        run(
            &mut s,
            "component.move_bodies",
            &json!({"bodies": bodies, "component": component}),
        )
        .map_err(fail)?;
        let material = material_name(part["material"].as_str().unwrap_or(doc::DEFAULT_MATERIAL));
        run(
            &mut s,
            "material.assign",
            &json!({"bodies": bodies, "material": material}),
        )
        .map_err(fail)?;
        let color = part["color"]
            .as_str()
            .and_then(hex)
            .unwrap_or([168, 174, 184]);
        run(
            &mut s,
            "appearance.assign",
            &json!({"bodies": bodies, "color": format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])}),
        )
        .map_err(fail)?;
        // Solids the history coloured on their own keep their colour.
        for (name, body) in bodies
            .iter()
            .zip(b.meta["bodies"].as_array().into_iter().flatten())
        {
            if let Some(own) = body["color"]
                .as_str()
                .filter(|c| Some(*c) != part["color"].as_str())
            {
                run(
                    &mut s,
                    "appearance.assign",
                    &json!({"bodies": [name], "color": own}),
                )
                .map_err(fail)?;
            }
        }
        colors.insert(key.clone(), color);
        finishes.insert(
            key.clone(),
            part["finish"].as_str().map(|f| f.trim().to_lowercase()),
        );
        run(&mut s, "component.activate", &json!({"component": "root"})).map_err(fail)?;
        component_of.insert(key.clone(), (component, occurrence));
    }
    // Placements: every copy first (a copy takes its source's current
    // place), then every move.
    let mut occurrences: BTreeMap<String, u64> = BTreeMap::new();
    let mut used: BTreeMap<String, bool> = BTreeMap::new();
    for instance in &instances {
        let id = instance["id"].as_str().unwrap_or_default();
        let part = instance["part"].as_str().unwrap_or_default();
        let (component, original) = component_of[part];
        let occurrence = if used.insert(part.to_owned(), true).is_none() {
            original
        } else {
            run(&mut s, "occurrence.copy", &json!({"component": component}))
                .map_err(fail)?["occurrence"]
                .as_u64()
                .unwrap_or(0)
        };
        occurrences.insert(id.to_owned(), occurrence);
    }
    let placements = placements(&assembly);
    for (id, occurrence) in &occurrences {
        let xf = placements[id];
        let (axis, angle) = xf.rotation_axis_angle();
        if angle.abs() > 1e-9 {
            run(
                &mut s,
                "occurrence.move",
                &json!({"occurrence": occurrence, "space": "world", "axis": axis,
                        "angle": format!("{} deg", angle.to_degrees()), "origin": [0, 0, 0]}),
            )
            .map_err(|e| vec![format!("Placing {id}: {e}")])?;
        }
        let t = xf.translation();
        if t.iter().any(|x| x.abs() > 1e-9) {
            run(
                &mut s,
                "occurrence.move",
                &json!({"occurrence": occurrence, "space": "world", "translate": t}),
            )
            .map_err(|e| vec![format!("Placing {id}: {e}")])?;
        }
    }
    // The solids, where they are.
    let mut owner: BTreeMap<String, (String, String)> = BTreeMap::new();
    for instance in &instances {
        let id = instance["id"].as_str().unwrap_or_default();
        let part = instance["part"].as_str().unwrap_or_default();
        for body in s
            .occurrence_bodies(&[occurrences[id]], true)
            .unwrap_or_default()
        {
            owner.insert(body, (id.to_owned(), part.to_owned()));
        }
    }
    let state = s.world_state();
    let mut solids = Vec::new();
    for body in &state.bodies {
        let Some((instance, part)) = owner.get(&body.name) else {
            continue;
        };
        let color = s
            .doc
            .body_color(body)
            .map(|[r, g, b]| {
                [
                    (r * 255.0).round() as u8,
                    (g * 255.0).round() as u8,
                    (b * 255.0).round() as u8,
                ]
            })
            .unwrap_or(colors[part]);
        solids.push(Solid {
            instance: instance.clone(),
            part: part.clone(),
            body: body.name.clone(),
            color,
            finish: finishes.get(part).cloned().flatten(),
            mesh: body.mesh(),
            closed: !body.body.is_surface() && !body.body.is_mesh(),
        });
    }
    drop(state);
    Ok(Built {
        session: s,
        solids,
        placements,
        occurrences,
        warnings,
    })
}

/// The check every edit must pass: each solid closed with volume, and no
/// two instances overlapping unless the assembly declares them in contact
/// or a joint joins them. Returns the problems and the overlaps allowed.
pub fn interference(built: &mut Built, assembly: &Value) -> (Vec<String>, Vec<Value>) {
    let mut problems = Vec::new();
    for solid in &built.solids {
        let volume = crate::mass::integrals(&solid.mesh).volume;
        if !solid.closed {
            problems.push(format!(
                "{} ({}) isn't a closed solid.",
                solid.instance, solid.body
            ));
        } else if volume <= 1e-6 {
            problems.push(format!(
                "{} ({}) has no volume.",
                solid.instance, solid.body
            ));
        }
    }
    let assembly = doc::as_assembly(assembly);
    let mut allowed: Vec<(String, String)> = Vec::new();
    for pair in assembly["contacts"].as_array().into_iter().flatten() {
        if let (Some(a), Some(b)) = (pair[0].as_str(), pair[1].as_str()) {
            allowed.push((a.to_owned(), b.to_owned()));
        }
    }
    for joint in assembly["joints"].as_array().into_iter().flatten() {
        if let (Some(a), Some(b)) = (joint["parent"].as_str(), joint["child"].as_str()) {
            allowed.push((a.to_owned(), b.to_owned()));
        }
    }
    let ok = |a: &str, b: &str| {
        a == b
            || allowed
                .iter()
                .any(|(x, y)| (x == a && y == b) || (x == b && y == a))
    };
    let mut contacts = Vec::new();
    // Every pair of solids of different instances whose boxes meet,
    // measured on their triangles (the kernel's exact intersection takes
    // minutes on touching curved faces), side by side.
    let boxes: Vec<([f64; 3], [f64; 3])> = built
        .solids
        .iter()
        .map(|s| crate::overlap::bounds(&s.mesh))
        .collect();
    let mut pairs = Vec::new();
    for i in 0..built.solids.len() {
        for j in i + 1..built.solids.len() {
            if built.solids[i].instance == built.solids[j].instance {
                continue;
            }
            let (a, b) = (boxes[i], boxes[j]);
            if (0..3).all(|k| a.0[k] < b.1[k] && b.0[k] < a.1[k]) {
                pairs.push((i, j));
            }
        }
    }
    let solids = &built.solids;
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 12);
    let measured: Vec<(usize, usize, f64)> = std::thread::scope(|scope| {
        let jobs: Vec<_> = pairs
            .chunks(pairs.len().div_ceil(threads).max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(i, j)| {
                            (
                                i,
                                j,
                                crate::overlap::volume(&solids[i].mesh, &solids[j].mesh),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter()
            .flat_map(|job| job.join().unwrap_or_default())
            .collect()
    });
    for (i, j, volume) in measured {
        if volume < crate::overlap::IGNORED_MM3 {
            continue;
        }
        let (ia, ib) = (&solids[i].instance, &solids[j].instance);
        if ok(ia, ib) {
            contacts.push(json!({"a": ia, "b": ib, "overlap_mm3": volume.round()}));
        } else {
            let (a, b) = (boxes[i], boxes[j]);
            let c: Vec<String> = (0..3)
                .map(|k| format!("{:.0}", (a.0[k].max(b.0[k]) + a.1[k].min(b.1[k])) / 2.0))
                .collect();
            problems.push(format!(
                "{ia} and {ib} overlap by about {volume:.0} mm³ near [{}]; move one, or declare them in contact if they're meant to touch.",
                c.join(", ")
            ));
        }
    }
    (problems, contacts)
}

/// Vec3 from an array.
#[must_use]
pub fn v3(a: [f64; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}
