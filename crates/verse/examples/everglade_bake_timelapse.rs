//! The town's offline-baked light through a day and through a meteor
//! strike (issue #10907), for acceptance with the licensed kit.
//!
//! Usage: `VERSE_KIT_PACK=KIT.vtp VERSE_KIT_BAKE=LAYERS.vlay
//! everglade_bake_timelapse OUT_DIR`. Keep `OUT_DIR` outside Git: the
//! frames show licensed geometry.
//!
//! From the street before Stoop Lane's first townhouse it pins the town
//! clock from 5:00 to 23:00 a quarter hour at a time, lets the layers
//! combine for each hour ([`zones::everglade::Everglade::settle_light`]),
//! and writes each frame to `OUT_DIR/frames`. `report.json` holds each
//! frame's mean luminance and the step from the frame before, so a jump
//! between baked suns shows as an outlier, and the cost of the frames that
//! delivered newly combined light against the frames that didn't.
//!
//! Then, at 17:30, it calls Meteor Swarm down on the townhouse, waits 40 s for
//! the debris and the relight, and writes `meteor-before.png`,
//! `meteor-after.png`, and, after `R`, `meteor-restored.png`. With
//! `VERSE_PHOTO_DEBUG=2` every frame shows the diffuse ambient alone, so a
//! floating baked shadow shows plainly.

use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::{
    controller::InputState,
    render::Offscreen,
    runtime::WorldRuntime,
    ui::{Atlas, UiBatch},
    zones::{self, everglade_pack},
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const DT: f32 = 1.0 / 60.0;

fn write_png(path: &Path, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(rgba))
        .map_err(|e| e.to_string())
}

/// Mean luminance, 0 to 255, of the middle half of the frame.
fn luminance(rgba: &[u8]) -> f64 {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in h / 4..h * 3 / 4 {
        for x in w / 4..w * 3 / 4 {
            let o = (y * w + x) * 4;
            sum += 0.2126 * f64::from(rgba[o])
                + 0.7152 * f64::from(rgba[o + 1])
                + 0.0722 * f64::from(rgba[o + 2]);
            n += 1.0;
        }
    }
    sum / n
}

