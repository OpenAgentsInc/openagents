use std::path::PathBuf;

use psionic_train::{
    compiled_agent_tailnet_governed_run_fixture_path,
    compiled_agent_tailnet_quarantine_report_fixture_path,
    compiled_agent_tailnet_staging_ledger_fixture_path, load_compiled_agent_tailnet_node_bundle,
    write_compiled_agent_tailnet_governed_run,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut local_bundle = None;
    let mut remote_bundle = None;
    let mut staging_output = None;
    let mut quarantine_output = None;
    let mut run_output = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--local-bundle" => {
                local_bundle = Some(PathBuf::from(
                    args.next().ok_or("missing value for --local-bundle")?,
                ));
            }
            "--remote-bundle" => {
                remote_bundle = Some(PathBuf::from(
                    args.next().ok_or("missing value for --remote-bundle")?,
                ));
            }
            "--staging-output" => {
                staging_output = Some(PathBuf::from(
                    args.next().ok_or("missing value for --staging-output")?,
                ));
            }
            "--quarantine-output" => {
                quarantine_output = Some(PathBuf::from(
                    args.next().ok_or("missing value for --quarantine-output")?,
                ));
            }
            "--run-output" => {
                run_output = Some(PathBuf::from(
                    args.next().ok_or("missing value for --run-output")?,
                ));
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let local_bundle_path = local_bundle.ok_or("missing required --local-bundle")?;
    let remote_bundle_path = remote_bundle.ok_or("missing required --remote-bundle")?;
    let local_bundle = load_compiled_agent_tailnet_node_bundle(local_bundle_path)?;
    let remote_bundle = load_compiled_agent_tailnet_node_bundle(remote_bundle_path)?;
    let staging_output =
        staging_output.unwrap_or_else(compiled_agent_tailnet_staging_ledger_fixture_path);
    let quarantine_output = quarantine_output
        .unwrap_or_else(compiled_agent_tailnet_quarantine_report_fixture_path);
    let run_output = run_output.unwrap_or_else(compiled_agent_tailnet_governed_run_fixture_path);
    let run = write_compiled_agent_tailnet_governed_run(
        &local_bundle,
        &remote_bundle,
        &staging_output,
        &quarantine_output,
        &run_output,
    )?;
    println!(
        "wrote compiled-agent tailnet governed run path={} digest={}",
        run_output.display(),
        run.run_digest
    );
    println!(
        "staging_ledger={} quarantine_report={}",
        staging_output.display(),
        quarantine_output.display()
    );
    Ok(())
}
