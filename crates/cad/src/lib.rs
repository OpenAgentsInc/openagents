//! OpenAgents CAD: typed part and assembly documents, built on an open
//! B-rep kernel, checked on every edit, measured and exported
//! (docs/cad-lab/README.md).
//!
//! - [`doc`]: `part.v1` (a feature history) and `assembly.v1` (parts,
//!   instances, joints, contacts), with plain-words validation and schemas.
//! - [`build`]: replays the documents in the kernel (SolidCraft's crates on
//!   Truck, pinned), places instances and moves joints.
//! - [`evaluate`]: a build plus the checks every edit must pass (each
//!   feature rebuilds, every solid closes, no undeclared overlaps) and the
//!   mass properties.
//! - [`export`]: STEP, 3MF, STL and glTF; [`render`]: PNG views.
//! - [`collision`]: the physics bake, convex pieces per solid (CoACD,
//!   offline) and a signed-distance grid.
//! - [`urdf`]: URDF for Robot Lab (links with the measured mass and
//!   inertia, the joints with their limits); [`feed`]: the typed
//!   mass-properties record Lagrange 2's spacecraft document reads.
//!
//! Callers see our documents and JSON reports; only [`build`], [`refs`],
//! [`render`] and [`export`] touch the kernel, so it can be swapped.

pub mod assets;
pub mod build;
pub mod clash;
pub mod collision;
pub mod configure;
pub mod doc;
pub mod drag;
pub mod drawing;
pub mod dxf;
pub mod exact;
pub mod export;
pub mod feed;
pub mod lab;
pub mod manufacture;
pub mod mass;
pub mod mesh;
pub mod overlap;
pub mod refs;
pub mod render;
pub mod solve;
pub mod sys;
pub mod tree;
pub mod urdf;
pub mod view;
pub mod xf;

use std::collections::BTreeMap;

use serde_json::{Value, json};

pub use build::Built;

/// A built, checked and measured document.
pub struct Evaluation {
    pub built: Built,
    /// The checks, the mass properties and the counts, as JSON.
    pub report: Value,
}

/// Builds `document`, checks it and measures it. The error lists, in plain
/// words, every reason it can't be accepted: a feature that fails, a solid
/// that doesn't close, an overlap nobody declared.
pub fn evaluate(document: &Value) -> Result<Evaluation, Vec<String>> {
    evaluate_with(document, &build::default_cache())
}

/// [`evaluate`], keeping built parts in `cache`.
pub fn evaluate_with(document: &Value, cache: &std::path::Path) -> Result<Evaluation, Vec<String>> {
    let started = sys::Timer::start();
    let mut built = build::build_with(document, cache)?;
    let (problems, contacts) = build::interference(&mut built, document);
    if !problems.is_empty() {
        return Err(problems);
    }
    let assembly = doc::as_assembly(document);
    let density_of = |part: &str| {
        doc::density(
            assembly["parts"][part]["material"]
                .as_str()
                .unwrap_or(doc::DEFAULT_MATERIAL),
        )
        .unwrap_or(2.7)
    };
    let mut per_instance: BTreeMap<String, mass::Sum> = BTreeMap::new();
    let mut triangles = 0usize;
    for solid in &built.solids {
        per_instance
            .entry(solid.instance.clone())
            .or_default()
            .add(&solid.mesh, density_of(&solid.part));
        triangles += solid.mesh.triangles.len();
    }
    let mut total = mass::Sum::default();
    let mut instances = Vec::new();
    for instance in assembly["instances"].as_array().into_iter().flatten() {
        let id = instance["id"].as_str().unwrap_or_default();
        let Some(sum) = per_instance.get(id) else {
            continue;
        };
        total.merge(sum);
        let mut entry = sum.mass().json();
        entry["id"] = json!(id);
        entry["part"] = instance["part"].clone();
        instances.push(entry);
    }
    // Each placed subassembly's mass properties: everything inside it.
    let mut groups = Vec::new();
    for instance in assembly["instances"].as_array().into_iter().flatten() {
        let (Some(id), Some(template)) = (instance["id"].as_str(), instance["group"].as_str())
        else {
            continue;
        };
        let inside = format!("{id}.");
        let mut sum = mass::Sum::default();
        for (_, s) in per_instance.iter().filter(|(k, _)| k.starts_with(&inside)) {
            sum.merge(s);
        }
        let mut entry = sum.mass().json();
        entry["id"] = json!(id);
        entry["assembly"] = json!(template);
        groups.push(entry);
    }
    let mut parts = Vec::new();
    for (key, part) in assembly["parts"].as_object().into_iter().flatten() {
        let count = instances
            .iter()
            .filter(|i| i["part"] == key.as_str())
            .count();
        let one = instances.iter().find(|i| i["part"] == key.as_str());
        parts.push(json!({
            "id": key,
            "title": part["title"],
            "material": part["material"].as_str().unwrap_or(doc::DEFAULT_MATERIAL),
            "features": part["features"].as_array().map_or(0, Vec::len),
            "instances": count,
            "mass_kg": one.map(|i| i["mass_kg"].clone()),
        }));
    }
    let bounds = render::bounds(&built, &[]);
    let size = if bounds.is_empty() {
        json!(null)
    } else {
        let s = bounds.size();
        json!([s.x.round(), s.y.round(), s.z.round()])
    };
    let features: usize = parts
        .iter()
        .filter_map(|p| p["features"].as_u64())
        .sum::<u64>() as usize;
    let mesh_parts: std::collections::BTreeSet<&str> = assembly["parts"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, part)| {
            part["features"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|f| f["op"] == "solid.import_stl" && f["suppressed"] != true)
        })
        .map(|(id, _)| id.as_str())
        .collect();
    let mesh_bodies = built
        .solids
        .iter()
        .filter(|s| mesh_parts.contains(s.part.as_str()))
        .count();
    let report = json!({
        "checks": {
            "rebuilt": true,
            "closed_solids": built.solids.len()-mesh_bodies,
            "closed_triangle_meshes":mesh_bodies,
            "interference": "none beyond declared contacts",
            "contacts": contacts,
            "overlap_check": built.interference,
        },
        "mesh_bodies":mesh_bodies,
        "mass": total.mass().json(),
        "parts": parts,
        "instances": instances,
        "joints": assembly["joints"].as_array().map_or(0, Vec::len),
        "features": features,
        "solids": built.solids.len(),
        "triangles": triangles,
        "size_mm": size,
        "subassemblies": groups,
        "warnings": built.warnings,
        "sketches": built.sketches,
        "build_ms": started.ms(),
    });
    let mut report = report;
    if !built.solver.is_null() {
        report["solver"] = built.solver.clone();
    }
    // Parts flagged for it: SMLib's exact mass properties must agree.
    let (exact, problems) = exact::cross_check(&built, exact::python().as_deref());
    if !problems.is_empty() {
        return Err(problems);
    }
    if !exact.is_empty() {
        report["exact_mass"] = json!(exact);
    }
    Ok(Evaluation { built, report })
}

