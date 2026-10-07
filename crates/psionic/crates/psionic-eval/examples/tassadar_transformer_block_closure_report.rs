use psionic_eval::{
    tassadar_transformer_block_closure_report_path, write_tassadar_transformer_block_closure_report,
};

fn main() {
    let report = write_tassadar_transformer_block_closure_report(
        tassadar_transformer_block_closure_report_path(),
    )
    .expect("write transformer block closure report");
    println!(
        "wrote {} with case_rows={} and transformer_block_contract_green={}",
        report.report_id,
        report.case_rows.len(),
        report.transformer_block_contract_green
    );
}
