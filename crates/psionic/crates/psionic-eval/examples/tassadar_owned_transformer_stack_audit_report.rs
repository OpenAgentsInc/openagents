use psionic_eval::{
    tassadar_owned_transformer_stack_audit_report_path,
    write_tassadar_owned_transformer_stack_audit_report,
};

fn main() {
    let report = write_tassadar_owned_transformer_stack_audit_report(
        tassadar_owned_transformer_stack_audit_report_path(),
    )
    .expect("write owned Transformer stack audit report");
    println!("{}", report.report_digest);
}
