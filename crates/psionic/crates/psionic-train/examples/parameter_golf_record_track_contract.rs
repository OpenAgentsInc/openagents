use psionic_train::{
    parameter_golf_record_track_contract_report_path,
    write_parameter_golf_record_track_contract_report,
};

fn main() {
    let path = parameter_golf_record_track_contract_report_path();
    let report = write_parameter_golf_record_track_contract_report(&path)
        .expect("write Parameter Golf record-track contract");
    println!(
        "wrote {} with digest {}",
        path.display(),
        report.report_digest
    );
}
