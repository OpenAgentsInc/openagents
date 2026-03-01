use crate::kernel::CadKernelAdapter;
use crate::params::{ParameterStore, ScalarUnit};
use crate::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec, build_primitive};
use crate::{CadError, CadResult};

/// Deterministic feature operation result.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureOpResult<Solid> {
    pub feature_id: String,
    pub geometry_hash: String,
    pub solid: Solid,
}

/// Feature op: parameter-bound primitive box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxFeatureOp {
    pub feature_id: String,
    pub width_param: String,
    pub depth_param: String,
    pub height_param: String,
}

impl BoxFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "box feature id must not be empty".to_string(),
            });
        }
        if self.width_param.trim().is_empty()
            || self.depth_param.trim().is_empty()
            || self.height_param.trim().is_empty()
        {
            return Err(CadError::InvalidPrimitive {
                reason: "box feature parameter bindings must not be empty".to_string(),
            });
        }
        Ok(())
    }

    pub fn resolve_primitive(&self, params: &ParameterStore) -> CadResult<BoxPrimitive> {
        self.validate()?;
        let width_mm = params.get_required_with_unit(&self.width_param, ScalarUnit::Millimeter)?;
        let depth_mm = params.get_required_with_unit(&self.depth_param, ScalarUnit::Millimeter)?;
        let height_mm =
            params.get_required_with_unit(&self.height_param, ScalarUnit::Millimeter)?;

        let primitive = BoxPrimitive {
            width_mm,
            depth_mm,
            height_mm,
        };
        primitive.validate()?;
        Ok(primitive)
    }

    /// Deterministic geometry hash for this operation payload.
    pub fn geometry_hash(&self, primitive: &BoxPrimitive) -> String {
        let payload = format!(
            "box|feature={}|w={:.6}|d={:.6}|h={:.6}|unit=mm",
            self.feature_id, primitive.width_mm, primitive.depth_mm, primitive.height_mm
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
    }
}

pub fn evaluate_box_feature<K: CadKernelAdapter>(
    kernel: &mut K,
    op: &BoxFeatureOp,
    params: &ParameterStore,
) -> CadResult<FeatureOpResult<K::Solid>> {
    let primitive = op.resolve_primitive(params)?;
    let hash = op.geometry_hash(&primitive);
    let solid = build_primitive(kernel, PrimitiveSpec::Box(primitive))?;
    Ok(FeatureOpResult {
        feature_id: op.feature_id.clone(),
        geometry_hash: hash,
        solid,
    })
}

/// Feature op: parameter-bound primitive cylinder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CylinderFeatureOp {
    pub feature_id: String,
    pub radius_param: String,
    pub height_param: String,
}

impl CylinderFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "cylinder feature id must not be empty".to_string(),
            });
        }
        if self.radius_param.trim().is_empty() || self.height_param.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "cylinder feature parameter bindings must not be empty".to_string(),
            });
        }
        Ok(())
    }

    pub fn resolve_primitive(&self, params: &ParameterStore) -> CadResult<CylinderPrimitive> {
        self.validate()?;
        let radius_mm = params.get_required_with_unit(&self.radius_param, ScalarUnit::Millimeter)?;
        let height_mm = params.get_required_with_unit(&self.height_param, ScalarUnit::Millimeter)?;
        let primitive = CylinderPrimitive {
            radius_mm,
            height_mm,
        };
        primitive.validate()?;
        Ok(primitive)
    }

    pub fn geometry_hash(&self, primitive: &CylinderPrimitive) -> String {
        let payload = format!(
            "cylinder|feature={}|r={:.6}|h={:.6}|unit=mm",
            self.feature_id, primitive.radius_mm, primitive.height_mm
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
    }
}

