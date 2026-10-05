//! Stable author identities and data-only edits over runtime types.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use verse_engine::{
    assets::Placement,
    director::{Actor, Cue, Scene},
    material::Material,
    motion::{Binding, State},
};
use verse_world::{
    content::Authored,
    play::social::Profile,
    service::{equipment, items, outfits, progression, rewards},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineCue {
    pub id: u64,
    pub cue: Cue,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEdit {
    pub states: Option<BTreeMap<State, Binding>>,
    pub materials: BTreeMap<usize, Material>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: String,
    pub zone: String,
    pub scene: Scene,
    pub timeline: Vec<TimelineCue>,
    pub placements: BTreeMap<u64, Placement>,
    pub models: BTreeMap<String, ModelEdit>,
    #[serde(default)]
    pub primitives: BTreeMap<String, super::geometry::BoxGeometry>,
    pub authored: Authored,
    pub social_profile: Option<Profile>,
    pub rewards: Vec<rewards::Policy>,
    pub progression: progression::Config,
    pub items: items::Catalog,
    pub outfits: outfits::Catalog,
    pub equipment: equipment::Catalog,
}
impl Document {
    pub fn from_scene(
        zone: String,
        mut scene: Scene,
        placements: Vec<Placement>,
        social_profile: Option<Profile>,
    ) -> Self {
        let timeline = std::mem::take(&mut scene.cues)
            .into_iter()
            .enumerate()
            .map(|(index, cue)| TimelineCue {
                id: index as u64 + 1,
                cue,
            })
            .collect();
        Self {
            schema: "verse.author.document.v1".into(),
            zone,
            scene,
            timeline,
            placements: placements
                .into_iter()
                .enumerate()
                .map(|(i, p)| (i as u64 + 1, p))
                .collect(),
            models: BTreeMap::new(),
            primitives: BTreeMap::new(),
            authored: Authored::default(),
            social_profile,
            rewards: vec![],
            progression: progression::Config::default(),
            items: items::Catalog::default(),
            outfits: outfits::Catalog::default(),
            equipment: equipment::Catalog::default(),
        }
    }
    pub fn runtime_scene(&self) -> Scene {
        let mut scene = self.scene.clone();
        let mut cues = self.timeline.clone();
        cues.sort_by(|a, b| a.cue.at.total_cmp(&b.cue.at).then(a.id.cmp(&b.id)));
        scene.cues = cues.into_iter().map(|c| c.cue).collect();
        scene
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Zone {
        name: String,
    },
    Scene {
        duration: f32,
        cut_at: f32,
        origin: [f32; 3],
        collision_profile: Option<String>,
    },
    Actor {
        actor: Actor,
    },
    RemoveActor {
        id: u64,
    },
    Placement {
        id: u64,
        placement: Placement,
    },
    RemovePlacement {
        id: u64,
    },
    Cue {
        cue: TimelineCue,
    },
    RemoveCue {
        id: u64,
    },
    Model {
        key: String,
        edit: ModelEdit,
    },
    Primitive {
        key: String,
        shape: super::geometry::BoxGeometry,
    },
    RemovePrimitive {
        key: String,
    },
    Authored {
        settings: Authored,
    },
    Social {
        profile: Option<Profile>,
    },
    Quest {
        quest: progression::Quest,
    },
    RemoveQuest {
        id: u64,
    },
    Rewards {
        policies: Vec<rewards::Policy>,
    },
    Levels {
        thresholds: Vec<u64>,
    },
    Items {
        catalog: items::Catalog,
    },
    Outfits {
        catalog: outfits::Catalog,
    },
    Equipment {
        catalog: equipment::Catalog,
    },
}
impl Edit {
    pub fn apply(&self, doc: &mut Document) -> Result<(), String> {
        match self {
            Self::Zone { name } => doc.zone = name.clone(),
            Self::Scene {
                duration,
                cut_at,
                origin,
                collision_profile,
            } => {
                doc.scene.duration = *duration;
                doc.scene.cut_at = *cut_at;
                doc.scene.origin = *origin;
                doc.scene.collision_profile = collision_profile.clone();
            }
            Self::Actor { actor } => {
                doc.scene.actors.retain(|a| a.id != actor.id);
                doc.scene.actors.push(actor.clone());
                doc.scene.actors.sort_by_key(|a| a.id);
            }
            Self::RemoveActor { id } => {
                if !doc.scene.actors.iter().any(|a| a.id == *id) {
                    return Err("Actor ID does not exist".into());
                }
                doc.scene.actors.retain(|a| a.id != *id);
            }
            Self::Placement { id, placement } => {
                doc.placements.insert(*id, placement.clone());
            }
            Self::RemovePlacement { id } => {
                doc.placements
                    .remove(id)
                    .ok_or("Placement ID does not exist")?;
            }
            Self::Cue { cue } => {
                doc.timeline.retain(|c| c.id != cue.id);
                doc.timeline.push(cue.clone());
                doc.timeline.sort_by_key(|c| c.id);
            }
            Self::RemoveCue { id } => {
                if !doc.timeline.iter().any(|c| c.id == *id) {
                    return Err("Cue ID does not exist".into());
                }
                doc.timeline.retain(|c| c.id != *id);
            }
            Self::Model { key, edit } => {
                doc.models.insert(key.clone(), edit.clone());
            }
            Self::Primitive { key, shape } => {
                doc.primitives.insert(key.clone(), shape.clone());
            }
            Self::RemovePrimitive { key } => {
                doc.primitives
                    .remove(key)
                    .ok_or("Primitive model key does not exist")?;
            }
            Self::Authored { settings } => doc.authored = settings.clone(),
            Self::Social { profile } => doc.social_profile = profile.clone(),
            Self::Quest { quest } => {
                doc.progression.quests.retain(|q| q.id != quest.id);
                doc.progression.quests.push(quest.clone());
                doc.progression.quests.sort_by_key(|q| q.id);
            }
            Self::RemoveQuest { id } => {
                if !doc.progression.quests.iter().any(|q| q.id == *id) {
                    return Err("Quest ID does not exist".into());
                }
                doc.progression.quests.retain(|q| q.id != *id);
            }
            Self::Rewards { policies } => doc.rewards = policies.clone(),
            Self::Levels { thresholds } => doc.progression.levels = thresholds.clone(),
            Self::Items { catalog } => doc.items = catalog.clone(),
            Self::Outfits { catalog } => doc.outfits = catalog.clone(),
            Self::Equipment { catalog } => doc.equipment = catalog.clone(),
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction {
    pub expected_revision: u64,
    pub label: String,
    pub edits: Vec<Edit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub schema: String,
    pub asset_digest: String,
    pub revision: u64,
    pub document: Document,
    pub undo: Vec<Document>,
    pub redo: Vec<Document>,
    pub last_label: String,
}
