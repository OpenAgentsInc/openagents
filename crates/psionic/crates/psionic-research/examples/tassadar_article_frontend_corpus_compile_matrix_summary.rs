use psionic_research::{
    tassadar_article_frontend_corpus_compile_matrix_summary_path,
    write_tassadar_article_frontend_corpus_compile_matrix_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = write_tassadar_article_frontend_corpus_compile_matrix_summary(
        tassadar_article_frontend_corpus_compile_matrix_summary_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_frontend_corpus_compile_matrix_summary_path().display(),
        summary.report_digest
    );
    Ok(())
}