pub fn evaluate_cylinder_feature<K: CadKernelAdapter>(
    kernel: &mut K,
    op: &CylinderFeatureOp,
    params: &ParameterStore,
) -> CadResult<FeatureOpResult<K::Solid>> {
    let primitive = op.resolve_primitive(params)?;
    let hash = op.geometry_hash(&primitive);
    let solid = build_primitive(kernel, PrimitiveSpec::Cylinder(primitive))?;
    Ok(FeatureOpResult {
        feature_id: op.feature_id.clone(),
        geometry_hash: hash,
        solid,
    })
}

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

        let composed = mat_mul(&translate, &mat_mul(&rot_z, &mat_mul(&rot_y, &mat_mul(&rot_x, &scale))));
        composed
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
        format!("{:016x}", fnv1a64(format!("{payload}|m={matrix_payload}").as_bytes()))
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

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{
        BoxFeatureOp, CylinderFeatureOp, TransformFeatureOp, compose_transform_sequence,
        evaluate_box_feature, evaluate_cylinder_feature, evaluate_transform_feature,
    };
    use crate::kernel::CadKernelAdapter;
    use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
    use crate::CadResult;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockKernel {
        box_calls: usize,
        cylinder_calls: usize,
    }

    impl CadKernelAdapter for MockKernel {
        type Solid = &'static str;

        fn create_box(
            &mut self,
            _primitive: &crate::primitives::BoxPrimitive,
        ) -> CadResult<Self::Solid> {
            self.box_calls = self.box_calls.saturating_add(1);
            Ok("solid-box")
        }

        fn create_cylinder(
            &mut self,
            _primitive: &crate::primitives::CylinderPrimitive,
        ) -> CadResult<Self::Solid> {
            self.cylinder_calls = self.cylinder_calls.saturating_add(1);
            Ok("solid-cylinder")
        }
    }

    fn params() -> ParameterStore {
        let mut params = ParameterStore::default();
        params
            .set(
                "width_mm",
                ScalarValue {
                    value: 120.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("width should set");
        params
            .set(
                "depth_mm",
                ScalarValue {
                    value: 200.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("depth should set");
        params
            .set(
                "height_mm",
                ScalarValue {
                    value: 80.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("height should set");
        params
    }

    fn golden_hashes() -> BTreeMap<String, String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{root}/tests/goldens/feature_box_geometry_hashes.json");
        let payload =
            std::fs::read_to_string(&path).expect("feature hash fixture should be readable");
        serde_json::from_str::<BTreeMap<String, String>>(&payload)
            .expect("feature hash fixture should parse")
    }

    fn cylinder_golden_hashes() -> BTreeMap<String, String> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{root}/tests/goldens/feature_cylinder_geometry_hashes.json");
        let payload = std::fs::read_to_string(&path)
            .expect("cylinder feature hash fixture should be readable");
        serde_json::from_str::<BTreeMap<String, String>>(&payload)
            .expect("cylinder feature hash fixture should parse")
    }

    #[test]
    fn box_feature_op_resolves_and_calls_kernel() {
        let mut kernel = MockKernel::default();
        let op = BoxFeatureOp {
            feature_id: "feature.base".to_string(),
            width_param: "width_mm".to_string(),
            depth_param: "depth_mm".to_string(),
            height_param: "height_mm".to_string(),
        };
        let result = evaluate_box_feature(&mut kernel, &op, &params());
        assert!(result.is_ok(), "box feature eval should succeed");
        assert_eq!(kernel.box_calls, 1);
        let result = result.expect("result should be present");
        assert_eq!(result.feature_id, "feature.base");
        assert_eq!(result.solid, "solid-box");
        let golden = golden_hashes();
        assert_eq!(
            result.geometry_hash,
            golden["feature.base|120.000000|200.000000|80.000000"]
        );
    }

    #[test]
    fn box_feature_rejects_invalid_param_units() {
        let mut params = params();
        params
            .set(
                "height_mm",
                ScalarValue {
                    value: 80.0,
                    unit: ScalarUnit::Degree,
                },
            )
            .expect("override should set");

        let op = BoxFeatureOp {
            feature_id: "feature.base".to_string(),
            width_param: "width_mm".to_string(),
            depth_param: "depth_mm".to_string(),
            height_param: "height_mm".to_string(),
        };

        let mut kernel = MockKernel::default();
        let result = evaluate_box_feature(&mut kernel, &op, &params);
        assert!(result.is_err(), "unit mismatch must be rejected");
        assert_eq!(kernel.box_calls, 0);
        assert_eq!(kernel.cylinder_calls, 0);
    }

    #[test]
    fn geometry_hash_matches_second_representative_fixture() {
        let mut params = ParameterStore::default();
        params
            .set(
                "w",
                ScalarValue {
                    value: 180.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("w should set");
        params
            .set(
                "d",
                ScalarValue {
                    value: 210.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("d should set");
        params
            .set(
                "h",
                ScalarValue {
                    value: 95.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("h should set");

        let op = BoxFeatureOp {
            feature_id: "feature.rack".to_string(),
            width_param: "w".to_string(),
            depth_param: "d".to_string(),
            height_param: "h".to_string(),
        };
        let primitive = op.resolve_primitive(&params).expect("primitive should resolve");
        let hash = op.geometry_hash(&primitive);
        let golden = golden_hashes();
        assert_eq!(
            hash,
            golden["feature.rack|180.000000|210.000000|95.000000"]
        );
    }

    #[test]
    fn cylinder_feature_op_resolves_and_calls_kernel() {
        let mut params = ParameterStore::default();
        params
            .set(
                "radius_mm",
                ScalarValue {
                    value: 6.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");
        params
            .set(
                "height_mm",
                ScalarValue {
                    value: 40.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("height should set");

        let op = CylinderFeatureOp {
            feature_id: "feature.mount_post".to_string(),
            radius_param: "radius_mm".to_string(),
            height_param: "height_mm".to_string(),
        };
        let mut kernel = MockKernel::default();
        let result = evaluate_cylinder_feature(&mut kernel, &op, &params)
            .expect("cylinder feature eval should succeed");
        assert_eq!(kernel.cylinder_calls, 1);
        assert_eq!(result.feature_id, "feature.mount_post");
        assert_eq!(result.solid, "solid-cylinder");
        let golden = cylinder_golden_hashes();
        assert_eq!(
            result.geometry_hash,
            golden["feature.mount_post|6.000000|40.000000"]
        );
    }

    #[test]
    fn cylinder_feature_rejects_tolerance_edge_case_radius() {
        let mut params = ParameterStore::default();
        params
            .set(
                "radius_mm",
                ScalarValue {
                    value: crate::policy::BASE_TOLERANCE_MM,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");
        params
            .set(
                "height_mm",
                ScalarValue {
                    value: 10.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("height should set");

        let op = CylinderFeatureOp {
            feature_id: "feature.edge_case".to_string(),
            radius_param: "radius_mm".to_string(),
            height_param: "height_mm".to_string(),
        };
        let mut kernel = MockKernel::default();
        let result = evaluate_cylinder_feature(&mut kernel, &op, &params);
        assert!(result.is_err(), "radius at tolerance must be rejected");
        assert_eq!(kernel.cylinder_calls, 0);
    }

    #[test]
    fn cylinder_geometry_hash_matches_second_representative_fixture() {
        let mut params = ParameterStore::default();
        params
            .set(
                "r",
                ScalarValue {
                    value: 3.2,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("r should set");
        params
            .set(
                "h",
                ScalarValue {
                    value: 18.5,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("h should set");

        let op = CylinderFeatureOp {
            feature_id: "feature.vent_tube".to_string(),
            radius_param: "r".to_string(),
            height_param: "h".to_string(),
        };
        let primitive = op.resolve_primitive(&params).expect("primitive should resolve");
        let hash = op.geometry_hash(&primitive);
        let golden = cylinder_golden_hashes();
        assert_eq!(
            hash,
            golden["feature.vent_tube|3.200000|18.500000"]
        );
    }

    #[test]
    fn transform_feature_rejects_invalid_scale() {
        let op = TransformFeatureOp {
            feature_id: "feature.transform".to_string(),
            source_feature_id: "feature.base".to_string(),
            translation_mm: [0.0, 0.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 0.0],
            scale_xyz: [0.0, 1.0, 1.0],
        };
        let result = evaluate_transform_feature(&op, "abc123");
        assert!(result.is_err(), "zero scale must be rejected");
    }

    #[test]
    fn transform_sequence_is_deterministic_for_same_order() {
        let ops = vec![
            TransformFeatureOp {
                feature_id: "feature.move".to_string(),
                source_feature_id: "feature.base".to_string(),
                translation_mm: [12.0, 0.0, -4.0],
                rotation_deg_xyz: [0.0, 45.0, 0.0],
                scale_xyz: [1.0, 1.0, 1.0],
            },
            TransformFeatureOp {
                feature_id: "feature.scale".to_string(),
                source_feature_id: "feature.move".to_string(),
                translation_mm: [0.0, 0.0, 0.0],
                rotation_deg_xyz: [0.0, 0.0, 0.0],
                scale_xyz: [1.2, 1.2, 1.2],
            },
        ];

        let first = compose_transform_sequence(&ops).expect("compose should succeed");
        let second = compose_transform_sequence(&ops).expect("compose should succeed");
        assert_eq!(first, second);
    }

    #[test]
    fn transform_sequence_output_depends_on_order() {
        let a = TransformFeatureOp {
            feature_id: "feature.a".to_string(),
            source_feature_id: "feature.base".to_string(),
            translation_mm: [10.0, 0.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 90.0],
            scale_xyz: [1.0, 1.0, 1.0],
        };
        let b = TransformFeatureOp {
            feature_id: "feature.b".to_string(),
            source_feature_id: "feature.a".to_string(),
            translation_mm: [0.0, 5.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 0.0],
            scale_xyz: [2.0, 1.0, 1.0],
        };

        let ab = compose_transform_sequence(&[a.clone(), b.clone()]).expect("ab compose");
        let ba = compose_transform_sequence(&[b, a]).expect("ba compose");
        assert_ne!(ab, ba, "transform composition must preserve input order");
    }
}
