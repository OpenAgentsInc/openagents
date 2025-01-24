use std::fs;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use git2::Repository;
use openagents::repomap::generate_repo_map;

fn main() {
    // Define the temporary directory path
    let temp_dir = env::temp_dir().join("rust_app_temp");

    // Create the temporary directory if it doesn't exist
    if !temp_dir.exists() {
        fs::create_dir_all(&temp_dir).expect("Failed to create temporary directory");
        println!("Temporary directory created at: {:?}", temp_dir);
    } else {
        println!("Temporary directory already exists at: {:?}", temp_dir);
    }

    // Clone the OpenAgentsInc/openagents repository into the temporary directory
    let repo_url = "https://github.com/OpenAgentsInc/openagents";
    println!("Cloning repository: {}", repo_url);
    let _repo = match Repository::clone(repo_url, &temp_dir) {
        Ok(repo) => repo,
        Err(e) => panic!("Failed to clone repository: {}", e),
    };
    println!("Repository cloned successfully into: {:?}", temp_dir);

    // Generate and print the repository map
    let map = generate_repo_map(&temp_dir);
    println!("Repository Map:\n{}", map);

    // Run cargo test in the cloned repository with streaming output
    println!("Running cargo test in the cloned repository...");
    let mut child = Command::new("cargo")
        .current_dir(&temp_dir)
        .arg("test")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start cargo test");

    // Stream stdout in real-time
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    // Spawn a thread to handle stdout
    let stdout_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line);
            }
        }
    });

    // Spawn a thread to handle stderr
    let stderr_thread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("{}", line);
            }
        }
    });

    // Wait for the command to complete
    let status = child.wait().expect("Failed to wait for cargo test");

    // Wait for output threads to finish
    stdout_thread.join().expect("Failed to join stdout thread");
    stderr_thread.join().expect("Failed to join stderr thread");

    // Print final status
    if status.success() {
        println!("\nTests completed successfully!");
    } else {
        println!("\nTests failed!");
    }

    // Cleanup: Remove the temporary directory
    fs::remove_dir_all(&temp_dir).expect("Failed to remove temporary directory");
    println!("Temporary directory removed.");
}