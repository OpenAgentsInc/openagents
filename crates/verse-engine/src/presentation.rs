//! Portable instance presentation and immutable, catalog-bound frame extraction.
use crate::{
    core::LifeId,
    motion::Selection,
    residency::{Catalog, CatalogId, ModelHandle},
};
use glam::{Mat4, Vec3};

/// A camera projection and eye position in the presentation coordinate frame.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub view_proj: Mat4,
    pub eye: Vec3,
}
impl View {
    pub fn validate(self) -> Result<(), String> {
        if !self.view_proj.is_finite() || !self.eye.is_finite() {
            return Err("Camera contains nonfinite values".into());
        }
        Ok(())
    }
}

/// A leaf attachment names one exact parent life and render model in this frame.
#[derive(Clone, Debug)]
pub struct Mount {
    pub parent: LifeId,
    pub parent_model: String,
    pub socket: u16,
    pub local: Mat4,
}
/// Presentation values contain no GPU resources or mutable simulation authority.
#[derive(Clone, Debug)]
pub struct Instance {
    pub mount: Option<Mount>,
    pub actor: Option<LifeId>,
    pub model: String,
    pub transform: Mat4,
    pub animation: Selection,
    pub time: f32,
    pub emission: Vec3,
}

/// Stable optional-visual selection. Actor roots and mounts cannot be omitted.
#[derive(Clone, Debug)]
pub struct VisualSelection<'a> {
    instances: &'a [Instance],
    indices: Vec<usize>,
    pub dropped_effects: usize,
}
impl<'a> VisualSelection<'a> {
    pub const MAX_SOURCE_INSTANCES: usize = 8192;
    /// Smaller priorities retain earlier optional effects; equal priorities keep source order.
    pub fn admit(
        catalog: &Catalog,
        instances: &'a [Instance],
        budget: crate::quality::Budget,
        optional: impl Fn(&Instance) -> Option<u8>,
    ) -> Result<Self, String> {
        if instances.len() > Self::MAX_SOURCE_INSTANCES {
            return Err("Visual source exceeds 8192 instances".into());
        }
        let mut required = Vec::new();
        let mut effects = Vec::new();
        for (index, instance) in instances.iter().enumerate() {
            if !instance.transform.is_finite()
                || !instance.time.is_finite()
                || instance.time < 0.
                || !instance.emission.is_finite()
            {
                return Err("Visual source contains invalid instance values".into());
            }
            let model = catalog.model(&instance.model)?;
            catalog.check_animation(model, instance.animation)?;
            if let Some(priority) = optional(instance) {
                if instance.actor.is_some() || instance.mount.is_some() {
                    return Err("Actor roots and mounts cannot be optional effects".into());
                }
                effects.push((priority, index));
            } else {
                required.push(index);
            }
        }
        if required.len() > budget.instances {
            return Err("Required visuals exceed the admitted instance budget".into());
        }
        effects.sort_unstable();
        let count = effects
            .len()
            .min(budget.optional_effects)
            .min(budget.instances - required.len());
        let dropped_effects = effects.len() - count;
        required.extend(effects.into_iter().take(count).map(|(_, index)| index));
        required.sort_unstable();
        Ok(Self {
            instances,
            indices: required,
            dropped_effects,
        })
    }
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }
    pub fn retained(&self) -> Vec<Instance> {
        self.indices
            .iter()
            .map(|index| self.instances[*index].clone())
            .collect()
    }
}

/// A frame borrows its source values and retains handles from one asset catalog.
#[derive(Clone, Debug)]
pub struct ResolvedInstances<'a> {
    catalog: CatalogId,
    instances: &'a [Instance],
    models: Vec<ModelHandle>,
    parents: Vec<Option<usize>>,
}
impl<'a> ResolvedInstances<'a> {
    pub const MAX_INSTANCES: usize = 1024;

    pub fn extract(catalog: &Catalog, instances: &'a [Instance]) -> Result<Self, String> {
        if instances.len() > Self::MAX_INSTANCES {
            return Err("Presentation frame exceeds 1024 instances".into());
        }
        if instances.iter().any(|i| {
            !i.transform.is_finite()
                || !i.time.is_finite()
                || i.time < 0.
                || !i.emission.is_finite()
        }) {
            return Err("Presentation frame contains invalid instance values".into());
        }
        let models: Vec<ModelHandle> = instances
            .iter()
            .map(|i| {
                let model = catalog.model(&i.model)?;
                catalog.check_animation(model, i.animation)?;
                Ok(model)
            })
            .collect::<Result<_, String>>()?;
        let mut roots = std::collections::BTreeMap::new();
        for (index, parent) in instances.iter().enumerate() {
            if let Some(life) = parent.actor.filter(|_| parent.mount.is_none()) {
                roots
                    .entry((life, parent.model.as_str()))
                    .and_modify(|entry: &mut (usize, bool)| entry.1 = true)
                    .or_insert((index, false));
            }
        }
        let mut parents = Vec::with_capacity(instances.len());
        for (index, instance) in instances.iter().enumerate() {
            let Some(mount) = &instance.mount else {
                parents.push(None);
                continue;
            };
            if instance.actor.is_some() || !crate::sockets::affine(mount.local) {
                return Err(
                    "Mounted instances must be static leaves with affine local transforms".into(),
                );
            }
            let &(parent, ambiguous) = roots
                .get(&(mount.parent, mount.parent_model.as_str()))
                .ok_or("Attachment parent life or model is missing")?;
            if parent == index || ambiguous {
                return Err("Attachment parent is ambiguous".into());
            }
            if !crate::sockets::affine(instances[parent].transform) {
                return Err("Attachment parent transform must be affine".into());
            }
            catalog.check_socket(models[parent], mount.socket)?;
            parents.push(Some(parent));
        }
        Ok(Self {
            catalog: catalog.id(),
            instances,
            models,
            parents,
        })
    }

