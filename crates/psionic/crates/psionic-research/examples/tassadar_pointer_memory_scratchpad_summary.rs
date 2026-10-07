use psionic_research::{
    tassadar_pointer_memory_scratchpad_summary_path,
    write_tassadar_pointer_memory_scratchpad_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_pointer_memory_scratchpad_summary_path();
    let summary = write_tassadar_pointer_memory_scratchpad_summary(&output_path)?;
    println!(
        "wrote pointer/memory/scratchpad summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
