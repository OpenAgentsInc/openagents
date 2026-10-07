use psionic_runtime::{
    tassadar_million_step_benchmark_root_path, write_tassadar_million_step_decode_benchmark_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle = write_tassadar_million_step_decode_benchmark_bundle(
        tassadar_million_step_benchmark_root_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
