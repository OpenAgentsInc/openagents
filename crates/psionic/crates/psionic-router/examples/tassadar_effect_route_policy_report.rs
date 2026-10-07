use psionic_router::{
    tassadar_effect_route_policy_report_path, write_tassadar_effect_route_policy_report,
};

fn main() {
    let path = tassadar_effect_route_policy_report_path();
    let report =
        write_tassadar_effect_route_policy_report(&path).expect("effect route policy report");
    println!("wrote {} with {} rows", path.display(), report.rows.len());
}
