use psionic_eval::{
    tassadar_quantization_truth_envelope_eval_report_path,
    write_tassadar_quantization_truth_envelope_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_quantization_truth_envelope_eval_report_path();
    let report = write_tassadar_quantization_truth_envelope_eval_report(&output_path)?;
    println!(
        "wrote quantization truth envelope eval report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
