use psionic_eval::{
    tassadar_attention_primitive_mask_closure_report_path,
    write_tassadar_attention_primitive_mask_closure_report,
};

fn main() {
    let report = write_tassadar_attention_primitive_mask_closure_report(
        tassadar_attention_primitive_mask_closure_report_path(),
    )
    .expect("write attention primitive mask closure report");
    println!(
        "wrote {} with case_rows={} and attention_primitive_contract_green={}",
        report.report_id,
        report.case_rows.len(),
        report.attention_primitive_contract_green
    );
}
