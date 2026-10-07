use std::{env, fs, path::PathBuf};

use psionic_eval::ParameterGolfDistributedThroughputReceipt;
use psionic_models::ParameterGolfReferenceModel;
use psionic_runtime::{ClusterExecutionCapabilityProfile, DeviceDescriptor};
use psionic_train::{
    benchmark_parameter_golf_distributed_8xh100, ParameterGolfDistributed8xH100Config,
    ParameterGolfTrainingHyperparameters,
};

fn main() {
    let mut args = env::args().skip(1);
    let devices_path = PathBuf::from(args.next().expect("missing devices json path"));
    let capability_profile_path =
        PathBuf::from(args.next().expect("missing capability-profile json path"));
    let config_path = PathBuf::from(args.next().expect("missing config json path"));
    let output_path = PathBuf::from(args.next().expect("missing output json path"));
    if args.next().is_some() {
        panic!("unexpected extra arguments");
    }

    let devices: Vec<DeviceDescriptor> =
        serde_json::from_slice(&fs::read(&devices_path).expect("read devices json"))
            .expect("decode devices json");
    let capability_profile: ClusterExecutionCapabilityProfile = serde_json::from_slice(
        &fs::read(&capability_profile_path).expect("read capability-profile json"),
    )
    .expect("decode capability-profile json");
    let config: ParameterGolfDistributed8xH100Config =
        serde_json::from_slice(&fs::read(&config_path).expect("read config json"))
            .expect("decode config json");

    let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())
        .expect("seed baseline Parameter Golf model");
    let receipt: ParameterGolfDistributedThroughputReceipt =
        benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            devices.as_slice(),
            &capability_profile,
            &config,
        )
        .expect("build distributed 8xH100 receipt");

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
