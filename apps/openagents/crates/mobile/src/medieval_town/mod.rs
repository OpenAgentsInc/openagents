//! Native Medieval Town surface. Rust owns loading, movement, rendering, and timing.
//! The host retains a CAMetalLayer and forwards display callbacks and touch input.

const MANIFEST_URL: &str = "https://m4552b29fd280c68f---aethyrforge-preview-ezxz4mgdsq-uc.a.run.app/private-assets/native-town-4552b29fd280c68fd91ce2ba00311e3297838a536f87b71212103214c0d21ea6.json";
const MANIFEST_SHA256: &str = "4552b29fd280c68fd91ce2ba00311e3297838a536f87b71212103214c0d21ea6";

mod collision;
mod gpu;

use collision::Collision;
use glam::{Mat4, Vec3};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MedievalTownError {
    #[error("{message}")]
    Failed { message: String },
}
fn failed(message: impl Into<String>) -> MedievalTownError {
    MedievalTownError::Failed {
        message: message.into(),
    }
}
type Result<T> = std::result::Result<T, MedievalTownError>;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSpec {
    url: String,
    sha256: String,
    byte_length: usize,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    decoded_byte_length: Option<usize>,
    #[serde(default)]
    decoded_sha256: Option<String>,
    #[serde(default)]
    count: usize,
    #[serde(default)]
    stride: usize,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    triangle_count: usize,
    #[serde(default)]
    layers: u32,
    #[serde(default)]
    mip_count: u32,
    #[serde(default)]
    format: Option<String>,
}
#[derive(Clone, Deserialize)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}
fn identity() -> [f32; 16] {
    Mat4::IDENTITY.to_cols_array()
}
fn one_instance() -> u32 {
    1
}
fn full_texture() -> [f32; 4] {
    [0., 0., 1., 1.]
}
fn unit_uv() -> [f32; 2] {
    [1., 1.]
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Draw {
    first_index: u32,
    index_count: u32,
    #[serde(default = "identity")]
    model: [f32; 16],
    #[serde(default = "full_texture")]
    atlas_rect: [f32; 4],
    #[serde(default)]
    material_layer: u32,
    #[serde(default)]
    first_instance: u32,
    #[serde(default = "one_instance")]
    instance_count: u32,
    #[serde(default = "unit_uv")]
    uv_scale: [f32; 2],
    #[serde(default)]
    alpha_cutoff: f32,
    bounds: Bounds,
}
#[derive(Deserialize)]
struct Manifest {
    schema: String,
    vertices: FileSpec,
    indices: FileSpec,
    #[serde(default)]
    instances: Option<FileSpec>,
    #[serde(default)]
    atlas: Option<FileSpec>,
    #[serde(rename = "textureArrays", default)]
    texture_arrays: Option<TextureArraySpecs>,
    collision: FileSpec,
    draws: Vec<Draw>,
    spawn: [f32; 3],
    bounds: Bounds,
    #[serde(rename = "walkBounds", default)]
    walk_bounds: Option<Bounds>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextureArraySpecs {
    base_color: FileSpec,
    normal: FileSpec,
    mra: FileSpec,
}
struct TextureArray {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    layers: u32,
    mip_count: u32,
}
struct TextureArrays {
    base_color: TextureArray,
    normal: TextureArray,
    mra: TextureArray,
}
impl Manifest {
    fn files(&self) -> Vec<&FileSpec> {
        let mut files = vec![&self.vertices, &self.indices];
        if let Some(instances) = &self.instances {
            files.push(instances);
        }
        if let Some(arrays) = &self.texture_arrays {
            files.extend([&arrays.base_color, &arrays.normal, &arrays.mra]);
        } else if let Some(atlas) = &self.atlas {
            files.push(atlas);
        }
        files.push(&self.collision);
        files
    }
}
struct Mesh {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    vertex_stride: u64,
    instances: Vec<[f32; 28]>,
    texture_arrays: Option<TextureArrays>,
    atlas: Vec<u8>,
    atlas_width: u32,
    atlas_height: u32,
    draws: Vec<Draw>,
    collision: Collision,
    spawn: Vec3,
    walk_bounds: Option<Bounds>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn validate_manifest(m: &Manifest) -> Result<()> {
    let v2 = m.schema == "native-town.v2";
    let stride = if v2 { 48 } else { 32 };
    let instance_count = if let Some(instances) = &m.instances {
        if !v2
            || instances.format.as_deref() != Some("float32x28-le")
            || instances.stride != 112
            || instances.count == 0
            || instances.count > 65_536
            || instances.count.checked_mul(112) != Some(raw_length(instances))
        {
            return Err(failed(
                "The town placements have an unsupported size or format.",
            ));
        }
        instances.count
    } else {
        1
    };
    let texture_layers = if let Some(arrays) = &m.texture_arrays {
        if !v2 || m.atlas.is_some() {
            return Err(failed("The town textures have an unsupported format."));
        }
        let layers = arrays.base_color.layers;
        for (spec, format, channels) in [
            (&arrays.base_color, "rgba8unorm-srgb", 4usize),
            (&arrays.normal, "rg8unorm", 2),
            (&arrays.mra, "rgba8unorm", 4),
        ] {
            if spec.format.as_deref() != Some(format)
                || spec.width == 0
                || spec.width != spec.height
                || !spec.width.is_power_of_two()
                || spec.width > 1024
                || spec.layers == 0
                || spec.layers > 128
                || spec.layers != layers
                || spec.mip_count != spec.width.ilog2() + 1
            {
                return Err(failed(
                    "The town textures have an unsupported size or format.",
                ));
            }
            let expected = (0..spec.mip_count).try_fold(0usize, |sum, level| {
                let size = (spec.width >> level).max(1) as usize;
                size.checked_mul(size)?
                    .checked_mul(channels)?
                    .checked_mul(layers as usize)?
                    .checked_add(sum)
            });
            if expected != Some(raw_length(spec)) {
                return Err(failed("A town texture has an unexpected size."));
            }
        }
        layers
    } else {
        if v2
            || m.atlas
                .as_ref()
                .is_none_or(|a| a.width == 0 || a.height == 0 || a.width > 2048 || a.height > 2048)
        {
            return Err(failed(
                "The town textures have an unsupported size or format.",
            ));
        }
        1
    };
    if (!v2 && m.schema != "native-town.v1")
        || m.vertices.stride != stride
        || m.vertices.count > 6_000_000
        || m.vertices.count.checked_mul(stride) != Some(raw_length(&m.vertices))
        || m.indices.count > 18_000_000
        || m.indices.count % 3 != 0
        || m.indices.count.checked_mul(4) != Some(raw_length(&m.indices))
        || m.collision.triangle_count > 500_000
        || m.collision.triangle_count.checked_mul(36) != Some(raw_length(&m.collision))
        || m.draws.is_empty()
        || m.draws.len() > 8192
        || !m
            .spawn
            .iter()
            .chain(m.bounds.min.iter())
            .chain(m.bounds.max.iter())
            .all(|v| v.is_finite() && v.abs() < 30_000.)
    {
        return Err(failed("The town files have an unsupported size or format."));
    }
    let files = m.files();
    if files
        .iter()
        .try_fold(0usize, |sum, file| sum.checked_add(raw_length(file)))
        .is_none_or(|bytes| bytes > 512 * 1024 * 1024)
    {
        return Err(failed("The town files are too large for this device."));
    }
    for file in files {
        if !valid_hash(&file.sha256)
            || file.byte_length == 0
            || file.byte_length > 256 * 1024 * 1024
            || raw_length(file) > 256 * 1024 * 1024
            || file.encoding.as_deref().is_some_and(|v| v != "gzip")
            || (file.encoding.is_some()
                && (file.decoded_byte_length.is_none()
                    || file
                        .decoded_sha256
                        .as_deref()
                        .is_none_or(|v| !valid_hash(v))))
        {
            return Err(failed("A town file has an invalid size or checksum."));
        }
    }
    if (0..3).any(|i| m.bounds.min[i] > m.bounds.max[i])
        || m.walk_bounds.as_ref().is_some_and(|b| {
            b.min
                .iter()
                .chain(&b.max)
                .any(|v| !v.is_finite() || v.abs() > 30_000.)
                || (0..3).any(|i| b.min[i] > b.max[i])
        })
    {
        return Err(failed("The town has invalid walking bounds."));
    }
    let mut covered = 0u32;
    let mut referenced_indices = 0u64;
    for d in &m.draws {
        if d.material_layer >= texture_layers
            || (m.instances.is_none() && d.first_index != covered)
            || d.first_index % 3 != 0
            || d.instance_count == 0
            || d.first_instance
                .checked_add(d.instance_count)
                .is_none_or(|n| n as usize > instance_count)
            || d.first_index
                .checked_add(d.index_count)
                .is_none_or(|n| n as usize > m.indices.count)
            || d.index_count % 3 != 0
            || !d
                .model
                .iter()
                .chain(d.atlas_rect.iter())
                .chain(d.uv_scale.iter())
                .chain(d.bounds.min.iter())
                .chain(d.bounds.max.iter())
                .all(|v| v.is_finite())
            || !(0. ..=1.).contains(&d.alpha_cutoff)
            || d.atlas_rect[2] <= 0.
            || d.atlas_rect[3] <= 0.
            || d.atlas_rect[0] < 0.
            || d.atlas_rect[1] < 0.
            || d.atlas_rect[0] + d.atlas_rect[2] > 1.00001
            || d.atlas_rect[1] + d.atlas_rect[3] > 1.00001
            || (0..3).any(|i| d.bounds.min[i] > d.bounds.max[i])
        {
            return Err(failed("A town section has invalid geometry."));
        }
        if m.instances.is_none() {
            covered += d.index_count;
        }
        referenced_indices += d.index_count as u64 * d.instance_count as u64;
    }
    if referenced_indices > 24_000_000
        || (m.instances.is_none() && covered as usize != m.indices.count)
    {
        return Err(failed("The town mesh is incomplete."));
    }
    Ok(())
}
fn raw_length(file: &FileSpec) -> usize {
    file.decoded_byte_length.unwrap_or(file.byte_length)
}
fn decode_file(bytes: Vec<u8>, spec: &FileSpec) -> Result<Vec<u8>> {
    if spec.encoding.as_deref() != Some("gzip") {
        return Ok(bytes);
    }
    let limit = raw_length(spec);
    let mut decoded = Vec::with_capacity(limit.min(8 * 1024 * 1024));
    flate2::read::GzDecoder::new(bytes.as_slice())
        .take(limit as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|_| failed("A town file could not be unpacked."))?;
    if decoded.len() != limit || Some(digest(&decoded)).as_deref() != spec.decoded_sha256.as_deref()
    {
        return Err(failed("A town file did not finish downloading."));
    }
    Ok(decoded)
}
fn validate_vertices(values: &[f32], stride: usize) -> Result<()> {
    if ![32, 48].contains(&stride) {
        return Err(failed("The town vertex format is unsupported."));
    }
    let floats = stride / 4;
    if values.len() % floats != 0
        || values.chunks_exact(floats).any(|v| {
            v[..3].iter().any(|n| n.abs() > 30_000.)
                || !(0.25..=1.1).contains(&(v[3] * v[3] + v[4] * v[4] + v[5] * v[5]))
                || v[6..8].iter().any(|n| n.abs() > 65536.)
                || (floats == 12
                    && (!(0.25..=1.1).contains(&(v[8] * v[8] + v[9] * v[9] + v[10] * v[10]))
                        || (v[11].abs() - 1.).abs() > 0.001))
        })
    {
        return Err(failed("The town mesh has invalid positions or normals."));
    }
    Ok(())
}
fn decode_floats(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(failed("A town mesh is incomplete."));
    }
    let result: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if result.iter().any(|v| !v.is_finite()) {
        return Err(failed("A town mesh contains an invalid number."));
    }
    Ok(result)
}
async fn download(
    client: &reqwest::Client,
    url: &reqwest::Url,
    expected: &str,
    size: Option<usize>,
    path: &Path,
) -> Result<Vec<u8>> {
    if let Ok(bytes) = std::fs::read(path) {
        if size.is_none_or(|n| n == bytes.len()) && digest(&bytes) == expected {
            return Ok(bytes);
        }
    }
    let maximum = size.unwrap_or(8 * 1024 * 1024);
    let mut last = String::new();
    for _ in 0..3 {
        let attempt = async {
            let mut response = client
                .get(url.clone())
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            if response
                .content_length()
                .is_some_and(|n| n > maximum as u64)
            {
                return Err("file is larger than expected".into());
            }
            let mut bytes = Vec::with_capacity(maximum.min(8 * 1024 * 1024));
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                if bytes.len() + chunk.len() > maximum {
                    return Err("file is larger than expected".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            if size.is_some_and(|n| n != bytes.len()) || digest(&bytes) != expected {
                return Err("file did not finish downloading".into());
            }
            Ok::<_, String>(bytes)
        }
        .await;
        match attempt {
            Ok(bytes) => {
                let temporary = path.with_extension("download");
                std::fs::write(&temporary, &bytes)
                    .map_err(|_| failed("The town could not be saved on this device."))?;
                std::fs::rename(&temporary, path)
                    .map_err(|_| failed("The town could not be saved on this device."))?;
                return Ok(bytes);
            }
            Err(error) => last = error,
        }
    }
    eprintln!(
        "Medieval Town download {}: {}",
        url.path(),
        last.chars().take(160).collect::<String>()
    );
    Err(failed("The town could not finish downloading. Try again."))
}
async fn load(cache: &Path) -> Result<Mesh> {
    let url = option_env!("OPENAGENTS_MEDIEVAL_MANIFEST_URL").unwrap_or(MANIFEST_URL);
    let expected = option_env!("OPENAGENTS_MEDIEVAL_MANIFEST_SHA256").unwrap_or(MANIFEST_SHA256);
    let url = reqwest::Url::parse(url)
        .map_err(|_| failed("The town download has not been configured."))?;
    if url.scheme() != "https" || !valid_hash(expected) {
        return Err(failed("The town download has not been configured."));
    }
    let cache = cache.join("MedievalTown").join(expected);
    std::fs::create_dir_all(&cache)
        .map_err(|_| failed("The town could not be saved on this device."))?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| failed("The town download could not start."))?;
    let bytes = download(&client, &url, expected, None, &cache.join("manifest.json")).await?;
    let m: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| failed("The town description could not be read."))?;
    validate_manifest(&m)?;
    let file = |spec: &FileSpec| -> Result<(reqwest::Url, PathBuf)> {
        let target = url
            .join(&spec.url)
            .map_err(|_| failed("A town file has an invalid address."))?;
        if target.origin() != url.origin() || target.scheme() != "https" {
            return Err(failed("A town file has an invalid address."));
        }
        Ok((target, cache.join(&spec.sha256)))
    };
    let mut parts = Vec::new();
    for spec in m.files() {
        let (url, path) = file(spec)?;
        parts.push(decode_file(
            download(&client, &url, &spec.sha256, Some(spec.byte_length), &path).await?,
            spec,
        )?);
    }
    decode_mesh(m, parts)
}
fn decode_mesh(m: Manifest, parts: Vec<Vec<u8>>) -> Result<Mesh> {
    validate_manifest(&m)?;
    if parts.len() != m.files().len() {
        return Err(failed("The town files are incomplete."));
    }
    for (spec, part) in m.files().into_iter().zip(&parts) {
        if part.len() != raw_length(spec) {
            return Err(failed("A town file has an unexpected size."));
        }
    }
    let mut parts = parts.into_iter();
    let vertices = decode_floats(&parts.next().unwrap())?;
    validate_vertices(&vertices, m.vertices.stride)?;
    let indices: Vec<u32> = parts
        .next()
        .unwrap()
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if indices.iter().any(|&i| i as usize >= m.vertices.count) {
        return Err(failed("A town mesh points to a missing vertex."));
    }
    let instances = if m.instances.is_some() {
        let values = decode_floats(&parts.next().unwrap())?;
        let mut instances = Vec::with_capacity(values.len() / 28);
        for values in values.chunks_exact(28) {
            if values.iter().any(|v| v.abs() > 30_000.)
                || values[3].abs() > 0.0001
                || values[7].abs() > 0.0001
                || values[11].abs() > 0.0001
                || (values[15] - 1.).abs() > 0.0001
                || (values[19].abs() - 1.).abs() > 0.001
            {
                return Err(failed("A town placement has an invalid transform."));
            }
            instances.push(values.try_into().unwrap());
        }
        instances
    } else {
        vec![identity_instance()]
    };
    let (texture_arrays, atlas, atlas_width, atlas_height) = if let Some(arrays) = &m.texture_arrays
    {
        let mut array = |spec: &FileSpec| TextureArray {
            bytes: parts.next().unwrap(),
            width: spec.width,
            height: spec.height,
            layers: spec.layers,
            mip_count: spec.mip_count,
        };
        (
            Some(TextureArrays {
                base_color: array(&arrays.base_color),
                normal: array(&arrays.normal),
                mra: array(&arrays.mra),
            }),
            Vec::new(),
            0,
            0,
        )
    } else {
        let spec = m.atlas.as_ref().unwrap();
        let bytes = parts.next().unwrap();
        let dimensions =
            image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png)
                .into_dimensions()
                .map_err(|_| failed("The town texture could not be read."))?;
        if dimensions != (spec.width, spec.height) {
            return Err(failed("The town texture has an unexpected size."));
        }
        let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
            .map_err(|_| failed("The town texture could not be read."))?
            .into_rgba8();
        (None, image.into_raw(), spec.width, spec.height)
    };
    let collision = Collision::new(&decode_floats(&parts.next().unwrap())?)?;
    Ok(Mesh {
        vertices,
        indices,
        vertex_stride: m.vertices.stride as u64,
        instances,
        texture_arrays,
        atlas,
        atlas_width,
        atlas_height,
        draws: m.draws,
        collision,
        spawn: Vec3::from_array(m.spawn),
        walk_bounds: m.walk_bounds,
    })
}

fn identity_instance() -> [f32; 28] {
    let mut value = [0.; 28];
    value[..16].copy_from_slice(&identity());
    value[16] = 1.;
    value[19] = 1.;
    value[21] = 1.;
    value[26] = 1.;
    value
}

struct Camera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    velocity_y: f32,
    grounded: bool,
    jump_held: bool,
    last: Option<f64>,
}
impl Camera {
    fn new(position: Vec3) -> Self {
        Self {
            position,
            yaw: std::f32::consts::PI,
            pitch: -0.04,
            velocity_y: 0.,
            grounded: false,
            jump_held: false,
            last: None,
        }
    }
    fn update(
        &mut self,
        mesh: &Mesh,
        time: f64,
        x: f32,
        z: f32,
        lx: f32,
        ly: f32,
        run: bool,
        jump: bool,
    ) {
        let dt = self
            .last
            .map_or(1. / 60., |last| (time - last).clamp(0., 0.05) as f32);
        self.last = Some(time);
        self.yaw = (self.yaw + lx.clamp(-500., 500.) * 0.004).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch - ly.clamp(-500., 500.) * 0.004).clamp(-1.45, 1.45);
        let mut input = Vec3::new(x.clamp(-1., 1.), 0., z.clamp(-1., 1.));
        if input.length_squared() > 1. {
            input = input.normalize();
        }
        let forward = Vec3::new(self.yaw.sin(), 0., -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0., self.yaw.sin());
        if jump && !self.jump_held && self.grounded {
            self.velocity_y = 6.2;
            self.grounded = false;
        }
        self.jump_held = jump;
        let mut delta = (right * input.x + forward * input.z) * dt * if run { 6.4 } else { 3.4 };
        if let Some(b) = &mesh.walk_bounds {
            let next = self.position + delta;
            delta.x = next.x.clamp(b.min[0], b.max[0]) - self.position.x;
            delta.z = next.z.clamp(b.min[2], b.max[2]) - self.position.z;
        }
        self.velocity_y = (self.velocity_y - 15.696 * dt).max(-50.);
        let dy = self.velocity_y * dt;
        self.position = mesh
            .collision
            .move_capsule(self.position, delta, dy, &mut self.grounded);
        if self.grounded {
            self.velocity_y = 0.;
        }
        if self.position.y < -100. {
            self.position = mesh.spawn;
            self.velocity_y = 0.;
        }
    }
    fn eye(&self) -> Vec3 {
        self.position + Vec3::Y * 1.62
    }
    fn direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
}
#[derive(Default)]
struct Timing {
    start: Option<Instant>,
    frozen_elapsed: Option<f64>,
    submitted: u64,
    cpu: VecDeque<f64>,
    draws: u32,
    triangles: u32,
    distance_walked: f64,
    max_displacement: f32,
}
impl Timing {
    fn sample(&mut self, milliseconds: f64, draws: u32, triangles: u32) {
        self.start.get_or_insert_with(Instant::now);
        self.submitted += 1;
        self.cpu.push_back(milliseconds);
        if self.cpu.len() > 3600 {
            self.cpu.pop_front();
        }
        self.draws = draws;
        self.triangles = triangles;
    }
}
struct Benchmark {
    start: Instant,
    warmed: bool,
    done: bool,
}
struct State {
    mesh: Option<Arc<Mesh>>,
    camera: Camera,
    timing: Timing,
    active: bool,
    width: u32,
    height: u32,
    benchmark: Option<Benchmark>,
}
#[derive(uniffi::Object)]
pub struct MedievalTownRenderer {
    cache: PathBuf,
    state: Mutex<State>,
    gpu: Mutex<Option<gpu::Gpu>>,
    loading: AtomicBool,
}
#[uniffi::export(async_runtime = "tokio")]
impl MedievalTownRenderer {
    #[uniffi::constructor]
    pub fn new(cache_directory: String) -> Arc<Self> {
        Arc::new(Self {
            cache: PathBuf::from(cache_directory),
            state: Mutex::new(State {
                mesh: None,
                camera: Camera::new(Vec3::ZERO),
                timing: Timing::default(),
                active: true,
                width: 0,
                height: 0,
                benchmark: None,
            }),
            gpu: Mutex::new(None),
            loading: AtomicBool::new(false),
        })
    }
    pub async fn prepare(&self) -> Result<()> {
        if self.loading.swap(true, Ordering::AcqRel) {
            return Err(failed("The town is already loading."));
        }
        struct LoadingGuard<'a>(&'a AtomicBool);
        impl Drop for LoadingGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let _loading = LoadingGuard(&self.loading);
        let result = load(&self.cache).await;
        let mesh = Arc::new(result?);
        let mut state = self.state.lock().unwrap();
        state.camera = Camera::new(mesh.spawn);
        state.mesh = Some(mesh);
        Ok(())
    }
    pub fn attach(&self, layer: u64, width: u32, height: u32, scale: f32) -> Result<()> {
        let (width, height) = extent(width, height, scale)?;
        let mut state = self.state.lock().unwrap();
        let mesh = state
            .mesh
            .as_ref()
            .ok_or_else(|| failed("Wait for the town to finish loading."))?;
        let renderer = gpu::Gpu::new(layer, width, height, mesh)?;
        *self.gpu.lock().unwrap() = Some(renderer);
        state.width = width;
        state.height = height;
        state.timing = Timing::default();
        Ok(())
    }
    pub fn resize(&self, width: u32, height: u32, scale: f32) -> Result<()> {
        let (width, height) = extent(width, height, scale)?;
        let mut state = self.state.lock().unwrap();
        if let Some(gpu) = self.gpu.lock().unwrap().as_mut() {
            gpu.resize(width, height);
        }
        state.width = width;
        state.height = height;
        Ok(())
    }
    pub fn set_active(&self, active: bool) {
        let mut state = self.state.lock().unwrap();
        state.active = active;
        state.camera.last = None;
    }
    pub fn detach(&self) {
        let mut state = self.state.lock().unwrap();
        *self.gpu.lock().unwrap() = None;
        state.active = false;
        state.camera.last = None;
    }
    pub fn frame(
        &self,
        timestamp: f64,
        move_x: f32,
        move_z: f32,
        look_x: f32,
        look_y: f32,
        run: bool,
        jump: bool,
    ) -> Result<()> {
        if !timestamp.is_finite()
            || ![move_x, move_z, look_x, look_y]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err(failed("The movement input could not be read."));
        }
        let start = Instant::now();
        let mut state = self.state.lock().unwrap();
        if !state.active {
            return Ok(());
        }
        let Some(mesh) = state.mesh.clone() else {
            return Ok(());
        };
        let mut guard = self.gpu.lock().unwrap();
        let Some(gpu) = guard.as_mut() else {
            return Ok(());
        };
        let mut input = (move_x, move_z, look_x, look_y, run, jump);
        if state.benchmark.as_ref().is_some_and(|b| b.done) {
            gpu.poll();
            return Ok(());
        }
        if let Some(benchmark) = state.benchmark.as_ref() {
            let elapsed = benchmark.start.elapsed().as_secs_f64();
            let warmed = benchmark.warmed;
            if elapsed >= 5. && !warmed {
                state.timing = Timing::default();
                gpu.reset_metrics();
                state.benchmark.as_mut().unwrap().warmed = true;
            }
            if elapsed >= 35. {
                state.timing.frozen_elapsed = state.timing.start.map(|t| t.elapsed().as_secs_f64());
                state.benchmark.as_mut().unwrap().done = true;
                gpu.poll();
                return Ok(());
            }
            let time = elapsed - 5.;
            input = if (7. ..11.).contains(&time) {
                (0., 0.4, 0., 0., false, false)
            } else if (11. ..15.).contains(&time) {
                (0., -0.4, 0., 0., false, false)
            } else if (15. ..19.).contains(&time) {
                (0.4, 0., 0., 0., false, false)
            } else if (19. ..23.).contains(&time) {
                (-0.4, 0., 0., 0., false, false)
            } else if time >= 23. {
                (0., 0., 0.5, 0., false, false)
            } else {
                (0., 0., 0., 0., false, false)
            };
        }
        let previous_position = state.camera.position;
        state.camera.update(
            &mesh, timestamp, input.0, input.1, input.2, input.3, input.4, input.5,
        );
        let moved = state.camera.position - previous_position;
        state.timing.distance_walked += moved.x.hypot(moved.z) as f64;
        state.timing.max_displacement = state
            .timing
            .max_displacement
            .max((state.camera.position - mesh.spawn).length());
        if let Some((draws, triangles)) = gpu.draw(&state.camera, &mesh)? {
            state
                .timing
                .sample(start.elapsed().as_secs_f64() * 1000., draws, triangles);
        }
        Ok(())
    }
    pub fn begin_benchmark(&self) {
        let mut state = self.state.lock().unwrap();
        state.benchmark = Some(Benchmark {
            start: Instant::now(),
            warmed: false,
            done: false,
        });
        state.active = true;
        if let Some(mesh) = state.mesh.clone() {
            state.camera = Camera::new(mesh.spawn);
        }
    }
    pub fn reset_metrics(&self) {
        let mut state = self.state.lock().unwrap();
        state.timing = Timing::default();
        if let Some(gpu) = self.gpu.lock().unwrap().as_ref() {
            gpu.reset_metrics();
        }
    }
    pub fn metrics(&self) -> String {
        let state = self.state.lock().unwrap();
        let gpu = self.gpu.lock().unwrap();
        let completed = gpu.as_ref().map_or(0, |g| g.completed());
        let elapsed = state
            .timing
            .frozen_elapsed
            .unwrap_or_else(|| state.timing.start.map_or(0., |t| t.elapsed().as_secs_f64()));
        let (frame_p95, missed) = gpu.as_ref().map_or((0., 0), |g| g.cadence());
        let mut cpu: Vec<_> = state.timing.cpu.iter().copied().collect();
        cpu.sort_by(f64::total_cmp);
        let mean = cpu.iter().sum::<f64>() / cpu.len().max(1) as f64;
        let p95 = cpu
            .get((cpu.len() as f64 * 0.95) as usize)
            .copied()
            .unwrap_or(0.);
        serde_json::json!({"ready":gpu.is_some(),"submittedFrames":state.timing.submitted,"completedFrames":completed,"fps":if elapsed>0.{completed as f64/elapsed}else{0.},"liveFps":gpu.as_ref().map_or(0.,|g|g.live_fps()),"cpuMs":mean,"cpuP95Ms":p95,"gpuMs":serde_json::Value::Null,"frameP95Ms":frame_p95,"missedFramesOver25Ms":missed,"benchmarkComplete":state.benchmark.as_ref().is_some_and(|b|b.done),"elapsed":elapsed,"playerPosition":state.camera.position.to_array(),"distanceWalked":state.timing.distance_walked,"maxDisplacement":state.timing.max_displacement,"yaw":state.camera.yaw,"pitch":state.camera.pitch,"drawCalls":state.timing.draws,"triangles":state.timing.triangles,"width":state.width,"height":state.height,"backend":"Metal","materialLayers":state.mesh.as_ref().and_then(|m| m.texture_arrays.as_ref()).map_or(1, |a| a.base_color.layers),"sceneDrawGroups":state.mesh.as_ref().map_or(0,|m|m.draws.len()),"sceneTriangles":state.mesh.as_ref().map_or(0,|m|m.draws.iter().map(|d|d.index_count as u64 / 3 * d.instance_count as u64).sum::<u64>()),"manifestSha256":option_env!("OPENAGENTS_MEDIEVAL_MANIFEST_SHA256").unwrap_or(MANIFEST_SHA256)}).to_string()
    }
}
fn extent(width: u32, height: u32, scale: f32) -> Result<(u32, u32)> {
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || !scale.is_finite()
        || scale <= 0.
    {
        return Err(failed("The town view has an invalid size."));
    }
    let long = width.max(height) as f32;
    let short = width.min(height) as f32;
    let ratio = (1280. / long).min(720. / short).min(1.);
    Ok((
        (width as f32 * ratio).round().max(1.) as u32,
        (height as f32 * ratio).round().max(1.) as u32,
    ))
}

#[cfg(test)]
mod tests;
