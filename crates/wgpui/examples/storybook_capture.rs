#![allow(clippy::all, clippy::pedantic, unfulfilled_lint_expectations)]
#![allow(dead_code)]
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

use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;

#[path = "shared/capture_support.rs"]
mod capture_support;
#[path = "storybook/constants.rs"]
mod constants;
#[path = "storybook/demos/mod.rs"]
mod demos;
#[path = "storybook/helpers.rs"]
mod helpers;
#[path = "storybook/scene.rs"]
mod scene;
#[path = "storybook/sections/mod.rs"]
mod sections;
#[path = "storybook/state.rs"]
mod state;

use state::Storybook;
use wgpui::{
    Bounds, CaptureArtifact, CaptureRequest, CaptureTarget, Scene, TextSystem, capture_scene,
};

const DEFAULT_STORYBOOK_WIDTH: u32 = 1280;
const DEFAULT_STORYBOOK_HEIGHT: u32 = 900;

fn main() {
    if let Err(error) = run() {
        eprintln!("storybook capture failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = StorybookCaptureArgs::parse();
    if args.list_targets {
        println!("section");
        println!("all-sections");
        return Ok(());
    }

    let story = Storybook::new();
    if args.list_sections {
        for section in story.section_names() {
            println!("{section}");
        }
        return Ok(());
    }

    let time_seconds =
        capture_support::resolve_capture_time_seconds(args.time_seconds, args.frame, args.fps)?;
    let selected_sections = resolve_storybook_sections(args.section.as_deref(), &story)?;
    if selected_sections.len() == 1 {
        run_storybook_single(&args, selected_sections[0], time_seconds)
    } else {
        run_storybook_batch(&args, &selected_sections, time_seconds)
    }
}

fn run_storybook_single(
    args: &StorybookCaptureArgs,
    section: &'static str,
    time_seconds: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let capture_target = CaptureTarget::StorybookSection {
        name: section.to_string(),
    };
    let default_dir = capture_support::default_capture_dir("storybook");
    let output_path = capture_support::resolve_single_output_path(
        args.output.as_ref(),
        args.output_dir.as_ref(),
        &default_dir,
        &capture_target,
    )?;
    let artifact = capture_storybook_section(args, section, time_seconds, output_path)?;
    capture_support::print_capture_artifacts(std::slice::from_ref(&artifact));
    Ok(())
}

fn run_storybook_batch(
    args: &StorybookCaptureArgs,
    sections: &[&'static str],
    time_seconds: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = capture_support::resolve_batch_output_dir(
        args.output.as_ref(),
        args.output_dir.as_ref(),
        &capture_support::default_capture_dir("storybook"),
    )?;
    let mut artifacts = Vec::new();
    for section in sections {
        let capture_target = CaptureTarget::StorybookSection {
            name: (*section).to_string(),
        };
        let output_path = output_dir.join(format!("{}.png", capture_target.slug()));
        artifacts.push(capture_storybook_section(
            args,
            section,
            time_seconds,
            output_path,
        )?);
    }
    let batch_manifest = capture_support::write_batch_manifest(
        &output_dir,
        "manifest.json",
        "storybook_capture",
        &artifacts,
    )?;
    capture_support::print_capture_artifacts(&artifacts);
    println!("batch manifest -> {}", batch_manifest.display());
    Ok(())
}

fn capture_storybook_section(
    args: &StorybookCaptureArgs,
    section: &str,
    time_seconds: f32,
    output_path: PathBuf,
) -> Result<CaptureArtifact, Box<dyn std::error::Error>> {
    let mut story = Storybook::new();
    let Some(section_name) = set_storybook_section(&mut story, section) else {
        return Err(format!("unknown storybook section: {section}").into());
    };
    if time_seconds > 0.0 {
        story.tick_with_delta(Duration::from_secs_f32(time_seconds));
    }

    let width = args.width.unwrap_or(DEFAULT_STORYBOOK_WIDTH);
    let height = args.height.unwrap_or(DEFAULT_STORYBOOK_HEIGHT);
    let mut scene_graph = Scene::new();
    let mut text_system = TextSystem::new(args.scale);
    scene::build_storybook_scene(
        &mut story,
        &mut scene_graph,
        &mut text_system,
        Bounds::new(0.0, 0.0, width as f32, height as f32),
        args.scale,
    );

    let mut request = CaptureRequest::new(
        CaptureTarget::StorybookSection {
            name: section_name.to_string(),
        },
        width,
        height,
        output_path,
    );
    request.scale_factor = args.scale;
    request.allow_fallback_adapter = args.allow_fallback_adapter;
    Ok(capture_scene(&request, &scene_graph, Some(&text_system))?)
}

fn resolve_storybook_sections<'a>(
    requested: Option<&'a str>,
    story: &'a Storybook,
) -> Result<Vec<&'static str>, String> {
    let sections = story.section_names();
    let Some(requested) = requested else {
        return Ok(sections.to_vec());
    };
    if requested.eq_ignore_ascii_case("all") {
        return Ok(sections.to_vec());
    }

    let normalized = normalize_section_name(requested);
    sections
        .iter()
        .find(|section| {
            section.eq_ignore_ascii_case(requested) || normalize_section_name(section) == normalized
        })
        .map(|section| vec![*section])
        .ok_or_else(|| format!("unknown storybook section: {requested}"))
}

fn set_storybook_section<'a>(story: &'a mut Storybook, requested: &str) -> Option<&'static str> {
    let normalized = normalize_section_name(requested);
    let section_names = story.section_names().to_vec();
    let index = section_names.iter().position(|section| {
        section.eq_ignore_ascii_case(requested) || normalize_section_name(section) == normalized
    })?;
    if !story.set_active_section(index) {
        return None;
    }
    section_names.get(index).copied()
}

fn normalize_section_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Parser, Debug)]
#[command(name = "storybook_capture")]
#[command(about = "Capture headless PNGs from WGPUI storybook sections")]
struct StorybookCaptureArgs {
    #[arg(long)]
    list_targets: bool,
    #[arg(long)]
    list_sections: bool,
    #[arg(long)]
    section: Option<String>,
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
