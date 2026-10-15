//! URDF from an `assembly.v1` (docs/cad-lab/README.md, "Robot Lab and
//! Lagrange 2"): a link per instance with its measured mass, centre of mass
//! and inertia, and a joint per assembly joint with its limits, so Robot
//! Lab (`robot-description`) reads a CAD Lab version as a robot.
//!
//! - **Links.** One per instance, named by its id, under a massless
//!   `base_link` at the assembly's origin (the ground). An instance moved by
//!   a revolute, slider or cylindrical joint has its link frame at the
//!   joint's origin, turned as its parent instance is, so the URDF joint
//!   turns about the link frame's own origin as URDF requires; any other
//!   instance's link frame is the instance's own frame.
//! - **Inertials.** The check report's mass, centre of mass and inertia
//!   about it (assembly axes) are carried into each link's frame: the
//!   centre as the inertial origin (metres), the tensor turned into the
//!   link's axes (kg m²).
//! - **Joints.** revolute → `revolute` (degrees → radians; unlimited, or
//!   beyond a turn each way → `continuous`), slider → `prismatic` (mm →
//!   m), cylindrical → a `prismatic` to a massless `<child>_slide` link
//!   and a `revolute` from it, fastened (or none) → `fixed`. The URDF's
//!   zero pose is the assembly with every joint at zero; the version's own
//!   joint values are in [`Urdf::pose`].
//! - **Visuals.** With the built solids, one binary STL per instance and
//!   colour (metres, in the link's frame) under `meshes/`.
//!
//! URDF lengths are metres and angles radians; the assembly's are mm and
//! degrees. Planar, spherical and gear joints are refused plainly, as the
//! build refuses them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::Value;

use crate::build::{self, Solid, joint_motion};
use crate::doc;
use crate::xf::Xf;

/// The assembly joint kinds a URDF carries.
pub const URDF_JOINTS: [&str; 4] = ["fastened", "revolute", "slider", "cylindrical"];

/// The base link every root instance hangs from.
pub const BASE: &str = "base_link";

/// The most links and meshes a URDF may carry (Robot Lab's bounds).
pub const MAX_LINKS: usize = 64;
pub const MAX_MESHES: usize = 64;

/// A moving joint's effort and speed ratings, which an assembly doesn't
/// carry: N·m (or N) and rad/s (or m/s).
pub const EFFORT: f64 = 100.0;
pub const VELOCITY: f64 = 2.0;

/// A URDF written from a version.
pub struct Urdf {
    /// The description.
    pub text: String,
    /// Mesh files the description reads: path (relative, `meshes/...`) and
    /// binary STL bytes.
    pub meshes: Vec<(String, Vec<u8>)>,
    /// The version's joint values in URDF units: joint name → radians or
    /// metres.
    pub pose: BTreeMap<String, f64>,
}

/// One link's frame and its instance's frame in it.
struct Frames {
    /// The link frame in the assembly at the version's joint values.
    link: Xf,
    /// The instance frame in the link frame (constant).
    offset: Xf,
}

fn mm(p: [f64; 3]) -> [f64; 3] {
    p.map(|x| x / 1000.0)
}

fn num(x: f64) -> String {
    let x = if x.abs() < 1e-15 { 0.0 } else { x };
    format!("{x}")
}

fn v(p: [f64; 3]) -> String {
    format!("{} {} {}", num(p[0]), num(p[1]), num(p[2]))
}

fn origin(xf: &Xf) -> String {
    format!("<origin xyz=\"{}\" rpy=\"{}\"/>", v(mm(xf.t)), v(xf.rpy()))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `rᵀ · i · r`: a tensor in the frame whose axes are `r`'s columns.
fn into_frame(r: &[[f64; 3]; 3], i: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (a, row) in out.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            *cell = (0..3)
                .flat_map(|k| (0..3).map(move |l| (k, l)))
                .map(|(k, l)| r[k][a] * i[k][l] * r[l][b])
                .sum();
        }
    }
    out
}

fn matrix(v: &Value) -> Option<[[f64; 3]; 3]> {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        *row = doc::triple(&v[i])?;
    }
    Some(out)
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    a.map(|x| x / n)
}

/// A joint's value list with every value at zero (the URDF's zero pose).
fn zeroed(assembly: &Value) -> Value {
    let mut a = assembly.clone();
    for joint in a["joints"].as_array_mut().into_iter().flatten() {
        let n = doc::joint_values(joint).len();
        joint["value"] = match n {
            0 => Value::Null,
            1 => serde_json::json!(0),
            _ => serde_json::json!(vec![0; n]),
        };
    }
    a
}

