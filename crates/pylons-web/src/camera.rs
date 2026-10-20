//! Where a pylon stands on the screen: the grid pass's camera, held still
//! at [`CAMERA_TIME`], projecting points on its floor to CSS pixels, and
//! the spots on the floor the layer stands pylons on.
//!
//! The constants mirror `crates/lander5-web/shaders/grid.glsl` (a test
//! reads them there): the camera sits `CAMERA_HEIGHT` m over the floor,
//! looks down the -z axis tilted so the horizon sits `HORIZON_UV` above
//! the middle, with focal length `FOCAL`, straight down the axis over the
//! major line at `x = 0`; at time 0 it is at `z = 0`.

/// The scene time the page holds the camera at.
pub const CAMERA_TIME: f32 = 0.0;
pub const CAMERA_HEIGHT: f64 = 3.2;
pub const HORIZON_UV: f64 = 0.16;
pub const FOCAL: f64 = 1.75;
/// The camera's `x` at [`CAMERA_TIME`].
pub const CAMERA_X: f64 = 0.0;
/// The Grid's minor line spacing, m: pylons stand where lines cross.
pub const CELL: f64 = 4.0;

/// A pillar's height and width in the world, m.
pub const PILLAR_HEIGHT: f64 = 5.5;
pub const PILLAR_WIDTH: f64 = 0.32;

/// The camera's basis: right, up, and forward.
fn basis() -> ([f64; 3], [f64; 3], [f64; 3]) {
    let (y, z) = (-HORIZON_UV / FOCAL, -1.0);
    let length = (y * y + z * z).sqrt();
    let w = [0.0, y / length, z / length];
    // cross(w, up) with up = +y is (−w.z, 0, w.x); normalized, (1, 0, 0).
    let u = [1.0, 0.0, 0.0];
    // cross(u, w)
    let v = [0.0, -w[2], w[1]];
    (u, v, w)
}

/// A point in the world on the screen: CSS pixels from the top left of a
/// `width` × `height` view, and its depth along the view, m. `None`
/// behind the camera.
#[must_use]
pub fn project(point: [f64; 3], width: f64, height: f64) -> Option<(f64, f64, f64)> {
    let (u, v, w) = basis();
    let d = [point[0] - CAMERA_X, point[1] - CAMERA_HEIGHT, point[2]];
    let dot = |a: [f64; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
    let depth = dot(w);
    if depth <= 0.5 {
        return None;
    }
    let uv = (FOCAL * dot(u) / depth, FOCAL * dot(v) / depth);
    Some((
        width / 2.0 + uv.0 * height / 2.0,
        height / 2.0 - uv.1 * height / 2.0,
        depth,
    ))
}

/// A pylon's place on the screen, CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Where the pillar meets the floor.
    pub x: f64,
    pub y: f64,
    /// The pillar's height and width.
    pub height: f64,
    pub width: f64,
    /// The tag's scale, smaller further away.
    pub scale: f64,
    pub depth: f64,
}

/// The pillar standing at floor point (`x`, `z`), on a `width` ×
/// `height` view.
#[must_use]
pub fn place(x: f64, z: f64, width: f64, height: f64) -> Option<Placed> {
    let (bx, by, depth) = project([x, 0.0, z], width, height)?;
    let (_, ty, _) = project([x, PILLAR_HEIGHT, z], width, height)?;
    let px_per_m = FOCAL / depth * height / 2.0;
    Some(Placed {
        x: bx,
        y: by,
        height: (by - ty).max(8.0),
        width: (PILLAR_WIDTH * px_per_m).clamp(2.0, 9.0),
        scale: (22.0 / depth).clamp(0.62, 1.0),
        depth,
    })
}

/// A box on the screen to keep pylons out of: left, top, right, bottom.
pub type Keep = (f64, f64, f64, f64);

/// The tag over a pillar, as a box: about 160 px wide and 56 px tall at
/// full scale, above the pillar's top.
fn tag(placed: &Placed) -> Keep {
    let half = 80.0 * placed.scale;
    let top = placed.y - placed.height - 10.0 - 56.0 * placed.scale;
    (
        placed.x - half,
        top,
        placed.x + half,
        placed.y - placed.height,
    )
}

fn overlaps(a: Keep, b: Keep) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Whether the pillar at `placed`, with its tag, overlaps one of `keep`.
#[must_use]
pub fn blocked(placed: &Placed, keep: &[Keep]) -> bool {
    let tag = tag(placed);
    keep.iter()
        .any(|k| overlaps((tag.0, tag.1, tag.2, placed.y + 6.0), *k))
}

