use psionic_eval::{
    tassadar_article_trace_vocabulary_binding_report_path,
    write_tassadar_article_trace_vocabulary_binding_report,
};

fn main() {
    let report = write_tassadar_article_trace_vocabulary_binding_report(
        tassadar_article_trace_vocabulary_binding_report_path(),
    )
    .expect("write article trace vocabulary binding report");
    println!("{}", report.report_digest);
}
