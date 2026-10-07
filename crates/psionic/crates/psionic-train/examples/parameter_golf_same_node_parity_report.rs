use std::{env, fs, path::PathBuf};

use psionic_train::{
    write_parameter_golf_same_node_parity_report, ParameterGolfSameNodeParityReport,
    ParameterGolfSingleH100TrainingReport, ParameterGolfTrainGptReferenceRunReceipt,
};

fn main() {
    let mut args = env::args().skip(1);
    let psionic_report_path = PathBuf::from(
        args.next()
            .expect("missing psionic single-H100 report path"),
    );
    let upstream_receipt_path = PathBuf::from(
        args.next()
            .expect("missing upstream reference receipt path"),
    );
    let output_path = PathBuf::from(args.next().expect("missing output path"));
    if args.next().is_some() {
        panic!("unexpected extra arguments");
    }

    let psionic_report: ParameterGolfSingleH100TrainingReport =
        serde_json::from_slice(&fs::read(&psionic_report_path).expect("read psionic report"))
            .expect("decode psionic report");
    let upstream_receipt: ParameterGolfTrainGptReferenceRunReceipt =
        serde_json::from_slice(&fs::read(&upstream_receipt_path).expect("read upstream receipt"))
            .expect("decode upstream receipt");
    let report: ParameterGolfSameNodeParityReport = write_parameter_golf_same_node_parity_report(
        &output_path,
        &psionic_report,
        &upstream_receipt,
    )
    .expect("write same-node parity report");
    println!("wrote {} ({})", output_path.display(), report.report_digest);
}
