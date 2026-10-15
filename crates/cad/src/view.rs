//! The viewer's model of a built document (`cad-view.v1`): every placed
//! solid's triangles in world millimetres (Z up), the B-rep face each
//! triangle belongs to, and each face's and edge's stable name
//! ([`crate::refs`]), so a click in the browser yields `body:top` or
//! `{"between": [..]}` rather than a point.
//!
//! The solids an assembly places are read back from their parts' STEP
//! files, which keep the faces but not their names. Each face of a placed
//! solid is matched to its part's named faces ([`Built::part_frames`]) by
//! its frame in the part's own frame: area, centre, and normal or axis.
//! A face that matches none (rare: the kernel split it differently) has no
//! name, and the viewer says so instead of guessing.
//!
//! ```text
//! {"kind": "cad-view.v1", "unit": "mm", "up": "+z", "size_mm", "center",
//!  "instances": [{"id", "part", "center"}],
//!  "solids": [{"instance", "part", "color": "#rrggbb", "gloss",
//!              "positions", "normals": base64 f32 x,y,z per vertex,
//!              "indices": base64 u32, three per triangle,
//!              "faces": base64 u32, the face of each triangle,
//!              "face_names": [name or null per face],
//!              "edges": [{"faces": [i, j], "points": base64 f32}]}]}
//! ```

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::build::{Built, Solid};
use crate::xf::Xf;

/// The kind a view model carries.
pub const KIND: &str = "cad-view.v1";

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn f32s(values: impl Iterator<Item = f64>) -> String {
    let bytes: Vec<u8> = values.flat_map(|v| (v as f32).to_le_bytes()).collect();
    base64(&bytes)
}

fn u32s(values: impl Iterator<Item = u32>) -> String {
    let bytes: Vec<u8> = values.flat_map(u32::to_le_bytes).collect();
    base64(&bytes)
}

fn gloss(finish: Option<&str>) -> f64 {
    match finish {
        Some("gloss") => 0.9,
        Some("satin") => 0.5,
        Some("matte") => 0.15,
        _ => 0.4,
    }
}

