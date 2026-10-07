use psionic_research::{
    tassadar_article_trace_vocabulary_binding_summary_path,
    write_tassadar_article_trace_vocabulary_binding_summary,
};

fn main() {
    let report = write_tassadar_article_trace_vocabulary_binding_summary(
        tassadar_article_trace_vocabulary_binding_summary_path(),
    )
    .expect("write article trace vocabulary binding summary");
    println!("{}", report.report_digest);
}
