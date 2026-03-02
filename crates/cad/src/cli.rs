use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::compact_ir::{from_compact as compact_ir_from_text, looks_like_compact_ir};
use crate::document::CadDocument;
use crate::export::export_step_from_mesh;
use crate::glb::export_glb_from_mesh;
use crate::mesh::CadMeshPayload;
use crate::step_checker::{check_step_text_structural, collect_step_entity_type_counts};
use crate::step_import::import_step_text_to_document;
use crate::stl::{export_stl_from_mesh, import_stl_to_mesh};

pub const CAD_CLI_SCAFFOLD_ISSUE_ID: &str = "VCAD-PARITY-083";
pub const CAD_CLI_IMPLEMENTATION_ISSUE_ID: &str = "VCAD-PARITY-084";
pub const CAD_CLI_APP_NAME: &str = "openagents-cad-cli";
pub const CAD_CLI_REFERENCE_COMMAND: &str = "vcad";
pub const CAD_CLI_STUB_EXIT_CODE: i32 = 3;
pub const CAD_CLI_SCAFFOLD_COMMANDS: [&str; 3] = ["export", "import", "info"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CadCliCommand {
    Export,
    Import,
    Info,
    Help,
}

impl CadCliCommand {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
            Self::Info => "info",
            Self::Help => "help",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "export" => Some(Self::Export),
            "import" => Some(Self::Import),
            "info" => Some(Self::Info),
            "help" | "-h" | "--help" => Some(Self::Help),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CadCliInvocation {
    command: CadCliCommand,
    passthrough: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadCliRunOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CadCliRunOutcome {
    fn success(stdout: String) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failure(exit_code: i32, stderr: String) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CadCliParseError {
    UnknownCommand(String),
}

#[derive(Default)]
struct ImportOptions {
    name: Option<String>,
}

pub fn run_cli_env_args(args: Vec<String>) -> CadCliRunOutcome {
    let tokens = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_cli_tokens(&tokens)
}

pub fn run_cli_tokens(tokens: &[&str]) -> CadCliRunOutcome {
    match parse_cli_tokens(tokens) {
        Ok(invocation) => execute_invocation(invocation),
        Err(CadCliParseError::UnknownCommand(command)) => CadCliRunOutcome::failure(
            2,
            format!(
                "unknown command: {command}\n\n{}",
                root_help_text(env!("CARGO_PKG_VERSION"))
            ),
        ),
    }
}

fn parse_cli_tokens(tokens: &[&str]) -> Result<CadCliInvocation, CadCliParseError> {
    let Some(command_token) = tokens.get(1).copied() else {
        return Ok(CadCliInvocation {
            command: CadCliCommand::Help,
            passthrough: Vec::new(),
        });
    };

    let Some(command) = CadCliCommand::from_token(command_token) else {
        return Err(CadCliParseError::UnknownCommand(command_token.to_string()));
    };

    Ok(CadCliInvocation {
        command,
        passthrough: tokens
            .iter()
            .skip(2)
            .map(|token| (*token).to_string())
            .collect(),
    })
}

fn execute_invocation(invocation: CadCliInvocation) -> CadCliRunOutcome {
    if invocation.command == CadCliCommand::Help {
        return CadCliRunOutcome::success(root_help_text(env!("CARGO_PKG_VERSION")));
    }

    if is_help_request(&invocation.passthrough) {
        return CadCliRunOutcome::success(subcommand_help_text(invocation.command));
    }

    if invocation.passthrough.is_empty() {
        return CadCliRunOutcome::failure(
            CAD_CLI_STUB_EXIT_CODE,
            scaffold_stub_error(invocation.command),
        );
    }

    match invocation.command {
        CadCliCommand::Export => handle_export(&invocation.passthrough),
        CadCliCommand::Import => handle_import(&invocation.passthrough),
        CadCliCommand::Info => handle_info(&invocation.passthrough),
        CadCliCommand::Help => CadCliRunOutcome::success(root_help_text(env!("CARGO_PKG_VERSION"))),
    }
}

fn handle_export(args: &[String]) -> CadCliRunOutcome {
    if args.len() != 2 {
        return CadCliRunOutcome::failure(
            2,
            format!("invalid export arguments\n\n{}", export_usage()),
        );
    }

    let input_path = Path::new(&args[0]);
    let output_path = Path::new(&args[1]);

    let mesh = match read_mesh_json(input_path) {
        Ok(mesh) => mesh,
        Err(reason) => return CadCliRunOutcome::failure(1, reason),
    };

    let extension = output_extension(output_path);
    let bytes = match extension.as_str() {
        "stl" => match export_stl_from_mesh(
            "cad-cli.export",
            mesh.document_revision,
            &mesh.variant_id,
            &mesh,
        ) {
            Ok(artifact) => artifact.bytes,
            Err(error) => {
                return CadCliRunOutcome::failure(
                    1,
                    format!(
                        "failed to export STL from {}: {error}",
                        input_path.display()
                    ),
                );
            }
        },
        "glb" => {
            match export_glb_from_mesh(
                "cad-cli.export",
                mesh.document_revision,
                &mesh.variant_id,
                &mesh,
            ) {
                Ok(artifact) => artifact.bytes,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!(
                            "failed to export GLB from {}: {error}",
                            input_path.display()
                        ),
                    );
                }
            }
        }
        "step" | "stp" => {
            match export_step_from_mesh(
                "cad-cli.export",
                mesh.document_revision,
                &mesh.variant_id,
                &mesh,
            ) {
                Ok(artifact) => artifact.bytes,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!(
                            "failed to export STEP from {}: {error}",
                            input_path.display()
                        ),
                    );
                }
            }
        }
        _ => {
            return CadCliRunOutcome::failure(
                2,
                format!(
                    "unknown export format for {}\n\n{}",
                    output_path.display(),
                    export_usage()
                ),
            );
        }
    };

    if let Err(error) = fs::write(output_path, &bytes) {
        return CadCliRunOutcome::failure(
            1,
            format!("failed writing {}: {error}", output_path.display()),
        );
    }

    CadCliRunOutcome::success(format!(
        "Exported {} to {}",
        extension.to_ascii_uppercase(),
        output_path.display()
    ))
}

