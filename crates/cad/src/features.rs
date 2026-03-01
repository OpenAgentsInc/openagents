use crate::kernel::CadKernelAdapter;
use crate::params::{ParameterStore, ScalarUnit};
use crate::primitives::{BoxPrimitive, PrimitiveSpec, build_primitive};
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
    use super::{BoxFeatureOp, evaluate_box_feature};
    use crate::kernel::CadKernelAdapter;
    use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
    use crate::CadResult;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MockKernel {
        box_calls: usize,
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
            Ok("unused")
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
}
