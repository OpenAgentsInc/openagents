use std::collections::{BTreeMap, BTreeSet};

use crate::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};
use crate::feature_graph::FeatureNode;
use crate::hash::stable_hex_digest;
use crate::history::CadHistoryCommand;
use crate::keys::warning_metadata as warning_keys;
use crate::sketch::{CadSketchEntity, CadSketchModel};
use crate::{CadError, CadResult};

pub const SKETCH_EXTRUDE_OPERATION_KEY: &str = "sketch.extrude.v1";
pub const SKETCH_CUT_OPERATION_KEY: &str = "sketch.cut.v1";
pub const SKETCH_REVOLVE_OPERATION_KEY: &str = "sketch.revolve.v1";
pub const SKETCH_SWEEP_OPERATION_KEY: &str = "sketch.sweep.v1";
pub const SKETCH_LOFT_OPERATION_KEY: &str = "sketch.loft.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchProfileFeatureKind {
    Extrude,
    Cut,
    Revolve,
    Sweep,
    Loft,
}

impl SketchProfileFeatureKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Extrude => "extrude",
            Self::Cut => "cut",
            Self::Revolve => "revolve",
            Self::Sweep => "sweep",
            Self::Loft => "loft",
        }
    }

    fn operation_key(self) -> &'static str {
        match self {
            Self::Extrude => SKETCH_EXTRUDE_OPERATION_KEY,
            Self::Cut => SKETCH_CUT_OPERATION_KEY,
            Self::Revolve => SKETCH_REVOLVE_OPERATION_KEY,
            Self::Sweep => SKETCH_SWEEP_OPERATION_KEY,
            Self::Loft => SKETCH_LOFT_OPERATION_KEY,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SketchProfileFeatureSpec {
    pub feature_id: String,
    pub profile_id: String,
    pub plane_id: String,
    pub profile_entity_ids: Vec<String>,
    pub kind: SketchProfileFeatureKind,
    pub source_feature_id: Option<String>,
    pub depth_mm: Option<f64>,
    pub revolve_angle_deg: Option<f64>,
    pub axis_anchor_ids: Option<[String; 2]>,
    pub sweep_path_entity_ids: Option<Vec<String>>,
    pub sweep_twist_deg: Option<f64>,
    pub sweep_scale_start: Option<f64>,
    pub sweep_scale_end: Option<f64>,
    pub loft_profile_ids: Option<Vec<String>>,
    pub loft_closed: Option<bool>,
    pub tolerance_mm: Option<f64>,
}

impl SketchProfileFeatureSpec {
    pub fn validate(&self) -> CadResult<()> {
        validate_stable_id(&self.feature_id, "feature_id")?;
        validate_stable_id(&self.profile_id, "profile_id")?;
        validate_stable_id(&self.plane_id, "plane_id")?;
        if self.profile_entity_ids.is_empty() {
            return Err(CadError::ParseFailed {
                reason: "profile_entity_ids must not be empty".to_string(),
            });
        }
        let mut seen_profile_entity_ids = BTreeSet::<String>::new();
        for entity_id in &self.profile_entity_ids {
            validate_stable_id(entity_id, "profile_entity_id")?;
            if !seen_profile_entity_ids.insert(entity_id.clone()) {
                return Err(CadError::ParseFailed {
                    reason: format!("profile_entity_ids must not contain duplicates: {entity_id}"),
                });
            }
        }
        match self.kind {
            SketchProfileFeatureKind::Extrude => {
                validate_positive_opt(self.depth_mm, "extrude depth_mm")?;
                if self.source_feature_id.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "extrude operations must not specify source_feature_id".to_string(),
                    });
                }
                if self.revolve_angle_deg.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "extrude operations must not specify revolve_angle_deg".to_string(),
                    });
                }
                if self.axis_anchor_ids.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "extrude operations must not specify axis_anchor_ids".to_string(),
                    });
                }
            }
            SketchProfileFeatureKind::Cut => {
                validate_positive_opt(self.depth_mm, "cut depth_mm")?;
                let source_feature_id =
                    self.source_feature_id
                        .as_deref()
                        .ok_or_else(|| CadError::ParseFailed {
                            reason: "cut operations require source_feature_id".to_string(),
                        })?;
                validate_stable_id(source_feature_id, "source_feature_id")?;
                if self.revolve_angle_deg.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "cut operations must not specify revolve_angle_deg".to_string(),
                    });
                }
                if self.axis_anchor_ids.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "cut operations must not specify axis_anchor_ids".to_string(),
                    });
                }
            }
            SketchProfileFeatureKind::Revolve => {
                if self.depth_mm.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "revolve operations must not specify depth_mm".to_string(),
                    });
                }
                if self.source_feature_id.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "revolve operations must not specify source_feature_id".to_string(),
                    });
                }
                let angle = self
                    .revolve_angle_deg
                    .ok_or_else(|| CadError::ParseFailed {
                        reason: "revolve operations require revolve_angle_deg".to_string(),
                    })?;
                if !angle.is_finite() || angle <= 0.0 || angle > 360.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!(
                            "revolve_angle_deg must be finite and in (0, 360], got {angle}"
                        ),
                    });
                }
                let anchors =
                    self.axis_anchor_ids
                        .as_ref()
                        .ok_or_else(|| CadError::ParseFailed {
                            reason: "revolve operations require axis_anchor_ids".to_string(),
                        })?;
                validate_stable_id(&anchors[0], "axis_anchor_ids[0]")?;
                validate_stable_id(&anchors[1], "axis_anchor_ids[1]")?;
                if anchors[0] == anchors[1] {
                    return Err(CadError::ParseFailed {
                        reason: "axis_anchor_ids must reference two distinct anchors".to_string(),
                    });
                }
            }
            SketchProfileFeatureKind::Sweep => {
                if self.source_feature_id.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "sweep operations must not specify source_feature_id".to_string(),
                    });
                }
                if self.depth_mm.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "sweep operations must not specify depth_mm".to_string(),
                    });
                }
                if self.revolve_angle_deg.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "sweep operations must not specify revolve_angle_deg".to_string(),
                    });
                }
                if self.axis_anchor_ids.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "sweep operations must not specify axis_anchor_ids".to_string(),
                    });
                }
                let path_entity_ids =
                    self.sweep_path_entity_ids
                        .as_ref()
                        .ok_or_else(|| CadError::ParseFailed {
                            reason: "sweep operations require sweep_path_entity_ids".to_string(),
                        })?;
                if path_entity_ids.is_empty() {
                    return Err(CadError::ParseFailed {
                        reason: "sweep_path_entity_ids must not be empty".to_string(),
                    });
                }
                for entity_id in path_entity_ids {
                    validate_stable_id(entity_id, "sweep_path_entity_id")?;
                }
                if let Some(twist_deg) = self.sweep_twist_deg
                    && !twist_deg.is_finite()
                {
                    return Err(CadError::ParseFailed {
                        reason: "sweep_twist_deg must be finite when provided".to_string(),
                    });
                }
                if let Some(scale_start) = self.sweep_scale_start
                    && (!scale_start.is_finite() || scale_start <= 0.0)
                {
                    return Err(CadError::ParseFailed {
                        reason: "sweep_scale_start must be finite and > 0 when provided"
                            .to_string(),
                    });
                }
                if let Some(scale_end) = self.sweep_scale_end
                    && (!scale_end.is_finite() || scale_end <= 0.0)
                {
                    return Err(CadError::ParseFailed {
                        reason: "sweep_scale_end must be finite and > 0 when provided".to_string(),
                    });
                }
            }
            SketchProfileFeatureKind::Loft => {
                if self.source_feature_id.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "loft operations must not specify source_feature_id".to_string(),
                    });
                }
                if self.depth_mm.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "loft operations must not specify depth_mm".to_string(),
                    });
                }
                if self.revolve_angle_deg.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "loft operations must not specify revolve_angle_deg".to_string(),
                    });
                }
                if self.axis_anchor_ids.is_some() {
                    return Err(CadError::ParseFailed {
                        reason: "loft operations must not specify axis_anchor_ids".to_string(),
                    });
                }
                if self.sweep_path_entity_ids.is_some()
                    || self.sweep_twist_deg.is_some()
                    || self.sweep_scale_start.is_some()
                    || self.sweep_scale_end.is_some()
                {
                    return Err(CadError::ParseFailed {
                        reason: "loft operations must not specify sweep controls".to_string(),
                    });
                }
                let loft_profile_ids =
                    self.loft_profile_ids
                        .as_ref()
                        .ok_or_else(|| CadError::ParseFailed {
                            reason: "loft operations require loft_profile_ids".to_string(),
                        })?;
                if loft_profile_ids.is_empty() {
                    return Err(CadError::ParseFailed {
                        reason: "loft_profile_ids must include at least one secondary profile"
                            .to_string(),
                    });
                }
                let mut seen = BTreeSet::<String>::new();
                for profile_id in loft_profile_ids {
                    validate_stable_id(profile_id, "loft_profile_id")?;
                    if profile_id == &self.profile_id {
                        return Err(CadError::ParseFailed {
                            reason: "loft_profile_ids must not include the primary profile_id"
                                .to_string(),
                        });
                    }
                    if !seen.insert(profile_id.clone()) {
                        return Err(CadError::ParseFailed {
                            reason: format!("duplicate loft_profile_id: {profile_id}"),
                        });
                    }
                }
            }
        }
        if let Some(source) = self.source_feature_id.as_deref() {
            validate_stable_id(source, "source_feature_id")?;
        }
        if let Some(tolerance) = self.tolerance_mm
            && (!tolerance.is_finite() || tolerance <= 0.0)
        {
            return Err(CadError::ParseFailed {
                reason: "tolerance_mm must be finite and > 0 when provided".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SketchProfileFeatureConversion {
    pub node: FeatureNode,
    pub warnings: Vec<CadWarning>,
    pub profile_bounds_mm: [f64; 4],
    pub profile_hash: String,
}

pub fn history_command_for_sketch_feature(spec: &SketchProfileFeatureSpec) -> CadHistoryCommand {
    CadHistoryCommand::ApplySketchFeature {
        operation_key: spec.kind.operation_key().to_string(),
        profile_id: spec.profile_id.clone(),
        feature_id: spec.feature_id.clone(),
    }
}

pub fn convert_sketch_profile_to_feature_node(
    sketch: &CadSketchModel,
    spec: &SketchProfileFeatureSpec,
) -> CadResult<SketchProfileFeatureConversion> {
    spec.validate()?;

    let mut solved = sketch.clone();
    let solve_report = solved.solve_constraints_deterministic()?;
    if !solve_report.passed {
        return Err(CadError::EvalFailed {
            reason: format!(
                "sketch profile {} has unsolved constraints; conversion blocked",
                spec.profile_id
            ),
        });
    }
    if let Some(path_entity_ids) = spec.sweep_path_entity_ids.as_ref() {
        for entity_id in path_entity_ids {
            if !solved.entities.contains_key(entity_id) {
                return Err(CadError::ParseFailed {
                    reason: format!("sweep path references unknown sketch entity {entity_id}"),
                });
            }
        }
    }

    let points = collect_profile_points(&solved, &spec.profile_entity_ids)?;
    let bounds = profile_bounds(&points)?;
    let closed_loop = profile_is_closed_loop(&solved, &spec.profile_entity_ids)?;
    let profile_hash = sketch_profile_hash(spec, bounds);

    let mut params = BTreeMap::<String, String>::new();
    params.insert("feature_kind".to_string(), spec.kind.as_str().to_string());
    params.insert("profile_id".to_string(), spec.profile_id.clone());
    params.insert("plane_id".to_string(), spec.plane_id.clone());
    params.insert(
        "entity_ids".to_string(),
        sorted_ids(&spec.profile_entity_ids).join(","),
    );
    params.insert("profile_hash".to_string(), profile_hash.clone());
    params.insert("profile_closed_loop".to_string(), closed_loop.to_string());
    params.insert("bounds_min_x_mm".to_string(), format!("{:.6}", bounds[0]));
    params.insert("bounds_max_x_mm".to_string(), format!("{:.6}", bounds[1]));
    params.insert("bounds_min_y_mm".to_string(), format!("{:.6}", bounds[2]));
    params.insert("bounds_max_y_mm".to_string(), format!("{:.6}", bounds[3]));

    if let Some(depth_mm) = spec.depth_mm {
        params.insert("depth_mm".to_string(), format!("{depth_mm:.6}"));
    }
    if let Some(angle) = spec.revolve_angle_deg {
        params.insert("revolve_angle_deg".to_string(), format!("{angle:.6}"));
    }
    if let Some(anchors) = spec.axis_anchor_ids.as_ref() {
        params.insert("axis_anchor_a".to_string(), anchors[0].clone());
        params.insert("axis_anchor_b".to_string(), anchors[1].clone());
    }
    if let Some(path_entity_ids) = spec.sweep_path_entity_ids.as_ref() {
        params.insert(
            "sweep_path_entity_ids".to_string(),
            sorted_ids(path_entity_ids).join(","),
        );
    }
    if let Some(twist_deg) = spec.sweep_twist_deg {
        params.insert("sweep_twist_deg".to_string(), format!("{twist_deg:.6}"));
    }
    if let Some(scale_start) = spec.sweep_scale_start {
        params.insert("sweep_scale_start".to_string(), format!("{scale_start:.6}"));
    }
    if let Some(scale_end) = spec.sweep_scale_end {
        params.insert("sweep_scale_end".to_string(), format!("{scale_end:.6}"));
    }
    if let Some(loft_profile_ids) = spec.loft_profile_ids.as_ref() {
        params.insert("loft_profile_ids".to_string(), loft_profile_ids.join(","));
    }
    if let Some(loft_closed) = spec.loft_closed {
        params.insert("loft_closed".to_string(), loft_closed.to_string());
    }
    if let Some(tolerance) = spec.tolerance_mm {
        params.insert("tolerance_mm".to_string(), format!("{tolerance:.6}"));
    }

    let mut warnings = Vec::<CadWarning>::new();
    if !closed_loop
        && matches!(
            spec.kind,
            SketchProfileFeatureKind::Extrude
                | SketchProfileFeatureKind::Cut
                | SketchProfileFeatureKind::Sweep
                | SketchProfileFeatureKind::Loft
        )
    {
        warnings.push(CadWarning {
            code: CadWarningCode::NonManifoldBody,
            severity: CadWarningSeverity::Warning,
            message: format!(
                "sketch profile {} is open; {} may generate non-manifold geometry",
                spec.profile_id,
                spec.kind.as_str()
            ),
            remediation_hint: "close the profile loop or constrain dangling endpoints".to_string(),
            semantic_refs: vec![format!("cad://sketch/profile/{}", spec.profile_id)],
            metadata: BTreeMap::from([
                (warning_keys::FEATURE_ID.owned(), spec.feature_id.clone()),
                (
                    warning_keys::WARNING_DOMAIN.owned(),
                    "sketch-profile".to_string(),
                ),
            ]),
        });
    }
    if matches!(spec.kind, SketchProfileFeatureKind::Revolve)
        && spec.revolve_angle_deg.unwrap_or(360.0) < 360.0
    {
        warnings.push(CadWarning {
            code: CadWarningCode::SliverFace,
            severity: CadWarningSeverity::Info,
            message: format!(
                "revolve profile {} uses partial angle; inspect seam faces",
                spec.profile_id
            ),
            remediation_hint: "increase revolve angle or add blend cleanup features".to_string(),
            semantic_refs: vec![format!("cad://sketch/profile/{}", spec.profile_id)],
            metadata: BTreeMap::from([
                (warning_keys::FEATURE_ID.owned(), spec.feature_id.clone()),
                (
                    "revolve_angle_deg".to_string(),
                    format!("{:.6}", spec.revolve_angle_deg.unwrap_or(360.0)),
                ),
            ]),
        });
    }

    let depends_on = match spec.kind {
        SketchProfileFeatureKind::Cut => {
            vec![
                spec.source_feature_id
                    .clone()
                    .ok_or_else(|| CadError::ParseFailed {
                        reason: format!(
                            "sketch feature {} of kind {} requires source_feature_id",
                            spec.feature_id,
                            spec.kind.as_str()
                        ),
                    })?,
            ]
        }
        _ => spec.source_feature_id.iter().cloned().collect(),
    };

    let node = FeatureNode {
        id: spec.feature_id.clone(),
        name: format!("Sketch {} {}", spec.kind.as_str(), spec.profile_id),
        operation_key: spec.kind.operation_key().to_string(),
        depends_on,
        params,
    };

    Ok(SketchProfileFeatureConversion {
        node,
        warnings,
        profile_bounds_mm: bounds,
        profile_hash,
    })
}

fn collect_profile_points(
    sketch: &CadSketchModel,
    entity_ids: &[String],
) -> CadResult<Vec<[f64; 2]>> {
    let mut points = Vec::<[f64; 2]>::new();
    for entity_id in entity_ids {
        let entity = sketch
            .entities
            .get(entity_id)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!("profile references unknown sketch entity {entity_id}"),
            })?;
        match entity {
            CadSketchEntity::Line {
                start_mm, end_mm, ..
            } => {
                if (start_mm[0] - end_mm[0]).abs() <= f64::EPSILON
                    && (start_mm[1] - end_mm[1]).abs() <= f64::EPSILON
                {
                    return Err(CadError::ParseFailed {
                        reason: format!("line {entity_id} is degenerate (zero length)"),
                    });
                }
                points.push(*start_mm);
                points.push(*end_mm);
            }
            CadSketchEntity::Arc {
                center_mm,
                radius_mm,
                start_deg,
                end_deg,
                ..
            } => {
                if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("arc {entity_id} has invalid radius"),
                    });
                }
                points.push(*center_mm);
                points.push(arc_point(*center_mm, *radius_mm, *start_deg)?);
                points.push(arc_point(*center_mm, *radius_mm, *end_deg)?);
            }
            CadSketchEntity::Rectangle { min_mm, max_mm, .. } => {
                let width = max_mm[0] - min_mm[0];
                let height = max_mm[1] - min_mm[1];
                if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("rectangle {entity_id} has non-positive extents"),
                    });
                }
                points.push(*min_mm);
                points.push([max_mm[0], min_mm[1]]);
                points.push(*max_mm);
                points.push([min_mm[0], max_mm[1]]);
            }
            CadSketchEntity::Circle {
                center_mm,
                radius_mm,
                ..
            } => {
                if !radius_mm.is_finite() || *radius_mm <= 0.0 {
                    return Err(CadError::ParseFailed {
                        reason: format!("circle {entity_id} has invalid radius"),
                    });
                }
                points.push(*center_mm);
                points.push([center_mm[0] + *radius_mm, center_mm[1]]);
                points.push([center_mm[0] - *radius_mm, center_mm[1]]);
                points.push([center_mm[0], center_mm[1] + *radius_mm]);
                points.push([center_mm[0], center_mm[1] - *radius_mm]);
            }
            CadSketchEntity::Spline {
                control_points_mm, ..
            } => {
                if control_points_mm.is_empty() {
                    return Err(CadError::ParseFailed {
                        reason: format!("spline {entity_id} has no control points"),
                    });
                }
                for point in control_points_mm {
                    points.push(*point);
                }
            }
            CadSketchEntity::Point { position_mm, .. } => {
                points.push(*position_mm);
            }
        }
    }
    Ok(points)
}

