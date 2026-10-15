//! A strand groom's levels of detail and the hair around it, for a
//! character imported with strand-ribbon hair (a MetaHuman export's groom,
//! `scripts/blender/metahuman_import.py`).
//!
//! The groom is one hairstyle at every distance: [`split_strands`] takes
//! it out of the character into its own model and tints the hair cards
//! under it to its colour; [`thinned`] cuts coarser levels from it (one
//! clump in `k`, widened `k` times, so coverage, outline and colour hold);
//! [`strand_levels`] says how much of each level to draw at a distance, so
//! neighbouring levels dissolve into each other
//! (`Renderer::set_hair_fade`) instead of switching.
use glam::Vec3;
use std::path::Path;
use verse_engine::assets::{Model, Pack};
use verse_engine::material::Shading;

/// Her frame: metres, Y up, feet at the origin (the importer's model
/// space through `verse_content::basis`).
fn to_body(p: [f32; 3]) -> Vec3 {
    verse_content::basis().transform_point3(Vec3::from(p))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(p), b.max(p)),
    )
}

/// The eyeballs' mean height in her frame: the small eye-shaded surfaces.
fn eye_height(model: &Model) -> Option<f32> {
    let mut ys = Vec::new();
    for s in &model.surfaces {
        if s.material.shading != Shading::Eye as u8 || s.vertices.is_empty() {
            continue;
        }
        let (min, max) = bounds(s.vertices.iter().map(|v| to_body(v.position)));
        if max.x - min.x <= 0.045 {
            ys.push((min.y + max.y) * 0.5);
        }
    }
    (ys.len() >= 2).then(|| ys.iter().sum::<f32>() / ys.len() as f32)
}

/// Splits her strand groom (every hair-shaded surface on the strand
/// texture: the body, the flyaways, the face-framing locks) into its own
/// model, which fades with distance, and makes the rest of her hair agree
/// with it:
///
/// - the hair cards are tinted so their mean color (by coverage) is the
///   strands', so the hairstyle keeps its color when the strands dissolve;
/// - the brows and lashes are lightened and thinned a little: dark,
///   full-opacity cards read heavy under candlelight.
pub fn split_strands(
    her: &mut Model,
    pack: &Pack,
    dir: &Path,
    grade: f32,
) -> Result<Model, String> {
    let hair = Shading::Hair as u8;
    let strand_texture = her
        .surfaces
        .iter()
        .filter(|s| s.material.shading == hair)
        .max_by_key(|s| s.vertices.len())
        .map(|s| s.texture)
        .ok_or("The model has no hair")?;
    let mut strands = her.clone();
    strands
        .surfaces
        .retain(|s| s.material.shading == hair && s.texture == strand_texture);
    for s in &mut strands.surfaces {
        s.tint = (Vec3::from(s.tint) * grade).to_array();
    }
    her.surfaces
        .retain(|s| !(s.material.shading == hair && s.texture == strand_texture));
    let strand_color = mean_color(pack, dir, strand_texture)?;
    let eyes_y = eye_height(her).ok_or("The model has no eyeballs (eye-shaded surfaces)")?;
    for s in &mut her.surfaces {
        if s.material.shading != hair || s.vertices.is_empty() {
            continue;
        }
        let (min, max) = bounds(s.vertices.iter().map(|v| to_body(v.position)));
        if max.y - min.y > 0.15 {
            // The cards: the strands' color, card for strand.
            let card = mean_color(pack, dir, s.texture)?;
            let ratio = (strand_color / card.max(Vec3::splat(1e-3)))
                .clamp(Vec3::splat(0.5), Vec3::splat(1.6));
            s.tint = (Vec3::from(s.tint) * ratio * grade).to_array();
        } else if s.texture == 0 {
            // The painted brow (a solid brow-shaped shell, no map): a soft
            // fair tone under the brow hairs, not a dark block.
            s.tint = (Vec3::from(s.tint) * 1.4).min(Vec3::ONE).to_array();
            s.material.opacity *= 0.35;
        } else {
            // Brow hairs and lashes: their cards' own hard cutout (alpha to
            // coverage turns a lash card's soft alpha into a smudge), the
            // brows a lighter brown.
            s.material.shading = Shading::Standard as u8;
            if (min.y + max.y) * 0.5 > eyes_y + 0.012 {
                s.tint = (Vec3::from(s.tint) * 1.5).min(Vec3::ONE).to_array();
            }
        }
    }
    Ok(strands)
}

