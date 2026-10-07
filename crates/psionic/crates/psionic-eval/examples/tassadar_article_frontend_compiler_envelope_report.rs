use psionic_eval::{
    tassadar_article_frontend_compiler_envelope_report_path,
    write_tassadar_article_frontend_compiler_envelope_report,
};

fn main() {
    let path = tassadar_article_frontend_compiler_envelope_report_path();
    let report = write_tassadar_article_frontend_compiler_envelope_report(&path)
        .expect("write article frontend/compiler envelope report");
    println!("wrote {} ({})", path.display(), report.report_digest);
}
