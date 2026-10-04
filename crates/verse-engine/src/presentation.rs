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

/// Presentation values contain no GPU resources or mutable simulation authority.
#[derive(Clone, Debug)]
pub struct Instance {
    pub actor: Option<LifeId>,
    pub model: String,
    pub transform: Mat4,
    pub animation: Selection,
    pub time: f32,
    pub emission: Vec3,
}

/// A frame borrows its source values and retains handles from one asset catalog.
#[derive(Clone, Debug)]
pub struct ResolvedInstances<'a> {
    catalog: CatalogId,
    instances: &'a [Instance],
    models: Vec<ModelHandle>,
}
impl<'a> ResolvedInstances<'a> {
    pub const MAX_INSTANCES: usize = 256;

    pub fn extract(catalog: &Catalog, instances: &'a [Instance]) -> Result<Self, String> {
        if instances.len() > Self::MAX_INSTANCES {
            return Err("Presentation frame exceeds 256 instances".into());
        }
        if instances
            .iter()
            .any(|i| !i.transform.is_finite() || !i.time.is_finite() || !i.emission.is_finite())
        {
            return Err("Presentation frame contains nonfinite values".into());
        }
        let models = instances
            .iter()
            .map(|i| catalog.model(&i.model))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            catalog: catalog.id(),
            instances,
            models,
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
                "room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[],"clips":[],"height":1,"attachments":[]}
            }
        })).unwrap();
        Catalog::new(&pack).unwrap()
    }
    fn instance() -> Instance {
        Instance {
            actor: None,
            model: "room".into(),
            transform: Mat4::IDENTITY,
            animation: Selection::Legacy(0),
            time: 0.,
            emission: Vec3::ZERO,
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
        assert!(ResolvedInstances::extract(&catalog, &vec![instance(); 257]).is_err());
        assert!(ResolvedInstances::extract(&catalog, &vec![instance(); 256]).is_ok());
    }
}
