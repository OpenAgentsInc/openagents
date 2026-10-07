use psionic_eval::{
    tassadar_compile_pipeline_matrix_report_path, write_tassadar_compile_pipeline_matrix_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_compile_pipeline_matrix_report(
        tassadar_compile_pipeline_matrix_report_path(),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .expect("compile-pipeline matrix report should serialize")
    );
    Ok(())
}
