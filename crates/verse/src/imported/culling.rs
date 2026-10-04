//! Conservative posed bounds for cube-shadow submission.
use super::Pose;
use glam::{Mat4, Vec3};
use verse_engine::assets::Model;

#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}
impl Bounds {
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let mut bounds = Self::point(points.next()?);
        for point in points {
            bounds.include(point);
        }
        Some(bounds)
    }
    fn point(point: Vec3) -> Self {
        Self {
            min: point,
            max: point,
        }
    }
    fn include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
    pub fn visible(self, view: Mat4) -> bool {
        let rows = view.transpose();
        let planes = [
            rows.w_axis + rows.x_axis,
            rows.w_axis - rows.x_axis,
            rows.w_axis + rows.y_axis,
            rows.w_axis - rows.y_axis,
            rows.z_axis,
            rows.w_axis - rows.z_axis,
        ];
        let center = (self.min + self.max) * 0.5;
        let half = (self.max - self.min) * 0.5;
        planes
            .into_iter()
            .all(|p| p.truncate().dot(center) + p.w + p.truncate().abs().dot(half) >= -0.0001)
    }
}
/// Every skinned vertex is a positive weighted combination of bone-transformed
/// points. Their union bounds contain the geometry without reskinning vertices.
pub(super) struct BoneBounds(Vec<(usize, Bounds)>);
impl BoneBounds {
    pub fn compile(model: &Model) -> Option<Self> {
        let mut bones: [Option<Bounds>; 256] = [None; 256];
        for vertex in model.surfaces.iter().flat_map(|s| &s.vertices) {
            let sum = vertex.weights.iter().sum::<f32>();
            if vertex.weights.iter().any(|w| *w < 0.) || !sum.is_finite() || sum <= 0. {
                return None;
            }
            for (joint, weight) in vertex.joints.iter().zip(vertex.weights) {
                if weight == 0. {
                    continue;
                }
                let bound = bones.get_mut(*joint as usize)?;
                let point = Vec3::from(vertex.position);
                match bound {
                    Some(b) => b.include(point),
                    None => *bound = Some(Bounds::point(point)),
                }
            }
        }
        Some(Self(
            bones
                .into_iter()
                .enumerate()
                .filter_map(|(i, b)| b.map(|b| (i, b)))
                .collect(),
        ))
    }
    pub fn posed(&self, pose: &Pose) -> Option<Bounds> {
        if pose.params[0] != 0. {
            return None;
        }
        let model = Mat4::from_cols_array_2d(&pose.model);
        let mut world: Option<Bounds> = None;
        for (joint, bound) in &self.0 {
            let transform = model * Mat4::from_cols_array_2d(&pose.bones[*joint]);
            if transform.x_axis.w != 0.
                || transform.y_axis.w != 0.
                || transform.z_axis.w != 0.
                || transform.w_axis.w != 1.
            {
                return None;
            }
            for x in [bound.min.x, bound.max.x] {
                for y in [bound.min.y, bound.max.y] {
                    for z in [bound.min.z, bound.max.z] {
                        let point = transform.transform_point3(Vec3::new(x, y, z));
                        match &mut world {
                            Some(b) => b.include(point),
                            None => world = Some(Bounds::point(point)),
                        }
                    }
                }
            }
        }
        world
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bone_bounds_contain_weighted_skinning_and_refuse_unsafe_culling() {
        use glam::Vec4;
        use verse_engine::assets::{Surface, Vertex};
        let mut model = Model {
            markers: Vec::new(),
            states: Default::default(),
            skin: None,
            source: "fixture".into(),
            source_sha256: String::new(),
            height: 2.,
            bones: vec![],
            clips: vec![],
            attachments: vec![],
            surfaces: vec![Surface {
                material: Default::default(),
                vertices: [[-2., 0., 1.], [1., 2., -1.], [0., -1., 0.]]
                    .into_iter()
                    .map(|position| Vertex {
                        position,
                        normal: [0., 1., 0.],
                        uv: [0.; 2],
                        joints: [0, 1, 0, 0],
                        weights: [0.6, 1.4, 0., 0.],
                    })
                    .collect(),
                indices: vec![0, 1, 2],
                texture: 0,
                blend: 0,
                emissive: false,
                tint: [1.; 3],
            }],
        };
        let bounds = BoneBounds::compile(&model).unwrap();
        let mut pose = Pose {
            model: Mat4::from_translation(Vec3::new(100., 20., -30.)).to_cols_array_2d(),
            params: [0.; 4],
            bones: [Mat4::IDENTITY.to_cols_array_2d(); 256],
        };
        for angle in [0., 0.3, 1.2, 2.8] {
            pose.bones[0] = (Mat4::from_translation(-Vec3::X * 3.) * Mat4::from_rotation_y(angle))
                .to_cols_array_2d();
            pose.bones[1] = (Mat4::from_translation(Vec3::X * 2.)
                * Mat4::from_scale(Vec3::new(2., 0.5, 1.)))
            .to_cols_array_2d();
            let world = bounds.posed(&pose).unwrap();
            for vertex in &model.surfaces[0].vertices {
                let point = Vec3::from(vertex.position).extend(1.);
                let skin = vertex
                    .joints
                    .iter()
                    .zip(vertex.weights)
                    .fold(Vec4::ZERO, |p, (j, w)| {
                        p + Mat4::from_cols_array_2d(&pose.bones[*j as usize]) * point * w
                    });
                let point = Mat4::from_cols_array_2d(&pose.model) * skin;
                let projected = point.truncate() / point.w;
                assert!(projected.cmpge(world.min - Vec3::splat(0.0001)).all());
                assert!(projected.cmple(world.max + Vec3::splat(0.0001)).all());
            }
        }
        pose.params[0] = 2.;
        assert!(bounds.posed(&pose).is_none());
        pose.params[0] = 0.;
        pose.model[0][3] = 0.1;
        assert!(bounds.posed(&pose).is_none());
        model.surfaces[0].vertices[0].weights[0] = -1.;
        assert!(BoneBounds::compile(&model).is_none());
    }
    #[test]
    fn cube_faces_keep_crossing_bounds_and_reject_only_separated_geometry() {
        let view = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1., 0.1, 30.)
            * Mat4::look_at_rh(Vec3::ZERO, Vec3::X, Vec3::Y);
        let ahead = Bounds {
            min: Vec3::new(2., -0.5, -0.5),
            max: Vec3::new(3., 0.5, 0.5),
        };
        assert!(ahead.visible(view));
        let behind = Bounds {
            min: -ahead.max,
            max: -ahead.min,
        };
        assert!(!behind.visible(view));
        let crossing = Bounds {
            min: Vec3::splat(-1.),
            max: Vec3::splat(1.),
        };
        assert!(crossing.visible(view));
        let distant = Bounds {
            min: Vec3::new(31., -0.5, -0.5),
            max: Vec3::new(32., 0.5, 0.5),
        };
        assert!(!distant.visible(view));
        let seam = Bounds {
            min: Vec3::new(2., -0.2, 1.9),
            max: Vec3::new(3., 0.2, 3.1),
        };
        assert!(seam.visible(view));
    }
}
