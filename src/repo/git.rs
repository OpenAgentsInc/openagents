use anyhow::Result;
use git2::Repository;
use std::fs;
use std::path::PathBuf;

pub fn cleanup_temp_dir(temp_dir: &PathBuf) {
    if temp_dir.exists() {
        if let Err(e) = fs::remove_dir_all(temp_dir) {
            eprintln!("Warning: Failed to clean up temporary directory: {}", e);
        } else {
            println!("Temporary directory removed.");
        }
    }
}

pub fn clone_repository(url: &str, temp_dir: &PathBuf) -> Result<Repository> {
    println!("Cloning repository: {}", url);
    let repo = Repository::clone(url, temp_dir)
        .map_err(|e| anyhow::anyhow!("Failed to clone repository: {}", e))?;
    println!("Repository cloned successfully into: {:?}", temp_dir);
    Ok(repo)
}