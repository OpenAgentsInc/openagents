use psionic_eval::{
    tassadar_post_article_canonical_route_universal_substrate_gate_report_path,
    write_tassadar_post_article_canonical_route_universal_substrate_gate_report,
};

fn main() {
    let path = tassadar_post_article_canonical_route_universal_substrate_gate_report_path();
    let report = write_tassadar_post_article_canonical_route_universal_substrate_gate_report(&path)
        .expect("write post-article canonical-route universal-substrate gate report");
    println!(
        "wrote post-article canonical-route universal-substrate gate report to {} ({})",
        path.display(),
        report.report_digest,
    );
}
