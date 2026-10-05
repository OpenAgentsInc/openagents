//! Block-letter labels in the world: signs over doors, arches, and stations.

use coder_ui::theme::Intensity;
use glam::Vec3;
use verse_pbr::mesh::Mesh;

/// Bounded ASCII text in an XY plane facing -Z. Quads use the world depth buffer.
pub fn label(mesh: &mut Mesh, text: &str, anchor: Vec3, height: f32, intensity: Intensity) {
    let letters: Vec<u8> = text
        .bytes()
        .take(32)
        .map(|b| b.to_ascii_uppercase())
        .collect();
    let cell = height / 7.0;
    let width = letters.len() as f32 * 6.0 * cell;
    for (i, letter) in letters.into_iter().enumerate() {
        let glyph = glyph(letter);
        for (row, bits) in glyph.into_iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                let x = width / 2.0 - (i as f32 * 6.0 + col as f32) * cell;
                let y = (6 - row) as f32 * cell;
                let p = anchor + Vec3::new(x, y, 0.0);
                mesh.amber_quad(
                    [
                        p,
                        p + Vec3::new(-cell, 0.0, 0.0),
                        p + Vec3::new(-cell, cell, 0.0),
                        p + Vec3::new(0.0, cell, 0.0),
                    ],
                    intensity,
                );
            }
        }
    }
}
fn glyph(c: u8) -> [u8; 7] {
    match c {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 2, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 30, 1, 1, 17, 14],
        b'6' => [6, 8, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 2, 12],
        _ => [0; 7],
    }
}
