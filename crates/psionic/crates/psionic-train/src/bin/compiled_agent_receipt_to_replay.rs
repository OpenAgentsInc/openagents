use psionic_train::{
    compiled_agent_learning_receipt_ledger_fixture_path, compiled_agent_replay_bundle_fixture_path,
    compiled_agent_runtime_fixture_dir, compiled_agent_source_fixture_dir,
    write_compiled_agent_learning_receipt_ledger, write_compiled_agent_replay_bundle,
    write_compiled_agent_runtime_receipts, write_compiled_agent_source_receipts,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_dir = compiled_agent_source_fixture_dir();
    let source_receipts = write_compiled_agent_source_receipts(&source_dir)?;
    let runtime_dir = compiled_agent_runtime_fixture_dir();
    let runtime_receipts = write_compiled_agent_runtime_receipts(&runtime_dir)?;
    let ledger_path = compiled_agent_learning_receipt_ledger_fixture_path();
    let ledger = write_compiled_agent_learning_receipt_ledger(&ledger_path)?;
    let bundle_path = compiled_agent_replay_bundle_fixture_path();
    let bundle = write_compiled_agent_replay_bundle(&bundle_path)?;
    println!(
        "wrote compiled-agent source receipts dir={} count={}",
        source_dir.display(),
        source_receipts.len()
    );
    println!(
        "wrote compiled-agent runtime receipts dir={} count={}",
        runtime_dir.display(),
        runtime_receipts.len()
    );
    println!(
        "wrote compiled-agent learning ledger={} digest={}",
        ledger_path.display(),
        ledger.ledger_digest
    );
    println!(
        "wrote compiled-agent replay bundle={} digest={}",
        bundle_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
