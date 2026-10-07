use psionic_train::{
    parameter_golf_record_folder_compatibility_report_path,
    write_parameter_golf_record_folder_compatibility_report,
};

fn main() {
    let path = parameter_golf_record_folder_compatibility_report_path();
    let report = write_parameter_golf_record_folder_compatibility_report(&path)
        .expect("write Parameter Golf record-folder compatibility report");
    println!(
        "wrote {} with digest {}",
        path.display(),
        report.report_digest
    );
}
