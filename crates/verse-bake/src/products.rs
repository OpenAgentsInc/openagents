//! Bake products and their file: a versioned header naming each layer,
//! followed by the layers' little-endian values.
//!
//! The file is `VBAK`, the format version and the header's length as
//! little-endian `u32`s, the header as JSON, zero padding to four bytes, and
//! then each layer's bytes at the offset the header gives. A reader keeps
//! layers it does not know in [`Products::extra`], so later bakes can add
//! layers, such as lightmaps, without breaking earlier readers.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::scene::hex;

/// The file's first bytes.
pub const MAGIC: &[u8; 4] = b"VBAK";
/// The file format's version.
pub const FORMAT: u32 = 1;

/// The probe grid a bake produced.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeLayer {
    /// The first probe's position, m.
    pub origin: [f32; 3],
    /// Spacing between probes, m.
    pub cell: f32,
    /// Probe counts along each axis; probes are stored x fastest.
    pub dims: [u32; 3],
    /// Irradiance in lux as order-one spherical harmonics, four
    /// coefficients for each of red, green, and blue.
    pub data: Vec<[f32; 12]>,
}

/// A layer this reader does not know, kept as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct RawLayer {
    pub entry: LayerEntry,
    pub bytes: Vec<u8>,
}

/// Everything one bake produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Products {
    /// The [`crate::bake_key`] the bake answered, hexadecimal.
    pub bake_key: String,
    /// The scene digest, hexadecimal.
    pub scene: String,
    /// Each vertex's light channel, as `TexturedVertex::light` holds it.
    pub vertex_light: Vec<[u8; 4]>,
    /// Each vertex's multiplier of the open sky's irradiance, per channel.
    pub vertex_ambient: Vec<[f32; 3]>,
    /// Each vertex's cosine-weighted open sky fraction.
    pub vertex_open: Vec<f32>,
    /// The sun directions of [`Self::vertex_sun`].
    pub suns: Vec<[f32; 3]>,
    /// For each sun direction, each vertex's visible share of the sun.
    pub vertex_sun: Vec<Vec<f32>>,
    pub probes: ProbeLayer,
    /// Layers a later format added.
    pub extra: Vec<RawLayer>,
}

/// One layer in the header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerEntry {
    pub name: String,
    /// `f32` or `u8`.
    pub format: String,
    /// Values per element.
    pub components: u32,
    /// Elements.
    pub count: u64,
    /// Where the layer's bytes start, from the end of the padded header.
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Header {
    bake_key: String,
    scene: String,
    suns: Vec<[f32; 3]>,
    probe_origin: [f32; 3],
    probe_cell: f32,
    probe_dims: [u32; 3],
    layers: Vec<LayerEntry>,
}

const LIGHT: &str = "vertex.light";
const AMBIENT: &str = "vertex.ambient";
const OPEN: &str = "vertex.open";
const SUN: &str = "vertex.sun.";
const PROBES: &str = "probe.sh";

fn floats(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn read_floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

impl Products {
    /// The file's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut layers: Vec<(LayerEntry, Vec<u8>)> = Vec::new();
        let mut add = |name: String, format: &str, components: u32, count: usize, bytes| {
            layers.push((
                LayerEntry {
                    name,
                    format: format.into(),
                    components,
                    count: count as u64,
                    offset: 0,
                },
                bytes,
            ));
        };
        add(
            LIGHT.into(),
            "u8",
            4,
            self.vertex_light.len(),
            self.vertex_light.iter().flatten().copied().collect(),
        );
        add(
            AMBIENT.into(),
            "f32",
            3,
            self.vertex_ambient.len(),
            floats(self.vertex_ambient.iter().flatten().copied()),
        );
        add(
            OPEN.into(),
            "f32",
            1,
            self.vertex_open.len(),
            floats(self.vertex_open.iter().copied()),
        );
        for (k, sun) in self.vertex_sun.iter().enumerate() {
            add(
                format!("{SUN}{k}"),
                "f32",
                1,
                sun.len(),
                floats(sun.iter().copied()),
            );
        }
        add(
            PROBES.into(),
            "f32",
            12,
            self.probes.data.len(),
            floats(self.probes.data.iter().flatten().copied()),
        );
        for raw in &self.extra {
            layers.push((raw.entry.clone(), raw.bytes.clone()));
        }
        let mut offset = 0u64;
        for (entry, bytes) in &mut layers {
            entry.offset = offset;
            offset += (bytes.len() as u64).next_multiple_of(4);
        }
        let header = Header {
            bake_key: self.bake_key.clone(),
            scene: self.scene.clone(),
            suns: self.suns.clone(),
            probe_origin: self.probes.origin,
            probe_cell: self.probes.cell,
            probe_dims: self.probes.dims,
            layers: layers.iter().map(|(e, _)| e.clone()).collect(),
        };
        let json = serde_json::to_vec(&header).unwrap_or_default();
        let mut out = Vec::with_capacity(16 + json.len() + offset as usize);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT.to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        out.resize(out.len().next_multiple_of(4), 0);
        for (_, bytes) in &layers {
            out.extend_from_slice(bytes);
            out.resize(out.len().next_multiple_of(4), 0);
        }
        out
    }

    /// SHA-256 of [`Self::encode`], hexadecimal.
    #[must_use]
    pub fn digest(&self) -> String {
        hex(&Sha256::digest(self.encode()))
    }

    /// Reads a products file.
    ///
    /// # Errors
    ///
    /// Returns a message when the bytes are not a products file of a known
    /// format, or a layer runs past the end.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let word = |at: usize| -> Result<u32, String> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| "bake products: truncated header".to_string())
        };
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err("bake products: not a VBAK file".into());
        }
        let format = word(4)?;
        if format != FORMAT {
            return Err(format!("bake products: format {format}, expected {FORMAT}"));
        }
        let length = word(8)? as usize;
        let json = bytes
            .get(12..12 + length)
            .ok_or("bake products: truncated header")?;
        let header: Header =
            serde_json::from_slice(json).map_err(|e| format!("bake products: {e}"))?;
        let base = (12 + length).next_multiple_of(4);
        let mut products = Self {
            bake_key: header.bake_key,
            scene: header.scene,
            vertex_light: Vec::new(),
            vertex_ambient: Vec::new(),
            vertex_open: Vec::new(),
            suns: header.suns.clone(),
            vertex_sun: vec![Vec::new(); header.suns.len()],
            probes: ProbeLayer {
                origin: header.probe_origin,
                cell: header.probe_cell,
                dims: header.probe_dims,
                data: Vec::new(),
            },
            extra: Vec::new(),
        };
        for entry in header.layers {
            let width = match entry.format.as_str() {
                "u8" => 1,
                "f32" => 4,
                other => return Err(format!("bake products: layer format {other}")),
            };
            let size = entry
                .count
                .checked_mul(u64::from(entry.components) * width)
                .and_then(|s| usize::try_from(s).ok())
                .ok_or("bake products: layer too large")?;
            let start = base + usize::try_from(entry.offset).map_err(|e| e.to_string())?;
            let data = start
                .checked_add(size)
                .and_then(|end| bytes.get(start..end))
                .ok_or_else(|| format!("bake products: layer {} runs past the end", entry.name))?;
            let name = entry.name.as_str();
            match (name, entry.components) {
                (LIGHT, 4) => {
                    products.vertex_light = data
                        .chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect();
                }
                (AMBIENT, 3) => {
                    products.vertex_ambient = read_floats(data)
                        .chunks_exact(3)
                        .map(|c| [c[0], c[1], c[2]])
                        .collect();
                }
                (OPEN, 1) => products.vertex_open = read_floats(data),
                (PROBES, 12) => {
                    products.probes.data = read_floats(data)
                        .chunks_exact(12)
                        .map(|c| std::array::from_fn(|i| c[i]))
                        .collect();
                }
                _ => match name
                    .strip_prefix(SUN)
                    .and_then(|k| k.parse::<usize>().ok())
                    .filter(|_| entry.components == 1)
                    .and_then(|k| products.vertex_sun.get_mut(k))
                {
                    Some(slot) => *slot = read_floats(data),
                    None => products.extra.push(RawLayer {
                        bytes: data.to_vec(),
                        entry,
                    }),
                },
            }
        }
        Ok(products)
    }
}

