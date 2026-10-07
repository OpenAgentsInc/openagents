use psionic_eval::{
    compiled_agent_module_eval_report_path, write_compiled_agent_module_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = compiled_agent_module_eval_report_path();
    let report = write_compiled_agent_module_eval_report(&report_path)?;
    println!(
        "wrote compiled-agent module eval report={} digest={}",
        report_path.display(),
        report.report_digest
    );
    Ok(())
}
