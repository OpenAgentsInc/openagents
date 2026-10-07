use psionic_research::{
    tassadar_supervision_density_summary_path, write_tassadar_supervision_density_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_supervision_density_summary_path();
    let summary = write_tassadar_supervision_density_summary(&output_path)?;
    println!(
        "wrote supervision-density summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
