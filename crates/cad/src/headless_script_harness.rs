use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::cli::{CAD_CLI_APP_NAME, run_cli_tokens};
use crate::export::export_step_from_mesh;
use crate::hash::stable_hex_digest;
use crate::mcp_tools::{
    CadMcpCreateFormat, CadMcpCreateInput, CadMcpDocument, CadMcpExportInput, CadMcpInspectInput,
    CadMcpOperation, CadMcpPartInput, CadMcpPrimitive, CadMcpPrimitiveType, CadMcpToolResponse,
    CadMcpVec3, cad_document_from_text, create_cad_document, export_cad, inspect_cad,
};
use crate::mesh::{
    CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
};
use crate::stl::export_stl_from_mesh;
use crate::{CadError, CadResult};

pub const HEADLESS_SCRIPT_HARNESS_WORKSPACE_ROOT: &str = "target/parity/headless-script-harness";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHeadlessScript {
    pub script_id: String,
    #[serde(default = "default_fail_fast")]
    pub fail_fast: bool,
    pub steps: Vec<CadHeadlessScriptStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadHeadlessScriptStep {
    CliCommand {
        name: String,
        args: Vec<String>,
        #[serde(default = "default_expected_exit_code")]
        expected_exit_code: i32,
    },
    McpCreateSample {
        name: String,
        document_id: String,
        #[serde(default)]
        format: Option<CadMcpCreateFormat>,
    },
    McpInspectDocument {
        name: String,
        document_id: String,
    },
    McpExportDocument {
        name: String,
        document_id: String,
        filename: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CadHeadlessStepStatus {
    Ok,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHeadlessStepResult {
    pub name: String,
    pub kind: String,
    pub status: CadHeadlessStepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHeadlessScriptReport {
    pub script_id: String,
    pub fail_fast: bool,
    pub halted: bool,
    pub executed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub steps: Vec<CadHeadlessStepResult>,
    pub deterministic_signature: String,
}

#[derive(Default)]
struct CadHeadlessExecutionContext {
    documents: BTreeMap<String, CadMcpDocument>,
}

pub fn run_headless_script(script: &CadHeadlessScript) -> CadResult<CadHeadlessScriptReport> {
    let _guard = headless_script_mutex()
        .lock()
        .map_err(|_| CadError::ParseFailed {
            reason: "headless script harness lock poisoned".to_string(),
        })?;

    if script.steps.is_empty() {
        return Err(CadError::InvalidParameter {
            name: "steps".to_string(),
            reason: "headless script must include at least one step".to_string(),
        });
    }

    let workspace = workspace_for_script(&script.script_id);
    reset_workspace(&workspace)?;
    seed_workspace_inputs(&workspace)?;

    let mut context = CadHeadlessExecutionContext::default();
    let mut halted = false;
    let mut step_results = Vec::with_capacity(script.steps.len());

    for step in &script.steps {
        if halted && script.fail_fast {
            step_results.push(skipped_result(step, "skipped due to fail-fast halt"));
            continue;
        }

        let result = execute_step(step, &workspace, &mut context);
        if result.status == CadHeadlessStepStatus::Failed && script.fail_fast {
            halted = true;
        }
        step_results.push(result);
    }

    let executed_steps = step_results
        .iter()
        .filter(|step| step.status != CadHeadlessStepStatus::Skipped)
        .count();
    let failed_steps = step_results
        .iter()
        .filter(|step| step.status == CadHeadlessStepStatus::Failed)
        .count();
    let skipped_steps = step_results
        .iter()
        .filter(|step| step.status == CadHeadlessStepStatus::Skipped)
        .count();
    let deterministic_signature = stable_hex_digest(
        serde_json::to_string(&(
            &script.script_id,
            script.fail_fast,
            halted,
            executed_steps,
            failed_steps,
            skipped_steps,
            &step_results,
        ))
        .unwrap_or_default()
        .as_bytes(),
    );

    Ok(CadHeadlessScriptReport {
        script_id: script.script_id.clone(),
        fail_fast: script.fail_fast,
        halted,
        executed_steps,
        failed_steps,
        skipped_steps,
        steps: step_results,
        deterministic_signature,
    })
}

pub fn canonical_headless_cli_workflow_script() -> CadHeadlessScript {
    CadHeadlessScript {
        script_id: "headless-cli-workflow".to_string(),
        fail_fast: true,
        steps: vec![
            CadHeadlessScriptStep::CliCommand {
                name: "export_seed_step".to_string(),
                args: vec![
                    "export".to_string(),
                    "{workspace}/seed_mesh.json".to_string(),
                    "{workspace}/workflow_export.step".to_string(),
                ],
                expected_exit_code: 0,
            },
            CadHeadlessScriptStep::CliCommand {
                name: "import_step_document".to_string(),
                args: vec![
                    "import".to_string(),
                    "{workspace}/workflow_export.step".to_string(),
                    "{workspace}/workflow_imported.vcad".to_string(),
                    "--name".to_string(),
                    "workflow-part".to_string(),
                ],
                expected_exit_code: 0,
            },
            CadHeadlessScriptStep::CliCommand {
                name: "info_imported_document".to_string(),
                args: vec![
                    "info".to_string(),
                    "{workspace}/workflow_imported.vcad".to_string(),
                ],
                expected_exit_code: 0,
            },
            CadHeadlessScriptStep::CliCommand {
                name: "export_seed_stl".to_string(),
                args: vec![
                    "export".to_string(),
                    "{workspace}/seed_mesh.json".to_string(),
                    "{workspace}/workflow_export.stl".to_string(),
                ],
                expected_exit_code: 0,
            },
        ],
    }
}

pub fn canonical_headless_mcp_workflow_script() -> CadHeadlessScript {
    CadHeadlessScript {
        script_id: "headless-mcp-workflow".to_string(),
        fail_fast: true,
        steps: vec![
            CadHeadlessScriptStep::McpCreateSample {
                name: "create_document".to_string(),
                document_id: "workflow_doc".to_string(),
                format: Some(CadMcpCreateFormat::Compact),
            },
            CadHeadlessScriptStep::McpInspectDocument {
                name: "inspect_document".to_string(),
                document_id: "workflow_doc".to_string(),
            },
            CadHeadlessScriptStep::McpExportDocument {
                name: "export_document_glb".to_string(),
                document_id: "workflow_doc".to_string(),
                filename: "{workspace}/workflow_mcp.glb".to_string(),
            },
        ],
    }
}

pub fn fail_fast_headless_workflow_script() -> CadHeadlessScript {
    CadHeadlessScript {
        script_id: "headless-fail-fast-workflow".to_string(),
        fail_fast: true,
        steps: vec![
            CadHeadlessScriptStep::CliCommand {
                name: "missing_info".to_string(),
                args: vec!["info".to_string(), "{workspace}/missing.vcad".to_string()],
                expected_exit_code: 0,
            },
            CadHeadlessScriptStep::CliCommand {
                name: "should_be_skipped".to_string(),
                args: vec![
                    "export".to_string(),
                    "{workspace}/seed_mesh.json".to_string(),
                    "{workspace}/skip.stl".to_string(),
                ],
                expected_exit_code: 0,
            },
        ],
    }
}

fn default_fail_fast() -> bool {
    true
}

fn default_expected_exit_code() -> i32 {
    0
}

fn headless_script_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn execute_step(
    step: &CadHeadlessScriptStep,
    workspace: &Path,
    context: &mut CadHeadlessExecutionContext,
) -> CadHeadlessStepResult {
    match step {
        CadHeadlessScriptStep::CliCommand {
            name,
            args,
            expected_exit_code,
        } => execute_cli_step(name, args, *expected_exit_code, workspace),
        CadHeadlessScriptStep::McpCreateSample {
            name,
            document_id,
            format,
        } => execute_mcp_create_step(name, document_id, *format, context),
        CadHeadlessScriptStep::McpInspectDocument { name, document_id } => {
            execute_mcp_inspect_step(name, document_id, context)
        }
        CadHeadlessScriptStep::McpExportDocument {
            name,
            document_id,
            filename,
        } => execute_mcp_export_step(name, document_id, filename, workspace, context),
    }
}

fn execute_cli_step(
    name: &str,
    args: &[String],
    expected_exit_code: i32,
    workspace: &Path,
) -> CadHeadlessStepResult {
    let resolved_args = args
        .iter()
        .map(|arg| resolve_template(arg, workspace))
        .collect::<Vec<_>>();

    let mut tokens = Vec::with_capacity(resolved_args.len() + 1);
    tokens.push(CAD_CLI_APP_NAME.to_string());
    tokens.extend(resolved_args.clone());

    let borrowed = tokens.iter().map(String::as_str).collect::<Vec<_>>();
    let outcome = run_cli_tokens(&borrowed);
    let status = if outcome.exit_code == expected_exit_code {
        CadHeadlessStepStatus::Ok
    } else {
        CadHeadlessStepStatus::Failed
    };

    let output_hash = resolved_args
        .last()
        .and_then(|path| read_file_hash_if_exists(Path::new(path)));
    let message = if status == CadHeadlessStepStatus::Ok {
        format!("cli step matched expected exit code {}", expected_exit_code)
    } else {
        format!(
            "cli exit code mismatch: expected {}, got {}",
            expected_exit_code, outcome.exit_code
        )
    };

    CadHeadlessStepResult {
        name: name.to_string(),
        kind: "cli_command".to_string(),
        status,
        expected_exit_code: Some(expected_exit_code),
        actual_exit_code: Some(outcome.exit_code),
        stdout_hash: Some(stable_hex_digest(outcome.stdout.as_bytes())),
        stderr_hash: Some(stable_hex_digest(outcome.stderr.as_bytes())),
        response_hash: None,
        output_hash,
        message,
    }
}

fn execute_mcp_create_step(
    name: &str,
    document_id: &str,
    format: Option<CadMcpCreateFormat>,
    context: &mut CadHeadlessExecutionContext,
) -> CadHeadlessStepResult {
    let mut input = sample_mcp_create_input();
    input.format = format;
    match create_cad_document(input)
        .and_then(|response| response_text(&response))
        .and_then(|text| cad_document_from_text(&text).map(|document| (document, text)))
    {
        Ok((document, response_text)) => {
            context.documents.insert(document_id.to_string(), document);
            CadHeadlessStepResult {
                name: name.to_string(),
                kind: "mcp_create_sample".to_string(),
                status: CadHeadlessStepStatus::Ok,
                expected_exit_code: None,
                actual_exit_code: None,
                stdout_hash: None,
                stderr_hash: None,
                response_hash: Some(stable_hex_digest(response_text.as_bytes())),
                output_hash: None,
                message: format!("mcp create stored document '{document_id}'"),
            }
        }
        Err(error) => CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_create_sample".to_string(),
            status: CadHeadlessStepStatus::Failed,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: None,
            output_hash: None,
            message: format!("mcp create failed: {error}"),
        },
    }
}

fn execute_mcp_inspect_step(
    name: &str,
    document_id: &str,
    context: &mut CadHeadlessExecutionContext,
) -> CadHeadlessStepResult {
    let Some(document) = context.documents.get(document_id) else {
        return CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_inspect_document".to_string(),
            status: CadHeadlessStepStatus::Failed,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: None,
            output_hash: None,
            message: format!("missing mcp document '{document_id}'"),
        };
    };

    match inspect_cad(CadMcpInspectInput {
        ir: document.clone(),
    })
    .and_then(|response| response_text(&response))
    {
        Ok(text) => CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_inspect_document".to_string(),
            status: CadHeadlessStepStatus::Ok,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: Some(stable_hex_digest(text.as_bytes())),
            output_hash: None,
            message: format!("mcp inspect succeeded for '{document_id}'"),
        },
        Err(error) => CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_inspect_document".to_string(),
            status: CadHeadlessStepStatus::Failed,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: None,
            output_hash: None,
            message: format!("mcp inspect failed: {error}"),
        },
    }
}

fn execute_mcp_export_step(
    name: &str,
    document_id: &str,
    filename: &str,
    workspace: &Path,
    context: &mut CadHeadlessExecutionContext,
) -> CadHeadlessStepResult {
    let Some(document) = context.documents.get(document_id) else {
        return CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_export_document".to_string(),
            status: CadHeadlessStepStatus::Failed,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: None,
            output_hash: None,
            message: format!("missing mcp document '{document_id}'"),
        };
    };

    let resolved_filename = resolve_template(filename, workspace);
    match export_cad(CadMcpExportInput {
        ir: document.clone(),
        filename: resolved_filename.clone(),
    })
    .and_then(|response| response_text(&response))
    {
        Ok(text) => CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_export_document".to_string(),
            status: CadHeadlessStepStatus::Ok,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: Some(stable_hex_digest(text.as_bytes())),
            output_hash: read_file_hash_if_exists(Path::new(&resolved_filename)),
            message: format!("mcp export wrote '{}'", resolved_filename),
        },
        Err(error) => CadHeadlessStepResult {
            name: name.to_string(),
            kind: "mcp_export_document".to_string(),
            status: CadHeadlessStepStatus::Failed,
            expected_exit_code: None,
            actual_exit_code: None,
            stdout_hash: None,
            stderr_hash: None,
            response_hash: None,
            output_hash: None,
            message: format!("mcp export failed: {error}"),
        },
    }
}

