use psionic_research::{
    run_tassadar_hungarian_10x10_compiled_executor_bundle,
    TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(TASSADAR_HUNGARIAN_10X10_COMPILED_EXECUTOR_OUTPUT_DIR);
    let bundle = run_tassadar_hungarian_10x10_compiled_executor_bundle(output_dir.as_path())?;
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
