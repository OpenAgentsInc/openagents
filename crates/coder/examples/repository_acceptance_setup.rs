//! Create two fresh admitted synthetic tasks; this helper runs no model.
use coder::task::{self, checks};
use coder_boundary::Snapshot;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
fn write(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn run() -> Result<Value, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(args.len() == 3 || args.len() == 4) {
        return Err("usage: repository_acceptance_setup NEW_ROOT MICROCODER_BINARY CHECKER_BINARY [CONTAINER_PROFILE.json]".into());
    }
    let root = PathBuf::from(&args[0]);
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let controller = PathBuf::from(&args[1])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let controller_digest =
        nostr::contracts::digest_bytes(&std::fs::read(&controller).map_err(|e| e.to_string())?);
    let checker = PathBuf::from(&args[2])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let profile: Option<Value> = args
        .get(3)
        .map(|p| {
            std::fs::read(p)
                .map_err(|e| e.to_string())
                .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
        })
        .transpose()?;
    let registry = root.join("registry");
    let capabilities = registry.join("capabilities");
    std::fs::create_dir_all(&capabilities).map_err(|e| e.to_string())?;
    let trust_file = root.join("trust.json");
    let mut trust = capability::Trust::load(&trust_file)?;
    let mut cases = Vec::new();
    for (name, source, prompt) in [
        (
            "ceil",
            include_str!("../../../bench/coder/repository-acceptance/ceil/lib.rs"),
            "Fix ceil_div in lib.rs for the documented contract over all u64 inputs. Preserve the public signature.",
        ),
        (
            "range",
            include_str!("../../../bench/coder/repository-acceptance/range/lib.rs"),
            "Fix inclusive_span in lib.rs for the documented contract over all i64 inputs. Preserve the public signature.",
        ),
    ] {
        let repo = root.join(format!("{name}-repo"));
        let workspace = root.join(format!("{name}-workspace"));
        std::fs::create_dir(&repo).map_err(|e| e.to_string())?;
        std::fs::write(repo.join("lib.rs"), source).map_err(|e| e.to_string())?;
        git(&repo, &["init", "-q"])?;
        git(&repo, &["add", "lib.rs"])?;
        git(
            &repo,
            &[
                "-c",
                "user.name=Acceptance Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "Original synthetic Rust bug",
            ],
        )?;
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--detach",
                "-q",
                workspace.to_str().ok_or("workspace path")?,
            ],
        )?;
        let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
        let program = root.join(format!("repository-check-{name}"));
        std::fs::copy(&checker, &program).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let program = program.canonicalize().map_err(|e| e.to_string())?;
        let slug = format!("repository-check-{name}");
        let manifest = capabilities.join(format!("{slug}.json"));
        write(
            &manifest,
            &capability::executor_document(
                &slug,
                program.to_str().ok_or("checker path")?,
                vec![program.display().to_string(), "--version".into()],
                json!({"name":"Independent synthetic Rust suite","invoke":[program],"isolation":["directory"]}),
            ),
        )?;
        let entry = capability::Entry::load(&manifest, capability::Source::Operator)?;
        trust.approve(Some(&registry), &slug, &[])?;
        let suite_digest =
            nostr::contracts::digest_bytes(&std::fs::read(&program).map_err(|e| e.to_string())?);
        let requirements = checks::Requirements {
            schema: checks::REQUIREMENTS_SCHEMA.into(),
            version: 1,
            requirements: vec![checks::Requirement {
                id: "contract".into(),
                statement: prompt.into(),
                checks: vec!["independent-rust".into()],
            }],
            plan: json!({"schema":coder::verification::SCHEMA,"input_digest":checks::CANDIDATE,"seconds":50,"allow_unrestricted_reads":true,"allow_network":true,
                "checks":[{"id":"independent-rust","manifest":manifest,"manifest_digest":entry.digest,"arguments":[checks::CANDIDATE],"seconds":45,"output_bytes":65536,
                "acceptance":{"kind":"suite","suite_digest":suite_digest,"input_digest":checks::CANDIDATE}}]}),
            instruction_targets: vec![],
            source_exclusions: vec![format!("repository-task-{name}")],
            task_sources: vec![format!("repository-task-{name}")],
            check_lineage: vec![checks::CheckLineage {
                check: "independent-rust".into(),
                sources: vec![format!("public-integer-contract-{name}")],
            }],
            knowledge: vec![],
        };
        let intent = task::TaskIntent {
            title: format!("Synthetic {name} repair"),
            prompt: prompt.into(),
            workspace: task::Workspace {
                path: workspace.display().to_string(),
                source_revision: Some(git(&workspace, &["rev-parse", "HEAD"])?),
            },
            configuration: task::RequestedConfiguration {
                adapter: task::adapter::NAME.into(),
                model: Some("gpt-6-luna".into()),
            },
        };
        let task_id = format!("repository-{name}");
        let store = root.join(format!("{name}-tasks"));
        let command = task::Command {
            schema: task::COMMAND_SCHEMA.into(),
            command_id: format!("submit-{name}"),
            task_id: task_id.clone(),
            expected_revision: None,
            action: task::Action::Submit { intent },
        };
        let bytes = serde_json::to_vec(&command).map_err(|e| e.to_string())?;
        let mut inbox = task::Store::open(&store).map_err(|e| e.to_string())?;
        inbox.apply(&bytes).map_err(|e| e.to_string())?;
        let task = inbox.show(&task_id).map_err(|e| e.to_string())?;
        drop(inbox);
        let grant = json!({"schema":task::owner::GRANT_SCHEMA,"task_id":task_id,"intent_digest":task.intent_digest,"expected_revision":task.revision,
            "expected_source_snapshot":Snapshot::observe(&workspace).digest(),"program":Path::new("/bin/bash").canonicalize().map_err(|e|e.to_string())?,
            "arguments":[],"write_workspace":true,"wall_seconds":300,"stream_bytes":65536,"memory_bytes":1073741824,"requirements":requirements,
            "adapter_configuration":{"schema":task::adapter::CONFIG_SCHEMA,"provider":"codex","model":"gpt-6-luna","effort":"medium",
            "generation_endpoint":"https://chatgpt.com/backend-api/codex","decision_endpoint":std::env::var("TYPESAFE_BASE_URL").unwrap_or_else(|_|"https://api.typesafe.ai".into()),
            "decision_model":std::env::var("TYPESAFE_DEFAULT_MODEL").unwrap_or_else(|_|"jev-latest".into()),"max_steps":8,"acceptance":false,"route":"never","knowledge":"off","dollar_limit_micros":null,
            "expected_controller_digest":controller_digest,"container":if name=="range"{profile.clone()}else{None}}});
        task::owner::Grant::parse(&serde_json::to_vec(&grant).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let grant_path = root.join(format!("{name}-grant.json"));
        write(&grant_path, &grant)?;
        write(&root.join(format!("{name}-submission.json")), &command)?;
        cases.push(json!({"name":name,"task_id":task_id,"store":store,"workspace":workspace,"grant":grant_path,"checker":program,"trust":trust_file,"controller":controller,"model_calls_started":false}));
    }
    let value = json!({"schema":"openagents.repository-acceptance-setup.v1","synthetic":true,"cases":cases});
    write(&root.join("cases.json"), &value)?;
    Ok(value)
}
fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(reason) => {
            eprintln!("{reason}");
            std::process::exit(1);
        }
    }
}
