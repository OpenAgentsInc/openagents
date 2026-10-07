//! `verse-bake`: bakes a scene's light offline and writes the products with
//! a receipt.
//!
//! ```text
//! verse-bake [--fixture | --scene FILE.glb | --everglade PACK.vtp]
//!            [--backend cpu|gpu] [--compare] [--threads N] [--out DIR]
//!            [--vertex-rays N] [--probe-rays N] [--bounces N]
//!            [--sun-rays N] [--seed N] [--quick]
//! ```
//!
//! It writes `DIR/<bake_key>.vbake` and `DIR/<bake_key>.<backend>.json`, the
//! receipt: the commit, the scene digest, the key, the backend, the wall
//! time, and the ray counts. `--compare` also bakes on the other backend and
//! records how far the two lie apart.
//!
//! The CPU backend uses four workers unless `--threads` asks for more.
//! The GPU backend needs the `gpu` feature and a Vulkan adapter.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use glam::{Mat4, Vec3};
use serde_json::json;
use verse_bake::{
    Backend, CpuBackend, GPU_TOLERANCE, Light, Products, Scene, Settings, Stats, bake, fixture,
};
use verse_pbr::pbr::textured::TexturedScene;

const USAGE: &str = "usage: verse-bake [--fixture | --scene FILE.glb | --everglade PACK.vtp] \
[--backend cpu|gpu] [--compare] [--threads N] [--out DIR] [--vertex-rays N] \
[--probe-rays N] [--bounces N] [--sun-rays N] [--seed N] [--quick]";

/// CPU workers unless `--threads` asks for more, so a bake leaves a shared
/// machine room for builds and its window server.
const DEFAULT_THREADS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Cpu,
    Gpu,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

struct Options {
    source: Source,
    backend: Kind,
    compare: bool,
    threads: usize,
    out: PathBuf,
    vertex_rays: Option<u32>,
    probe_rays: Option<u32>,
    bounces: Option<u32>,
    sun_rays: Option<u32>,
    seed: Option<u64>,
    quick: bool,
}

