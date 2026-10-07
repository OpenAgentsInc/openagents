use psionic_research::{
    tassadar_architecture_bakeoff_summary_path, write_tassadar_architecture_bakeoff_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_architecture_bakeoff_summary_path();
    let summary = write_tassadar_architecture_bakeoff_summary(&output_path)?;
    println!(
        "wrote architecture bakeoff summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
