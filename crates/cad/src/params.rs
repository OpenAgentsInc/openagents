use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Typed scalar units supported by the CAD parameter store.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ScalarUnit {
    #[serde(rename = "mm")]
    Millimeter,
    #[serde(rename = "deg")]
    Degree,
    #[serde(rename = "unitless")]
    Unitless,
}

impl ScalarUnit {
    pub fn parse(value: &str) -> CadResult<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mm" => Ok(Self::Millimeter),
            "deg" | "degree" | "degrees" => Ok(Self::Degree),
            "unitless" | "scalar" => Ok(Self::Unitless),
            other => Err(CadError::InvalidParameter {
                name: "unit".to_string(),
                reason: format!("unsupported scalar unit '{other}'"),
            }),
        }
    }
}

/// Scalar CAD parameter with explicit unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScalarValue {
    pub value: f64,
    pub unit: ScalarUnit,
}

impl ScalarValue {
    pub fn validate(self, name: &str) -> CadResult<()> {
        if !self.value.is_finite() {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: "value must be finite".to_string(),
            });
        }
        Ok(())
    }
}

/// Deterministic CAD parameter store backed by sorted keys.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterStore {
    pub values: BTreeMap<String, ScalarValue>,
}

impl ParameterStore {
    pub fn set(&mut self, name: impl Into<String>, value: ScalarValue) -> CadResult<()> {
        let name = name.into();
        validate_parameter_name(&name)?;
        value.validate(&name)?;
        self.values.insert(name, value);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<ScalarValue> {
        self.values.get(name).copied()
    }

    pub fn get_required_with_unit(&self, name: &str, unit: ScalarUnit) -> CadResult<f64> {
        let Some(parameter) = self.values.get(name) else {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: "parameter not found".to_string(),
            });
        };
        if parameter.unit != unit {
            return Err(CadError::InvalidParameter {
                name: name.to_string(),
                reason: format!(
                    "unit mismatch: expected {:?}, got {:?}",
                    unit, parameter.unit
                ),
            });
        }
        Ok(parameter.value)
    }
}

fn validate_parameter_name(name: &str) -> CadResult<()> {
    if name.is_empty() {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must not be empty".to_string(),
        });
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must not be empty".to_string(),
        });
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name must start with ascii letter or '_'".to_string(),
        });
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')) {
        return Err(CadError::InvalidParameter {
            name: name.to_string(),
            reason: "parameter name contains unsupported characters".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ParameterStore, ScalarUnit, ScalarValue};

    #[test]
    fn setting_and_getting_valid_parameter_is_deterministic() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "thickness_mm",
            ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_ok(), "valid parameter should set successfully");
        assert_eq!(
            params.get("thickness_mm"),
            Some(ScalarValue {
                value: 4.0,
                unit: ScalarUnit::Millimeter
            })
        );
    }

    #[test]
    fn invalid_parameter_name_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "9thickness",
            ScalarValue {
                value: 3.0,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_err(), "invalid parameter name should be rejected");
    }

    #[test]
    fn invalid_parameter_value_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "thickness_mm",
            ScalarValue {
                value: f64::NAN,
                unit: ScalarUnit::Millimeter,
            },
        );
        assert!(set.is_err(), "non-finite values should be rejected");
    }

    #[test]
    fn invalid_unit_string_is_rejected() {
        let parsed = ScalarUnit::parse("meters");
        assert!(parsed.is_err(), "unsupported unit should be rejected");
    }

    #[test]
    fn unit_mismatch_is_rejected() {
        let mut params = ParameterStore::default();
        let set = params.set(
            "draft_angle",
            ScalarValue {
                value: 5.0,
                unit: ScalarUnit::Degree,
            },
        );
        assert!(set.is_ok(), "parameter setup should succeed");

        let value = params.get_required_with_unit("draft_angle", ScalarUnit::Millimeter);
        assert!(value.is_err(), "unit mismatch should be rejected");
    }
}