fn skipped_result(step: &CadHeadlessScriptStep, reason: &str) -> CadHeadlessStepResult {
    let (name, kind) = step_name_and_kind(step);
    CadHeadlessStepResult {
        name,
        kind,
        status: CadHeadlessStepStatus::Skipped,
        expected_exit_code: None,
        actual_exit_code: None,
        stdout_hash: None,
        stderr_hash: None,
        response_hash: None,
        output_hash: None,
        message: reason.to_string(),
    }
}

fn step_name_and_kind(step: &CadHeadlessScriptStep) -> (String, String) {
    match step {
        CadHeadlessScriptStep::CliCommand { name, .. } => (name.clone(), "cli_command".to_string()),
        CadHeadlessScriptStep::McpCreateSample { name, .. } => {
            (name.clone(), "mcp_create_sample".to_string())
        }
        CadHeadlessScriptStep::McpInspectDocument { name, .. } => {
            (name.clone(), "mcp_inspect_document".to_string())
        }
        CadHeadlessScriptStep::McpExportDocument { name, .. } => {
            (name.clone(), "mcp_export_document".to_string())
        }
    }
}

fn response_text(response: &CadMcpToolResponse) -> CadResult<String> {
    response
        .content
        .first()
        .map(|content| content.text.clone())
        .ok_or_else(|| CadError::ParseFailed {
            reason: "mcp response content is empty".to_string(),
        })
}

