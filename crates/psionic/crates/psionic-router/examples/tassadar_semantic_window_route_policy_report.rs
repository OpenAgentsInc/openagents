use psionic_router::{
    tassadar_semantic_window_route_policy_report_path,
    write_tassadar_semantic_window_route_policy_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_semantic_window_route_policy_report(
        tassadar_semantic_window_route_policy_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
