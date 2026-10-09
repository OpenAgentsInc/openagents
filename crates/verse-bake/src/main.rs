//! `verse-bake`: bakes a scene's light offline and writes the products with
//! a receipt.
//!
//! ```text
//! verse-bake [--fixture | --scene FILE.glb | --everglade PACK.vtp]
//!            [--backend cpu|gpu] [--compare] [--threads N] [--out DIR]
//!            [--vertex-rays N] [--probe-rays N] [--bounces N]
//!            [--sun-rays N] [--seed N] [--quick] [--layers]
//! ```
//!
//! It writes `DIR/<bake_key>.vbake` and `DIR/<bake_key>.<backend>.json`, the
//! receipt: the commit, the scene digest, the key, the backend, the wall
//! time, and the ray counts. `--compare` also bakes on the other backend and
//! records how far the two lie apart.
//!
//! `--layers` bakes the light layers a zone mixes at run time instead
//! (`verse_pbr::pbr::baked_layers`): the sky, each sun direction, and the
//! lamps. Everglade's layers take four suns, at 8:00, 12:00, 15:30, and
//! 17:30 by its town clock. It writes `DIR/<key>.vlay` and
//! `DIR/<key>.layers.<backend>.json`, both with mode 0600, since layers
//! baked from licensed geometry, such as the medieval kit's town, stay
//! outside the repository. For Everglade it also writes the layers into the
//! desktop's zone cache (`$VERSE_HOME/zones-cache`, or
//! `~/.openagents/verse/zones-cache`) and prints the two pin lines of
//! `everglade_pack::kit_bake`, which the artifact queue writes
//! (`artifacts/everglade-kit-bake.json`). `--layers --check` bakes nothing:
//! it fails unless a layer file in `DIR` is the pinned one and was baked
//! for the scene the sources build now.
//!
//! `--phone-layers [--check]` bakes nothing either: it derives the phone
//! tier's layers (`kit_bake::PHONE_SUNS`, #10908) from the pinned layer file
//! in `DIR`, writes `DIR/phone/<sha256>.vlay`, and prints the two
//! `KIT_BAKE_PHONE_` pin lines (`artifacts/everglade-kit-bake-phone.json`).
//!
//! The CPU backend uses four workers unless `--threads` asks for more.
//! The GPU backend needs the `gpu` feature and a Vulkan adapter.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use glam::{Mat4, Vec3};
use serde_json::json;
use sha2::Digest as _;
use verse_bake::{
    Backend, CpuBackend, GPU_TOLERANCE, Light, Products, Scene, Settings, Stats, bake, fixture,
};
use verse_pbr::pbr::textured::TexturedScene;

const USAGE: &str = "usage: verse-bake [--fixture | --scene FILE.glb | --everglade PACK.vtp] \
[--backend cpu|gpu] [--compare] [--threads N] [--out DIR] [--vertex-rays N] \
[--probe-rays N] [--bounces N] [--sun-rays N] [--seed N] [--quick] [--layers [--check | --reuse-only]] \
| --phone-layers [--check] [--out DIR]";

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
    layers: bool,
    check: bool,
    reuse_only: bool,
    phone_layers: bool,
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
        layers: false,
        check: false,
        reuse_only: false,
        phone_layers: false,
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
            "--layers" => options.layers = true,
            "--check" => options.check = true,
            "--reuse-only" => options.reuse_only = true,
            "--phone-layers" => options.phone_layers = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    if options.reuse_only && (!options.layers || options.check) {
        return Err("--reuse-only requires --layers and cannot accompany --check".into());
    }
    Ok(options)
}

/// The sun directions a layered bake of `source` takes: Everglade's
/// morning, noon, afternoon, and dusk, or the fixture's own.
fn layer_suns(source: &Source, settings: &Settings) -> Vec<[f32; 3]> {
    match source {
        Source::Everglade(_) => everglade_suns(),
        _ => settings.suns.clone(),
    }
}

#[cfg(feature = "everglade")]
fn everglade_suns() -> Vec<[f32; 3]> {
    use verse_zone_everglade::zones::everglade::time_of_day::Light as Hour;
    [8.0, 12.0, 15.5, 17.5]
        .map(|h| Hour::at_hours(h).sun.normalize().to_array())
        .to_vec()
}

