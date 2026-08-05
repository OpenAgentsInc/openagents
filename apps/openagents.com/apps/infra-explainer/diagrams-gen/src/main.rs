//! Build-time mermaid renderer for the /infra explainer document.
//!
//! Reads the `.mmd` sources in `../diagrams/`, renders each one with merman
//! (the Rust mermaid engine Zed uses, MIT OR Apache-2.0) themed with the Aiur
//! palette, flattens every `<text>` run into paths with usvg so the wasm
//! document needs no fonts to rasterize the result, and writes the final SVGs
//! into `../assets/diagrams/`. The generated SVGs are committed; this tool
//! only runs when a diagram source or the theme changes.
//!
//! Run from this directory: `cargo run --release`

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};

// Aiur palette (omega assets/themes/aiur/aiur.json).
const AIUR_PAGE: &str = "#05070d"; // editor.background — the page void
const AIUR_SURFACE: &str = "#0b1220"; // background — panels, clusters
const AIUR_ELEMENT: &str = "#141f36"; // element.background — node fill
const AIUR_BORDER: &str = "#1f2b45"; // border
const AIUR_TEXT: &str = "#eef3ff"; // text
const AIUR_MUTED: &str = "#a9b1d6"; // text.muted — lines, edges
const AIUR_LINK: &str = "#5c96f8"; // link accent

const DIAGRAMS: [&str; 6] = [
    "01-what-went-down",
    "02-decentralized-shape",
    "03-monorepo",
    "04-swap-end-to-end",
    "05-inside-a-provider",
    "06-die-safely",
];

const LILEX_REGULAR: &[u8] = include_bytes!("../fonts/Lilex-Regular.ttf");
const LILEX_BOLD: &[u8] = include_bytes!("../fonts/Lilex-Bold.ttf");

fn aiur_site_config() -> merman::MermaidConfig {
    merman::MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "darkMode": true,
        "fontFamily": "Lilex, monospace",
        // resvg cannot rasterize HTML `<foreignObject>` labels; emit native
        // SVG text labels instead (nodes read the top-level key, edges read
        // `flowchart`).
        "htmlLabels": false,
        "flowchart": { "htmlLabels": false, "padding": 16 },
        "sequence": { "mirrorActors": false },
        "themeVariables": {
            "primaryColor": AIUR_ELEMENT,
            "primaryTextColor": AIUR_TEXT,
            "primaryBorderColor": AIUR_BORDER,
            "lineColor": AIUR_MUTED,
            "secondaryColor": AIUR_SURFACE,
            "secondaryTextColor": AIUR_TEXT,
            "tertiaryColor": AIUR_SURFACE,
            "tertiaryTextColor": AIUR_TEXT,
            "background": AIUR_PAGE,
            "mainBkg": AIUR_ELEMENT,
            "nodeBorder": AIUR_BORDER,
            "nodeTextColor": AIUR_TEXT,
            "clusterBkg": AIUR_SURFACE,
            "clusterBorder": AIUR_BORDER,
            "titleColor": AIUR_TEXT,
            "edgeLabelBackground": AIUR_SURFACE,
            "textColor": AIUR_TEXT,
            "fontFamily": "Lilex, monospace",
            "noteBkgColor": AIUR_ELEMENT,
            "noteBorderColor": AIUR_BORDER,
            "noteTextColor": AIUR_TEXT,
            "actorBkg": AIUR_ELEMENT,
            "actorBorder": AIUR_BORDER,
            "actorTextColor": AIUR_TEXT,
            "actorLineColor": AIUR_MUTED,
            "labelTextColor": AIUR_TEXT,
            "loopTextColor": AIUR_TEXT,
            "signalColor": AIUR_MUTED,
            "signalTextColor": AIUR_TEXT,
            "sequenceNumberColor": AIUR_PAGE,
            "activationBkgColor": AIUR_BORDER,
            "activationBorderColor": AIUR_LINK,
        },
    }))
}

