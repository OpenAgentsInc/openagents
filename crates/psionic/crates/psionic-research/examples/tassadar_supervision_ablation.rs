use psionic_research::{run_tassadar_supervision_ablation, tassadar_supervision_ablation_output_path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = tassadar_supervision_ablation_output_path();
    let report = run_tassadar_supervision_ablation(output_dir.as_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
