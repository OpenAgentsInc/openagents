//! Renders: a built document from a named view, as a PNG, on the CPU (the
//! kernel's rasteriser: shaded solids with their edges, on a studio floor).

use std::sync::Arc;

use solidcraft_engine::geom::{Aabb3, Vec3};
use solidcraft_engine::render::{Camera, Rgb, Scene, SceneLine, SceneMesh, render_png};
use solidcraft_engine::view::Palette;

use crate::build::Built;

/// Where a render looks from.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    /// One of [`crate::doc::VIEWS`], or a turn of the hero view (`orbit`).
    pub name: String,
    /// Instances to frame (all when empty).
    pub focus: Vec<String>,
    /// Extra turn about the vertical, degrees (for build films).
    pub yaw: f64,
    /// Zoom: 1 frames the subject; more is closer.
    pub zoom: f64,
    /// A box (min, max) to frame instead of the subject's own, so pictures
    /// of different versions keep one camera.
    pub frame: Option<([f64; 3], [f64; 3])>,
}

impl View {
    #[must_use]
    pub fn named(name: &str) -> View {
        View {
            name: name.to_owned(),
            focus: Vec::new(),
            yaw: 0.0,
            zoom: 1.0,
            frame: None,
        }
    }
}

/// (yaw, pitch) in degrees for a named view. The model's front is +X and
/// its left +Y; the eye sits along (−sin yaw, −cos yaw), so yaw −90 looks
/// from the front and 180 from the left.
fn angles(name: &str) -> (f64, f64) {
    match name {
        "rear" => (130.0, 22.0),
        "plan" => (0.0, 89.9),
        "front" => (-90.0, 6.0),
        "side" => (180.0, 4.0),
        "iso" => (-45.0, 35.26),
        "under" => (-125.0, -25.0),
        "detail" => (-115.0, 26.0),
        // hero: front left, from a little above
        _ => (-128.0, 22.0),
    }
}

/// The world box of some solids (all when `focus` is empty).
#[must_use]
pub fn bounds(built: &Built, focus: &[String]) -> Aabb3 {
    let mut b = Aabb3::default();
    for solid in built
        .solids
        .iter()
        .filter(|s| focus.is_empty() || focus.contains(&s.instance))
    {
        for p in &solid.mesh.positions {
            b.add(*p);
        }
    }
    b
}

fn rgb(c: [u8; 3]) -> Rgb {
    Rgb(c[0], c[1], c[2])
}

/// The camera for a view.
#[must_use]
pub fn camera(built: &Built, view: &View) -> Camera {
    let mut b = bounds(built, &view.focus);
    if let Some((lo, hi)) = view.frame {
        b = Aabb3::default();
        b.add(Vec3::new(lo[0], lo[1], lo[2]));
        b.add(Vec3::new(hi[0], hi[1], hi[2]));
    }
    if b.is_empty() {
        b = bounds(built, &[]);
    }
    let mut cam = Camera::default();
    let (yaw, pitch) = angles(&view.name);
    cam.yaw = (yaw + view.yaw).to_radians();
    cam.pitch = pitch.to_radians().clamp(-1.5707, 1.5707);
    cam.fov = 30f64.to_radians();
    if !b.is_empty() {
        cam.fit(&b);
        // The kernel's fit leaves room for any turn; a still can sit closer.
        cam.distance *= 0.74 / view.zoom.max(0.05);
    }
    cam
}

/// The scene: every solid in its colour with its edges, and a floor grid
/// under the model.
#[must_use]
pub fn scene(built: &Built, cam: &Camera, grid: bool) -> Scene {
    let p = Palette::DEFAULT;
    let mut sc = Scene {
        background: Some(p.background),
        ..Default::default()
    };
    let all = bounds(built, &[]);
    sc.radius =
        (all.diagonal() * 0.5).max(10.0) + all.center().dist(cam.target) + cam.distance * 0.05;
    if grid && !all.is_empty() {
        let floor = all.min.z;
        let size = all.diagonal().max(100.0);
        let step = if size > 2000.0 {
            100.0
        } else if size > 400.0 {
            50.0
        } else {
            10.0
        };
        let ext = (size * 1.5 / step).ceil() * step;
        let c = all.center();
        let (cx, cy) = ((c.x / step).round() * step, (c.y / step).round() * step);
        let n = (ext / step) as i64;
        for i in -n..=n {
            let t = i as f64 * step;
            let col = if i % 5 == 0 { p.grid_major } else { p.grid };
            sc.lines.push(SceneLine {
                points: vec![
                    Vec3::new(cx + t, cy - ext, floor),
                    Vec3::new(cx + t, cy + ext, floor),
                ],
                color: col,
                width: 0.6,
                on_top: false,
            });
            sc.lines.push(SceneLine {
                points: vec![
                    Vec3::new(cx - ext, cy + t, floor),
                    Vec3::new(cx + ext, cy + t, floor),
                ],
                color: col,
                width: 0.6,
                on_top: false,
            });
        }
    }
    for solid in &built.solids {
        let mesh = Arc::clone(&solid.mesh);
        for (i, e) in mesh.edges.iter().enumerate() {
            if mesh.shaded_edge_visible(i) {
                sc.lines.push(SceneLine {
                    points: e.clone(),
                    color: p.edge,
                    width: 1.1,
                    on_top: false,
                });
            }
        }
        sc.meshes.push(SceneMesh {
            mesh,
            color: rgb(solid.color),
            face_colors: Vec::new(),
        });
    }
    sc
}

/// A PNG of the built document from `view`, `w`×`h` pixels.
///
/// # Errors
/// Nothing to draw, or the rasteriser refused the size.
pub fn png(built: &Built, view: &View, w: usize, h: usize) -> Result<Vec<u8>, String> {
    if built.solids.is_empty() {
        return Err("There's nothing to draw yet.".into());
    }
    let cam = camera(built, view);
    let sc = scene(built, &cam, view.name != "under");
    render_png(&sc, &cam, w.clamp(16, 4096), h.clamp(16, 4096))
        .ok_or_else(|| "The render failed.".into())
}
