use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let report = psionic_train::run_qwen_legal_rl_rollout_cli(&args)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
