//! Initialize database

use taskmaster::{IssueRepository, Result};

pub fn run(repo: &impl IssueRepository) -> Result<()> {
    repo.init()?;
    println!("Database initialized successfully");
    Ok(())
}
