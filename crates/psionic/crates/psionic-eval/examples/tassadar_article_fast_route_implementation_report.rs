use psionic_eval::{
    tassadar_article_fast_route_implementation_report_path,
    write_tassadar_article_fast_route_implementation_report,
};

fn main() {
    let report = write_tassadar_article_fast_route_implementation_report(
        tassadar_article_fast_route_implementation_report_path(),
    )
    .expect("write article fast-route implementation report");
    println!(
        "wrote {} with fast_route_implementation_green={} and article_equivalence_green={}",
        report.report_id, report.fast_route_implementation_green, report.article_equivalence_green
    );
}
