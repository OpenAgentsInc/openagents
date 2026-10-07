use psionic_eval::{
    tassadar_article_abi_closure_report_path, write_tassadar_article_abi_closure_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_article_abi_closure_report_path();
    let report = write_tassadar_article_abi_closure_report(&report_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("article ABI closure report should serialize")
    );
    Ok(())
}
