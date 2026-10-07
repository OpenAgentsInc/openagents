//! Water meshes baked from `physics::water` bodies at zone build time: a
//! grid per body at the tier's spacing, each vertex carrying the depth
//! under it at rest (so Low absorbs over depth with no depth buffer), its
//! distance to the shore (for shoreline foam on every tier), and the
//! current there from the body's flow grid.
//!
//! The shore distance is an exact Euclidean distance transform of the
//! grid's dry vertices, computed separably with the lower envelope of
//! parabolas from Felzenszwalb and Huttenlocher, "Distance Transforms of
//! Sampled Functions" (*Theory of Computing*, 2012).

use glam::{DVec2, Vec2, Vec3};
use physics::water::{Outline, WaterBody};
use verse_engine::quality::Tier;

use super::frame::{DRY, Kind, WaterPatch, WaterVertex};

/// The most vertices along one side of a baked body's grid.
pub const MAX_SIDE: usize = 1024;

/// A tier's grid spacing, m: Low 1.5, Medium 0.75, and High 0.35, inside
/// the specification's 1–2, 0.5–1, and 0.25–0.5 m.
#[must_use]
pub fn spacing(tier: Tier) -> f32 {
    match tier {
        Tier::Low => 1.5,
        Tier::Medium => 0.75,
        Tier::High => 0.35,
    }
}

/// The distance from each cell of a `cols` × `rows` grid to the nearest
/// cell that is not `wet`, in cells times `spacing`; zero for dry cells,
/// and infinite when no cell is dry.
#[must_use]
pub fn shore_distance(cols: usize, rows: usize, wet: &[bool], spacing: f32) -> Vec<f32> {
    assert_eq!(wet.len(), cols * rows, "one flag a cell");
    let far = f64::from(u32::MAX);
    let mut d: Vec<f64> = wet.iter().map(|&w| if w { far } else { 0.0 }).collect();
    let mut line = Vec::new();
    // Along rows, then along columns of the result.
    for r in 0..rows {
        line.clear();
        line.extend((0..cols).map(|c| d[r * cols + c]));
        let out = transform(&line);
        for c in 0..cols {
            d[r * cols + c] = out[c];
        }
    }
    for c in 0..cols {
        line.clear();
        line.extend((0..rows).map(|r| d[r * cols + c]));
        let out = transform(&line);
        for r in 0..rows {
            d[r * cols + c] = out[r];
        }
    }
    d.iter()
        .map(|&s| {
            if s >= far {
                f32::INFINITY
            } else {
                (s.sqrt() as f32) * spacing
            }
        })
        .collect()
}

