//! Released collision without editor state, used by the native world server.
use crate::{
    Collider, EPS, MAX_COLLIDERS, RADIUS, can_occupy_height, finite,
    static_collision::{StaticGeometry, Vertices},
    sweep_box, valid_collider,
};
use af_mmorpg_core::{Surface, V3};

#[derive(Clone, Default)]
pub struct CollisionScene {
    ground: f32,
    colliders: Vec<Collider>,
    triangles: StaticGeometry,
}
impl CollisionScene {
    pub fn new(ground: f32, colliders: Vec<Collider>) -> Result<Self, &'static str> {
        if !ground.is_finite()
            || colliders.len() > MAX_COLLIDERS
            || colliders.iter().any(|c| !valid_collider(c))
        {
            return Err("Choose valid released collision surfaces.");
        }
        Ok(Self {
            ground,
            colliders,
            triangles: StaticGeometry::default(),
        })
    }
    pub fn append_triangles(&mut self, triangles: Vec<Vertices>) -> Result<(), &'static str> {
        self.triangles.append_owned(triangles, None)
    }
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
}
impl Surface for CollisionScene {
    fn height(&self, _: f32, _: f32) -> f32 {
        self.ground
    }
    fn support(&self, p: V3, max_y: f32) -> f32 {
        let mut height = self.ground;
        for c in &self.colliders {
            if c.max[1] > max_y + EPS {
                continue;
            }
            let dx = p[0] - p[0].clamp(c.min[0], c.max[0]);
            let dz = p[2] - p[2].clamp(c.min[2], c.max[2]);
            if dx * dx + dz * dz < RADIUS * RADIUS {
                height = height.max(c.max[1]);
            }
        }
        self.triangles
            .support_height(p, max_y, RADIUS)
            .map_or(height, |y| height.max(y))
    }
    fn step_support(&self, p: V3, max_y: f32) -> f32 {
        self.triangles
            .step_height(p, max_y, RADIUS)
            .map_or(self.support(p, max_y), |y| self.support(p, max_y).max(y))
    }
    fn can_occupy(&self, p: V3, height: f32) -> bool {
        finite(&p)
            && can_occupy_height(p, &self.colliders, height)
            && self.triangles.can_occupy(p, RADIUS, height)
    }
    fn slide_down(&self, p: V3, height: f32) -> Option<V3> {
        self.triangles.slide_down(p, RADIUS, height)
    }
    fn clear(&self, from: V3, to: V3) -> bool {
        let delta: V3 = std::array::from_fn(|i| to[i] - from[i]);
        let length = delta.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length < 0.02 {
            return true;
        }
        self.triangles
            .raycast(from, delta.map(|v| v / length), length - 0.02)
            .is_none()
            && self
                .colliders
                .iter()
                .all(|c| sweep_box(from, to, 0., c.min, c.max) >= 1.)
    }
}

impl Surface for crate::Engine {
    fn height(&self, _: f32, _: f32) -> f32 {
        self.world.ground
    }
    fn support(&self, p: V3, max_y: f32) -> f32 {
        self.support_height(p, max_y)
    }
    fn step_support(&self, p: V3, max_y: f32) -> f32 {
        self.step_support_height(p, max_y)
    }
    fn can_occupy(&self, p: V3, height: f32) -> bool {
        self.can_stand_height(p, height)
    }
    fn slide_down(&self, p: V3, height: f32) -> Option<V3> {
        self.static_geometry.slide_down(p, RADIUS, height)
    }
    fn clear(&self, from: V3, to: V3) -> bool {
        self.spell_path_clear(from, to)
    }
}

pub(crate) struct PredictionSurface<'a>(pub &'a crate::Engine);
impl Surface for PredictionSurface<'_> {
    fn height(&self, x: f32, z: f32) -> f32 {
        af_mmorpg_core::terrain::height(x as f64, z as f64) as f32
    }
    fn support(&self, p: V3, max_y: f32) -> f32 {
        self.height(p[0], p[2]).max(self.0.support_height(p, max_y))
    }
    fn step_support(&self, p: V3, max_y: f32) -> f32 {
        self.support(p, max_y)
            .max(self.0.step_support_height(p, max_y))
    }
    fn can_occupy(&self, p: V3, height: f32) -> bool {
        p[1] >= self.height(p[0], p[2]) - EPS
            && self.0.can_stand_height(p, height)
            && !af_mmorpg_core::scenery::tree_blocks(p[0], p[2])
    }
    fn slide_down(&self, p: V3, height: f32) -> Option<V3> {
        self.0.static_geometry.slide_down(p, RADIUS, height)
    }
    fn clear(&self, a: V3, b: V3) -> bool {
        self.0.spell_path_clear(a, b)
            && af_mmorpg_core::height_line_clear(a, b, |x, z| self.height(x, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn released_collision_has_height_aware_contacts_and_cover_without_an_editor() {
        let scene = CollisionScene::new(
            0.,
            vec![Collider {
                id: "roof".into(),
                min: [-1., 1.4, -1.],
                max: [1., 1.5, 1.],
            }],
        )
        .unwrap();
        assert!(scene.can_occupy([0.; 3], 1.2));
        assert!(!scene.can_occupy([0.; 3], crate::HEIGHT));
        assert!(!scene.clear([0., 2., 0.], [0., 0., 0.]));
    }
}
