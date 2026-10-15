//! Topology checks for imported triangle meshes in millimeters.
use solidcraft_engine::geom::Mesh;
use std::collections::BTreeMap;

/// Every welded edge must occur twice, in opposite directions. The mesh
/// remains a triangle mesh; this check does not turn it into exact CAD.
pub fn closed(mesh: &Mesh) -> bool {
    if mesh.triangles.is_empty() || mesh.triangles.len() > 200_000 {
        return false;
    }
    let mut ids = BTreeMap::new();
    let mut vertices = Vec::with_capacity(mesh.positions.len());
    for p in &mesh.positions {
        let point = [p.x, p.y, p.z];
        if point
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000_000.)
        {
            return false;
        }
        let key = point.map(|v| (v * 1_000_000.).round() as i64);
        let next = ids.len();
        vertices.push(*ids.entry(key).or_insert(next));
    }
    let mut edges = BTreeMap::<(usize, usize), (usize, i32)>::new();
    for t in &mesh.triangles {
        let Some(v) = t
            .iter()
            .map(|i| vertices.get(*i as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        if v[0] == v[1] || v[1] == v[2] || v[0] == v[2] {
            return false;
        }
        for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += sign;
            if entry.0 > 2 {
                return false;
            }
        }
    }
    edges
        .values()
        .all(|(count, winding)| *count == 2 && *winding == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_closed_stl_stays_a_mesh_and_an_open_one_is_refused() {
        let seed = serde_json::json!({"kind":"part.v1","id":"cube","features":[{"id":"body","op":"solid.box","params":{"length":10,"width":12,"height":14,"center":[0,0,7]}}]});
        let mut exact = crate::evaluate(&seed).unwrap();
        let bytes = crate::export::bytes(&mut exact.built, "stl").unwrap();
        let cache = crate::build::default_cache();
        let file = crate::build::store_file(&cache, &bytes, "stl").unwrap();
        let doc = serde_json::json!({"kind":"part.v1","id":"scan","features":[{"id":"imported","op":"solid.import_stl","params":{"file":file}}]});
        let built = crate::evaluate(&doc).unwrap_or_else(|e| panic!("{e:?}"));
        assert!(!built.built.solids.is_empty());
        assert!(built.built.solids.iter().all(|s| closed(&s.mesh)));
        assert!((built.report["mass"]["volume_cm3"].as_f64().unwrap() - 1.68).abs() < 0.001);
        let mut open = (*built.built.solids[0].mesh).clone();
        open.triangles.pop();
        assert!(!closed(&open));
    }
}