fn handle_import(args: &[String]) -> CadCliRunOutcome {
    if args.len() < 2 {
        return CadCliRunOutcome::failure(
            2,
            format!("invalid import arguments\n\n{}", import_usage()),
        );
    }

    let input_path = Path::new(&args[0]);
    let output_path = Path::new(&args[1]);
    let options = match parse_import_options(&args[2..]) {
        Ok(options) => options,
        Err(reason) => {
            return CadCliRunOutcome::failure(2, format!("{reason}\n\n{}", import_usage()));
        }
    };

    let extension = output_extension(input_path);
    match extension.as_str() {
        "stl" => {
            let payload = match fs::read(input_path) {
                Ok(payload) => payload,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!("failed reading {}: {error}", input_path.display()),
                    );
                }
            };

            let variant_id = options.name.unwrap_or_else(|| {
                format!("variant.import.{}", stem_or_default(input_path, "stl"))
            });
            let imported = match import_stl_to_mesh(1, &variant_id, &payload) {
                Ok(result) => result,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!("failed to import STL {}: {error}", input_path.display()),
                    );
                }
            };

            if let Err(reason) = write_pretty_json(output_path, &imported.mesh) {
                return CadCliRunOutcome::failure(1, reason);
            }

            CadCliRunOutcome::success(format!("Imported STL to {}", output_path.display()))
        }
        "step" | "stp" => {
            let payload = match fs::read_to_string(input_path) {
                Ok(payload) => payload,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!("failed reading {}: {error}", input_path.display()),
                    );
                }
            };

            let document_id = options
                .name
                .unwrap_or_else(|| format!("doc.import.{}", stem_or_default(input_path, "step")));
            let imported = match import_step_text_to_document(&payload, &document_id) {
                Ok(result) => result,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!("failed to import STEP {}: {error}", input_path.display()),
                    );
                }
            };

            if let Err(reason) = write_pretty_json(output_path, &imported.document) {
                return CadCliRunOutcome::failure(1, reason);
            }

            CadCliRunOutcome::success(format!("Imported STEP to {}", output_path.display()))
        }
        "cad0" | "vcadc" => {
            let payload = match fs::read_to_string(input_path) {
                Ok(payload) => payload,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!("failed reading {}: {error}", input_path.display()),
                    );
                }
            };

            let document = match compact_ir_from_text(&payload) {
                Ok(document) => document,
                Err(error) => {
                    return CadCliRunOutcome::failure(
                        1,
                        format!(
                            "failed to import compact IR {}: {error}",
                            input_path.display()
                        ),
                    );
                }
            };

            if let Err(reason) = write_pretty_json(output_path, &document) {
                return CadCliRunOutcome::failure(1, reason);
            }

            CadCliRunOutcome::success(format!("Imported compact IR to {}", output_path.display()))
        }
        _ => CadCliRunOutcome::failure(
            2,
            format!(
                "unknown import format for {}\n\n{}",
                input_path.display(),
                import_usage()
            ),
        ),
    }
}

