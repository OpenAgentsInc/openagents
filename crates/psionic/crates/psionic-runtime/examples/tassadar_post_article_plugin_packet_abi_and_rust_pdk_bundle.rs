use psionic_runtime::{
    tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle_path,
    write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle_path();
    let bundle = write_tassadar_post_article_plugin_packet_abi_and_rust_pdk_bundle(&output_path)?;
    println!(
        "wrote {} to {}",
        bundle.bundle_id,
        output_path.display()
    );
    Ok(())
}
