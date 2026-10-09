//! Measures one Everglade client tier's kit and light files (#10908): their
//! transfer and decoded sizes, how long they take to decode, and how long
//! the town takes to load and light, with the offline layers or, without a
//! layer file, the load-time bake.
//!
//! ```text
//! cargo run --release -p verse --example everglade_tier_measure -- KIT.vtp [LAYERS.vlay]
//! ```
//!
//! Both files are licensed-derived and stay outside the repository. It
//! prints one JSON object; it renders nothing, so frame cost is not here.

use std::path::Path;
use std::time::Instant;

use serde_json::json;
use verse::runtime::WorldRuntime;
use verse::zones::everglade_pack::{self, kit, kit_bake};
use verse_pbr::pbr::baked_layers::Layers;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kit_path = args
        .first()
        .ok_or("usage: everglade_tier_measure KIT.vtp [LAYERS.vlay]")?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in args.iter() {
        let path = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
        if path.starts_with(repository.canonicalize().map_err(|e| e.to_string())?) {
            return Err("licensed files stay outside the repository".into());
        }
    }
    let pack_path = repository
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let started = Instant::now();
    let mut pack = everglade_pack::ZonePack::load_local(&pack_path)?;
    let pack_ms = started.elapsed().as_secs_f64() * 1e3;

    let kit_bytes = std::fs::read(kit_path).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let pieces = everglade_pack::compile::kit::decode(&kit_bytes)?;
    let kit_decode_ms = started.elapsed().as_secs_f64() * 1e3;
    let kit_decoded = pieces.decoded_texture_bytes();
    let largest_edge = pieces
        .textures
        .iter()
        .map(|t| t.width.max(t.height))
        .max()
        .unwrap_or(0);
    kit::install(&mut pack, Some(&pieces));
    drop(pieces);

    let layers = match args.get(1) {
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let started = Instant::now();
            let layers = Layers::decode(&bytes)?;
            let decode_ms = started.elapsed().as_secs_f64() * 1e3;
            let n = layers.vertex_count();
            let probes = layers.sky_probes.len();
            let decoded = n * 4 * (1 + layers.suns.len())
                + layers.lamps.len() * 8
                + probes * 48 * (1 + layers.suns.len());
            let summary = json!({
                "transfer_bytes": bytes.len(),
                "decoded_bytes": decoded,
                "suns": layers.suns.len(),
                "vertices": n,
                "decode_ms": decode_ms,
            });
            kit_bake::offer(layers);
            Some(summary)
        }
        None => None,
    };

    let mut runtime = WorldRuntime::new();
    let started = Instant::now();
    runtime.install_everglade(&pack);
    let install_ms = started.elapsed().as_secs_f64() * 1e3;
    let started = Instant::now();
    runtime.settle_zone_light();
    let light_ms = started.elapsed().as_secs_f64() * 1e3;
    let baked = runtime
        .everglade_zone_mut()
        .is_some_and(|zone| zone.uses_baked_light());
    let report = json!({
        "schema": "openagents.verse.everglade-tier.v1",
        "kit": {
            "transfer_bytes": kit_bytes.len(),
            "decoded_texture_bytes": kit_decoded,
            "largest_edge": largest_edge,
            "decode_ms": kit_decode_ms,
        },
        "layers": layers,
        "everglade_pack_decode_ms": pack_ms,
        "zone_install_ms": install_ms,
        "zone_light_ms": light_ms,
        "offline_light": baked,
        "machine": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}
