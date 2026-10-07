use psionic_train::{
    compiled_agent_external_worker_beta_contract_fixture_path,
    compiled_agent_external_worker_dry_run_fixture_path,
    compiled_agent_external_worker_receipts_fixture_path,
    compiled_agent_external_worker_snapshot, write_compiled_agent_external_worker_beta_contract,
    write_compiled_agent_external_worker_dry_run, write_compiled_agent_external_worker_receipts,
    CompiledAgentDecentralizedRoleKind,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("--role") {
        let role = parse_role(
            args.get(1)
                .ok_or("expected one role after --role")?,
        )?;
        let (definition, receipt) = compiled_agent_external_worker_snapshot(role)?;
        println!("{}", serde_json::to_string_pretty(&definition)?);
        println!("{}", serde_json::to_string_pretty(&receipt)?);
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--dry-run") {
        let report = write_compiled_agent_external_worker_dry_run(
            compiled_agent_external_worker_dry_run_fixture_path(),
        )?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let contract_path = compiled_agent_external_worker_beta_contract_fixture_path();
    let receipts_path = compiled_agent_external_worker_receipts_fixture_path();
    let dry_run_path = compiled_agent_external_worker_dry_run_fixture_path();

    let contract = write_compiled_agent_external_worker_beta_contract(&contract_path)?;
    let receipts = write_compiled_agent_external_worker_receipts(&receipts_path)?;
    let report = write_compiled_agent_external_worker_dry_run(&dry_run_path)?;

    println!(
        "wrote compiled-agent external worker beta contract={} digest={}",
        contract_path.display(),
        contract.contract_digest
    );
    println!(
        "wrote compiled-agent external worker receipts={} digest={}",
        receipts_path.display(),
        receipts.receipts_digest
    );
    println!(
        "wrote compiled-agent external worker dry run={} digest={}",
        dry_run_path.display(),
        report.report_digest
    );
    Ok(())
}

fn parse_role(input: &str) -> Result<CompiledAgentDecentralizedRoleKind, String> {
    match input {
        "replay_generation" => Ok(CompiledAgentDecentralizedRoleKind::ReplayGeneration),
        "ranking_labeling" => Ok(CompiledAgentDecentralizedRoleKind::RankingLabeling),
        "validator_scoring" => Ok(CompiledAgentDecentralizedRoleKind::ValidatorScoring),
        "bounded_module_training" => Ok(CompiledAgentDecentralizedRoleKind::BoundedModuleTraining),
        other => Err(format!("unsupported role `{other}`")),
    }
}
