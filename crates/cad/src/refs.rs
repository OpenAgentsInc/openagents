//! Stable face and edge references: a feature names the faces and edges it
//! works on by how they were made, not by a point in space, so editing an
//! earlier feature doesn't make the pick miss.
//!
//! A **face** is named after the feature that made it and its role there:
//!
//! - `body:top`, `body:start` (an extrude's end and start caps), and
//!   `body:side:l3` (the side an extrude or revolve swept from the sketch
//!   curve `l3`; a rectangle's sides are `l1` bottom, `l2` right, `l3` top,
//!   `l4` left in drawing order, and `sketch.line` takes `ids` of its own);
//! - `boss:+z`, `boss:-x`, ... (a box's faces by their outward direction);
//! - `round:blend:...` (a fillet's rounded face), `shell:inner:body:top`
//!   (a shell's inside copy of a face), and `hole:side` for a cylinder;
//! - a face split in pieces keeps its name plus `#...`; a reference without
//!   the suffix means every piece.
//!
//! An **edge** is the meeting of two faces: `{"between": ["body:top",
//! "body:side:l1"]}` (either order; `"*"` for any face), or every edge of a
//! face: `{"of": "body:top"}`.
//!
//! References are worked out on every build from the kernel's own
//! persistent names (its feature numbers translated to our feature ids), so
//! they follow a face through later edits the way the kernel does. Points
//! (`[x, y, z]`) still work for both.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcraft_engine::Session;
use solidcraft_engine::doc::ModelBody;
use solidcraft_engine::geom::Vec3;

/// Kernel feature numbers → our feature ids (`F2` → `body`), from the
/// session's timeline.
fn feature_names(s: &mut Session, part: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Ok(inspect) = s.execute("document.inspect", &json!({"measure": false})) {
        for entry in inspect["timeline"].as_array().into_iter().flatten() {
            let (Some(id), Some(name)) = (entry["id"].as_u64(), entry["name"].as_str()) else {
                continue;
            };
            let ours = name.strip_prefix(&format!("{part}.")).unwrap_or(name);
            out.insert(format!("F{id}"), ours.to_owned());
        }
    }
    out
}

