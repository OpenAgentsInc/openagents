//! Standalone crypt lab, rendered through the zone's textured path.
//!
//! Usage: crypt_lab OUT_DIR
//!
//! Loads the original models in `assets/verse/generated/chamber` and writes
//! `assets/<name>.png` for each model, `scene.png` from inside the hall, and
//! `scene.mp4`, a six-second walk from the entrance to the circle. The
//! current ritual chamber is not involved.
//!
//! The hall is an open ruin. These frames use the zone's directional light,
//! so a closed ceiling would leave the floor in shadow.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use verse::mesh::Mesh;
use verse::pbr::textured::TexturedScene;
use verse::pbr::{Key, LitVertex, Material, Neon};
use verse::render::View;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

fn main() -> Result<(), String> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(out.join("assets")).map_err(|e| e.to_string())?;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated/chamber");

    for name in MODELS {
        let mut scene = TexturedScene::default();
        let mesh = scene.import_gltf(&dir.join(format!("{name}.glb")))?;
        let (min, max) = bounds(&scene, mesh);
        scene.place(mesh, Mat4::from_translation(Vec3::new(0.0, -min.y, 0.0)));
        let center = (min + max) * 0.5 + Vec3::new(0.0, -min.y, 0.0);
        let reach = (max - min).max_element().max(0.8);
        let eye = center + Vec3::new(reach * 0.95, reach * 0.62, reach * 1.05);
        let pixels = render(&scene, stage(reach * 1.4), eye, center, studio())?;
        write_png(&out.join("assets").join(format!("{name}.png")), &pixels)?;
        eprintln!("asset {name}");
    }

    let mut scene = TexturedScene::default();
    let mut load = |name: &str| scene.import_gltf(&dir.join(format!("{name}.glb")));
    let hall = load("crypt_hall")?;
    let table = load("slab_table")?;
    let green = load("cauldron_green")?;
    let red = load("cauldron_red")?;
    let amber = load("cauldron_amber")?;
    let tall = load("candelabrum_tall")?;
    let short = load("candelabrum_short")?;
    let candles = load("floor_candles")?;
    let rug = load("ritual_rug")?;
    let jar = load("specimen_jar")?;
    let bones = load("specimen_jar_bones")?;
    let bench = load("alchemy_bench")?;
    let scatter = load("bone_scatter")?;
    let web = load("cobweb")?;

    scene.place(hall, Mat4::IDENTITY);
    for (x, z) in [
        (15.6, -24.0),
        (15.6, -8.0),
        (15.6, 8.0),
        (15.6, 24.0),
        (-15.6, -24.0),
        (-15.6, -8.0),
        (-15.6, 8.0),
        (-15.6, 24.0),
    ] {
        scene.place(table, yaw(x, 0.0, z, std::f32::consts::FRAC_PI_2));
    }
    scene.place(green, at(13.2, 0.0, -9.0));
    scene.place(red, at(-13.2, 0.0, -21.0));
    scene.place(amber, at(13.2, 0.0, -21.0));
    scene.place(amber, at(-13.2, 0.0, -9.0));
    scene.place(tall, at(7.0, 0.0, -26.0));
    scene.place(tall, at(-7.0, 0.0, -26.0));
    for (x, z) in [(2.4, 16.0), (-2.4, 16.0), (2.4, -14.0), (-2.4, -14.0)] {
        scene.place(short, at(x, 0.0, z));
    }
    scene.place(candles, at(0.0, 0.0, 16.0));
    scene.place(candles, at(0.0, 0.0, -14.0));
    scene.place(rug, at(0.0, 0.02, 16.0));
    scene.place(rug, at(0.0, 0.02, -14.0));
    // Ledge top is 3.24 m. The jars and two benches stand on it.
    scene.place(jar, at(-18.5, 3.24, 4.0));
    scene.place(bones, at(18.5, 3.24, -6.0));
    scene.place(bench, at(10.0, 0.0, 26.0));
    scene.place(bench, at(-10.0, 0.0, -26.5));
    scene.place(bench, yaw(18.5, 3.24, 12.0, std::f32::consts::FRAC_PI_2));
    scene.place(bench, yaw(-18.5, 3.24, -16.0, -std::f32::consts::FRAC_PI_2));
    scene.place(scatter, at(6.5, 0.0, -18.0));
    scene.place(scatter, at(-5.5, 0.0, 10.0));
    scene.place(scatter, at(4.0, 0.0, 22.0));
    for (x, z, angle) in [
        (18.2, 27.5, 0.6),
        (-18.2, 27.5, -0.6),
        (18.2, -27.5, 2.4),
        (-18.2, -27.5, -2.4),
    ] {
        scene.place(web, yaw(x, 4.3, z, angle));
    }

    let world = Arc::new(scene);
    let ground = exterior_ground();
    let light = crypt_light();
    let hero_eye = Vec3::new(-2.0, 2.0, 14.0);
    let hero_at = Vec3::new(8.0, 1.4, -8.0);
    let pixels = render(
        world.as_ref(),
        ground.clone(),
        hero_eye,
        hero_at,
        light.clone(),
    )?;
    write_png(&out.join("scene.png"), &pixels)?;
    eprintln!("scene");

    encode_walk(&out.join("scene.mp4"), world.as_ref(), &ground, &light)?;
    eprintln!("video {}", out.join("scene.mp4").display());
    Ok(())
}

