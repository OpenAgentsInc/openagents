use psionic_research::{
    tassadar_relaxed_simd_research_summary_path, write_tassadar_relaxed_simd_research_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_relaxed_simd_research_summary(
        tassadar_relaxed_simd_research_summary_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
