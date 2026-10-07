use psionic_research::{
    tassadar_owned_transformer_stack_audit_summary_path,
    write_tassadar_owned_transformer_stack_audit_summary,
};

fn main() {
    let report = write_tassadar_owned_transformer_stack_audit_summary(
        tassadar_owned_transformer_stack_audit_summary_path(),
    )
    .expect("write owned Transformer stack audit summary");
    println!("{}", report.report_digest);
}