#[cfg(not(feature = "everglade"))]
fn everglade_suns() -> Vec<[f32; 3]> {
    Vec::new()
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

/// The vertices where two bakes differ most, so a reader can see where
/// the backends disagree.
fn outliers(scene: &Scene, a: &Products, b: &Products) -> serde_json::Value {
    let describe = |i: usize, difference: f32, values: serde_json::Value| {
        let v = &scene.vertices[i];
        json!({
            "vertex": i,
            "difference": difference,
            "pos": v.pos,
            "normal": v.normal,
            "foliage": scene.foliage[i],
            "far": scene.far[i],
            "values": values,
        })
    };
    let worst = |differences: Vec<f32>| {
        let mut order: Vec<usize> = (0..differences.len()).collect();
        order.sort_by(|&x, &y| differences[y].total_cmp(&differences[x]));
        order.truncate(5);
        order.into_iter().map(move |i| (i, differences[i]))
    };
    let ambient = worst(
        a.vertex_ambient
            .iter()
            .zip(&b.vertex_ambient)
            .map(|(x, y)| (0..3).map(|c| (x[c] - y[c]).abs()).fold(0.0, f32::max))
            .collect(),
    )
    .map(|(i, d)| describe(i, d, json!([a.vertex_ambient[i], b.vertex_ambient[i]])))
    .collect::<Vec<_>>();
    let sun = a
        .vertex_sun
        .iter()
        .zip(&b.vertex_sun)
        .enumerate()
        .flat_map(|(k, (x, y))| {
            worst(x.iter().zip(y).map(|(p, q)| (p - q).abs()).collect())
                .map(|(i, d)| describe(i, d, json!({"sun": k, "values": [x[i], y[i]]})))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let faint = scene.triangles.iter().filter(|t| t.opacity < 0.01).count();
    let partial = scene
        .triangles
        .iter()
        .filter(|t| t.opacity < verse_pbr::pbr::bake::SOLID)
        .count();
    json!({
        "ambient": ambient,
        "sun": sun,
        "partial_triangles": partial,
        "clear_triangles": faint,
    })
}

/// Derives the phone tier's layers from the pinned layer file in `dir`.
#[cfg(feature = "everglade")]
fn phone_layers(dir: &Path, check: bool) -> Result<(), String> {
    use verse_zone_everglade::zones::everglade_pack::kit_bake;
    if kit_bake::KIT_BAKE_BYTES == 0 {
        return Err("no kit light layers are pinned".into());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let full = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "vlay"))
        .find_map(|path| {
            let bytes = std::fs::read(&path).ok()?;
            kit_bake::decode_pinned(&bytes).ok()
        })
        .ok_or_else(|| format!("no pinned layer file in {}", dir.display()))?;
    let bytes = kit_bake::phone_layers(&full)?.encode();
    let digest = verse_bake::hex(&sha2::Sha256::digest(&bytes));
    if check {
        if digest != kit_bake::KIT_BAKE_PHONE_SHA256
            || bytes.len() as u64 != kit_bake::KIT_BAKE_PHONE_BYTES
        {
            return Err(format!(
                "the phone layers derive to {digest} ({} bytes), not the pinned {} ({} bytes)",
                bytes.len(),
                kit_bake::KIT_BAKE_PHONE_SHA256,
                kit_bake::KIT_BAKE_PHONE_BYTES
            ));
        }
        eprintln!("verse-bake: the pinned layers derive to the pinned phone layers");
        return Ok(());
    }
    private_write(&dir.join("phone").join(format!("{digest}.vlay")), &bytes)?;
    println!("pub const KIT_BAKE_PHONE_SHA256: &str = \"{digest}\";");
    println!("pub const KIT_BAKE_PHONE_BYTES: u64 = {};", bytes.len());
    Ok(())
}

#[cfg(not(feature = "everglade"))]
fn phone_layers(_: &Path, _: bool) -> Result<(), String> {
    Err("--phone-layers needs the everglade feature".into())
}

fn main() -> ExitCode {
    let parsed = parse(std::env::args().skip(1));
    if let Ok(options) = &parsed {
        if options.phone_layers {
            return match phone_layers(&options.out, options.check) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("verse-bake: {error}");
                    ExitCode::FAILURE
                }
            };
        }
    }
    match parsed.and_then(|options| execute(&options)) {
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
    if options.layers {
        settings.suns = layer_suns(&options.source, &settings);
        return layered(
            options, &textured, &scene, &source, &light, &settings, load_ms,
        );
    }
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
            "outliers": outliers(&scene, &main.products, &second.products),
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

/// Bakes the light layers and writes them with a receipt.
fn layered(
    options: &Options,
    textured: &TexturedScene,
    scene: &Scene,
    source: &str,
    light: &Light,
    settings: &Settings,
    load_ms: u128,
) -> Result<(), String> {
    use verse_pbr::pbr::baked_layers::scene_digest;
    let digest = scene_digest(textured, &textured.merge()?);
    if options.check {
        return check_layers(&options.out, &verse_bake::hex(&digest));
    }
    let key = verse_bake::hex(&verse_bake::layers_key(scene, light, settings));
    if options.reuse_only {
        let digest = verse_bake::hex(&digest);
        let compatibility = layer_compatibility(&options.source);
        let record = compatibility.as_ref().filter(|record| {
            record.targets.iter().any(|target| {
                target.scene == digest && target.bake_key.as_deref() == Some(key.as_str())
            })
        });
        let source_key = record.map_or(key.as_str(), |record| record.baked_key.as_str());
        let source_scene = record.map_or(digest.as_str(), |record| record.baked_scene.as_str());
        let path = options.out.join(format!("{source_key}.vlay"));
        let receipt_path = options.out.join(format!(
            "{source_key}.layers.{}.json",
            options.backend.label()
        ));
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&receipt_path)
                .map_err(|e| format!("{}: {e}; reuse does not bake", receipt_path.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("{}: {e}; reuse does not bake", path.display()))?;
        verify_reuse_receipt(&receipt, source_key, source_scene, &bytes)?;
        let layers = verse_pbr::pbr::baked_layers::Layers::decode(&bytes)?;
        let identity_matches = if let Some(record) = record {
            record.accepts(&layers, &digest, Some(&key))
        } else {
            layers.scene == digest && layers.bake_key == key
        };
        if !identity_matches || layers.vertex_count() != scene.vertices.len() {
            return Err("The completed layers do not match the current scene or recipe".into());
        }
        let file_digest = verse_bake::hex(&sha2::Sha256::digest(&bytes));
        println!("pub const KIT_BAKE_SHA256: &str = \"{file_digest}\";");
        println!("pub const KIT_BAKE_BYTES: u64 = {};", bytes.len());
        eprintln!(
            "verse-bake: reused verified layers at {} without baking",
            path.display()
        );
        return Ok(());
    }
    eprintln!(
        "verse-bake: {source}: {} vertices, {} triangles, {} emissive, {} suns, layers {}",
        scene.vertices.len(),
        scene.triangles.len(),
        scene.emitters.len(),
        settings.suns.len(),
        &key[..16],
    );
    let start = Instant::now();
    let mut backend = backend(options.backend, scene, options.threads)?;
    let (layers, stats) = verse_bake::bake_layers(
        scene,
        digest,
        light,
        settings,
        backend.as_mut(),
        options.threads,
    )?;
    let wall_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let bytes = layers.encode();
    let file_digest = verse_bake::hex(&sha2::Sha256::digest(&bytes));
    std::fs::create_dir_all(&options.out).map_err(|e| format!("{}: {e}", options.out.display()))?;
    let path = options.out.join(format!("{key}.vlay"));
    private_write(&path, &bytes)?;
    let (commit, dirty) = commit();
    let receipt = json!({
        "schema": "openagents.verse-bake.layers-receipt.v1",
        "commit": commit,
        "dirty": dirty,
        "source": source,
        "scene": {
            "digest": layers.scene,
            "vertices": scene.vertices.len(),
            "triangles": scene.triangles.len(),
            "emissive_triangles": scene.emitters.len(),
        },
        "bake_key": key,
        "layers": {
            "file": path.file_name().map(|n| n.to_string_lossy().into_owned()),
            "sha256": file_digest,
            "bytes": bytes.len(),
            "suns": settings.suns,
        },
        "backend": backend.name(),
        "threads": options.threads,
        "load_ms": load_ms,
        "wall_ms": wall_ms,
        "stats": stats,
        "light": light,
        "settings": settings,
    });
    let receipt_path = options
        .out
        .join(format!("{key}.layers.{}.json", options.backend.label()));
    let text = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    private_write(&receipt_path, format!("{text}\n").as_bytes())?;
    println!("{text}");
    eprintln!(
        "verse-bake: layers in {wall_ms} ms, {} bytes, {}",
        bytes.len(),
        path.display()
    );
    if matches!(options.source, Source::Everglade(_)) {
        let cache = std::env::var_os("VERSE_HOME").map_or_else(
            || {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join(".openagents/verse")
            },
            PathBuf::from,
        );
        private_write(
            &cache
                .join("zones-cache")
                .join(format!("{file_digest}.vlay")),
            &bytes,
        )?;
        println!("pub const KIT_BAKE_SHA256: &str = \"{file_digest}\";");
        println!("pub const KIT_BAKE_BYTES: u64 = {};", bytes.len());
    }
    Ok(())
}

/// Supplies only the zone's checked-in, content-bound compatibility record.
fn layer_compatibility(
    source: &Source,
) -> Option<std::sync::Arc<verse_pbr::pbr::baked_layers::SceneCompatibility>> {
    #[cfg(feature = "everglade")]
    if matches!(source, Source::Everglade(_)) {
        return Some(verse_zone_everglade::zones::everglade_pack::kit_bake::compatibility());
    }
    let _ = source;
    None
}

/// Writes `bytes` to `path` readable by its owner only.
fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Fails unless a layer file in `dir` is the pinned one and was baked for
/// the scene whose digest is `digest`.
#[cfg(feature = "everglade")]
fn check_layers(dir: &Path, digest: &str) -> Result<(), String> {
    use verse_zone_everglade::zones::everglade_pack::kit_bake;
    if kit_bake::KIT_BAKE_BYTES == 0 {
        return Err("no kit light layers are pinned".into());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "vlay") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(layers) = kit_bake::decode_pinned(&bytes) else {
            continue;
        };
        if layers.scene != digest && !kit_bake::compatibility().accepts(&layers, digest, None) {
            return Err(format!(
                "the pinned layers were baked for scene {}, and the sources build {digest}",
                layers.scene
            ));
        }
        eprintln!(
            "verse-bake: {} is pinned and fits the scene",
            path.display()
        );
        return Ok(());
    }
    Err(format!("no pinned layer file in {}", dir.display()))
}

#[cfg(not(feature = "everglade"))]
fn check_layers(_: &Path, _: &str) -> Result<(), String> {
    Err("--check needs the everglade feature".into())
}

/// Checks the completed artifact's receipt without trusting its filename.
fn verify_reuse_receipt(
    receipt: &serde_json::Value,
    key: &str,
    scene: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let digest = verse_bake::hex(&sha2::Sha256::digest(bytes));
    if receipt["schema"] != "openagents.verse-bake.layers-receipt.v1"
        || receipt["dirty"] != false
        || receipt["bake_key"] != key
        || receipt["scene"]["digest"] != scene
        || receipt["layers"]["sha256"] != digest
        || receipt["layers"]["bytes"].as_u64() != Some(bytes.len() as u64)
    {
        return Err(
            "The completed layers receipt does not match the current scene, recipe, or bytes"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod reuse_tests {
    use super::*;

    #[test]
    fn reuse_refuses_changed_scene_recipe_dirty_source_and_corrupt_bytes() {
        let bytes = b"completed layer bytes";
        let receipt = json!({"schema":"openagents.verse-bake.layers-receipt.v1", "dirty":false,
            "bake_key":"recipe", "scene":{"digest":"scene"},
            "layers":{"sha256":verse_bake::hex(&sha2::Sha256::digest(bytes)), "bytes":bytes.len()}});
        assert!(verify_reuse_receipt(&receipt, "recipe", "scene", bytes).is_ok());
        assert!(verify_reuse_receipt(&receipt, "other", "scene", bytes).is_err());
        assert!(verify_reuse_receipt(&receipt, "recipe", "other", bytes).is_err());
        assert!(verify_reuse_receipt(&receipt, "recipe", "scene", b"corrupt").is_err());
        let mut dirty = receipt;
        dirty["dirty"] = json!(true);
        assert!(verify_reuse_receipt(&dirty, "recipe", "scene", bytes).is_err());
        assert!(parse(["--reuse-only".to_owned()]).is_err());
        assert!(parse(["--layers", "--reuse-only", "--check"].map(str::to_owned)).is_err());
        assert!(parse(["--layers", "--reuse-only"].map(str::to_owned)).is_ok());
    }
}
