use psionic_runtime::{
    tassadar_article_fast_route_throughput_root_path,
    write_tassadar_article_fast_route_throughput_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle = write_tassadar_article_fast_route_throughput_bundle(
        tassadar_article_fast_route_throughput_root_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}
