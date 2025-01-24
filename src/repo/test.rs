use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::path::PathBuf;

pub async fn run_cargo_tests(temp_dir: &PathBuf) -> Result<String> {
    println!("Running cargo test in the cloned repository...");
    let mut test_output = String::new();
    let mut child = Command::new("cargo")
        .current_dir(temp_dir)
        .arg("test")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to start cargo test: {}", e))?;

    // Stream stdout in real-time and capture it
    let stdout = child.stdout.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
    let stderr = child.stderr.take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
    
    // Spawn a thread to handle stdout
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut output = String::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
        }
        output
    });

    // Spawn a thread to handle stderr
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut output = String::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
        }
        output
    });

    // Wait for the command to complete
    let status = child.wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for cargo test: {}", e))?;

    // Wait for output threads to finish and collect their output
    let stdout_output = stdout_thread.join()
        .map_err(|_| anyhow::anyhow!("Failed to join stdout thread"))?;
    let stderr_output = stderr_thread.join()
        .map_err(|_| anyhow::anyhow!("Failed to join stderr thread"))?;
    test_output.push_str(&stdout_output);
    test_output.push_str(&stderr_output);

    // Print final test status
    if status.success() {
        println!("\nTests completed successfully!");
    } else {
        println!("\nTests failed!");
    }

    Ok(test_output)
}