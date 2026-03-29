use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use wgpui::{CaptureArtifact, CaptureTarget};

pub const DEFAULT_CAPTURE_SECONDS: f32 = 1.0;
pub const DEFAULT_CAPTURE_FPS: f32 = 60.0;

pub fn default_capture_dir(name: &str) -> PathBuf {
    PathBuf::from("target/wgpui-captures").join(name)
}

pub fn resolve_capture_time_seconds(
    time_seconds: Option<f32>,
    frame: Option<u32>,
    fps: f32,
) -> Result<f32, String> {
    if let Some(time_seconds) = time_seconds {
        if frame.is_some() {
            return Err("--time-seconds and --frame cannot be used together".to_string());
        }
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err("--time-seconds must be a finite non-negative value".to_string());
        }
        return Ok(time_seconds);
    }

    if !fps.is_finite() || fps <= 0.0 {
        return Err("--fps must be a finite value greater than zero".to_string());
    }

    Ok(frame.map_or(DEFAULT_CAPTURE_SECONDS, |frame| frame as f32 / fps))
}

pub fn resolve_single_output_path(
    output: Option<&PathBuf>,
    output_dir: Option<&PathBuf>,
    default_dir: &Path,
    target: &CaptureTarget,
) -> Result<PathBuf, String> {
    if output.is_some() && output_dir.is_some() {
        return Err("--output and --output-dir cannot be used together".to_string());
    }

    if let Some(output) = output {
        return Ok(output.clone());
    }

    let resolved_dir = output_dir
        .cloned()
        .unwrap_or_else(|| default_dir.to_path_buf());
    Ok(resolved_dir.join(format!("{}.png", target.slug())))
}

pub fn resolve_batch_output_dir(
    output: Option<&PathBuf>,
    output_dir: Option<&PathBuf>,
    default_dir: &Path,
) -> Result<PathBuf, String> {
    if output.is_some() {
        return Err("--output can only be used for a single capture target".to_string());
    }

    Ok(output_dir
        .cloned()
        .unwrap_or_else(|| default_dir.to_path_buf()))
}

pub fn write_batch_manifest(
    output_dir: &Path,
    manifest_name: &str,
    command: &str,
    artifacts: &[CaptureArtifact],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;
    let manifest_path = output_dir.join(manifest_name);
    let manifest = CaptureBatchManifest {
        version: 1,
        command,
        artifact_count: artifacts.len(),
        artifacts,
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest_path)
}

pub fn print_capture_artifacts(artifacts: &[CaptureArtifact]) {
    for artifact in artifacts {
        println!(
            "{} -> {} ({})",
            describe_target(&artifact.target),
            artifact.png_path.display(),
            artifact.manifest_path.display()
        );
    }
}

fn describe_target(target: &CaptureTarget) -> String {
    match target {
        CaptureTarget::VizPrimitives => "viz_primitives".to_string(),
        CaptureTarget::ComponentShowcase => "component_showcase".to_string(),
        CaptureTarget::StorybookSection { name } => format!("storybook:{name}"),
        CaptureTarget::AdHoc { name } => name.clone(),
    }
}

#[derive(Serialize)]
struct CaptureBatchManifest<'a> {
    version: u32,
    command: &'a str,
    artifact_count: usize,
    artifacts: &'a [CaptureArtifact],
}