fn v3(value: &Value) -> Option<[f64; 3]> {
    let a = value.as_array().filter(|a| a.len() == 3)?;
    Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// A face's frame, as [`crate::refs::face_frame`] gives it.
struct Frame {
    area: f64,
    centre: [f64; 3],
    normal: Option<[f64; 3]>,
    axis: Option<[f64; 3]>,
}

impl Frame {
    fn of(value: &Value) -> Option<Frame> {
        Some(Frame {
            area: value["area"].as_f64()?,
            centre: v3(&value["centre"])?,
            normal: v3(&value["normal"]),
            axis: v3(&value["axis"]),
        })
    }

    /// How far `other` is from this frame, in units of the tolerance; `None`
    /// when they can't be the same face.
    fn distance(&self, other: &Frame) -> Option<f64> {
        let area = self.area.max(other.area).max(1e-9);
        let area_off = (self.area - other.area).abs() / area;
        if area_off > 0.03 && (self.area - other.area).abs() > 0.05 {
            return None;
        }
        match (self.normal, other.normal, self.axis, other.axis) {
            (Some(a), Some(b), ..) if dot(a, b) < 0.995 => return None,
            (Some(_), None, ..) | (None, Some(_), ..) => return None,
            (None, None, Some(a), Some(b)) if dot(a, b).abs() < 0.995 => return None,
            (None, None, Some(_), None) | (None, None, None, Some(_)) => return None,
            _ => {}
        }
        let tolerance = 0.02 * area.sqrt() + 0.02;
        let off = distance(self.centre, other.centre) / tolerance;
        (off <= 1.0).then_some(off + area_off)
    }
}

/// The names of `solid`'s faces (by the kernel's face index), matched to
/// its part's named faces.
pub(crate) fn face_names(solid: &Solid, xf: &Xf, frames: &Value) -> Vec<Option<String>> {
    let m = &solid.mesh;
    let count = m
        .tri_face
        .iter()
        .map(|f| *f as usize + 1)
        .max()
        .unwrap_or(0);
    // Each face's triangles in the part's own frame.
    let back = xf.inverse();
    let mut per: Vec<Vec<(f64, [f64; 3], [f64; 3])>> = vec![Vec::new(); count];
    for (t, f) in m.triangles.iter().zip(&m.tri_face) {
        let [p, q, r] = t.map(|i| {
            let v = m.positions[i as usize];
            back.apply([v.x, v.y, v.z])
        });
        let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        let w = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
        let c = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        let len = dot(c, c).sqrt();
        if len <= 1e-12 {
            continue;
        }
        let centre = [0, 1, 2].map(|k| (p[k] + q[k] + r[k]) / 3.0);
        per[*f as usize].push((len / 2.0, centre, c.map(|x| x / len)));
    }
    let mut named: Vec<(String, Frame)> = frames["faces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f["name"].as_str()?.to_owned(), Frame::of(f)?)))
        .collect();
    // A face the kernel keeps in pieces (`hole:side#1`, `#2`) can read back
    // from STEP as one face: the pieces together are named without the
    // suffix, which means every piece.
    let mut groups: BTreeMap<String, Vec<&Frame>> = BTreeMap::new();
    for (name, frame) in &named {
        if let Some((base, _)) = name.split_once('#') {
            groups.entry(base.to_owned()).or_default().push(frame);
        }
    }
    let joined: Vec<(String, Frame)> = groups
        .into_iter()
        .filter(|(_, pieces)| pieces.len() > 1)
        .filter_map(|(base, pieces)| {
            let area: f64 = pieces.iter().map(|f| f.area).sum();
            if area <= 0.0 {
                return None;
            }
            let centre =
                [0, 1, 2].map(|k| pieces.iter().map(|f| f.area * f.centre[k]).sum::<f64>() / area);
            let first = pieces[0];
            let same = |a: Option<[f64; 3]>, b: Option<[f64; 3]>| match (a, b) {
                (Some(a), Some(b)) => dot(a, b).abs() > 0.995,
                (None, None) => true,
                _ => false,
            };
            pieces
                .iter()
                .all(|f| same(f.normal, first.normal) && same(f.axis, first.axis))
                .then_some((
                    base,
                    Frame {
                        area,
                        centre,
                        normal: first.normal,
                        axis: first.axis,
                    },
                ))
        })
        .collect();
    named.extend(joined);
    per.iter()
        .map(|tris| {
            let own = Frame::of(&crate::refs::face_frame(tris)?)?;
            named
                .iter()
                .filter_map(|(name, frame)| Some((own.distance(frame)?, name)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, name)| name.clone())
        })
        .collect()
}

/// The viewer's model of `built`.
#[must_use]
pub fn model(built: &Built) -> Value {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    let mut instance_box: BTreeMap<&str, ([f64; 3], [f64; 3], &str)> = BTreeMap::new();
    let mut solids = Vec::new();
    for solid in &built.solids {
        let m = &solid.mesh;
        if m.triangles.is_empty() {
            continue;
        }
        let entry = instance_box.entry(&solid.instance).or_insert((
            [f64::MAX; 3],
            [f64::MIN; 3],
            &solid.part,
        ));
        for p in &m.positions {
            for (k, x) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[k] = lo[k].min(x);
                hi[k] = hi[k].max(x);
                entry.0[k] = entry.0[k].min(x);
                entry.1[k] = entry.1[k].max(x);
            }
        }
        let xf = built
            .placements
            .get(&solid.instance)
            .copied()
            .unwrap_or(Xf::IDENTITY);
        let names = built
            .part_frames
            .get(&solid.part)
            .map(|frames| face_names(solid, &xf, frames))
            .unwrap_or_default();
        let normals: Vec<f64> = if m.normals.len() == m.positions.len() {
            m.normals
                .iter()
                .flat_map(|n| {
                    let l = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt().max(1e-12);
                    [n.x / l, n.y / l, n.z / l]
                })
                .collect()
        } else {
            vec![0.0; m.positions.len() * 3]
        };
        let edges: Vec<Value> = m
            .edges
            .iter()
            .enumerate()
            .filter(|(i, points)| points.len() >= 2 && !m.seams.get(*i).copied().unwrap_or(false))
            .map(|(i, points)| {
                json!({
                    "faces": m.edge_faces.get(i).cloned().unwrap_or_default(),
                    "points": f32s(points.iter().flat_map(|p| [p.x, p.y, p.z])),
                })
            })
            .collect();
        let [r, g, b] = solid.color;
        solids.push(json!({
            "instance": solid.instance,
            "part": solid.part,
            "color": format!("#{r:02x}{g:02x}{b:02x}"),
            "gloss": gloss(solid.finish.as_deref()),
            "positions": f32s(m.positions.iter().flat_map(|p| [p.x, p.y, p.z])),
            "normals": f32s(normals.into_iter()),
            "indices": u32s(m.triangles.iter().flatten().copied()),
            "faces": u32s(m.tri_face.iter().copied()),
            "face_names": names,
            "edges": edges,
        }));
    }
    let centre = |a: [f64; 3], b: [f64; 3]| [0, 1, 2].map(|k| (a[k] + b[k]) / 2.0);
    let instances: Vec<Value> = instance_box
        .iter()
        .map(|(id, (a, b, part))| json!({"id": id, "part": part, "center": centre(*a, *b)}))
        .collect();
    let (size, center) = if solids.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        ([0, 1, 2].map(|k| hi[k] - lo[k]), centre(lo, hi))
    };
    json!({
        "kind": KIND,
        "unit": "mm",
        "up": "+z",
        "size_mm": size,
        "center": center,
        "instances": instances,
        "solids": solids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(text: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let value = |c: u8| B64.iter().position(|b| *b == c).unwrap_or(0) as u32;
        for chunk in text.as_bytes().chunks(4) {
            let n = chunk.iter().fold(0u32, |n, c| (n << 6) | value(*c));
            let pad = chunk.iter().filter(|c| **c == b'=').count();
            let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
            out.extend_from_slice(&bytes[..3 - pad]);
        }
        out
    }

    #[test]
    fn base64_round_trips() {
        for text in [&b""[..], b"a", b"ab", b"abc", b"abcd", b"\xff\x00\x10\x80"] {
            assert_eq!(decode(&base64(text)), text);
        }
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }

    #[test]
    fn a_placed_part_names_its_faces_in_the_view() {
        let document = json!({"kind": "assembly.v1", "id": "pair", "title": "Pair",
            "parts": {"block": {"kind": "part.v1", "id": "block", "material": "Aluminum", "features": [
                {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": [
                    {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [20, 10]}}]}},
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 6}},
                {"id": "hole", "op": "solid.cylinder", "params": {"radius": 3, "height": 10, "base": [0, 0, -2], "operation": "cut"}}]}},
            "instances": [{"id": "a", "part": "block"},
                          {"id": "b", "part": "block", "at": [0, 50, 0], "rotate": [0, 0, 90]}]});
        let cache = std::env::temp_dir().join(format!("cad-view-test-{}", std::process::id()));
        let eval = crate::evaluate_with(&document, &cache).expect("builds");
        let view = model(&eval.built);
        assert_eq!(view["kind"], KIND);
        assert_eq!(view["instances"].as_array().unwrap().len(), 2);
        for solid in view["solids"].as_array().unwrap() {
            let names: Vec<&str> = solid["face_names"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            for wanted in ["body:top", "body:start", "body:side:l1", "body:side:l3"] {
                assert!(names.contains(&wanted), "{wanted} in {names:?}");
            }
            assert!(
                names.iter().any(|n| n.starts_with("hole")),
                "{} {}",
                solid["face_names"],
                eval.built.part_frames["block"]
            );
            // Every face has a name, and no two flat faces share one.
            assert_eq!(
                names.len(),
                solid["face_names"].as_array().unwrap().len(),
                "{names:?}"
            );
            let triangles = decode(solid["indices"].as_str().unwrap()).len() / 12;
            assert_eq!(
                decode(solid["faces"].as_str().unwrap()).len() / 4,
                triangles
            );
            assert!(!solid["edges"].as_array().unwrap().is_empty());
        }
        // The turned copy's top is still its top.
        let b = view["solids"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["instance"] == "b")
            .unwrap();
        let positions: Vec<f32> = decode(b["positions"].as_str().unwrap())
            .chunks(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let max_y = positions.chunks(3).map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!((max_y - 70.0).abs() < 0.01, "{max_y}");
        let _ = std::fs::remove_dir_all(cache);
    }
}
