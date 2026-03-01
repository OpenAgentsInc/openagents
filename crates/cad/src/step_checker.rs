use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{CadError, CadResult};

/// Stable severity for machine-readable STEP checker diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadStepCheckerSeverity {
    Info,
    Warning,
    Error,
}

/// Stable machine-readable STEP checker diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepCheckerDiagnostic {
    pub code: String,
    pub severity: CadStepCheckerSeverity,
    pub message: String,
    pub remediation_hint: String,
    pub count: u64,
}

/// Deterministic STEP checker report contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadStepCheckerReport {
    pub checker_version: u32,
    pub backend: String,
    pub source: String,
    pub passed: bool,
    pub solid_count: usize,
    pub shell_count: usize,
    pub face_count: usize,
    pub poly_loop_count: usize,
    pub non_manifold_edge_count: usize,
    pub bbox_min_mm: Option<[f64; 3]>,
    pub bbox_max_mm: Option<[f64; 3]>,
    pub volume_mm3: Option<f64>,
    pub diagnostics: Vec<CadStepCheckerDiagnostic>,
}

impl CadStepCheckerReport {
    pub fn to_pretty_json(&self) -> CadResult<String> {
        serde_json::to_string_pretty(self).map_err(|error| CadError::Serialization {
            reason: format!("failed to serialize step checker report: {error}"),
        })
    }
}

/// STEP checker backend modes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CadStepCheckerBackend {
    Structural,
    OpenCascadeCommand { program: String, args: Vec<String> },
}

/// Run structural checker directly on STEP text.
pub fn check_step_text_structural(step_text: &str, source: &str) -> CadStepCheckerReport {
    let entities = parse_step_entities(step_text);
    let point_by_ref = entities
        .iter()
        .filter_map(|(id, payload)| parse_cartesian_point(payload).map(|point| (*id, point)))
        .collect::<BTreeMap<_, _>>();
    let point_key_by_ref = point_by_ref
        .iter()
        .map(|(id, point)| (*id, vertex_key_from_point(*point)))
        .collect::<BTreeMap<_, _>>();
    let (bbox_min_mm, bbox_max_mm) = compute_bounds(point_by_ref.values().copied());

    let mut solid_count = 0usize;
    let mut shell_count = 0usize;
    let mut face_count = 0usize;
    let mut poly_loop_count = 0usize;
    let mut diagnostics = Vec::<CadStepCheckerDiagnostic>::new();
    let mut edge_use_count = BTreeMap::<(String, String), u64>::new();
    let mut signed_volume_mm3 = 0.0f64;

    for (_id, payload) in entities {
        if payload.starts_with("FACETED_BREP(") || payload.starts_with("MANIFOLD_SOLID_BREP(") {
            solid_count = solid_count.saturating_add(1);
        }
        if payload.starts_with("CLOSED_SHELL(") {
            shell_count = shell_count.saturating_add(1);
        }
        if payload.starts_with("FACE(") {
            face_count = face_count.saturating_add(1);
        }
        if payload.starts_with("POLY_LOOP((") {
            poly_loop_count = poly_loop_count.saturating_add(1);
            if let Some(loop_refs) = parse_poly_loop_refs(&payload) {
                if loop_refs.len() >= 3 {
                    if let Some(p0) = point_by_ref.get(&loop_refs[0]).copied() {
                        for idx in 1..(loop_refs.len() - 1) {
                            let p1 = point_by_ref.get(&loop_refs[idx]).copied();
                            let p2 = point_by_ref.get(&loop_refs[idx + 1]).copied();
                            if let (Some(p1), Some(p2)) = (p1, p2) {
                                signed_volume_mm3 += tetra_signed_volume(p0, p1, p2);
                            }
                        }
                    }
                    for idx in 0..loop_refs.len() {
                        let a = loop_refs[idx];
                        let b = loop_refs[(idx + 1) % loop_refs.len()];
                        if a == b {
                            continue;
                        }
                        let a_key = point_key_by_ref
                            .get(&a)
                            .cloned()
                            .unwrap_or_else(|| format!("#{a}"));
                        let b_key = point_key_by_ref
                            .get(&b)
                            .cloned()
                            .unwrap_or_else(|| format!("#{b}"));
                        let edge = if a_key <= b_key {
                            (a_key, b_key)
                        } else {
                            (b_key, a_key)
                        };
                        *edge_use_count.entry(edge).or_insert(0) += 1;
                    }
                } else {
                    diagnostics.push(CadStepCheckerDiagnostic {
                        code: "STEP_POLY_LOOP_TOO_SHORT".to_string(),
                        severity: CadStepCheckerSeverity::Error,
                        message: "poly loop contains fewer than 3 points".to_string(),
                        remediation_hint:
                            "ensure each face loop references at least three distinct points"
                                .to_string(),
                        count: 1,
                    });
                }
            } else {
                diagnostics.push(CadStepCheckerDiagnostic {
                    code: "STEP_POLY_LOOP_PARSE_FAILED".to_string(),
                    severity: CadStepCheckerSeverity::Error,
                    message: "failed to parse POLY_LOOP point references".to_string(),
                    remediation_hint:
                        "verify POLY_LOOP syntax uses (#point,#point,#point) reference lists"
                            .to_string(),
                    count: 1,
                });
            }
        }
    }

    let non_manifold_edge_count = edge_use_count.values().filter(|count| **count != 2).count();
    let volume_mm3 = if poly_loop_count > 0 {
        Some(signed_volume_mm3.abs())
    } else {
        None
    };

    if solid_count == 0 {
        diagnostics.push(CadStepCheckerDiagnostic {
            code: "STEP_INVALID_SOLID".to_string(),
            severity: CadStepCheckerSeverity::Error,
            message: "no solid entities found (expected FACETED_BREP or MANIFOLD_SOLID_BREP)"
                .to_string(),
            remediation_hint: "export at least one solid body before running checker".to_string(),
            count: 1,
        });
    }

    if shell_count == 0 || shell_count < solid_count {
        diagnostics.push(CadStepCheckerDiagnostic {
            code: "STEP_MISSING_SHELL".to_string(),
            severity: CadStepCheckerSeverity::Error,
            message: format!(
                "shell count is insufficient for solids: solids={solid_count} shells={shell_count}"
            ),
            remediation_hint: "ensure each solid references a CLOSED_SHELL entity".to_string(),
            count: (solid_count.saturating_sub(shell_count)).max(1) as u64,
        });
    }

    if non_manifold_edge_count > 0 {
        diagnostics.push(CadStepCheckerDiagnostic {
            code: "STEP_NON_MANIFOLD_EDGE".to_string(),
            severity: CadStepCheckerSeverity::Error,
            message: format!(
                "detected {} non-manifold/open edges (edge-use count != 2)",
                non_manifold_edge_count
            ),
            remediation_hint:
                "repair face loops so every shell edge is shared by exactly two faces".to_string(),
            count: non_manifold_edge_count as u64,
        });
    }

    let passed = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != CadStepCheckerSeverity::Error);

    CadStepCheckerReport {
        checker_version: 1,
        backend: "structural".to_string(),
        source: source.to_string(),
        passed,
        solid_count,
        shell_count,
        face_count,
        poly_loop_count,
        non_manifold_edge_count,
        bbox_min_mm,
        bbox_max_mm,
        volume_mm3,
        diagnostics,
    }
}