fn reset_workspace(path: &Path) -> CadResult<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| CadError::ParseFailed {
            reason: format!(
                "failed clearing headless workspace {}: {error}",
                path.display()
            ),
        })?;
    }
    fs::create_dir_all(path).map_err(|error| CadError::ParseFailed {
        reason: format!(
            "failed creating headless workspace {}: {error}",
            path.display()
        ),
    })?;
    Ok(())
}

fn seed_workspace_inputs(workspace: &Path) -> CadResult<()> {
    let mesh = sample_tetra_mesh();
    write_pretty_json(&workspace.join("seed_mesh.json"), &mesh)?;

    let step = export_step_from_mesh(
        "headless-script-harness.seed",
        mesh.document_revision,
        &mesh.variant_id,
        &mesh,
    )?;
    fs::write(workspace.join("seed_import.step"), &step.bytes).map_err(|error| {
        CadError::ParseFailed {
            reason: format!(
                "failed writing step seed {}: {error}",
                workspace.join("seed_import.step").display()
            ),
        }
    })?;

    let stl = export_stl_from_mesh(
        "headless-script-harness.seed",
        mesh.document_revision,
        &mesh.variant_id,
        &mesh,
    )?;
    fs::write(workspace.join("seed_import.stl"), &stl.bytes).map_err(|error| {
        CadError::ParseFailed {
            reason: format!(
                "failed writing stl seed {}: {error}",
                workspace.join("seed_import.stl").display()
            ),
        }
    })?;

    Ok(())
}

