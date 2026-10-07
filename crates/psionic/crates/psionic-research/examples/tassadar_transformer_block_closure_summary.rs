use psionic_research::{
    tassadar_transformer_block_closure_summary_path,
    write_tassadar_transformer_block_closure_summary,
};

fn main() {
    let report = write_tassadar_transformer_block_closure_summary(
        tassadar_transformer_block_closure_summary_path(),
    )
    .expect("write transformer block closure summary");
    println!(
        "wrote {} with case_count={} and transformer_block_contract_green={}",
        report.report_id, report.case_count, report.transformer_block_contract_green
    );
}
