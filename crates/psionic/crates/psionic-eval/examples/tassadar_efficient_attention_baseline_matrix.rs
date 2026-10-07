use psionic_eval::{
    tassadar_efficient_attention_baseline_matrix_report_path,
    write_tassadar_efficient_attention_baseline_matrix_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_efficient_attention_baseline_matrix_report(
        tassadar_efficient_attention_baseline_matrix_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