enum Source {
    Fixture,
    Scene(PathBuf),
    Everglade(PathBuf),
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        source: Source::Fixture,
        backend: Kind::Cpu,
        compare: false,
        threads: CpuBackend::available_threads().min(DEFAULT_THREADS),
        out: PathBuf::from("verse-bake-out"),
        vertex_rays: None,
        probe_rays: None,
        bounces: None,
        sun_rays: None,
        seed: None,
        quick: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        let number = |text: String, name: &str| {
            text.parse::<u64>()
                .map_err(|_| format!("{name}: not a number: {text}"))
        };
        match arg.as_str() {
            "--fixture" => options.source = Source::Fixture,
            "--scene" => options.source = Source::Scene(value("--scene")?.into()),
            "--everglade" => options.source = Source::Everglade(value("--everglade")?.into()),
            "--backend" => {
                options.backend = match value("--backend")?.as_str() {
                    "cpu" => Kind::Cpu,
                    "gpu" => Kind::Gpu,
                    other => return Err(format!("--backend: {other} is not cpu or gpu")),
                }
            }
            "--compare" => options.compare = true,
            "--threads" => {
                options.threads = number(value("--threads")?, "--threads")?.max(1) as usize;
            }
            "--out" => options.out = value("--out")?.into(),
            "--vertex-rays" => {
                options.vertex_rays = Some(number(value(&arg)?, &arg)? as u32);
            }
            "--probe-rays" => options.probe_rays = Some(number(value(&arg)?, &arg)? as u32),
            "--bounces" => options.bounces = Some(number(value(&arg)?, &arg)? as u32),
            "--sun-rays" => options.sun_rays = Some(number(value(&arg)?, &arg)? as u32),
            "--seed" => options.seed = Some(number(value(&arg)?, &arg)?),
            "--quick" => options.quick = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

/// The scene to bake, a name for it in the receipt, and its light and
/// default settings.
fn load(source: &Source) -> Result<(TexturedScene, String, Light, Settings), String> {
    match source {
        Source::Fixture => Ok((
            fixture::scene(),
            "fixture".into(),
            fixture::light(),
            fixture::settings(),
        )),
        Source::Scene(path) => {
            let mut scene = TexturedScene::default();
            let mesh = scene.import_gltf(path)?;
            scene.place(mesh, Mat4::IDENTITY);
            let prepared = Scene::new(&scene)?;
            let (min, max) = prepared.bounds();
            let light = fixture::light();
            let settings = Settings::new(min, max + Vec3::Y * 2.0, 2.0, &light);
            Ok((scene, path.display().to_string(), light, settings))
        }
        Source::Everglade(pack) => everglade(pack),
    }
}

#[cfg(feature = "everglade")]
fn everglade(pack: &Path) -> Result<(TexturedScene, String, Light, Settings), String> {
    use verse_zone_everglade::zones::everglade::{Everglade, HALF_EXTENT, MAX_HEIGHT};
    use verse_zone_everglade::zones::everglade_pack::{PACK_SHA256, ZonePack};
    let loaded = ZonePack::load_local(pack)?;
    let world = Everglade::world(&loaded)?;
    let scene = world
        .mesh
        .textured
        .ok_or("Everglade's world has no textured scene")?;
    // The zone's afternoon key and probe box (`Everglade::bake_light`).
    let light = fixture::light();
    let settings = Settings::new(
        Vec3::new(-HALF_EXTENT, 0.0, -HALF_EXTENT),
        Vec3::new(HALF_EXTENT, MAX_HEIGHT + 3.0, HALF_EXTENT),
        8.1,
        &light,
    );
    Ok((
        (*scene).clone(),
        format!("everglade:{PACK_SHA256}"),
        light,
        settings,
    ))
}

#[cfg(not(feature = "everglade"))]
fn everglade(_: &Path) -> Result<(TexturedScene, String, Light, Settings), String> {
    Err(
        "--everglade needs the everglade feature: cargo run -p verse-bake --features everglade"
            .into(),
    )
}

#[cfg(feature = "gpu")]
fn gpu_backend(scene: &Scene) -> Result<Box<dyn Backend>, String> {
    Ok(Box::new(verse_bake::gpu::GpuBackend::new(
        &scene.triangles,
    )?))
}

#[cfg(not(feature = "gpu"))]
fn gpu_backend(_: &Scene) -> Result<Box<dyn Backend>, String> {
    Err("--backend gpu needs the gpu feature: cargo run -p verse-bake --features gpu".into())
}

fn backend(kind: Kind, scene: &Scene, threads: usize) -> Result<Box<dyn Backend>, String> {
    match kind {
        Kind::Cpu => Ok(Box::new(CpuBackend::new(&scene.triangles, threads))),
        Kind::Gpu => gpu_backend(scene),
    }
}

/// The checkout's commit, and whether it has uncommitted changes.
fn commit() -> (String, bool) {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let head = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    (head, dirty)
}

struct Run {
    products: Products,
    stats: Stats,
    backend: String,
    wall_ms: u64,
}

fn run(
    kind: Kind,
    scene: &Scene,
    light: &Light,
    settings: &Settings,
    threads: usize,
) -> Result<Run, String> {
    let start = Instant::now();
    let mut backend = backend(kind, scene, threads)?;
    let (products, stats) = bake(scene, light, settings, backend.as_mut(), threads)?;
    Ok(Run {
        products,
        stats,
        backend: backend.name(),
        wall_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1)).and_then(|options| execute(&options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verse-bake: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(options: &Options) -> Result<(), String> {
    let loading = Instant::now();
    let (textured, source, light, mut settings) = load(&options.source)?;
    let scene = Scene::new(&textured)?;
    let load_ms = loading.elapsed().as_millis();
    if options.quick {
        settings.vertex_rays = 32;
        settings.probe_rays = 64;
        settings.sun_rays = 2;
    }
    settings.vertex_rays = options.vertex_rays.unwrap_or(settings.vertex_rays);
    settings.probe_rays = options.probe_rays.unwrap_or(settings.probe_rays);
    settings.bounces = options.bounces.unwrap_or(settings.bounces);
    settings.sun_rays = options.sun_rays.unwrap_or(settings.sun_rays);
    settings.seed = options.seed.unwrap_or(settings.seed);
    eprintln!(
        "verse-bake: {source}: {} vertices, {} triangles, key {}",
        scene.vertices.len(),
        scene.triangles.len(),
        &verse_bake::hex(&verse_bake::bake_key(&scene, &light, &settings))[..16],
    );
    let main = run(options.backend, &scene, &light, &settings, options.threads)?;
    let agreement = if options.compare {
        let other = match options.backend {
            Kind::Cpu => Kind::Gpu,
            Kind::Gpu => Kind::Cpu,
        };
        let second = run(other, &scene, &light, &settings, options.threads)?;
        let agreement = main.products.compare(&second.products)?;
        let within = [
            agreement.ambient,
            agreement.open,
            agreement.sun,
            agreement.probes,
        ]
        .iter()
        .all(|s| s.within(&GPU_TOLERANCE));
        Some(json!({
            "backend": second.backend,
            "wall_ms": second.wall_ms,
            "products_digest": second.products.digest(),
            "agreement": agreement,
            "tolerance": GPU_TOLERANCE,
            "within_tolerance": within,
        }))
    } else {
        None
    };
    std::fs::create_dir_all(&options.out).map_err(|e| format!("{}: {e}", options.out.display()))?;
    let key = main.products.bake_key.clone();
    let bytes = main.products.encode();
    let products_path = options.out.join(format!("{key}.vbake"));
    std::fs::write(&products_path, &bytes)
        .map_err(|e| format!("{}: {e}", products_path.display()))?;
    let (commit, dirty) = commit();
    let receipt = json!({
        "schema": "openagents.verse-bake.receipt.v1",
        "commit": commit,
        "dirty": dirty,
        "source": source,
        "scene": {
            "digest": main.products.scene,
            "vertices": scene.vertices.len(),
            "triangles": scene.triangles.len(),
        },
        "bake_key": key,
        "products": {
            "file": products_path.file_name().map(|n| n.to_string_lossy().into_owned()),
            "digest": main.products.digest(),
            "bytes": bytes.len(),
        },
        "backend": main.backend,
        "threads": options.threads,
        "load_ms": load_ms,
        "wall_ms": main.wall_ms,
        "stats": main.stats,
        "light": light,
        "settings": settings,
        "compared": agreement,
    });
    let receipt_path = options
        .out
        .join(format!("{key}.{}.json", options.backend.label()));
    let text = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    std::fs::write(&receipt_path, format!("{text}\n"))
        .map_err(|e| format!("{}: {e}", receipt_path.display()))?;
    println!("{text}");
    eprintln!(
        "verse-bake: {} in {} ms, products {}",
        main.backend,
        main.wall_ms,
        products_path.display()
    );
    Ok(())
}
