use psionic_eval::{
    tassadar_article_fast_route_throughput_floor_report_path,
    write_tassadar_article_fast_route_throughput_floor_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_fast_route_throughput_floor_report_path();
    let report = write_tassadar_article_fast_route_throughput_floor_report(&path)?;
    println!(
        "wrote {} ({})",
        path.display(),
        serde_json::to_string_pretty(&report)?
    );
    Ok(())
}
