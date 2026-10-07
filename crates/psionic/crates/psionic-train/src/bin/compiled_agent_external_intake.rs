use psionic_train::{
    compiled_agent_external_quarantine_report_fixture_path,
    compiled_agent_external_replay_proposal_fixture_path,
    compiled_agent_external_runtime_receipt_submission_fixture_path,
    compiled_agent_external_submission_staging_ledger_fixture_path,
    write_compiled_agent_external_quarantine_report,
    write_compiled_agent_external_replay_proposal,
    write_compiled_agent_external_runtime_receipt_submission,
    write_compiled_agent_external_submission_staging_ledger,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime_path = compiled_agent_external_runtime_receipt_submission_fixture_path();
    let proposal_path = compiled_agent_external_replay_proposal_fixture_path();
    let staging_path = compiled_agent_external_submission_staging_ledger_fixture_path();
    let quarantine_path = compiled_agent_external_quarantine_report_fixture_path();

    let runtime = write_compiled_agent_external_runtime_receipt_submission(&runtime_path)?;
    let proposal = write_compiled_agent_external_replay_proposal(&proposal_path)?;
    let staging = write_compiled_agent_external_submission_staging_ledger(&staging_path)?;
    let quarantine = write_compiled_agent_external_quarantine_report(&quarantine_path)?;

    println!(
        "wrote compiled-agent external runtime receipt submission={} digest={}",
        runtime_path.display(),
        runtime.payload_digest
    );
    println!(
        "wrote compiled-agent external replay proposal={} digest={}",
        proposal_path.display(),
        proposal.payload_digest
    );
    println!(
        "wrote compiled-agent external staging ledger={} digest={}",
        staging_path.display(),
        staging.ledger_digest
    );
    println!(
        "wrote compiled-agent external quarantine report={} digest={}",
        quarantine_path.display(),
        quarantine.report_digest
    );
    Ok(())
}
