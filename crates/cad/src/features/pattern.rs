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
