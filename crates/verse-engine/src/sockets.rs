//! Bind-space equipment frames from a caller's evaluated animation palette.
use crate::assets::{Attachment, Model};
use glam::{Mat4, Vec3};
use std::collections::BTreeSet;

pub const MAX_SOCKETS: usize = 64;
fn affine(matrix: Mat4) -> bool {
    matrix.is_finite()
        && matrix.x_axis.w.abs() <= 0.00001
        && matrix.y_axis.w.abs() <= 0.00001
        && matrix.z_axis.w.abs() <= 0.00001
        && (matrix.w_axis.w - 1.).abs() <= 0.00001
}
/// Borrows immutable socket metadata after checking unique IDs and bone bounds.
#[derive(Clone, Copy, Debug)]
pub struct Sockets<'a> {
    model: &'a Model,
    bindings: &'a [Attachment],
}
/// Borrows evaluated bind-space skin matrices, including inverse-bind correction.
/// This is not a palette of local joint transforms.
#[derive(Clone, Copy, Debug)]
pub struct Palette<'a> {
    model: &'a Model,
    matrices: &'a [Mat4],
}
impl<'a> Palette<'a> {
    pub fn admit(model: &'a Model, matrices: &'a [Mat4]) -> Result<Self, String> {
        if model.bones.len() > 256
            || matrices.len() != model.bones.len().max(1)
            || matrices.iter().any(|m| !affine(*m))
        {
            return Err("Invalid equipment socket pose palette".into());
        }
        Ok(Self { model, matrices })
    }
}
impl<'a> Sockets<'a> {
    pub fn admit(model: &'a Model) -> Result<Self, String> {
        let mut ids = BTreeSet::new();
        if model.bones.len() > 256
            || model.attachments.len() > MAX_SOCKETS
            || model.attachments.iter().any(|a| {
                a.bone >= model.bones.len()
                    || !ids.insert(a.id)
                    || a.position.iter().any(|p| !p.is_finite())
            })
        {
            return Err("Invalid or ambiguous equipment socket metadata".into());
        }
        Ok(Self {
            model,
            bindings: &model.attachments,
        })
    }
    /// Applies the parent transform, evaluated skin matrix, bind-space socket
    /// position, then the equipment's authored local transform.
    pub fn frame(
        &self,
        palette: Palette<'_>,
        parent: Mat4,
        id: u16,
        local: Mat4,
    ) -> Result<Mat4, String> {
        if !std::ptr::eq(palette.model, self.model) || !affine(parent) || !affine(local) {
            return Err("Equipment socket palette or transform is incompatible".into());
        }
        let socket = self
            .bindings
            .iter()
            .find(|a| a.id == id)
            .ok_or("Equipment socket is missing")?;
        let frame = parent
            * palette.matrices[socket.bone]
            * Mat4::from_translation(socket.position.into())
            * local;
        if !affine(frame) {
            return Err("Equipment socket frame overflowed".into());
        }
        Ok(frame)
    }
    pub fn point(&self, palette: Palette<'_>, parent: Mat4, id: u16) -> Result<Vec3, String> {
        Ok(self
            .frame(palette, parent, id, Mat4::IDENTITY)?
            .transform_point3(Vec3::ZERO))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Bone;
    use glam::Quat;
    fn model() -> Model {
        Model {
            graph: None,
            markers: vec![],
            states: Default::default(),
            skin: None,
            source: "socket-fixture".into(),
            source_sha256: String::new(),
            surfaces: vec![],
            bones: vec![
                Bone {
                    parent: -1,
                    pivot: [0.; 3],
                },
                Bone {
                    parent: 0,
                    pivot: [0.; 3],
                },
            ],
            clips: vec![],
            height: 1.,
            attachments: vec![Attachment {
                id: 5,
                bone: 1,
                position: [1., 0., 0.],
            }],
        }
    }
    fn close(actual: Vec3, expected: Vec3) {
        assert!(
            (actual - expected).length() < 0.0001,
            "{actual:?} != {expected:?}"
        );
    }
    #[test]
    fn supplied_skin_palette_rotates_translates_and_scales_equipment_frames() {
        let model = model();
        let sockets = Sockets::admit(&model).unwrap();
        let matrices = [
            Mat4::IDENTITY,
            Mat4::from_scale_rotation_translation(
                Vec3::new(1., 2., 3.),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                Vec3::new(0., 2., 0.),
            ),
        ];
        let palette = Palette::admit(&model, &matrices).unwrap();
        let parent = Mat4::from_rotation_translation(
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(10., 0., 0.),
        );
        close(
            sockets.point(palette, parent, 5).unwrap(),
            Vec3::new(10., 3., 0.),
        );
        let frame = sockets
            .frame(palette, parent, 5, Mat4::from_translation(Vec3::Y))
            .unwrap();
        close(frame.transform_point3(Vec3::ZERO), Vec3::new(10., 3., 2.));
        close(frame.transform_vector3(Vec3::X), Vec3::Y);
        close(frame.transform_vector3(Vec3::Y), Vec3::Z * 2.);
        close(frame.transform_vector3(Vec3::Z), Vec3::X * 3.);
    }
    #[test]
    fn malformed_metadata_palettes_and_derived_overflow_are_refused() {
        let model = model();
        let matrices = [Mat4::IDENTITY; 2];
        let palette = Palette::admit(&model, &matrices).unwrap();
        let sockets = Sockets::admit(&model).unwrap();
        assert!(sockets.point(palette, Mat4::IDENTITY, 9).is_err());
        assert!(
            sockets
                .frame(
                    palette,
                    Mat4::IDENTITY,
                    5,
                    Mat4::perspective_rh(1., 1., 0.1, 100.)
                )
                .is_err()
        );
        assert!(
            sockets
                .frame(
                    palette,
                    Mat4::from_scale(Vec3::splat(f32::MAX)),
                    5,
                    Mat4::from_scale(Vec3::splat(2.))
                )
                .is_err()
        );
        assert!(Palette::admit(&model, &matrices[..1]).is_err());
        assert!(
            Palette::admit(
                &model,
                &[Mat4::IDENTITY, Mat4::from_cols_array(&[f32::NAN; 16])]
            )
            .is_err()
        );
        for case in 0..4 {
            let mut bad = model.clone();
            match case {
                0 => bad.attachments.push(bad.attachments[0].clone()),
                1 => bad.attachments[0].bone = 2,
                2 => bad.attachments[0].position[0] = f32::INFINITY,
                _ => bad.attachments = vec![bad.attachments[0].clone(); 65],
            };
            assert!(Sockets::admit(&bad).is_err());
        }
        let other = model.clone();
        let other = Sockets::admit(&other).unwrap();
        assert!(other.point(palette, Mat4::IDENTITY, 5).is_err());
        let mut smaller = model.clone();
        smaller.bones.pop();
        smaller.attachments[0].bone = 0;
        let smaller = Sockets::admit(&smaller).unwrap();
        assert!(smaller.point(palette, Mat4::IDENTITY, 5).is_err());
        assert_eq!(matrices, [Mat4::IDENTITY; 2]);
    }
}
