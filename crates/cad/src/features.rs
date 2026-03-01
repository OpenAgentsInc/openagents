use crate::kernel::CadKernelAdapter;
use crate::params::{ParameterStore, ScalarUnit};
use crate::primitives::{BoxPrimitive, CylinderPrimitive, PrimitiveSpec, build_primitive};
use crate::feature_graph::FeatureNode;
use crate::{CadError, CadResult};
use std::collections::BTreeMap;

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
        let radius_mm = params.get_required_with_unit(&self.radius_param, ScalarUnit::Millimeter)?;
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
            self.tolerance_mm.unwrap_or(crate::policy::BASE_TOLERANCE_MM)
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
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

fn evaluate_cut_hole_feature_with_boolean<K: CadKernelAdapter, F>(
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

/// Feature op: deterministic linear pattern for repeated features.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearPatternFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub count_param: String,
    pub spacing_param: String,
    pub direction_unit_xyz: [f64; 3],
    pub start_index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinearPatternInstance {
    pub pattern_index: u32,
    pub translation_mm: [f64; 3],
    pub geometry_hash: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinearPatternFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub pattern_hash: String,
    pub instances: Vec<LinearPatternInstance>,
}

impl LinearPatternFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "linear pattern feature ids must not be empty".to_string(),
            });
        }
        if self.count_param.trim().is_empty() || self.spacing_param.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "linear pattern parameter bindings must not be empty".to_string(),
            });
        }
        for value in self.direction_unit_xyz {
            if !value.is_finite() {
                return Err(CadError::InvalidPrimitive {
                    reason: "linear pattern direction must use finite values".to_string(),
                });
            }
        }
        let norm = (self.direction_unit_xyz[0] * self.direction_unit_xyz[0]
            + self.direction_unit_xyz[1] * self.direction_unit_xyz[1]
            + self.direction_unit_xyz[2] * self.direction_unit_xyz[2])
            .sqrt();
        if norm <= f64::EPSILON {
            return Err(CadError::InvalidPrimitive {
                reason: "linear pattern direction must not be a zero vector".to_string(),
            });
        }
        Ok(())
    }

    fn resolve_count(&self, params: &ParameterStore) -> CadResult<u32> {
        let count_value = params.get_required_with_unit(&self.count_param, ScalarUnit::Unitless)?;
        if !count_value.is_finite() || count_value < 1.0 {
            return Err(CadError::InvalidParameter {
                name: self.count_param.clone(),
                reason: "linear pattern count must be finite and >= 1".to_string(),
            });
        }
        if (count_value.fract()).abs() > f64::EPSILON {
            return Err(CadError::InvalidParameter {
                name: self.count_param.clone(),
                reason: "linear pattern count must be an integer value".to_string(),
            });
        }
        let count = count_value as u32;
        Ok(count)
    }

    fn resolve_spacing_mm(&self, params: &ParameterStore) -> CadResult<f64> {
        let spacing_mm =
            params.get_required_with_unit(&self.spacing_param, ScalarUnit::Millimeter)?;
        if !spacing_mm.is_finite() || spacing_mm <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: self.spacing_param.clone(),
                reason: "linear pattern spacing must be finite and > 0 mm".to_string(),
            });
        }
        Ok(spacing_mm)
    }

    fn normalized_direction(&self) -> [f64; 3] {
        let norm = (self.direction_unit_xyz[0] * self.direction_unit_xyz[0]
            + self.direction_unit_xyz[1] * self.direction_unit_xyz[1]
            + self.direction_unit_xyz[2] * self.direction_unit_xyz[2])
            .sqrt();
        [
            self.direction_unit_xyz[0] / norm,
            self.direction_unit_xyz[1] / norm,
            self.direction_unit_xyz[2] / norm,
        ]
    }

    fn instance_hash(
        &self,
        source_geometry_hash: &str,
        pattern_index: u32,
        translation_mm: [f64; 3],
    ) -> String {
        let payload = format!(
            "linear_pattern_instance|feature={}|source={}|src_hash={}|index={}|tx={:.6}|ty={:.6}|tz={:.6}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            pattern_index,
            translation_mm[0],
            translation_mm[1],
            translation_mm[2]
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
    }

    fn pattern_hash(&self, source_geometry_hash: &str, instances: &[LinearPatternInstance]) -> String {
        let instance_payload = instances
            .iter()
            .map(|entry| format!("{}:{}", entry.pattern_index, entry.geometry_hash))
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            "linear_pattern|feature={}|source={}|src_hash={}|instances={}",
            self.feature_id, self.source_feature_id, source_geometry_hash, instance_payload
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
    }
}

