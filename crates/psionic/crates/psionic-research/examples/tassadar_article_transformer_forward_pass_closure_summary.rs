use psionic_research::{
    tassadar_article_transformer_forward_pass_closure_summary_path,
    write_tassadar_article_transformer_forward_pass_closure_summary,
};

fn main() {
    let summary = write_tassadar_article_transformer_forward_pass_closure_summary(
        tassadar_article_transformer_forward_pass_closure_summary_path(),
    )
    .expect("write article Transformer forward-pass closure summary");
    println!("{}", summary.report_digest);
}