/// A kernel name in our words: every `F<n>` token becomes its feature id.
#[must_use]
pub fn translate(name: &str, features: &BTreeMap<String, String>) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let boundary = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if chars[i] == 'F' && boundary {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let ends = j == chars.len() || !(chars[j].is_alphanumeric() || chars[j] == '_');
            if j > i + 1 && ends {
                let token: String = chars[i..j].iter().collect();
                match features.get(&token) {
                    Some(ours) => out.push_str(ours),
                    None => out.push_str(&token),
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Whether a face's name answers to a reference: the same, a piece of it
/// (`name#...`), or `*`.
fn answers(name: &str, reference: &str) -> bool {
    reference == "*"
        || name == reference
        || name
            .strip_prefix(reference)
            .is_some_and(|rest| rest.starts_with('#'))
}

/// One body's faces and edges in our words.
struct Named {
    body: String,
    faces: Vec<String>,
    /// A point well inside each face (the centre of its largest triangle).
    face_points: Vec<Option<Vec3>>,
    /// Each edge's faces (indices into `faces`) and its middle point.
    edges: Vec<(Vec<usize>, Vec3)>,
}

fn named(b: &ModelBody, features: &BTreeMap<String, String>) -> Named {
    let faces: Vec<String> = solidcraft_engine::doc::naming::face_names(b)
        .iter()
        .map(|n| translate(n, features))
        .collect();
    let m = b.mesh();
    let mut best: Vec<(f64, Option<Vec3>)> = vec![(0.0, None); faces.len()];
    for (t, f) in m.triangles.iter().zip(&m.tri_face) {
        let [a, b, c] = t.map(|i| m.positions[i as usize]);
        let area = (b - a).cross(c - a).len() / 2.0;
        if let Some(slot) = best.get_mut(*f as usize)
            && area > slot.0
        {
            *slot = (area, Some((a + b + c) / 3.0));
        }
    }
    let edges = m
        .edge_faces
        .iter()
        .zip(&m.edges)
        .map(|(fs, points)| (fs.iter().map(|f| *f as usize).collect(), halfway(points)))
        .collect();
    Named {
        body: b.name.clone(),
        faces,
        face_points: best.into_iter().map(|(_, p)| p).collect(),
        edges,
    }
}

/// The point halfway along a polyline.
fn halfway(points: &[Vec3]) -> Vec3 {
    let total: f64 = points.windows(2).map(|w| (w[1] - w[0]).len()).sum();
    let mut left = total / 2.0;
    for w in points.windows(2) {
        let len = (w[1] - w[0]).len();
        if len >= left && len > 0.0 {
            return w[0] + (w[1] - w[0]) * (left / len);
        }
        left -= len;
    }
    points.first().copied().unwrap_or(Vec3::ZERO)
}

fn bodies(s: &mut Session, part: &str, body: Option<&str>) -> Vec<Named> {
    let features = feature_names(s, part);
    let state = s.world_state();
    state
        .bodies
        .iter()
        .filter(|b| body.is_none_or(|n| b.name == n) && !b.body.is_mesh())
        .map(|b| named(b, &features))
        .collect()
}

fn listing(all: &[Named], edges: bool) -> String {
    let mut names: Vec<String> = Vec::new();
    for n in all {
        if edges {
            for (fs, _) in &n.edges {
                let pair: Vec<&str> = fs.iter().map(|f| n.faces[*f].as_str()).collect();
                names.push(format!("[{}]", pair.join(", ")));
            }
        } else {
            names.extend(n.faces.iter().cloned());
        }
    }
    names.sort();
    names.dedup();
    let more = names.len().saturating_sub(24);
    names.truncate(24);
    let mut text = names.join(", ");
    if more > 0 {
        text.push_str(&format!(" and {more} more (cad_faces lists them all)"));
    }
    text
}

fn p3(v: Vec3) -> Value {
    let r = |x: f64| (x * 1e4).round() / 1e4;
    json!([r(v.x), r(v.y), r(v.z)])
}

/// Whether `params` names any face or edge by reference.
#[must_use]
pub fn has_references(params: &Value) -> bool {
    ["faces", "edges"].iter().any(|key| {
        params[*key].as_array().is_some_and(|list| {
            list.iter()
                .any(|e| e.is_string() || e.get("between").is_some() || e.get("of").is_some())
        })
    })
}

/// `params` with every face and edge reference turned into points on them
/// (what the kernel picks by), in the session as it stands before the
/// feature runs.
///
/// # Errors
/// A reference that matches nothing, with the names there are.
pub fn resolve(s: &mut Session, part: &str, params: &Value) -> Result<Value, String> {
    if !has_references(params) {
        return Ok(params.clone());
    }
    let body = params["body"].as_str().map(str::to_owned);
    let all = bodies(s, part, body.as_deref());
    let mut out = params.clone();
    if let Some(list) = params["faces"].as_array() {
        let mut picked = Vec::new();
        for entry in list {
            let Some(reference) = entry.as_str() else {
                picked.push(entry.clone());
                continue;
            };
            let mut found = 0;
            for n in &all {
                for (i, name) in n.faces.iter().enumerate() {
                    if answers(name, reference)
                        && let Some(p) = n.face_points[i]
                    {
                        picked.push(json!({"point": p3(p)}));
                        found += 1;
                    }
                }
            }
            if found == 0 {
                return Err(format!(
                    "there's no face {reference}; the faces are: {}",
                    listing(&all, false)
                ));
            }
        }
        out["faces"] = Value::Array(picked);
    }
    if let Some(list) = params["edges"].as_array() {
        let mut picked = Vec::new();
        for entry in list {
            let (a, b) = if let Some(of) = entry["of"].as_str() {
                (of.to_owned(), "*".to_owned())
            } else if let Some(pair) = entry["between"].as_array() {
                let name = |i: usize| {
                    pair.get(i)
                        .and_then(Value::as_str)
                        .unwrap_or("*")
                        .to_owned()
                };
                (name(0), name(1))
            } else {
                picked.push(entry.clone());
                continue;
            };
            let mut found = 0;
            for n in &all {
                for (fs, mid) in &n.edges {
                    let names: Vec<&str> = fs.iter().map(|f| n.faces[*f].as_str()).collect();
                    let hit = match names.as_slice() {
                        [x, y] => {
                            (answers(x, &a) && answers(y, &b)) || (answers(x, &b) && answers(y, &a))
                        }
                        [x] => answers(x, &a) && answers(x, &b),
                        _ => false,
                    };
                    if hit {
                        picked.push(json!({"point": p3(*mid)}));
                        found += 1;
                    }
                }
            }
            if found == 0 {
                return Err(format!(
                    "there's no edge between {a} and {b}; the edges are: {}",
                    listing(&all, true)
                ));
            }
        }
        out["edges"] = Value::Array(picked);
    }
    Ok(out)
}

/// Every solid's faces and edges as the session stands, in our words: what
/// a feature can name.
#[must_use]
pub fn list(s: &mut Session, part: &str) -> Value {
    let all = bodies(s, part, None);
    let state = s.world_state();
    let solids: Vec<Value> = all
        .iter()
        .map(|n| {
            let normals: BTreeMap<usize, Value> = state
                .body(&n.body)
                .and_then(|b| b.body.faces((b.body.size() * 2e-3).max(1e-3)).ok())
                .into_iter()
                .flatten()
                .map(|f| {
                    (
                        f.index,
                        json!({"area_mm2": (f.area * 100.0).round() / 100.0, "centre": p3(f.centroid),
                               "normal": f.plane_normal.map(p3)}),
                    )
                })
                .collect();
            let faces: Vec<Value> = n
                .faces
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let mut face = normals.get(&i).cloned().unwrap_or(json!({}));
                    face["name"] = json!(name);
                    face
                })
                .collect();
            let edges: Vec<Value> = n
                .edges
                .iter()
                .map(|(fs, mid)| {
                    json!({"between": fs.iter().map(|f| n.faces[*f].clone()).collect::<Vec<_>>(), "mid": p3(*mid)})
                })
                .collect();
            json!({"body": n.body.split_once('.').map_or(n.body.as_str(), |(_, b)| b), "faces": faces, "edges": edges})
        })
        .collect();
    json!({"solids": solids})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_feature_numbers_become_our_ids() {
        let map: BTreeMap<String, String> = [("F2", "body"), ("F3", "round")]
            .into_iter()
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        assert_eq!(
            translate("F3:blend:F2:side:l2|F2:side:l3", &map),
            "round:blend:body:side:l2|body:side:l3"
        );
        assert_eq!(translate("F2:top#[F12:side]", &map), "body:top#[F12:side]");
        assert!(answers("body:top#[hole:side]", "body:top"));
        assert!(!answers("body:topper", "body:top"));
    }
}
