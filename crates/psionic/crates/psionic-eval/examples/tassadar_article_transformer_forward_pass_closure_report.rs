use psionic_eval::{
    tassadar_article_transformer_forward_pass_closure_report_path,
    write_tassadar_article_transformer_forward_pass_closure_report,
};

fn main() {
    let report = write_tassadar_article_transformer_forward_pass_closure_report(
        tassadar_article_transformer_forward_pass_closure_report_path(),
    )
    .expect("write article Transformer forward-pass closure report");
    println!("{}", report.report_digest);
}
