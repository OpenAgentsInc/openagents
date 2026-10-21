//! A mesh chunk: a few named triangle lists, packed small.
//!
//! The game's meshes are plain triangle lists of 13 floats a vertex
//! (`icecream_web::mesh::STRIDE`): position, flat normal, hull direction,
//! colour, and outline weight. A chunk keeps the position as three `f32`
//! and packs the rest into ten bytes, 22 bytes a vertex instead of 52. The
//! colours are exact (they were bytes to begin with); normals and hull
//! directions keep about a degree.
//!
//! ```text
//! "ICZ1"  u32 parts
//! per part: u8 name length, name, u32 vertices, then each vertex:
//!   f32 x, y, z   i8 normal x3   i8 hull x3   u8 colour x3   u8 outline
//! ```
//!
//! All numbers are little-endian.

/// Floats per decoded vertex.
pub const STRIDE: usize = 13;
/// Bytes per packed vertex.
pub const PACKED: usize = 22;
const MAGIC: &[u8; 4] = b"ICZ1";

/// One named triangle list, in the game's 13-float layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub data: Vec<f32>,
}

fn snorm(x: f32) -> u8 {
    ((x.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8
}

fn unorm(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Packs `parts` into a chunk.
#[must_use]
pub fn encode(parts: &[Part]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(parts.len() as u32).to_le_bytes());
    for part in parts {
        let name = part.name.as_bytes();
        out.push(name.len().min(255) as u8);
        out.extend_from_slice(&name[..name.len().min(255)]);
        let vertices = part.data.len() / STRIDE;
        out.extend_from_slice(&(vertices as u32).to_le_bytes());
        for v in part.data.chunks_exact(STRIDE) {
            for x in &v[0..3] {
                out.extend_from_slice(&x.to_le_bytes());
            }
            out.extend(v[3..9].iter().map(|x| snorm(*x)));
            out.extend(v[9..12].iter().map(|x| unorm(*x)));
            out.push(unorm(v[12]));
        }
    }
    out
}

/// Unpacks a chunk.
///
/// # Errors
/// The bytes aren't a chunk, or end early.
pub fn decode(bytes: &[u8]) -> Result<Vec<Part>, String> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let slice = bytes
            .get(at..at + n)
            .ok_or_else(|| "the chunk ends early".to_owned())?;
        at += n;
        Ok(slice)
    };
    if take(4)? != MAGIC {
        return Err("not a mesh chunk".into());
    }
    let word = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let count = word(take(4)?);
    let mut parts = Vec::new();
    for _ in 0..count {
        let len = take(1)?[0] as usize;
        let name = String::from_utf8_lossy(take(len)?).into_owned();
        let vertices = word(take(4)?) as usize;
        let packed = take(vertices.checked_mul(PACKED).ok_or("the chunk is too big")?)?;
        let mut data = Vec::with_capacity(vertices * STRIDE);
        for v in packed.chunks_exact(PACKED) {
            for i in 0..3 {
                data.push(f32::from_le_bytes([
                    v[i * 4],
                    v[i * 4 + 1],
                    v[i * 4 + 2],
                    v[i * 4 + 3],
                ]));
            }
            data.extend(v[12..18].iter().map(|b| f32::from(*b as i8) / 127.0));
            data.extend(v[18..22].iter().map(|b| f32::from(*b) / 255.0));
        }
        parts.push(Part { name, data });
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunk_round_trips_positions_and_colours_exactly() {
        let vertex = [
            1.5, -2.25, 3.0, 0.0, 1.0, 0.0, 0.6, 0.8, 0.0, 0.2, 0.4, 1.0, 1.0,
        ];
        let parts = vec![
            Part {
                name: "room".into(),
                data: vertex.repeat(3),
            },
            Part {
                name: "lights".into(),
                data: Vec::new(),
            },
        ];
        let bytes = encode(&parts);
        assert_eq!(bytes.len(), 8 + 1 + 4 + 4 + 3 * PACKED + 1 + 6 + 4);
        let back = decode(&bytes).unwrap();
        assert_eq!(back[0].name, "room");
        assert_eq!(back[0].data.len(), 3 * STRIDE);
        assert_eq!(&back[0].data[0..3], &vertex[0..3]);
        for (a, b) in back[0].data.iter().zip(vertex.iter().cycle()) {
            assert!((a - b).abs() < 0.01, "{a} vs {b}");
        }
        assert!(back[1].data.is_empty());
    }

    #[test]
    fn a_short_or_foreign_chunk_is_refused() {
        assert!(decode(b"nope").is_err());
        let bytes = encode(&[Part {
            name: "x".into(),
            data: vec![0.0; STRIDE],
        }]);
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    }
}
