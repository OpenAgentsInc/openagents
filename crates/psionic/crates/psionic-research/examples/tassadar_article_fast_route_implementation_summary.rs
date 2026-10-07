use psionic_research::{
    tassadar_article_fast_route_implementation_summary_path,
    write_tassadar_article_fast_route_implementation_summary,
};

fn main() {
    let summary = write_tassadar_article_fast_route_implementation_summary(
        tassadar_article_fast_route_implementation_summary_path(),
    )
    .expect("write article fast-route implementation summary");
    println!(
        "wrote {} with fast_route_implementation_green={} and article_equivalence_green={}",
        summary.report_id, summary.fast_route_implementation_green, summary.article_equivalence_green
    );
}
