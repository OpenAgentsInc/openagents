use std::path::PathBuf;

use psionic_train::{FirstSwarmTrustedLanRuntimeRole, run_first_swarm_trusted_lan_runtime};

fn parse_role(raw: &str) -> Result<FirstSwarmTrustedLanRuntimeRole, String> {
    match raw {
        "coordinator" => Ok(FirstSwarmTrustedLanRuntimeRole::Coordinator),
        "contributor" => Ok(FirstSwarmTrustedLanRuntimeRole::Contributor),
        other => Err(format!(
            "unknown role `{other}`; expected `coordinator` or `contributor`"
        )),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut role = None;
    let mut run_id = None;
    let mut topology_contract = None;
    let mut workflow_plan = None;
    let mut local_endpoint = None;
    let mut peer_endpoint = None;
    let mut output = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--role" => role = args.next(),
            "--run-id" => run_id = args.next(),
            "--topology-contract" => topology_contract = args.next(),
            "--workflow-plan" => workflow_plan = args.next(),
            "--local-endpoint" => local_endpoint = args.next(),
            "--peer-endpoint" => peer_endpoint = args.next(),
            "--output" => output = args.next(),
            "--help" | "-h" => {
                eprintln!(
                    "Usage: first_swarm_trusted_lan_live_runtime --role <coordinator|contributor> --run-id <id> --topology-contract <path> --workflow-plan <path> --local-endpoint <ip:port> --peer-endpoint <ip:port> --output <path>"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }

    let role = parse_role(role.as_deref().ok_or("--role is required")?)?;
    let run_id = run_id.ok_or("--run-id is required")?;
    let topology_contract =
        PathBuf::from(topology_contract.ok_or("--topology-contract is required")?);
    let workflow_plan = PathBuf::from(workflow_plan.ok_or("--workflow-plan is required")?);
    let local_endpoint = local_endpoint.ok_or("--local-endpoint is required")?;
    let peer_endpoint = peer_endpoint.ok_or("--peer-endpoint is required")?;
    let output = PathBuf::from(output.ok_or("--output is required")?);

    let report = run_first_swarm_trusted_lan_runtime(
        role,
        run_id,
        topology_contract,
        workflow_plan,
        local_endpoint.as_str(),
        peer_endpoint.as_str(),
        output,
    )?;
    println!(
        "wrote first swarm trusted-LAN runtime report for {} node `{}` with digest {}",
        report.runtime_role.label(),
        report.node_id,
        report.report_digest
    );
    Ok(())
}
