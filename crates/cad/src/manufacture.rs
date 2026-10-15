//! Geometric print and three-axis milling checks on named CAD faces.
use crate::{build::Built, clash::Bvh, xf::Xf};
use glam::DVec3 as V;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const EPS: f64 = 0.0001;
const SAMPLES: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub process: String,
    pub min_wall_mm: f64,
    pub overhang_degrees: f64,
    pub bridge_mm: f64,
    pub nozzle_mm: f64,
    pub tool_radius_mm: f64,
    pub up: [f64; 3],
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            process: "print".into(),
            min_wall_mm: 0.8,
            overhang_degrees: 45.0,
            bridge_mm: 10.0,
            nozzle_mm: 0.4,
            tool_radius_mm: 3.0,
            up: [0.0, 0.0, 1.0],
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.process.as_str(), "print" | "cnc" | "both")
            || ![
                self.min_wall_mm,
                self.bridge_mm,
                self.nozzle_mm,
                self.tool_radius_mm,
            ]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 100_000.0)
            || !self.overhang_degrees.is_finite()
            || !(0.0..90.0).contains(&self.overhang_degrees)
            || !self.up.iter().all(|v| v.is_finite())
            || !V::from(self.up).length().is_finite()
            || V::from(self.up).length() < 1e-6
        {
            return Err("Choose print, cnc, or both, positive dimensions, an angle below 90 degrees, and a build direction.".into());
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Triangle {
    p: [V; 3],
    center: V,
    normal: V,
    area: f64,
}
struct Face {
    solid: usize,
    name: String,
    triangles: Vec<Triangle>,
}
fn point(v: solidcraft_engine::geom::Vec3) -> V {
    V::new(v.x, v.y, v.z)
}
fn v3(value: &Value) -> Option<V> {
    Some(V::new(
        value[0].as_f64()?,
        value[1].as_f64()?,
        value[2].as_f64()?,
    ))
}
fn scene(trees: &[Bvh], p: V, d: V, limit: f64) -> Option<f64> {
    trees
        .iter()
        .filter_map(|tree| tree.distance(p.to_array(), d.to_array(), limit))
        .min_by(f64::total_cmp)
}

/// A concave join between flat faces has zero inside radius. Use a
/// triangle's directed edge so convex outer edges aren't reported.
fn sharp_inside(
    mesh: &solidcraft_engine::geom::Mesh,
    names: &[Option<String>],
) -> std::collections::BTreeSet<String> {
    let mut sums = BTreeMap::<u32, V>::new();
    for (indices, face) in mesh.triangles.iter().zip(&mesh.tri_face) {
        let p = indices.map(|i| point(mesh.positions[i as usize]));
        *sums.entry(*face).or_default() += (p[1] - p[0]).cross(p[2] - p[0]);
    }
    let normals: BTreeMap<u32, V> = sums
        .into_iter()
        .map(|(f, n)| (f, n.normalize_or_zero()))
        .collect();
    let mut curved = std::collections::BTreeSet::new();
    for (indices, face) in mesh.triangles.iter().zip(&mesh.tri_face) {
        let p = indices.map(|i| point(mesh.positions[i as usize]));
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        if normals[face].dot(n) < 0.9999 {
            curved.insert(*face);
        }
    }
    let mut weld = BTreeMap::new();
    let mut vertices = Vec::new();
    for p in &mesh.positions {
        let key = [p.x, p.y, p.z].map(|v| (v * 1e6).round() as i64);
        let next = weld.len();
        vertices.push(*weld.entry(key).or_insert(next));
    }
    let mut edges = BTreeMap::<(usize, usize), (u32, V)>::new();
    let mut result = std::collections::BTreeSet::new();
    for (indices, face) in mesh.triangles.iter().zip(&mesh.tri_face) {
        if curved.contains(face) {
            continue;
        }
        for (a, b) in [
            (indices[0], indices[1]),
            (indices[1], indices[2]),
            (indices[2], indices[0]),
        ] {
            let (va, vb) = (vertices[a as usize], vertices[b as usize]);
            if va == vb {
                continue;
            }
            let key = if va < vb { (va, vb) } else { (vb, va) };
            if let Some((other, tangent)) = edges.get(&key) {
                if *other != *face && normals[other].cross(normals[face]).dot(*tangent) < -1e-5 {
                    for id in [*other, *face] {
                        if let Some(Some(name)) = names.get(id as usize) {
                            result.insert(name.clone());
                        }
                    }
                }
            } else {
                edges.insert(
                    key,
                    (
                        *face,
                        (point(mesh.positions[b as usize]) - point(mesh.positions[a as usize]))
                            .normalize_or_zero(),
                    ),
                );
            }
        }
    }
    result
}

/// Report measurements and named failing faces. Curves use the kernel's
/// display tessellation; sampling is bounded and never a production approval.
pub fn check(built: &Built, profile: &Profile) -> Result<Value, String> {
    profile.validate()?;
    let up = V::from(profile.up).normalize();
    let u = up
        .cross(if up.x.abs() < 0.9 { V::X } else { V::Y })
        .normalize();
    let v = up.cross(u);
    let trees: Vec<Bvh> = built
        .solids
        .iter()
        .map(|s| Bvh::new(&s.mesh, [0.; 3]))
        .collect();
    let bed = built
        .solids
        .iter()
        .flat_map(|s| s.mesh.positions.iter())
        .map(|p| point(*p).dot(up))
        .min_by(f64::total_cmp)
        .ok_or("This design has no geometry to check.")?;
    let limit = built
        .solids
        .iter()
        .flat_map(|s| s.mesh.positions.iter())
        .map(|p| point(*p).length())
        .fold(1.0, f64::max)
        * 4.0
        + 1.0;
    let mut faces = Vec::new();
    let mut sharp = std::collections::BTreeSet::new();
    let mut unnamed = 0;
    for (si, solid) in built.solids.iter().enumerate() {
        let xf = built
            .placements
            .get(&solid.instance)
            .copied()
            .unwrap_or(Xf::IDENTITY);
        let names = built
            .part_frames
            .get(&solid.part)
            .map(|frames| crate::view::face_names(solid, &xf, frames))
            .unwrap_or_default();
        if profile.process != "print" {
            for name in sharp_inside(&solid.mesh, &names) {
                sharp.insert((si, name));
            }
        }
        let mut groups = BTreeMap::<u32, Vec<Triangle>>::new();
        for (indices, face) in solid.mesh.triangles.iter().zip(&solid.mesh.tri_face) {
            let p = indices.map(|i| point(solid.mesh.positions[i as usize]));
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            let area = n.length() / 2.0;
            if area > 1e-12 {
                groups.entry(*face).or_default().push(Triangle {
                    p,
                    center: (p[0] + p[1] + p[2]) / 3.0,
                    normal: n.normalize(),
                    area,
                });
            }
        }
        for (id, triangles) in groups {
            if let Some(Some(name)) = names.get(id as usize) {
                faces.push(Face {
                    solid: si,
                    name: name.clone(),
                    triangles,
                });
            } else {
                unnamed += 1;
            }
        }
    }
    if faces.len() > 4096
        || built
            .solids
            .iter()
            .map(|s| s.mesh.triangles.len())
            .sum::<usize>()
            > 200_000
    {
        return Err(
            "Check a smaller part: this model is too large for these surface checks.".into(),
        );
    }
    if faces.is_empty() {
        return Err("These checks need named CAD faces. Import an exact STEP model or build the part with CAD features.".into());
    }
    let mut findings = Vec::new();
    let mut sampled = 0usize;
    let mut unmeasured = 0usize;
    let mut measurements = Vec::new();
    for face in &faces {
        let solid = &built.solids[face.solid];
        let stride = face.triangles.len().div_ceil(SAMPLES).max(1);
        let samples: Vec<&Triangle> = face
            .triangles
            .iter()
            .step_by(stride)
            .take(SAMPLES)
            .collect();
        sampled += samples.len();
        let mut wall = f64::INFINITY;
        let mut angle = 0.0f64;
        let mut bridge = 0.0f64;
        let mut unreachable = false;
        for t in &samples {
            if let Some(distance) = trees[face.solid].distance(
                (t.center - t.normal * EPS).to_array(),
                (-t.normal).to_array(),
                limit,
            ) {
                wall = wall.min(distance + EPS);
            }
            let below = t.center + t.normal * EPS;
            if t.normal.dot(up) < -1e-6 && t.center.dot(up) > bed + 0.01 {
                let mut span = f64::INFINITY;
                for axis in [u, v] {
                    if let (Some(a), Some(b)) = (
                        scene(&trees, below, axis, limit),
                        scene(&trees, below, -axis, limit),
                    ) {
                        span = span.min(a + b);
                    }
                }
                if span.is_finite() {
                    bridge = bridge.max(span);
                }
                if !span.is_finite() || span > profile.bridge_mm {
                    angle = angle.max((-t.normal.dot(up)).clamp(0., 1.).asin().to_degrees());
                }
            }
            if t.normal.dot(up) < -1e-4 {
                unreachable = true;
            } else {
                let sideways = t.normal - up * t.normal.dot(up);
                let center = t.center
                    + t.normal * EPS
                    + if t.normal.dot(up) < 0.05 {
                        sideways.normalize_or_zero() * (profile.tool_radius_mm + EPS)
                    } else {
                        V::ZERO
                    };
                if [
                    V::ZERO,
                    u * profile.tool_radius_mm,
                    -u * profile.tool_radius_mm,
                    v * profile.tool_radius_mm,
                    -v * profile.tool_radius_mm,
                ]
                .into_iter()
                .any(|offset| scene(&trees, center + offset, up, limit).is_some())
                {
                    unreachable = true;
                }
            }
        }
        if profile.process != "cnc" && !wall.is_finite() {
            unmeasured += 1;
            findings.push(json!({"part":solid.part,"instance":solid.instance,"face":face.name,"check":"wall","value":null,"limit":profile.min_wall_mm,"unit":"mm","message":"Wall thickness couldn't be measured on this face."}));
        }
        let mut add = |check: &str, value: f64, threshold: f64, unit: &str, message: &str| {
            findings.push(json!({"part":solid.part,"instance":solid.instance,"face":face.name,"check":check,"value":value,"limit":threshold,"unit":unit,"message":message}));
        };
        if profile.process != "cnc" {
            if wall < profile.min_wall_mm {
                add(
                    "wall",
                    wall,
                    profile.min_wall_mm,
                    "mm",
                    "Wall is thinner than the selected minimum.",
                );
            }
            if angle > profile.overhang_degrees + 1e-6 {
                add(
                    "overhang",
                    angle,
                    profile.overhang_degrees,
                    "degrees",
                    "This overhang needs support or another print orientation.",
                );
            }
            if bridge > profile.bridge_mm {
                add(
                    "bridge",
                    bridge,
                    profile.bridge_mm,
                    "mm",
                    "Bridge is longer than the selected limit.",
                );
            }
        }
        if profile.process != "print" && unreachable {
            add(
                "tool_reach",
                1.0,
                0.0,
                "",
                "This face needs a different milling setup or a smaller tool.",
            );
        }
        if profile.process != "print" && sharp.contains(&(face.solid, face.name.clone())) {
            add(
                "inside_corner",
                0.0,
                profile.tool_radius_mm,
                "mm",
                "Add an inside corner radius for this tool.",
            );
        }
        let tris: Vec<_> = face
            .triangles
            .iter()
            .map(|t| (t.area, t.center.to_array(), t.normal.to_array()))
            .collect();
        if let Some(frame) = crate::refs::face_frame(&tris) {
            if frame.get("normal").is_none() {
                if let (Some(axis), Some(center)) = (v3(&frame["axis"]), v3(&frame["centre"])) {
                    let mut radii = Vec::new();
                    let mut concavity = 0.0;
                    let mut directions = V::ZERO;
                    let mut weight = 0.0;
                    for t in &face.triangles {
                        let delta = t.center - center;
                        let radial = delta - axis * delta.dot(axis);
                        if radial.length() > EPS {
                            let direction = radial.normalize();
                            concavity += t.area * t.normal.dot(direction);
                            directions += direction * t.area;
                            weight += t.area;
                        }
                        for p in t.p {
                            let delta = p - center;
                            radii.push((delta - axis * delta.dot(axis)).length());
                        }
                    }
                    radii.sort_by(f64::total_cmp);
                    if weight > 0.0 && concavity / weight < -0.5 && !radii.is_empty() {
                        let radius = radii[radii.len() / 2];
                        let spread = radii.last().unwrap() - radii[0];
                        if spread < radius * 0.05 + 0.01 {
                            if profile.process != "cnc"
                                && directions.length() / weight < 0.3
                                && 2.0 * radius < profile.nozzle_mm
                            {
                                add(
                                    "hole",
                                    2.0 * radius,
                                    profile.nozzle_mm,
                                    "mm",
                                    "Hole is smaller than the selected nozzle.",
                                );
                            }
                            if profile.process != "print" && radius < profile.tool_radius_mm {
                                add(
                                    "inside_corner",
                                    radius,
                                    profile.tool_radius_mm,
                                    "mm",
                                    "Inside radius is smaller than the selected tool.",
                                );
                            }
                        }
                    }
                }
            }
        }
        if wall.is_finite() {
            measurements.push(json!({"part":solid.part,"instance":solid.instance,"face":face.name,"wall_mm":wall}));
        }
    }
    let total_findings = findings.len();
    findings.truncate(512);
    measurements.truncate(512);
    Ok(
        json!({"schema":"openagents.cad.manufacture.v1","profile":profile,"passed":findings.is_empty()&&unnamed==0&&unmeasured==0,"complete":unnamed==0&&unmeasured==0,"faces_checked":faces.len(),"unmatched_faces":unnamed,"unmeasured_faces":unmeasured,"samples":sampled,"samples_per_face":SAMPLES,"findings_total":total_findings,"truncated":total_findings>512,"findings":findings,"measurements":measurements,"method":"triangle surface samples"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked(features: Value, profile: Profile) -> Value {
        let doc = json!({"kind":"part.v1","id":"bracket","features":features});
        let eval = crate::evaluate(&doc).unwrap_or_else(|p| panic!("{p:?}"));
        check(&eval.built, &profile).unwrap()
    }
    fn has(report: &Value, check: &str, face: &str) -> bool {
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["check"] == check && f["face"].as_str().unwrap().starts_with(face))
    }
    #[test]
    fn walls_are_measured_and_named_and_a_thicker_wall_passes() {
        let plate = |height: f64| json!([{"id":"body","op":"solid.box","params":{"length":20,"width":10,"height":height,"center":[0,0,height/2.]}}]);
        let thin = checked(plate(0.3), Profile::default());
        assert!(has(&thin, "wall", "body:+z"), "{thin}");
        let thick = checked(plate(3.0), Profile::default());
        assert!(
            !thick["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["check"] == "wall"),
            "{thick}"
        );
        let tilted = checked(
            plate(0.3),
            Profile {
                up: [1., 0., 0.],
                ..Profile::default()
            },
        );
        assert!(has(&tilted, "wall", "body:+z"));
    }
    #[test]
    fn cantilevers_and_long_bridges_name_their_undersides() {
        let mut features = vec![
            json!({"id":"left","op":"solid.box","params":{"length":2,"width":10,"height":20,"center":[-9,0,10]}}),
            json!({"id":"beam","op":"solid.box","params":{"length":20,"width":10,"height":2,"center":[0,0,19],"operation":"join"}}),
        ];
        let cantilever = checked(json!(features), Profile::default());
        assert!(has(&cantilever, "overhang", "beam:-z"), "{cantilever}");
        features.insert(1,json!({"id":"right","op":"solid.box","params":{"length":2,"width":10,"height":20,"center":[9,0,10],"operation":"new"}}));
        let bridge = checked(json!(features), Profile::default());
        assert!(has(&bridge, "bridge", "beam:-z"), "{bridge}");
    }
    #[test]
    fn small_holes_and_inside_radii_use_the_selected_nozzle_and_tool() {
        let features = json!([{"id":"body","op":"solid.box","params":{"length":20,"width":20,"height":5,"center":[0,0,2.5]}},{"id":"hole","op":"solid.cylinder","params":{"radius":1,"height":9,"base":[0,0,-2],"operation":"cut"}}]);
        let report = checked(
            features,
            Profile {
                process: "both".into(),
                nozzle_mm: 3.0,
                tool_radius_mm: 3.0,
                ..Profile::default()
            },
        );
        assert!(has(&report, "hole", "hole:side"), "{report}");
        assert!(has(&report, "inside_corner", "hole:side"), "{report}");
        assert!(has(&report, "tool_reach", "body:-z"), "{report}");
        assert!(!has(&report, "tool_reach", "body:+z"), "{report}");
    }
    #[test]
    fn sharp_concave_edges_fail_while_convex_box_edges_do_not() {
        let box_feature = json!({"id":"body","op":"solid.box","params":{"length":20,"width":20,"height":5,"center":[0,0,2.5]}});
        let profile = Profile {
            process: "cnc".into(),
            ..Profile::default()
        };
        let plain = checked(json!([box_feature]), profile.clone());
        assert!(
            !plain["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["check"] == "inside_corner"),
            "{plain}"
        );
        let notch = json!({"id":"notch","op":"solid.box","params":{"length":15,"width":15,"height":9,"center":[7.5,7.5,2.5],"operation":"cut"}});
        let cut = checked(json!([box_feature, notch]), profile);
        assert!(has(&cut, "inside_corner", "notch:"), "{cut}");
    }

    #[test]
    fn an_upward_face_below_a_roof_is_out_of_reach() {
        let profile = Profile {
            process: "cnc".into(),
            tool_radius_mm: 0.5,
            ..Profile::default()
        };
        let report = checked(
            json!([
                {"id":"floor","op":"solid.box","params":{"length":20,"width":10,"height":2,"center":[0,0,1]}},
                {"id":"left","op":"solid.box","params":{"length":2,"width":10,"height":20,"center":[-9,0,10],"operation":"join"}},
                {"id":"right","op":"solid.box","params":{"length":2,"width":10,"height":20,"center":[9,0,10],"operation":"join"}},
                {"id":"roof","op":"solid.box","params":{"length":20,"width":10,"height":2,"center":[0,0,19],"operation":"join"}}
            ]),
            profile,
        );
        assert!(has(&report, "tool_reach", "floor:+z"), "{report}");
    }

    #[test]
    fn profiles_refuse_invalid_units_and_directions() {
        assert!(
            Profile {
                up: [0.; 3],
                ..Profile::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Profile {
                tool_radius_mm: -1.,
                ..Profile::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Profile {
                overhang_degrees: 90.,
                ..Profile::default()
            }
            .validate()
            .is_err()
        );
    }
}
