//! Short films of the Genesis port mechanisms in `crates/physics`, drawn
//! with Verse's renderer on the Grid.
//! Usage: physics_ports_capture SCENE OUT.mp4 [STILLS_DIR]
//!
//! - `rocks` (#11656): irregular convex-hull rocks poured onto a slab,
//!   tumbling and piling.
//! - `rubble` (#11657): a rubble field of mounds and a broken slab, one
//!   closed mesh baked to a static distance grid, with rocks and blocks
//!   poured over it.
//! - `drape` (#11658): a cloth dropped over a CAD bracket (the U bracket's
//!   convex pieces from the CoACD bake), stepped beside the world.
//!
//! The scene is simulated by `physics::World` at 120 Hz; only the camera is
//! staged. Frames go to `ffmpeg` as 1920x1080 at 30 fps; with STILLS_DIR,
//! every second's frame is also written there as a PNG.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glam::{DQuat, DVec3, Vec3};
use physics::{
    Body, BodyKind, Cloth, ClothSettings, Collider, ConvexHull, HullRef, Shape, Uniform, World,
};
use verse::{
    mesh::Vertex,
    runtime::WorldRuntime,
    ui::{Atlas, UiBatch},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: u32 = 30;
const STEPS_PER_FRAME: u32 = 4;

/// What a frame draws besides the world: shaded triangles and edges.
#[derive(Clone, Default)]
struct Draw {
    faces: Vec<Vertex>,
    lines: Vec<Vertex>,
}

impl Draw {
    /// A flat triangle, shaded here: a key light from above and to the
    /// side, a sky fill, and a little light from below, so solids read on
    /// the Grid, which has no sun.
    fn tri(&mut self, p: [Vec3; 3], normal: Vec3, color: [f32; 3], _rough: f32) {
        let key = Vec3::new(0.45, 0.8, 0.35).normalize();
        let n = normal.normalize_or(Vec3::Y);
        let k = 0.18 + 0.9 * n.dot(key).max(0.0) + 0.12 * n.y.max(0.0) + 0.05 * (-n.y).max(0.0);
        let shaded = color.map(|c| c * k);
        for q in p {
            self.faces.push(Vertex {
                pos: q.to_array(),
                color: shaded,
                fog: 1.0,
            });
        }
    }

    fn edge(&mut self, a: Vec3, b: Vec3, color: [f32; 3]) {
        for q in [a, b] {
            self.lines.push(Vertex {
                pos: q.to_array(),
                color,
                fog: 1.0,
            });
        }
    }

    /// Every collider of `world` as flat-shaded faces, colored by `color`.
    fn world(&mut self, world: &World, color: &dyn Fn(usize, &Body) -> [f32; 3]) {
        for c in world.colliders() {
            let body = &world[c.body];
            let rgb = color(c.body.0 as usize, body);
            let (pos, rotation) = c.pose(world);
            let at = |v: DVec3| (pos + rotation * v).as_vec3();
            let turn = |n: DVec3| (rotation * n).as_vec3();
            let hull = match c.shape {
                Shape::Hull { hull } => hull.get(),
                Shape::Cuboid { half } => std::sync::Arc::new(ConvexHull::cuboid(half).unwrap()),
                _ => continue,
            };
            for face in hull.faces() {
                let v = &face.vertices;
                for k in 1..v.len() - 1 {
                    let p = [v[0], v[k], v[k + 1]].map(|i| at(hull.vertices()[i as usize]));
                    self.tri(p, turn(face.normal), rgb, 0.85);
                }
            }
            let edge = rgb.map(|c| (c * 0.35).min(1.0));
            for [a, b] in hull.edges() {
                self.edge(
                    at(hull.vertices()[*a as usize]),
                    at(hull.vertices()[*b as usize]),
                    edge,
                );
            }
        }
    }
}

fn encoder(path: &Path) -> Result<std::process::Child, String> {
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
        ])
        .arg(format!("{WIDTH}x{HEIGHT}"))
        .arg("-r")
        .arg(FPS.to_string())
        .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .args(["-crf", "18", "-movflags", "+faststart"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A scene: its world, how many frames, the camera for a frame, how to
/// color a body, and anything else it draws.
struct Scene {
    world: World,
    frames: usize,
    camera: Box<dyn Fn(usize) -> (Vec3, Vec3)>,
    color: Box<dyn Fn(usize, &Body) -> [f32; 3]>,
    /// Fixed triangles drawn every frame (a distance grid's source mesh).
    ground: Draw,
    /// A cloth stepped after the world, with its color.
    cloth: Option<(Cloth, [f32; 3])>,
}

/// #11658: a 40 x 40 cloth dropped over a CAD bracket, the U bracket's
/// convex pieces from the CoACD bake (the physics fixture), scaled up.
fn drape() -> Scene {
    let mut world = World::new(1.0 / 120.0);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.1, 0.0),
        DVec3::new(2.4, 0.1, 2.4),
    );
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../physics/src/collision/fixtures/u-bracket.coacd.json"
    ))
    .map_err(|e| e.to_string())
    .unwrap();
    let scale = 4.0;
    let bracket = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    for hull in fixture["hulls"].as_array().unwrap() {
        let points: Vec<DVec3> = hull
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let a = p.as_array().unwrap();
                let v = DVec3::new(
                    a[0].as_f64().unwrap(),
                    a[1].as_f64().unwrap(),
                    a[2].as_f64().unwrap(),
                );
                // Stand the bracket on the slab, its posts up.
                DVec3::new(v.x, v.y + 0.1, v.z) * scale
            })
            .collect();
        world.add_collider(Collider::new(
            bracket,
            Shape::Hull {
                hull: HullRef::new(ConvexHull::new(&points).unwrap()),
            },
        ));
    }
    let side = 40;
    let size = 2.0;
    let cloth = Cloth::grid(
        DVec3::new(-size / 2.0 + 0.05, 1.3, -size / 2.0 - 0.1),
        DVec3::X,
        DVec3::Z,
        side,
        side,
        size / f64::from(side - 1),
        ClothSettings {
            friction: 0.6,
            ..ClothSettings::default()
        },
    );
    Scene {
        world,
        frames: FPS as usize * 8,
        camera: Box::new(|k| {
            let t = k as f32 / FPS as f32;
            let a = 0.7 + t * 0.07;
            (
                Vec3::new(3.0 * a.sin(), 1.7 - t * 0.04, 3.0 * a.cos()),
                Vec3::new(0.0, 0.45, 0.0),
            )
        }),
        color: Box::new(|_, body| {
            if body.pos.length() < 0.01 {
                [0.55, 0.58, 0.62]
            } else {
                [0.3, 0.3, 0.3]
            }
        }),
        ground: Draw::default(),
        cloth: Some((cloth, [0.75, 0.18, 0.12])),
    }
}

