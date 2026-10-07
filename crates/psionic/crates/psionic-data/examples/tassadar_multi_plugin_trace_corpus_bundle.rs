use psionic_data::{
    tassadar_multi_plugin_trace_corpus_bundle_path, write_tassadar_multi_plugin_trace_corpus_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_multi_plugin_trace_corpus_bundle_path();
    let bundle = write_tassadar_multi_plugin_trace_corpus_bundle(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