fn handle_info(args: &[String]) -> CadCliRunOutcome {
    if args.len() != 1 {
        return CadCliRunOutcome::failure(2, format!("invalid info arguments\n\n{}", info_usage()));
    }

    let input_path = Path::new(&args[0]);
    let payload = match fs::read_to_string(input_path) {
        Ok(payload) => payload,
        Err(error) => {
            return CadCliRunOutcome::failure(
                1,
                format!("failed reading {}: {error}", input_path.display()),
            );
        }
    };

    if let Ok(mesh) = serde_json::from_str::<CadMeshPayload>(&payload) {
        return CadCliRunOutcome::success(format_mesh_info(input_path, &mesh));
    }

    if let Ok(document) = CadDocument::from_json(&payload) {
        return CadCliRunOutcome::success(format_document_info(input_path, &document));
    }

    if looks_like_compact_ir(&payload) {
        if let Ok(document) = compact_ir_from_text(&payload) {
            return CadCliRunOutcome::success(format_compact_ir_info(input_path, &document));
        }
    }

    let extension = output_extension(input_path);
    if extension == "step" || extension == "stp" {
        let report = check_step_text_structural(&payload, "openagents-cad-cli");
        let entity_counts = collect_step_entity_type_counts(&payload);
        let total_entities = entity_counts.values().copied().sum::<usize>();
        return CadCliRunOutcome::success(format_step_info(
            input_path,
            report.solid_count,
            report.shell_count,
            report.face_count,
            total_entities,
            report.passed,
        ));
    }

    CadCliRunOutcome::failure(
        1,
        format!(
            "failed to parse {} as CadMeshPayload, CadDocument, or STEP text",
            input_path.display()
        ),
    )
}

fn parse_import_options(args: &[String]) -> Result<ImportOptions, String> {
    let mut options = ImportOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("missing value for --name".to_string());
                };
                options.name = Some(value.clone());
                index += 2;
            }
            unknown => {
                return Err(format!("unknown import option: {unknown}"));
            }
        }
    }

    Ok(options)
}

fn read_mesh_json(path: &Path) -> Result<CadMeshPayload, String> {
    let payload = fs::read_to_string(path)
        .map_err(|error| format!("failed reading {}: {error}", path.display()))?;
    serde_json::from_str::<CadMeshPayload>(&payload).map_err(|error| {
        format!(
            "failed parsing {} as CadMeshPayload JSON: {error}",
            path.display()
        )
    })
}

fn write_pretty_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut payload = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize json for {}: {error}", path.display()))?;
    payload.push('\n');
    fs::write(path, payload).map_err(|error| format!("failed writing {}: {error}", path.display()))
}

fn format_mesh_info(path: &Path, mesh: &CadMeshPayload) -> String {
    format!(
        "openagents cad mesh: {}\n  mesh id: {}\n  variant: {}\n  vertices: {}\n  triangles: {}\n  materials: {}",
        path.display(),
        mesh.mesh_id,
        mesh.variant_id,
        mesh.vertices.len(),
        mesh.triangle_indices.len() / 3,
        mesh.material_slots.len(),
    )
}

fn format_document_info(path: &Path, document: &CadDocument) -> String {
    format!(
        "openagents cad document: {}\n  schema version: {}\n  document id: {}\n  revision: {}\n  feature ids: {}\n  metadata entries: {}",
        path.display(),
        document.schema_version,
        document.document_id,
        document.revision,
        document.feature_ids.len(),
        document.metadata.len(),
    )
}

