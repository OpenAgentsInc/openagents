use psionic_train::{
    compiled_agent_confidence_policy_fixture_path,
    compiled_agent_grounded_candidate_report_fixture_path,
    compiled_agent_grounded_model_artifact_fixture_path,
    compiled_agent_grounded_stronger_candidate_report_fixture_path,
    compiled_agent_grounded_stronger_model_artifact_fixture_path,
    compiled_agent_phase_six_operational_report_fixture_path,
    compiled_agent_promoted_artifact_contract_fixture_path,
    compiled_agent_route_candidate_report_fixture_path,
    compiled_agent_route_model_artifact_fixture_path,
    compiled_agent_route_stronger_candidate_report_fixture_path,
    compiled_agent_route_stronger_model_artifact_fixture_path,
    compiled_agent_shadow_disagreement_receipts_fixture_path,
    compiled_agent_stronger_candidate_family_report_fixture_path,
    compiled_agent_xtrain_cycle_receipt_fixture_path, write_compiled_agent_confidence_policy,
    write_compiled_agent_grounded_candidate_report, write_compiled_agent_grounded_model_artifact,
    write_compiled_agent_grounded_stronger_candidate_report,
    write_compiled_agent_grounded_stronger_model_artifact,
    write_compiled_agent_phase_six_operational_report,
    write_compiled_agent_promoted_artifact_contract, write_compiled_agent_route_candidate_report,
    write_compiled_agent_route_model_artifact,
    write_compiled_agent_route_stronger_candidate_report,
    write_compiled_agent_route_stronger_model_artifact,
    write_compiled_agent_shadow_disagreement_receipts,
    write_compiled_agent_stronger_candidate_family_report,
    write_compiled_agent_xtrain_cycle_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let route_model_path = compiled_agent_route_model_artifact_fixture_path();
    let route_model = write_compiled_agent_route_model_artifact(&route_model_path)?;
    let grounded_model_path = compiled_agent_grounded_model_artifact_fixture_path();
    let grounded_model = write_compiled_agent_grounded_model_artifact(&grounded_model_path)?;
    let route_stronger_model_path = compiled_agent_route_stronger_model_artifact_fixture_path();
    let route_stronger_model =
        write_compiled_agent_route_stronger_model_artifact(&route_stronger_model_path)?;
    let grounded_stronger_model_path =
        compiled_agent_grounded_stronger_model_artifact_fixture_path();
    let grounded_stronger_model =
        write_compiled_agent_grounded_stronger_model_artifact(&grounded_stronger_model_path)?;
    let route_path = compiled_agent_route_candidate_report_fixture_path();
    let route_report = write_compiled_agent_route_candidate_report(&route_path)?;
    let grounded_path = compiled_agent_grounded_candidate_report_fixture_path();
    let grounded_report = write_compiled_agent_grounded_candidate_report(&grounded_path)?;
    let route_stronger_path = compiled_agent_route_stronger_candidate_report_fixture_path();
    let route_stronger_report =
        write_compiled_agent_route_stronger_candidate_report(&route_stronger_path)?;
    let grounded_stronger_path = compiled_agent_grounded_stronger_candidate_report_fixture_path();
    let grounded_stronger_report =
        write_compiled_agent_grounded_stronger_candidate_report(&grounded_stronger_path)?;
    let stronger_family_report_path =
        compiled_agent_stronger_candidate_family_report_fixture_path();
    let stronger_family_report =
        write_compiled_agent_stronger_candidate_family_report(&stronger_family_report_path)?;
    let cycle_path = compiled_agent_xtrain_cycle_receipt_fixture_path();
    let cycle = write_compiled_agent_xtrain_cycle_receipt(&cycle_path)?;
    let contract_path = compiled_agent_promoted_artifact_contract_fixture_path();
    let contract = write_compiled_agent_promoted_artifact_contract(&contract_path)?;
    let confidence_policy_path = compiled_agent_confidence_policy_fixture_path();
    let confidence_policy = write_compiled_agent_confidence_policy(&confidence_policy_path)?;
    let disagreement_receipts_path = compiled_agent_shadow_disagreement_receipts_fixture_path();
    let disagreement_receipts =
        write_compiled_agent_shadow_disagreement_receipts(&disagreement_receipts_path)?;
    let phase_six_report_path = compiled_agent_phase_six_operational_report_fixture_path();
    let phase_six_report =
        write_compiled_agent_phase_six_operational_report(&phase_six_report_path)?;

    println!(
        "wrote route model artifact={} digest={}",
        route_model_path.display(),
        route_model.artifact_digest
    );
    println!(
        "wrote grounded-answer model artifact={} digest={}",
        grounded_model_path.display(),
        grounded_model.artifact_digest
    );
    println!(
        "wrote stronger route model artifact={} digest={}",
        route_stronger_model_path.display(),
        route_stronger_model.artifact_digest
    );
    println!(
        "wrote stronger grounded-answer model artifact={} digest={}",
        grounded_stronger_model_path.display(),
        grounded_stronger_model.artifact_digest
    );
    println!(
        "wrote route candidate module eval report={} digest={}",
        route_path.display(),
        route_report.report_digest
    );
    println!(
        "wrote grounded candidate module eval report={} digest={}",
        grounded_path.display(),
        grounded_report.report_digest
    );
    println!(
        "wrote stronger route candidate module eval report={} digest={}",
        route_stronger_path.display(),
        route_stronger_report.report_digest
    );
    println!(
        "wrote stronger grounded candidate module eval report={} digest={}",
        grounded_stronger_path.display(),
        grounded_stronger_report.report_digest
    );
    println!(
        "wrote stronger candidate family report={} digest={}",
        stronger_family_report_path.display(),
        stronger_family_report.report_digest
    );
    println!(
        "wrote compiled-agent xtrain cycle receipt={} digest={}",
        cycle_path.display(),
        cycle.receipt_digest
    );
    println!(
        "wrote compiled-agent promoted artifact contract={} digest={}",
        contract_path.display(),
        contract.contract_digest
    );
    println!(
        "wrote compiled-agent confidence policy={} digest={}",
        confidence_policy_path.display(),
        confidence_policy.policy_digest
    );
    println!(
        "wrote compiled-agent shadow disagreement receipts={} digest={}",
        disagreement_receipts_path.display(),
        disagreement_receipts.receipts_digest
    );
    println!(
        "wrote compiled-agent phase-six operational report={} digest={}",
        phase_six_report_path.display(),
        phase_six_report.report_digest
    );
    Ok(())
}