const MODELS: &[&str] = &[
    "crypt_hall",
    "slab_table",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "ritual_rug",
    "specimen_jar",
    "specimen_jar_bones",
    "alchemy_bench",
    "bone_scatter",
    "cobweb",
];

fn at(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z))
}

fn yaw(x: f32, y: f32, z: f32, angle: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_rotation_y(angle)
}

fn bounds(scene: &TexturedScene, mesh: usize) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for primitive in &scene.meshes[mesh].primitives {
        for vertex in &primitive.vertices {
            let p = Vec3::from(vertex.pos);
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

fn render(
    scene: &TexturedScene,
    ground: Vec<LitVertex>,
    eye: Vec3,
    target: Vec3,
    neon: Neon,
) -> Result<Vec<u8>, String> {
    let world = Mesh {
        lit: ground,
        textured: Some(Arc::new(scene.clone())),
        ..Mesh::default()
    };
    let dynamic = Mesh {
        neon: Some(neon),
        ..Mesh::default()
    };
    let view = View {
        view_proj: Mat4::perspective_rh(0.72, WIDTH as f32 / HEIGHT as f32, 0.15, 250.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    };
    verse::render::capture_rgba(
        WIDTH,
        HEIGHT,
        &world,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )
}

fn encode_walk(
    path: &Path,
    scene: &TexturedScene,
    ground: &[LitVertex],
    light: &Neon,
) -> Result<(), String> {
    let frames = 90u32;
    let mut encoder = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{WIDTH}x{HEIGHT}"),
            "-framerate",
            "15",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let mut pipe = encoder.stdin.take().ok_or("ffmpeg has no stdin")?;
    for i in 0..frames {
        let t = i as f32 / (frames - 1) as f32;
        // Stay under the 3.3 m lintel until the eye is inside the door at z ≈ 30.
        let eye = Vec3::new(0.0, 2.2, 40.0).lerp(Vec3::new(-4.0, 2.4, 8.0), smooth(t));
        let target = Vec3::new(0.0, 1.2, 6.0).lerp(Vec3::new(8.0, 1.5, -16.0), smooth(t));
        let pixels = render(scene, ground.to_vec(), eye, target, light.clone())?;
        use std::io::Write;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        if i % 15 == 0 {
            eprintln!("frame {i}/{frames}");
        }
    }
    drop(pipe);
    let status = encoder.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited {status}"))
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn studio() -> Neon {
    let mut neon = Neon::neutral(0.0);
    neon.key = Some(Key {
        dir: Vec3::new(0.45, 0.75, 0.4).normalize(),
        illuminance: 5_500.0,
        angular_radius: 0.04,
        rim_dir: Vec3::new(-0.5, 0.3, -0.7).normalize(),
        rim_illuminance: 2_200.0,
        rim_angular_radius: 0.12,
        sky: 500.0,
        ground: 80.0,
        ev100: 10.0,
        shadow_center: Vec3::ZERO,
        shadow_half: 8.0,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    neon
}

fn crypt_light() -> Neon {
    let mut neon = Neon::neutral(0.0);
    neon.key = Some(Key {
        dir: Vec3::new(0.35, 0.82, 0.25).normalize(),
        illuminance: 3_200.0,
        angular_radius: 0.03,
        rim_dir: Vec3::new(-0.6, 0.25, -0.55).normalize(),
        rim_illuminance: 900.0,
        rim_angular_radius: 0.14,
        sky: 220.0,
        ground: 30.0,
        ev100: 9.6,
        shadow_center: Vec3::ZERO,
        shadow_half: 46.0,
        shadow_distance: Some(120.0),
        cache_far_shadows: false,
    });
    neon
}

fn stage(half: f32) -> Vec<LitVertex> {
    ground_quad(half, [0.45, 0.46, 0.48])
}

fn exterior_ground() -> Vec<LitVertex> {
    ground_quad(70.0, [0.11, 0.105, 0.09])
}

fn ground_quad(half: f32, color: [f32; 3]) -> Vec<LitVertex> {
    let corner = |x: f32, z: f32| LitVertex {
        pos: [x * half, -0.02, z * half],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        local: [x * half, 0.0, z * half],
        color,
        params: [0.0, 0.95, Material::WhitePaint.code(), 1.0],
    };
    let [a, b, c, d] = [
        corner(-1.0, -1.0),
        corner(-1.0, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
    ];
    vec![a, b, c, a, c, d]
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
