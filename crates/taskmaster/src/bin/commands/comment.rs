//! Comment command

use clap::{Args, Subcommand};
use colored::Colorize;
use taskmaster::{CommentCreate, IssueRepository, Result};

#[derive(Args)]
pub struct CommentArgs {
    #[command(subcommand)]
    command: CommentCommand,
}

#[derive(Subcommand)]
enum CommentCommand {
    /// Add a comment to an issue
    Add {
        /// Issue ID
        id: String,
        /// Comment body
        body: String,
        /// Author name
        #[arg(long)]
        author: Option<String>,
    },
    /// List comments on an issue
    List {
        /// Issue ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

pub fn run(repo: &impl IssueRepository, args: CommentArgs) -> Result<()> {
    match args.command {
        CommentCommand::Add { id, body, author } => {
            let create = CommentCreate::new(author.unwrap_or_else(|| "unknown".to_string()), body);
            let comment = repo.add_comment(&id, create)?;
            println!("{} comment to {}", "Added".green().bold(), id.cyan());
            println!("{} {}", "Author:".bold(), comment.author);
            println!("{} {}", "Created:".bold(), comment.created_at);
        }
        CommentCommand::List { id, json } => {
            let comments = repo.comments(&id)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&comments)?);
                return Ok(());
            }

            if comments.is_empty() {
                println!("{}", "No comments found".yellow());
                return Ok(());
            }

            println!("{} {} comments:", id.cyan(), "Comments".bold());
            for comment in comments {
                println!("\n{}", "─".repeat(60));
                println!("{} {}", "ID:".bold(), comment.id);
                println!("{} {}", "Author:".bold(), comment.author);
                println!("{} {}", "Created:".bold(), comment.created_at);
                println!("\n{}", comment.body);
            }
            println!("{}", "─".repeat(60));
        }
    }

    Ok(())
}
