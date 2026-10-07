use psionic_sandbox::{
    tassadar_post_article_plugin_packet_abi_and_rust_pdk_report_path,
    write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_packet_abi_and_rust_pdk_report_path();
    let report = write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_report(&output_path)?;
    println!(
        "wrote {} to {}",
        report.report_id,
        output_path.display()
    );
    Ok(())
}
