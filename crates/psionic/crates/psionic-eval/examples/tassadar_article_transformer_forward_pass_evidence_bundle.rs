use psionic_eval::{
    tassadar_article_transformer_forward_pass_evidence_bundle_path,
    write_tassadar_article_transformer_forward_pass_evidence_bundle,
};

fn main() {
    let bundle = write_tassadar_article_transformer_forward_pass_evidence_bundle(
        tassadar_article_transformer_forward_pass_evidence_bundle_path(),
    )
    .expect("write article Transformer forward-pass evidence bundle");
    println!("{}", bundle.bundle_digest);
}