fn main() -> Result<(), String> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("Expected OUT_DIR")?);
    // `meteor` as the second argument skips the day.
    let day = std::env::args().nth(2).is_none_or(|a| a != "meteor");
    std::fs::create_dir_all(out.join("frames")).map_err(|e| e.to_string())?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let pack = everglade_pack::ZonePack::load_local(&path)?;
    let mut runtime = WorldRuntime::new();
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(5.0)));
    runtime.install_everglade(&pack);
    if runtime.zone != zones::ZoneId::Everglade {
        return Err("Everglade did not install".into());
    }
    runtime.settle_zone_light();
    let layered = runtime
        .everglade_zone_mut()
        .is_some_and(|z| z.uses_baked_light());
    eprintln!("baked layers in use: {layered}");
    let (_, house) = zones::everglade::layout::city::kit_houses()
        .into_iter()
        .find(|(b, _)| b.name == "townhouse 1")
        .ok_or("no townhouse 1")?;
    let floor = house.floor();
    let street = house.world([-8.0, house.depth / 2.0 + 10.0]);
    let front = house.front().0;
    let toward = (house.center[0] - street[0]).atan2(house.center[1] - street[1]);
    runtime.set_spawn(Vec3::new(street[0], floor, street[1]), toward)?;
    let eye = Vec3::new(street[0], floor + 5.0, street[1]);
    let target = Vec3::new(front[0], floor + 3.0, front[1]);
    runtime.set_shot(Some((eye, target)));
    let atlas = Atlas::new(16.0);
    let ui = UiBatch::default();
    let atmosphere = runtime
        .everglade_zone_mut()
        .ok_or("no Everglade")?
        .atmosphere();
    let mut renderer = Offscreen::new(WIDTH, HEIGHT, &runtime.world.mesh, &atlas, atmosphere)?;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let idle = InputState::default();
    let mut frames = Vec::new();
    let mut last: Option<f64> = None;
    let mut k = 0;
    let mut hour = 5.0_f64;
    while day && hour <= 23.0 + 1e-9 {
        runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(hour)));
        runtime.tick(&idle, DT);
        let settled = std::time::Instant::now();
        runtime.settle_zone_light();
        let settle_ms = settled.elapsed().as_secs_f64() * 1000.0;
        // The light lands on this tick; the frame after it uploads it.
        let ticked = std::time::Instant::now();
        runtime.tick(&idle, DT);
        let tick_ms = ticked.elapsed().as_secs_f64() * 1000.0;
        runtime.set_shot(Some((eye, target)));
        let dynamic = runtime.dynamic_mesh();
        let pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        let (encode_ms, gpu_ms) = renderer.last_timing();
        // Let exposure settle at this hour, as play would over seconds.
        let mut pixels = pixels;
        for _ in 0..12 {
            runtime.tick(&idle, DT);
            runtime.set_shot(Some((eye, target)));
            let dynamic = runtime.dynamic_mesh();
            pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        }
        let (steady_encode, steady_gpu) = renderer.last_timing();
        let lum = luminance(&pixels);
        write_png(&out.join("frames").join(format!("{k:04}.png")), &pixels)?;
        frames.push(serde_json::json!({
            "hour": hour, "luminance": lum,
            "step": last.map(|l| lum - l),
            "settle_ms": settle_ms, "tick_ms": tick_ms,
            "delivery_encode_ms": encode_ms, "delivery_gpu_ms": gpu_ms,
            "steady_encode_ms": steady_encode, "steady_gpu_ms": steady_gpu,
        }));
        eprintln!(
            "{hour:5.2} h  luminance {lum:6.2}  settle {settle_ms:7.1} ms  delivery encode {encode_ms:5.2} ms vs {steady_encode:5.2}"
        );
        last = Some(lum);
        k += 1;
        hour += 0.25;
    }
    // The baked light itself through the day, as the town combines it now
    // (the suns blended) and as it did (the nearest sun alone): the mean
    // change of the light channel's multiplier from one quarter hour to the
    // next, over every 31st vertex. A jump between baked suns shows here
    // whatever the view.
    let mut layer_steps = Vec::new();
    if let Some(layers) = everglade_pack::kit_bake::offered() {
        let sample = |lights: &[[u8; 4]]| -> Vec<glam::Vec3> {
            lights
                .iter()
                .step_by(31)
                .map(|&l| verse::pbr::textured_bake::decode(l).0)
                .collect()
        };
        let mut last: Option<(Vec<glam::Vec3>, Vec<glam::Vec3>)> = None;
        let mut hour = 5.0_f32;
        while hour <= 23.0 + 1e-6 {
            let light = zones::everglade::time_of_day::Light::at_hours(hour);
            let ratio = layers.sun_ratio(light.key_lux, light.sky_lux);
            let blend = sample(&layers.lights_blend(&layers.sun_weights(light.key_dir), ratio));
            let nearest = sample(&layers.lights(layers.nearest_sun(light.key_dir), ratio));
            if let Some((b, n)) = &last {
                let mean = |a: &[glam::Vec3], b: &[glam::Vec3]| {
                    a.iter()
                        .zip(b)
                        .map(|(x, y)| (*x - *y).abs().element_sum() / 3.0)
                        .sum::<f32>()
                        / a.len().max(1) as f32
                };
                layer_steps.push(serde_json::json!({
                    "hour": hour,
                    "blended_step": mean(&blend, b),
                    "nearest_step": mean(&nearest, n),
                }));
                eprintln!(
                    "{hour:5.2} h  layer step blended {:.4}  nearest {:.4}",
                    mean(&blend, b),
                    mean(&nearest, n)
                );
            }
            last = Some((blend, nearest));
            hour += 0.25;
        }
    }
    // A meteor strike on the townhouse at golden hour.
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(17.5)));
    runtime.tick(&idle, DT);
    runtime.settle_zone_light();
    let mut still = |runtime: &mut WorldRuntime, name: &str| -> Result<(), String> {
        runtime.set_shot(Some((eye, target)));
        let mut pixels = Vec::new();
        for _ in 0..12 {
            runtime.tick(&idle, DT);
            runtime.set_shot(Some((eye, target)));
            let dynamic = runtime.dynamic_mesh();
            pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        }
        write_png(&out.join(format!("{name}.png")), &pixels)
    };
    still(&mut runtime, "meteor-before")?;
    // The developer's destruction, as the dev bar turns it on.
    runtime.set_dev_destruction(true)?;
    if let Some(zone) = runtime.everglade_zone_mut() {
        zone.set_free_casting();
    }
    runtime.set_shot(None);
    runtime.tick(&idle, DT);
    runtime.zone_intent(zones::Intent::MeteorSwarm)?;
    let center = Vec3::new(house.center[0], floor, house.center[1]);
    let clip = runtime.view(aspect).view_proj * center.extend(1.0);
    let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
    let cast = runtime.demolition_aim(aspect, x, y) && runtime.demolition_confirm();
    eprintln!("meteor cast: {cast}");
    for _ in 0..(40.0 / DT) as usize {
        runtime.tick(&idle, DT);
    }
    let wreck = runtime.everglade_wreckage();
    runtime.settle_zone_light();
    let relit = runtime.everglade_relit();
    still(&mut runtime, "meteor-after")?;
    runtime.zone_intent(zones::Intent::Rebuild)?;
    runtime.tick(&idle, DT);
    runtime.settle_zone_light();
    still(&mut runtime, "meteor-restored")?;
    let report = serde_json::json!({
        "baked_layers_in_use": layered,
        "adapter": renderer.adapter_info().name,
        "frames": frames,
        "layer_steps": layer_steps,
        "meteor": {
            "cast": cast,
            "wreckage": wreck,
            "relit": relit.map(|r| serde_json::json!({
                "hidden_triangles": r.hidden, "vertices": r.vertices,
                "probes": r.probes, "ms": r.ms,
            })),
        },
    });
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