fn profile_bounds(points: &[[f64; 2]]) -> CadResult<[f64; 4]> {
    if points.is_empty() {
        return Err(CadError::ParseFailed {
            reason: "profile has no points".to_string(),
        });
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        if !point[0].is_finite() || !point[1].is_finite() {
            return Err(CadError::ParseFailed {
                reason: "profile contains non-finite point".to_string(),
            });
        }
        min_x = min_x.min(point[0]);
        max_x = max_x.max(point[0]);
        min_y = min_y.min(point[1]);
        max_y = max_y.max(point[1]);
    }
    Ok([min_x, max_x, min_y, max_y])
}

fn profile_is_closed_loop(sketch: &CadSketchModel, entity_ids: &[String]) -> CadResult<bool> {
    if entity_ids.is_empty() {
        return Ok(false);
    }

    let mut endpoint_anchor_ids = BTreeSet::<String>::new();
    let mut all_intrinsically_closed = true;
    for entity_id in entity_ids {
        let entity = sketch
            .entities
            .get(entity_id)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!("profile references unknown sketch entity {entity_id}"),
            })?;
        match entity {
            CadSketchEntity::Line { anchor_ids, .. } => {
                endpoint_anchor_ids.insert(anchor_ids[0].clone());
                endpoint_anchor_ids.insert(anchor_ids[1].clone());
                all_intrinsically_closed = false;
            }
            CadSketchEntity::Arc { anchor_ids, .. } => {
                endpoint_anchor_ids.insert(anchor_ids[1].clone());
                endpoint_anchor_ids.insert(anchor_ids[2].clone());
                all_intrinsically_closed = false;
            }
            CadSketchEntity::Spline {
                anchor_ids, closed, ..
            } => {
                if *closed {
                    continue;
                }
                if anchor_ids.len() < 2 {
                    return Ok(false);
                }
                endpoint_anchor_ids.insert(anchor_ids[0].clone());
                endpoint_anchor_ids.insert(anchor_ids[anchor_ids.len() - 1].clone());
                all_intrinsically_closed = false;
            }
            CadSketchEntity::Rectangle { .. } | CadSketchEntity::Circle { .. } => {}
            CadSketchEntity::Point { .. } => {
                all_intrinsically_closed = false;
            }
        }
    }

    if endpoint_anchor_ids.is_empty() {
        return Ok(all_intrinsically_closed);
    }

    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for anchor in &endpoint_anchor_ids {
        adjacency.insert(anchor.clone(), BTreeSet::new());
    }
    for constraint in sketch.constraints.values() {
        if let crate::sketch::CadSketchConstraint::Coincident {
            first_anchor_id,
            second_anchor_id,
            ..
        } = constraint
            && endpoint_anchor_ids.contains(first_anchor_id)
            && endpoint_anchor_ids.contains(second_anchor_id)
        {
            adjacency
                .entry(first_anchor_id.clone())
                .or_default()
                .insert(second_anchor_id.clone());
            adjacency
                .entry(second_anchor_id.clone())
                .or_default()
                .insert(first_anchor_id.clone());
        }
    }

    let mut counts = BTreeMap::<String, usize>::new();
    for entity_id in entity_ids {
        let entity = sketch
            .entities
            .get(entity_id)
            .ok_or_else(|| CadError::ParseFailed {
                reason: format!("profile references unknown sketch entity {entity_id}"),
            })?;
        match entity {
            CadSketchEntity::Line { anchor_ids, .. } => {
                for anchor in anchor_ids {
                    let canonical = canonical_anchor(anchor, &adjacency);
                    *counts.entry(canonical).or_insert(0) += 1;
                }
            }
            CadSketchEntity::Arc { anchor_ids, .. } => {
                for anchor in [&anchor_ids[1], &anchor_ids[2]] {
                    let canonical = canonical_anchor(anchor, &adjacency);
                    *counts.entry(canonical).or_insert(0) += 1;
                }
            }
            CadSketchEntity::Spline {
                anchor_ids, closed, ..
            } => {
                if *closed {
                    continue;
                }
                if anchor_ids.len() < 2 {
                    return Ok(false);
                }
                for anchor in [&anchor_ids[0], &anchor_ids[anchor_ids.len() - 1]] {
                    let canonical = canonical_anchor(anchor, &adjacency);
                    *counts.entry(canonical).or_insert(0) += 1;
                }
            }
            CadSketchEntity::Rectangle { .. } | CadSketchEntity::Circle { .. } => {}
            CadSketchEntity::Point { .. } => {
                return Ok(false);
            }
        }
    }
    if counts.is_empty() {
        return Ok(false);
    }
    Ok(counts.values().all(|count| *count == 2))
}