/// A level of detail of the strand groom: one clump in `k` (a hashed
/// choice, so no pattern shows), each widened `k` times about its centre
/// line, so the hair's coverage stays what it was. The groom is ribbons:
/// vertices in left/right pairs, quads joining each pair to the next along
/// a clump. Surfaces of hair finer than 0.2 mm (the flyaways) are left out.
pub fn thinned(strands: &Model, k: usize) -> Result<Model, String> {
    let mut out = strands.clone();
    out.surfaces.clear();
    for s in &strands.surfaces {
        let v = &s.vertices;
        if v.len() % 2 != 0 || v.is_empty() {
            return Err("The strand groom is not left/right ribbon pairs".into());
        }
        let pairs = v.len() / 2;
        let width =
            |p: usize| Vec3::from(v[2 * p].position).distance(Vec3::from(v[2 * p + 1].position));
        let mut widths: Vec<f32> = (0..pairs).step_by(97).map(width).collect();
        widths.sort_by(f32::total_cmp);
        // Model units are yards (`basis`); 0.2 mm.
        if widths[widths.len() / 2] < 0.0002 / 0.9144 {
            continue;
        }
        let mut link = vec![false; pairs];
        let mut odd = 0usize;
        for t in s.indices.chunks_exact(3) {
            let ps = [t[0] as usize / 2, t[1] as usize / 2, t[2] as usize / 2];
            let lo = *ps.iter().min().unwrap_or(&0);
            let hi = *ps.iter().max().unwrap_or(&0);
            if hi == lo + 1 {
                link[lo] = true;
            } else if hi != lo {
                odd += 1;
            }
        }
        if odd * 20 > s.indices.len() / 3 {
            return Err("The strand groom's triangles do not join ribbon pairs".into());
        }
        let mut keep = vec![false; pairs];
        let mut strand = 0u32;
        for p in 0..pairs {
            if p > 0 && !link[p - 1] {
                strand += 1;
            }
            let hash = strand.wrapping_mul(2_654_435_761) >> 7;
            keep[p] = hash as usize % k == 0;
        }
        let mut map = vec![u32::MAX; pairs];
        let mut vertices = Vec::new();
        for p in 0..pairs {
            if !keep[p] {
                continue;
            }
            map[p] = (vertices.len() / 2) as u32;
            let (l, r) = (
                Vec3::from(v[2 * p].position),
                Vec3::from(v[2 * p + 1].position),
            );
            let (c, half) = ((l + r) * 0.5, (r - l) * 0.5 * k as f32);
            let mut a = v[2 * p].clone();
            let mut b = v[2 * p + 1].clone();
            a.position = (c - half).to_array();
            b.position = (c + half).to_array();
            vertices.push(a);
            vertices.push(b);
        }
        let mut indices = Vec::new();
        for t in s.indices.chunks_exact(3) {
            if t.iter().all(|&i| keep[i as usize / 2]) {
                indices.extend(t.iter().map(|&i| map[i as usize / 2] * 2 + i % 2));
            }
        }
        let mut surface = s.clone();
        surface.vertices = vertices;
        surface.indices = indices;
        out.surfaces.push(surface);
    }
    Ok(out)
}

/// How much of each strand level to draw `distance` metres from her face:
/// weights that sum to one, crossing over in the `BANDS`.
pub fn strand_levels(distance: f32, bands: [(f32, f32); 2]) -> [f32; 3] {
    let a = smooth(bands[0].0, bands[0].1, distance);
    let b = smooth(bands[1].0, bands[1].1, distance);
    [1.0 - a, a * (1.0 - b), b]
}

/// A texture's mean linear color, weighted by its alpha.
pub fn mean_color(pack: &Pack, dir: &Path, texture: usize) -> Result<Vec3, String> {
    let file = &pack.textures.get(texture).ok_or("No such texture")?.file;
    let path = dir.join(file);
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    ));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let channels = info.color_type.samples();
    if info.bit_depth != png::BitDepth::Eight || channels < 3 {
        return Err(format!("{}: expected 8-bit RGB(A)", path.display()));
    }
    let linear = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut sum = Vec3::ZERO;
    let mut weight = 0.0;
    for px in buf[..info.buffer_size()].chunks_exact(channels).step_by(7) {
        let a = if channels == 4 {
            f32::from(px[3]) / 255.0
        } else {
            1.0
        };
        sum += Vec3::new(linear(px[0]), linear(px[1]), linear(px[2])) * a;
        weight += a;
    }
    Ok(sum / weight.max(1e-6))
}