/// Writes the URDF of `assembly` (or a part, as one link) from its check
/// `report` (`cad::evaluate`'s), with STL visuals when `solids` are given.
///
/// # Errors
/// A joint kind URDF can't say, too many links or meshes, or a report
/// without an instance's mass.
pub fn write(
    name: &str,
    assembly: &Value,
    report: &Value,
    solids: Option<&[Solid]>,
) -> Result<Urdf, String> {
    let assembly = doc::as_assembly(assembly);
    let instances: Vec<&Value> = assembly["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let joints: BTreeMap<&str, &Value> = assembly["joints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|j| Some((j["child"].as_str()?, j)))
        .collect();
    for joint in joints.values() {
        let kind = joint["type"].as_str().unwrap_or_default();
        if !URDF_JOINTS.contains(&kind) {
            return Err(format!(
                "Joint {} is {kind}; URDF from CAD Lab carries fastened, revolute, slider and cylindrical joints (Robot Lab reads no planar, ball or geared joints).",
                joint["id"].as_str().unwrap_or_default()
            ));
        }
    }
    let ids: BTreeSet<&str> = instances.iter().filter_map(|i| i["id"].as_str()).collect();
    if ids.contains(BASE) {
        return Err(format!(
            "An instance is called {BASE}, the name the URDF's base link takes; rename it first."
        ));
    }
    let moving = |id: &str| {
        joints
            .get(id)
            .is_some_and(|j| j["type"].as_str() != Some("fastened"))
    };
    let extra = joints
        .values()
        .filter(|j| j["type"].as_str() == Some("cylindrical"))
        .count();
    if instances.len() + extra + 1 > MAX_LINKS {
        return Err(format!(
            "This assembly makes {} links; Robot Lab reads at most {MAX_LINKS}.",
            instances.len() + extra + 1
        ));
    }
    // Frames at the version's joint values (where the report measured).
    let placed = build::placements(&assembly);
    let mut frames: BTreeMap<&str, Frames> = BTreeMap::new();
    for instance in &instances {
        let id = instance["id"].as_str().unwrap_or_default();
        let own = placed.get(id).copied().unwrap_or(Xf::IDENTITY);
        let link = if moving(id) {
            let joint = joints[id];
            let o = doc::triple(&joint["origin"]).unwrap_or([0.0; 3]);
            instance["parent"]
                .as_str()
                .and_then(|p| placed.get(p).copied())
                .unwrap_or(Xf::IDENTITY)
                .mul(&joint_motion(joint))
                .mul(&Xf::translate(o))
        } else {
            own
        };
        let offset = link.inverse().mul(&own);
        frames.insert(id, Frames { link, offset });
    }
    // The same frames at zero, for the joints' origins.
    let rest = build::placements(&zeroed(&assembly));
    let rest_link = |id: &str| -> Xf {
        let own = rest.get(id).copied().unwrap_or(Xf::IDENTITY);
        own.mul(&frames[id].offset.inverse())
    };
    let report_of: BTreeMap<&str, &Value> = report["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| Some((i["id"].as_str()?, i)))
        .collect();

    // Visuals: each instance's solids by colour, in its link's frame.
    let mut meshes: Vec<(String, Vec<u8>)> = Vec::new();
    let mut visuals: BTreeMap<&str, Vec<(String, [u8; 3])>> = BTreeMap::new();
    if let Some(solids) = solids {
        let mut groups: BTreeMap<(&str, [u8; 3]), Vec<&Solid>> = BTreeMap::new();
        for s in solids {
            if let Some((id, _)) = frames.get_key_value(s.instance.as_str()) {
                groups.entry((id, s.color)).or_default().push(s);
            }
        }
        let by_instance = groups.len() > MAX_MESHES;
        let mut merged: BTreeMap<(&str, [u8; 3]), Vec<&Solid>> = BTreeMap::new();
        if by_instance {
            for ((id, color), list) in groups {
                let key = merged
                    .keys()
                    .find(|(i, _)| *i == id)
                    .copied()
                    .unwrap_or((id, color));
                merged.entry(key).or_default().extend(list);
            }
        } else {
            merged = groups;
        }
        if merged.len() > MAX_MESHES {
            return Err(format!(
                "This assembly needs {} meshes; Robot Lab reads at most {MAX_MESHES}.",
                merged.len()
            ));
        }
        let mut count: BTreeMap<&str, usize> = BTreeMap::new();
        for ((id, color), list) in merged {
            let k = count.entry(id).or_default();
            *k += 1;
            let file = if *k == 1 {
                format!("meshes/{id}.stl")
            } else {
                format!("meshes/{id}-{k}.stl")
            };
            let into = frames[id].link.inverse();
            meshes.push((file.clone(), stl(&list, &into)));
            visuals.entry(id).or_default().push((file, color));
        }
    }

    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\"?>");
    let _ = writeln!(
        out,
        "<!-- Written by OpenAgents CAD Lab from {} (assembly.v1): lengths in metres, angles in radians, mass in kg, inertia in kg m^2 about each link's centre of mass. -->",
        escape(name)
    );
    let _ = writeln!(out, "<robot name=\"{}\">", escape(name));
    let _ = writeln!(out, "  <link name=\"{BASE}\"/>");
    for instance in &instances {
        let id = instance["id"].as_str().unwrap_or_default();
        let f = &frames[id];
        let _ = writeln!(out, "  <link name=\"{id}\">");
        if let Some(entry) = report_of.get(id) {
            let mass = entry["mass_kg"].as_f64().unwrap_or(0.0);
            let com = doc::triple(&entry["center_of_mass_mm"])
                .ok_or_else(|| format!("The report has no centre of mass for {id}."))?;
            let inertia = matrix(&entry["inertia_kg_m2"])
                .ok_or_else(|| format!("The report has no inertia for {id}."))?;
            let c = f.link.inverse().apply(com);
            let i = into_frame(&f.link.r, &inertia);
            let _ = writeln!(out, "    <inertial>");
            let _ = writeln!(out, "      <origin xyz=\"{}\" rpy=\"0 0 0\"/>", v(mm(c)));
            let _ = writeln!(out, "      <mass value=\"{}\"/>", num(mass));
            let _ = writeln!(
                out,
                "      <inertia ixx=\"{}\" ixy=\"{}\" ixz=\"{}\" iyy=\"{}\" iyz=\"{}\" izz=\"{}\"/>",
                num(i[0][0]),
                num(i[0][1]),
                num(i[0][2]),
                num(i[1][1]),
                num(i[1][2]),
                num(i[2][2])
            );
            let _ = writeln!(out, "    </inertial>");
        }
        for (file, color) in visuals.get(id).into_iter().flatten() {
            let rgba = color.map(|c| f64::from(c) / 255.0);
            let _ = writeln!(out, "    <visual>");
            let _ = writeln!(out, "      <origin xyz=\"0 0 0\" rpy=\"0 0 0\"/>");
            let _ = writeln!(
                out,
                "      <geometry><mesh filename=\"{file}\"/></geometry>"
            );
            let _ = writeln!(
                out,
                "      <material name=\"rgb_{}_{}_{}\"><color rgba=\"{} {} {} 1\"/></material>",
                color[0],
                color[1],
                color[2],
                num((rgba[0] * 1000.0).round() / 1000.0),
                num((rgba[1] * 1000.0).round() / 1000.0),
                num((rgba[2] * 1000.0).round() / 1000.0)
            );
            let _ = writeln!(out, "    </visual>");
        }
        let _ = writeln!(out, "  </link>");
    }
    let mut names: BTreeSet<String> = joints
        .values()
        .filter_map(|j| j["id"].as_str().map(str::to_owned))
        .collect();
    let mut fresh = |base: String| {
        let mut name = base.clone();
        let mut k = 2;
        while names.contains(&name) {
            name = format!("{base}_{k}");
            k += 1;
        }
        names.insert(name.clone());
        name
    };
    let mut pose = BTreeMap::new();
    for instance in &instances {
        let id = instance["id"].as_str().unwrap_or_default();
        let parent = instance["parent"]
            .as_str()
            .filter(|p| frames.contains_key(p));
        let (parent_name, parent_rest) = match parent {
            Some(p) => (p, rest_link(p)),
            None => (BASE, Xf::IDENTITY),
        };
        let at = parent_rest.inverse().mul(&rest_link(id));
        let joint = joints.get(id).filter(|_| moving(id));
        let Some(joint) = joint else {
            let name = fresh(format!("{id}_fixed"));
            let _ = writeln!(
                out,
                "  <joint name=\"{name}\" type=\"fixed\">\n    <parent link=\"{parent_name}\"/>\n    <child link=\"{id}\"/>\n    {}\n  </joint>",
                origin(&at)
            );
            continue;
        };
        let jid = joint["id"].as_str().unwrap_or(id);
        let axis = unit(doc::triple(&joint["axis"]).unwrap_or([0.0, 0.0, 1.0]));
        let values = doc::joint_values(joint);
        let limits: Vec<[f64; 2]> = joint["limits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| Some([l[0].as_f64()?, l[1].as_f64()?]))
            .collect();
        let turn = |name: &str, parent: &str, child: &str, at: &Xf, limit: Option<[f64; 2]>| {
            let mut s = String::new();
            let kind = match limit {
                Some([lo, hi]) if lo.abs() <= 360.0 && hi.abs() <= 360.0 && lo < hi => "revolute",
                _ => "continuous",
            };
            let _ = write!(
                s,
                "  <joint name=\"{name}\" type=\"{kind}\">\n    <parent link=\"{parent}\"/>\n    <child link=\"{child}\"/>\n    {}\n    <axis xyz=\"{}\"/>\n",
                origin(at),
                v(axis)
            );
            if kind == "revolute"
                && let Some([lo, hi]) = limit
            {
                let _ = writeln!(
                    s,
                    "    <limit lower=\"{}\" upper=\"{}\" effort=\"{EFFORT}\" velocity=\"{VELOCITY}\"/>",
                    num(lo.to_radians()),
                    num(hi.to_radians())
                );
            }
            s.push_str("  </joint>\n");
            s
        };
        let slide = |name: &str, parent: &str, child: &str, at: &Xf, limit: Option<[f64; 2]>| {
            let [lo, hi] = limit.unwrap_or([-1000.0, 1000.0]);
            format!(
                "  <joint name=\"{name}\" type=\"prismatic\">\n    <parent link=\"{parent}\"/>\n    <child link=\"{child}\"/>\n    {}\n    <axis xyz=\"{}\"/>\n    <limit lower=\"{}\" upper=\"{}\" effort=\"{EFFORT}\" velocity=\"{VELOCITY}\"/>\n  </joint>\n",
                origin(at),
                v(axis),
                num(lo / 1000.0),
                num(hi / 1000.0)
            )
        };
        match joint["type"].as_str().unwrap_or_default() {
            "revolute" => {
                out.push_str(&turn(jid, parent_name, id, &at, limits.first().copied()));
                pose.insert(
                    jid.to_owned(),
                    values.first().copied().unwrap_or(0.0).to_radians(),
                );
            }
            "slider" => {
                out.push_str(&slide(jid, parent_name, id, &at, limits.first().copied()));
                pose.insert(
                    jid.to_owned(),
                    values.first().copied().unwrap_or(0.0) / 1000.0,
                );
            }
            _ => {
                // Cylindrical: values are [angle, distance]; slide, then turn.
                let middle = format!("{id}_slide");
                let slide_name = fresh(format!("{jid}_slide"));
                let _ = writeln!(out, "  <link name=\"{middle}\"/>");
                out.push_str(&slide(
                    &slide_name,
                    parent_name,
                    &middle,
                    &at,
                    limits.get(1).copied(),
                ));
                out.push_str(&turn(
                    jid,
                    &middle,
                    id,
                    &Xf::IDENTITY,
                    limits.first().copied(),
                ));
                pose.insert(
                    jid.to_owned(),
                    values.first().copied().unwrap_or(0.0).to_radians(),
                );
                pose.insert(slide_name, values.get(1).copied().unwrap_or(0.0) / 1000.0);
            }
        }
    }
    let _ = writeln!(out, "</robot>");
    Ok(Urdf {
        text: out,
        meshes,
        pose,
    })
}

/// Binary STL of `solids` (world mm) moved by `into` and in metres.
fn stl(solids: &[&Solid], into: &Xf) -> Vec<u8> {
    let triangles: usize = solids.iter().map(|s| s.mesh.triangles.len()).sum();
    let mut out = Vec::with_capacity(84 + triangles * 50);
    let mut header = [0u8; 80];
    let label = b"OpenAgents CAD Lab, metres";
    header[..label.len()].copy_from_slice(label);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(triangles as u32).to_le_bytes());
    for s in solids {
        let m = &s.mesh;
        let p = |i: u32| {
            let q = &m.positions[i as usize];
            mm(into.apply([q.x, q.y, q.z]))
        };
        for t in &m.triangles {
            let [a, b, c] = [p(t[0]), p(t[1]), p(t[2])];
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = unit([
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ]);
            for x in n.into_iter().chain(a).chain(b).chain(c) {
                out.extend_from_slice(&(x as f32).to_le_bytes());
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}
