use psionic_eval::{
    compiled_agent_module_optimization_proof_report_path,
    write_compiled_agent_module_optimization_proof_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = compiled_agent_module_optimization_proof_report_path();
    let report = write_compiled_agent_module_optimization_proof_report(&report_path)?;
    println!(
        "wrote compiled-agent module optimization proof report={} digest={}",
        report_path.display(),
        report.report_digest
    );
    Ok(())
}