/// merman's sequence-diagram parity CSS is hardcoded to mermaid's light
/// defaults (`#ECECFF` actors, `#fff5ad` notes, `#333` text). The pipeline
/// already strips `!important`, so appending equal-specificity rules at the
/// end of the existing `<style>` block lets the Aiur values win the cascade.
fn inject_aiur_overrides(svg: &str, diagram_id: &str) -> String {
    let id = format!("#m-{diagram_id}");
    let overrides = [
        format!("{id}{{fill:{AIUR_TEXT};}}"),
        format!("{id} .actor{{stroke:{AIUR_BORDER};fill:{AIUR_ELEMENT};}}"),
        format!("{id} text.actor>tspan{{fill:{AIUR_TEXT};stroke:none;}}"),
        format!("{id} .actor-line{{stroke:{AIUR_MUTED};}}"),
        format!("{id} .messageLine0{{stroke:{AIUR_MUTED};}}"),
        format!("{id} .messageLine1{{stroke:{AIUR_MUTED};}}"),
        format!("{id} .messageText{{fill:{AIUR_TEXT};stroke:none;}}"),
        format!("{id} #arrowhead path{{fill:{AIUR_MUTED};stroke:{AIUR_MUTED};}}"),
        format!("{id} #crosshead path{{fill:{AIUR_MUTED};stroke:{AIUR_MUTED};}}"),
        format!("{id} .sequenceNumber{{fill:{AIUR_PAGE};}}"),
        format!("{id} #sequencenumber{{fill:{AIUR_MUTED};}}"),
        format!("{id} .labelBox{{stroke:{AIUR_BORDER};fill:{AIUR_ELEMENT};}}"),
        format!("{id} .labelText,{id} .labelText>tspan{{fill:{AIUR_TEXT};stroke:none;}}"),
        format!("{id} .loopText,{id} .loopText>tspan{{fill:{AIUR_TEXT};}}"),
        format!("{id} .loopLine{{stroke:{AIUR_BORDER};}}"),
        format!("{id} .note{{stroke:{AIUR_BORDER};fill:{AIUR_ELEMENT};}}"),
        format!("{id} .noteText,{id} .noteText>tspan{{fill:{AIUR_TEXT};}}"),
        format!("{id} .activation0,{id} .activation1,{id} .activation2{{fill:{AIUR_BORDER};stroke:{AIUR_LINK};}}"),
        format!("{id} .marker{{fill:{AIUR_MUTED};stroke:{AIUR_MUTED};}}"),
    ]
    .concat();
    match svg.find("</style>") {
        Some(index) => {
            let mut out = String::with_capacity(svg.len() + overrides.len());
            out.push_str(&svg[..index]);
            out.push_str(&overrides);
            out.push_str(&svg[index..]);
            out
        }
        None => svg.to_owned(),
    }
}

fn flatten_text_to_paths(svg: &str) -> Result<String> {
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(LILEX_REGULAR.to_vec());
    fontdb.load_font_data(LILEX_BOLD.to_vec());
    fontdb.set_monospace_family("Lilex");
    fontdb.set_sans_serif_family("Lilex");
    fontdb.set_serif_family("Lilex");

    let options = usvg::Options {
        font_family: "Lilex".to_owned(),
        fontdb: Arc::new(fontdb),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(svg.as_bytes(), &options)
        .context("usvg could not parse the merman SVG")?;
    Ok(tree.to_string(&usvg::WriteOptions::default()))
}

fn assert_rasterizes(svg: &str, name: &str) -> Result<()> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg.as_bytes(), &options)
        .with_context(|| format!("{name}: flattened SVG no longer parses"))?;
    let size = tree.size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width().ceil() as u32, size.height().ceil() as u32)
            .ok_or_else(|| anyhow!("{name}: zero-sized SVG"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    let painted = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
    anyhow::ensure!(
        painted > 100,
        "{name}: rasterized to an (almost) empty pixmap ({painted} painted pixels)"
    );
    // Debug aid: DIAGRAMS_PNG_DIR renders review PNGs of exactly what
    // resvg-based consumers will rasterize. Never committed.
    if let Ok(dir) = std::env::var("DIAGRAMS_PNG_DIR") {
        fs::create_dir_all(&dir)?;
        pixmap
            .save_png(Path::new(&dir).join(format!("{name}.png")))
            .with_context(|| format!("{name}: debug PNG write failed"))?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sources = manifest_dir.join("../diagrams");
    let output = manifest_dir.join("../assets/diagrams");
    fs::create_dir_all(&output)?;

    let config = aiur_site_config();
    let pipeline = merman::render::SvgPipeline::resvg_safe().with_postprocessor(
        merman::render::CssOverridePostprocessor::strip_existing_important(),
    );

    for name in DIAGRAMS {
        let source_path = sources.join(format!("{name}.mmd"));
        let source = fs::read_to_string(&source_path)
            .with_context(|| format!("reading {}", source_path.display()))?;

        let renderer = merman::render::HeadlessRenderer::new()
            .with_site_config(config.clone())
            .with_vendored_text_measurer()
            .with_diagram_id(name);
        let svg = renderer
            .render_svg_with_pipeline_sync(&source, &pipeline)
            .with_context(|| format!("{name}: merman render failed"))?
            .ok_or_else(|| anyhow!("{name}: merman returned no SVG"))?;

        // merman emits mermaid's hardcoded white canvas; the document renders
        // on the Aiur page void, so recolor the canvas before flattening.
        let svg = svg
            .replace("background-color:white", "background-color:#05070d")
            .replace("background-color: white;", "background-color: #05070d;");
        let svg = inject_aiur_overrides(&svg, name);
        if let Ok(dir) = std::env::var("DIAGRAMS_PNG_DIR") {
            fs::create_dir_all(&dir)?;
            fs::write(Path::new(&dir).join(format!("{name}.raw.svg")), &svg)?;
        }

        let flattened = flatten_text_to_paths(&svg)
            .with_context(|| format!("{name}: text flattening failed"))?;
        assert_rasterizes(&flattened, name)?;

        let output_path = output.join(format!("{name}.svg"));
        fs::write(&output_path, &flattened)
            .with_context(|| format!("writing {}", output_path.display()))?;
        println!(
            "{name}: {} bytes -> {}",
            flattened.len(),
            relative(&output_path, &manifest_dir)
        );
    }
    Ok(())
}

fn relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base.parent().unwrap_or(base))
        .unwrap_or(path)
        .display()
        .to_string()
}
