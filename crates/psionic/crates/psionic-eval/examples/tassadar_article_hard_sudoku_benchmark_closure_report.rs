use psionic_eval::{
    tassadar_article_hard_sudoku_benchmark_closure_report_path,
    write_tassadar_article_hard_sudoku_benchmark_closure_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_hard_sudoku_benchmark_closure_report(
        tassadar_article_hard_sudoku_benchmark_closure_report_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_hard_sudoku_benchmark_closure_report_path().display(),
        report.report_digest
    );
    Ok(())
}
