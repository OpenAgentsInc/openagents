//! Offline-baked light layers for a static textured scene, and how a zone
//! combines them at run time.
//!
//! An offline bake (`verse-bake --layers`) lights a scene once per light
//! source, so the sources can be mixed at run time instead of baked
//! together at zone load:
//!
//! - **Sky:** the open sky's light after several bounces, as a multiplier of
//!   the open sky's irradiance per vertex, with the open sky fraction.
//! - **Sun:** for each of a few sun directions, the sun's light bounced off
//!   the scene, as a multiplier of the reference sky's irradiance, and the
//!   share of the sun's disk each vertex sees.
//! - **Lamps:** the light of every emissive surface, such as a lamp's
//!   glass or a candle's flame, shadowed and bounced, in lux.
//!
//! The sky and sun layers also come as probe grids for characters. The
//! combination follows the issue's rule, sky × sky intensity + sun(t) +
//! lamps × lamp intensity: [`Layers::lights`] adds the nearest sun's bounce
//! to the sky's multiplier for the light texture, and the shader adds the
//! lamp layer ([`Layers::lamp_texels`]) times the frame's lamp intensity.
//!
//! The file is `VLAY`, the format version and the header's length as
//! little-endian `u32`s, the header as JSON, and the layers deflated. Each
//! layer is stored as planes of one byte channel each, which deflate well.
//! A layer set belongs to exactly one scene: [`scene_digest`] covers every
//! merged vertex, index, material, and image, and a zone uses the layers
//! only when the digest matches the scene it builds. Otherwise it falls back
//! to the load-time bake in [`super::textured_bake`].

use glam::Vec3;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ProbeGrid;
use super::textured::{Merged, TexturedScene};
use super::textured_bake::{AmbientProbes, BakeLight, MAX_AMBIENT, decode, encode};

/// The file's first bytes.
pub const MAGIC: &[u8; 4] = b"VLAY";
/// The file format's version.
pub const FORMAT: u32 = 1;
/// The most bytes a layer file inflates to.
pub const MAX_INFLATED: usize = 1 << 31;
/// The most sun directions a layer file holds.
pub const MAX_SUNS: usize = 16;
/// Lamp light: alpha `a` scales red, green, and blue by
/// `2^((a − 1) / LAMP_STEPS + LAMP_LOW)` lux, so the scale covers
/// `2^LAMP_LOW` to about 940 lux in steps of 4.4 percent.
pub const LAMP_STEPS: f32 = 16.0;
pub const LAMP_LOW: f32 = -6.0;

/// The light the multipliers are relative to: the bake's sun illuminance
/// and its open sky.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Sun illuminance on a surface facing it, lux.
    pub sun: f32,
    /// Open-sky irradiance on surfaces facing up and down, lux.
    pub sky: f32,
    pub ground: f32,
}

impl Reference {
    /// The reference as a bake light toward `sun_dir`.
    #[must_use]
    pub fn light(&self, sun_dir: Vec3) -> BakeLight {
        BakeLight {
            sun_dir: sun_dir.normalize_or(Vec3::Y),
            sun_illuminance: self.sun,
            sky: self.sky,
            ground: self.ground,
        }
    }
}

/// One sun direction's layer.
#[derive(Clone, Debug, PartialEq)]
pub struct SunLayer {
    /// Unit direction toward the sun.
    pub dir: [f32; 3],
    /// Per vertex: the bounced sunlight as a multiplier of the reference
    /// sky's irradiance ([`encode_sun`]), and the visible share of the
    /// sun's disk in alpha.
    pub vertices: Vec<[u8; 4]>,
    /// The bounced sunlight at each probe, lux at the reference sun.
    pub probes: Vec<[f32; 12]>,
}

/// Neighboring baked suns that bracket a direction on their daily arc.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SunBlend {
    pub first: Option<usize>,
    pub second: Option<usize>,
    pub weight: f32,
}

impl SunBlend {
    /// Shader indices are one-based so zero means no baked sun.
    #[must_use]
    pub fn uniform(self, ratio: f32) -> [f32; 4] {
        [
            self.first.map_or(0.0, |i| (i + 1) as f32),
            self.second.map_or(0.0, |i| (i + 1) as f32),
            self.weight,
            ratio,
        ]
    }
}

