use psionic_eval::{
    tassadar_post_article_universality_portability_minimality_matrix_report_path,
    write_tassadar_post_article_universality_portability_minimality_matrix_report,
};

fn main() {
    let path = tassadar_post_article_universality_portability_minimality_matrix_report_path();
    let report =
        write_tassadar_post_article_universality_portability_minimality_matrix_report(&path)
            .expect("write post-article universality portability/minimality matrix report");
    println!(
        "wrote post-article universality portability/minimality matrix report to {} ({})",
        path.display(),
        report.report_digest,
    );
}
