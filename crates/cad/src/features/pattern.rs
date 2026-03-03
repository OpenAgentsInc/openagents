use crate::hash::stable_hex_digest;
use crate::params::{ParameterStore, ScalarUnit};
use crate::{CadError, CadResult};

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

/// Feature op: deterministic circular pattern for repeated features.
#[derive(Clone, Debug, PartialEq)]
pub struct CircularPatternFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub count_param: String,
    pub angle_deg_param: String,
    pub radius_param: String,
    pub axis_origin_mm: [f64; 3],
    pub axis_direction_xyz: [f64; 3],
    pub start_index: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircularPatternInstance {
    pub pattern_index: u32,
    pub angle_deg: f64,
    pub center_mm: [f64; 3],
    pub geometry_hash: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircularPatternFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub pattern_hash: String,
    pub instances: Vec<CircularPatternInstance>,
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
        resolve_pattern_count(params, &self.count_param, "linear")
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
        stable_hex_digest(payload.as_bytes())
    }

    fn pattern_hash(
        &self,
        source_geometry_hash: &str,
        instances: &[LinearPatternInstance],
    ) -> String {
        let instance_payload = instances
            .iter()
            .map(|entry| format!("{}:{}", entry.pattern_index, entry.geometry_hash))
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            "linear_pattern|feature={}|source={}|src_hash={}|instances={}",
            self.feature_id, self.source_feature_id, source_geometry_hash, instance_payload
        );
        stable_hex_digest(payload.as_bytes())
    }
}