/// #11657: a rubble field (mounds and slabs) as one closed mesh, baked to
/// a distance grid, with rocks and blocks poured over it.
fn rubble() -> Scene {
    let (n, size) = (48_u32, 4.0_f64);
    let height = |x: f64, z: f64| {
        let mounds = [
            (-0.9, -0.6, 0.55, 0.7),
            (0.8, -0.3, 0.45, 0.6),
            (0.1, 0.9, 0.6, 0.8),
            (-0.4, 0.4, 0.3, 0.35),
            (1.2, 1.1, 0.35, 0.5),
        ];
        let mut h = 0.12 + 0.03 * (x * 7.1).sin() * (z * 5.3).cos();
        for (cx, cz, top, r) in mounds {
            let d2 = ((x - cx) * (x - cx) + (z - cz) * (z - cz)) / (r * r);
            h += top * (-d2 * 1.5).exp();
        }
        // A tilted slab, broken off a wall.
        if (x + 0.2).abs() < 0.5 && (z + 1.3).abs() < 0.25 {
            h = h.max(0.35 + 0.4 * (x + 0.7));
        }
        h
    };
    let step = size / f64::from(n);
    let at = |i: u32, j: u32| -size / 2.0 + f64::from(i) * step + 0.0 * f64::from(j);
    let mut vertices = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (x, z) = (at(i, 0), at(j, 0));
            vertices.push(DVec3::new(x, height(x, z), z));
        }
    }
    let top = |i: u32, j: u32| j * (n + 1) + i;
    let base = vertices.len() as u32;
    for j in 0..=n {
        for i in 0..=n {
            vertices.push(DVec3::new(at(i, 0), -0.3, at(j, 0)));
        }
    }
    let bottom = |i: u32, j: u32| base + j * (n + 1) + i;
    let mut triangles = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let (a, b, c, d) = (top(i, j), top(i + 1, j), top(i + 1, j + 1), top(i, j + 1));
            triangles.push([a, c, b]);
            triangles.push([a, d, c]);
            let (a, b, c, d) = (
                bottom(i, j),
                bottom(i + 1, j),
                bottom(i + 1, j + 1),
                bottom(i, j + 1),
            );
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    // The four sides.
    for k in 0..n {
        for (p, q) in [
            ((k, 0), (k + 1, 0)),
            ((n, k), (n, k + 1)),
            ((k + 1, n), (k, n)),
            ((0, k + 1), (0, k)),
        ] {
            let (t0, t1) = (top(p.0, p.1), top(q.0, q.1));
            let (b0, b1) = (bottom(p.0, p.1), bottom(q.0, q.1));
            triangles.push([t0, t1, b1]);
            triangles.push([t0, b1, b0]);
        }
    }
    let started = std::time::Instant::now();
    let grid = physics::SdfGrid::bake(&vertices, &triangles, 0.04, 0.1).unwrap();
    eprintln!(
        "baked {:?} grid in {:?}, sha256 {}",
        grid.dims(),
        started.elapsed(),
        grid.hash()
    );
    let mut world = World::new(1.0 / 120.0);
    let ground = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(
        ground,
        Shape::Sdf {
            grid: physics::SdfRef::new(grid),
        },
    ));
    let shapes: Vec<HullRef> = (0..10_u64)
        .map(|k| {
            let s = 0.07 + 0.03 * (k % 3) as f64;
            if k % 4 == 3 {
                HullRef::new(ConvexHull::cuboid(DVec3::new(s * 1.6, s * 0.6, s)).unwrap())
            } else {
                HullRef::new(ConvexHull::rock(k + 40, DVec3::new(s * 1.2, s, s * 0.9), 20).unwrap())
            }
        })
        .collect();
    let mut count = 0_usize;
    for layer in 0..6 {
        for cell in 0..25 {
            let pos = DVec3::new(
                f64::from(cell % 5) * 0.62 - 1.24 + 0.1 * f64::from(layer % 2),
                1.6 + 0.35 * f64::from(layer),
                f64::from(cell / 5) * 0.62 - 1.24,
            );
            let turn =
                DQuat::from_euler(glam::EulerRot::XYZ, count as f64, count as f64 * 0.7, 0.3);
            world.add_hull_body(shapes[count % shapes.len()], 2400.0, pos, turn);
            count += 1;
        }
    }
    eprintln!("{count} pieces of rubble");
    let mut draw = Draw::default();
    for t in &triangles {
        let [a, b, c] = t.map(|k| vertices[k as usize].as_vec3());
        let normal = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let shade = 0.36 + 0.06 * (a.y * 3.0).sin();
        draw.tri([a, b, c], normal, [shade, shade * 0.93, shade * 0.85], 0.95);
    }
    Scene {
        world,
        frames: FPS as usize * 8,
        camera: Box::new(|k| {
            let t = k as f32 / FPS as f32;
            let a = 2.4 + t * 0.05;
            (
                Vec3::new(4.6 * a.sin(), 2.6, 4.6 * a.cos()),
                Vec3::new(0.0, 0.3, 0.0),
            )
        }),
        color: Box::new(|i, _| {
            let tones = [
                [0.62, 0.6, 0.56],
                [0.48, 0.42, 0.36],
                [0.7, 0.66, 0.6],
                [0.4, 0.4, 0.42],
            ];
            tones[i % tones.len()]
        }),
        ground: draw,
        cloth: None,
    }
}

