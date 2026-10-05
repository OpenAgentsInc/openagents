//! Unlit content for the engine renderer: line segments and flat faces that
//! take their color from the surface tint, fade with the scene's fog, and
//! neither receive light nor cast shadows. The Grid's hidden-line look is
//! built from these.
use glam::{Mat4, Vec3};
use verse_engine::assets::{Model, Pack, Surface, Texture, Topology, Vertex};

/// The file name of the white texel every flat surface samples.
pub const WHITE_TEXTURE: &str = "verse-flat-white.png";

/// Writes the white texel into `dir` and returns its texture slot, adding
/// it to the pack when it is not there yet.
pub fn white_texture(pack: &mut Pack, dir: &std::path::Path) -> Result<usize, String> {
    use sha2::{Digest, Sha256};
    if let Some(slot) = pack.textures.iter().position(|t| t.file == WHITE_TEXTURE) {
        return Ok(slot);
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&[255; 4])
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join(WHITE_TEXTURE), &bytes).map_err(|e| e.to_string())?;
    pack.textures.push(Texture {
        file: WHITE_TEXTURE.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: 1,
        height: 1,
    });
    Ok(pack.textures.len() - 1)
}

fn vertex(p: Vec3) -> Vertex {
    Vertex {
        position: p.to_array(),
        normal: [0., 1., 0.],
        uv: [0.5, 0.5],
        joints: [0; 4],
        weights: [1., 0., 0., 0.],
    }
}

/// An empty surface of `topology` in `tint`, sampling the white texel.
#[must_use]
pub fn surface(texture: usize, tint: [f32; 3], topology: Topology) -> Surface {
    Surface {
        vertices: vec![],
        indices: vec![],
        texture,
        material: Default::default(),
        blend: 0,
        emissive: false,
        tint,
        topology,
        unlit: true,
    }
}

/// Appends one segment to a lines surface.
pub fn line(surface: &mut Surface, a: Vec3, b: Vec3) {
    let base = surface.vertices.len() as u32;
    surface.vertices.extend([vertex(a), vertex(b)]);
    surface.indices.extend([base, base + 1]);
}

/// Appends the twelve edges of a unit cube under `transform`.
pub fn cube_edges(surface: &mut Surface, transform: Mat4) {
    let corner = |i: usize| {
        transform.transform_point3(Vec3::new(
            if i & 1 == 0 { -0.5 } else { 0.5 },
            if i & 2 == 0 { -0.5 } else { 0.5 },
            if i & 4 == 0 { -0.5 } else { 0.5 },
        ))
    };
    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                line(surface, corner(i), corner(i | bit));
            }
        }
    }
}

/// Appends a flat quad with the corners in order.
pub fn quad(surface: &mut Surface, corners: [Vec3; 4]) {
    let base = surface.vertices.len() as u32;
    surface.vertices.extend(corners.map(vertex));
    surface
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A model made only of flat surfaces.
#[must_use]
pub fn model(source: &str, surfaces: Vec<Surface>, height: f32) -> Model {
    Model {
        graph: None,
        markers: vec![],
        states: Default::default(),
        skin: None,
        source: source.into(),
        source_sha256: String::new(),
        surfaces,
        bones: vec![],
        clips: vec![],
        height,
        attachments: vec![],
    }
}

/// A pack with a red line cube over a dark quad, for captures and tests.
pub fn sample_pack(dir: &std::path::Path) -> Result<Pack, String> {
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "flat-sample".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    let white = white_texture(&mut pack, dir)?;
    let mut edges = surface(white, [1., 0.1, 0.1], Topology::Lines);
    cube_edges(
        &mut edges,
        Mat4::from_translation(Vec3::new(0., 0.5, 0.)) * Mat4::from_scale(Vec3::splat(1.0)),
    );
    let mut floor = surface(white, [0.02, 0.02, 0.03], Topology::Triangles);
    quad(
        &mut floor,
        [
            Vec3::new(-40., 0., -40.),
            Vec3::new(40., 0., -40.),
            Vec3::new(40., 0., 40.),
            Vec3::new(-40., 0., 40.),
        ],
    );
    pack.models
        .insert("flat/cube".into(), model("flat/cube", vec![edges], 1.0));
    pack.models
        .insert("flat/floor".into(), model("flat/floor", vec![floor], 0.0));
    pack.validate()?;
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use verse_engine::presentation::Instance;

    fn red(pixels: &[u8], width: usize, x: usize, y: usize) -> (u8, u8, u8) {
        let i = (y * width + x) * 4;
        (pixels[i], pixels[i + 1], pixels[i + 2])
    }

    #[test]
    fn odd_line_indices_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut pack = sample_pack(dir.path()).unwrap();
        pack.models.get_mut("flat/cube").unwrap().surfaces[0]
            .indices
            .push(0);
        assert!(pack.validate().is_err());
    }

    #[test]
    fn lines_take_their_tint_and_fade_into_the_fog() {
        let dir = tempfile::tempdir().unwrap();
        let pack = sample_pack(dir.path()).unwrap();
        let atlas = crate::ui::Atlas::new(1.);
        let (width, height) = (256usize, 128usize);
        let mut renderer = super::super::Renderer::new(
            pack,
            dir.path(),
            width as u32,
            height as u32,
            &atlas,
            &[Instance {
                mount: None,
                actor: None,
                model: "flat/floor".into(),
                transform: Mat4::IDENTITY,
                animation: 0.into(),
                time: 0.,
                animation_epoch: None,
                emission: Vec3::ZERO,
            }],
        )
        .unwrap();
        let cube = |at: Vec3| Instance {
            mount: None,
            actor: None,
            model: "flat/cube".into(),
            transform: Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(2.0)),
            animation: 0.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ZERO,
        };
        let eye = Vec3::new(0., 1.5, -6.);
        let view = verse_engine::presentation::View {
            view_proj: Mat4::perspective_rh(60f32.to_radians(), 2., 0.1, 200.)
                * Mat4::look_at_rh(eye, Vec3::new(0., 1., 10.), Vec3::Y),
            eye,
        };
        let lighting = super::super::lighting::Lighting {
            ambient: Vec3::ZERO,
            density: 0.06,
            shadowed: 0,
            fog: Vec3::splat(0.0),
            ..Default::default()
        };
        let ui = crate::ui::UiBatch::default();
        let reddest = |pixels: &[u8]| {
            (0..height)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .map(|(x, y)| red(pixels, width, x, y))
                .max_by_key(|(r, g, b)| i32::from(*r) - i32::from(*g) - i32::from(*b))
                .unwrap()
        };
        let near = renderer
            .draw(view, &[cube(Vec3::new(-2.5, 0., 0.))], &ui, &lighting)
            .unwrap();
        assert_eq!(near.len(), width * height * 4);
        let (r, g, b) = reddest(&near);
        assert!(r > 150 && g < r / 2 && b < r / 2, "near edge {r} {g} {b}");
        let far = renderer
            .draw(view, &[cube(Vec3::new(1.5, 0., 60.))], &ui, &lighting)
            .unwrap();
        let (fr, _, _) = reddest(&far);
        assert!(fr < r / 2, "far edge {fr} is not fogged against near {r}");
        let pixels = near;
        // No light in the scene: a lit floor would be black, and the unlit floor
        // is not.
        let floor = red(&pixels, width, width / 2, height - 4);
        assert!(floor.0 > 0 || floor.1 > 0 || floor.2 > 0, "floor {floor:?}");
    }
}
