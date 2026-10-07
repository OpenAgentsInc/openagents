use psionic_runtime::{
    tassadar_pointer_memory_scratchpad_runtime_report_path,
    write_tassadar_pointer_memory_scratchpad_runtime_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_pointer_memory_scratchpad_runtime_report_path();
    let report = write_tassadar_pointer_memory_scratchpad_runtime_report(&output_path)?;
    println!(
        "wrote pointer/memory/scratchpad runtime report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