/// One pass of the 1-D squared distance transform: for each `q`, the least
/// `(q − p)² + f(p)`, from the lower envelope of the parabolas rooted at
/// each `p`.
fn transform(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut out = vec![0.0; n];
    if n == 0 {
        return out;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let sq = |i: usize| (i * i) as f64;
    let meet =
        |q: usize, p: usize| ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * (q as f64 - p as f64));
    for q in 1..n {
        // z[0] is −∞, so the envelope never empties.
        let mut s = meet(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = meet(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, slot) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let d = q as f64 - p as f64;
        *slot = d * d + f[p];
    }
    out
}

/// How a body bakes.
#[derive(Clone, Copy, Debug)]
pub struct Bake {
    /// Grid spacing, m ([`spacing`]).
    pub spacing: f32,
    /// How far past the outline the grid reaches, m, so its edge tucks
    /// under the banks.
    pub pad: f32,
    /// The look of every vertex.
    pub kind: Kind,
    /// The body's index in the frame's [`super::frame::Water::bodies`].
    pub body: usize,
    /// For an outline without an edge (the ocean): the area to cover.
    pub extent: Option<(Vec2, Vec2)>,
}

/// A patch for `body` over the ground `bed` (height at x and z, m): a grid
/// at the bake's spacing over the outline's bounds, each vertex at the
/// body's level there, with its depth, shore distance, and current.
/// Vertices outside the outline are dry, so the patch ends at its edge.
///
/// # Errors
/// The outline has no bounds and the bake gives no extent, or the grid
/// would exceed [`MAX_SIDE`] on a side.
pub fn body(
    body: &WaterBody,
    bed: &dyn Fn(f64, f64) -> f64,
    bake: &Bake,
) -> Result<WaterPatch, String> {
    let (lo, hi) = match (body.outline.bounds(), bake.extent) {
        (_, Some((lo, hi))) => (lo.as_dvec2(), hi.as_dvec2()),
        (Some(bounds), None) => bounds,
        (None, None) => return Err("An unbounded body bakes only over an extent".into()),
    };
    let pad = f64::from(bake.pad);
    let (lo, hi) = (lo - DVec2::splat(pad), hi + DVec2::splat(pad));
    let step = f64::from(bake.spacing.max(0.05));
    let cols = ((hi.x - lo.x) / step).ceil() as usize + 1;
    let rows = ((hi.y - lo.y) / step).ceil() as usize + 1;
    if cols > MAX_SIDE || rows > MAX_SIDE {
        return Err(format!(
            "Water body {} would bake a {cols}×{rows} grid, over {MAX_SIDE} a side",
            body.id.0
        ));
    }
    let mut vertices = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            let p = DVec2::new(lo.x + c as f64 * step, lo.y + r as f64 * step);
            let along = match &body.outline {
                Outline::River { course } => course.locate(p).along,
                _ => 0.0,
            };
            let level = body.level.at(along).0;
            let inside = body.outline.contains(p);
            let mut depth = (level - bed(p.x, p.y)) as f32;
            if !inside {
                depth = depth.min(-(DRY + 0.05));
            }
            let mut v = WaterVertex::new(
                Vec3::new(p.x as f32, level as f32, p.y as f32),
                depth,
                bake.kind,
            )
            .in_body(bake.body);
            if let Some(flow) = &body.flow {
                let f = flow.sample(p);
                v.flow = [f.x as f32, f.y as f32];
            }
            vertices.push(v);
        }
    }
    let mut patch = WaterPatch {
        cols: cols as u32,
        rows: rows as u32,
        vertices,
        decimate: false,
        dry: DRY,
    };
    patch.bake_shore();
    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::water::{Course, FlowGrid, Kind as BodyKind, Level, WaterId};

    /// The transform is exact: each cell's distance is the brute-force
    /// distance to the nearest dry cell.
    #[test]
    fn the_shore_distance_is_the_exact_euclidean_distance() {
        let (cols, rows) = (23, 17);
        let wet: Vec<bool> = (0..cols * rows)
            .map(|i| {
                let (c, r) = ((i % cols) as i32, (i / cols) as i32);
                !((c - 4) * (c - 4) + (r - 3) * (r - 3) < 3 || (c == 18 && r > 9))
            })
            .collect();
        let d = shore_distance(cols, rows, &wet, 0.5);
        for i in 0..cols * rows {
            let (c, r) = ((i % cols) as f32, (i / cols) as f32);
            let brute = (0..cols * rows)
                .filter(|&j| !wet[j])
                .map(|j| ((j % cols) as f32 - c).hypot((j / cols) as f32 - r))
                .fold(f32::INFINITY, f32::min);
            assert!(
                (d[i] - brute * 0.5).abs() < 1e-4,
                "{i}: {} vs {}",
                d[i],
                brute * 0.5
            );
        }
        assert!(
            shore_distance(3, 3, &[true; 9], 1.0)
                .iter()
                .all(|d| d.is_infinite())
        );
    }

    /// A pond bakes to its outline: wet inside with depth over the bowl,
    /// dry outside, foam distance growing toward the middle.
    #[test]
    fn a_pond_bakes_depth_shore_and_its_edge() {
        let ring: Vec<DVec2> = (0..24)
            .map(|i| {
                let a = i as f64 / 24.0 * std::f64::consts::TAU;
                DVec2::new(a.cos(), a.sin()) * 6.0
            })
            .collect();
        let pond = WaterBody::pond(WaterId(2), ring, 1.0);
        let bed = |x: f64, z: f64| 1.0 - 2.0 * (1.0 - (x * x + z * z) / 36.0).max(0.0);
        let bake = Bake {
            spacing: spacing(Tier::Medium),
            pad: 1.0,
            kind: Kind::Body(0.0),
            body: 2,
            extent: None,
        };
        let patch = body(&pond, &bed, &bake).unwrap();
        let center = patch
            .vertices
            .iter()
            .min_by(|a, b| {
                Vec2::new(a.pos[0], a.pos[2])
                    .length()
                    .total_cmp(&Vec2::new(b.pos[0], b.pos[2]).length())
            })
            .unwrap();
        assert!((center.depth - 2.0).abs() < 0.05, "{}", center.depth);
        assert!((center.shore - 6.0).abs() < 1.0, "{}", center.shore);
        assert_eq!(center.body, 2.0);
        assert!(patch.vertices.iter().all(|v| v.pos[1] == 1.0));
        let outside = patch.vertices.iter().find(|v| v.pos[0] > 6.5).unwrap();
        assert!(outside.depth < -DRY && outside.shore == 0.0);
        // Low's coarser grid has fewer vertices.
        let low = body(
            &pond,
            &bed,
            &Bake {
                spacing: spacing(Tier::Low),
                ..bake
            },
        )
        .unwrap();
        assert!(low.vertices.len() * 3 < patch.vertices.len());
    }

    /// A river bakes its sloped level along the course and its current.
    #[test]
    fn a_river_bakes_its_level_and_current() {
        let course = Course::new(vec![DVec2::ZERO, DVec2::new(20.0, 0.0)], vec![4.0, 4.0]);
        let flow = FlowGrid::river(&course, &[], 0.5, 0.8);
        let river = WaterBody::new(
            WaterId(1),
            BodyKind::River,
            Outline::River { course },
            Level::Profile {
                points: vec![[0.0, 2.0], [20.0, 1.0]],
            },
        )
        .with_flow(flow);
        let bake = Bake {
            spacing: 0.5,
            pad: 0.5,
            kind: Kind::Stream,
            body: 1,
            extent: None,
        };
        let patch = body(&river, &|_, _| 0.0, &bake).unwrap();
        let at = |x: f32| {
            patch
                .vertices
                .iter()
                .find(|v| (v.pos[0] - x).abs() < 0.26 && v.pos[2].abs() < 0.26)
                .copied()
                .unwrap()
        };
        assert!((at(0.0).pos[1] - 2.0).abs() < 0.05);
        assert!((at(10.0).pos[1] - 1.5).abs() < 0.05);
        assert!((at(10.0).flow[0] - 0.8).abs() < 0.2, "{:?}", at(10.0).flow);
        assert!(at(10.0).shore > 1.5);
    }
}
