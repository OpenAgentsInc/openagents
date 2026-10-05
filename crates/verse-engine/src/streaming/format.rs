//! Stable, bounded little-endian static geometry and RGBA image chunks.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
const HEADER: usize = 32;
const MAGIC: &[u8; 8] = b"VSC1\r\n\x1a\n";
pub const MAX_CHUNK_BYTES: u64 = 16 * 1024 * 1024 + HEADER as u64;
#[repr(C)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub fog: f32,
}
impl Vertex {
    fn validate(&self) -> bool {
        self.pos
            .iter()
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            && self
                .color
                .iter()
                .all(|n| n.is_finite() && (0.0..=64.).contains(n))
            && self.uv.iter().all(|n| n.is_finite() && n.abs() <= 256.)
            && self.fog.is_finite()
            && (0.0..=1.).contains(&self.fog)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Kind {
    Triangles { vertices: u32 },
    Lines { vertices: u32 },
    Image { width: u32, height: u32 },
}
impl Kind {
    pub fn bytes(&self) -> u64 {
        match self {
            Self::Triangles { vertices } | Self::Lines { vertices } => u64::from(*vertices) * 36,
            Self::Image { width, height } => u64::from(*width)
                .saturating_mul(u64::from(*height))
                .saturating_mul(4),
        }
    }
    fn validate(&self) -> Result<(), String> {
        let valid = match self {
            Self::Triangles { vertices } => *vertices > 0 && vertices.is_multiple_of(3),
            Self::Lines { vertices } => *vertices > 0 && vertices.is_multiple_of(2),
            Self::Image { width, height } => {
                (1..=2048).contains(width) && (1..=2048).contains(height)
            }
        };
        if !valid || self.bytes().saturating_add(HEADER as u64) > MAX_CHUNK_BYTES {
            return Err("Invalid cooked chunk shape".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub sha256: String,
    pub encoded_bytes: u64,
    pub payload: Kind,
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// An image dependency sampled by this geometry; other dependencies are shared geometry.
    pub image: Option<String>,
    /// Identity of an offline lighting recipe, when vertex colors contain its baked result.
    pub lighting: Option<String>,
}
impl Descriptor {
    pub fn gpu_bytes(&self) -> u64 {
        self.payload.bytes()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub chunks: BTreeMap<String, Descriptor>,
}
impl Manifest {
    pub fn from_json(bytes: &[u8], expected: &str) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 || hash(bytes) != expected {
            return Err("Cooked manifest extent or identity differs".into());
        }
        let manifest: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.chunks.is_empty() || self.chunks.len() > 4096 {
            return Err("Invalid cooked chunk manifest".into());
        }
        // Bound metadata independently of payload residency, including programmatic manifests.
        let edges: usize = self.chunks.values().map(|d| d.dependencies.len()).sum();
        if edges > 8192 {
            return Err("Cooked dependency metadata exceeds its bound".into());
        }
        for (id, d) in &self.chunks {
            if !digest(id)
                || id != &d.sha256
                || d.encoded_bytes != d.gpu_bytes().saturating_add(HEADER as u64)
                || d.dependencies.len() > 64
                || d.dependencies.iter().collect::<BTreeSet<_>>().len() != d.dependencies.len()
                || d.lighting.as_ref().is_some_and(|v| !digest(v))
            {
                return Err("Invalid cooked chunk descriptor".into());
            }
            d.payload.validate()?;
            for dep in &d.dependencies {
                if !self.chunks.contains_key(dep) || dep == id {
                    return Err("Missing or recursive chunk dependency".into());
                }
            }
            if let Some(image) = &d.image {
                if !d.dependencies.contains(image)
                    || !matches!(
                        self.chunks.get(image).map(|v| &v.payload),
                        Some(Kind::Image { .. })
                    )
                    || matches!(d.payload, Kind::Image { .. })
                {
                    return Err("Geometry image is not an admitted image dependency".into());
                }
            }
        }
        // Color-marked iterative DFS admits the DAG without recursion on untrusted input.
        let mut done = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        for root in self.chunks.keys() {
            let mut stack = vec![(root.clone(), false)];
            while let Some((id, exit)) = stack.pop() {
                if exit {
                    visiting.remove(&id);
                    done.insert(id);
                    continue;
                }
                if done.contains(&id) {
                    continue;
                }
                if !visiting.insert(id.clone()) {
                    return Err("Chunk dependencies contain a cycle".into());
                }
                stack.push((id.clone(), true));
                for dep in self.chunks[&id].dependencies.iter().rev() {
                    stack.push((dep.clone(), false));
                }
            }
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<String, String> {
        self.validate()?;
        Ok(hash(&serde_json::to_vec(self).map_err(|e| e.to_string())?))
    }
    pub(super) fn closure(
        &self,
        root: &str,
        seen: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if !self.chunks.contains_key(root) {
            return Err("Unknown desired chunk".into());
        }
        let mut stack = vec![(root.to_owned(), false)];
        let mut entered = BTreeSet::new();
        while let Some((id, exit)) = stack.pop() {
            if exit {
                if seen.insert(id.clone()) {
                    order.push(id);
                }
                continue;
            }
            if seen.contains(&id) || !entered.insert(id.clone()) {
                continue;
            }
            stack.push((id.clone(), true));
            for dep in self.chunks[&id].dependencies.iter().rev() {
                stack.push((dep.clone(), false));
            }
        }
        Ok(())
    }
}
pub struct Decoded {
    data: Arc<Vec<u8>>,
    descriptor: Descriptor,
}
impl Decoded {
    pub fn decode(bytes: Vec<u8>, descriptor: &Descriptor) -> Result<Self, String> {
        descriptor.payload.validate()?;
        if bytes.len() as u64 != descriptor.encoded_bytes
            || bytes.len() > MAX_CHUNK_BYTES as usize
            || bytes.len() < HEADER
            || &bytes[..8] != MAGIC
            || hash(&bytes) != descriptor.sha256
        {
            return Err("Cooked chunk content identity or size differs".into());
        }
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let shape = match word(8) {
            1 => Kind::Triangles { vertices: word(12) },
            2 => Kind::Lines { vertices: word(12) },
            3 => Kind::Image {
                width: word(12),
                height: word(16),
            },
            _ => return Err("Unsupported cooked chunk kind".into()),
        };
        if shape != descriptor.payload
            || bytes[20..HEADER].iter().any(|v| *v != 0)
            || (!matches!(shape, Kind::Image { .. }) && word(16) != 0)
            || shape.bytes() != bytes.len() as u64 - HEADER as u64
        {
            return Err("Cooked chunk header differs from its descriptor".into());
        }
        if !matches!(shape, Kind::Image { .. }) {
            for raw in bytes[HEADER..].chunks_exact(36) {
                let n: [f32; 9] = std::array::from_fn(|i| {
                    f32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
                });
                let vertex = Vertex {
                    pos: n[..3].try_into().unwrap(),
                    color: n[3..6].try_into().unwrap(),
                    uv: n[6..8].try_into().unwrap(),
                    fog: n[8],
                };
                if !vertex.validate() {
                    return Err("Cooked vertex contains invalid values".into());
                }
            }
        }
        Ok(Self {
            data: Arc::new(bytes),
            descriptor: descriptor.clone(),
        })
    }
    pub fn payload(&self) -> &[u8] {
        &self.data[HEADER..]
    }
    pub(super) fn matches(&self, d: &Descriptor) -> bool {
        self.descriptor == *d
    }
}
fn digest(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn header(kind: u32, a: u32, b: u32) -> Vec<u8> {
    let mut bytes = vec![0; HEADER];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..12].copy_from_slice(&kind.to_le_bytes());
    bytes[12..16].copy_from_slice(&a.to_le_bytes());
    bytes[16..20].copy_from_slice(&b.to_le_bytes());
    bytes
}
fn finish(
    bytes: Vec<u8>,
    payload: Kind,
    dependencies: Vec<String>,
    image: Option<String>,
    lighting: Option<String>,
) -> Result<(Descriptor, Vec<u8>), String> {
    let d = Descriptor {
        sha256: hash(&bytes),
        encoded_bytes: bytes.len() as u64,
        payload,
        dependencies,
        image,
        lighting,
    };
    Decoded::decode(bytes.clone(), &d)?;
    Ok((d, bytes))
}
pub fn cook_geometry(
    vertices: &[Vertex],
    lines: bool,
    dependencies: Vec<String>,
    image: Option<String>,
    lighting: Option<String>,
) -> Result<(Descriptor, Vec<u8>), String> {
    let count = u32::try_from(vertices.len()).map_err(|_| "Too many cooked vertices")?;
    let payload = if lines {
        Kind::Lines { vertices: count }
    } else {
        Kind::Triangles { vertices: count }
    };
    payload.validate()?;
    let mut bytes = header(if lines { 2 } else { 1 }, count, 0);
    for v in vertices {
        for value in v.pos.into_iter().chain(v.color).chain(v.uv).chain([v.fog]) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    finish(bytes, payload, dependencies, image, lighting)
}
pub fn cook_image(width: u32, height: u32, rgba: &[u8]) -> Result<(Descriptor, Vec<u8>), String> {
    let payload = Kind::Image { width, height };
    payload.validate()?;
    if rgba.len() as u64 != payload.bytes() {
        return Err("Cooked image size differs".into());
    }
    let mut bytes = header(3, width, height);
    bytes.extend_from_slice(rgba);
    finish(bytes, payload, vec![], None, None)
}
