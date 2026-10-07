use psionic_train::{
    psion_plugin_argument_construction_benchmark_bundle_path,
    write_psion_plugin_argument_construction_benchmark_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = psion_plugin_argument_construction_benchmark_bundle_path();
    let bundle = write_psion_plugin_argument_construction_benchmark_bundle(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    println!("package items: {}", bundle.package.items.len());
    println!("receipt metrics: {}", bundle.receipt.observed_metrics.len());
    Ok(())
}