fn write_pretty_json(path: &Path, value: &impl Serialize) -> CadResult<()> {
    let payload = serde_json::to_string_pretty(value).map_err(|error| CadError::ParseFailed {
        reason: format!("failed serializing json for {}: {error}", path.display()),
    })?;
    fs::write(path, payload).map_err(|error| CadError::ParseFailed {
        reason: format!("failed writing {}: {error}", path.display()),
    })
}

fn resolve_template(value: &str, workspace: &Path) -> String {
    value.replace("{workspace}", &workspace.to_string_lossy())
}

fn workspace_for_script(script_id: &str) -> PathBuf {
    repo_root()
        .join(HEADLESS_SCRIPT_HARNESS_WORKSPACE_ROOT)
        .join(sanitize_script_id(script_id))
}

fn sanitize_script_id(script_id: &str) -> String {
    let mut out = String::with_capacity(script_id.len());
    for ch in script_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "script".to_string()
    } else {
        out
    }
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.to_path_buf())
        .unwrap_or(manifest_dir)
}

fn read_file_hash_if_exists(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    fs::read(path).ok().map(|bytes| stable_hex_digest(&bytes))
}

fn sample_mcp_create_input() -> CadMcpCreateInput {
    CadMcpCreateInput {
        parts: vec![CadMcpPartInput {
            name: "workflow_cube".to_string(),
            primitive: CadMcpPrimitive {
                primitive_type: CadMcpPrimitiveType::Cube,
                size: Some(CadMcpVec3 {
                    x: 12.0,
                    y: 10.0,
                    z: 8.0,
                }),
                radius: None,
                height: None,
                segments: None,
                radius_bottom: None,
                radius_top: None,
            },
            operations: vec![CadMcpOperation::Translate {
                offset: CadMcpVec3 {
                    x: 4.0,
                    y: 0.0,
                    z: 0.0,
                },
            }],
            material: Some("aluminum".to_string()),
        }],
        format: None,
    }
}