fn canonical_anchor(anchor: &str, adjacency: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut stack = vec![anchor.to_string()];
    let mut visited = BTreeSet::<String>::new();
    let mut canonical = anchor.to_string();
    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        if current < canonical {
            canonical.clone_from(&current);
        }
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    stack.push(neighbor.clone());
                }
            }
        }
    }
    canonical
}

fn sorted_ids(values: &[String]) -> Vec<String> {
    let mut set = BTreeSet::<String>::new();
    for value in values {
        set.insert(value.clone());
    }
    set.into_iter().collect()
}

fn sketch_profile_hash(spec: &SketchProfileFeatureSpec, bounds: [f64; 4]) -> String {
    let sweep_path_ids = spec
        .sweep_path_entity_ids
        .as_ref()
        .map(|ids| sorted_ids(ids).join(","))
        .unwrap_or_default();
    let loft_profile_ids = spec.loft_profile_ids.as_ref().map(|ids| ids.join(","));
    let payload = format!(
        "{}|{}|{}|{}|{:.6}|{:.6}|{:.6}|{:.6}|{}|{:?}|{:?}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}",
        spec.kind.as_str(),
        spec.feature_id,
        spec.profile_id,
        sorted_ids(&spec.profile_entity_ids).join(","),
        bounds[0],
        bounds[1],
        bounds[2],
        bounds[3],
        spec.source_feature_id.as_deref().unwrap_or(""),
        spec.depth_mm,
        spec.revolve_angle_deg,
        spec.axis_anchor_ids,
        sweep_path_ids,
        spec.sweep_twist_deg,
        spec.sweep_scale_start,
        spec.sweep_scale_end,
        loft_profile_ids,
        spec.loft_closed,
    );
    stable_hex_digest(payload.as_bytes())
}

