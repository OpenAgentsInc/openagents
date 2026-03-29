use crate::{Scene, TextSystem};
use futures::executor::block_on;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use wgpu::Color;
use wgpui_render::{
    OffscreenGlyphAtlas, OffscreenRenderError, OffscreenRenderOutput, OffscreenRenderRequest,
    OffscreenRenderer, OffscreenRendererOptions,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CaptureTarget {
    VizPrimitives,
    ComponentShowcase,
    StorybookSection { name: String },
    AdHoc { name: String },
}

impl CaptureTarget {
    pub fn slug(&self) -> String {
        match self {
            Self::VizPrimitives => "viz_primitives".to_string(),
            Self::ComponentShowcase => "component_showcase".to_string(),
            Self::StorybookSection { name } => format!("storybook_{}", slugify(name)),
            Self::AdHoc { name } => slugify(name),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureClearColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl From<Color> for CaptureClearColor {
    fn from(color: Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

impl From<CaptureClearColor> for Color {
    fn from(color: CaptureClearColor) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: color.a,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureRequest {
    pub target: CaptureTarget,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub clear_color: CaptureClearColor,
    pub output_path: PathBuf,
    pub manifest_path: Option<PathBuf>,
    pub allow_fallback_adapter: bool,
}

impl CaptureRequest {
    pub fn new(target: CaptureTarget, width: u32, height: u32, output_path: PathBuf) -> Self {
        Self {
            target,
            width,
            height,
            scale_factor: 1.0,
            clear_color: CaptureClearColor::from(Color::BLACK),
            output_path,
            manifest_path: None,
            allow_fallback_adapter: false,
        }
    }

    pub fn resolved_output_path(&self) -> Result<PathBuf, CaptureError> {
        absolutize_path(&self.output_path)
    }

    pub fn resolved_manifest_path(&self) -> Result<PathBuf, CaptureError> {
        let manifest_path = self
            .manifest_path
            .clone()
            .unwrap_or_else(|| self.output_path.with_extension("json"));
        absolutize_path(&manifest_path)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureManifest {
    pub version: u32,
    pub target: CaptureTarget,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub clear_color: CaptureClearColor,
    pub texture_format: String,
    pub png_path: PathBuf,
    pub byte_len: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureArtifact {
    pub target: CaptureTarget,
    pub png_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: CaptureManifest,
}

#[derive(Debug)]
pub enum CaptureError {
    Offscreen(OffscreenRenderError),
    Io(std::io::Error),
    Encode(image::ImageError),
    Serialize(serde_json::Error),
    InvalidPixelLength { expected: usize, actual: usize },
    CurrentDirectory(std::io::Error),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offscreen(error) => write!(f, "offscreen capture failed: {error}"),
            Self::Io(error) => write!(f, "capture file IO failed: {error}"),
            Self::Encode(error) => write!(f, "capture PNG encode failed: {error}"),
            Self::Serialize(error) => write!(f, "capture manifest serialization failed: {error}"),
            Self::InvalidPixelLength { expected, actual } => write!(
                f,
                "capture pixel buffer length mismatch: expected {expected} bytes, got {actual}"
            ),
            Self::CurrentDirectory(error) => {
                write!(
                    f,
                    "failed to resolve current directory for capture path: {error}"
                )
            }
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<OffscreenRenderError> for CaptureError {
    fn from(error: OffscreenRenderError) -> Self {
        Self::Offscreen(error)
    }
}

impl From<std::io::Error> for CaptureError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<image::ImageError> for CaptureError {
    fn from(error: image::ImageError) -> Self {
        Self::Encode(error)
    }
}

impl From<serde_json::Error> for CaptureError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

pub fn capture_scene(
    request: &CaptureRequest,
    scene: &Scene,
    text_system: Option<&TextSystem>,
) -> Result<CaptureArtifact, CaptureError> {
    let offscreen = block_on(OffscreenRenderer::new(OffscreenRendererOptions {
        allow_fallback_adapter: request.allow_fallback_adapter,
        ..Default::default()
    }))?;
    let mut renderer = offscreen.create_renderer();
    let atlas = text_system.map(|text_system| OffscreenGlyphAtlas {
        data: text_system.atlas_data(),
        size: text_system.atlas_size(),
    });
    let output = offscreen.render_scene(
        &mut renderer,
        scene,
        OffscreenRenderRequest {
            width: request.width,
            height: request.height,
            scale_factor: request.scale_factor,
            clear_color: request.clear_color.into(),
            atlas,
        },
    )?;

    write_capture_artifacts(request, &output)
}

pub fn write_capture_artifacts(
    request: &CaptureRequest,
    output: &OffscreenRenderOutput,
) -> Result<CaptureArtifact, CaptureError> {
    let output_path = request.resolved_output_path()?;
    let manifest_path = request.resolved_manifest_path()?;
    ensure_parent_dir(&output_path)?;
    ensure_parent_dir(&manifest_path)?;
    validate_pixel_buffer(output)?;
    write_png(output, &output_path)?;

    let manifest = CaptureManifest {
        version: 1,
        target: request.target.clone(),
        width: output.width,
        height: output.height,
        scale_factor: output.scale_factor,
        clear_color: request.clear_color,
        texture_format: format!("{:?}", output.format),
        png_path: output_path.clone(),
        byte_len: output.bytes.len(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    fs::write(&manifest_path, manifest_bytes)?;

    Ok(CaptureArtifact {
        target: request.target.clone(),
        png_path: output_path,
        manifest_path,
        manifest,
    })
}

fn write_png(output: &OffscreenRenderOutput, output_path: &Path) -> Result<(), CaptureError> {
    let file = fs::File::create(output_path)?;
    let encoder = PngEncoder::new(file);
    encoder.write_image(
        &output.bytes,
        output.width,
        output.height,
        ColorType::Rgba8.into(),
    )?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<(), CaptureError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn validate_pixel_buffer(output: &OffscreenRenderOutput) -> Result<(), CaptureError> {
    let expected = output.width as usize * output.height as usize * 4;
    if output.bytes.len() != expected {
        return Err(CaptureError::InvalidPixelLength {
            expected,
            actual: output.bytes.len(),
        });
    }
    Ok(())
}

fn absolutize_path(path: &Path) -> Result<PathBuf, CaptureError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let current_dir = std::env::current_dir().map_err(CaptureError::CurrentDirectory)?;
        Ok(current_dir.join(path))
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut previous_was_dash = false;
    for ch in value.chars() {
        let mapped = match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' => ch,
            _ => '-',
        };
        if mapped == '-' {
            if !previous_was_dash && !slug.is_empty() {
                slug.push(mapped);
            }
            previous_was_dash = true;
        } else {
            slug.push(mapped);
            previous_was_dash = false;
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{CaptureRequest, CaptureTarget, write_capture_artifacts};
    use image::ImageReader;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wgpu::TextureFormat;
    use wgpui_render::OffscreenRenderOutput;

    #[test]
    fn write_capture_artifacts_roundtrip_png_and_manifest() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wgpui-capture-{unique}"));
        let request = CaptureRequest::new(
            CaptureTarget::AdHoc {
                name: "smoke test".to_string(),
            },
            2,
            2,
            root.join("smoke-test.png"),
        );
        let output = OffscreenRenderOutput {
            width: 2,
            height: 2,
            scale_factor: 1.0,
            format: TextureFormat::Rgba8UnormSrgb,
            bytes: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        };

        let artifact =
            write_capture_artifacts(&request, &output).expect("artifact write should succeed");

        let decoded = ImageReader::open(&artifact.png_path)
            .expect("png should exist")
            .decode()
            .expect("png should decode");
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);

        let manifest =
            std::fs::read_to_string(&artifact.manifest_path).expect("manifest should exist");
        assert!(manifest.contains("\"version\": 1"));
        assert!(manifest.contains("\"kind\": \"ad_hoc\""));
        assert!(manifest.contains("smoke test"));
    }
}
