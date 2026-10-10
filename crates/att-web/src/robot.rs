//! The Grid robot (`docs/verse/grid-robot.md`), baked by
//! `examples/bake_grid.rs` from the Grid pack in its idle pose: near-black
//! facets in four shades, gray edge lines and white glow, in meters, +Y up,
//! facing +Z, feet at y = 0.

use glam::Vec3;

use crate::mesh::{Lines, Mesh};

/// The baked robot.
pub const BAKED: &[u8] = include_bytes!("../assets/grid-robot.bin");

fn linear(srgb: u8) -> f32 {
    let s = f32::from(srgb) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let out = self
            .bytes
            .get(self.at..self.at + n)
            .ok_or("the baked robot is cut short")?;
        self.at += n;
        Ok(out)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn corner(&mut self) -> Result<Vec3, String> {
        let b = self.take(6)?;
        let v = |i: usize| f32::from(i16::from_le_bytes([b[i], b[i + 1]])) / 1000.0;
        Ok(Vec3::new(v(0), v(2), v(4)))
    }
}

/// The robot's facets (unlit, with their own shades) and edge lines.
pub fn parse(bytes: &[u8]) -> Result<(Mesh, Lines), String> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != b"GRB1" {
        return Err("not a baked Grid robot".into());
    }
    let (tris, lines) = (r.u32()?, r.u32()?);
    let edge = linear(r.u8()?);
    let mut mesh = Mesh::new();
    for _ in 0..tris {
        let shade = linear(r.u8()?);
        let p = [r.corner()?, r.corner()?, r.corner()?];
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        for v in p {
            mesh.data
                .extend_from_slice(&[v.x, v.y, v.z, n.x, n.y, n.z, shade, shade, shade]);
        }
    }
    let mut out = Lines::new();
    for _ in 0..lines {
        let (a, b) = (r.corner()?, r.corner()?);
        out.line(a, b, [edge; 3]);
    }
    if r.at != bytes.len() {
        return Err("the baked robot has trailing bytes".into());
    }
    Ok((mesh, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_baked_robot_stands_on_its_feet() {
        let (mesh, lines) = parse(BAKED).unwrap();
        assert_eq!(mesh.vertices(), 1300 * 3);
        assert_eq!(lines.vertices(), 909 * 2);
        let ys: Vec<f32> = mesh
            .data
            .chunks(crate::mesh::STRIDE)
            .map(|v| v[1])
            .collect();
        let (low, high) = ys
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!(low.abs() < 0.08, "{low}");
        assert!((1.7..2.0).contains(&high), "{high}");
        // Near-black armor and white glow: every shade is gray.
        let shades: Vec<f32> = mesh
            .data
            .chunks(crate::mesh::STRIDE)
            .map(|v| v[6])
            .collect();
        assert!(shades.iter().any(|s| *s < 0.02));
        assert!(shades.iter().any(|s| *s > 0.9));
        assert!(parse(b"nope").is_err());
    }
}
