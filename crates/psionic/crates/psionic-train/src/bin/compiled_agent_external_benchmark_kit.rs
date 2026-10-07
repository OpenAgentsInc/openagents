use psionic_train::{
    canonical_compiled_agent_external_benchmark_kit,
    canonical_compiled_agent_external_benchmark_run,
    compiled_agent_external_benchmark_kit_fixture_path,
    compiled_agent_external_benchmark_run_fixture_path,
    write_compiled_agent_external_benchmark_kit,
    write_compiled_agent_external_benchmark_run,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut print_run = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--run" => {
                print_run = true;
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    if print_run {
        let run = canonical_compiled_agent_external_benchmark_run()?;
        println!("{}", serde_json::to_string_pretty(&run)?);
        return Ok(());
    }

    let kit_path = compiled_agent_external_benchmark_kit_fixture_path();
    let run_path = compiled_agent_external_benchmark_run_fixture_path();
    let kit = write_compiled_agent_external_benchmark_kit(&kit_path)?;
    let run = write_compiled_agent_external_benchmark_run(&run_path)?;
    let _ = canonical_compiled_agent_external_benchmark_kit()?;
    let _ = canonical_compiled_agent_external_benchmark_run()?;
    println!(
        "wrote compiled-agent external benchmark kit={} digest={}",
        kit_path.display(),
        kit.contract_digest
    );
    println!(
        "wrote compiled-agent external benchmark run={} digest={}",
        run_path.display(),
        run.run_digest
    );
    Ok(())
}
