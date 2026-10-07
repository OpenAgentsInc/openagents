use psionic_research::{
    tassadar_article_frontend_compiler_envelope_summary_path,
    write_tassadar_article_frontend_compiler_envelope_summary,
};

fn main() {
    let path = tassadar_article_frontend_compiler_envelope_summary_path();
    let summary = write_tassadar_article_frontend_compiler_envelope_summary(&path)
        .expect("write article frontend/compiler envelope summary");
    println!("wrote {} ({})", path.display(), summary.report_digest);
}
