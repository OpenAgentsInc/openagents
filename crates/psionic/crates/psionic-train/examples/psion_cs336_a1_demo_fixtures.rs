use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
    PsionCs336A1DemoAutomaticExecutionRequest, PsionCs336A1DemoCloseoutBundle,
    PsionCs336A1DemoCurrentRunStatus, PsionCs336A1DemoRetainedSummary,
    PsionicTrainCheckpointManifest, PsionicTrainCheckpointPointer, PsionicTrainCoordinationContext,
    PsionicTrainOperation, PsionicTrainRole, run_psion_cs336_a1_demo_manifest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/training");
    let run_root = fixtures_dir
        .join("psion_cs336_a1_demo_example")
        .join("run-psion-cs336-a1-demo-fixture");
    if run_root.exists() {
        fs::remove_dir_all(&run_root)?;
    }
    fs::create_dir_all(&fixtures_dir)?;

    let request = PsionCs336A1DemoAutomaticExecutionRequest {
        schema_version: String::from(
            PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
        ),
        role: PsionicTrainRole::Worker,
        operation: PsionicTrainOperation::Start,
        coordination: PsionicTrainCoordinationContext {
            network_id: Some(String::from("network.psionic.cs336-a1-demo-fixture")),
            window_id: Some(String::from("window.cs336-a1-demo-fixture")),
            assignment_id: Some(String::from("assignment.cs336-a1-demo-fixture")),
            challenge_id: None,
            node_pubkey: Some(String::from("npub1-cs336-a1-demo-fixture")),
            membership_revision: Some(1),
        },
        build_digest: String::from("sha256:psion-cs336-a1-demo-fixture"),
        run_id: String::from("psion-cs336-a1-demo-fixture"),
        output_root: Some(run_root.display().to_string()),
        run_root: None,
        selected_git_ref: String::from("refs/heads/main"),
        allow_dirty_tree: true,
        dry_run: false,
    };
    let outputs = request.expected_outputs()?;
    let manifest = request.to_invocation_manifest()?;
    run_psion_cs336_a1_demo_manifest(&manifest)?;

    fs::write(
        fixtures_dir.join("psion_cs336_a1_demo_automatic_execution_request_v1.json"),
        serde_json::to_vec_pretty(&request)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_cs336_a1_demo_automatic_execution_outputs_v1.json"),
        serde_json::to_vec_pretty(&outputs)?,
    )?;

    let retained_paths = psionic_train::psion_cs336_a1_demo_retained_paths();
    let current_status: PsionCs336A1DemoCurrentRunStatus =
        parse_json(run_root.join(&retained_paths.current_status_path))?;
    let retained_summary: PsionCs336A1DemoRetainedSummary =
        parse_json(run_root.join(&retained_paths.retained_summary_path))?;
    let checkpoint_pointer: PsionicTrainCheckpointPointer =
        parse_json(run_root.join(&retained_paths.checkpoint_pointer_path))?;
    let checkpoint_manifest: PsionicTrainCheckpointManifest =
        parse_json(run_root.join(&retained_paths.checkpoint_manifest_path))?;
    let closeout_bundle: PsionCs336A1DemoCloseoutBundle =
        parse_json(run_root.join(&retained_paths.closeout_bundle_path))?;

    println!(
        "wrote {} request={} outputs={} phase={} checkpoint_label={} final_loss={:.6}",
        run_root.display(),
        fixtures_dir
            .join("psion_cs336_a1_demo_automatic_execution_request_v1.json")
            .display(),
        fixtures_dir
            .join("psion_cs336_a1_demo_automatic_execution_outputs_v1.json")
            .display(),
        current_status.phase,
        checkpoint_pointer.checkpoint_label,
        retained_summary
            .final_loss
            .ok_or("retained summary missing final_loss")?,
    );
    println!(
        "closeout={} checkpoint_manifest_digest={}",
        closeout_bundle.schema_version, checkpoint_manifest.manifest_digest
    );
    Ok(())
}

fn parse_json<T: serde::de::DeserializeOwned>(
    path: impl AsRef<std::path::Path>,
) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path.as_ref())?)?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
