//! `first-frame-capture OUTDIR`: writes the first frame of Foreman's
//! AgentGraph view for each size class the page uses, as gzipped
//! `openagents-first-frame` scenes (`foreman-CLASS.ffs.gz`), and a `manifest.json` naming them with
//! the digest of the graph they show.
//!
//! GPUI runs on its headless test platform (`HeadlessAppContext`) with the
//! web build's text system, `CosmicTextSystem` over the same bundled fonts,
//! so layout and glyph rasterization match the browser. Instead of a GPU,
//! the window's renderer here records the frame: [`capture::convert`] turns
//! GPUI's `Scene` into the first-frame format, and [`capture::Recorder`]
//! keeps the glyph pixels as GPUI's atlas receives them.

mod capture;

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// A size class: the graph box's content size on `/agent/foreman` at a
/// viewport, from `agent-map.css` (`.oa-agent-graph.is-live`: the page
/// column's width less its border, and `min(82vh, 860px)` tall, or `72vh`
/// at 640 px and under, less the border). The page paints a snapshot only
/// when its box measures exactly this, so a stale class costs the early
/// frame and nothing else.
pub struct Class {
    pub name: &'static str,
    pub viewport: (u32, u32),
    pub size: (f32, f32),
}

pub const CLASSES: [Class; 2] = [
    Class {
        name: "desktop",
        viewport: (1440, 900),
        size: (990.0, 736.0),
    },
    Class {
        name: "phone",
        viewport: (390, 844),
        size: (356.0, 605.671875),
    },
];

/// The scale the classes are captured at: GPUI's headless window is
/// always 2.
pub const SCALE: f32 = 2.0;

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next().map(PathBuf::from) else {
        bail!("usage: first-frame-capture OUTDIR");
    };
    std::fs::create_dir_all(&out)?;
    let graph = agent_graph::foreman::graph();
    let json = serde_json::to_string(&graph)?;
    let digest = hex(&Sha256::digest(json.as_bytes()));
    let mut classes = vec![];
    for class in &CLASSES {
        // Subpixel glyphs, as the browser's WebGPU renderer draws them.
        let scene = capture::capture(&json, class.size, true)
            .with_context(|| format!("capturing {}", class.name))?;
        let raw = scene.encode()?;
        // The format reads back what it wrote.
        anyhow::ensure!(openagents_first_frame::Scene::decode(&raw)? == scene);
        // gzip, which the page's `DecompressionStream` reads; no file
        // time, so a rebuild writes the same bytes.
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        gzip.write_all(&raw)?;
        let packed = gzip.finish()?;
        let file = format!("foreman-{}.ffs.gz", class.name);
        write(&out.join(&file), &packed)?;
        eprintln!(
            "{}: {}x{} px, {} commands, {} quads, {} paths, {} glyphs, atlas {}x{}; {} bytes, {} gzipped",
            class.name,
            scene.width,
            scene.height,
            scene.commands.len(),
            scene.quads.len(),
            scene.paths.len(),
            scene.sprites.len(),
            scene.atlas.width,
            scene.atlas.height,
            raw.len(),
            packed.len()
        );
        classes.push(serde_json::json!({
            "name": class.name,
            "viewport": [class.viewport.0, class.viewport.1],
            "width": class.size.0,
            "height": class.size.1,
            "scale": SCALE,
            "file": file,
        }));
    }
    let manifest = serde_json::json!({
        "schema": "openagents.first-frame.v1",
        "agent": "foreman",
        "graph_sha256": digest,
        "classes": classes,
    });
    write(
        &out.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?.as_bytes(),
    )?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
