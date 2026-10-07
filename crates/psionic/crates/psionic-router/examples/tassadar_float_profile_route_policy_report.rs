use psionic_router::{
    tassadar_float_profile_route_policy_report_path,
    write_tassadar_float_profile_route_policy_report,
};

fn main() {
    let path = tassadar_float_profile_route_policy_report_path();
    let report =
        write_tassadar_float_profile_route_policy_report(&path).expect("route policy report");
    println!(
        "wrote float profile route policy to {} ({})",
        path.display(),
        report.report_id
    );
}
