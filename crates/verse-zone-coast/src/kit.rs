//! Original coast kit placements. The pack is decoded only on zone entry.

use crate::{ground, pack};
use glam::{Mat4, Quat, Vec3};
use verse_pbr::pbr::textured::{
    self, BaseColorImage, Detail, DetailGroup, Primitive, TexturedMaterial, TexturedMesh,
    TexturedScene, TexturedVertex,
};
use verse_world::social::controller::Footprint;

/// One static kit placement; moving boats and wildlife belong to C3 and C4.
#[derive(Clone, Copy)]
pub struct Placement {
    pub name: &'static str,
    pub at: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}

pub fn placements() -> Vec<Placement> {
    let mut out = Vec::new();
    let mut land = |name, x, z, scale| {
        out.push(Placement {
            name,
            at: [x, ground(x, z), z],
            yaw: 0.0,
            scale,
        })
    };
    for (name, x, z, scale) in [
        ("lighthouse/lighthouse", -330., -260., 1.),
        ("lighthouse/keeper_cottage", -313., -264., 1.),
        ("lighthouse/fog_bell", -327., -250., 1.),
        ("harbor/boathouse", -220., -285., 1.),
        ("rocks/cliff_straight", -225., -175., 1.),
        ("rocks/cliff_corner", -243., -175., 1.),
        ("rocks/cliff_inlet", -260., -168., 1.),
        ("rocks/sea_stack", -380., -65., 1.8),
        ("rocks/sea_arch", -390., -48., 1.8),
        ("rocks/sea_cave", -380., -30., 1.8),
        ("rocks/boulder", -315., -250., 1.),
        ("rocks/reef_rock", -100., 115., 1.),
        ("rocks/tide_pool_shelf", -230., -120., 1.),
        ("boats/wreck_bow", -100., -120., 1.),
        ("boats/wreck_stern", -100., -115., 1.),
        ("reef/kelp", -102., 120., 1.),
        ("reef/anemone", -98., 120., 1.),
        ("reef/reef_cluster", -100., 125., 1.),
    ] {
        land(name, x, z, scale);
    }
    for (i, name) in [
        "driftwood",
        "dune_fence",
        "beach_grass",
        "shell",
        "seaweed",
        "rope",
        "net",
        "crate",
        "barrel",
    ]
    .into_iter()
    .enumerate()
    {
        let name = match name {
            "driftwood" => "beach/driftwood",
            "dune_fence" => "beach/dune_fence",
            "beach_grass" => "beach/beach_grass",
            "shell" => "beach/shell",
            "seaweed" => "beach/seaweed",
            "rope" => "beach/rope",
            "net" => "beach/net",
            "crate" => "beach/crate",
            _ => "beach/barrel",
        };
        land(
            name,
            45. + (i % 3) as f32 * 3.,
            -153. + (i / 3) as f32 * 3.,
            1.,
        );
    }
    for (name, at) in [
        ("harbor/pier", [-130., 1.5, -263.]),
        ("harbor/mooring", [-131., 1.6, -264.]),
        ("harbor/bollard", [-129., 1.6, -264.]),
        ("harbor/hand_crane", [-130., 1.6, -265.]),
        ("harbor/buoy", [-130., 0., -250.]),
        ("boats/sloop", [-134., 0., -259.]),
        ("boats/sloop", [-126., 0., -259.]),
        ("boats/oars", [-130., 1.7, -263.]),
    ] {
        out.push(Placement {
            name,
            at,
            yaw: 0.,
            scale: 1.,
        });
    }
    for x in [-131.2, -128.8] {
        for z in [-264.5, -261.5] {
            let floor = ground(x, z);
            out.push(Placement {
                name: "harbor/piling",
                at: [x, floor, z],
                yaw: 0.,
                scale: (1.6 - floor) / 4.,
            });
        }
    }
    // Stone modules follow the existing terrain breakwater, without a bake.
    let [a, b] = verse_zone_water::coast::BREAKWATER.map(glam::Vec2::from);
    for i in 0..20 {
        let p = a.lerp(b, i as f32 / 19.);
        out.push(Placement {
            name: "harbor/breakwater",
            at: [p.x, 1.5, p.y],
            yaw: -(b.y - a.y).atan2(b.x - a.x),
            scale: 1.,
        });
    }
    out
}

