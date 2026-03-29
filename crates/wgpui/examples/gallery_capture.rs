#![allow(clippy::all, clippy::pedantic, unfulfilled_lint_expectations)]
#![expect(
    clippy::expect_used,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]
#![expect(
    clippy::unwrap_used,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]
#![expect(
    clippy::panic,
    reason = "Example/demo lane accepts explicit fail-fast setup paths."
)]

use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::time::Duration;

#[path = "shared/capture_support.rs"]
mod capture_support;
#[path = "shared/component_showcase_scene.rs"]
mod component_showcase_scene;
#[path = "shared/viz_primitives_scene.rs"]
mod viz_primitives_scene;

use wgpui::{CaptureArtifact, CaptureRequest, CaptureTarget, Scene, TextSystem, capture_scene};

fn main() {
    if let Err(error) = run() {
        eprintln!("gallery capture failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = GalleryCaptureArgs::parse();
    if args.list_targets {
        print_gallery_targets();
        return Ok(());
    }

    let time_seconds =
        capture_support::resolve_capture_time_seconds(args.time_seconds, args.frame, args.fps)?;
    let selection = GalleryTargetSelection::from(args.target);
    match selection {
        GalleryTargetSelection::All => run_gallery_batch(&args, time_seconds),
        GalleryTargetSelection::Single(target) => run_gallery_single(&args, target, time_seconds),
    }
}

fn run_gallery_single(
    args: &GalleryCaptureArgs,
    target: GalleryTarget,
    time_seconds: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let capture_target = target.capture_target();
    let default_dir = capture_support::default_capture_dir("gallery");
    let output_path = capture_support::resolve_single_output_path(
        args.output.as_ref(),
        args.output_dir.as_ref(),
        &default_dir,
        &capture_target,
    )?;
    let artifact = capture_gallery_target(args, target, time_seconds, output_path)?;
    capture_support::print_capture_artifacts(std::slice::from_ref(&artifact));
    Ok(())
}

fn run_gallery_batch(
    args: &GalleryCaptureArgs,
    time_seconds: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = capture_support::resolve_batch_output_dir(
        args.output.as_ref(),
        args.output_dir.as_ref(),
        &capture_support::default_capture_dir("gallery"),
    )?;
    let mut artifacts = Vec::new();
    for target in [
        GalleryTarget::VizPrimitives,
        GalleryTarget::ComponentShowcase,
    ] {
        let capture_target = target.capture_target();
        let output_path = output_dir.join(format!("{}.png", capture_target.slug()));
        artifacts.push(capture_gallery_target(
            args,
            target,
            time_seconds,
            output_path,
        )?);
    }
    let batch_manifest = capture_support::write_batch_manifest(
        &output_dir,
        "manifest.json",
        "gallery_capture",
        &artifacts,
    )?;
    capture_support::print_capture_artifacts(&artifacts);
    println!("batch manifest -> {}", batch_manifest.display());
    Ok(())
}

fn capture_gallery_target(
    args: &GalleryCaptureArgs,
    target: GalleryTarget,
    time_seconds: f32,
    output_path: PathBuf,
) -> Result<CaptureArtifact, Box<dyn std::error::Error>> {
    let (width, height) = resolved_gallery_dimensions(target, args.width, args.height);
    let mut scene = Scene::new();
    let mut text_system = TextSystem::new(args.scale);

    let capture_target = target.capture_target();
    match target {
        GalleryTarget::VizPrimitives => {
            viz_primitives_scene::build_viz_primitives_demo(
                &mut scene,
                &mut text_system,
                width as f32,
                height as f32,
                time_seconds,
            );
        }
        GalleryTarget::ComponentShowcase => {
            let mut demo = component_showcase_scene::ComponentShowcaseState::default();
            if time_seconds > 0.0 {
                demo.tick(Duration::from_secs_f32(time_seconds));
            }
            component_showcase_scene::build_component_showcase(
                &mut scene,
                &mut text_system,
                &mut demo,
                width as f32,
                height as f32,
            );
        }
        GalleryTarget::All => unreachable!("batch selection handled before single capture"),
    }

    let mut request = CaptureRequest::new(capture_target, width, height, output_path);
    request.scale_factor = args.scale;
    request.allow_fallback_adapter = args.allow_fallback_adapter;
    Ok(capture_scene(&request, &scene, Some(&text_system))?)
}

fn resolved_gallery_dimensions(
    target: GalleryTarget,
    width: Option<u32>,
    height: Option<u32>,
) -> (u32, u32) {
    let (default_width, default_height) = match target {
        GalleryTarget::VizPrimitives => (
            viz_primitives_scene::DEFAULT_VIZ_PRIMITIVES_WIDTH as u32,
            viz_primitives_scene::DEFAULT_VIZ_PRIMITIVES_HEIGHT as u32,
        ),
        GalleryTarget::ComponentShowcase => (
            component_showcase_scene::DEFAULT_COMPONENT_SHOWCASE_WIDTH as u32,
            component_showcase_scene::DEFAULT_COMPONENT_SHOWCASE_HEIGHT as u32,
        ),
        GalleryTarget::All => unreachable!("batch selection handled before dimension resolve"),
    };
    (
        width.unwrap_or(default_width),
        height.unwrap_or(default_height),
    )
}

fn print_gallery_targets() {
    println!("all");
    println!("viz-primitives");
    println!("component-showcase");
}

#[derive(Parser, Debug)]
#[command(name = "gallery_capture")]
#[command(about = "Capture headless PNGs from WGPUI example gallery scenes")]
struct GalleryCaptureArgs {
    #[arg(long)]
    list_targets: bool,
    #[arg(long, value_enum, default_value_t = GalleryTarget::All)]
    target: GalleryTarget,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
    #[arg(long, default_value_t = 1.0)]
    scale: f32,
    #[arg(long = "time-seconds")]
    time_seconds: Option<f32>,
    #[arg(long)]
    frame: Option<u32>,
    #[arg(long, default_value_t = capture_support::DEFAULT_CAPTURE_FPS)]
    fps: f32,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long)]
    allow_fallback_adapter: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GalleryTarget {
    All,
    VizPrimitives,
    ComponentShowcase,
}

enum GalleryTargetSelection {
    All,
    Single(GalleryTarget),
}

impl From<GalleryTarget> for GalleryTargetSelection {
    fn from(value: GalleryTarget) -> Self {
        match value {
            GalleryTarget::All => Self::All,
            GalleryTarget::VizPrimitives | GalleryTarget::ComponentShowcase => Self::Single(value),
        }
    }
}

impl GalleryTarget {
    fn capture_target(self) -> CaptureTarget {
        match self {
            Self::VizPrimitives => CaptureTarget::VizPrimitives,
            Self::ComponentShowcase => CaptureTarget::ComponentShowcase,
            Self::All => CaptureTarget::AdHoc {
                name: "gallery".to_string(),
            },
        }
    }
}
