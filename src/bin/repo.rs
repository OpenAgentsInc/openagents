use std::fs;
use std::env;
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

    // Cleanup: Remove the temporary directory
    fs::remove_dir_all(&temp_dir).expect("Failed to remove temporary directory");
    println!("Temporary directory removed.");
}