pub fn append(scene: &mut TexturedScene) -> Result<Vec<Footprint>, String> {
    let pack = pack::bundled()?;
    let image_base = scene.images.len();
    for t in &pack.textures {
        scene.add_image(BaseColorImage {
            name: t.name.clone(),
            width: t.width,
            height: t.height,
            rgba: t.rgba.clone(),
        });
    }
    let material_base = scene.materials.len();
    for m in &pack.materials {
        use verse_zone_everglade::zones::everglade_pack::format::AlphaMode as A;
        scene.add_material(TexturedMaterial {
            image: m.texture.map(|i| image_base + i as usize),
            base_color: m.base_color,
            alpha: match m.alpha {
                A::Opaque => textured::AlphaMode::Opaque,
                A::Mask { cutoff } => textured::AlphaMode::Mask { cutoff },
                A::Blend => textured::AlphaMode::Blend,
            },
            double_sided: m.double_sided,
            roughness: 0.85,
            emissive: if m.name.contains("Lens") { 8. } else { 0. },
            ..TexturedMaterial::default()
        });
    }
    let mut copied = std::collections::BTreeMap::new();
    let mut blockers = Vec::new();
    for placement in placements() {
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(placement.scale),
            Quat::from_rotation_y(placement.yaw),
            Vec3::from(placement.at),
        );
        let group = scene.detail_groups.len() as u16;
        scene.detail_groups.push(DetailGroup {
            anchor: [placement.at[0], placement.at[2]],
            switches: [60., 150.],
            fallback: 0,
        });
        for level in 0..3 {
            let name = if level == 0 {
                placement.name.to_string()
            } else {
                format!("lod/{}.lod{level}", placement.name.replace('/', "."))
            };
            let model = pack
                .model(&name)
                .ok_or_else(|| format!("The coast pack has no {name}"))?;
            let mesh = *copied.entry(name).or_insert_with(|| {
                scene.add_mesh(TexturedMesh {
                    primitives: model
                        .primitives
                        .iter()
                        .map(|p| Primitive {
                            material: material_base + p.material as usize,
                            indices: p.indices.clone(),
                            vertices: p
                                .vertices
                                .iter()
                                .map(|v| TexturedVertex {
                                    pos: v.position,
                                    normal: v.normal,
                                    uv: v.uv,
                                    color: v.color,
                                    light: textured::UNBAKED,
                                })
                                .collect(),
                        })
                        .collect(),
                })
            });
            scene.place_instanced(mesh, transform, Detail::Group { group, level });
            if level == 0
                && matches!(
                    placement.name,
                    "lighthouse/lighthouse" | "lighthouse/keeper_cottage" | "harbor/boathouse"
                )
            {
                let (lo, hi) = model.bounds();
                blockers.push(Footprint {
                    min: [lo[0] + placement.at[0], lo[2] + placement.at[2]],
                    max: [hi[0] + placement.at[0], hi[2] + placement.at[2]],
                });
            }
        }
    }
    Ok(blockers)
}

/// Static building solids and the pier deck share the rendered placements.
pub fn solids() -> Result<verse_world::social::solids::Solids, String> {
    let mut solids = verse_world::social::solids::Solids::over(ground);
    let pack = pack::bundled()?;
    for p in placements() {
        if !matches!(
            p.name,
            "lighthouse/lighthouse"
                | "lighthouse/keeper_cottage"
                | "harbor/boathouse"
                | "harbor/pier"
        ) {
            continue;
        }
        let (lo, hi) = pack
            .model(p.name)
            .ok_or("Missing coast collision model")?
            .bounds();
        let footprint = Footprint {
            min: [lo[0] + p.at[0], lo[2] + p.at[2]],
            max: [hi[0] + p.at[0], hi[2] + p.at[2]],
        };
        let top = if p.name == "harbor/pier" {
            p.at[1] + 0.08
        } else {
            p.at[1] + hi[1]
        };
        solids.add_block(footprint, top);
    }
    Ok(solids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_static_model_has_a_placement_and_three_levels() {
        let pack = pack::bundled().unwrap();
        let placements = placements();
        for model in pack.models.iter().filter(|m| !m.name.starts_with("lod/")) {
            assert!(
                placements.iter().any(|p| p.name == model.name),
                "{}",
                model.name
            );
            for level in 1..3 {
                assert!(
                    pack.model(&format!("lod/{}.lod{level}", model.name.replace('/', ".")))
                        .is_some()
                );
            }
        }
        let mut scene = TexturedScene::default();
        let blockers = append(&mut scene).unwrap();
        scene.validate().unwrap();
        assert_eq!(scene.placements.len(), placements.len() * 3);
        assert!(
            !blockers
                .iter()
                .any(|b| b.contains(crate::SPAWN.x, crate::SPAWN.z, 3.))
        );
        let solids = solids().unwrap();
        assert!((solids.top(-130., -263.) - 1.58).abs() < 0.01);
        assert!(solids.top(-330., -260.) > ground(-330., -260.) + 20.);
        assert!(scene.gpu_bytes() + pack.decoded_texture_bytes() < 16 * 1024 * 1024);
    }
}
