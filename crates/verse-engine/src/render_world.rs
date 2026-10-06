//! Read-only presentation frames shared by interactive drawing and capture.
use crate::{
    lighting::Lighting,
    overlay::{ResolvedOverlay, Vertex},
    presentation::{Instance, ResolvedInstances, View},
    residency::Catalog,
};

/// One admitted frame. Borrowing prevents mutation of its presentation inputs.
/// World authority and platform resources do not enter this contract.
#[derive(Clone, Debug)]
pub struct RenderWorld<'a> {
    view: View,
    lighting: &'a Lighting,
    instances: ResolvedInstances<'a>,
    overlay: ResolvedOverlay<'a>,
    animation: Option<Animation<'a>>,
}
impl<'a> RenderWorld<'a> {
    pub fn extract(
        catalog: &Catalog,
        view: View,
        instances: &'a [Instance],
        overlay: &'a [Vertex],
        lighting: &'a Lighting,
    ) -> Result<Self, String> {
        let instances = ResolvedInstances::extract(catalog, instances)?;
        Self::from_resolved(catalog, view, &instances, overlay, lighting)
    }

    /// Preserve typed model handles produced by an earlier extraction stage.
    pub fn from_resolved(
        catalog: &Catalog,
        view: View,
        instances: &ResolvedInstances<'a>,
        overlay: &'a [Vertex],
        lighting: &'a Lighting,
    ) -> Result<Self, String> {
        instances.validate(catalog)?;
        lighting.validate(view)?;
        let overlay = ResolvedOverlay::extract(catalog, overlay)?;
        Ok(Self {
            view,
            lighting,
            instances: instances.clone(),
            overlay,
            animation: None,
        })
    }

    pub fn with_animation(
        mut self,
        support: &'a dyn crate::locomotion::Support,
        controls: &'a [crate::locomotion::Control],
    ) -> Result<Self, String> {
        if controls.len() > 4096 {
            return Err("Animation controls exceed the frame budget".into());
        }
        let mut lives = std::collections::BTreeSet::new();
        for control in controls {
            control.aim.validate()?;
            if !lives.insert(control.life)
                || !self
                    .instances
                    .instances()
                    .iter()
                    .any(|i| i.actor == Some(control.life))
            {
                return Err("Animation controls must name unique current actor lives".into());
            }
        }
        self.animation = Some(Animation { support, controls });
        Ok(self)
    }
    pub fn animation(&self) -> Option<Animation<'a>> {
        self.animation
    }
    /// Check the complete immutable frame before any resource writes.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        self.instances.validate(catalog)?;
        self.overlay.validate(catalog)
    }
    pub fn view(&self) -> View {
        self.view
    }
    pub fn lighting(&self) -> &'a Lighting {
        self.lighting
    }
    pub fn instances(&self) -> &ResolvedInstances<'a> {
        &self.instances
    }
    pub fn overlay(&self) -> &ResolvedOverlay<'a> {
        &self.overlay
    }
}

#[derive(Clone, Copy)]
pub struct Animation<'a> {
    pub support: &'a dyn crate::locomotion::Support,
    pub controls: &'a [crate::locomotion::Control],
}
impl std::fmt::Debug for Animation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("controls", &self.controls)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};
    fn catalog() -> Catalog {
        let pack = serde_json::from_value(serde_json::json!({
            "version":1,"source_revision":"test","textures":[],"models":{
                "room":{"source":"authored","source_sha256":"","surfaces":[],"bones":[],"clips":[{"id":0,"duration":1,"bones":[]}],"states":{"idle":{"clip":0,"mode":"loop","transition_seconds":0.1}},"height":1,"attachments":[]}
            }
        })).unwrap();
        Catalog::new(&pack).unwrap()
    }
    fn view() -> View {
        View {
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
        }
    }
    fn instance() -> Instance {
        Instance {
            mount: None,
            actor: None,
            model: "room".into(),
            transform: Mat4::IDENTITY,
            animation: crate::motion::State::Idle.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ONE,
        }
    }
    #[test]
    fn frame_borrows_inputs_and_fences_all_catalog_replacements() {
        let replacement = catalog();
        let catalog = catalog();
        let lighting = Lighting::default();
        let instances = [instance()];
        let vertices = [Vertex {
            pos: [0.; 2],
            uv: [0.; 2],
            color: [1.; 4],
        }; 3];
        for (source, overlay) in [(&instances[..], &vertices[..]), (&[][..], &[][..])] {
            let frame = RenderWorld::extract(&catalog, view(), source, overlay, &lighting).unwrap();
            assert!(std::ptr::eq(frame.lighting(), &lighting));
            assert!(std::ptr::eq(
                frame.instances().instances().as_ptr(),
                source.as_ptr()
            ));
            assert!(std::ptr::eq(
                frame.overlay().vertices().as_ptr(),
                overlay.as_ptr()
            ));
            assert!(frame.validate(&catalog).is_ok());
            assert!(frame.validate(&replacement).is_err());
            assert!(
                RenderWorld::from_resolved(
                    &replacement,
                    view(),
                    frame.instances(),
                    overlay,
                    &lighting
                )
                .is_err()
            );
        }
    }
    #[test]
    fn no_invalid_input_produces_an_admitted_world() {
        let catalog = catalog();
        let lighting = Lighting::default();
        let mut camera = view();
        camera.eye.x = f32::NAN;
        assert!(RenderWorld::extract(&catalog, camera, &[], &[], &lighting).is_err());
        let mut bad_lighting = lighting.clone();
        bad_lighting.exposure = 0.;
        assert!(RenderWorld::extract(&catalog, view(), &[], &[], &bad_lighting).is_err());
        let mut source = instance();
        source.model = "missing".into();
        assert!(RenderWorld::extract(&catalog, view(), &[source], &[], &lighting).is_err());
        let vertices = [Vertex {
            pos: [0.; 2],
            uv: [0.; 2],
            color: [1.; 4],
        }; 2];
        assert!(RenderWorld::extract(&catalog, view(), &[], &vertices, &lighting).is_err());
    }
}