/// Every layer one offline bake produced for one scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Layers {
    /// The offline bake's key, hexadecimal, for its receipt.
    pub bake_key: String,
    /// [`scene_digest`] of the scene baked, hexadecimal.
    pub scene: String,
    pub reference: Reference,
    /// Per vertex: the sky's multiplier and open fraction, as
    /// [`super::textured::TexturedVertex::light`] holds them.
    pub sky: Vec<[u8; 4]>,
    /// The sky's light at each probe, lux.
    pub sky_probes: Vec<[f32; 12]>,
    pub suns: Vec<SunLayer>,
    /// The vertices that lamps reach, in increasing order, with their
    /// light ([`encode_lamp`]). Every other vertex gets none.
    pub lamps: Vec<(u32, [u8; 4])>,
    /// The probe grid's first probe, spacing, and counts; probes are
    /// stored x fastest.
    pub probe_origin: [f32; 3],
    pub probe_cell: f32,
    pub probe_dims: [u32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Header {
    bake_key: String,
    scene: String,
    reference: Reference,
    vertices: u64,
    lamps: u64,
    suns: Vec<[f32; 3]>,
    probe_origin: [f32; 3],
    probe_cell: f32,
    probe_dims: [u32; 3],
}

/// One byte from a value in `[0, 1]`.
fn byte(x: f32) -> u8 {
    let x = if x.is_finite() { x } else { 0.0 };
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A sun layer's texel: each color byte is `255 × sqrt(b / 4)` of the
/// bounce multiplier `b`, as [`encode`] stores the sky's, and alpha the
/// visible share of the sun.
#[must_use]
pub fn encode_sun(bounce: Vec3, visibility: f32) -> [u8; 4] {
    let m = bounce
        .to_array()
        .map(|c| byte((c.max(0.0) / MAX_AMBIENT).sqrt()));
    [m[0], m[1], m[2], byte(visibility)]
}

/// The bounce multiplier and visible share of the sun a sun texel holds.
#[must_use]
pub fn decode_sun(texel: [u8; 4]) -> (Vec3, f32) {
    let unit = |b: u8| f32::from(b) / 255.0;
    let m = Vec3::new(unit(texel[0]), unit(texel[1]), unit(texel[2]));
    (m * m * MAX_AMBIENT, unit(texel[3]))
}

/// A lamp texel for `lux` per channel: alpha picks a scale of
/// `2^((a − 1) / 16 − 6)` lux at least as large as the brightest channel,
/// and the color bytes are each channel over that scale. Zero alpha, for
/// light under `2^−6` lux, means none.
#[must_use]
pub fn encode_lamp(lux: Vec3) -> [u8; 4] {
    let lux = if lux.is_finite() {
        lux.max(Vec3::ZERO)
    } else {
        Vec3::ZERO
    };
    let peak = lux.max_element();
    if peak < LAMP_LOW.exp2() {
        return [0; 4];
    }
    let a = ((peak.log2() - LAMP_LOW) * LAMP_STEPS).ceil() + 1.0;
    let a = a.clamp(1.0, 255.0);
    let scale = ((a - 1.0) / LAMP_STEPS + LAMP_LOW).exp2();
    let c = (lux / scale).to_array().map(byte);
    [c[0], c[1], c[2], a as u8]
}

/// The lux per channel a lamp texel holds.
#[must_use]
pub fn decode_lamp(texel: [u8; 4]) -> Vec3 {
    if texel[3] == 0 {
        return Vec3::ZERO;
    }
    let scale = ((f32::from(texel[3]) - 1.0) / LAMP_STEPS + LAMP_LOW).exp2();
    Vec3::new(
        f32::from(texel[0]),
        f32::from(texel[1]),
        f32::from(texel[2]),
    ) / 255.0
        * scale
}

/// SHA-256 of everything about a scene that its light depends on: the
/// merged vertices (all but their light), indices, and batches, and the
/// materials and images they use. Two scenes with one digest bake alike.
#[must_use]
pub fn scene_digest(scene: &TexturedScene, merged: &Merged) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"openagents.verse-layers.scene.v1\0");
    hash.update((merged.vertices.len() as u64).to_le_bytes());
    let mut buffer = Vec::with_capacity(1 << 16);
    for chunk in merged.vertices.chunks(1024) {
        buffer.clear();
        for v in chunk {
            for x in v.pos.iter().chain(&v.normal).chain(&v.uv) {
                buffer.extend_from_slice(&x.to_le_bytes());
            }
            buffer.extend_from_slice(&v.color);
        }
        hash.update(&buffer);
    }
    hash.update((merged.indices.len() as u64).to_le_bytes());
    for chunk in merged.indices.chunks(4096) {
        buffer.clear();
        for i in chunk {
            buffer.extend_from_slice(&i.to_le_bytes());
        }
        hash.update(&buffer);
    }
    hash.update((merged.batches.len() as u64).to_le_bytes());
    for b in &merged.batches {
        hash.update((b.material as u64).to_le_bytes());
        hash.update(b.first.to_le_bytes());
        hash.update(b.count.to_le_bytes());
        hash.update(format!("{:?}", b.level).as_bytes());
    }
    hash.update((scene.materials.len() as u64).to_le_bytes());
    for m in &scene.materials {
        hash.update(format!("{m:?}").as_bytes());
    }
    hash.update((scene.images.len() as u64).to_le_bytes());
    for image in &scene.images {
        hash.update(image.width.to_le_bytes());
        hash.update(image.height.to_le_bytes());
        hash.update(&image.rgba);
    }
    hash.finalize().into()
}

