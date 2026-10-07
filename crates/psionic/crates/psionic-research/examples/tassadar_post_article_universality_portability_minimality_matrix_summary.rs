use psionic_research::{
    tassadar_post_article_universality_portability_minimality_matrix_summary_path,
    write_tassadar_post_article_universality_portability_minimality_matrix_summary,
};

fn main() {
    let path = tassadar_post_article_universality_portability_minimality_matrix_summary_path();
    let summary =
        write_tassadar_post_article_universality_portability_minimality_matrix_summary(&path)
            .expect("write post-article universality portability/minimality matrix summary");
    println!(
        "wrote post-article universality portability/minimality matrix summary to {} ({})",
        path.display(),
        summary.summary_digest,
    );
}
