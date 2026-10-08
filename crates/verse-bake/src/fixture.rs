//! The built-in fixture: a small yard with each kind of surface a bake
//! treats differently. It has tessellated ground, a closed house under an
//! overhanging roof, a white wall beside the grass, a leaf canopy whose
//! texels are half cut out, a glass pane, and a far level of detail.

use glam::{Mat4, Vec3};
use verse_pbr::pbr::textured::{
    AlphaMode, BaseColorImage, Detail, Primitive, TexturedMaterial, TexturedMesh, TexturedScene,
    TexturedVertex,
};

use crate::bake::{Light, Settings};

/// The fixture's light: the afternoon sun Everglade bakes under.
#[must_use]
pub fn light() -> Light {
    Light {
        sun_dir: Vec3::new(-0.42, 0.6, -0.56).normalize().to_array(),
        sun_illuminance: 4_000.0,
        sky: 1_200.0,
        ground: 450.0,
    }
}

/// Settings over the fixture's yard at the baker's default quality.
#[must_use]
pub fn settings() -> Settings {
    let light = light();
    let mut settings = Settings::new(
        Vec3::new(-12.0, 0.5, -12.0),
        Vec3::new(12.0, 6.5, 12.0),
        2.0,
        &light,
    );
    // Morning and evening too, as a time-of-day bake would.
    settings
        .suns
        .push(Vec3::new(0.8, 0.35, 0.2).normalize().to_array());
    settings
        .suns
        .push(Vec3::new(-0.7, 0.25, 0.4).normalize().to_array());
    settings
}

/// A grid of `n` by `n` quads with half extents `u` and `v` around
/// `center`, facing `normal`.
fn grid(center: Vec3, u: Vec3, v: Vec3, normal: Vec3, n: u32, material: usize) -> TexturedMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (a, b) = (i as f32 / n as f32, j as f32 / n as f32);
            vertices.push(TexturedVertex::new(
                center + u * (2.0 * a - 1.0) + v * (2.0 * b - 1.0),
                normal,
                [a, b],
            ));
        }
    }
    for j in 0..n {
        for i in 0..n {
            let k = j * (n + 1) + i;
            indices.extend([k, k + 1, k + n + 2, k, k + n + 2, k + n + 1]);
        }
    }
    TexturedMesh {
        primitives: vec![Primitive {
            vertices,
            indices,
            material,
        }],
    }
}

/// A closed box with half extents `half` around `center`.
fn cuboid(center: Vec3, half: Vec3, material: usize) -> TexturedMesh {
    let mut primitives = Vec::new();
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for sign in [1.0, -1.0] {
            let normal = axis * sign;
            let (u, v) = if axis == Vec3::Y {
                (Vec3::X * half.x, Vec3::Z * half.z)
            } else if axis == Vec3::X {
                (Vec3::Z * half.z, Vec3::Y * half.y)
            } else {
                (Vec3::X * half.x, Vec3::Y * half.y)
            };
            let mut face = grid(center + normal * half.dot(axis), u, v, normal, 2, material);
            primitives.append(&mut face.primitives);
        }
    }
    TexturedMesh { primitives }
}

fn material(scene: &mut TexturedScene, color: [f32; 4], alpha: AlphaMode) -> usize {
    scene.add_material(TexturedMaterial {
        base_color: color,
        alpha,
        double_sided: !matches!(alpha, AlphaMode::Opaque),
        ..TexturedMaterial::default()
    })
}

fn add(scene: &mut TexturedScene, mesh: TexturedMesh) {
    let mesh = scene.add_mesh(mesh);
    scene.place(mesh, Mat4::IDENTITY);
}

/// The fixture scene.
#[must_use]
pub fn scene() -> TexturedScene {
    let mut scene = TexturedScene::default();
    let grass = material(&mut scene, [0.12, 0.42, 0.1, 1.0], AlphaMode::Opaque);
    add(
        &mut scene,
        grid(
            Vec3::ZERO,
            Vec3::X * 12.0,
            Vec3::Z * 12.0,
            Vec3::Y,
            24,
            grass,
        ),
    );
    let plaster = material(&mut scene, [0.8, 0.74, 0.62, 1.0], AlphaMode::Opaque);
    add(
        &mut scene,
        cuboid(
            Vec3::new(-4.0, 2.0, -3.0),
            Vec3::new(3.0, 2.0, 2.5),
            plaster,
        ),
    );
    let roof = material(&mut scene, [0.45, 0.2, 0.15, 1.0], AlphaMode::Opaque);
    add(
        &mut scene,
        cuboid(Vec3::new(-4.0, 4.15, -3.0), Vec3::new(4.0, 0.15, 3.5), roof),
    );
    let white = material(&mut scene, [0.9, 0.9, 0.9, 1.0], AlphaMode::Opaque);
    add(
        &mut scene,
        grid(
            Vec3::new(5.0, 1.5, 4.0),
            Vec3::X * 3.0,
            Vec3::Y * 1.5,
            Vec3::Z,
            6,
            white,
        ),
    );
    // Half the canopy's texels pass the cutoff.
    let leaves = scene.add_image(BaseColorImage {
        name: "fixture-leaves".into(),
        width: 2,
        height: 2,
        rgba: [255u8, 0, 0, 255]
            .iter()
            .flat_map(|&a| [60, 140, 40, a])
            .collect(),
    });
    let leaf = scene.add_material(TexturedMaterial {
        image: Some(leaves),
        alpha: AlphaMode::Mask { cutoff: 0.5 },
        double_sided: true,
        ..TexturedMaterial::default()
    });
    add(
        &mut scene,
        grid(
            Vec3::new(5.0, 3.5, -5.0),
            Vec3::X * 2.5,
            Vec3::Z * 2.5,
            Vec3::Y,
            4,
            leaf,
        ),
    );
    let glass = material(&mut scene, [0.7, 0.8, 0.85, 0.3], AlphaMode::Blend);
    add(
        &mut scene,
        grid(
            Vec3::new(-6.0, 1.5, 5.0),
            Vec3::Z * 1.5,
            Vec3::Y * 1.5,
            Vec3::X,
            2,
            glass,
        ),
    );
    // A shed with a near and a far level: only the near one occludes.
    let wood = material(&mut scene, [0.4, 0.28, 0.16, 1.0], AlphaMode::Opaque);
    let near = scene.add_mesh(cuboid(
        Vec3::new(8.0, 1.0, -9.0),
        Vec3::new(1.2, 1.0, 1.2),
        wood,
    ));
    let far = scene.add_mesh(cuboid(
        Vec3::new(8.0, 1.0, -9.0),
        Vec3::new(1.15, 0.95, 1.15),
        wood,
    ));
    scene.place_detail(near, Mat4::IDENTITY, Detail::Near(0));
    scene.place_detail(far, Mat4::IDENTITY, Detail::Far(0));
    scene.switches = vec![60.0];
    scene
}

/// Where [`lamp_scene`]'s lamp hangs: 1.5 m in front of the white wall.
pub const LAMP: Vec3 = Vec3::new(5.0, 1.5, 5.5);

/// The fixture with a lamp: a warm emissive cube 0.3 m across at [`LAMP`].
#[must_use]
pub fn lamp_scene() -> TexturedScene {
    let mut scene = scene();
    let glow = scene.add_material(TexturedMaterial {
        base_color: [1.0, 0.8, 0.5, 1.0],
        emissive: 6_000.0,
        ..TexturedMaterial::default()
    });
    add(&mut scene, cuboid(LAMP, Vec3::splat(0.15), glow));
    scene
}
