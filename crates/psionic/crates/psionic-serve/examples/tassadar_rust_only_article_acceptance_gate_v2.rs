use psionic_serve::{
    tassadar_rust_only_article_acceptance_gate_v2_report_path,
    write_tassadar_rust_only_article_acceptance_gate_v2_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_rust_only_article_acceptance_gate_v2_report_path();
    let report = write_tassadar_rust_only_article_acceptance_gate_v2_report(&report_path)?;
    println!("wrote {} ({})", report_path.display(), report.report_digest);
    Ok(())
}
