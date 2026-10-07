use psionic_research::{
    tassadar_attention_primitive_mask_closure_summary_path,
    write_tassadar_attention_primitive_mask_closure_summary,
};

fn main() {
    let report = write_tassadar_attention_primitive_mask_closure_summary(
        tassadar_attention_primitive_mask_closure_summary_path(),
    )
    .expect("write attention primitive mask closure summary");
    println!(
        "wrote {} with case_count={} and attention_primitive_contract_green={}",
        report.report_id,
        report.case_count,
        report.attention_primitive_contract_green
    );
}