/// How far two bakes of one scene lie apart, value by value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    /// Mean absolute difference.
    pub mean: f32,
    /// The absolute difference that 99 percent of values stay within.
    pub p99: f32,
    /// The largest absolute difference.
    pub max: f32,
}

impl Spread {
    fn of(mut differences: Vec<f32>) -> Self {
        if differences.is_empty() {
            return Self::default();
        }
        differences.sort_by(f32::total_cmp);
        let n = differences.len();
        Self {
            mean: differences.iter().sum::<f32>() / n as f32,
            p99: differences[((n - 1) as f32 * 0.99).round() as usize],
            max: differences[n - 1],
        }
    }

    /// Whether the spread stays within a tolerance.
    #[must_use]
    pub fn within(&self, tolerance: &Spread) -> bool {
        self.mean <= tolerance.mean && self.p99 <= tolerance.p99 && self.max <= tolerance.max
    }
}

/// How two bakes of one scene compare, layer by layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Agreement {
    /// Ambient multipliers, per channel.
    pub ambient: Spread,
    /// Open sky fractions.
    pub open: Spread,
    /// Sun visibility, every direction together.
    pub sun: Spread,
    /// Probe coefficients, relative to the larger probe's order-zero term.
    pub probes: Spread,
}

impl Products {
    /// How `self` and `other`, bakes of the same scene and settings,
    /// compare.
    ///
    /// # Errors
    ///
    /// Returns a message when their layers differ in size.
    pub fn compare(&self, other: &Self) -> Result<Agreement, String> {
        if self.vertex_ambient.len() != other.vertex_ambient.len()
            || self.vertex_sun.len() != other.vertex_sun.len()
            || self.probes.data.len() != other.probes.data.len()
        {
            return Err("the bakes are of different scenes".into());
        }
        let ambient = self
            .vertex_ambient
            .iter()
            .zip(&other.vertex_ambient)
            .flat_map(|(a, b)| (0..3).map(move |c| (a[c] - b[c]).abs()))
            .collect();
        let open = self
            .vertex_open
            .iter()
            .zip(&other.vertex_open)
            .map(|(a, b)| (a - b).abs())
            .collect();
        let sun = self
            .vertex_sun
            .iter()
            .zip(&other.vertex_sun)
            .flat_map(|(a, b)| a.iter().zip(b).map(|(x, y)| (x - y).abs()))
            .collect();
        let probes = self
            .probes
            .data
            .iter()
            .zip(&other.probes.data)
            .flat_map(|(a, b)| {
                let scale = [0, 4, 8]
                    .iter()
                    .map(|&c| a[c].abs().max(b[c].abs()))
                    .fold(1e-6f32, f32::max);
                (0..12).map(move |i| (a[i] - b[i]).abs() / scale)
            })
            .collect();
        Ok(Agreement {
            ambient: Spread::of(ambient),
            open: Spread::of(open),
            sun: Spread::of(sun),
            probes: Spread::of(probes),
        })
    }
}
