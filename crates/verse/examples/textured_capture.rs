//! Offline visual acceptance of textured static meshes in the zone renderer.
//! Usage: textured_capture OUTPUT.png
//!
//! Imports four Fantasy Props models from `assets/verse/props/quaternius`
//! into one textured scene and renders them on a neutral neon stage under a
//! studio key light, through the same physical path a zone uses:
//!
//! - a barrel and a table, opaque, with their trim-sheet base colors;
//! - the banner, alpha-masked: the kit has no cutout foliage, so its cloth
//!   image gets a lattice of holes in its alpha channel here, which the mask
//!   cuts out of the cloth and out of its shadow;
//! - the potion, blended as tinted glass.
//!
//! It prints the scene's size and writes a 1280×720 PNG.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use verse::mesh::Mesh;
use verse::pbr::textured::{AlphaMode, TexturedScene};
use verse::pbr::{Key, LitVertex, Material, Neon};
use verse::render::View;

fn main() -> Result<(), String> {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output PNG path")?,
    );
    let props = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/props/quaternius");
    let mut scene = TexturedScene::default();
    let import = |scene: &mut TexturedScene, name: &str| {
        scene.import_gltf(&props.join(format!("{name}.gltf")))
    };
    let barrel = import(&mut scene, "Barrel")?;
    let table = import(&mut scene, "Table_Large")?;
    let banner = import(&mut scene, "Banner_2")?;
    let potion = import(&mut scene, "Potion_1")?;

    // Cut a lattice of round holes into the banner cloth's alpha and mask it.
    let cloth = scene
        .images
        .iter()
        .position(|image| image.name.ends_with("T_Trim_Cloth_BaseColor.png"))
        .ok_or("Banner_2 has no cloth image")?;
    let mut lace = scene.images[cloth].clone();
    lace.name = format!("{} with holes", lace.name);
    let (width, height) = (lace.width as usize, lace.height as usize);
    for (i, texel) in lace.rgba.chunks_exact_mut(4).enumerate() {
        let u = (i % width) as f32 / width as f32 * 48.0;
        let v = (i / width) as f32 / height as f32 * 48.0;
        let (du, dv) = (u.fract() - 0.5, v.fract() - 0.5);
        if du * du + dv * dv < 0.09 {
            texel[3] = 0;
        }
    }
    let lace = scene.add_image(lace);
    for material in &mut scene.materials {
        if material.image == Some(cloth) {
            material.image = Some(lace);
            material.alpha = AlphaMode::Mask { cutoff: 0.5 };
        }
    }
    // The potion's materials become tinted glass.
    let glass: Vec<usize> = scene.meshes[potion]
        .primitives
        .iter()
        .map(|p| p.material)
        .collect();
    for index in glass {
        let material = &mut scene.materials[index];
        material.alpha = AlphaMode::Blend;
        material.base_color[3] = 0.45;
        material.roughness = 0.15;
    }

    // Stand each prop on the floor.
    let lowest = |scene: &TexturedScene, mesh: usize| {
        scene.meshes[mesh]
            .primitives
            .iter()
            .flat_map(|p| &p.vertices)
            .map(|v| v.pos[1])
            .fold(f32::INFINITY, f32::min)
    };
    for (mesh, position, yaw) in [
        (barrel, Vec3::new(-2.2, 0.0, 0.3), 0.4),
        (table, Vec3::new(0.0, 0.0, -0.6), 0.0),
        (banner, Vec3::new(2.4, 0.0, -0.8), -0.5),
        (potion, Vec3::new(0.9, 0.0, 1.2), 0.0),
    ] {
        let lift = Vec3::Y * -lowest(&scene, mesh);
        scene.place(
            mesh,
            Mat4::from_translation(position + lift) * Mat4::from_rotation_y(yaw),
        );
    }
    eprintln!("{scene:?}");

    let world = Mesh {
        lit: floor(8.0),
        textured: Some(Arc::new(scene)),
        ..Mesh::default()
    };
    let mut neon = Neon::neutral(0.0);
    neon.key = Some(Key {
        dir: Vec3::new(0.55, 0.7, 0.45).normalize(),
        illuminance: 4_200.0,
        angular_radius: 0.035,
        rim_dir: Vec3::new(-0.4, 0.35, -0.85).normalize(),
        rim_illuminance: 2_000.0,
        rim_angular_radius: 0.12,
        sky: 420.0,
        ground: 60.0,
        ev100: 10.0,
        shadow_center: Vec3::ZERO,
        shadow_half: 6.0,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    let dynamic = Mesh {
        neon: Some(neon),
        ..Mesh::default()
    };
    let (width, height) = (1280, 720);
    let eye = Vec3::new(0.0, 2.0, 6.5);
    let view = View {
        view_proj: Mat4::perspective_rh(0.75, width as f32 / height as f32, 0.1, 200.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        eye,
    };
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &world,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )
}

/// A square stage floor `half` meters from the origin to each edge.
fn floor(half: f32) -> Vec<LitVertex> {
    let (color, metallic, roughness) = Material::Stage.parameters();
    let corner = |x: f32, z: f32| LitVertex {
        pos: [x * half, 0.0, z * half],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        local: [x * half, 0.0, z * half],
        color,
        params: [metallic, roughness, Material::Stage.code(), 1.0],
    };
    let [a, b, c, d] = [
        corner(-1.0, -1.0),
        corner(-1.0, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
    ];
    vec![a, b, c, a, c, d]
}
