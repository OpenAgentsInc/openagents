use psionic_eval::{
    tassadar_relaxed_simd_research_ladder_report_path,
    write_tassadar_relaxed_simd_research_ladder_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_relaxed_simd_research_ladder_report(
        tassadar_relaxed_simd_research_ladder_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
