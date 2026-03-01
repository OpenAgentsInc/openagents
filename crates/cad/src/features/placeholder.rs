use std::collections::BTreeMap;

use crate::feature_graph::FeatureNode;
use crate::hash::stable_hex_digest;
use crate::keys::feature_params as feature_keys;
use crate::params::{ParameterStore, ScalarUnit};
use crate::{CadError, CadResult};

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
            (feature_keys::KIND.owned(), self.kind.as_str().to_string()),
            (
                feature_keys::RADIUS_PARAM.owned(),
                self.radius_param.clone(),
            ),
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
                reason: format!("feature {} is not a fillet placeholder operation", node.id),
            });
        }
        let source_feature_id =
            node.depends_on
                .first()
                .cloned()
                .ok_or_else(|| CadError::InvalidPrimitive {
                    reason: format!(
                        "fillet placeholder node {} must depend on exactly one source feature",
                        node.id
                    ),
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
            .get(feature_keys::KIND.as_str())
            .ok_or_else(|| CadError::InvalidPrimitive {
                reason: format!("fillet placeholder node {} missing kind param", node.id),
            })
            .and_then(|value| FilletPlaceholderKind::parse(value))?;
        let radius_param = node
            .params
            .get(feature_keys::RADIUS_PARAM.as_str())
            .cloned()
            .ok_or_else(|| CadError::InvalidPrimitive {
                reason: format!("fillet placeholder node {} missing radius_param", node.id),
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
        stable_hex_digest(payload.as_bytes())
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
