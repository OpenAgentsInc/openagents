use std::{env, path::PathBuf};

use psionic_train::{
    load_parameter_golf_runpod_8xh100_measurements,
    write_parameter_golf_distributed_8xh100_bringup_report,
    ParameterGolfDistributed8xH100BringupConfig,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut output_path = PathBuf::from("/tmp/parameter_golf_distributed_8xh100_bringup.json");
    let mut measurements_path = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => {
                output_path = PathBuf::from(args.next().ok_or("--output requires a path")?);
            }
            "--measurements" => {
                measurements_path = Some(PathBuf::from(
                    args.next().ok_or("--measurements requires a path")?,
                ));
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let config = ParameterGolfDistributed8xH100BringupConfig::challenge_defaults();
    let measurements = measurements_path
        .as_ref()
        .map(load_parameter_golf_runpod_8xh100_measurements)
        .transpose()?;
    let report = write_parameter_golf_distributed_8xh100_bringup_report(
        &output_path,
        &config,
        measurements.as_ref(),
    )?;
    println!(
        "wrote {} with disposition {:?} ready_to_attempt={}",
        output_path.display(),
        report.disposition,
        report.ready_to_attempt(),
    );
    println!(
        "matching_h100_device_count={} machine_contract_satisfied={}",
        report.matching_h100_device_count, report.machine_contract_satisfied
    );
    if let Some(receipt) = report.distributed_receipt.as_ref() {
        println!(
            "distributed_receipt disposition={:?} measured={}",
            receipt.disposition,
            receipt.disposition == psionic_eval::ParameterGolfDistributedLaneDisposition::Measured
        );
    }
    if let Some(refusal) = report.refusal {
        println!(
            "refusal subject={:?} detail={}",
            refusal.subject, refusal.detail
        );
    }
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage: parameter_golf_distributed_8xh100_bringup [--output <path>] [--measurements <path>]"
    );
}