fn sample_tetra_mesh() -> CadMeshPayload {
    CadMeshPayload {
        mesh_id: "mesh.headless.script".to_string(),
        document_revision: 7,
        variant_id: "variant.headless.script".to_string(),
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
                position_mm: [20.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 20.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                material_slot: 0,
                flags: 0,
            },
            CadMeshVertex {
                position_mm: [0.0, 0.0, 20.0],
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
            max_mm: [20.0, 20.0, 20.0],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CadHeadlessStepStatus, canonical_headless_cli_workflow_script,
        canonical_headless_mcp_workflow_script, fail_fast_headless_workflow_script,
        run_headless_script,
    };

    #[test]
    fn canonical_cli_workflow_script_runs_successfully() {
        let report =
            run_headless_script(&canonical_headless_cli_workflow_script()).expect("cli workflow");
        assert_eq!(report.script_id, "headless-cli-workflow");
        assert!(!report.halted);
        assert_eq!(report.failed_steps, 0);
        assert_eq!(report.skipped_steps, 0);
        assert_eq!(report.executed_steps, 4);
        assert!(
            report
                .steps
                .iter()
                .all(|step| step.status == CadHeadlessStepStatus::Ok)
        );
    }

    #[test]
    fn canonical_mcp_workflow_script_runs_successfully() {
        let report =
            run_headless_script(&canonical_headless_mcp_workflow_script()).expect("mcp workflow");
        assert_eq!(report.script_id, "headless-mcp-workflow");
        assert!(!report.halted);
        assert_eq!(report.failed_steps, 0);
        assert_eq!(report.executed_steps, 3);
        assert!(
            report
                .steps
                .iter()
                .all(|step| step.status == CadHeadlessStepStatus::Ok)
        );
    }

    #[test]
    fn fail_fast_script_halts_and_skips_remaining_steps() {
        let report =
            run_headless_script(&fail_fast_headless_workflow_script()).expect("fail-fast workflow");
        assert!(report.halted);
        assert_eq!(report.failed_steps, 1);
        assert_eq!(report.skipped_steps, 1);
        assert_eq!(report.executed_steps, 1);
        assert_eq!(report.steps[0].status, CadHeadlessStepStatus::Failed);
        assert_eq!(report.steps[0].actual_exit_code, Some(1));
        assert_eq!(report.steps[1].status, CadHeadlessStepStatus::Skipped);
    }

    #[test]
    fn canonical_script_reports_are_deterministic() {
        let first = run_headless_script(&canonical_headless_cli_workflow_script())
            .expect("first cli report");
        let second = run_headless_script(&canonical_headless_cli_workflow_script())
            .expect("second cli report");
        assert_eq!(first, second);
    }
}
