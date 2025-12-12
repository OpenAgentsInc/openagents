//! Doctor command (health check)

use clap::Args;
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct DoctorArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Automatically repair issues
    #[arg(long)]
    repair: bool,
}

pub fn run(repo: &impl IssueRepository, args: DoctorArgs) -> Result<()> {
    if args.repair {
        let doctor_report = repo.doctor()?;
        let report = repo.repair(&doctor_report.problems)?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }

        println!("{}", "Repair Report:".bold().green());
        println!(
            "{} {} problems fixed",
            "Fixed:".bold(),
            report.repaired.len()
        );

        if !report.repaired.is_empty() {
            println!("\n{}", "Repaired:".bold());
            for problem_id in &report.repaired {
                println!("  {} {}", "✓".green(), problem_id);
            }
        }

        if !report.failed.is_empty() {
            println!("\n{}", "Failed:".red().bold());
            for (problem_id, error) in &report.failed {
                println!("  {} {} - {}", "✗".red(), problem_id, error);
            }
        }
    } else {
        let report = repo.doctor()?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }

        if report.problems.is_empty() {
            println!("{}", "✓ No problems found".green().bold());
            return Ok(());
        }

        println!(
            "{} {} problems found",
            "⚠".yellow().bold(),
            report.problems.len()
        );

        for problem in &report.problems {
            println!("\n{}", "─".repeat(60));
            println!("{} {:?}", "Category:".bold(), problem.category);
            if let Some(issue_id) = &problem.issue_id {
                println!("{} {}", "Issue:".bold(), issue_id);
            }
            println!("{} {}", "Problem:".bold(), problem.description);
        }

        println!("\n{}", "─".repeat(60));
        println!("\n{}", "Run with --repair to fix automatically".cyan());
    }

    Ok(())
}
