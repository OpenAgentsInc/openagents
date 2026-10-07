use psionic_research::{
    tassadar_post_article_canonical_route_universal_substrate_gate_summary_path,
    write_tassadar_post_article_canonical_route_universal_substrate_gate_summary,
};

fn main() {
    let path = tassadar_post_article_canonical_route_universal_substrate_gate_summary_path();
    let summary = write_tassadar_post_article_canonical_route_universal_substrate_gate_summary(
        &path,
    )
    .expect("write post-article canonical-route universal-substrate gate summary");
    println!(
        "wrote post-article canonical-route universal-substrate gate summary to {} ({})",
        path.display(),
        summary.summary_digest,
    );
}