fn format_compact_ir_info(path: &Path, document: &crate::mcp_tools::CadMcpDocument) -> String {
    format!(
        "openagents cad compact-ir: {}\n  version: {}\n  nodes: {}\n  roots: {}\n  materials: {}",
        path.display(),
        document.version,
        document.nodes.len(),
        document.roots.len(),
        document.materials.len(),
    )
}

fn format_step_info(
    path: &Path,
    solid_count: usize,
    shell_count: usize,
    face_count: usize,
    entity_count: usize,
    checker_pass: bool,
) -> String {
    format!(
        "openagents cad step: {}\n  solids: {}\n  shells: {}\n  faces: {}\n  entities: {}\n  checker pass: {}",
        path.display(),
        solid_count,
        shell_count,
        face_count,
        entity_count,
        checker_pass,
    )
}

fn stem_or_default(path: &Path, default: &str) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map_or_else(|| default.to_string(), |stem| stem.to_string())
}

fn output_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(String::new, |extension| extension.to_ascii_lowercase())
}

fn is_help_request(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--help" || arg == "-h" || arg == "help")
}

fn root_help_text(version: &str) -> String {
    format!(
        "{CAD_CLI_APP_NAME} {version}\n\nUSAGE:\n  {CAD_CLI_APP_NAME} <COMMAND> [ARGS]\n  {CAD_CLI_APP_NAME} --help\n\nCOMMANDS:\n  export   Export CAD document to a target format\n  import   Import CAD data into OpenAgents CAD document\n  info     Inspect CAD document metadata and mesh summary\n  help     Print command help\n\nScaffold status: {CAD_CLI_SCAFFOLD_ISSUE_ID} established command surface; command handlers land in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}."
    )
}

fn subcommand_help_text(command: CadCliCommand) -> String {
    format!(
        "USAGE:\n  {CAD_CLI_APP_NAME} {} [ARGS]\n\nCommand parity: {} handler is implemented in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}.",
        command.as_str(),
        command.as_str(),
    )
}

fn scaffold_stub_error(command: CadCliCommand) -> String {
    format!(
        "{} command scaffold is present; implementation lands in {CAD_CLI_IMPLEMENTATION_ISSUE_ID}",
        command.as_str()
    )
}

fn export_usage() -> String {
    format!(
        "USAGE:\n  {CAD_CLI_APP_NAME} export <input_mesh_json> <output.stl|output.glb|output.step|output.stp>"
    )
}

fn import_usage() -> String {
    format!(
        "USAGE:\n  {CAD_CLI_APP_NAME} import <input.stl|input.step|input.stp|input.cad0|input.vcadc> <output_json> [--name <id>]"
    )
}

fn info_usage() -> String {
    format!("USAGE:\n  {CAD_CLI_APP_NAME} info <input_json|input.step|input.stp>")
}

#[cfg(test)]
mod tests {
    use super::{CAD_CLI_SCAFFOLD_COMMANDS, CAD_CLI_STUB_EXIT_CODE, run_cli_tokens};

    #[test]
    fn root_help_lists_scaffold_commands() {
        let outcome = run_cli_tokens(&["openagents-cad-cli", "--help"]);
        assert_eq!(outcome.exit_code, 0);
        for command in CAD_CLI_SCAFFOLD_COMMANDS {
            assert!(outcome.stdout.contains(command));
        }
    }

    #[test]
    fn scaffold_commands_return_stub_exit_code_without_args() {
        for command in CAD_CLI_SCAFFOLD_COMMANDS {
            let outcome = run_cli_tokens(&["openagents-cad-cli", command]);
            assert_eq!(outcome.exit_code, CAD_CLI_STUB_EXIT_CODE);
            assert!(
                outcome
                    .stderr
                    .contains("implementation lands in VCAD-PARITY-084")
            );
        }
    }

    #[test]
    fn unknown_command_returns_usage_error() {
        let outcome = run_cli_tokens(&["openagents-cad-cli", "unknown"]);
        assert_eq!(outcome.exit_code, 2);
        assert!(outcome.stderr.contains("unknown command: unknown"));
        assert!(outcome.stderr.contains("USAGE:"));
    }
}