/// The Solid API: direct operations on solids with no history kept. Runs
/// `ops` (direct steps, each `{op, params, repeat?}`) after any `sketches`
/// they extrude or revolve, in a scratch part of `material`, and checks and
/// measures the result like any edit; nothing is saved.
///
/// # Errors
/// Why the operations don't make a valid solid.
pub fn solid(
    ops: &[Value],
    sketches: &[Value],
    material: &str,
    cache: &std::path::Path,
) -> Result<Evaluation, Vec<String>> {
    let mut part = doc::new_part("solid", "Solid", material, None);
    let mut features: Vec<Value> = sketches.to_vec();
    features.push(json!({"id": "ops", "op": "block", "params": {"ops": ops}}));
    part["features"] = Value::Array(features);
    for f in part["features"].as_array().into_iter().flatten() {
        if f["op"] != "sketch" && f["id"] != "ops" {
            return Err(vec![format!(
                "{} isn't a sketch; the Solid API takes sketches and direct steps.",
                f["id"]
            )]);
        }
    }
    evaluate_with(&part, cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plate with rounded corners and four holes; the analytic volume is
    /// 80·50·6 − 4·5²(1−π/4)·6 − 4·π·1.7²·6.
    fn plate() -> Value {
        let mut part = doc::new_part("plate", "Plate", "Aluminum", Some("#8899aa"));
        part["features"] = json!([
            {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": [
                {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [40, 25]}}]}},
            {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 6}},
            {"id": "corners", "op": "solid.fillet", "params": {"edges": [[40, 25, 3], [-40, 25, 3], [40, -25, 3], [-40, -25, 3]], "radius": 5}},
            {"id": "hole", "op": "solid.cylinder", "params": {"diameter": 3.4, "height": 10, "base": [32, 17, -2], "operation": "cut"}},
            {"id": "holes", "op": "solid.pattern.rectangular", "params": {"features": ["hole"], "dir1": [-1, 0, 0], "count1": 2, "spacing1": 64, "dir2": [0, -1, 0], "count2": 2, "spacing2": 34}}
        ]);
        part
    }

    #[test]
    fn a_filleted_plate_with_holes_measures_its_analytic_volume() {
        let eval = evaluate(&plate()).unwrap_or_else(|p| panic!("{p:?}"));
        let pi = std::f64::consts::PI;
        let expected =
            80.0 * 50.0 * 6.0 - 4.0 * 25.0 * (1.0 - pi / 4.0) * 6.0 - 4.0 * pi * 1.7 * 1.7 * 6.0;
        let cm3 = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        assert!(
            (cm3 - expected).abs() / expected < 0.005,
            "{cm3} vs {expected}"
        );
        let kg = eval.report["mass"]["mass_kg"].as_f64().unwrap();
        assert!((kg - expected * 2.7e-6).abs() < 0.001, "{kg}");
        assert_eq!(eval.report["checks"]["closed_solids"], 1);
    }

    #[test]
    fn fillets_and_shells_name_their_edges_and_faces_and_survive_a_resize() {
        // The corners are named by the sketch sides they join, so making the
        // plate longer moves them and the fillet still finds them (a point
        // pick at the old corner would miss).
        let plate = |half: f64| {
            let mut part = doc::new_part("tray", "Tray", "Aluminum", None);
            part["features"] = json!([
                {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": [
                    {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [half, 25]}}]}},
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 20}},
                {"id": "corners", "op": "solid.fillet", "params": {"radius": 5, "edges": [
                    {"between": ["body:side:l1", "body:side:l2"]}, {"between": ["body:side:l2", "body:side:l3"]},
                    {"between": ["body:side:l3", "body:side:l4"]}, {"between": ["body:side:l4", "body:side:l1"]}]}},
                {"id": "hollow", "op": "solid.shell", "params": {"faces": ["body:top"], "thickness": 2}},
                {"id": "rim", "op": "solid.chamfer", "params": {"distance": 1, "edges": [{"of": "body:start"}]}}
            ]);
            part
        };
        let volume = |part: &Value| {
            let eval = evaluate(part).unwrap_or_else(|p| panic!("{p:?}"));
            eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0
        };
        let (short, long) = (volume(&plate(40.0)), volume(&plate(60.0)));
        // A rounded open tray: about 2 mm walls on 2·(2·half + 50) − corners of
        // perimeter, 20 high, plus a 2 mm floor.
        let pi = std::f64::consts::PI;
        let solid = |half: f64| 2.0 * half * 50.0 - 4.0 * 25.0 * (1.0 - pi / 4.0);
        let hollow = |half: f64| (2.0 * half - 4.0) * 46.0 - 4.0 * 9.0 * (1.0 - pi / 4.0);
        // The 1 mm chamfer round the bottom takes half a square mm per mm.
        let chamfer = |half: f64| 0.5 * (2.0 * (2.0 * half + 50.0) - 40.0 + 2.0 * pi * 5.0);
        for (half, v) in [(40.0, short), (60.0, long)] {
            let expected = solid(half) * 20.0 - hollow(half) * 18.0 - chamfer(half);
            assert!(
                (v - expected).abs() / expected < 0.006,
                "{half}: {v} vs about {expected}"
            );
        }
        // A name that isn't there says which names are.
        let mut bad = plate(40.0);
        bad["features"][3]["params"]["faces"] = json!(["body:roof"]);
        let why = evaluate(&bad).err().unwrap().join(" ");
        assert!(
            why.contains("no face body:roof") && why.contains("body:top"),
            "{why}"
        );
        // What a feature can name, after the extrude.
        let listed =
            build::faces_of(&plate(40.0), "tray", Some("body"), &build::default_cache()).unwrap();
        let text = listed.to_string();
        for name in ["body:top", "body:start", "body:side:l3"] {
            assert!(text.contains(name), "{name}: {text:.600}");
        }
        assert_eq!(listed["solids"][0]["faces"].as_array().unwrap().len(), 6);
        assert_eq!(listed["solids"][0]["edges"].as_array().unwrap().len(), 12);
    }

    #[test]
    fn a_direct_block_repeats_steps_and_recomputes_when_its_inputs_change() {
        // Eight holes along a sine wave, cut by one repeated direct step
        // sized by a parameter: change the parameter and the block follows.
        let strip = |d: f64| {
            let mut part = doc::new_part("strip", "Strip", "Aluminum", None);
            part["parameters"] = json!([{"name": "hole_d", "expression": format!("{d} mm")}]);
            part["features"] = json!([
                {"id": "base", "op": "solid.box", "params": {"length": 100, "width": 30, "height": 5, "center": [0, 0, 2.5]}},
                {"id": "holes", "op": "block", "params": {"inputs": ["base", "hole_d"], "ops": [
                    {"op": "solid.cylinder", "repeat": {"var": "i", "from": 0, "to": 7},
                     "params": {"diameter": "hole_d", "height": 9, "operation": "cut",
                                "base": ["-42 + {i} * 12", "8 * sin({i} * 45)", -2]}}]}}
            ]);
            part
        };
        let pi = std::f64::consts::PI;
        for d in [4.0, 6.0] {
            let eval = evaluate(&strip(d)).unwrap_or_else(|p| panic!("{p:?}"));
            let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
            let expected = 100.0 * 30.0 * 5.0 - 8.0 * pi * (d / 2.0) * (d / 2.0) * 5.0;
            assert!(
                (v - expected).abs() / expected < 0.003,
                "{d}: {v} vs {expected}"
            );
        }
        // An input that isn't there is refused before anything builds.
        let mut bad = strip(4.0);
        bad["features"][1]["params"]["inputs"] = json!(["base", "hole_size"]);
        let why = evaluate(&bad).err().unwrap().join(" ");
        assert!(why.contains("reads hole_size"), "{why}");
        bad["features"][1]["params"]["inputs"] = json!([]);
        bad["features"][1]["params"]["ops"][0]["repeat"] = json!({"from": 0, "to": 5000});
        let why = evaluate(&bad).err().unwrap().join(" ");
        assert!(why.contains("500 is the most"), "{why}");
        // The Solid API: the same steps with no history, measured, not saved.
        let ops = json!([
            {"op": "solid.box", "params": {"length": 20, "width": 20, "height": 20, "center": [0, 0, 0]}},
            {"op": "solid.cylinder", "params": {"radius": 5, "height": 30, "base": [0, 0, -15], "operation": "cut"}}
        ]);
        let eval = solid(
            ops.as_array().unwrap(),
            &[],
            "Steel",
            &build::default_cache(),
        )
        .unwrap_or_else(|p| panic!("{p:?}"));
        let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        let expected = 8000.0 - pi * 25.0 * 20.0;
        assert!((v - expected).abs() / expected < 0.01, "{v} vs {expected}");
    }

    #[test]
    fn sketch_constraints_and_dimensions_drive_the_profile() {
        // An outline drawn roughly, then held square and sized by
        // dimensions, one of them a parameter: the solid follows it.
        let block = |w: f64| {
            let mut part = doc::new_part("block", "Block", "Aluminum", None);
            part["parameters"] = json!([{"name": "w", "expression": format!("{w} mm")}]);
            part["features"] = json!([
                {"id": "outline", "op": "sketch", "params": {"plane": "XY",
                    "draw": [{"op": "sketch.line", "params": {"points": [[0, 0], [12, 1], [11, 9], [-1, 10]], "closed": true,
                              "infer": false, "ids": ["bottom", "right", "top", "left"]}}],
                    "constraints": [
                        {"type": "fix", "entity": "bottom.start"},
                        {"type": "horizontal", "line": "bottom"}, {"type": "horizontal", "line": "top"},
                        {"type": "vertical", "line": "left"}, {"type": "vertical", "line": "right"}],
                    "dimensions": [{"entities": ["bottom"], "value": "w"}, {"entities": ["left"], "value": 20}]}},
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 5}}
            ]);
            part
        };
        for w in [30.0, 45.0] {
            let eval = evaluate(&block(w)).unwrap_or_else(|p| panic!("{p:?}"));
            let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
            assert!((v - w * 20.0 * 5.0).abs() < 0.5, "{w}: {v}");
            let sketch = &eval.report["sketches"][0];
            assert_eq!(sketch["sketch"], "outline", "{sketch}");
            assert_eq!(sketch["dof"], 0, "{sketch}");
        }
        let mut bad = block(30.0);
        bad["features"][0]["params"]["constraints"][1]["type"] = json!("level");
        let why = evaluate(&bad).err().unwrap().join(" ");
        assert!(why.contains("\"level\""), "{why}");
    }

    fn volume_of(features: Value) -> f64 {
        let mut part = doc::new_part("p", "P", "Aluminum", None);
        part["features"] = features;
        let eval = evaluate(&part).unwrap_or_else(|p| panic!("{p:?}"));
        eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0
    }

    #[test]
    fn sweep_loft_draft_and_mirror_build_from_documents() {
        let pi = std::f64::consts::PI;
        // A rod: a circle swept 50 mm along a line.
        let rod = volume_of(json!([
            {"id": "section", "op": "sketch", "params": {"plane": "YZ", "draw": [
                {"op": "sketch.circle.center", "params": {"center": [0, 0], "radius": 3}}]}},
            {"id": "route", "op": "sketch", "params": {"plane": "XY", "draw": [
                {"op": "sketch.line", "params": {"points": [[0, 0], [50, 0]], "ids": ["run"]}}]}},
            {"id": "rod", "op": "solid.sweep", "params": {"sketch": "section", "path_sketch": "route", "path": ["run"]}}
        ]));
        assert!((rod - pi * 9.0 * 50.0).abs() / rod < 0.01, "sweep {rod}");
        // A square frustum: 20 mm square to 10 mm square over 30 mm.
        let frustum = volume_of(json!([
            {"id": "foot", "op": "sketch", "params": {"plane": "XY", "draw": [
                {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [10, 10]}}]}},
            {"id": "head", "op": "sketch", "params": {"plane": "XY", "offset": 30, "draw": [
                {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [5, 5]}}]}},
            {"id": "tower", "op": "solid.loft", "params": {"sections": [{"sketch": "foot"}, {"sketch": "head"}]}}
        ]));
        assert!((frustum - 7000.0).abs() < 20.0, "loft {frustum}");
        // A 5° draft on one side of a 20 mm cube, about the floor: a wedge
        // of ½·20²·tan 5°·20 comes off or goes on.
        let drafted = volume_of(json!([
            {"id": "b", "op": "solid.box", "params": {"length": 20, "width": 20, "height": 20, "center": [0, 0, 10]}},
            {"id": "taper", "op": "solid.draft", "params": {"faces": ["b:+x"], "angle": 5, "neutral": "XY"}}
        ]));
        let wedge = 0.5 * 400.0 * 5f64.to_radians().tan() * 20.0;
        assert!(
            ((drafted - 8000.0).abs() - wedge).abs() < 5.0,
            "draft {drafted}"
        );
        // A block mirrored across YZ: two solids, twice the volume.
        let pair = volume_of(json!([
            {"id": "b", "op": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "center": [20, 0, 5]}},
            {"id": "twin", "op": "solid.mirror", "params": {"bodies": ["b"], "plane": "YZ"}}
        ]));
        assert!((pair - 2000.0).abs() < 1.0, "mirror {pair}");
    }

    #[test]
    fn polylines_and_dxf_drawings_make_solids_and_go_back_out() {
        let pi = std::f64::consts::PI;
        let slot = 40.0 * 10.0 + pi * 25.0;
        let with_draw = |draw: Value| {
            let mut part = doc::new_part("plate", "Plate", "Aluminum", None);
            part["features"] = json!([
                {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": draw}},
                // The region round (5, 5): the slot, with any island inside it a hole.
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 4,
                    "profiles": [{"point": [5, 5]}]}}
            ]);
            part
        };
        // A slot as one bulged polyline.
        let part = with_draw(json!([{"op": "sketch.polyline", "params": {"closed": true,
            "points": [[0, 0], [40, 0, 1], [40, 10], [0, 10, 1]]}}]));
        let eval = evaluate(&part).unwrap_or_else(|p| panic!("{p:?}"));
        let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        assert!((v - slot * 4.0).abs() / v < 0.005, "polyline {v}");
        // The same slot drawn in DXF, with a hole: imported as JSON steps.
        let drawing = "0\nSECTION\n2\nENTITIES\n\
            0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n0\n20\n0\n10\n40\n20\n0\n42\n1\n10\n40\n20\n10\n10\n0\n20\n10\n42\n1\n\
            0\nCIRCLE\n8\n0\n10\n20\n20\n5\n40\n2\n\
            0\nENDSEC\n0\nEOF\n";
        let steps = dxf::import(drawing, 1.0, [0.0, 0.0]).unwrap();
        let eval = evaluate(&with_draw(json!(steps))).unwrap_or_else(|p| panic!("{p:?}"));
        let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        let expected = (slot - pi * 4.0) * 4.0;
        assert!((v - expected).abs() / v < 0.005, "dxf {v} vs {expected}");
        // Out again: the sketch as DXF, and a section of the solid.
        let (text, entities) = build::sketch_dxf(
            &with_draw(json!(steps)),
            "plate",
            "outline",
            &build::default_cache(),
        )
        .unwrap_or_else(|p| panic!("{p:?}"));
        assert!(
            entities >= 5 && text.contains("ARC") && text.contains("CIRCLE"),
            "{entities} {text:.300}"
        );
        let (cut, lines) = dxf::section(&eval.built, "XY", 2.0).unwrap();
        assert!(lines > 20 && cut.contains("LINE"), "{lines}");
        assert!(dxf::section(&eval.built, "XY", 50.0).is_err());
    }

    #[test]
    fn step_and_3mf_keep_the_assembly_tree_and_step_comes_back_as_a_feature() {
        let mut a = cubes();
        a["parts"]["rod"] = doc::new_part("rod", "Rod", "Steel", None);
        a["parts"]["rod"]["features"] =
            json!([{"id": "c", "op": "solid.cylinder", "params": {"radius": 2, "height": 30}}]);
        a["assemblies"] = json!({"stack": {"instances": [
            {"id": "base", "part": "cube"}, {"id": "top", "part": "cube", "parent": "base", "at": [0, 0, 10], "rotate": [0, 0, 30]}]}});
        a["instances"] = json!([
            {"id": "left", "assembly": "stack", "at": [0, 40, 0]},
            {"id": "right", "assembly": "stack", "at": [0, -40, 0], "rotate": [0, 0, 90]},
            {"id": "post", "part": "rod", "at": [60, 0, 0]}
        ]);
        let mut eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        let total = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        let dir = std::env::temp_dir().join(format!("cad-tree-{}", std::process::id()));
        export::write(&mut eval.built, "step", &dir.join("a.step")).unwrap();
        export::write(&mut eval.built, "3mf", &dir.join("a.3mf")).unwrap();
        let step = std::fs::read_to_string(dir.join("a.step")).unwrap();
        // The whole, two subassemblies, two parts; seven placements.
        assert!(step.contains("AP242_MANAGED_MODEL_BASED_3D_ENGINEERING"));
        assert_eq!(
            step.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE(").count(),
            7,
            "{step:.2000}"
        );
        assert_eq!(step.matches("=PRODUCT(").count(), 5);
        assert!(step.contains("'stack (right)'"), "{step:.3000}");
        // Read back: the same solids, the same volume, in the same places.
        let mut back = solidcraft_engine::Session::default();
        let read = back
            .execute(
                "file.insert_step",
                &json!({"path": dir.join("a.step").to_string_lossy()}),
            )
            .unwrap();
        assert_eq!(
            read["created_bodies"].as_array().unwrap().len(),
            5,
            "{read}"
        );
        let measured = back.execute("inspect.measure", &json!({})).unwrap();
        let v = measured["total"]["volume_mm3"].as_f64().unwrap();
        assert!((v - total).abs() / total < 0.005, "{v} vs {total}");
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for b in &back.world_state().bodies {
            for p in &b.mesh().positions {
                for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
                    lo[k] = lo[k].min(x);
                    hi[k] = hi[k].max(x);
                }
            }
        }
        let ours = render::bounds(&eval.built, &[]);
        for (k, (l, h)) in [
            (ours.min.x, ours.max.x),
            (ours.min.y, ours.max.y),
            (ours.min.z, ours.max.z),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                (lo[k] - l).abs() < 0.5 && (hi[k] - h).abs() < 0.5,
                "{k}: {lo:?} {hi:?} vs {ours:?}"
            );
        }
        // The 3MF: one build item, the whole, made of components.
        let bytes = std::fs::read(dir.join("a.3mf")).unwrap();
        let mut package = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
        let mut model = String::new();
        std::io::Read::read_to_string(
            &mut package.by_name("3D/3dmodel.model").unwrap(),
            &mut model,
        )
        .unwrap();
        // Two meshes, two parts, two subassemblies and the whole; one item.
        assert_eq!(model.matches("<mesh>").count(), 2, "{model:.1500}");
        assert_eq!(model.matches("<components>").count(), 5);
        assert_eq!(model.matches("<component ").count(), 2 + 4 + 3);
        assert_eq!(model.matches("<item ").count(), 1);
        // Another reader places every solid from the tree: the same volume.
        let (objects, _) = solidcraft_engine::io::read_3mf(&bytes).unwrap();
        let mut placed = 0.0;
        for o in &objects {
            placed += mass::integrals(&solidcraft_engine::geom::Mesh {
                positions: o.positions.clone(),
                triangles: o.triangles.clone(),
                ..Default::default()
            })
            .volume;
        }
        assert!(
            (placed - total).abs() / total < 0.01,
            "3mf {placed} vs {total}"
        );
        // A STEP file comes back as a feature of a part (AP242 as we write
        // it, and AP214 too), and later features work on it.
        let cache = build::default_cache();
        for schema in ["AP242", "AP214"] {
            let mut text = std::fs::read_to_string(dir.join("a.step")).unwrap();
            if schema == "AP214" {
                text = text.replace(
                    "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 1 1 4 }",
                    "AUTOMOTIVE_DESIGN { 1 0 10303 214 3 1 1 }",
                );
            }
            let file = build::store_file(&cache, text.as_bytes(), "step").unwrap();
            let mut part = doc::new_part("copy", "Copy", "Steel", None);
            part["features"] = json!([
                {"id": "model", "op": "solid.import_step", "params": {"file": file}},
                {"id": "notch", "op": "solid.box", "params": {"length": 4, "width": 4, "height": 40, "center": [60, 0, 0], "operation": "cut"}}
            ]);
            let eval = evaluate(&part).unwrap_or_else(|p| panic!("{schema}: {p:?}"));
            let v = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
            // The notch takes the rod's middle: π·2²·30 minus nothing else.
            let notch = std::f64::consts::PI * 4.0 * 30.0
                - (v - (total - std::f64::consts::PI * 4.0 * 30.0)).max(0.0);
            assert!(v < total - 100.0 && notch > 0.0, "{schema}: {v} vs {total}");
        }
        let missing = json!({"kind": "part.v1", "id": "x", "features": [{"id": "m", "op": "solid.import_step",
            "params": {"file": format!("sha256:{}", "0".repeat(64))}}]});
        assert!(
            evaluate(&missing)
                .err()
                .unwrap()
                .join(" ")
                .contains("isn't in this Lab")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn flagged_parts_cross_check_their_mass_against_smlib() {
        // A wheel: a tyre with a bore, turned 30° and lifted, so the centre
        // and the inertia have to come into the assembly's axes.
        let mut a = doc::new_assembly("cart", "Cart");
        a["parts"]["wheel"] = doc::new_part("wheel", "Wheel", "Rubber", None);
        a["parts"]["wheel"]["exact_mass"] = json!(true);
        a["parts"]["wheel"]["features"] = json!([
            {"id": "tyre", "op": "solid.cylinder", "params": {"radius": 30, "height": 12, "axis": "Y", "base": [0, -6, 0]}},
            {"id": "bore", "op": "solid.cylinder", "params": {"radius": 4, "height": 20, "axis": "Y", "base": [6, -10, 0], "operation": "cut"}}
        ]);
        a["instances"] =
            json!([{"id": "w", "part": "wheel", "at": [10, 20, 40], "rotate": [0, 0, 30]}]);
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        // Where SMLib is installed (CAD_SMLIB_PYTHON or python3), the check
        // ran and agreed; elsewhere it says it didn't run.
        let entry = &eval.report["exact_mass"][0];
        assert_eq!(entry["part"], "wheel", "{entry}");
        let Some(python) = exact::python() else {
            assert_eq!(entry["checked"], false, "{entry}");
            assert!(entry["why"].as_str().unwrap().contains("smlibpy"));
            eprintln!("SMLib isn't installed here; the exact cross-check was skipped.");
            return;
        };
        assert_eq!(entry["agrees"], true, "{entry}");
        let v = entry["exact"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        let pi = std::f64::consts::PI;
        assert!((v - pi * (900.0 - 16.0) * 12.0).abs() < 0.01, "{v}");
        // A tolerance tighter than the triangles can meet refuses the edit.
        let (_, problems) = exact::cross_check(
            &{
                let mut b = eval.built;
                b.assembly["parts"]["wheel"]["exact_mass"] = json!({"tolerance": 1e-9});
                b
            },
            Some(&python),
        );
        assert!(problems.join(" ").contains("disagree"), "{problems:?}");
    }

    /// #11657: the collision bake splits the plate (non-convex: four holes)
    /// into convex pieces and bakes its distance grid, the same bytes each
    /// time. Skips when this machine has no CoACD (`pip install coacd`).
    #[test]
    fn the_collision_bake_is_convex_pieces_and_a_grid_reproducibly() {
        let options = collision::Options::default();
        let probe = std::process::Command::new(&options.python)
            .args(["-c", "import coacd"])
            .output();
        if !probe.is_ok_and(|o| o.status.success()) {
            eprintln!("skipped: no CoACD for {}", options.python);
            return;
        }
        let mut eval = evaluate(&plate()).unwrap();
        let dir = std::env::temp_dir().join(format!("cad-collision-{}", std::process::id()));
        let mut hashes = Vec::new();
        for run in ["a", "b"] {
            let path = dir.join(format!("plate-{run}.collision.json"));
            export::write(&mut eval.built, "collision", &path).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let report: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(report["format"], collision::FORMAT);
            assert_eq!(report["decomposition"]["tool"], "CoACD");
            assert_eq!(report["decomposition"]["license"], "MIT");
            assert_eq!(report["decomposition"]["params"]["seed"], 0);
            let hulls = report["solids"][0]["hulls"].as_array().unwrap();
            assert!(
                hulls.len() > 1,
                "a plate with holes is several convex pieces"
            );
            // Every piece builds as an engine hull, and together they cover
            // the plate: their volumes sum to at least its volume (pieces
            // bridge the small holes and overlap a little).
            let volume: f64 = hulls
                .iter()
                .map(|h| {
                    let points: Vec<glam::DVec3> = h
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| {
                            let a: Vec<f64> = serde_json::from_value(p.clone()).unwrap();
                            glam::DVec3::new(a[0], a[1], a[2])
                        })
                        .collect();
                    physics::ConvexHull::new(&points).unwrap().volume()
                })
                .sum();
            let plate_m3 = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1e-6;
            assert!(
                volume > 0.95 * plate_m3 && volume < 1.3 * plate_m3,
                "{volume} vs {plate_m3}"
            );
            let sdf = std::fs::read(dir.join(format!("plate-{run}.collision.sdf"))).unwrap();
            let grid = physics::SdfGrid::from_bytes(&sdf).unwrap();
            assert_eq!(report["sdf"]["sha256"], grid.hash());
            // Inside the plate's middle (off the holes) is inside.
            assert!(grid.sample(glam::DVec3::new(0.0, 0.0, 0.003)).0 < 0.0);
            hashes.push((text.replace(&format!("plate-{run}"), "plate"), grid.hash()));
        }
        assert_eq!(
            hashes[0], hashes[1],
            "the same document bakes the same files"
        );
    }

    #[test]
    fn exports_step_3mf_and_glb_and_reads_step_back() {
        let mut eval = evaluate(&plate()).unwrap();
        let dir = std::env::temp_dir().join(format!("cad-test-{}", std::process::id()));
        for format in ["step", "3mf", "glb"] {
            let path = dir.join(format!("plate.{format}"));
            let bytes = export::write(&mut eval.built, format, &path).unwrap();
            assert!(bytes > 100, "{format}");
        }
        let glb = std::fs::read(dir.join("plate.glb")).unwrap();
        assert_eq!(&glb[..4], b"glTF");
        let step = std::fs::read_to_string(dir.join("plate.step")).unwrap();
        assert!(step.contains("ISO-10303-21"));
        // The STEP file comes back as a solid of the same volume.
        let mut back = solidcraft_engine::Session::default();
        back.execute(
            "file.insert_step",
            &json!({"path": dir.join("plate.step").to_string_lossy()}),
        )
        .unwrap();
        let measured = back.execute("inspect.measure", &json!({})).unwrap();
        let v = measured["total"]["volume_mm3"].as_f64().unwrap();
        let ours = eval.report["mass"]["volume_cm3"].as_f64().unwrap() * 1000.0;
        assert!((v - ours).abs() / ours < 0.005, "{v} vs {ours}");
        let png = render::png(&eval.built, &render::View::named("hero"), 320, 200).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn patterned_and_mirrored_copies_keep_their_solids_colour() {
        // A red hub with a blue post and a green one of the same size, both
        // patterned four times round, the blue one mirrored below too, and
        // coloured only after: every copy takes its own source's colour,
        // though the two sources can't be told apart by volume.
        let mut part = doc::new_part("spinner", "Spinner", "ABS Plastic", Some("#cc2222"));
        part["features"] = json!([
            {"id": "hub", "op": "solid.cylinder", "params": {"radius": 10, "height": 5}},
            {"id": "post", "op": "solid.box", "params": {"length": 4, "width": 4, "height": 10, "center": [20, 0, 5], "operation": "new"}},
            {"id": "peg", "op": "solid.box", "params": {"length": 4, "width": 4, "height": 10, "center": [30, 0, 5], "operation": "new"}},
            {"id": "ring", "op": "solid.pattern.circular", "params": {"features": ["post", "peg"], "axis": "Z", "count": 4}},
            {"id": "under", "op": "solid.mirror", "params": {"bodies": ["post"], "plane": "XY"}},
            {"id": "paint", "op": "appearance.assign", "params": {"bodies": ["post"], "color": "#2244dd"}},
            {"id": "paint_peg", "op": "appearance.assign", "params": {"bodies": ["peg"], "color": "#22aa44"}}
        ]);
        let eval = evaluate(&part).unwrap_or_else(|p| panic!("{p:?}"));
        let count = |c: [u8; 3]| eval.built.solids.iter().filter(|s| s.color == c).count();
        assert_eq!(eval.built.solids.len(), 10);
        assert_eq!(count([0x22, 0x44, 0xdd]), 5);
        assert_eq!(count([0x22, 0xaa, 0x44]), 4);
        assert_eq!(count([0xcc, 0x22, 0x22]), 1);
        // Each green solid is where a peg is: 30 mm from the axis.
        for s in eval
            .built
            .solids
            .iter()
            .filter(|s| s.color == [0x22, 0xaa, 0x44])
        {
            let mut b = solidcraft_engine::geom::Aabb3::default();
            for p in &s.mesh.positions {
                b.add(*p);
            }
            let m = b.center();
            assert!(((m.x * m.x + m.y * m.y).sqrt() - 30.0).abs() < 0.5, "{m:?}");
        }
        // A glossy part's glTF materials are smooth; a plain one keeps 0.6.
        let text =
            |solids: &[build::Solid]| String::from_utf8_lossy(&export::glb(solids)).into_owned();
        assert!(text(&eval.built.solids).contains("\"roughnessFactor\":0.6"));
        part["finish"] = json!("gloss");
        let glossy = evaluate(&part).unwrap_or_else(|p| panic!("{p:?}"));
        assert!(
            glossy
                .built
                .solids
                .iter()
                .all(|s| s.finish.as_deref() == Some("gloss"))
        );
        let glb = text(&glossy.built.solids);
        assert!(
            glb.contains("\"roughnessFactor\":0.18") && glb.contains("#2244dd gloss"),
            "{glb:.400}"
        );
        part["finish"] = json!("shiny");
        assert!(
            evaluate(&part)
                .err()
                .unwrap()
                .join(" ")
                .contains("finish must be one of gloss, satin, matte")
        );
    }

    fn com(eval: &Evaluation, id: &str) -> [f64; 3] {
        let i = eval.report["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .unwrap_or_else(|| panic!("no instance {id}: {}", eval.report["instances"]));
        let c = &i["center_of_mass_mm"];
        [0, 1, 2].map(|k| c[k].as_f64().unwrap())
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.5)
    }

    /// A 10 mm cube centred on (x, 0, 5).
    fn cubes() -> Value {
        let mut a = doc::new_assembly("joints", "Joints");
        a["parts"]["cube"] = doc::new_part("cube", "Cube", "Steel", None);
        a["parts"]["cube"]["features"] = json!([{"id": "b", "op": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "center": [0, 0, 5]}}]);
        a
    }

    #[test]
    fn planar_spherical_and_gear_joints_move_their_children() {
        let mut a = cubes();
        a["instances"] = json!([
            {"id": "base", "part": "cube"},
            {"id": "puck", "part": "cube", "parent": "base", "at": [40, 0, 0]},
            {"id": "ball", "part": "cube", "parent": "base", "at": [0, 40, 0]},
            {"id": "wheel", "part": "cube", "parent": "base", "at": [-40, 0, 0]},
            {"id": "pinion", "part": "cube", "parent": "base", "at": [0, -40, 0]}
        ]);
        a["joints"] = json!([
            // Slides 10 along X, 5 along Y and turns 90° about Z through (0, 0, 0).
            {"id": "table", "type": "planar", "parent": "base", "child": "puck",
             "origin": [0, 0, 0], "axis": [0, 0, 1], "x_dir": [1, 0, 0], "value": [10, 5, 90],
             "limits": [[-50, 50], [-50, 50], [-180, 180]]},
            // Turns 90° about Z through the base.s origin.
            {"id": "socket", "type": "spherical", "parent": "base", "child": "ball",
             "origin": [0, 0, 0], "value": [0, 0, 90]},
            {"id": "spin", "type": "revolute", "parent": "base", "child": "wheel",
             "origin": [0, 0, 0], "axis": [0, 0, 1], "value": 30},
            // Half as fast the other way: −15° about Z through the origin.
            {"id": "mesh", "type": "gear", "parent": "base", "child": "pinion",
             "origin": [0, 0, 0], "axis": [0, 0, 1], "driver": "spin", "ratio": -0.5}
        ]);
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        // (40, 0, 5) turned 90° about Z → (0, 40, 5), then moved (10, 5, 0).
        assert!(
            near(com(&eval, "puck"), [10.0, 45.0, 5.0]),
            "{:?}",
            com(&eval, "puck")
        );
        // (0, 40, 5) turned 90° about Z → (−40, 0, 5).
        assert!(
            near(com(&eval, "ball"), [-40.0, 0.0, 5.0]),
            "{:?}",
            com(&eval, "ball")
        );
        let r = |deg: f64, p: [f64; 2]| {
            let (s, c) = deg.to_radians().sin_cos();
            [c * p[0] - s * p[1], s * p[0] + c * p[1], 5.0]
        };
        assert!(near(com(&eval, "wheel"), r(30.0, [-40.0, 0.0])));
        assert!(
            near(com(&eval, "pinion"), r(-15.0, [0.0, -40.0])),
            "{:?}",
            com(&eval, "pinion")
        );
        // A gear without a driver, or driving itself, is refused plainly.
        a["joints"][3]["driver"] = json!("mesh");
        let why = evaluate(&a).err().unwrap().join(" ");
        assert!(why.contains("loop of gears"), "{why}");
        a["joints"][3]["driver"] = json!("socket");
        let why = evaluate(&a).err().unwrap().join(" ");
        assert!(why.contains("is spherical"), "{why}");
    }

    #[test]
    fn instance_patterns_place_rows_grids_and_rings() {
        let mut a = cubes();
        a["assemblies"] = json!({"pair": {"instances": [
            {"id": "low", "part": "cube"}, {"id": "high", "part": "cube", "at": [0, 0, 10]}]}});
        a["instances"] = json!([
            {"id": "row", "part": "cube", "at": [0, -100, 0], "pattern": {"count": 3, "step": [20, 0, 0], "count2": 2, "step2": [0, -20, 0]}},
            {"id": "ring", "part": "cube", "at": [40, 0, 0], "pattern": {"count": 4, "axis": [0, 0, 1]}},
            {"id": "stacks", "assembly": "pair", "at": [0, 100, 0], "pattern": {"count": 2, "step": [30, 0, 0]}}
        ]);
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(
            eval.report["instances"].as_array().unwrap().len(),
            6 + 4 + 4
        );
        assert!(
            near(com(&eval, "row-6"), [40.0, -120.0, 5.0]),
            "{:?}",
            com(&eval, "row-6")
        );
        // The ring turns each copy about Z: the second sits at (0, 40).
        assert!(
            near(com(&eval, "ring-2"), [0.0, 40.0, 5.0]),
            "{:?}",
            com(&eval, "ring-2")
        );
        assert!(near(com(&eval, "ring-3"), [-40.0, 0.0, 5.0]));
        // A patterned subassembly copies its whole tree.
        assert!(near(com(&eval, "stacks-2.high"), [30.0, 100.0, 15.0]));
        // Each placed subassembly's mass: its two cubes, centred between them.
        let sub = &eval.report["subassemblies"][1];
        assert_eq!(sub["id"], "stacks-2", "{sub}");
        let one = eval.report["instances"][0]["mass_kg"].as_f64().unwrap();
        assert!(
            (sub["mass_kg"].as_f64().unwrap() - 2.0 * one).abs() < 1e-6,
            "{sub}"
        );
        let c = &sub["center_of_mass_mm"];
        assert!(
            near(
                [0, 1, 2].map(|k| c[k].as_f64().unwrap()),
                [30.0, 100.0, 10.0]
            ),
            "{c}"
        );
        a["instances"][1]["pattern"]["count"] = json!(0);
        let why = doc::check_assembly(&a).join(" ");
        assert!(why.contains("ring's pattern"), "{why}");
    }

    #[test]
    fn subassembly_templates_place_copies_with_their_own_joints() {
        let mut a = cubes();
        // A hinge: a cube with a lid cube on it, opened by a revolute.
        a["assemblies"] = json!({"hinge": {
            "title": "Hinge",
            "instances": [{"id": "frame", "part": "cube"},
                          {"id": "lid", "part": "cube", "parent": "frame", "at": [0, 0, 10]}],
            "joints": [{"id": "pin", "type": "revolute", "parent": "frame", "child": "lid",
                        "origin": [5, 0, 10], "axis": [0, 1, 0], "value": 0}]
        }});
        a["instances"] = json!([
            {"id": "left", "assembly": "hinge", "at": [0, 50, 0]},
            {"id": "right", "assembly": "hinge", "at": [0, -50, 0]}
        ]);
        // The outer assembly opens the right lid by its path.
        a["joints"] =
            json!([{"id": "open", "type": "fastened", "parent": "left", "child": "left.frame"}]);
        let mut opened = a.clone();
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(eval.report["instances"].as_array().unwrap().len(), 4);
        assert!(near(com(&eval, "left.lid"), [0.0, 50.0, 15.0]));
        assert!(near(com(&eval, "right.lid"), [0.0, -50.0, 15.0]));
        opened["assemblies"]["hinge"]["joints"][0]["value"] = json!(90);
        let eval = evaluate(&opened).unwrap_or_else(|p| panic!("{p:?}"));
        // Turned 90° about its edge at x = 5, z = 10: it flips over onto x 5..15.
        assert!(
            near(com(&eval, "right.lid"), [10.0, -50.0, 15.0]),
            "{:?}",
            com(&eval, "right.lid")
        );
        // Unknown and self-containing templates are refused plainly.
        a["instances"][0]["assembly"] = json!("nope");
        assert!(doc::check_assembly(&a).join(" ").contains("doesn't have"));
        a["instances"][0]["assembly"] = json!("hinge");
        a["assemblies"]["hinge"]["instances"][1] = json!({"id": "again", "assembly": "hinge"});
        assert!(
            doc::check_assembly(&a)
                .join(" ")
                .contains("contains itself")
        );
    }

    /// A bar of `length` between its pin holes' centres, 10 wide, 6 thick,
    /// lying along X from its first pin at the origin, `z` up.
    fn bar(id: &str, length: f64, z: f64) -> Value {
        let mut p = doc::new_part(id, id, "Aluminum", None);
        p["features"] = json!([{"id": "bar", "op": "solid.box", "params": {
            "length": length + 10.0, "width": 10, "height": 6, "center": [length / 2.0, 0, z + 3.0]}}]);
        p
    }

    #[test]
    fn a_four_bar_of_parts_closes_and_follows_its_driven_crank() {
        let (a, b, c, d) = (40.0, 120.0, 80.0, 100.0);
        let mut asm = doc::new_assembly("fourbar", "Four-bar");
        asm["parts"] = json!({"ground": bar("ground", d, 0.0), "crank": bar("crank", a, 6.0),
            "coupler": bar("coupler", b, 12.0), "rocker": bar("rocker", c, 6.0)});
        asm["instances"] = json!([
            {"id": "ground", "part": "ground"},
            {"id": "crank", "part": "crank", "parent": "ground"},
            {"id": "coupler", "part": "coupler", "parent": "crank", "at": [a, 0, 0]},
            {"id": "rocker", "part": "rocker", "parent": "ground", "at": [d, 0, 0]}
        ]);
        asm["joints"] = json!([
            {"id": "a", "type": "revolute", "parent": "ground", "child": "crank",
             "origin": [0, 0, 0], "axis": [0, 0, 1], "value": 30, "drive": true, "limits": [[-360, 360]]},
            {"id": "b", "type": "revolute", "parent": "crank", "child": "coupler",
             "origin": [a, 0, 0], "axis": [0, 0, 1], "value": 30},
            {"id": "d", "type": "revolute", "parent": "ground", "child": "rocker",
             "origin": [d, 0, 0], "axis": [0, 0, 1], "value": 70},
            {"id": "c", "type": "revolute", "closes": true, "parent": "coupler", "child": "rocker",
             "origin": [b, 0, 0], "axis": [0, 0, 1], "child_origin": [c, 0, 0]}
        ]);
        assert!(
            doc::check_assembly(&asm).is_empty(),
            "{:?}",
            doc::check_assembly(&asm)
        );
        let eval = evaluate(&asm).unwrap_or_else(|p| panic!("{p:?}"));
        let solver = &eval.report["solver"];
        assert!(solver["residual"].as_f64().unwrap() < 1e-6, "{solver}");
        assert_eq!(solver["loops"][0]["mobility"], 1, "{solver}");
        assert_eq!(solver["dof"], 0, "{solver}");
        // The rocker's middle where the closed-form linkage puts it.
        let t = 30f64.to_radians();
        let (bx, by) = (a * t.cos(), a * t.sin());
        let (dx, dy) = (d - bx, -by);
        let l = (dx * dx + dy * dy).sqrt();
        let along = (l * l + b * b - c * c) / (2.0 * l);
        let h = (b * b - along * along).sqrt();
        let (cx, cy) = (
            bx + (along * dx - h * dy) / l,
            by + (along * dy + h * dx) / l,
        );
        let phi = cy.atan2(cx - d);
        let want = [d + c / 2.0 * phi.cos(), c / 2.0 * phi.sin(), 9.0];
        assert!(
            near(com(&eval, "rocker"), want),
            "{:?} vs {want:?}",
            com(&eval, "rocker")
        );
        // Driving the rocker as well asks too much of a one-degree loop.
        asm["joints"][2]["drive"] = json!(true);
        let why = evaluate(&asm).err().unwrap().join(" ");
        assert!(
            why.contains("can't close") && why.contains("driven joints"),
            "{why}"
        );
        // A closing joint must say where it holds the child.
        asm["joints"][2]["drive"] = Value::Null;
        asm["joints"][3]["child_origin"] = Value::Null;
        assert!(doc::check_assembly(&asm).join(" ").contains("child_origin"));
    }

    #[test]
    fn mates_take_joint_frames_from_named_faces_and_edges() {
        let mut asm = doc::new_assembly("mates", "Mates");
        let mut base = doc::new_part("base", "Base", "Aluminum", None);
        base["features"] = json!([{"id": "base", "op": "solid.box", "params": {"length": 100, "width": 100, "height": 10, "center": [0, 0, 5]}}]);
        let mut lid = doc::new_part("lid", "Lid", "Aluminum", None);
        lid["features"] = json!([{"id": "lid", "op": "solid.box", "params": {"length": 100, "width": 100, "height": 4, "center": [0, 0, 2]}}]);
        let mut post = doc::new_part("post", "Post", "Steel", None);
        post["features"] = json!([{"id": "post", "op": "solid.cylinder", "params": {"radius": 5, "height": 30, "base": [30, 0, 0]}}]);
        let mut tab = doc::new_part("tab", "Tab", "Steel", None);
        tab["features"] = json!([{"id": "tab", "op": "solid.box", "params": {"length": 4, "width": 4, "height": 4, "center": [0, 0, 0]}}]);
        asm["parts"] = json!({"base": base, "lid": lid, "post": post, "tab": tab});
        asm["instances"] = json!([
            {"id": "base", "part": "base"},
            {"id": "lid", "part": "lid", "parent": "base", "at": [0, 0, 10]},
            {"id": "post", "part": "post", "at": [0, 200, 0]},
            {"id": "tab", "part": "tab", "parent": "post", "at": [42, 0, 15]}
        ]);
        asm["joints"] = json!([
            // The lid hinges on the base's top right edge (along Y).
            {"id": "hinge", "type": "revolute", "parent": "base", "child": "lid",
             "origin": {"edge": ["base:+z", "base:+x"]}, "axis": {"edge": ["base:+z", "base:+x"]}, "value": 90},
            // The tab turns about the post's round side.
            {"id": "swing", "type": "revolute", "parent": "post", "child": "tab",
             "origin": {"face": "post:side"}, "axis": {"face": "post:side"}, "value": 90}
        ]);
        assert!(
            doc::check_assembly(&asm).is_empty(),
            "{:?}",
            doc::check_assembly(&asm)
        );
        let eval = evaluate(&asm).unwrap_or_else(|p| panic!("{p:?}"));
        // About +Y through (50, 0, 10): the lid's centre (0, 0, 12) → (52, 0, 60).
        assert!(
            near(com(&eval, "lid"), [52.0, 0.0, 60.0]),
            "{:?}",
            com(&eval, "lid")
        );
        // A quarter turn about the post's axis (x = 30): (42, 200) → (30, 212).
        assert!(
            near(com(&eval, "tab"), [30.0, 212.0, 15.0]),
            "{:?}",
            com(&eval, "tab")
        );
        // The joint as built has numbers where the mates were.
        let hinge = &eval.built.assembly["joints"][0];
        assert!(doc::triple(&hinge["origin"]).is_some(), "{hinge}");
        asm["joints"][0]["origin"] = json!({"face": "base:nope"});
        let why = evaluate(&asm).err().unwrap().join(" ");
        assert!(why.contains("has no face base:nope"), "{why}");
    }

    #[test]
    fn a_thin_overlap_is_refused_and_resting_faces_are_contact() {
        // A 100 mm block, and a bracket whose top plate dips 0.2 mm into it
        // while its leg runs down beside it: the sampler stepped 3.6 mm
        // through their shared box and missed the 2000 mm³ slab.
        let mut asm = doc::new_assembly("sliver", "Sliver");
        let mut block = doc::new_part("block", "Block", "Steel", None);
        block["features"] = json!([{"id": "b", "op": "solid.box", "params": {"length": 100, "width": 100, "height": 100, "center": [50, 50, 50]}}]);
        let mut bracket = doc::new_part("bracket", "Bracket", "Steel", None);
        bracket["features"] = json!([{"id": "l", "op": "block", "params": {"ops": [
            {"op": "solid.box", "params": {"length": 111, "width": 100, "height": 10, "center": [55.5, 50, 5]}},
            {"op": "solid.box", "params": {"length": 10, "width": 100, "height": 100, "center": [106, 50, -50], "operation": "join"}}
        ]}}]);
        asm["parts"] = json!({"block": block, "bracket": bracket});
        asm["instances"] = json!([
            {"id": "block", "part": "block"},
            {"id": "bracket", "part": "bracket", "at": [0, 0, 99.8]}
        ]);
        let why = evaluate(&asm).err().unwrap().join(" ");
        assert!(why.contains("overlap by 2000 mm³"), "{why}");
        // Resting exactly on top: touching, allowed, and said.
        asm["instances"][1]["at"] = json!([0, 0, 100]);
        let eval = evaluate(&asm).unwrap_or_else(|p| panic!("{p:?}"));
        let check = &eval.report["checks"]["overlap_check"];
        assert_eq!(check["touching"], json!([["block", "bracket"]]), "{check}");
        // Measured once: the same edit again is remembered.
        let again = evaluate(&asm).unwrap_or_else(|p| panic!("{p:?}"));
        let check = &again.report["checks"]["overlap_check"];
        assert_eq!(check["measured"], 0, "{check}");
    }

    /// The interference check's time on a real document (the rover):
    /// `CAD_BENCH_DOC=path/to/doc.json cargo test --release -p cad --lib
    /// interference_time -- --ignored --nocapture`. Prints the first
    /// check, the same check again (every pair remembered), an edit that
    /// moves one instance (only its pairs measured), and the old 28³
    /// sampler on the same pairs.
    #[test]
    #[ignore = "needs CAD_BENCH_DOC"]
    fn interference_time() {
        let Some(path) = std::env::var_os("CAD_BENCH_DOC") else {
            return;
        };
        let document: Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut built = build::build(&document).unwrap_or_else(|p| panic!("{p:?}"));
        let pairs = |b: &mut Built, label: &str| {
            let (problems, _) = build::interference(b, &document);
            println!("{label}: {} problems {problems:?}", b.interference);
        };
        pairs(&mut built, "first check");
        pairs(&mut built, "again");
        let started = sys::Timer::start();
        let solids = &built.solids;
        let mut old = 0;
        for i in 0..solids.len() {
            for j in i + 1..solids.len() {
                if solids[i].instance != solids[j].instance {
                    let (a, b) = (
                        overlap::bounds(&solids[i].mesh),
                        overlap::bounds(&solids[j].mesh),
                    );
                    if (0..3).all(|k| a.0[k] < b.1[k] && b.0[k] < a.1[k]) {
                        let _ = overlap::volume(&solids[i].mesh, &solids[j].mesh);
                        old += 1;
                    }
                }
            }
        }
        println!(
            "old sampler, one thread: {old} pairs in {} ms",
            started.ms()
        );
        // An edit: one instance moves 1 mm; only the pairs it is in change.
        let mut moved = document.clone();
        let assembly = doc::as_assembly(&moved);
        let pick = std::env::var("CAD_BENCH_MOVE").ok();
        let last = assembly["instances"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|i| match &pick {
                Some(id) => i["id"] == id.as_str(),
                None => i["part"].is_string() && i["id"].as_str().is_some_and(|s| !s.contains('.')),
            })
            .cloned()
            .unwrap();
        for i in moved["instances"].as_array_mut().into_iter().flatten() {
            if i["id"] == last["id"] {
                let at = doc::triple(&i["at"]).unwrap_or([0.0; 3]);
                i["at"] = json!([at[0] + 1.0, at[1], at[2]]);
            }
        }
        let started = sys::Timer::start();
        let mut built = build::build(&moved).unwrap_or_else(|p| panic!("{p:?}"));
        let build_ms = started.ms();
        let (problems, _) = build::interference(&mut built, &moved);
        println!(
            "edit (moved {} 1 mm): build {build_ms} ms, check {} problems {problems:?}",
            last["id"], built.interference
        );
    }

    #[test]
    fn overlaps_are_refused_unless_declared_and_joints_move_children() {
        let mut a = doc::new_assembly("pair", "Pair");
        a["parts"]["block"] = doc::new_part("block", "Block", "Steel", None);
        a["parts"]["block"]["features"] = json!([{"id": "b", "op": "solid.box", "params": {"length": 40, "width": 40, "height": 40, "center": [0, 0, 20]}}]);
        a["instances"] = json!([{"id": "one", "part": "block"}, {"id": "two", "part": "block", "at": [30, 0, 0]}]);
        let why = evaluate(&a).err().unwrap().join(" ");
        assert!(
            why.contains("one and two overlap") || why.contains("two and one overlap"),
            "{why}"
        );
        a["contacts"] = json!([["one", "two"]]);
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(
            eval.report["checks"]["contacts"].as_array().unwrap().len(),
            1
        );
        // A hinge turns the second block up and away: no contact needed.
        a["contacts"] = json!([]);
        a["instances"][1]["at"] = json!([60, 0, 0]);
        a["instances"][1]["parent"] = json!("one");
        a["joints"] = json!([{"id": "hinge", "type": "revolute", "parent": "one", "child": "two",
            "origin": [20, 0, 40], "axis": [0, -1, 0], "value": 90, "limits": [[0, 120]]}]);
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        let com = &eval.report["instances"][1]["center_of_mass_mm"];
        // Turned 90° about the hinge at (20, 0, 40): the centre (60, 0, 20) → (40, 0, 80).
        assert!((com[0].as_f64().unwrap() - 40.0).abs() < 0.5, "{com}");
        assert!((com[2].as_f64().unwrap() - 80.0).abs() < 0.5, "{com}");
    }

    /// A base with an arm on a hinge, a carriage on a rail, a spindle that
    /// turns and slides, and a tip fixed to the arm, every joint away from
    /// zero.
    fn machine() -> Value {
        let mut a = doc::new_assembly("machine", "Machine");
        let block = |id: &str, material: &str, l: f64, w: f64, h: f64| {
            let mut p = doc::new_part(id, id, material, None);
            p["features"] = json!([{"id": "b", "op": "solid.box", "params": {"length": l, "width": w, "height": h, "center": [0, 0, h / 2.0]}}]);
            p
        };
        a["parts"]["base"] = block("base", "Steel", 200.0, 100.0, 40.0);
        a["parts"]["arm"] = block("arm", "Aluminum", 20.0, 20.0, 120.0);
        a["parts"]["carriage"] = block("carriage", "Brass", 30.0, 30.0, 10.0);
        a["parts"]["tip"] = block("tip", "Titanium", 10.0, 10.0, 30.0);
        a["parts"]["spindle"] = doc::new_part("spindle", "spindle", "ABS Plastic", None);
        a["parts"]["spindle"]["features"] = json!([{"id": "c", "op": "solid.box", "params": {"length": 24, "width": 8, "height": 20, "center": [4, 0, 10]}}]);
        a["instances"] = json!([
            {"id": "base", "part": "base"},
            {"id": "arm", "part": "arm", "parent": "base", "at": [80, 0, 40], "rotate": [0, 0, 30]},
            {"id": "carriage", "part": "carriage", "parent": "base", "at": [-60, 0, 40]},
            {"id": "spindle", "part": "spindle", "parent": "base", "at": [0, 30, 40]},
            {"id": "tip", "part": "tip", "parent": "arm", "at": [0, 0, 120], "rotate": [90, 0, 0]}
        ]);
        a["joints"] = json!([
            {"id": "shoulder", "type": "revolute", "parent": "base", "child": "arm",
             "origin": [80, 0, 40], "axis": [0, 1, 0], "value": 30, "limits": [[-90, 90]]},
            {"id": "rail", "type": "slider", "parent": "base", "child": "carriage",
             "origin": [-60, 0, 40], "axis": [1, 0, 0], "value": 15, "limits": [[-20, 40]]},
            {"id": "spin", "type": "cylindrical", "parent": "base", "child": "spindle",
             "origin": [0, 30, 40], "axis": [0, 0, 1], "value": [45, 5], "limits": [[-180, 180], [0, 10]]}
        ]);
        a["contacts"] = json!([["arm", "tip"]]);
        a
    }

    #[test]
    fn urdf_round_trips_mass_inertia_and_joints_through_robot_description() {
        use glam::{DMat3, DVec3};
        let a = machine();
        let eval = evaluate(&a).unwrap_or_else(|p| panic!("{p:?}"));
        let out = urdf::write("machine", &a, &eval.report, Some(&eval.built.solids)).unwrap();
        let dir = std::env::temp_dir().join(format!("cad-urdf-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("meshes")).unwrap();
        std::fs::write(dir.join("machine.urdf"), &out.text).unwrap();
        for (path, bytes) in &out.meshes {
            std::fs::write(dir.join(path), bytes).unwrap();
        }
        let robot = robot_description::load(&dir.join("machine.urdf"))
            .unwrap_or_else(|e| panic!("{e}\n{}", out.text));
        // The version's own pose, in URDF units.
        let pose: Vec<f64> = robot.movable().map(|j| out.pose[&j.name]).collect();
        let frames = robot.forward(&pose).unwrap();
        for entry in eval.report["instances"].as_array().unwrap() {
            let id = entry["id"].as_str().unwrap();
            let link = robot.link(id).unwrap();
            let inertial = robot.links()[link].inertial.expect("an inertial");
            let frame = frames[link];
            assert!((inertial.mass - entry["mass_kg"].as_f64().unwrap()).abs() < 1e-12);
            let com = frame.transform_point3(inertial.origin.w_axis.truncate()) * 1000.0;
            let want = doc::triple(&entry["center_of_mass_mm"]).unwrap();
            assert!(
                (com - DVec3::from(want)).length() < 0.06,
                "{id}: {com} vs {want:?}"
            );
            let r = DMat3::from_mat4(frame);
            let world = r * inertial.inertia_in_link() * r.transpose();
            for row in 0..3 {
                for col in 0..3 {
                    let want = entry["inertia_kg_m2"][row][col].as_f64().unwrap();
                    let got = world.col(col)[row];
                    assert!(
                        (got - want).abs() <= 1e-9 + 1e-6 * want.abs(),
                        "{id} [{row}][{col}]: {got} vs {want}"
                    );
                }
            }
            // Its meshes sit where its solids are.
            let solid = eval.built.solids.iter().find(|s| s.instance == id).unwrap();
            let q = solid.mesh.positions[0];
            let near = robot.links()[link].visuals.iter().any(|v| {
                let robot_description::Geometry::Mesh { mesh, .. } = &v.geometry else {
                    return false;
                };
                mesh.triangles.iter().flatten().any(|p| {
                    let w =
                        frame.transform_point3(DVec3::new(p[0].into(), p[1].into(), p[2].into()));
                    (w * 1000.0 - DVec3::new(q.x, q.y, q.z)).length() < 0.01
                })
            });
            assert!(near, "{id}'s mesh isn't where its solid is");
        }
        // Every joint, with its kind and limits.
        let joint = |name: &str| robot.joints().iter().find(|j| j.name == name).unwrap();
        use robot_description::JointKind::{Fixed, Prismatic, Revolute};
        let pi = std::f64::consts::PI;
        assert_eq!(joint("shoulder").kind, Revolute);
        let l = joint("shoulder").limit.unwrap();
        assert!((l.lower + pi / 2.0).abs() < 1e-12 && (l.upper - pi / 2.0).abs() < 1e-12);
        assert_eq!(joint("rail").kind, Prismatic);
        let l = joint("rail").limit.unwrap();
        assert!((l.lower + 0.02).abs() < 1e-12 && (l.upper - 0.04).abs() < 1e-12);
        assert_eq!(joint("spin").kind, Revolute);
        assert_eq!(joint("spin_slide").kind, Prismatic);
        assert_eq!(joint("tip_fixed").kind, Fixed);
        assert_eq!(joint("base_fixed").kind, Fixed);
        assert_eq!(robot.movable().count(), 4);
        assert!((out.pose["shoulder"] - 30f64.to_radians()).abs() < 1e-12);
        assert!((out.pose["spin_slide"] - 0.005).abs() < 1e-12);
        // The mass feed: every part, ports on both sides, and the total
        // is the report's.
        let feed = feed::feed(
            feed::Source {
                doc: "machine".into(),
                version: 1,
                digest: "0".into(),
            },
            &a,
            &eval.report,
        )
        .unwrap();
        let text = serde_json::to_string(&feed).unwrap();
        let back: feed::MassFeed = serde_json::from_str(&text).unwrap();
        assert_eq!(back, feed);
        assert_eq!(feed.parts.len(), 5);
        let total = eval.report["mass"]["mass_kg"].as_f64().unwrap();
        assert!(
            (feed.total.mass_kg - total).abs() < 1e-3,
            "{}",
            feed.total.mass_kg
        );
        let com = doc::triple(&eval.report["mass"]["center_of_mass_mm"]).unwrap();
        for k in 0..3 {
            assert!((feed.total.center_of_mass_m[k] * 1000.0 - com[k]).abs() < 0.1);
            for j in 0..3 {
                let want = eval.report["mass"]["inertia_kg_m2"][k][j].as_f64().unwrap();
                assert!((feed.total.inertia_kg_m2[k][j] - want).abs() < 1e-6 + 1e-4 * want.abs());
            }
        }
        let base = feed.parts.iter().find(|p| p.id == "base").unwrap();
        assert_eq!(base.ports.len(), 3);
        assert!(base.ports.iter().all(|p| p.parent));
        let arm = feed.parts.iter().find(|p| p.id == "arm").unwrap();
        assert_eq!(arm.ports[0].mate, "base");
        assert!((arm.ports[0].origin_m[0] - 0.08).abs() < 1e-12);
        let _ = std::fs::remove_dir_all(dir);
    }
}
