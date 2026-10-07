use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_train::write_qwen_legal_ft_report;

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let run_id = required_flag(&args, "--run")?;
    let out_dir = optional_flag(&args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/legal/ft_runs"));
    let output = write_qwen_legal_ft_report(run_id.as_str(), out_dir)?;
    println!("{}", output.human_summary);
    println!("{}", serde_json::to_string_pretty(&output.report)?);
    Ok(())
}

fn required_flag(args: &[String], flag: &str) -> Result<String, io::Error> {
    optional_flag(args, flag).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen_legal_ft_report --run <run-id> [--out <dir>]",
        )
    })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