pub fn evaluate_linear_pattern_feature(
    op: &LinearPatternFeatureOp,
    params: &ParameterStore,
    source_geometry_hash: &str,
) -> CadResult<LinearPatternFeatureResult> {
    op.validate()?;
    let count = op.resolve_count(params)?;
    let spacing_mm = op.resolve_spacing_mm(params)?;
    let direction = op.normalized_direction();
    let mut instances = Vec::with_capacity(count as usize);
    for offset in 0..count {
        let pattern_index = op
            .start_index
            .checked_add(offset)
            .ok_or_else(|| CadError::EvalFailed {
                reason: format!(
                    "linear pattern index overflow for feature {} at offset {}",
                    op.feature_id, offset
                ),
            })?;
        let distance = f64::from(offset) * spacing_mm;
        let translation_mm = [
            direction[0] * distance,
            direction[1] * distance,
            direction[2] * distance,
        ];
        let geometry_hash = op.instance_hash(source_geometry_hash, pattern_index, translation_mm);
        instances.push(LinearPatternInstance {
            pattern_index,
            translation_mm,
            geometry_hash,
        });
    }
    let pattern_hash = op.pattern_hash(source_geometry_hash, &instances);
    Ok(LinearPatternFeatureResult {
        feature_id: op.feature_id.clone(),
        source_feature_id: op.source_feature_id.clone(),
        pattern_hash,
        instances,
    })
}

pub const FILLET_PLACEHOLDER_OPERATION_KEY: &str = "fillet.placeholder.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilletPlaceholderKind {
    Fillet,
    Chamfer,
}

impl FilletPlaceholderKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fillet => "fillet",
            Self::Chamfer => "chamfer",
        }
    }

    fn parse(value: &str) -> CadResult<Self> {
        match value {
            "fillet" => Ok(Self::Fillet),
            "chamfer" => Ok(Self::Chamfer),
            other => Err(CadError::InvalidPrimitive {
                reason: format!("unsupported fillet placeholder kind '{other}'"),
            }),
        }
    }
}

/// Feature marker: no-op fillet/chamfer placeholder that preserves graph compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilletPlaceholderFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub radius_param: String,
    pub kind: FilletPlaceholderKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilletPlaceholderFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub kind: FilletPlaceholderKind,
    pub radius_mm: f64,
    pub geometry_hash: String,
    pub passthrough_source_hash: String,
}

