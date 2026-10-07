use psionic_research::{
    tassadar_post_article_canonical_computational_model_statement_summary_path,
    write_tassadar_post_article_canonical_computational_model_statement_summary,
};

fn main() {
    let path = tassadar_post_article_canonical_computational_model_statement_summary_path();
    let summary =
        write_tassadar_post_article_canonical_computational_model_statement_summary(&path)
            .expect("write post-article canonical computational-model statement summary");
    println!(
        "wrote post-article canonical computational-model statement summary to {} ({})",
        path.display(),
        summary.summary_digest
    );
}
