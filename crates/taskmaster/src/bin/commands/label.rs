//! Label command

use clap::{Args, Subcommand};
use colored::Colorize;
use taskmaster::{IssueRepository, Result};

#[derive(Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    command: LabelCommand,
}

#[derive(Subcommand)]
enum LabelCommand {
    /// Add labels to an issue
    Add {
        /// Issue ID
        id: String,
        /// Labels to add
        labels: Vec<String>,
        /// Actor name
        #[arg(long)]
        actor: Option<String>,
    },
    /// Remove labels from an issue
    Remove {
        /// Issue ID
        id: String,
        /// Labels to remove
        labels: Vec<String>,
        /// Actor name
        #[arg(long)]
        actor: Option<String>,
    },
    /// List all labels in the repository
    List,
}

pub fn run(repo: &impl IssueRepository, args: LabelArgs) -> Result<()> {
    match args.command {
        LabelCommand::Add { id, labels, actor } => {
            for label in &labels {
                repo.add_label(&id, label, actor.as_deref())?;
            }
            println!(
                "{} {} labels to {}",
                "Added".green().bold(),
                labels.join(", ").yellow(),
                id.cyan()
            );
        }
        LabelCommand::Remove { id, labels, actor } => {
            for label in &labels {
                repo.remove_label(&id, label, actor.as_deref())?;
            }
            println!(
                "{} {} labels from {}",
                "Removed".red().bold(),
                labels.join(", ").yellow(),
                id.cyan()
            );
        }
        LabelCommand::List => {
            let labels = repo.all_labels()?;
            println!("{} labels:", "All".bold());
            if labels.is_empty() {
                println!("  {}", "No labels found".yellow());
            } else {
                for label_count in labels {
                    println!("  {} ({})", label_count.label.yellow(), label_count.count);
                }
            }
        }
    }

    Ok(())
}
