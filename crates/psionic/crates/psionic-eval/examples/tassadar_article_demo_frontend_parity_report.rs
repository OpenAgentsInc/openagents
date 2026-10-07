use psionic_eval::{
    tassadar_article_demo_frontend_parity_report_path,
    write_tassadar_article_demo_frontend_parity_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_demo_frontend_parity_report(
        tassadar_article_demo_frontend_parity_report_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_demo_frontend_parity_report_path().display(),
        report.report_digest
    );
    Ok(())
}
