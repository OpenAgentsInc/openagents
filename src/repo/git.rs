use anyhow::{anyhow, Result};
use git2::{Repository, Signature};
use std::fs;
use std::path::PathBuf;
use tracing::debug;

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
        .map_err(|e| anyhow!("Failed to clone repository: {}", e))?;
    println!("Repository cloned successfully into: {:?}", temp_dir);
    Ok(repo)
}

pub fn commit_changes(repo: &Repository, files: &[String], message: &str) -> Result<()> {
    debug!("Committing changes to files: {:?}", files);
    
    let mut index = repo.index()?;
    
    // Add all modified files to the index
    for file in files {
        debug!("Adding file to index: {}", file);
        index.add_path(std::path::Path::new(file))?;
    }
    
    index.write()?;
    
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    
    let head = repo.head()?;
    let parent_commit = repo.find_commit(head.target().ok_or_else(|| anyhow!("No HEAD target"))?)?;
    
    let signature = Signature::now("OpenAgents Solver", "solver@openagents.com")?;
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent_commit],
    )?;
    
    debug!("Changes committed successfully");
    Ok(())
}

pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    debug!("Checking out branch: {}", branch_name);
    
    let (object, reference) = repo.revparse_ext(branch_name)?;
    
    repo.checkout_tree(&object, None)?;
    
    match reference {
        Some(gref) => repo.set_head(gref.name().unwrap()),
        None => repo.set_head(&format!("refs/heads/{}", branch_name)),
    }?;
    
    debug!("Successfully checked out branch: {}", branch_name);
    Ok(())
}