fn validate_stable_id(value: &str, label: &str) -> CadResult<()> {
    if value.trim().is_empty() {
        return Err(CadError::ParseFailed {
            reason: format!("{label} must not be empty"),
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(CadError::ParseFailed {
            reason: format!("{label} contains invalid characters: {value}"),
        });
    }
    Ok(())
}

fn validate_positive_opt(value: Option<f64>, label: &str) -> CadResult<()> {
    let value = value.ok_or_else(|| CadError::ParseFailed {
        reason: format!("{label} is required"),
    })?;
    if !value.is_finite() || value <= 0.0 {
        return Err(CadError::ParseFailed {
            reason: format!("{label} must be finite and > 0"),
        });
    }
    Ok(())
}

fn arc_point(center: [f64; 2], radius_mm: f64, angle_deg: f64) -> CadResult<[f64; 2]> {
    if !angle_deg.is_finite() {
        return Err(CadError::ParseFailed {
            reason: "arc angle must be finite".to_string(),
        });
    }
    let angle = angle_deg.to_radians();
    Ok([
        center[0] + radius_mm * angle.cos(),
        center[1] + radius_mm * angle.sin(),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        SKETCH_CUT_OPERATION_KEY, SKETCH_EXTRUDE_OPERATION_KEY, SKETCH_LOFT_OPERATION_KEY,
        SKETCH_REVOLVE_OPERATION_KEY, SKETCH_SWEEP_OPERATION_KEY, SketchProfileFeatureKind,
        SketchProfileFeatureSpec, convert_sketch_profile_to_feature_node,
        history_command_for_sketch_feature,
    };
    use crate::contracts::CadWarningCode;
    use crate::history::CadHistoryCommand;
    use crate::sketch::{
        CadDimensionConstraintKind, CadSketchConstraint, CadSketchEntity, CadSketchModel,
        CadSketchPlane,
    };

    fn constrained_rectangle_model() -> CadSketchModel {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.a".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [40.0, 0.5],
                anchor_ids: ["anchor.a.start".to_string(), "anchor.a.end".to_string()],
                construction: false,
            })
            .expect("entity.a should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.b".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [40.0, 0.5],
                end_mm: [40.0, 20.0],
                anchor_ids: ["anchor.b.start".to_string(), "anchor.b.end".to_string()],
                construction: false,
            })
            .expect("entity.b should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.c".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [40.0, 20.0],
                end_mm: [0.0, 20.2],
                anchor_ids: ["anchor.c.start".to_string(), "anchor.c.end".to_string()],
                construction: false,
            })
            .expect("entity.c should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.d".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 20.2],
                end_mm: [0.0, 0.0],
                anchor_ids: ["anchor.d.start".to_string(), "anchor.d.end".to_string()],
                construction: false,
            })
            .expect("entity.d should insert");

        model
            .insert_constraint(CadSketchConstraint::Coincident {
                id: "constraint.01".to_string(),
                first_anchor_id: "anchor.a.end".to_string(),
                second_anchor_id: "anchor.b.start".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.01");
        model
            .insert_constraint(CadSketchConstraint::Coincident {
                id: "constraint.02".to_string(),
                first_anchor_id: "anchor.b.end".to_string(),
                second_anchor_id: "anchor.c.start".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.02");
        model
            .insert_constraint(CadSketchConstraint::Coincident {
                id: "constraint.03".to_string(),
                first_anchor_id: "anchor.c.end".to_string(),
                second_anchor_id: "anchor.d.start".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.03");
        model
            .insert_constraint(CadSketchConstraint::Coincident {
                id: "constraint.04".to_string(),
                first_anchor_id: "anchor.d.end".to_string(),
                second_anchor_id: "anchor.a.start".to_string(),
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.04");
        model
            .insert_constraint(CadSketchConstraint::Horizontal {
                id: "constraint.05".to_string(),
                line_entity_id: "entity.a".to_string(),
            })
            .expect("constraint.05");
        model
            .insert_constraint(CadSketchConstraint::Horizontal {
                id: "constraint.06".to_string(),
                line_entity_id: "entity.c".to_string(),
            })
            .expect("constraint.06");
        model
            .insert_constraint(CadSketchConstraint::Vertical {
                id: "constraint.07".to_string(),
                line_entity_id: "entity.b".to_string(),
            })
            .expect("constraint.07");
        model
            .insert_constraint(CadSketchConstraint::Vertical {
                id: "constraint.08".to_string(),
                line_entity_id: "entity.d".to_string(),
            })
            .expect("constraint.08");
        model
            .insert_constraint(CadSketchConstraint::Dimension {
                id: "constraint.09".to_string(),
                entity_id: "entity.a".to_string(),
                dimension_kind: CadDimensionConstraintKind::Length,
                target_mm: 40.0,
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.09");
        model
            .insert_constraint(CadSketchConstraint::Dimension {
                id: "constraint.10".to_string(),
                entity_id: "entity.b".to_string(),
                dimension_kind: CadDimensionConstraintKind::Length,
                target_mm: 20.0,
                tolerance_mm: Some(0.001),
            })
            .expect("constraint.10");
        let report = model
            .solve_constraints_deterministic()
            .expect("constraints should solve");
        assert!(report.passed, "rectangle constraints should pass");
        model
    }

    #[test]
    fn sketch_profile_spec_validation_rejects_cross_kind_parameters() {
        let extrude = SketchProfileFeatureSpec {
            feature_id: "feature.extrude".to_string(),
            profile_id: "profile.extrude".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.a".to_string()],
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(5.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let cut = SketchProfileFeatureSpec {
            feature_id: "feature.cut".to_string(),
            profile_id: "profile.cut".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.a".to_string()],
            kind: SketchProfileFeatureKind::Cut,
            source_feature_id: Some("feature.source".to_string()),
            depth_mm: Some(5.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let revolve = SketchProfileFeatureSpec {
            feature_id: "feature.revolve".to_string(),
            profile_id: "profile.revolve".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.a".to_string()],
            kind: SketchProfileFeatureKind::Revolve,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: Some(270.0),
            axis_anchor_ids: Some(["anchor.axis.a".to_string(), "anchor.axis.b".to_string()]),
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let sweep = SketchProfileFeatureSpec {
            feature_id: "feature.sweep".to_string(),
            profile_id: "profile.sweep".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.a".to_string()],
            kind: SketchProfileFeatureKind::Sweep,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: Some(vec!["entity.path".to_string()]),
            sweep_twist_deg: Some(30.0),
            sweep_scale_start: Some(1.0),
            sweep_scale_end: Some(0.5),
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let loft = SketchProfileFeatureSpec {
            feature_id: "feature.loft".to_string(),
            profile_id: "profile.loft".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.a".to_string()],
            kind: SketchProfileFeatureKind::Loft,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: Some(vec!["profile.loft.secondary".to_string()]),
            loft_closed: Some(true),
            tolerance_mm: Some(0.001),
        };

        let extrude_with_source = SketchProfileFeatureSpec {
            source_feature_id: Some("feature.source".to_string()),
            ..extrude.clone()
        };
        assert!(
            extrude_with_source
                .validate()
                .expect_err("extrude must reject source_feature_id")
                .to_string()
                .contains("source_feature_id")
        );

        let extrude_with_revolve_angle = SketchProfileFeatureSpec {
            revolve_angle_deg: Some(180.0),
            ..extrude.clone()
        };
        assert!(
            extrude_with_revolve_angle
                .validate()
                .expect_err("extrude must reject revolve_angle_deg")
                .to_string()
                .contains("revolve_angle_deg")
        );

        let extrude_with_axis = SketchProfileFeatureSpec {
            axis_anchor_ids: Some(["anchor.axis.a".to_string(), "anchor.axis.b".to_string()]),
            ..extrude
        };
        assert!(
            extrude_with_axis
                .validate()
                .expect_err("extrude must reject axis_anchor_ids")
                .to_string()
                .contains("axis_anchor_ids")
        );

        let cut_with_revolve_angle = SketchProfileFeatureSpec {
            revolve_angle_deg: Some(180.0),
            ..cut.clone()
        };
        assert!(
            cut_with_revolve_angle
                .validate()
                .expect_err("cut must reject revolve_angle_deg")
                .to_string()
                .contains("revolve_angle_deg")
        );

        let cut_with_axis = SketchProfileFeatureSpec {
            axis_anchor_ids: Some(["anchor.axis.a".to_string(), "anchor.axis.b".to_string()]),
            ..cut
        };
        assert!(
            cut_with_axis
                .validate()
                .expect_err("cut must reject axis_anchor_ids")
                .to_string()
                .contains("axis_anchor_ids")
        );

        let revolve_with_depth = SketchProfileFeatureSpec {
            depth_mm: Some(5.0),
            ..revolve.clone()
        };
        assert!(
            revolve_with_depth
                .validate()
                .expect_err("revolve must reject depth_mm")
                .to_string()
                .contains("depth_mm")
        );

        let revolve_with_source = SketchProfileFeatureSpec {
            source_feature_id: Some("feature.source".to_string()),
            ..revolve
        };
        assert!(
            revolve_with_source
                .validate()
                .expect_err("revolve must reject source_feature_id")
                .to_string()
                .contains("source_feature_id")
        );

        let sweep_with_source = SketchProfileFeatureSpec {
            source_feature_id: Some("feature.source".to_string()),
            ..sweep.clone()
        };
        assert!(
            sweep_with_source
                .validate()
                .expect_err("sweep must reject source_feature_id")
                .to_string()
                .contains("source_feature_id")
        );

        let sweep_with_empty_path = SketchProfileFeatureSpec {
            sweep_path_entity_ids: Some(Vec::new()),
            ..sweep.clone()
        };
        assert!(
            sweep_with_empty_path
                .validate()
                .expect_err("sweep must reject empty path")
                .to_string()
                .contains("sweep_path_entity_ids")
        );

        let sweep_with_non_positive_scale = SketchProfileFeatureSpec {
            sweep_scale_end: Some(0.0),
            ..sweep
        };
        assert!(
            sweep_with_non_positive_scale
                .validate()
                .expect_err("sweep must reject non-positive scale")
                .to_string()
                .contains("sweep_scale_end")
        );

        let loft_with_source = SketchProfileFeatureSpec {
            source_feature_id: Some("feature.source".to_string()),
            ..loft.clone()
        };
        assert!(
            loft_with_source
                .validate()
                .expect_err("loft must reject source_feature_id")
                .to_string()
                .contains("source_feature_id")
        );

        let loft_with_empty_profiles = SketchProfileFeatureSpec {
            loft_profile_ids: Some(Vec::new()),
            ..loft.clone()
        };
        assert!(
            loft_with_empty_profiles
                .validate()
                .expect_err("loft must reject empty loft_profile_ids")
                .to_string()
                .contains("loft_profile_ids")
        );

        let loft_with_primary_profile = SketchProfileFeatureSpec {
            loft_profile_ids: Some(vec!["profile.loft".to_string()]),
            ..loft.clone()
        };
        assert!(
            loft_with_primary_profile
                .validate()
                .expect_err("loft must reject primary profile in loft_profile_ids")
                .to_string()
                .contains("primary profile_id")
        );

        let loft_with_sweep_control = SketchProfileFeatureSpec {
            sweep_twist_deg: Some(15.0),
            ..loft
        };
        assert!(
            loft_with_sweep_control
                .validate()
                .expect_err("loft must reject sweep controls")
                .to_string()
                .contains("sweep controls")
        );

        let extrude_with_duplicate_entities = SketchProfileFeatureSpec {
            profile_entity_ids: vec!["entity.a".to_string(), "entity.a".to_string()],
            ..extrude_with_revolve_angle
        };
        assert!(
            extrude_with_duplicate_entities
                .validate()
                .expect_err("duplicate profile entities must fail validation")
                .to_string()
                .contains("duplicates")
        );
    }

    #[test]
    fn converts_constrained_profile_into_extrude_cut_revolve_feature_nodes() {
        let model = constrained_rectangle_model();
        let entity_ids = vec![
            "entity.c".to_string(),
            "entity.a".to_string(),
            "entity.d".to_string(),
            "entity.b".to_string(),
        ];
        let extrude = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.extrude".to_string(),
            profile_id: "profile.rack".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: entity_ids.clone(),
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(25.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let extrude_rev = SketchProfileFeatureSpec {
            profile_entity_ids: entity_ids.into_iter().rev().collect(),
            ..extrude.clone()
        };
        let conversion = convert_sketch_profile_to_feature_node(&model, &extrude)
            .expect("extrude conversion should succeed");
        let conversion_rev = convert_sketch_profile_to_feature_node(&model, &extrude_rev)
            .expect("reordered extrude conversion should succeed");
        assert_eq!(conversion.node.operation_key, SKETCH_EXTRUDE_OPERATION_KEY);
        assert!(
            conversion.warnings.is_empty(),
            "closed profile should not warn"
        );
        assert_eq!(
            conversion
                .node
                .params
                .get("profile_closed_loop")
                .map(String::as_str),
            Some("true")
        );
        assert_eq!(
            conversion.node.params.get("profile_hash"),
            conversion_rev.node.params.get("profile_hash"),
            "profile hash should be independent of profile entity insertion order"
        );

        let cut = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.cut".to_string(),
            profile_id: "profile.rack.cut".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec![
                "entity.a".to_string(),
                "entity.b".to_string(),
                "entity.c".to_string(),
                "entity.d".to_string(),
            ],
            kind: SketchProfileFeatureKind::Cut,
            source_feature_id: Some("feature.sketch.extrude".to_string()),
            depth_mm: Some(10.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let cut_conversion =
            convert_sketch_profile_to_feature_node(&model, &cut).expect("cut conversion");
        assert_eq!(cut_conversion.node.operation_key, SKETCH_CUT_OPERATION_KEY);
        assert_eq!(
            cut_conversion.node.depends_on,
            vec!["feature.sketch.extrude".to_string()]
        );

        let revolve = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.revolve".to_string(),
            profile_id: "profile.rack.revolve".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec![
                "entity.a".to_string(),
                "entity.b".to_string(),
                "entity.c".to_string(),
                "entity.d".to_string(),
            ],
            kind: SketchProfileFeatureKind::Revolve,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: Some(270.0),
            axis_anchor_ids: Some(["anchor.a.start".to_string(), "anchor.d.end".to_string()]),
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let revolve_conversion =
            convert_sketch_profile_to_feature_node(&model, &revolve).expect("revolve conversion");
        assert_eq!(
            revolve_conversion.node.operation_key,
            SKETCH_REVOLVE_OPERATION_KEY
        );
        assert!(
            revolve_conversion
                .warnings
                .iter()
                .any(|warning| warning.code == CadWarningCode::SliverFace),
            "partial revolve should emit seam/sliver advisory warning"
        );
    }

    #[test]
    fn sweep_conversion_maps_path_controls_and_open_profile_warning() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Rectangle {
                id: "entity.rect".to_string(),
                plane_id: "plane.front".to_string(),
                min_mm: [0.0, 0.0],
                max_mm: [40.0, 20.0],
                anchor_ids: [
                    "anchor.rect.00".to_string(),
                    "anchor.rect.10".to_string(),
                    "anchor.rect.11".to_string(),
                    "anchor.rect.01".to_string(),
                ],
                construction: false,
            })
            .expect("rectangle should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.path.a".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [60.0, 0.0],
                end_mm: [90.0, 20.0],
                anchor_ids: ["anchor.path.a".to_string(), "anchor.path.b".to_string()],
                construction: true,
            })
            .expect("path line should insert");

        let closed_spec = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.sweep.closed".to_string(),
            profile_id: "profile.sweep.closed".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.rect".to_string()],
            kind: SketchProfileFeatureKind::Sweep,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: Some(vec!["entity.path.a".to_string()]),
            sweep_twist_deg: Some(45.0),
            sweep_scale_start: Some(1.0),
            sweep_scale_end: Some(0.7),
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let closed_conversion = convert_sketch_profile_to_feature_node(&model, &closed_spec)
            .expect("closed sweep conversion should succeed");
        assert_eq!(
            closed_conversion.node.operation_key,
            SKETCH_SWEEP_OPERATION_KEY
        );
        assert_eq!(
            closed_conversion
                .node
                .params
                .get("sweep_path_entity_ids")
                .map(String::as_str),
            Some("entity.path.a")
        );
        assert_eq!(
            closed_conversion
                .node
                .params
                .get("sweep_twist_deg")
                .map(String::as_str),
            Some("45.000000")
        );
        assert!(
            closed_conversion
                .warnings
                .iter()
                .all(|warning| warning.code != CadWarningCode::NonManifoldBody),
            "closed sweep profile should not emit open-profile warning"
        );

        let missing_path_error = convert_sketch_profile_to_feature_node(
            &model,
            &SketchProfileFeatureSpec {
                sweep_path_entity_ids: Some(vec!["entity.path.missing".to_string()]),
                ..closed_spec.clone()
            },
        )
        .expect_err("missing sweep path entity must fail conversion")
        .to_string();
        assert!(missing_path_error.contains("unknown sketch entity"));

        let mut open_model = CadSketchModel::default();
        open_model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        open_model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.open".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [20.0, 0.0],
                anchor_ids: [
                    "anchor.open.start".to_string(),
                    "anchor.open.end".to_string(),
                ],
                construction: false,
            })
            .expect("open profile line should insert");
        open_model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.path.open".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [30.0, 0.0],
                end_mm: [50.0, 10.0],
                anchor_ids: [
                    "anchor.path.open.a".to_string(),
                    "anchor.path.open.b".to_string(),
                ],
                construction: true,
            })
            .expect("open path line should insert");
        let open_spec = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.sweep.open".to_string(),
            profile_id: "profile.sweep.open".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.open".to_string()],
            kind: SketchProfileFeatureKind::Sweep,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: Some(vec!["entity.path.open".to_string()]),
            sweep_twist_deg: Some(0.0),
            sweep_scale_start: Some(1.0),
            sweep_scale_end: Some(1.0),
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)
            .expect("open sweep profile should convert with warning");
        assert!(
            open_conversion
                .warnings
                .iter()
                .any(|warning| warning.code == CadWarningCode::NonManifoldBody)
        );
    }

    #[test]
    fn loft_conversion_maps_profile_ids_closed_mode_and_open_profile_warning() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Rectangle {
                id: "entity.rect".to_string(),
                plane_id: "plane.front".to_string(),
                min_mm: [0.0, 0.0],
                max_mm: [30.0, 15.0],
                anchor_ids: [
                    "anchor.rect.00".to_string(),
                    "anchor.rect.10".to_string(),
                    "anchor.rect.11".to_string(),
                    "anchor.rect.01".to_string(),
                ],
                construction: false,
            })
            .expect("rectangle should insert");

        let closed_spec = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.loft.closed".to_string(),
            profile_id: "profile.loft.closed".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.rect".to_string()],
            kind: SketchProfileFeatureKind::Loft,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: Some(vec![
                "profile.section.2".to_string(),
                "profile.section.3".to_string(),
            ]),
            loft_closed: Some(true),
            tolerance_mm: Some(0.001),
        };
        let closed_conversion = convert_sketch_profile_to_feature_node(&model, &closed_spec)
            .expect("closed loft conversion should succeed");
        assert_eq!(
            closed_conversion.node.operation_key,
            SKETCH_LOFT_OPERATION_KEY
        );
        assert_eq!(
            closed_conversion
                .node
                .params
                .get("loft_profile_ids")
                .map(String::as_str),
            Some("profile.section.2,profile.section.3")
        );
        assert_eq!(
            closed_conversion
                .node
                .params
                .get("loft_closed")
                .map(String::as_str),
            Some("true")
        );
        assert!(
            closed_conversion
                .warnings
                .iter()
                .all(|warning| warning.code != CadWarningCode::NonManifoldBody)
        );

        let mut open_model = CadSketchModel::default();
        open_model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("open plane should insert");
        open_model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.open".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [20.0, 0.0],
                anchor_ids: [
                    "anchor.open.start".to_string(),
                    "anchor.open.end".to_string(),
                ],
                construction: false,
            })
            .expect("open line should insert");

        let open_spec = SketchProfileFeatureSpec {
            feature_id: "feature.sketch.loft.open".to_string(),
            profile_id: "profile.loft.open".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.open".to_string()],
            kind: SketchProfileFeatureKind::Loft,
            source_feature_id: None,
            depth_mm: None,
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: Some(vec!["profile.section.2".to_string()]),
            loft_closed: Some(false),
            tolerance_mm: Some(0.001),
        };
        let open_conversion = convert_sketch_profile_to_feature_node(&open_model, &open_spec)
            .expect("open loft profile should convert with warning");
        assert!(
            open_conversion
                .warnings
                .iter()
                .any(|warning| warning.code == CadWarningCode::NonManifoldBody)
        );
    }

    #[test]
    fn degenerate_profile_entities_are_rejected_deterministically() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.line.degenerate".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [2.0, 3.0],
                end_mm: [2.5, 3.0],
                anchor_ids: [
                    "anchor.degenerate.start".to_string(),
                    "anchor.degenerate.end".to_string(),
                ],
                construction: false,
            })
            .expect("degenerate line should insert");
        if let CadSketchEntity::Line {
            start_mm, end_mm, ..
        } = model
            .entities
            .get_mut("entity.line.degenerate")
            .expect("degenerate line should exist")
        {
            *end_mm = *start_mm;
        } else {
            panic!("entity.line.degenerate must be a line");
        }

        let spec = SketchProfileFeatureSpec {
            feature_id: "feature.extrude.degenerate".to_string(),
            profile_id: "profile.degenerate".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.line.degenerate".to_string()],
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(5.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let error = convert_sketch_profile_to_feature_node(&model, &spec)
            .expect_err("degenerate line profile must fail conversion")
            .to_string();
        assert!(error.contains("degenerate"));
    }

    #[test]
    fn open_profile_emits_warning_for_extrude_and_maps_history_command() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Line {
                id: "entity.open".to_string(),
                plane_id: "plane.front".to_string(),
                start_mm: [0.0, 0.0],
                end_mm: [20.0, 0.0],
                anchor_ids: [
                    "anchor.open.start".to_string(),
                    "anchor.open.end".to_string(),
                ],
                construction: false,
            })
            .expect("open line should insert");
        let spec = SketchProfileFeatureSpec {
            feature_id: "feature.open.extrude".to_string(),
            profile_id: "profile.open".to_string(),
            plane_id: "plane.front".to_string(),
            profile_entity_ids: vec!["entity.open".to_string()],
            kind: SketchProfileFeatureKind::Extrude,
            source_feature_id: None,
            depth_mm: Some(8.0),
            revolve_angle_deg: None,
            axis_anchor_ids: None,
            sweep_path_entity_ids: None,
            sweep_twist_deg: None,
            sweep_scale_start: None,
            sweep_scale_end: None,
            loft_profile_ids: None,
            loft_closed: None,
            tolerance_mm: Some(0.001),
        };
        let conversion = convert_sketch_profile_to_feature_node(&model, &spec)
            .expect("open profile conversion should succeed with warning");
        assert!(
            conversion
                .warnings
                .iter()
                .any(|warning| warning.code == CadWarningCode::NonManifoldBody)
        );

        let command = history_command_for_sketch_feature(&spec);
        match command {
            CadHistoryCommand::ApplySketchFeature {
                operation_key,
                profile_id,
                feature_id,
            } => {
                assert_eq!(operation_key, SKETCH_EXTRUDE_OPERATION_KEY);
                assert_eq!(profile_id, "profile.open");
                assert_eq!(feature_id, "feature.open.extrude");
            }
            _ => panic!("history command should map to ApplySketchFeature"),
        }
    }

    #[test]
    fn rectangle_circle_and_closed_spline_profiles_are_closed_for_extrude() {
        let mut model = CadSketchModel::default();
        model
            .insert_plane(CadSketchPlane {
                id: "plane.front".to_string(),
                name: "Front".to_string(),
                origin_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
            })
            .expect("plane should insert");
        model
            .insert_entity(CadSketchEntity::Rectangle {
                id: "entity.rect".to_string(),
                plane_id: "plane.front".to_string(),
                min_mm: [0.0, 0.0],
                max_mm: [20.0, 10.0],
                anchor_ids: [
                    "anchor.rect.00".to_string(),
                    "anchor.rect.10".to_string(),
                    "anchor.rect.11".to_string(),
                    "anchor.rect.01".to_string(),
                ],
                construction: false,
            })
            .expect("rectangle should insert");
        model
            .insert_entity(CadSketchEntity::Circle {
                id: "entity.circle".to_string(),
                plane_id: "plane.front".to_string(),
                center_mm: [40.0, 10.0],
                radius_mm: 5.0,
                anchor_ids: [
                    "anchor.circle.center".to_string(),
                    "anchor.circle.radius".to_string(),
                ],
                construction: false,
            })
            .expect("circle should insert");
        model
            .insert_entity(CadSketchEntity::Spline {
                id: "entity.spline.closed".to_string(),
                plane_id: "plane.front".to_string(),
                control_points_mm: vec![[60.0, 0.0], [65.0, 8.0], [72.0, 5.0], [68.0, -2.0]],
                anchor_ids: vec![
                    "anchor.spline.0".to_string(),
                    "anchor.spline.1".to_string(),
                    "anchor.spline.2".to_string(),
                    "anchor.spline.3".to_string(),
                ],
                closed: true,
                construction: false,
            })
            .expect("closed spline should insert");

        for (feature_id, profile_id, entity_id) in [
            ("feature.rect.extrude", "profile.rect", "entity.rect"),
            ("feature.circle.extrude", "profile.circle", "entity.circle"),
            (
                "feature.spline.extrude",
                "profile.spline.closed",
                "entity.spline.closed",
            ),
        ] {
            let spec = SketchProfileFeatureSpec {
                feature_id: feature_id.to_string(),
                profile_id: profile_id.to_string(),
                plane_id: "plane.front".to_string(),
                profile_entity_ids: vec![entity_id.to_string()],
                kind: SketchProfileFeatureKind::Extrude,
                source_feature_id: None,
                depth_mm: Some(5.0),
                revolve_angle_deg: None,
                axis_anchor_ids: None,
                sweep_path_entity_ids: None,
                sweep_twist_deg: None,
                sweep_scale_start: None,
                sweep_scale_end: None,
                loft_profile_ids: None,
                loft_closed: None,
                tolerance_mm: Some(0.001),
            };
            let conversion = convert_sketch_profile_to_feature_node(&model, &spec)
                .expect("profile conversion should succeed");
            assert_eq!(
                conversion
                    .node
                    .params
                    .get("profile_closed_loop")
                    .map(String::as_str),
                Some("true"),
                "{entity_id} should be treated as closed profile"
            );
            assert!(
                conversion
                    .warnings
                    .iter()
                    .all(|warning| warning.code != CadWarningCode::NonManifoldBody),
                "{entity_id} should not emit open-profile warning"
            );
        }
    }
}