impl CircularPatternFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        if self.feature_id.trim().is_empty() || self.source_feature_id.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "circular pattern feature ids must not be empty".to_string(),
            });
        }
        if self.count_param.trim().is_empty()
            || self.angle_deg_param.trim().is_empty()
            || self.radius_param.trim().is_empty()
        {
            return Err(CadError::InvalidPrimitive {
                reason: "circular pattern parameter bindings must not be empty".to_string(),
            });
        }
        if self.axis_origin_mm.iter().any(|value| !value.is_finite()) {
            return Err(CadError::InvalidPrimitive {
                reason: "circular pattern axis origin must use finite values".to_string(),
            });
        }
        if self
            .axis_direction_xyz
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(CadError::InvalidPrimitive {
                reason: "circular pattern axis direction must use finite values".to_string(),
            });
        }
        if vec_norm(self.axis_direction_xyz) <= f64::EPSILON {
            return Err(CadError::InvalidPrimitive {
                reason: "circular pattern axis direction must not be a zero vector".to_string(),
            });
        }
        Ok(())
    }

    fn resolve_count(&self, params: &ParameterStore) -> CadResult<u32> {
        resolve_pattern_count(params, &self.count_param, "circular")
    }

    fn resolve_angle_deg(&self, params: &ParameterStore) -> CadResult<f64> {
        let angle_deg =
            params.get_required_with_unit(&self.angle_deg_param, ScalarUnit::Unitless)?;
        if !angle_deg.is_finite() || angle_deg <= 0.0 {
            return Err(CadError::InvalidParameter {
                name: self.angle_deg_param.clone(),
                reason: "circular pattern angle must be finite and > 0 degrees".to_string(),
            });
        }
        Ok(angle_deg)
    }

    fn resolve_radius_mm(&self, params: &ParameterStore) -> CadResult<f64> {
        let radius_mm =
            params.get_required_with_unit(&self.radius_param, ScalarUnit::Millimeter)?;
        if !radius_mm.is_finite() || radius_mm < 0.0 {
            return Err(CadError::InvalidParameter {
                name: self.radius_param.clone(),
                reason: "circular pattern radius must be finite and >= 0 mm".to_string(),
            });
        }
        Ok(radius_mm)
    }

    fn normalized_axis_direction(&self) -> [f64; 3] {
        vec_normalize(self.axis_direction_xyz)
            .expect("circular axis direction should be non-zero after validate")
    }

    fn radial_basis(&self, axis: [f64; 3]) -> [f64; 3] {
        let candidate = if axis[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        vec_normalize(vec_cross(axis, candidate)).expect("candidate must produce radial basis")
    }

    fn instance_hash(
        &self,
        source_geometry_hash: &str,
        pattern_index: u32,
        angle_deg: f64,
        center_mm: [f64; 3],
    ) -> String {
        let payload = format!(
            "circular_pattern_instance|feature={}|source={}|src_hash={}|index={}|angle_deg={:.6}|cx={:.6}|cy={:.6}|cz={:.6}",
            self.feature_id,
            self.source_feature_id,
            source_geometry_hash,
            pattern_index,
            angle_deg,
            center_mm[0],
            center_mm[1],
            center_mm[2]
        );
        stable_hex_digest(payload.as_bytes())
    }

    fn pattern_hash(
        &self,
        source_geometry_hash: &str,
        instances: &[CircularPatternInstance],
    ) -> String {
        let instance_payload = instances
            .iter()
            .map(|entry| format!("{}:{}", entry.pattern_index, entry.geometry_hash))
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!(
            "circular_pattern|feature={}|source={}|src_hash={}|instances={}",
            self.feature_id, self.source_feature_id, source_geometry_hash, instance_payload
        );
        stable_hex_digest(payload.as_bytes())
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
        let pattern_index =
            op.start_index
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

pub fn evaluate_circular_pattern_feature(
    op: &CircularPatternFeatureOp,
    params: &ParameterStore,
    source_geometry_hash: &str,
) -> CadResult<CircularPatternFeatureResult> {
    op.validate()?;
    let count = op.resolve_count(params)?;
    let angle_deg = op.resolve_angle_deg(params)?;
    let radius_mm = op.resolve_radius_mm(params)?;
    let axis = op.normalized_axis_direction();
    let radial = op.radial_basis(axis);
    let tangent = vec_cross(axis, radial);

    let mut instances = Vec::with_capacity(count as usize);
    for offset in 0..count {
        let pattern_index =
            op.start_index
                .checked_add(offset)
                .ok_or_else(|| CadError::EvalFailed {
                    reason: format!(
                        "circular pattern index overflow for feature {} at offset {}",
                        op.feature_id, offset
                    ),
                })?;
        let angle = angle_deg * f64::from(offset) / f64::from(count);
        let radians = angle.to_radians();
        let cos_a = radians.cos();
        let sin_a = radians.sin();
        let offset_mm = [
            (radial[0] * cos_a + tangent[0] * sin_a) * radius_mm,
            (radial[1] * cos_a + tangent[1] * sin_a) * radius_mm,
            (radial[2] * cos_a + tangent[2] * sin_a) * radius_mm,
        ];
        let center_mm = [
            op.axis_origin_mm[0] + offset_mm[0],
            op.axis_origin_mm[1] + offset_mm[1],
            op.axis_origin_mm[2] + offset_mm[2],
        ];
        let geometry_hash = op.instance_hash(source_geometry_hash, pattern_index, angle, center_mm);
        instances.push(CircularPatternInstance {
            pattern_index,
            angle_deg: angle,
            center_mm,
            geometry_hash,
        });
    }

    let pattern_hash = op.pattern_hash(source_geometry_hash, &instances);
    Ok(CircularPatternFeatureResult {
        feature_id: op.feature_id.clone(),
        source_feature_id: op.source_feature_id.clone(),
        pattern_hash,
        instances,
    })
}

fn resolve_pattern_count(params: &ParameterStore, param: &str, label: &str) -> CadResult<u32> {
    let count_value = params.get_required_with_unit(param, ScalarUnit::Unitless)?;
    if !count_value.is_finite() || count_value < 1.0 {
        return Err(CadError::InvalidParameter {
            name: param.to_string(),
            reason: format!("{label} pattern count must be finite and >= 1"),
        });
    }
    if (count_value.fract()).abs() > f64::EPSILON {
        return Err(CadError::InvalidParameter {
            name: param.to_string(),
            reason: format!("{label} pattern count must be an integer value"),
        });
    }
    Ok(count_value as u32)
}

fn vec_cross(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [
        lhs[1] * rhs[2] - lhs[2] * rhs[1],
        lhs[2] * rhs[0] - lhs[0] * rhs[2],
        lhs[0] * rhs[1] - lhs[1] * rhs[0],
    ]
}

fn vec_norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn vec_normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let norm = vec_norm(v);
    if norm <= f64::EPSILON || !norm.is_finite() {
        return None;
    }
    Some([v[0] / norm, v[1] / norm, v[2] / norm])
}
