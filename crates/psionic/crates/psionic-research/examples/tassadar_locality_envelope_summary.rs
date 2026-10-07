use psionic_research::{
    tassadar_locality_envelope_summary_path, write_tassadar_locality_envelope_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_locality_envelope_summary_path();
    let summary = write_tassadar_locality_envelope_summary(&output_path)?;
    println!(
        "wrote locality-envelope summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