impl FilletPlaceholderFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "fillet placeholder feature ids must not be empty".to_string(),
            });
        }
        if self.radius_param.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "fillet placeholder radius parameter binding must not be empty".to_string(),
            });
        }
        Ok(())
    }

    pub fn to_feature_node(&self) -> CadResult<FeatureNode> {
        self.validate()?;
        let params = BTreeMap::from([
            ("kind".to_string(), self.kind.as_str().to_string()),
            ("radius_param".to_string(), self.radius_param.clone()),
        ]);
        Ok(FeatureNode {
            id: self.feature_id.clone(),
            name: format!("{} marker", self.kind.as_str()),
            operation_key: FILLET_PLACEHOLDER_OPERATION_KEY.to_string(),
            depends_on: vec![self.source_feature_id.clone()],
            params,
        })
    }

    pub fn from_feature_node(node: &FeatureNode) -> CadResult<Self> {
        if node.operation_key != FILLET_PLACEHOLDER_OPERATION_KEY {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "feature {} is not a fillet placeholder operation",
                    node.id
                ),
            });
        }
        let source_feature_id = node.depends_on.first().cloned().ok_or_else(|| {
            CadError::InvalidPrimitive {
                reason: format!(
                    "fillet placeholder node {} must depend on exactly one source feature",
                    node.id
                ),
            }
        })?;
        if node.depends_on.len() != 1 {
            return Err(CadError::InvalidPrimitive {
                reason: format!(
                    "fillet placeholder node {} must have exactly one dependency",
                    node.id
                ),
            });
        }
        let kind = node
            .params
            .get("kind")
            .ok_or_else(|| CadError::InvalidPrimitive {
                reason: format!("fillet placeholder node {} missing kind param", node.id),
            })
            .and_then(|value| FilletPlaceholderKind::parse(value))?;
        let radius_param =
            node.params
                .get("radius_param")
                .cloned()
                .ok_or_else(|| CadError::InvalidPrimitive {
                    reason: format!(
                        "fillet placeholder node {} missing radius_param",
                        node.id
                    ),
                })?;
        let op = Self {
            feature_id: node.id.clone(),
            source_feature_id,
            radius_param,
            kind,
        };
        op.validate()?;
        Ok(op)
    }

    pub fn geometry_hash(&self, source_geometry_hash: &str, radius_mm: f64) -> String {
        let payload = format!(
            "fillet_placeholder|feature={}|source={}|src_hash={}|kind={}|radius_mm={:.6}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            self.kind.as_str(),
            radius_mm
        );
        format!("{:016x}", fnv1a64(payload.as_bytes()))
    }
}

