use psionic_train::{
    CompiledAgentDecentralizedRoleKind, canonical_compiled_agent_decentralized_role_dry_run_report,
    canonical_compiled_agent_decentralized_role_receipts,
    canonical_compiled_agent_decentralized_roles_contract,
    compiled_agent_decentralized_role_dry_run_fixture_path,
    compiled_agent_decentralized_role_receipts_fixture_path,
    compiled_agent_decentralized_role_snapshot,
    compiled_agent_decentralized_roles_contract_fixture_path,
    write_compiled_agent_decentralized_role_dry_run,
    write_compiled_agent_decentralized_role_receipts,
    write_compiled_agent_decentralized_roles_contract,
};
use serde::Serialize;

#[derive(Serialize)]
struct CompiledAgentRoleSnapshot {
    role: psionic_train::CompiledAgentDecentralizedRoleDefinition,
    receipt: psionic_train::CompiledAgentDecentralizedRoleReceipt,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut selected_role = None;
    let mut print_dry_run = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--role" => {
                let role_name = args.next().ok_or("expected role name after --role")?;
                selected_role = Some(parse_role(role_name.as_str())?);
            }
            "--dry-run" => {
                print_dry_run = true;
            }
            other => {
                return Err(format!("unsupported argument `{other}`").into());
            }
        }
    }

    if selected_role.is_some() && print_dry_run {
        return Err("use either --role <name> or --dry-run, not both".into());
    }

    if let Some(role) = selected_role {
        let (definition, receipt) = compiled_agent_decentralized_role_snapshot(role)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&CompiledAgentRoleSnapshot {
                role: definition,
                receipt,
            })?
        );
        return Ok(());
    }

    if print_dry_run {
        let report = canonical_compiled_agent_decentralized_role_dry_run_report()?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let contract_path = compiled_agent_decentralized_roles_contract_fixture_path();
    let receipts_path = compiled_agent_decentralized_role_receipts_fixture_path();
    let dry_run_path = compiled_agent_decentralized_role_dry_run_fixture_path();
    let contract = write_compiled_agent_decentralized_roles_contract(&contract_path)?;
    let receipts = write_compiled_agent_decentralized_role_receipts(&receipts_path)?;
    let dry_run = write_compiled_agent_decentralized_role_dry_run(&dry_run_path)?;
    let _ = canonical_compiled_agent_decentralized_roles_contract()?;
    let _ = canonical_compiled_agent_decentralized_role_receipts()?;
    let _ = canonical_compiled_agent_decentralized_role_dry_run_report()?;
    println!(
        "wrote compiled-agent decentralized roles contract={} digest={}",
        contract_path.display(),
        contract.contract_digest
    );
    println!(
        "wrote compiled-agent decentralized role receipts={} digest={}",
        receipts_path.display(),
        receipts.receipts_digest
    );
    println!(
        "wrote compiled-agent decentralized role dry run={} digest={}",
        dry_run_path.display(),
        dry_run.report_digest
    );
    Ok(())
}

fn parse_role(role_name: &str) -> Result<CompiledAgentDecentralizedRoleKind, String> {
    match role_name {
        "replay_generation" => Ok(CompiledAgentDecentralizedRoleKind::ReplayGeneration),
        "ranking_labeling" => Ok(CompiledAgentDecentralizedRoleKind::RankingLabeling),
        "validator_scoring" => Ok(CompiledAgentDecentralizedRoleKind::ValidatorScoring),
        "bounded_module_training" => Ok(CompiledAgentDecentralizedRoleKind::BoundedModuleTraining),
        _ => Err(format!("unsupported role `{role_name}`")),
    }
}