    /// Check even empty frames so catalog replacement cannot admit stale work.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        catalog.check(self.catalog)?;
        for handle in &self.models {
            catalog.model_name(*handle)?;
        }
        Ok(())
    }
    pub fn catalog(&self) -> CatalogId {
        self.catalog
    }
    pub fn instances(&self) -> &'a [Instance] {
        self.instances
    }
    pub fn parents(&self) -> &[Option<usize>] {
        &self.parents
    }
    pub fn models(&self) -> &[ModelHandle] {
        &self.models
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        let pack = serde_json::from_value(serde_json::json!({
            "version":1,"source_revision":"test","textures":[],"models":{
                "room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[],"clips":[{"id":0,"duration":1,"bones":[]}],"states":{"idle":{"clip":0,"mode":"loop","transition_seconds":0.1}},"height":1,"attachments":[]}
            }
        })).unwrap();
        Catalog::new(&pack).unwrap()
    }
    fn instance() -> Instance {
        Instance {
            mount: None,
            actor: None,
            model: "room".into(),
            transform: Mat4::IDENTITY,
            animation: Selection::Legacy(0),
            time: 0.,
            emission: Vec3::ZERO,
        }
    }
    #[test]
    fn optional_overflow_preserves_actor_roots_and_mounts_in_source_order() {
        let catalog = catalog();
        let life = LifeId {
            instance: 1,
            actor: 1,
            generation: 1,
        };
        let mut actor = instance();
        actor.actor = Some(life);
        let mut mount = instance();
        mount.mount = Some(Mount {
            parent: life,
            parent_model: "room".into(),
            socket: 1,
            local: Mat4::IDENTITY,
        });
        let mut values = vec![actor, mount];
        for priority in 0..1500 {
            let mut effect = instance();
            effect.time = (priority % 3) as f32;
            values.push(effect);
        }
        let budget = crate::quality::Tier::Low.quality().budget();
        let selected = VisualSelection::admit(&catalog, &values, budget, |value| {
            (value.actor.is_none() && value.mount.is_none()).then_some(value.time as u8)
        })
        .unwrap();
        assert_eq!(selected.indices().len(), budget.optional_effects + 2);
        assert_eq!(selected.dropped_effects, 1500 - budget.optional_effects);
        assert_eq!(&selected.indices()[..2], &[0, 1]);
        assert!(
            selected.indices()[2..]
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            selected.retained()[2..]
                .iter()
                .all(|value| value.time == 0.)
        );
        assert!(VisualSelection::admit(&catalog, &values, budget, |_| Some(0)).is_err());
    }
    #[test]
    fn omitted_visuals_still_require_valid_models_and_values() {
        let catalog = catalog();
        let mut budget = crate::quality::Tier::Low.quality().budget();
        budget.optional_effects = 0;
        let mut invalid = instance();
        invalid.model = "missing".into();
        assert!(VisualSelection::admit(&catalog, &[invalid], budget, |_| Some(0)).is_err());
        let mut invalid = instance();
        invalid.time = f32::NAN;
        assert!(VisualSelection::admit(&catalog, &[invalid], budget, |_| Some(0)).is_err());
        assert!(
            VisualSelection::admit(
                &catalog,
                &vec![instance(); VisualSelection::MAX_SOURCE_INSTANCES + 1],
                budget,
                |_| Some(0)
            )
            .is_err()
        );
    }
    #[test]
    fn modular_battle_frames_remain_bounded_above_the_previous_limit() {
        let catalog = catalog();
        let mut instances = vec![instance(); 600];
        let resolved = ResolvedInstances::extract(&catalog, &instances).unwrap();
        assert_eq!(resolved.models().len(), 600);
        resolved.validate(&catalog).unwrap();
        instances.resize(ResolvedInstances::MAX_INSTANCES + 1, instance());
        assert!(ResolvedInstances::extract(&catalog, &instances).is_err());
    }
    #[test]
    fn mounts_bind_exact_lives_models_and_unique_parents_independent_of_order() {
        let pack=serde_json::from_value(serde_json::json!({"version":1,"source_revision":"test","textures":[],"models":{"room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[{"parent":-1,"pivot":[0,0,0]}],"clips":[],"height":1,"attachments":[{"id":5,"bone":0,"position":[0,0,1]}]}}})).unwrap();
        let catalog = Catalog::new(&pack).unwrap();
        let life = LifeId {
            instance: 1,
            actor: 11,
            generation: 4,
        };
        let mut parent = instance();
        parent.actor = Some(life);
        let mut other = parent.clone();
        other.actor.as_mut().unwrap().actor = 12;
        let mut child = instance();
        child.mount = Some(Mount {
            parent: life,
            parent_model: "room".into(),
            socket: 5,
            local: Mat4::from_translation(Vec3::X),
        });
        let valid = vec![child.clone(), other, parent.clone()];
        let resolved = ResolvedInstances::extract(&catalog, &valid).unwrap();
        assert_eq!(resolved.parents(), &[Some(2), None, None]);
        let replacement = Catalog::new(&pack).unwrap();
        assert!(resolved.validate(&replacement).is_err());
        for case in 0..10 {
            let mut bad = valid.clone();
            match case {
                0 => bad[0].mount.as_mut().unwrap().parent.generation += 1,
                1 => bad[0].mount.as_mut().unwrap().parent.instance += 1,
                2 => bad[0].mount.as_mut().unwrap().parent_model = "missing-model".into(),
                3 => bad[0].mount.as_mut().unwrap().socket = 6,
                4 => bad[0].mount.as_mut().unwrap().local = Mat4::perspective_rh(1., 1., 0.1, 10.),
                5 => bad[0].mount.as_mut().unwrap().local = Mat4::from_scale(Vec3::splat(f32::NAN)),
                6 => bad.push(parent.clone()),
                7 => bad[0].actor = Some(life),
                8 => {
                    bad[2].mount = Some(child.mount.clone().unwrap());
                }
                _ => bad[2].transform = Mat4::perspective_rh(1., 1., 0.1, 10.),
            }
            assert!(
                ResolvedInstances::extract(&catalog, &bad).is_err(),
                "case {case}"
            );
        }
    }
    #[test]
    fn reload_fences_empty_and_populated_frames() {
        let old = catalog();
        let replacement = catalog();
        let source = [instance()];
        for instances in [&source[..], &[][..]] {
            let frame = ResolvedInstances::extract(&old, instances).unwrap();
            assert!(frame.validate(&old).is_ok());
            assert!(frame.validate(&replacement).is_err());
        }
    }
    #[test]
    fn extraction_preserves_life_and_animation_without_copying_source_instances() {
        let catalog = catalog();
        let mut value = instance();
        value.actor = Some(LifeId {
            instance: 8,
            actor: 13,
            generation: 21,
        });
        value.animation = crate::motion::State::Idle.into();
        value.time = 1.25;
        let source = [value];
        let frame = ResolvedInstances::extract(&catalog, &source).unwrap();
        assert!(std::ptr::eq(frame.instances().as_ptr(), source.as_ptr()));
        assert_eq!(frame.instances()[0].actor, source[0].actor);
        assert_eq!(frame.instances()[0].animation, source[0].animation);
        assert_eq!(frame.instances()[0].time, 1.25);
        assert_eq!(frame.catalog(), catalog.id());
        assert_eq!(catalog.model_name(frame.models()[0]).unwrap(), "room");
    }
    #[test]
    fn animation_admission_matches_named_and_compatibility_playback() {
        let catalog = catalog();
        let mut value = instance();
        value.animation = crate::motion::State::Cast.into();
        assert!(ResolvedInstances::extract(&catalog, &[value.clone()]).is_err());
        value.animation = crate::motion::State::Idle.into();
        assert!(ResolvedInstances::extract(&catalog, &[value.clone()]).is_ok());
        value.animation = 65535.into();
        assert!(ResolvedInstances::extract(&catalog, &[value.clone()]).is_ok());
        value.time = -0.01;
        assert!(ResolvedInstances::extract(&catalog, &[value]).is_err());
    }
    #[test]
    fn invalid_inputs_never_produce_an_extracted_frame() {
        let catalog = catalog();
        let mut i = instance();
        i.time = f32::NAN;
        assert!(ResolvedInstances::extract(&catalog, &[i]).is_err());
        let mut i = instance();
        i.emission.x = f32::INFINITY;
        assert!(ResolvedInstances::extract(&catalog, &[i]).is_err());
        let mut i = instance();
        i.transform.x_axis.x = f32::NAN;
        assert!(ResolvedInstances::extract(&catalog, &[i]).is_err());
        let mut i = instance();
        i.model = "absent".into();
        assert!(ResolvedInstances::extract(&catalog, &[i]).is_err());
        assert!(
            ResolvedInstances::extract(
                &catalog,
                &vec![instance(); ResolvedInstances::MAX_INSTANCES + 1]
            )
            .is_err()
        );
        assert!(ResolvedInstances::extract(&catalog, &vec![instance(); 256]).is_ok());
    }
}
