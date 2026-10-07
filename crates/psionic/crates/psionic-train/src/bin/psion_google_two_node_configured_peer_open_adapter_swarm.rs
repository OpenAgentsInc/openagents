use std::path::PathBuf;

use psionic_train::{run_psion_google_two_node_swarm_runtime, PsionGoogleTwoNodeSwarmRuntimeRole};

fn parse_role(raw: &str) -> Result<PsionGoogleTwoNodeSwarmRuntimeRole, String> {
    match raw {
        "coordinator" => Ok(PsionGoogleTwoNodeSwarmRuntimeRole::Coordinator),
        "contributor" => Ok(PsionGoogleTwoNodeSwarmRuntimeRole::Contributor),
        other => Err(format!(
            "unknown role `{other}`; expected `coordinator` or `contributor`"
        )),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut role = None;
    let mut cluster_manifest = None;
    let mut local_endpoint = None;
    let mut peer_endpoint = None;
    let mut output = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--role" => role = args.next(),
            "--cluster-manifest" => cluster_manifest = args.next(),
            "--local-endpoint" => local_endpoint = args.next(),
            "--peer-endpoint" => peer_endpoint = args.next(),
            "--output" => output = args.next(),
            "--help" | "-h" => {
                eprintln!(
                    "Usage: psion_google_two_node_configured_peer_open_adapter_swarm --role <coordinator|contributor> --cluster-manifest <path> --local-endpoint <path> --peer-endpoint <path> --output <path>"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }

    let role = parse_role(role.as_deref().ok_or("--role is required")?)?;
    let cluster_manifest = PathBuf::from(cluster_manifest.ok_or("--cluster-manifest is required")?);
    let local_endpoint = PathBuf::from(local_endpoint.ok_or("--local-endpoint is required")?);
    let peer_endpoint = PathBuf::from(peer_endpoint.ok_or("--peer-endpoint is required")?);
    let output = PathBuf::from(output.ok_or("--output is required")?);

    let report = run_psion_google_two_node_swarm_runtime(
        role,
        cluster_manifest,
        local_endpoint,
        peer_endpoint,
        output,
    )?;
    println!(
        "wrote Google two-node swarm runtime report for {} node `{}` with digest {}",
        report.runtime_role.label(),
        report.node_id,
        report.report_digest
    );
    Ok(())
}