/// Lowercase hexadecimal of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn planes(out: &mut Vec<u8>, texels: &[[u8; 4]]) {
    for c in 0..4 {
        out.extend(texels.iter().map(|t| t[c]));
    }
}

fn read_planes(bytes: &[u8], count: usize) -> Vec<[u8; 4]> {
    (0..count)
        .map(|i| std::array::from_fn(|c| bytes[c * count + i]))
        .collect()
}

fn floats(out: &mut Vec<u8>, probes: &[[f32; 12]]) {
    for p in probes {
        for x in p {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
}

fn read_floats(bytes: &[u8]) -> Vec<[f32; 12]> {
    bytes
        .chunks_exact(48)
        .map(|c| {
            std::array::from_fn(|i| {
                f32::from_le_bytes([c[i * 4], c[i * 4 + 1], c[i * 4 + 2], c[i * 4 + 3]])
            })
        })
        .collect()
}

impl Layers {
    /// Interpolates the nearest directions on either side of the sun's arc.
    /// Outside the baked arc, the nearest endpoint holds its light.
    #[must_use]
    pub fn sun_blend(&self, dir: Vec3) -> SunBlend {
        let dir = dir.normalize_or(Vec3::Y);
        let mut blend = SunBlend {
            first: self.nearest_sun(dir),
            ..SunBlend::default()
        };
        let mut shortest = f32::INFINITY;
        for (i, a) in self.suns.iter().enumerate() {
            let a = Vec3::from(a.dir).normalize_or(Vec3::Y);
            for (j, b) in self.suns.iter().enumerate().skip(i + 1) {
                let b = Vec3::from(b.dir).normalize_or(Vec3::Y);
                let span = a.angle_between(b);
                let before = a.angle_between(dir);
                let after = b.angle_between(dir);
                // Select the shortest containing arc, so unequal spacing
                // never switches the other layer before an exact baked sun.
                if span > 1e-5 && span < shortest && before + after <= span + 1e-4 {
                    shortest = span;
                    blend = SunBlend {
                        first: Some(i),
                        second: Some(j),
                        weight: (before / span).clamp(0.0, 1.0),
                    };
                }
            }
        }
        blend
    }

    /// Interpolated character probes under the reference sky.
    #[must_use]
    pub fn blended_probes(&self, blend: SunBlend, ratio: f32) -> AmbientProbes {
        let mut probes = self.probes(None, 0.0);
        for (index, weight) in [
            (blend.first, 1.0 - blend.weight),
            (blend.second, blend.weight),
        ] {
            if let Some(sun) = index.and_then(|i| self.suns.get(i)) {
                for (p, s) in probes.grid.data.iter_mut().zip(&sun.probes) {
                    for (value, bounce) in p.iter_mut().zip(s) {
                        *value += bounce * weight * ratio;
                    }
                }
            }
        }
        let mut version = probes.grid.version;
        for byte in blend
            .first
            .map_or(u64::MAX, |i| i as u64)
            .to_le_bytes()
            .into_iter()
            .chain(blend.second.map_or(u64::MAX, |i| i as u64).to_le_bytes())
            .chain(blend.weight.to_bits().to_le_bytes())
            .chain(ratio.to_bits().to_le_bytes())
        {
            version ^= u64::from(byte);
            version = version.wrapping_mul(0x0100_0000_01b3);
        }
        probes.grid.version = (version >> 1) | 1;
        probes
    }
    /// Vertices in each layer.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.sky.len()
    }

    fn probe_count(dims: [u32; 3]) -> usize {
        dims.iter().map(|&d| d as usize).product()
    }

    /// The file's bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let n = self.sky.len();
        let mut body = Vec::new();
        planes(&mut body, &self.sky);
        for sun in &self.suns {
            planes(&mut body, &sun.vertices);
        }
        let mut previous = 0u32;
        for &(i, _) in &self.lamps {
            body.extend_from_slice(&(i - previous).to_le_bytes());
            previous = i;
        }
        let lamps: Vec<[u8; 4]> = self.lamps.iter().map(|&(_, t)| t).collect();
        planes(&mut body, &lamps);
        floats(&mut body, &self.sky_probes);
        for sun in &self.suns {
            floats(&mut body, &sun.probes);
        }
        let header = Header {
            bake_key: self.bake_key.clone(),
            scene: self.scene.clone(),
            reference: self.reference,
            vertices: n as u64,
            lamps: self.lamps.len() as u64,
            suns: self.suns.iter().map(|s| s.dir).collect(),
            probe_origin: self.probe_origin,
            probe_cell: self.probe_cell,
            probe_dims: self.probe_dims,
        };
        let json = serde_json::to_vec(&header).unwrap_or_default();
        let stored = miniz_oxide::deflate::compress_to_vec(&body, 6);
        let mut out = Vec::with_capacity(12 + json.len() + stored.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT.to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&stored);
        out
    }

    /// Reads a layer file.
    ///
    /// # Errors
    ///
    /// Returns a message when the bytes are not a layer file of this
    /// format, are larger than the bounds allow, or do not inflate to the
    /// layers their header names.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let word = |at: usize| -> Result<u32, String> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| "light layers: truncated header".to_string())
        };
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err("light layers: not a VLAY file".into());
        }
        let format = word(4)?;
        if format != FORMAT {
            return Err(format!("light layers: format {format}, expected {FORMAT}"));
        }
        let length = word(8)? as usize;
        let json = bytes
            .get(12..12 + length)
            .ok_or("light layers: truncated header")?;
        let header: Header =
            serde_json::from_slice(json).map_err(|e| format!("light layers: {e}"))?;
        if header.suns.len() > MAX_SUNS {
            return Err("light layers: too many suns".into());
        }
        let n = usize::try_from(header.vertices).map_err(|e| e.to_string())?;
        let m = usize::try_from(header.lamps).map_err(|e| e.to_string())?;
        let probes = Self::probe_count(header.probe_dims);
        let suns = header.suns.len();
        let expected = n
            .checked_mul(4 * (1 + suns))
            .and_then(|v| v.checked_add(m.checked_mul(8)?))
            .and_then(|v| v.checked_add(probes.checked_mul(48 * (1 + suns))?))
            .filter(|&v| v <= MAX_INFLATED)
            .ok_or("light layers: larger than the bounds allow")?;
        let body =
            miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes[12 + length..], expected)
                .map_err(|e| format!("light layers: {e:?}"))?;
        if body.len() != expected {
            return Err("light layers: the body does not match its header".into());
        }
        let mut at = 0;
        let mut take = |size: usize| {
            let part = &body[at..at + size];
            at += size;
            part
        };
        let sky = read_planes(take(4 * n), n);
        let sun_vertices: Vec<Vec<[u8; 4]>> =
            (0..suns).map(|_| read_planes(take(4 * n), n)).collect();
        let mut index = 0u32;
        let mut indices = Vec::with_capacity(m);
        for chunk in take(4 * m).chunks_exact(4) {
            index = index
                .checked_add(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .ok_or("light layers: a lamp index overflows")?;
            if index as usize >= n {
                return Err("light layers: a lamp names a vertex past the end".into());
            }
            indices.push(index);
        }
        let lamp_texels = read_planes(take(4 * m), m);
        let sky_probes = read_floats(take(48 * probes));
        let sun_probes: Vec<Vec<[f32; 12]>> =
            (0..suns).map(|_| read_floats(take(48 * probes))).collect();
        Ok(Self {
            bake_key: header.bake_key,
            scene: header.scene,
            reference: header.reference,
            sky,
            sky_probes,
            suns: header
                .suns
                .into_iter()
                .zip(sun_vertices)
                .zip(sun_probes)
                .map(|((dir, vertices), probes)| SunLayer {
                    dir,
                    vertices,
                    probes,
                })
                .collect(),
            lamps: indices.into_iter().zip(lamp_texels).collect(),
            probe_origin: header.probe_origin,
            probe_cell: header.probe_cell,
            probe_dims: header.probe_dims,
        })
    }

    /// The baked sun direction nearest `dir`, or `None` without suns.
    #[must_use]
    pub fn nearest_sun(&self, dir: Vec3) -> Option<usize> {
        let dir = dir.normalize_or(Vec3::Y);
        self.suns
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                Vec3::from(a.dir)
                    .dot(dir)
                    .total_cmp(&Vec3::from(b.dir).dot(dir))
            })
            .map(|(k, _)| k)
    }

    /// How strongly the sun lights the scene relative to the sky, as a
    /// share of the reference's: `(sun / sky) / (reference sun / reference
    /// sky)`, for sun and sky lux at the time of day.
    #[must_use]
    pub fn sun_ratio(&self, sun_lux: f32, sky_lux: f32) -> f32 {
        let reference = self.reference.sun / self.reference.sky.max(1e-3);
        let now = sun_lux.max(0.0) / sky_lux.max(1e-3);
        let ratio = now / reference.max(1e-6);
        if ratio.is_finite() {
            ratio.clamp(0.0, 16.0)
        } else {
            0.0
        }
    }

    /// The light texture's texels: the sky's multiplier plus sun `sun`'s
    /// bounce times `ratio` ([`Self::sun_ratio`]), with the sky's open
    /// fraction.
    #[must_use]
    pub fn lights(&self, sun: Option<usize>, ratio: f32) -> Vec<[u8; 4]> {
        let table: [f32; 256] = std::array::from_fn(|b| {
            let u = b as f32 / 255.0;
            u * u * MAX_AMBIENT
        });
        let sun = sun.and_then(|k| self.suns.get(k));
        self.sky
            .iter()
            .enumerate()
            .map(|(i, &sky)| {
                if sky[3] == 0 {
                    return sky;
                }
                let (mut m, open) = decode(sky);
                if let Some(t) = sun.and_then(|s| s.vertices.get(i)) {
                    m += Vec3::new(
                        table[t[0] as usize],
                        table[t[1] as usize],
                        table[t[2] as usize],
                    ) * ratio;
                }
                encode(m, open)
            })
            .collect()
    }

    /// The lamp texture's texels, one a vertex: zero where no lamp reaches.
    #[must_use]
    pub fn lamp_texels(&self) -> Vec<[u8; 4]> {
        let mut out = vec![[0u8; 4]; self.sky.len()];
        for &(i, t) in &self.lamps {
            if let Some(slot) = out.get_mut(i as usize) {
                *slot = t;
            }
        }
        out
    }

    /// The probes characters sample: the sky's plus sun `sun`'s times
    /// `ratio`, against the reference light.
    #[must_use]
    pub fn probes(&self, sun: Option<usize>, ratio: f32) -> AmbientProbes {
        let sun_layer = sun.and_then(|k| self.suns.get(k));
        let data = self
            .sky_probes
            .iter()
            .enumerate()
            .map(|(i, sky)| {
                let mut p = *sky;
                if let Some(s) = sun_layer.and_then(|s| s.probes.get(i)) {
                    for (x, y) in p.iter_mut().zip(s) {
                        *x += y * ratio;
                    }
                }
                p
            })
            .collect();
        // Never zero and never tagged as a studio key's grid; changes with
        // the sun and its strength.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for b in self
            .scene
            .bytes()
            .chain(sun.map_or(u64::MAX, |k| k as u64).to_le_bytes())
            .chain(ratio.to_bits().to_le_bytes())
        {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        let dir = sun_layer.map_or(Vec3::Y, |s| Vec3::from(s.dir));
        AmbientProbes {
            grid: ProbeGrid {
                origin: Vec3::from(self.probe_origin),
                cell: self.probe_cell,
                dims: self.probe_dims,
                data,
                version: (hash >> 1) | 1,
            },
            light: self.reference.light(dir),
        }
    }

    /// Checks that every layer has one entry a vertex or probe.
    ///
    /// # Errors
    ///
    /// Returns which layer is the wrong size.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.sky.len();
        let probes = Self::probe_count(self.probe_dims);
        if self.suns.len() > MAX_SUNS {
            return Err("light layers: too many suns".into());
        }
        let bytes = n
            .checked_mul(4 * (1 + self.suns.len()))
            .and_then(|v| v.checked_add(self.lamps.len().checked_mul(8)?))
            .and_then(|v| v.checked_add(probes.checked_mul(48 * (1 + self.suns.len()))?));
        if bytes.is_none_or(|bytes| bytes > MAX_INFLATED) {
            return Err("light layers: larger than the bounds allow".into());
        }
        if self.sky_probes.len() != probes {
            return Err("light layers: the sky's probes are the wrong size".into());
        }
        for (k, sun) in self.suns.iter().enumerate() {
            if sun.vertices.len() != n || sun.probes.len() != probes {
                return Err(format!("light layers: sun {k} is the wrong size"));
            }
        }
        if self.lamps.windows(2).any(|w| w[0].0 >= w[1].0)
            || self.lamps.last().is_some_and(|l| l.0 as usize >= n)
        {
            return Err("light layers: lamp vertices out of order".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_layers_interpolate_through_the_midpoint_and_exact_directions() {
        let layers = sample();
        let at = layers.sun_blend(Vec3::Y);
        assert_eq!(at.first, Some(0));
        assert_eq!(at.weight, 0.0);
        let middle = layers.sun_blend((Vec3::X + Vec3::Y).normalize());
        assert!((middle.weight - 0.5).abs() < 1e-5);
        let probes = layers.blended_probes(middle, 1.0);
        assert!((probes.grid.data[0][0] - 2.0).abs() < 1e-4);
        let east = layers.blended_probes(layers.sun_blend(Vec3::X), 1.0);
        let high = layers.blended_probes(layers.sun_blend(Vec3::Y), 1.0);
        assert_ne!(east.grid.data, high.grid.data);
        assert_ne!(east.grid.version, high.grid.version);
        for angle in [0.784_f32, 0.786] {
            let blend = layers.sun_blend(Vec3::new(angle.sin(), angle.cos(), 0.0));
            let irradiance = layers.blended_probes(blend, 1.0).grid.data[0][0];
            assert!((irradiance - 2.0).abs() < 0.01);
        }
        let mut single = layers.clone();
        single.suns.truncate(1);
        assert_eq!(single.sun_blend(Vec3::X).weight, 0.0);
        single.suns.clear();
        assert_eq!(single.sun_blend(Vec3::Y).uniform(1.0), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn in_memory_layers_share_the_decoders_sun_bound() {
        let mut layers = sample();
        layers.suns.resize(MAX_SUNS, layers.suns[0].clone());
        assert!(layers.validate().is_ok());
        layers.suns.push(layers.suns[0].clone());
        assert!(layers.validate().unwrap_err().contains("too many suns"));
    }

    fn sample() -> Layers {
        let n = 50;
        Layers {
            bake_key: "key".into(),
            scene: "scene".into(),
            reference: Reference {
                sun: 4_000.0,
                sky: 1_200.0,
                ground: 450.0,
            },
            sky: (0..n)
                .map(|i| encode(Vec3::splat(0.2 + i as f32 * 0.01), 0.5))
                .collect(),
            sky_probes: vec![[1.0; 12]; 8],
            suns: vec![
                SunLayer {
                    dir: [0.0, 1.0, 0.0],
                    vertices: (0..n)
                        .map(|i| encode_sun(Vec3::splat(i as f32 * 0.02), 1.0))
                        .collect(),
                    probes: vec![[2.0; 12]; 8],
                },
                SunLayer {
                    dir: [1.0, 0.0, 0.0],
                    vertices: vec![[0; 4]; n],
                    probes: vec![[0.0; 12]; 8],
                },
            ],
            lamps: vec![
                (3, encode_lamp(Vec3::new(10.0, 5.0, 1.0))),
                (40, [9, 9, 9, 9]),
            ],
            probe_origin: [0.0; 3],
            probe_cell: 2.0,
            probe_dims: [2, 2, 2],
        }
    }

    #[test]
    fn layers_round_trip_through_their_file() {
        let layers = sample();
        layers.validate().unwrap();
        let bytes = layers.encode();
        assert_eq!(Layers::decode(&bytes).unwrap(), layers);
        // A truncated body or a foreign file is refused.
        assert!(Layers::decode(&bytes[..bytes.len() - 4]).is_err());
        assert!(Layers::decode(b"VBAK\x01\0\0\0").is_err());
    }

    #[test]
    fn lamp_texels_keep_lux_within_their_step() {
        for lux in [0.02, 0.5, 3.0, 47.0, 600.0] {
            let rgb = Vec3::new(lux, lux * 0.6, lux * 0.2);
            let back = decode_lamp(encode_lamp(rgb));
            assert!(
                (back.x - rgb.x).abs() <= rgb.x * 0.05 + 1e-3,
                "{lux}: {back}"
            );
            assert!(
                (back.y - rgb.y).abs() <= rgb.x * 0.05 + 1e-3,
                "{lux}: {back}"
            );
        }
        assert_eq!(encode_lamp(Vec3::splat(1e-4)), [0; 4]);
        assert_eq!(decode_lamp([0; 4]), Vec3::ZERO);
    }

    #[test]
    fn the_nearest_sun_adds_its_bounce_in_proportion_to_its_strength() {
        let layers = sample();
        assert_eq!(layers.nearest_sun(Vec3::new(0.1, 0.9, 0.0)), Some(0));
        assert_eq!(layers.nearest_sun(Vec3::new(0.9, 0.1, 0.0)), Some(1));
        assert!((layers.sun_ratio(4_000.0, 1_200.0) - 1.0).abs() < 1e-5);
        assert!((layers.sun_ratio(2_000.0, 1_200.0) - 0.5).abs() < 1e-5);
        let sky_only = layers.lights(None, 1.0);
        assert_eq!(sky_only, layers.sky);
        let lit = layers.lights(Some(0), 1.0);
        let (sky, _) = decode(layers.sky[30]);
        let (both, open) = decode(lit[30]);
        let (bounce, _) = decode_sun(layers.suns[0].vertices[30]);
        assert!(
            (both - sky - bounce).abs().max_element() < 0.03,
            "{both} {sky} {bounce}"
        );
        assert!((open - 0.5).abs() < 0.01);
        let probes = layers.probes(Some(0), 0.5);
        assert_eq!(probes.grid.data[0][0], 2.0);
        assert_ne!(
            probes.grid.version,
            layers.probes(Some(1), 0.5).grid.version
        );
        let lamps = layers.lamp_texels();
        assert_eq!(lamps.len(), 50);
        assert_eq!(lamps[40], [9, 9, 9, 9]);
        assert_eq!(lamps[0], [0; 4]);
    }
}