/// Run structural checker against a STEP file path.
pub fn check_step_file_structural(path: &Path) -> CadResult<CadStepCheckerReport> {
    let payload = std::fs::read_to_string(path).map_err(|error| CadError::ExportFailed {
        format: "step".to_string(),
        reason: format!("failed reading step file {}: {error}", path.display()),
    })?;
    Ok(check_step_text_structural(
        &payload,
        &path.display().to_string(),
    ))
}

/// Run checker with explicit backend selection.
pub fn check_step_file_with_backend(
    path: &Path,
    backend: CadStepCheckerBackend,
) -> CadResult<CadStepCheckerReport> {
    match backend {
        CadStepCheckerBackend::Structural => check_step_file_structural(path),
        CadStepCheckerBackend::OpenCascadeCommand { program, args } => {
            let output = Command::new(&program)
                .args(&args)
                .arg("--input")
                .arg(path)
                .output()
                .map_err(|error| CadError::ExportFailed {
                    format: "step".to_string(),
                    reason: format!(
                        "failed to launch OpenCascade checker command {}: {error}",
                        program
                    ),
                })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CadError::ExportFailed {
                    format: "step".to_string(),
                    reason: format!(
                        "OpenCascade checker command failed (status={}): {}",
                        output
                            .status
                            .code()
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| "signal".to_string()),
                        stderr.trim()
                    ),
                });
            }
            let stdout =
                String::from_utf8(output.stdout).map_err(|error| CadError::ExportFailed {
                    format: "step".to_string(),
                    reason: format!("OpenCascade checker output was not valid utf-8: {error}"),
                })?;
            serde_json::from_str::<CadStepCheckerReport>(&stdout).map_err(|error| {
                CadError::ExportFailed {
                    format: "step".to_string(),
                    reason: format!("failed to parse OpenCascade checker JSON output: {error}"),
                }
            })
        }
    }
}

fn parse_step_entities(step_text: &str) -> Vec<(u64, String)> {
    let mut entities = Vec::<(u64, String)>::new();
    for line in step_text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        let Some(eq_index) = trimmed.find('=') else {
            continue;
        };
        let Some(semi_index) = trimmed.rfind(';') else {
            continue;
        };
        if eq_index <= 1 || semi_index <= eq_index {
            continue;
        }
        let id_text = &trimmed[1..eq_index];
        let Ok(id) = id_text.parse::<u64>() else {
            continue;
        };
        let payload = trimmed[eq_index + 1..semi_index].trim().to_string();
        entities.push((id, payload));
    }
    entities
}

