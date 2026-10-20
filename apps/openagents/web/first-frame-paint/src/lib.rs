//! Paints a captured first frame (`openagents-first-frame`) into a canvas
//! with WebGL2, the way GPUI's wgpu renderer would draw the same scene.
//!
//! The page carries this module and the scene inline, so the frame shows
//! after one round trip, while the full app downloads behind it. The
//! painter reads the scene through [`openagents_first_frame::Scene::decode`],
//! which refuses anything malformed, then draws its commands in order:
//!
//! - quads and shadows as instanced rectangles shaded by GPUI's SDFs;
//! - paths as triangles into a multisampled offscreen layer, then copied
//!   back through their bounds, as GPUI's path pipeline does;
//! - glyphs as instanced sprites from one coverage texture, with GPUI's
//!   contrast and gamma correction.
//!
//! The shader math is ported from zui's `gpui_wgpu` (Apache-2.0); see
//! [`shaders`].

use openagents_first_frame::{Error, Scene};

pub mod shaders;

/// Floats per instance, and per path vertex.
pub const QUAD_FLOATS: usize = 24;
pub const SHADOW_FLOATS: usize = 28;
pub const SPRITE_FLOATS: usize = 16;
pub const PATH_FLOATS: usize = 12;
pub const LAYER_FLOATS: usize = 4;

/// The scene's primitives as GPU instance data, one array a kind.
#[derive(Debug, Default, PartialEq)]
pub struct Instances {
    pub quads: Vec<f32>,
    pub shadows: Vec<f32>,
    pub sprites: Vec<f32>,
    pub path_vertices: Vec<f32>,
    pub layers: Vec<f32>,
}

fn rect(out: &mut Vec<f32>, r: &openagents_first_frame::Rect) {
    out.extend_from_slice(&[r.x, r.y, r.w, r.h]);
}

/// Builds the instance arrays. The scene must already be valid, as
/// [`Scene::decode`] guarantees.
pub fn instances(scene: &Scene) -> Instances {
    let color = |i: u16| scene.colors[i as usize];
    let mask = |i: u16| &scene.masks[i as usize];
    let mut out = Instances::default();
    for q in &scene.quads {
        rect(&mut out.quads, &q.bounds);
        rect(&mut out.quads, mask(q.mask));
        out.quads.extend_from_slice(&color(q.background));
        out.quads.extend_from_slice(&color(q.border_color));
        out.quads.extend_from_slice(&q.radii);
        out.quads.extend_from_slice(&q.borders);
    }
    for s in &scene.shadows {
        rect(&mut out.shadows, &s.bounds);
        out.shadows.extend_from_slice(&s.radii);
        rect(&mut out.shadows, mask(s.mask));
        out.shadows.extend_from_slice(&color(s.color));
        rect(&mut out.shadows, &s.element);
        out.shadows.extend_from_slice(&s.element_radii);
        out.shadows
            .extend_from_slice(&[s.blur, if s.inset { 1.0 } else { 0.0 }, 0.0, 0.0]);
    }
    for s in &scene.sprites {
        let t = &scene.tiles[s.tile as usize];
        let (w, h) = (t.w as f32, t.h as f32);
        out.sprites.extend_from_slice(&[s.x, s.y, w, h]);
        rect(&mut out.sprites, mask(s.mask));
        out.sprites.extend_from_slice(&color(s.color));
        out.sprites
            .extend_from_slice(&[t.x as f32, t.y as f32, w, h]);
    }
    for p in &scene.paths {
        let c = color(p.color);
        for &i in &scene.path_indices[p.indices.start as usize..p.indices.end as usize] {
            let v = &scene.path_vertices[i as usize];
            out.path_vertices.extend_from_slice(&[v.x, v.y, v.s, v.t]);
            out.path_vertices.extend_from_slice(&c);
            rect(&mut out.path_vertices, &p.clip);
        }
    }
    for r in &scene.path_sprites {
        rect(&mut out.layers, r);
    }
    out
}

/// Where each path's vertices start in [`Instances::path_vertices`], in
/// vertices, with the total at the end.
pub fn path_offsets(scene: &Scene) -> Vec<u32> {
    let mut at = 0;
    let mut offsets = Vec::with_capacity(scene.paths.len() + 1);
    for p in &scene.paths {
        offsets.push(at);
        at += p.indices.end - p.indices.start;
    }
    offsets.push(at);
    offsets
}

