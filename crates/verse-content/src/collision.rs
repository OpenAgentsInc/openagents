//! Original furnishing collision admission, shared by hosts and clients.
use glam::{Mat4, Quat, Vec3};
use verse_engine::assets::Pack;
/// Admit furniture bounds through the same world prop path as navigation blockers.
pub fn admit_collision(pack: &Pack, game: &mut verse_world::play::Game) -> Result<(), String> {
    let life = game.player_life();
    for (index, placement) in pack.placements.iter().enumerate() {
        if placement.model == "prop/flame" {
            let position = crate::basis()
                .transform_point3(placement.position.into())
                .as_dvec3();
            let id = index as u32;
            if !game.spells.flames.iter().any(|f| f.id == id) {
                game.spells.flames.push(verse_world::gust::Flame {
                    id,
                    position,
                    protected: position.x.abs() > 20. && (position.z + 18.).abs() < 1.,
                    lit: true,
                });
            }
        }
        if ![
            "Table_Large",
            "Cauldron",
            "Cage_Small",
            "Chest_Wood",
            "Barrel",
            "CandleStick_Stand",
        ]
        .iter()
        .any(|name| placement.model == format!("prop/{name}"))
        {
            continue;
        }
        let transform = crate::basis()
            * Mat4::from_scale_rotation_translation(
                Vec3::splat(placement.scale),
                Quat::from_array(placement.rotation),
                placement.position.into(),
            );
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in pack.models[&placement.model]
            .surfaces
            .iter()
            .flat_map(|s| &s.vertices)
        {
            let p = transform.transform_point3(vertex.position.into());
            min = min.min(p);
            max = max.max(p);
        }
        game.set_navigation_blocker(
            physics::queries::Life {
                instance: life.instance,
                entity: 10_000 + index as u64,
                generation: life.generation,
            },
            min.as_dvec3(),
            max.as_dvec3(),
        )?;
    }
    Ok(())
}
