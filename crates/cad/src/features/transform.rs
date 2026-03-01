use crate::hash::stable_hex_digest;
use crate::{CadError, CadResult};

/// Feature op: transform an existing feature output using translation/rotation/scale.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub translation_mm: [f64; 3],
    pub rotation_deg_xyz: [f64; 3],
    pub scale_xyz: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub geometry_hash: String,
    pub matrix_row_major: [f64; 16],
}

impl TransformFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "transform feature ids must not be empty".to_string(),
            });
        }
        for (axis, value) in ["x", "y", "z"].iter().zip(self.scale_xyz) {
            if !value.is_finite() || value <= 0.0 {
                return Err(CadError::InvalidPrimitive {
                    reason: format!("transform scale {axis} must be finite and > 0"),
                });
            }
        }
        for value in self
            .translation_mm
            .into_iter()
            .chain(self.rotation_deg_xyz)
            .chain(self.scale_xyz)
        {
            if !value.is_finite() {
                return Err(CadError::InvalidPrimitive {
                    reason: "transform components must be finite".to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn matrix_row_major(&self) -> [f64; 16] {
        // Compose as T * Rz * Ry * Rx * S.
        let sx = self.scale_xyz[0];
        let sy = self.scale_xyz[1];
        let sz = self.scale_xyz[2];

        let rx = self.rotation_deg_xyz[0].to_radians();
        let ry = self.rotation_deg_xyz[1].to_radians();
        let rz = self.rotation_deg_xyz[2].to_radians();

        let cx = rx.cos();
        let sxr = rx.sin();
        let cy = ry.cos();
        let syr = ry.sin();
        let cz = rz.cos();
        let szr = rz.sin();

        let scale = [
            sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let rot_x = [
            1.0, 0.0, 0.0, 0.0, 0.0, cx, -sxr, 0.0, 0.0, sxr, cx, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let rot_y = [
            cy, 0.0, syr, 0.0, 0.0, 1.0, 0.0, 0.0, -syr, 0.0, cy, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let rot_z = [
            cz, -szr, 0.0, 0.0, szr, cz, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let translate = [
            1.0,
            0.0,
            0.0,
            self.translation_mm[0],
            0.0,
            1.0,
            0.0,
            self.translation_mm[1],
            0.0,
            0.0,
            1.0,
            self.translation_mm[2],
            0.0,
            0.0,
            0.0,
            1.0,
        ];

        mat_mul(
            &translate,
            &mat_mul(&rot_z, &mat_mul(&rot_y, &mat_mul(&rot_x, &scale))),
        )
    }

    pub fn geometry_hash(&self, source_geometry_hash: &str) -> String {
        let matrix = self.matrix_row_major();
        let payload = format!(
            "transform|feature={}|source={}|src_hash={}|tx={:.6}|ty={:.6}|tz={:.6}|rx={:.6}|ry={:.6}|rz={:.6}|sx={:.6}|sy={:.6}|sz={:.6}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            self.translation_mm[0],
            self.translation_mm[1],
            self.translation_mm[2],
            self.rotation_deg_xyz[0],
            self.rotation_deg_xyz[1],
            self.rotation_deg_xyz[2],
            self.scale_xyz[0],
            self.scale_xyz[1],
            self.scale_xyz[2],
        );
        let matrix_payload = matrix
            .iter()
            .map(|value| format!("{value:.8}"))
            .collect::<Vec<_>>()
            .join(",");
        stable_hex_digest(format!("{payload}|m={matrix_payload}").as_bytes())
    }
}

pub fn evaluate_transform_feature(
    op: &TransformFeatureOp,
    source_geometry_hash: &str,
) -> CadResult<TransformFeatureResult> {
    op.validate()?;
    let matrix = op.matrix_row_major();
    Ok(TransformFeatureResult {
        feature_id: op.feature_id.clone(),
        source_feature_id: op.source_feature_id.clone(),
        geometry_hash: op.geometry_hash(source_geometry_hash),
        matrix_row_major: matrix,
    })
}

pub fn compose_transform_sequence(ops: &[TransformFeatureOp]) -> CadResult<[f64; 16]> {
    let mut composed = identity_matrix();
    for op in ops {
        op.validate()?;
        let matrix = op.matrix_row_major();
        composed = mat_mul(&composed, &matrix);
    }
    Ok(composed)
}

fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat_mul(lhs: &[f64; 16], rhs: &[f64; 16]) -> [f64; 16] {
    let mut out = [0.0_f64; 16];
    for row in 0..4 {
        for col in 0..4 {
            out[row * 4 + col] = lhs[row * 4] * rhs[col]
                + lhs[row * 4 + 1] * rhs[4 + col]
                + lhs[row * 4 + 2] * rhs[8 + col]
                + lhs[row * 4 + 3] * rhs[12 + col];
        }
    }
    out
}
