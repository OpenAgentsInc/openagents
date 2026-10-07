use psionic_research::{
    tassadar_article_hard_sudoku_benchmark_closure_summary_path,
    write_tassadar_article_hard_sudoku_benchmark_closure_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = write_tassadar_article_hard_sudoku_benchmark_closure_summary(
        tassadar_article_hard_sudoku_benchmark_closure_summary_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_hard_sudoku_benchmark_closure_summary_path().display(),
        summary.report_digest
    );
    Ok(())
}
