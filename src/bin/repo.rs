use std::fs;
use std::env;

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

    // Simulate some operations (e.g., cloning a repo, running tests, etc.)
    println!("Performing operations in the temporary directory...");

    // Cleanup: Remove the temporary directory
    fs::remove_dir_all(&temp_dir).expect("Failed to remove temporary directory");
    println!("Temporary directory removed.");
}