/// Decodes `bytes`, refusing a malformed scene, and checks it's one this
/// painter can draw.
pub fn read(bytes: &[u8]) -> Result<Scene, Error> {
    let scene = Scene::decode(bytes)?;
    if scene.commands.is_empty() {
        return Err(Error("the scene draws nothing"));
    }
    Ok(scene)
}

#[cfg(target_family = "wasm")]
mod gl;

#[cfg(target_family = "wasm")]
pub use gl::paint;

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_first_frame::{Atlas, Command, Path, PathVertex, Quad, Rect, Sprite, Tile};

    fn scene() -> Scene {
        Scene {
            width: 10,
            height: 10,
            scale: 2.0,
            colors: vec![[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 0.5]],
            masks: vec![Rect::new(0.0, 0.0, 10.0, 10.0)],
            quads: vec![Quad {
                bounds: Rect::new(1.0, 2.0, 3.0, 4.0),
                background: 0,
                border_color: 1,
                radii: [1.0, 2.0, 3.0, 4.0],
                borders: [0.5; 4],
                mask: 0,
            }],
            paths: vec![
                Path {
                    color: 1,
                    clip: Rect::new(0.0, 0.0, 5.0, 5.0),
                    indices: 3..6,
                },
                Path {
                    color: 0,
                    clip: Rect::new(0.0, 0.0, 5.0, 5.0),
                    indices: 0..3,
                },
            ],
            path_vertices: (0..6)
                .map(|i| PathVertex {
                    x: i as f32,
                    y: 0.0,
                    s: 0.0,
                    t: 1.0,
                })
                .collect(),
            path_indices: (0..6).collect(),
            path_sprites: vec![Rect::new(0.0, 0.0, 5.0, 5.0)],
            tiles: vec![Tile {
                x: 2,
                y: 0,
                w: 2,
                h: 3,
            }],
            sprites: vec![Sprite {
                x: 4.0,
                y: 5.0,
                mask: 0,
                color: 1,
                tile: 0,
            }],
            atlas: Atlas {
                width: 4,
                height: 3,
                channels: 1,
                pixels: vec![0; 12],
            },
            commands: vec![
                Command::Quads(0..1),
                Command::Paths {
                    paths: 0..2,
                    sprites: 0..1,
                },
                Command::Sprites(0..1),
            ],
            ..Scene::default()
        }
    }

    #[test]
    fn instances_carry_resolved_colors_masks_and_tiles() {
        let s = scene();
        let i = instances(&s);
        assert_eq!(i.quads.len(), QUAD_FLOATS);
        assert_eq!(&i.quads[..8], &[1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 10.0, 10.0]);
        assert_eq!(&i.quads[8..16], &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.5]);
        assert_eq!(i.sprites.len(), SPRITE_FLOATS);
        assert_eq!(&i.sprites[..4], &[4.0, 5.0, 2.0, 3.0]);
        assert_eq!(&i.sprites[12..], &[2.0, 0.0, 2.0, 3.0]);
        // Paths in scene order, each vertex with its path's color and clip.
        assert_eq!(i.path_vertices.len(), 6 * PATH_FLOATS);
        assert_eq!(i.path_vertices[0], 3.0);
        assert_eq!(&i.path_vertices[4..8], &[0.0, 0.0, 1.0, 0.5]);
        assert_eq!(i.path_vertices[3 * PATH_FLOATS], 0.0);
        assert_eq!(path_offsets(&s), vec![0, 3, 6]);
        assert_eq!(i.layers, vec![0.0, 0.0, 5.0, 5.0]);
    }

    #[test]
    fn the_painter_refuses_malformed_scenes() {
        let bytes = scene().encode().unwrap();
        assert!(read(&bytes).is_ok());
        assert!(read(&bytes[..bytes.len() - 1]).is_err());
        assert!(read(b"not a scene").is_err());
        let mut bad = bytes.clone();
        bad[5] = 7;
        assert!(read(&bad).is_err());
        let empty = Scene {
            width: 1,
            height: 1,
            scale: 1.0,
            ..Scene::default()
        };
        assert_eq!(
            read(&empty.encode().unwrap()),
            Err(Error("the scene draws nothing"))
        );
    }

    #[test]
    fn every_shader_has_its_entry_point_and_head() {
        for body in [
            shaders::QUAD_FS,
            shaders::SHADOW_FS,
            shaders::SPRITE_FS,
            shaders::PATH_FS,
            shaders::LAYER_FS,
        ] {
            let full = shaders::fs(body);
            assert!(full.starts_with("#version 300 es"));
            assert!(full.contains("void main()"));
        }
        assert!(shaders::vs(shaders::QUAD_VS).contains("gl_VertexID"));
    }
}
