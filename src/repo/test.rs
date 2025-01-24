use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

pub async fn run_cargo_tests(repo_path: &Path) -> Result<String> {
    println!("\nRunning cargo test...");

    let mut output = String::new();

    // Run cargo test and capture stdout
    let mut cmd = Command::new("cargo")
        .arg("test")
        .current_dir(repo_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Handle stdout
    if let Some(stdout) = cmd.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            println!("{}", line);
            output.push_str(&line);
            output.push('\n');
        }
    }

    // Handle stderr
    if let Some(stderr) = cmd.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("{}", line);
            output.push_str(&line);
            output.push('\n');
        }
    }

    // Wait for the command to complete
    let status = cmd.wait()?;

    if !status.success() {
        println!("\nNote: Some tests failed, but continuing with analysis...");
    }

    Ok(output)
}
