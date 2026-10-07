use std::{env, path::PathBuf};

use psionic_train::{
    default_contract_fixture_path, verify_default_contract_fixture,
    write_compiled_agent_default_row_contract,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut output_path = None;
    let mut check_fixture = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => output_path = args.next().map(PathBuf::from),
            "--check-fixture" => check_fixture = true,
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    if check_fixture {
        let contract = verify_default_contract_fixture()?;
        println!(
            "{{\"verdict\":\"verified\",\"row_id\":\"{}\",\"model_artifact\":\"{}\",\"host_label\":\"{}\"}}",
            contract.row_id, contract.model_artifact, contract.host_label
        );
        return Ok(());
    }

    let output_path = output_path.unwrap_or_else(default_contract_fixture_path);
    let contract = write_compiled_agent_default_row_contract(output_path.as_path())?;
    println!(
        "wrote contract={} row_id={}",
        output_path.display(),
        contract.row_id
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -q -p psionic-train --bin compiled_agent_default_row_contract -- [--output <path>] [--check-fixture]"
    );
}
