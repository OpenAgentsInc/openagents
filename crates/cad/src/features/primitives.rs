use crate::hash::stable_hex_digest;
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
        stable_hex_digest(payload.as_bytes())
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
        let radius_mm =
            params.get_required_with_unit(&self.radius_param, ScalarUnit::Millimeter)?;
        let height_mm =
            params.get_required_with_unit(&self.height_param, ScalarUnit::Millimeter)?;
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
        stable_hex_digest(payload.as_bytes())
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

/// Feature op: subtraction-style cylindrical cut/hole.
#[derive(Clone, Debug, PartialEq)]
pub struct CutHoleFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub radius_param: String,
    pub depth_param: String,
    pub tolerance_mm: Option<f64>,
}

impl CutHoleFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "cut/hole feature ids must not be empty".to_string(),
            });
        }
        if self.radius_param.trim().is_empty() || self.depth_param.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "cut/hole parameter bindings must not be empty".to_string(),
            });
        }
        Ok(())
    }

    pub fn resolve_cutter(&self, params: &ParameterStore) -> CadResult<CylinderPrimitive> {
        self.validate()?;
        let radius_mm =
            params.get_required_with_unit(&self.radius_param, ScalarUnit::Millimeter)?;
        let height_mm = params.get_required_with_unit(&self.depth_param, ScalarUnit::Millimeter)?;
        let primitive = CylinderPrimitive {
            radius_mm,
            height_mm,
        };
        primitive.validate()?;
        Ok(primitive)
    }

    pub fn geometry_hash(&self, source_geometry_hash: &str, cutter: &CylinderPrimitive) -> String {
        let payload = format!(
            "cut_hole|feature={}|source={}|src_hash={}|r={:.6}|d={:.6}|tol={}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            cutter.radius_mm,
            cutter.height_mm,
            self.tolerance_mm
                .unwrap_or(crate::policy::BASE_TOLERANCE_MM)
        );
        stable_hex_digest(payload.as_bytes())
    }
}

pub fn evaluate_cut_hole_feature<K: CadKernelAdapter>(
    kernel: &mut K,
    target_solid: &K::Solid,
    source_geometry_hash: &str,
    op: &CutHoleFeatureOp,
    params: &ParameterStore,
) -> CadResult<FeatureOpResult<K::Solid>> {
    evaluate_cut_hole_feature_with_boolean(
        kernel,
        target_solid,
        source_geometry_hash,
        op,
        params,
        |kernel, left, right, tolerance| {
            crate::boolean::boolean_op(
                kernel,
                crate::boolean::BooleanOp::Difference,
                left,
                right,
                tolerance,
            )
        },
    )
}

pub(super) fn evaluate_cut_hole_feature_with_boolean<K: CadKernelAdapter, F>(
    kernel: &mut K,
    target_solid: &K::Solid,
    source_geometry_hash: &str,
    op: &CutHoleFeatureOp,
    params: &ParameterStore,
    boolean_apply: F,
) -> CadResult<FeatureOpResult<K::Solid>>
where
    F: FnOnce(&mut K, &K::Solid, &K::Solid, Option<f64>) -> CadResult<K::Solid>,
{
    let cutter = op.resolve_cutter(params)?;
    let cutter_solid = build_primitive(kernel, PrimitiveSpec::Cylinder(cutter))?;
    let result_solid = boolean_apply(kernel, target_solid, &cutter_solid, op.tolerance_mm)
        .map_err(|error| CadError::EvalFailed {
            reason: format!(
                "cut/hole feature {} failed with structured error: {error}",
                op.feature_id
            ),
        })?;
    Ok(FeatureOpResult {
        feature_id: op.feature_id.clone(),
        geometry_hash: op.geometry_hash(source_geometry_hash, &cutter),
        solid: result_solid,
    })
}
