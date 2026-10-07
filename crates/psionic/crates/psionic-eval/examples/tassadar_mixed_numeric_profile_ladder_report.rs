use psionic_eval::{
    tassadar_mixed_numeric_profile_ladder_report_path,
    write_tassadar_mixed_numeric_profile_ladder_report,
};

fn main() {
    let path = tassadar_mixed_numeric_profile_ladder_report_path();
    let report = write_tassadar_mixed_numeric_profile_ladder_report(&path)
        .expect("mixed numeric report should write");
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report should serialize")
    );
}
