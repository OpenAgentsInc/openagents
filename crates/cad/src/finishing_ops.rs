use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};
use crate::feature_graph::FeatureNode;
use crate::hash::stable_hex_digest;
use crate::keys::{feature_params as feature_keys, warning_metadata as warning_keys};
use crate::params::{ParameterStore, ScalarUnit};
use crate::{CadError, CadResult};

pub const FILLET_OPERATION_KEY: &str = "fillet.v2";
pub const CHAMFER_OPERATION_KEY: &str = "chamfer.v2";
pub const SHELL_OPERATION_KEY: &str = "shell.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishingFailureClass {
    InvalidInput,
    TopologyRisk,
    ZeroThicknessRisk,
    KernelRejected,
}

impl FinishingFailureClass {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "FINISHING_INVALID_INPUT",
            Self::TopologyRisk => "FINISHING_TOPOLOGY_RISK",
            Self::ZeroThicknessRisk => "FINISHING_ZERO_THICKNESS_RISK",
            Self::KernelRejected => "FINISHING_KERNEL_REJECTED",
        }
    }

    pub fn remediation_hint(self) -> &'static str {
        match self {
            Self::InvalidInput => "Use finite positive dimensions and valid edge/face refs.",
            Self::TopologyRisk => "Reduce operation size or simplify edge selection.",
            Self::ZeroThicknessRisk => {
                "Increase wall thickness or reduce shell/fillet/chamfer distance."
            }
            Self::KernelRejected => "Retry with smaller values or split operation into stages.",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishingStatus {
    Applied,
    FallbackKeptSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FinishingContext {
    pub source_geometry_hash: String,
    pub source_min_thickness_mm: Option<f64>,
    pub source_volume_mm3: Option<f64>,
}

impl FinishingContext {
    pub fn validate(&self) -> CadResult<()> {
        if self.source_geometry_hash.trim().is_empty() {
            return Err(CadError::InvalidPrimitive {
                reason: "source geometry hash must not be empty".to_string(),
            });
        }
        if let Some(value) = self.source_min_thickness_mm
            && (!value.is_finite() || value <= 0.0)
        {
            return Err(CadError::InvalidPrimitive {
                reason: "source_min_thickness_mm must be finite and > 0".to_string(),
            });
        }
        if let Some(value) = self.source_volume_mm3
            && (!value.is_finite() || value <= 0.0)
        {
            return Err(CadError::InvalidPrimitive {
                reason: "source_volume_mm3 must be finite and > 0".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FinishingFeatureResult {
    pub feature_id: String,
    pub source_feature_id: String,
    pub operation_key: String,
    pub status: FinishingStatus,
    pub geometry_hash: String,
    pub failure_classification: Option<FinishingFailureClass>,
    pub fallback_message: Option<String>,
    pub remediation_hint: String,
    pub warnings: Vec<CadWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilletFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub radius_param: String,
    pub edge_refs: Vec<String>,
    pub allow_fallback: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChamferFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub distance_param: String,
    pub edge_refs: Vec<String>,
    pub allow_fallback: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellFeatureOp {
    pub feature_id: String,
    pub source_feature_id: String,
    pub thickness_param: String,
    pub remove_face_refs: Vec<String>,
    pub allow_fallback: bool,
}

impl FilletFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        validate_finishing_refs(
            &self.feature_id,
            &self.source_feature_id,
            &self.radius_param,
            &self.edge_refs,
            "fillet",
        )
    }

    pub fn to_feature_node(&self) -> CadResult<FeatureNode> {
        self.validate()?;
        Ok(FeatureNode {
            id: self.feature_id.clone(),
            name: "Fillet".to_string(),
            operation_key: FILLET_OPERATION_KEY.to_string(),
            depends_on: vec![self.source_feature_id.clone()],
            params: BTreeMap::from([
                (
                    feature_keys::RADIUS_PARAM.owned(),
                    self.radius_param.clone(),
                ),
                (
                    "edge_refs".to_string(),
                    canonical_refs(&self.edge_refs).join(","),
                ),
                (
                    "allow_fallback".to_string(),
                    self.allow_fallback.to_string(),
                ),
            ]),
        })
    }

    pub fn from_feature_node(node: &FeatureNode) -> CadResult<Self> {
        if node.operation_key != FILLET_OPERATION_KEY {
            return Err(CadError::InvalidPrimitive {
                reason: format!("feature {} is not a fillet operation", node.id),
            });
        }
        let source_feature_id =
            node.depends_on
                .first()
                .cloned()
                .ok_or_else(|| CadError::InvalidPrimitive {
                    reason: "fillet node must have exactly one dependency".to_string(),
                })?;
        if node.depends_on.len() != 1 {
            return Err(CadError::InvalidPrimitive {
                reason: "fillet node must have exactly one dependency".to_string(),
            });
        }
        let radius_param = required_param(node, feature_keys::RADIUS_PARAM.as_str())?;
        let edge_refs = parse_csv_refs(required_param(node, "edge_refs")?);
        let allow_fallback = parse_bool(required_param(node, "allow_fallback")?)?;
        let op = Self {
            feature_id: node.id.clone(),
            source_feature_id,
            radius_param,
            edge_refs,
            allow_fallback,
        };
        op.validate()?;
        Ok(op)
    }
}

impl ChamferFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        validate_finishing_refs(
            &self.feature_id,
            &self.source_feature_id,
            &self.distance_param,
            &self.edge_refs,
            "chamfer",
        )
    }

    pub fn to_feature_node(&self) -> CadResult<FeatureNode> {
        self.validate()?;
        Ok(FeatureNode {
            id: self.feature_id.clone(),
            name: "Chamfer".to_string(),
            operation_key: CHAMFER_OPERATION_KEY.to_string(),
            depends_on: vec![self.source_feature_id.clone()],
            params: BTreeMap::from([
                ("distance_param".to_string(), self.distance_param.clone()),
                (
                    "edge_refs".to_string(),
                    canonical_refs(&self.edge_refs).join(","),
                ),
                (
                    "allow_fallback".to_string(),
                    self.allow_fallback.to_string(),
                ),
            ]),
        })
    }

    pub fn from_feature_node(node: &FeatureNode) -> CadResult<Self> {
        if node.operation_key != CHAMFER_OPERATION_KEY {
            return Err(CadError::InvalidPrimitive {
                reason: format!("feature {} is not a chamfer operation", node.id),
            });
        }
        let source_feature_id =
            node.depends_on
                .first()
                .cloned()
                .ok_or_else(|| CadError::InvalidPrimitive {
                    reason: "chamfer node must have exactly one dependency".to_string(),
                })?;
        if node.depends_on.len() != 1 {
            return Err(CadError::InvalidPrimitive {
                reason: "chamfer node must have exactly one dependency".to_string(),
            });
        }
        let distance_param = required_param(node, "distance_param")?;
        let edge_refs = parse_csv_refs(required_param(node, "edge_refs")?);
        let allow_fallback = parse_bool(required_param(node, "allow_fallback")?)?;
        let op = Self {
            feature_id: node.id.clone(),
            source_feature_id,
            distance_param,
            edge_refs,
            allow_fallback,
        };
        op.validate()?;
        Ok(op)
    }
}

impl ShellFeatureOp {
    pub fn validate(&self) -> CadResult<()> {
        validate_finishing_refs(
            &self.feature_id,
            &self.source_feature_id,
            &self.thickness_param,
            &self.remove_face_refs,
            "shell",
        )
    }

    pub fn to_feature_node(&self) -> CadResult<FeatureNode> {
        self.validate()?;
        Ok(FeatureNode {
            id: self.feature_id.clone(),
            name: "Shell".to_string(),
            operation_key: SHELL_OPERATION_KEY.to_string(),
            depends_on: vec![self.source_feature_id.clone()],
            params: BTreeMap::from([
                ("thickness_param".to_string(), self.thickness_param.clone()),
                (
                    "remove_face_refs".to_string(),
                    canonical_refs(&self.remove_face_refs).join(","),
                ),
                (
                    "allow_fallback".to_string(),
                    self.allow_fallback.to_string(),
                ),
            ]),
        })
    }

    pub fn from_feature_node(node: &FeatureNode) -> CadResult<Self> {
        if node.operation_key != SHELL_OPERATION_KEY {
            return Err(CadError::InvalidPrimitive {
                reason: format!("feature {} is not a shell operation", node.id),
            });
        }
        let source_feature_id =
            node.depends_on
                .first()
                .cloned()
                .ok_or_else(|| CadError::InvalidPrimitive {
                    reason: "shell node must have exactly one dependency".to_string(),
                })?;
        if node.depends_on.len() != 1 {
            return Err(CadError::InvalidPrimitive {
                reason: "shell node must have exactly one dependency".to_string(),
            });
        }
        let thickness_param = required_param(node, "thickness_param")?;
        let remove_face_refs = parse_csv_refs(required_param(node, "remove_face_refs")?);
        let allow_fallback = parse_bool(required_param(node, "allow_fallback")?)?;
        let op = Self {
            feature_id: node.id.clone(),
            source_feature_id,
            thickness_param,
            remove_face_refs,
            allow_fallback,
        };
        op.validate()?;
        Ok(op)
    }
}

pub fn evaluate_fillet_feature(
    op: &FilletFeatureOp,
    params: &ParameterStore,
    context: &FinishingContext,
) -> CadResult<FinishingFeatureResult> {
    op.validate()?;
    context.validate()?;
    let radius_mm = params.get_required_with_unit(&op.radius_param, ScalarUnit::Millimeter)?;
    let risk_threshold_mm = context
        .source_min_thickness_mm
        .map(|value| (value * 0.45).max(1.0))
        .unwrap_or(12.0);
    evaluate_finishing_value(FinishingEvalRequest {
        feature_id: &op.feature_id,
        source_feature_id: &op.source_feature_id,
        operation_key: FILLET_OPERATION_KEY,
        operation_label: "fillet",
        value_mm: radius_mm,
        risk_threshold_mm,
        allow_fallback: op.allow_fallback,
        context,
    })
}

pub fn evaluate_chamfer_feature(
    op: &ChamferFeatureOp,
    params: &ParameterStore,
    context: &FinishingContext,
) -> CadResult<FinishingFeatureResult> {
    op.validate()?;
    context.validate()?;
    let distance_mm = params.get_required_with_unit(&op.distance_param, ScalarUnit::Millimeter)?;
    let risk_threshold_mm = context
        .source_min_thickness_mm
        .map(|value| (value * 0.40).max(0.8))
        .unwrap_or(10.0);
    evaluate_finishing_value(FinishingEvalRequest {
        feature_id: &op.feature_id,
        source_feature_id: &op.source_feature_id,
        operation_key: CHAMFER_OPERATION_KEY,
        operation_label: "chamfer",
        value_mm: distance_mm,
        risk_threshold_mm,
        allow_fallback: op.allow_fallback,
        context,
    })
}

pub fn evaluate_shell_feature(
    op: &ShellFeatureOp,
    params: &ParameterStore,
    context: &FinishingContext,
) -> CadResult<FinishingFeatureResult> {
    op.validate()?;
    context.validate()?;
    let thickness_mm =
        params.get_required_with_unit(&op.thickness_param, ScalarUnit::Millimeter)?;
    let risk_threshold_mm = context
        .source_min_thickness_mm
        .map(|value| (value * 0.35).max(0.8))
        .unwrap_or(6.0);
    evaluate_finishing_value(FinishingEvalRequest {
        feature_id: &op.feature_id,
        source_feature_id: &op.source_feature_id,
        operation_key: SHELL_OPERATION_KEY,
        operation_label: "shell",
        value_mm: thickness_mm,
        risk_threshold_mm,
        allow_fallback: op.allow_fallback,
        context,
    })
}

struct FinishingEvalRequest<'a> {
    feature_id: &'a str,
    source_feature_id: &'a str,
    operation_key: &'a str,
    operation_label: &'a str,
    value_mm: f64,
    risk_threshold_mm: f64,
    allow_fallback: bool,
    context: &'a FinishingContext,
}

fn evaluate_finishing_value(
    request: FinishingEvalRequest<'_>,
) -> CadResult<FinishingFeatureResult> {
    if !request.value_mm.is_finite() || request.value_mm <= 0.0 {
        return Err(CadError::EvalFailed {
            reason: format!(
                "{}: {} value must be finite and > 0 (class={})",
                request.operation_key,
                request.operation_label,
                FinishingFailureClass::InvalidInput.code()
            ),
        });
    }
    if request.value_mm > request.risk_threshold_mm {
        let failure_class = if request.operation_key == SHELL_OPERATION_KEY {
            FinishingFailureClass::ZeroThicknessRisk
        } else {
            FinishingFailureClass::TopologyRisk
        };
        if request.allow_fallback {
            let fallback_message = format!(
                "{operation_label} value {:.6}mm exceeded threshold {:.6}mm; fallback kept source geometry",
                request.value_mm,
                request.risk_threshold_mm,
                operation_label = request.operation_label
            );
            return Ok(FinishingFeatureResult {
                feature_id: request.feature_id.to_string(),
                source_feature_id: request.source_feature_id.to_string(),
                operation_key: request.operation_key.to_string(),
                status: FinishingStatus::FallbackKeptSource,
                geometry_hash: request.context.source_geometry_hash.clone(),
                failure_classification: Some(failure_class),
                fallback_message: Some(fallback_message.clone()),
                remediation_hint: failure_class.remediation_hint().to_string(),
                warnings: vec![CadWarning {
                    code: CadWarningCode::FilletFailed,
                    severity: CadWarningSeverity::Warning,
                    message: fallback_message,
                    remediation_hint: failure_class.remediation_hint().to_string(),
                    semantic_refs: vec![format!("cad://feature/{}", request.feature_id)],
                    metadata: BTreeMap::from([
                        (
                            warning_keys::OPERATION_KEY.owned(),
                            request.operation_key.to_string(),
                        ),
                        (
                            warning_keys::CLASSIFICATION.owned(),
                            failure_class.code().to_string(),
                        ),
                        (
                            warning_keys::SOURCE_FEATURE_ID.owned(),
                            request.source_feature_id.to_string(),
                        ),
                    ]),
                }],
            });
        }
        return Err(CadError::EvalFailed {
            reason: format!(
                "{}: {} value {:.6}mm exceeded threshold {:.6}mm (class={}) remediation={}",
                request.operation_key,
                request.operation_label,
                request.value_mm,
                request.risk_threshold_mm,
                failure_class.code(),
                failure_class.remediation_hint()
            ),
        });
    }

    let payload = format!(
        "{}|feature={}|source={}|src_hash={}|value_mm={:.6}|threshold={:.6}",
        request.operation_key,
        request.feature_id,
        request.source_feature_id,
        request.context.source_geometry_hash,
        request.value_mm,
        request.risk_threshold_mm
    );
    let geometry_hash = stable_hex_digest(payload.as_bytes());
    Ok(FinishingFeatureResult {
        feature_id: request.feature_id.to_string(),
        source_feature_id: request.source_feature_id.to_string(),
        operation_key: request.operation_key.to_string(),
        status: FinishingStatus::Applied,
        geometry_hash,
        failure_classification: None,
        fallback_message: None,
        remediation_hint: "operation applied".to_string(),
        warnings: Vec::new(),
    })
}

fn validate_finishing_refs(
    feature_id: &str,
    source_feature_id: &str,
    value_param: &str,
    refs: &[String],
    label: &str,
) -> CadResult<()> {
    validate_stable_id(feature_id, "feature_id")?;
    validate_stable_id(source_feature_id, "source_feature_id")?;
    validate_stable_id(value_param, "value_param")?;
    if refs.is_empty() {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{label} refs must not be empty"),
        });
    }
    for value in refs {
        validate_stable_id(value, "reference id")?;
    }
    Ok(())
}

fn required_param(node: &FeatureNode, key: &str) -> CadResult<String> {
    node.params
        .get(key)
        .cloned()
        .ok_or_else(|| CadError::InvalidPrimitive {
            reason: format!("node {} missing param {}", node.id, key),
        })
}

fn parse_csv_refs(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_bool(value: String) -> CadResult<bool> {
    match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(CadError::InvalidPrimitive {
            reason: format!("invalid bool value: {}", value),
        }),
    }
}

fn canonical_refs(values: &[String]) -> Vec<String> {
    let mut refs = BTreeSet::<String>::new();
    for value in values {
        refs.insert(value.clone());
    }
    refs.into_iter().collect()
}

fn validate_stable_id(value: &str, label: &str) -> CadResult<()> {
    if value.trim().is_empty() {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{label} must not be empty"),
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(CadError::InvalidPrimitive {
            reason: format!("{label} has invalid characters: {value}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CHAMFER_OPERATION_KEY, ChamferFeatureOp, FILLET_OPERATION_KEY, FilletFeatureOp,
        FinishingContext, FinishingFailureClass, FinishingStatus, SHELL_OPERATION_KEY,
        ShellFeatureOp, evaluate_chamfer_feature, evaluate_fillet_feature, evaluate_shell_feature,
    };
    use crate::CadError;
    use crate::contracts::CadWarningCode;
    use crate::params::{ParameterStore, ScalarUnit, ScalarValue};

    fn params() -> ParameterStore {
        let mut params = ParameterStore::default();
        params
            .set(
                "fillet_radius_mm",
                ScalarValue {
                    value: 2.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("fillet radius");
        params
            .set(
                "chamfer_distance_mm",
                ScalarValue {
                    value: 1.5,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("chamfer distance");
        params
            .set(
                "shell_thickness_mm",
                ScalarValue {
                    value: 1.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("shell thickness");
        params
    }

    fn context() -> FinishingContext {
        FinishingContext {
            source_geometry_hash: "hash.source.001".to_string(),
            source_min_thickness_mm: Some(20.0),
            source_volume_mm3: Some(2_000_000.0),
        }
    }

    #[test]
    fn fillet_applies_with_safe_radius() {
        let op = FilletFeatureOp {
            feature_id: "feature.fillet".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "fillet_radius_mm".to_string(),
            edge_refs: vec!["edge.1".to_string(), "edge.2".to_string()],
            allow_fallback: true,
        };
        let result =
            evaluate_fillet_feature(&op, &params(), &context()).expect("fillet should apply");
        assert_eq!(result.operation_key, FILLET_OPERATION_KEY);
        assert_eq!(result.status, FinishingStatus::Applied);
        assert!(result.failure_classification.is_none());
        assert!(result.fallback_message.is_none());
        assert_ne!(result.geometry_hash, "hash.source.001");
    }

    #[test]
    fn fillet_risk_uses_fallback_with_warning_when_enabled() {
        let mut params = params();
        params
            .set(
                "fillet_radius_mm",
                ScalarValue {
                    value: 12.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("fillet radius override");
        let op = FilletFeatureOp {
            feature_id: "feature.fillet".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "fillet_radius_mm".to_string(),
            edge_refs: vec!["edge.1".to_string()],
            allow_fallback: true,
        };
        let result = evaluate_fillet_feature(&op, &params, &context())
            .expect("fallback should return deterministic result");
        assert_eq!(result.status, FinishingStatus::FallbackKeptSource);
        assert_eq!(result.geometry_hash, "hash.source.001");
        assert_eq!(
            result.failure_classification,
            Some(FinishingFailureClass::TopologyRisk)
        );
        assert!(
            result
                .fallback_message
                .as_deref()
                .unwrap_or_default()
                .contains("fallback kept source geometry")
        );
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].code, CadWarningCode::FilletFailed);
    }

    #[test]
    fn fillet_risk_returns_classified_error_when_fallback_disabled() {
        let mut params = params();
        params
            .set(
                "fillet_radius_mm",
                ScalarValue {
                    value: 15.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("fillet radius override");
        let op = FilletFeatureOp {
            feature_id: "feature.fillet".to_string(),
            source_feature_id: "feature.base".to_string(),
            radius_param: "fillet_radius_mm".to_string(),
            edge_refs: vec!["edge.1".to_string()],
            allow_fallback: false,
        };
        let error = evaluate_fillet_feature(&op, &params, &context()).expect_err("must fail");
        match error {
            CadError::EvalFailed { reason } => {
                assert!(reason.contains(FinishingFailureClass::TopologyRisk.code()));
                assert!(reason.contains("Reduce operation size"));
            }
            _ => panic!("expected EvalFailed for classified fillet risk"),
        }
    }

    #[test]
    fn chamfer_node_round_trip_and_eval_is_deterministic() {
        let op = ChamferFeatureOp {
            feature_id: "feature.chamfer".to_string(),
            source_feature_id: "feature.base".to_string(),
            distance_param: "chamfer_distance_mm".to_string(),
            edge_refs: vec![
                "edge.2".to_string(),
                "edge.1".to_string(),
                "edge.1".to_string(),
            ],
            allow_fallback: false,
        };
        let node = op.to_feature_node().expect("node should build");
        assert_eq!(node.operation_key, CHAMFER_OPERATION_KEY);
        let parsed = ChamferFeatureOp::from_feature_node(&node).expect("node should parse");
        assert_eq!(
            parsed.edge_refs,
            vec!["edge.1".to_string(), "edge.2".to_string()]
        );
        let result = evaluate_chamfer_feature(&parsed, &params(), &context())
            .expect("safe chamfer should apply");
        assert_eq!(result.status, FinishingStatus::Applied);
    }

    #[test]
    fn shell_fallback_classification_and_message_are_explicit() {
        let mut params = params();
        params
            .set(
                "shell_thickness_mm",
                ScalarValue {
                    value: 10.0,
                    unit: ScalarUnit::Millimeter,
                },
            )
            .expect("shell override");
        let op = ShellFeatureOp {
            feature_id: "feature.shell".to_string(),
            source_feature_id: "feature.base".to_string(),
            thickness_param: "shell_thickness_mm".to_string(),
            remove_face_refs: vec!["face.001".to_string()],
            allow_fallback: true,
        };
        let node = op.to_feature_node().expect("shell node should build");
        assert_eq!(node.operation_key, SHELL_OPERATION_KEY);
        let parsed = ShellFeatureOp::from_feature_node(&node).expect("shell node should parse");
        let result = evaluate_shell_feature(&parsed, &params, &context())
            .expect("shell fallback should succeed");
        assert_eq!(result.status, FinishingStatus::FallbackKeptSource);
        assert_eq!(
            result.failure_classification,
            Some(FinishingFailureClass::ZeroThicknessRisk)
        );
        assert!(
            result
                .fallback_message
                .as_deref()
                .unwrap_or_default()
                .contains("fallback kept source geometry")
        );
    }
}