pub fn evaluate_fillet_placeholder_feature(
    op: &FilletPlaceholderFeatureOp,
    params: &ParameterStore,
    source_geometry_hash: &str,
) -> CadResult<FilletPlaceholderFeatureResult> {
    op.validate()?;
    let radius_mm = params.get_required_with_unit(&op.radius_param, ScalarUnit::Millimeter)?;
    if !radius_mm.is_finite() || radius_mm <= 0.0 {
        return Err(CadError::InvalidParameter {
            name: op.radius_param.clone(),
            reason: "fillet placeholder radius must be finite and > 0 mm".to_string(),
        });
    }
    Ok(FilletPlaceholderFeatureResult {
        feature_id: op.feature_id.clone(),
        source_feature_id: op.source_feature_id.clone(),
        kind: op.kind,
        radius_mm,
        geometry_hash: op.geometry_hash(source_geometry_hash, radius_mm),
        passthrough_source_hash: source_geometry_hash.to_string(),
    })
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
        BoxFeatureOp, CutHoleFeatureOp, CylinderFeatureOp, LinearPatternFeatureOp,
        TransformFeatureOp, compose_transform_sequence, evaluate_box_feature, evaluate_cut_hole_feature,
        evaluate_cut_hole_feature_with_boolean, evaluate_cylinder_feature,
        evaluate_fillet_placeholder_feature, evaluate_linear_pattern_feature, evaluate_transform_feature,
        FilletPlaceholderFeatureOp, FilletPlaceholderKind,
    };
    use crate::feature_graph::{FeatureGraph, FeatureNode};
    use crate::kernel::CadKernelAdapter;
    use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
    use crate::{CadError, CadResult};
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

    #[test]
    fn cut_hole_returns_structured_failure_when_boolean_unavailable() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_radius_mm",
                ScalarValue {
                    value: 3.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");
        params
            .set(
                "hole_depth_mm",
                ScalarValue {
                    value: 12.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("depth should set");

        let op = CutHoleFeatureOp {
            feature_id: "feature.hole_001".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "hole_radius_mm".to_string(),
            depth_param: "hole_depth_mm".to_string(),
            tolerance_mm: None,
        };
        let mut kernel = MockKernel::default();
        let result = evaluate_cut_hole_feature(
            &mut kernel,
            &"solid-base",
            "hash-base",
            &op,
            &params,
        );
        assert!(result.is_err(), "missing boolean backend must return structured failure");
        let error = result.expect_err("error should be present");
        assert!(
            matches!(error, CadError::EvalFailed { .. }),
            "cut/hole failures must map to EvalFailed"
        );
    }

    #[test]
    fn cut_hole_returns_valid_result_when_boolean_succeeds() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_radius_mm",
                ScalarValue {
                    value: 4.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");
        params
            .set(
                "hole_depth_mm",
                ScalarValue {
                    value: 10.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("depth should set");
        let op = CutHoleFeatureOp {
            feature_id: "feature.hole_002".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "hole_radius_mm".to_string(),
            depth_param: "hole_depth_mm".to_string(),
            tolerance_mm: Some(0.001),
        };
        let mut kernel = MockKernel::default();
        let result = evaluate_cut_hole_feature_with_boolean(
            &mut kernel,
            &"solid-base",
            "hash-base",
            &op,
            &params,
            |_kernel, _left, _right, _tol| Ok("solid-with-hole"),
        )
        .expect("cut/hole should succeed");
        assert_eq!(result.feature_id, "feature.hole_002");
        assert_eq!(result.solid, "solid-with-hole");
        assert!(!result.geometry_hash.is_empty());
    }

    #[test]
    fn linear_pattern_indices_are_stable_across_rebuilds() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_count",
                ScalarValue {
                    value: 4.0,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("count should set");
        params
            .set(
                "hole_spacing_mm",
                ScalarValue {
                    value: 18.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("spacing should set");

        let op = LinearPatternFeatureOp {
            feature_id: "feature.vents".to_string(),
            source_feature_id: "feature.hole_002".to_string(),
            count_param: "hole_count".to_string(),
            spacing_param: "hole_spacing_mm".to_string(),
            direction_unit_xyz: [1.0, 0.0, 0.0],
            start_index: 100,
        };

        let first =
            evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");
        let second =
            evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("eval should pass");

        let first_indexes: Vec<u32> = first.instances.iter().map(|entry| entry.pattern_index).collect();
        let second_indexes: Vec<u32> = second.instances.iter().map(|entry| entry.pattern_index).collect();
        assert_eq!(first_indexes, vec![100, 101, 102, 103]);
        assert_eq!(first_indexes, second_indexes);
        assert_eq!(first.instances, second.instances);
        assert_eq!(first.pattern_hash, second.pattern_hash);
    }

    #[test]
    fn linear_pattern_rejects_non_integer_count() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_count",
                ScalarValue {
                    value: 2.5,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("count should set");
        params
            .set(
                "hole_spacing_mm",
                ScalarValue {
                    value: 15.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("spacing should set");
        let op = LinearPatternFeatureOp {
            feature_id: "feature.vents".to_string(),
            source_feature_id: "feature.hole".to_string(),
            count_param: "hole_count".to_string(),
            spacing_param: "hole_spacing_mm".to_string(),
            direction_unit_xyz: [1.0, 0.0, 0.0],
            start_index: 0,
        };
        let error = evaluate_linear_pattern_feature(&op, &params, "hash-hole")
            .expect_err("fractional count must be rejected");
        assert!(matches!(error, CadError::InvalidParameter { .. }));
    }

    #[test]
    fn linear_pattern_rejects_zero_direction_vector() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_count",
                ScalarValue {
                    value: 3.0,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("count should set");
        params
            .set(
                "hole_spacing_mm",
                ScalarValue {
                    value: 10.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("spacing should set");
        let op = LinearPatternFeatureOp {
            feature_id: "feature.vents".to_string(),
            source_feature_id: "feature.hole".to_string(),
            count_param: "hole_count".to_string(),
            spacing_param: "hole_spacing_mm".to_string(),
            direction_unit_xyz: [0.0, 0.0, 0.0],
            start_index: 0,
        };
        let error = evaluate_linear_pattern_feature(&op, &params, "hash-hole")
            .expect_err("zero direction must be rejected");
        assert!(matches!(error, CadError::InvalidPrimitive { .. }));
    }

    #[test]
    fn linear_pattern_preserves_prefix_indices_when_count_increases() {
        let mut params = ParameterStore::default();
        params
            .set(
                "hole_count",
                ScalarValue {
                    value: 3.0,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("count should set");
        params
            .set(
                "hole_spacing_mm",
                ScalarValue {
                    value: 22.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("spacing should set");
        let op = LinearPatternFeatureOp {
            feature_id: "feature.vents".to_string(),
            source_feature_id: "feature.hole".to_string(),
            count_param: "hole_count".to_string(),
            spacing_param: "hole_spacing_mm".to_string(),
            direction_unit_xyz: [0.0, 1.0, 0.0],
            start_index: 10,
        };
        let baseline =
            evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("baseline eval");
        params
            .set(
                "hole_count",
                ScalarValue {
                    value: 5.0,
                    unit: ScalarUnit::Unitless,
                },
            )
            .expect("count update should set");
        let expanded =
            evaluate_linear_pattern_feature(&op, &params, "hash-hole").expect("expanded eval");
        assert_eq!(&expanded.instances[..baseline.instances.len()], baseline.instances.as_slice());
    }

    #[test]
    fn fillet_placeholder_survives_save_load_and_rebuild() {
        let op = FilletPlaceholderFeatureOp {
            feature_id: "feature.fillet_marker".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "fillet_radius_mm".to_string(),
            kind: FilletPlaceholderKind::Fillet,
        };
        let node = op.to_feature_node().expect("feature node conversion should succeed");
        let graph = FeatureGraph {
            nodes: vec![FeatureNode {
                id: "feature.base".to_string(),
                name: "base".to_string(),
                operation_key: "primitive.box.v1".to_string(),
                depends_on: vec![],
                params: BTreeMap::new(),
            }, node],
        };
        let payload = serde_json::to_string(&graph).expect("graph should serialize");
        let parsed: FeatureGraph = serde_json::from_str(&payload).expect("graph should parse");
        let restored_marker = parsed
            .nodes
            .iter()
            .find(|entry| entry.operation_key == super::FILLET_PLACEHOLDER_OPERATION_KEY)
            .expect("marker node should be retained");
        let restored_op =
            FilletPlaceholderFeatureOp::from_feature_node(restored_marker).expect("restore op");
        assert_eq!(op, restored_op);

        let mut params = ParameterStore::default();
        params
            .set(
                "fillet_radius_mm",
                ScalarValue {
                    value: 4.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");

        let first = evaluate_fillet_placeholder_feature(&restored_op, &params, "hash-base")
            .expect("first rebuild");
        let second = evaluate_fillet_placeholder_feature(&restored_op, &params, "hash-base")
            .expect("second rebuild");
        assert_eq!(first, second, "placeholder rebuild must be deterministic");
        assert_eq!(first.passthrough_source_hash, "hash-base");
    }

    #[test]
    fn fillet_placeholder_rejects_invalid_radius() {
        let op = FilletPlaceholderFeatureOp {
            feature_id: "feature.chamfer_marker".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "chamfer_radius_mm".to_string(),
            kind: FilletPlaceholderKind::Chamfer,
        };
        let mut params = ParameterStore::default();
        params
            .set(
                "chamfer_radius_mm",
                ScalarValue {
                    value: 0.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("radius should set");
        let error = evaluate_fillet_placeholder_feature(&op, &params, "hash-base")
            .expect_err("zero radius must fail");
        assert!(matches!(error, CadError::InvalidParameter { .. }));
    }

    #[test]
    fn fillet_placeholder_from_node_rejects_wrong_operation_key() {
        let node = FeatureNode {
            id: "feature.not_marker".to_string(),
            name: "bad".to_string(),
            operation_key: "primitive.box.v1".to_string(),
            depends_on: vec!["feature.base".to_string()],
            params: BTreeMap::from([
                ("kind".to_string(), "fillet".to_string()),
                ("radius_param".to_string(), "radius_mm".to_string()),
            ]),
        };
        let error =
            FilletPlaceholderFeatureOp::from_feature_node(&node).expect_err("wrong key fails");
        assert!(matches!(error, CadError::InvalidPrimitive { .. }));
    }
}
