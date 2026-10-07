use std::{env, fs, path::PathBuf};

use psionic_eval::ParameterGolfDistributedThroughputReceipt;
use psionic_models::ParameterGolfReferenceModel;
use psionic_train::{
    benchmark_parameter_golf_runpod_8xh100_from_measurements,
    ParameterGolfRunPod8xH100Measurements, ParameterGolfTrainingHyperparameters,
};

fn main() {
    let mut args = env::args().skip(1);
    let run_root = PathBuf::from(args.next().expect("missing run-root path"));
    let measurements_path = PathBuf::from(args.next().expect("missing measurements json path"));
    let output_path = PathBuf::from(args.next().expect("missing output json path"));
    if args.next().is_some() {
        panic!("unexpected extra arguments");
    }

    let inventory_path = run_root.join("nvidia_smi_inventory.txt");
    let inventory_csv = fs::read_to_string(&inventory_path).expect("read RunPod inventory");
    let measurements: ParameterGolfRunPod8xH100Measurements =
        serde_json::from_slice(&fs::read(&measurements_path).expect("read measurements json"))
            .expect("decode measurements json");
    let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
        .expect("seed baseline Parameter Golf model");
    let receipt: ParameterGolfDistributedThroughputReceipt =
        benchmark_parameter_golf_runpod_8xh100_from_measurements(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            run_root
                .file_name()
                .and_then(|value| value.to_str())
                .expect("resolve run-root basename"),
            &inventory_csv,
            measurements,
        )
        .expect("build RunPod 8xH100 receipt");

    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&receipt).expect("encode receipt"),
    )
    .expect("write receipt json");
    println!(
        "wrote {} ({})",
        output_path.display(),
        receipt.receipt_digest
    );
}