/// Up to `count` spots on a `width` × `height` view, nearest the middle of
/// the floor first: crossings of the Grid's lines whose pillar and tag fit
/// on the screen, below `top` (the corner name), outside `keep` (the list
/// panel), and clear of the spots taken before them. When the floor runs
/// out of clear spots the rest may overlap.
#[must_use]
pub fn spots(count: usize, width: f64, height: f64, top: f64, keep: &[Keep]) -> Vec<Placed> {
    let mut candidates: Vec<(f64, Placed)> = Vec::new();
    // The floor's middle, ahead of the camera: where the first pylon stands.
    let (middle_x, middle_z) = (CAMERA_X, -26.0);
    for row in 2..48 {
        let z = -CELL * f64::from(row);
        for column in -24..=24 {
            let x = CAMERA_X + CELL * f64::from(column);
            let Some(placed) = place(x, z, width, height) else {
                continue;
            };
            let tag = tag(&placed);
            let fits = placed.x > 24.0
                && placed.x < width - 24.0
                && tag.0 > 8.0
                && tag.2 < width - 8.0
                && tag.1 > top
                && placed.y < height - 10.0
                && keep
                    .iter()
                    .all(|k| !overlaps((tag.0, tag.1, tag.2, placed.y + 6.0), *k));
            if fits {
                let (dx, dz) = (x - middle_x, (z - middle_z) * 0.8);
                candidates.push(((dx * dx + dz * dz).sqrt(), placed));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<Placed> = Vec::new();
    let mut rest = Vec::new();
    for (_, placed) in candidates {
        if out.len() == count {
            break;
        }
        // Tags never cover each other; a pillar may stand behind another's.
        let clear = out.iter().all(|other| !overlaps(tag(&placed), tag(other)));
        if clear {
            out.push(placed);
        } else {
            rest.push(placed);
        }
    }
    for placed in rest {
        if out.len() >= count {
            break;
        }
        out.push(placed);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants are the grid pass's own.
    #[test]
    fn the_camera_matches_the_grid_pass() {
        let grid = include_str!("../../lander5-web/shaders/grid.glsl");
        for line in [
            "const float CELL = 4.0;",
            "const float CAMERA_HEIGHT = 3.2;",
            "const float HORIZON_UV = 0.16;",
            "const float FOCAL = 1.75;",
            "vec3 ro = vec3(0.0, CAMERA_HEIGHT, z);",
            "vec3 ta = ro + vec3(0.0, -HORIZON_UV / FOCAL, -1.0);",
        ] {
            assert!(grid.contains(line), "{line}");
        }
        assert_eq!(CAMERA_TIME, 0.0);
    }

    #[test]
    fn the_floor_ahead_projects_below_the_horizon() {
        let (w, h) = (1600.0, 1000.0);
        // Straight ahead, far away: just under the horizon, in the middle.
        let (x, y, depth) = project([CAMERA_X, 0.0, -10_000.0], w, h).unwrap();
        assert!((x - 800.0).abs() < 0.01);
        assert!(depth > 9_000.0);
        let horizon = h / 2.0 - HORIZON_UV * h / 2.0;
        assert!(y > horizon && y - horizon < 2.0, "{y} {horizon}");
        // Nearer is lower on the screen, and behind is nowhere.
        let near = project([CAMERA_X, 0.0, -10.0], w, h).unwrap();
        assert!(near.1 > y + 100.0);
        assert!(project([CAMERA_X, 0.0, 5.0], w, h).is_none());
        // A pillar stands up from its base.
        let pillar = place(CAMERA_X, -20.0, w, h).unwrap();
        assert!(pillar.height > 100.0 && pillar.height < 400.0, "{pillar:?}");
    }

    #[test]
    fn spots_fit_the_screen_and_keep_clear_of_each_other() {
        // How many clear spots each view holds before tags must overlap.
        for (w, h, clear) in [(1600.0, 1000.0, 12), (1280.0, 720.0, 8), (390.0, 844.0, 4)] {
            let panel = (16.0, h - 200.0, 356.0, h - 16.0);
            let spots = spots(24, w, h, 48.0, &[panel]);
            assert_eq!(spots.len(), 24, "{w}x{h}");
            for spot in &spots {
                assert!(spot.x > 0.0 && spot.x < w);
                assert!(spot.y < h);
                assert!(spot.y - spot.height > 48.0);
            }
            for (i, a) in spots.iter().take(clear).enumerate() {
                for b in spots.iter().take(clear).skip(i + 1) {
                    assert!(!overlaps(tag(a), tag(b)), "{w}x{h}: {a:?} {b:?}");
                }
            }
        }
    }
}
