use psionic_research::{
    tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary_path,
    write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary_path();
    let summary = write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_summary(&output_path)?;
    println!(
        "wrote {} to {}",
        summary.report_id,
        output_path.display()
    );
    Ok(())
}
