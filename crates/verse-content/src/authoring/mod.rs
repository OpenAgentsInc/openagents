//! Local data authoring with runtime admission and immutable published generations.
pub mod cli;
mod document;
mod geometry;
pub use geometry::BoxGeometry;
mod preview;
pub mod publication;
pub mod release;
mod workspace;
pub use document::{Document, Edit, Journal, ModelEdit, TimelineCue, Transaction};
pub use preview::{Preview, PreviewReport};
use serde::{Deserialize, Serialize};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    play::Game,
    service::{Chamber, Principal},
};
pub use workspace::{Build, Workspace};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub source: String,
    pub field: String,
    pub message: String,
}
impl Diagnostic {
    pub fn at(
        source: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            field: field.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.source, self.field, self.message)
    }
}
impl std::error::Error for Diagnostic {}
pub type Result<T> = std::result::Result<T, Diagnostic>;
fn checked<T>(field: impl Into<String>, result: std::result::Result<T, String>) -> Result<T> {
    result.map_err(|e| Diagnostic::at("document.json", field, e))
}
pub fn parse<T: serde::de::DeserializeOwned>(
    source: &str,
    bytes: &[u8],
    limit: usize,
) -> Result<T> {
    if bytes.is_empty() || bytes.len() > limit {
        return Err(Diagnostic::at(
            source,
            "$",
            "Document exceeds its byte budget or is empty",
        ));
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut decoder)
        .map_err(|e| Diagnostic::at(source, e.path().to_string(), e.inner().to_string()))?;
    decoder
        .end()
        .map_err(|e| Diagnostic::at(source, "$", e.to_string()))?;
    Ok(value)
}
/// Build candidate values without altering the source pack or active authority.
pub fn admit(
    doc: &Document,
    base: &Pack,
) -> Result<(Pack, Scene, verse_world::service::auth::Gateway)> {
    if doc.schema != "verse.author.document.v1"
        || doc.zone.is_empty()
        || doc.zone.len() > 64
        || !doc
            .zone
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Diagnostic::at(
            "document.json",
            "zone",
            "Use 1..64 lowercase letters, digits, or hyphens and schema verse.author.document.v1",
        ));
    }
    if !doc.scene.cues.is_empty() {
        return Err(Diagnostic::at(
            "document.json",
            "scene.cues",
            "Edit the stable-ID timeline instead",
        ));
    }
    checked("scene", doc.scene.validate())?;
    if doc
        .scene
        .actors
        .iter()
        .filter(|a| a.model == "adventurer")
        .count()
        != 1
    {
        return Err(Diagnostic::at(
            "document.json",
            "scene.actors",
            "Author exactly one primary adventurer",
        ));
    }
    for actor in &doc.scene.actors {
        let mut one = doc.scene.clone();
        one.actors = vec![actor.clone()];
        checked(
            format!("scene.actors[id={}].properties", actor.id),
            one.validate(),
        )?;
    }
    if doc.timeline.len() > 1024
        || doc.timeline.iter().any(|c| c.id == 0)
        || doc
            .timeline
            .iter()
            .map(|c| c.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != doc.timeline.len()
    {
        return Err(Diagnostic::at(
            "document.json",
            "timeline.id",
            "Use unique nonzero cue IDs; limit 1024",
        ));
    }
    let scene = doc.runtime_scene();
    for cue in &doc.timeline {
        let mut one = doc.scene.clone();
        one.cues = vec![cue.cue.clone()];
        checked(format!("timeline[id={}].cue", cue.id), one.validate())?;
    }
    checked("timeline", scene.validate())?;
    if doc.placements.len() > 4096
        || doc.placements.keys().any(|id| *id == 0)
        || doc.models.len() > 256
    {
        return Err(Diagnostic::at(
            "document.json",
            "placements",
            "Use nonzero placement IDs; limits are 4096 placements and 256 model edits",
        ));
    }
    let mut pack = base.clone();
    pack.placements = doc.placements.values().cloned().collect();
    if doc.primitives.len() > 128 {
        return Err(Diagnostic::at(
            "document.json",
            "primitives",
            "Primitive model budget is 128",
        ));
    }
    for (key, shape) in &doc.primitives {
        if pack.models.contains_key(key) {
            return Err(Diagnostic::at(
                "document.json",
                format!("primitives.{key}"),
                "Primitive key collides with a source model",
            ));
        }
        let model = geometry::compile(key, shape, pack.textures.len())?;
        if let Some(inventory) = &mut pack.inventory {
            use verse_engine::inventory::{Asset, AssetId, Binding, License, Origin};
            let id = checked(
                format!("primitives.{key}"),
                AssetId::new(format!("verse.authoring.model/{key}")),
            )?;
            let source_id = checked(
                format!("primitives.{key}"),
                AssetId::new(format!("verse.authoring.source/{}", model.source_sha256)),
            )?;
            if !inventory.assets.iter().any(|a| a.id == source_id) {
                let encoded = serde_json::to_vec(shape).map_err(|e| {
                    Diagnostic::at("document.json", format!("primitives.{key}"), e.to_string())
                })?;
                inventory.assets.push(Asset {
                    id: source_id.clone(),
                    binding: Binding::Source {
                        origin: Origin {
                            creator: "Local author".into(),
                            license: License::OwnerSuppliedLocal,
                            revision: model.source_sha256.clone(),
                            format: "verse.author.box.v1".into(),
                        },
                    },
                    sha256: model.source_sha256.clone(),
                    bytes: encoded.len() as u64,
                    dependencies: vec![],
                });
            }
            let texture = inventory
                .assets
                .iter()
                .find(|a| {
                    a.binding
                        == Binding::Texture {
                            slot: shape.texture,
                        }
                })
                .ok_or_else(|| {
                    Diagnostic::at(
                        "document.json",
                        format!("primitives.{key}.texture"),
                        "Texture provenance is absent",
                    )
                })?
                .id
                .clone();
            let (sha256, bytes) = checked(
                format!("primitives.{key}"),
                verse_engine::inventory::fingerprint(&model),
            )?;
            inventory.assets.push(Asset {
                id,
                binding: Binding::Model { key: key.clone() },
                sha256,
                bytes,
                dependencies: vec![source_id, texture],
            });
        }
        pack.models.insert(key.clone(), model);
    }
    for actor in &doc.scene.actors {
        if !pack.models.contains_key(&actor.model) {
            return Err(Diagnostic::at(
                "document.json",
                format!("scene.actors[id={}].model", actor.id),
                "Choose a model listed by inspect",
            ));
        }
    }

    for (key, edit) in &doc.models {
        let model = pack.models.get_mut(key).ok_or_else(|| {
            Diagnostic::at(
                "document.json",
                format!("models.{key}"),
                "Choose an existing admitted model",
            )
        })?;
        if let Some(states) = &edit.states {
            model.states = states.clone();
        }
        for (index, material) in &edit.materials {
            checked(
                format!("models.{key}.materials.{index}"),
                material.validate(pack.textures.len()),
            )?;
            let surface = model.surfaces.get_mut(*index).ok_or_else(|| {
                Diagnostic::at(
                    "document.json",
                    format!("models.{key}.materials.{index}"),
                    "Surface index is absent; inspect the model first",
                )
            })?;
            surface.material = material.clone();
        }
        if let Some(graph) = &edit.graph {
            checked(format!("models.{key}.graph"), graph.validate(model))?;
            model.graph = Some(graph.clone());
        }
        checked(format!("models.{key}.states"), model.validate_animation())?;
    }
    // Retain source declarations and stable IDs while sealing changed model bytes.
    if let Some(inventory) = &mut pack.inventory {
        for asset in &mut inventory.assets {
            if let verse_engine::inventory::Binding::Model { key } = &asset.binding {
                let (digest, length) = checked(
                    format!("models.{key}"),
                    verse_engine::inventory::fingerprint(&pack.models[key]),
                )?;
                asset.sha256 = digest;
                asset.bytes = length;
                for slot in pack.models[key]
                    .surfaces
                    .iter()
                    .flat_map(|s| s.texture_slots())
                {
                    let dependency = base
                        .inventory
                        .as_ref()
                        .and_then(|i| {
                            i.assets.iter().find(|a| {
                                a.binding == verse_engine::inventory::Binding::Texture { slot }
                            })
                        })
                        .ok_or_else(|| {
                            Diagnostic::at(
                                "document.json",
                                format!("models.{key}.materials"),
                                "Texture has no retained provenance identity",
                            )
                        })?;
                    if !asset.dependencies.contains(&dependency.id) {
                        asset.dependencies.push(dependency.id.clone());
                    }
                }
            }
        }
    }
    for (id, placement) in &doc.placements {
        if !pack.models.contains_key(&placement.model)
            || placement
                .position
                .iter()
                .chain(&placement.rotation)
                .any(|v| !v.is_finite())
            || !placement.scale.is_finite()
            || placement.scale == 0.
            || glam::Quat::from_array(placement.rotation).length_squared() < 1e-8
        {
            return Err(Diagnostic::at(
                "document.json",
                format!("placements[id={id}]"),
                "Choose an admitted model and finite nonzero transform",
            ));
        }
    }
    checked("models", pack.validate())?;
    let catalog = checked("models", verse_engine::residency::Catalog::new(&pack))?;
    checked(
        "outfits",
        crate::remote_content::outfit_models(&pack, &doc.outfits),
    )?;
    checked(
        "equipment",
        crate::remote_content::equipment_models(&pack, &scene, &doc.outfits, &doc.equipment),
    )?;
    for cue in &doc.timeline {
        if let verse_engine::director::Action::Yell { animation, .. } = cue.cue.action {
            let actor = scene
                .actors
                .iter()
                .find(|a| a.id == cue.cue.actor)
                .ok_or_else(|| {
                    Diagnostic::at(
                        "document.json",
                        format!("timeline[id={}].actor", cue.id),
                        "Actor is absent",
                    )
                })?;
            let handle = checked(
                format!("scene.actors[id={}].model", actor.id),
                catalog.model(&actor.model),
            )?;
            checked(
                format!("timeline[id={}].animation", cue.id),
                catalog.check_animation(handle, animation),
            )?;
        }
    }
    if let Some(character) = &doc.authored.character {
        for (slot, tuning) in &character.catalog {
            if !tuning.cooldown.is_finite() || !(0. ..=600.).contains(&tuning.cooldown) {
                return Err(Diagnostic::at(
                    "document.json",
                    format!("authored.character.catalog.{slot}.cooldown"),
                    "Use a finite cooldown from 0 to 600 seconds",
                ));
            }
            if !(0..=20).contains(&tuning.cost) || tuning.cost > character.mana {
                return Err(Diagnostic::at(
                    "document.json",
                    format!("authored.character.catalog.{slot}.cost"),
                    "Use 0..20 mana within the character's available mana",
                ));
            }
        }
    }
    checked("authored", doc.authored.validate())?;
    checked("progression", doc.progression.validate())?;
    checked(
        "rewards",
        verse_world::service::rewards::Policy::validate(&doc.rewards),
    )?;
    checked("items", doc.items.validate())?;
    checked(
        "equipment",
        doc.equipment.validate_catalogs(&doc.items, &doc.outfits),
    )?;
    for quest in &doc.progression.quests {
        if let Some(giver) = quest.giver {
            if !scene
                .actors
                .iter()
                .any(|a| a.id == giver && a.friendly && a.nameplate && a.model != "adventurer")
            {
                return Err(Diagnostic::at(
                    "document.json",
                    format!("progression.quests[id={}].giver", quest.id),
                    "Choose a friendly named NPC in this scene",
                ));
            }
        }
        if !doc
            .rewards
            .iter()
            .any(|r| r.quests.iter().any(|e| e.id == quest.objective))
        {
            return Err(Diagnostic::at(
                "document.json",
                format!("progression.quests[id={}].objective", quest.id),
                "Add a defeat reward that advances this objective",
            ));
        }
    }
    for reward in &doc.rewards {
        if !scene
            .actors
            .iter()
            .any(|a| a.id == reward.target && a.nameplate && !a.friendly && a.model != "adventurer")
        {
            return Err(Diagnostic::at(
                "document.json",
                format!("rewards[target={}].target", reward.target),
                "Choose a hostile named NPC in this scene",
            ));
        }
    }
    let mut game = if let Some(profile) = &doc.social_profile {
        if doc.authored.character.is_some() || !doc.authored.blockers.is_empty() {
            return Err(Diagnostic::at(
                "document.json",
                "social_profile",
                "Author social collision in the profile; character tuning requires combat",
            ));
        }
        if !doc.rewards.is_empty() {
            return Err(Diagnostic::at(
                "document.json",
                "rewards",
                "Social worlds cannot grant defeat rewards",
            ));
        }
        checked(
            "social_profile",
            Game::social_in(scene.clone(), 1, profile.clone()),
        )?
    } else {
        checked("scene", Game::combat_content_in(scene.clone(), 1))?
    };
    checked("authored", doc.authored.apply(&mut game))?;
    checked(
        "placements",
        crate::collision::admit_collision(&pack, &mut game),
    )?;
    let mut chamber = checked("scene", Chamber::new(game))?;
    let principal = Principal([1; 32]);
    checked("scene.actors", chamber.enroll_primary(principal))?;
    // Exercise the host's catalog admission rather than duplicating its giver checks.
    let gateway = checked(
        "progression",
        verse_world::service::auth::Gateway::new(chamber),
    )?;
    let gateway = checked("rewards", gateway.with_rewards(doc.rewards.clone()))?;
    let gateway = checked(
        "progression",
        gateway.with_progression(doc.progression.clone()),
    )?;
    let gateway = checked("items", gateway.with_items(doc.items.clone()))?;
    let gateway = checked("outfits", gateway.with_outfits(doc.outfits.clone()))?;
    let gateway = checked("equipment", gateway.with_equipment(doc.equipment.clone()))?;
    Ok((pack, scene, gateway))
}

#[cfg(test)]
mod tests;