fn fixed_box(world: &mut World, center: DVec3, half: DVec3) {
    let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }));
}

/// #11656: rocks of a dozen shapes poured onto a slab.
fn rocks() -> Scene {
    let mut world = World::new(1.0 / 120.0);
    fixed_box(
        &mut world,
        DVec3::new(0.0, -0.1, 0.0),
        DVec3::new(2.6, 0.1, 2.6),
    );
    let shapes: Vec<HullRef> = (0..12_u64)
        .map(|k| {
            let s = 0.1 + 0.05 * (k % 4) as f64;
            let half = DVec3::new(s * 1.3, s * 0.8, s);
            HullRef::new(ConvexHull::rock(k + 11, half, 20 + (k as usize % 3) * 8).unwrap())
        })
        .collect();
    let mut n = 0_usize;
    for layer in 0..10 {
        for ring in 0..12 {
            let angle = f64::from(ring) * std::f64::consts::TAU / 12.0 + f64::from(layer) * 0.4;
            let r = 0.25 + 0.12 * f64::from(layer % 3);
            let pos = DVec3::new(
                r * angle.cos(),
                1.2 + 0.45 * f64::from(layer),
                r * angle.sin(),
            );
            let turn = DQuat::from_euler(
                glam::EulerRot::XYZ,
                n as f64 * 0.9,
                n as f64 * 0.37,
                n as f64 * 1.7,
            );
            let (id, c) = world.add_hull_body(shapes[n % shapes.len()], 2600.0, pos, turn);
            world[id].vel = DVec3::new(-angle.sin(), -1.0, angle.cos()) * 0.5;
            world.collider_mut(c).material = physics::Material {
                friction: 0.8,
                torsional: 0.01,
                restitution: 0.05,
            };
            n += 1;
        }
    }
    eprintln!("{n} rocks");
    Scene {
        world,
        frames: FPS as usize * 8,
        camera: Box::new(|k| {
            let t = k as f32 / FPS as f32;
            let a = 0.6 + t * 0.06;
            let eye = Vec3::new(4.2 * a.sin(), 2.2 - t * 0.05, 4.2 * a.cos());
            (eye, Vec3::new(0.0, 0.45, 0.0))
        }),
        color: Box::new(|i, body| {
            if body.kind != BodyKind::Dynamic {
                return [0.32, 0.31, 0.3];
            }
            let tones = [
                [0.42, 0.36, 0.3],
                [0.5, 0.47, 0.43],
                [0.33, 0.3, 0.28],
                [0.55, 0.45, 0.35],
                [0.38, 0.38, 0.4],
            ];
            tones[i % tones.len()]
        }),
        ground: Draw::default(),
        cloth: None,
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let name = args.next().ok_or("Expected a scene: rocks")?;
    let out = PathBuf::from(args.next().ok_or("Expected an output MP4 path")?);
    let stills = args.next().map(PathBuf::from);
    if let Some(dir) = &stills {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut scene = match name.as_str() {
        "rocks" => rocks(),
        "rubble" => rubble(),
        "drape" => drape(),
        other => return Err(format!("Unknown scene {other}")),
    };
    let mut runtime = WorldRuntime::bare();
    let atlas = Atlas::new(30.0);
    let (eye, target) = (scene.camera)(0);
    runtime.set_shot(Some((eye, target)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut film = encoder(&out)?;
    let g = Uniform(DVec3::new(0.0, -9.81, 0.0));
    let mut slowest = std::time::Duration::ZERO;
    for k in 0..scene.frames {
        if k > 0 {
            for _ in 0..STEPS_PER_FRAME {
                scene.world.step(&g);
                if let Some((cloth, _)) = &mut scene.cloth {
                    cloth.step(&mut scene.world, &g);
                }
                slowest = slowest.max(scene.world.stats.total);
            }
        }
        let (eye, target) = (scene.camera)(k);
        runtime.set_shot(Some((eye, target)));
        let view = runtime.view(aspect);
        let mut draw = scene.ground.clone();
        draw.world(&scene.world, &scene.color);
        if let Some((cloth, color)) = &scene.cloth {
            let points = verse_pbr::cloth::positions(cloth, 1.0, Vec3::ZERO);
            for (corners, normals) in verse_pbr::cloth::two_sided(cloth, &points) {
                draw.tri(corners, normals[0] + normals[1] + normals[2], *color, 0.9);
            }
        }
        let mut dynamic = verse::mesh::Mesh::default();
        dynamic.faces = draw.faces;
        dynamic.lines = draw.lines;
        let pixels = renderer.render(view, &dynamic, &UiBatch::default())?;
        film.stdin
            .as_mut()
            .ok_or("ffmpeg has no input")?
            .write_all(&pixels)
            .map_err(|e| e.to_string())?;
        if let Some(dir) = &stills
            && k % FPS as usize == 0
        {
            write_png(
                &dir.join(format!("{name}-{:02}.png", k / FPS as usize)),
                &pixels,
            )?;
        }
    }
    drop(film.stdin.take());
    let status = film.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed".into());
    }
    eprintln!(
        "wrote {}: {} frames, slowest step {slowest:?}, {} asleep",
        out.display(),
        scene.frames,
        scene.world.stats.asleep
    );
    Ok(())
}
