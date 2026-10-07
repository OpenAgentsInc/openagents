use psionic_eval::{
    tassadar_article_demo_benchmark_equivalence_gate_report_path,
    write_tassadar_article_demo_benchmark_equivalence_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_demo_benchmark_equivalence_gate_report(
        tassadar_article_demo_benchmark_equivalence_gate_report_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_demo_benchmark_equivalence_gate_report_path().display(),
        report.report_digest
    );
    Ok(())
}
