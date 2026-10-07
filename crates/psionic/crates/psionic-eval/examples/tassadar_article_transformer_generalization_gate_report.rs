use psionic_eval::{
    tassadar_article_transformer_generalization_gate_report_path,
    write_tassadar_article_transformer_generalization_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_transformer_generalization_gate_report_path();
    let report = write_tassadar_article_transformer_generalization_gate_report(&path)?;
    println!(
        "wrote {} with case_count={} and generalization_green={}",
        path.display(),
        report.case_count,
        report.generalization_green,
    );
    Ok(())
}