fn parse_poly_loop_refs(payload: &str) -> Option<Vec<u64>> {
    let prefix = "POLY_LOOP((";
    let suffix = "))";
    if !payload.starts_with(prefix) || !payload.ends_with(suffix) {
        return None;
    }
    let inner = &payload[prefix.len()..payload.len().saturating_sub(suffix.len())];
    let mut refs = Vec::<u64>::new();
    for token in inner.split(',') {
        let token = token.trim();
        let id = token.strip_prefix('#')?.parse::<u64>().ok()?;
        refs.push(id);
    }
    Some(refs)
}

fn parse_cartesian_point(payload: &str) -> Option<[f64; 3]> {
    if !payload.starts_with("CARTESIAN_POINT(") {
        return None;
    }
    let coords_start = payload.find(",(")? + 2;
    let coords_end = payload.rfind("))")?;
    if coords_end <= coords_start {
        return None;
    }
    let mut values = payload[coords_start..coords_end]
        .split(',')
        .map(|token| token.trim().parse::<f64>().ok());
    let x = values.next()??;
    let y = values.next()??;
    let z = values.next()??;
    if values.next().is_some() {
        return None;
    }
    if !x.is_finite() || !y.is_finite() || !z.is_finite() {
        return None;
    }
    Some([x, y, z])
}

fn vertex_key_from_point(point: [f64; 3]) -> String {
    format!(
        "{:.6}:{:.6}:{:.6}",
        normalize_signed_zero(point[0]),
        normalize_signed_zero(point[1]),
        normalize_signed_zero(point[2])
    )
}

fn normalize_signed_zero(value: f64) -> f64 {
    if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    }
}

fn compute_bounds(points: impl Iterator<Item = [f64; 3]>) -> (Option<[f64; 3]>, Option<[f64; 3]>) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut found = false;
    for point in points {
        found = true;
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if found {
        (Some(min), Some(max))
    } else {
        (None, None)
    }
}

fn tetra_signed_volume(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let cross = [
        b[1] * c[2] - b[2] * c[1],
        b[2] * c[0] - b[0] * c[2],
        b[0] * c[1] - b[1] * c[0],
    ];
    (a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.0
}

#[cfg(test)]
mod tests {
    use super::{CadStepCheckerBackend, check_step_file_with_backend, check_step_text_structural};
    use crate::export::export_step_from_mesh;
    use crate::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };

    fn sample_tetra_mesh(variant_id: &str) -> CadMeshPayload {
        CadMeshPayload {
            mesh_id: format!("mesh.{}.abc123", variant_id.replace('.', "-")),
            document_revision: 4,
            variant_id: variant_id.to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [40.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 40.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    material_slot: 0,
                    flags: 0,
                },
                CadMeshVertex {
                    position_mm: [0.0, 0.0, 40.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.5, 0.5],
                    material_slot: 0,
                    flags: 0,
                },
            ],
            triangle_indices: vec![0, 1, 2, 0, 1, 3, 1, 2, 3, 0, 2, 3],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [40.0, 40.0, 40.0],
            },
        }
    }

    #[test]
    fn structural_checker_passes_exported_baseline_and_variant_step() {
        for variant_id in ["variant.baseline", "variant.lightweight"] {
            let mesh = sample_tetra_mesh(variant_id);
            let artifact = export_step_from_mesh("doc.step-check", 4, variant_id, &mesh)
                .expect("step export should succeed");
            let report = check_step_text_structural(
                artifact.text().expect("step payload should decode"),
                variant_id,
            );
            assert!(report.passed, "{variant_id} should pass structural checker");
            assert_eq!(report.solid_count, 1);
            assert_eq!(report.shell_count, 1);
            assert_eq!(report.non_manifold_edge_count, 0);
        }
    }

    #[test]
    fn structural_checker_flags_missing_shell() {
        let payload = r#"
#1=FACETED_BREP('solid',#2);
"#;
        let report = check_step_text_structural(payload, "missing-shell");
        assert!(!report.passed);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "STEP_MISSING_SHELL")
        );
    }

    #[test]
    fn structural_checker_flags_non_manifold_edges() {
        let payload = r#"
#1=CARTESIAN_POINT('',(0.0,0.0,0.0));
#2=CARTESIAN_POINT('',(1.0,0.0,0.0));
#3=CARTESIAN_POINT('',(0.0,1.0,0.0));
#4=POLY_LOOP((#1,#2,#3));
#5=FACE_OUTER_BOUND('',#4,.T.);
#6=FACE((#5));
#7=CLOSED_SHELL('',(#6));
#8=FACETED_BREP('solid',#7);
"#;
        let report = check_step_text_structural(payload, "non-manifold");
        assert!(!report.passed);
        assert_eq!(report.non_manifold_edge_count, 3);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "STEP_NON_MANIFOLD_EDGE")
        );
    }

    #[test]
    fn checker_backend_rejects_unavailable_command() {
        let path = std::env::temp_dir().join("openagents-step-checker-unavailable.step");
        std::fs::write(&path, "ISO-10303-21;").expect("temp step fixture should write");
        let result = check_step_file_with_backend(
            &path,
            CadStepCheckerBackend::OpenCascadeCommand {
                program: "definitely-missing-opencascade-checker".to_string(),
                args: Vec::new(),
            },
        );
        assert!(result.is_err(), "missing command should fail explicitly");
    }
}
