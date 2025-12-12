//! Dependency command

use clap::{Args, Subcommand};
use colored::Colorize;
use taskmaster::{DependencyType, IssueRepository, Result};

#[derive(Args)]
pub struct DepArgs {
    #[command(subcommand)]
    command: DepCommand,
}

#[derive(Subcommand)]
enum DepCommand {
    /// Add a dependency
    Add {
        /// Issue ID
        id: String,
        /// Target issue ID
        target: String,
        /// Dependency type (blocks, related, parent_child, discovered_from)
        #[arg(short, long, default_value = "blocks")]
        dep_type: String,
        /// Actor name
        #[arg(long)]
        actor: Option<String>,
    },
    /// Remove a dependency
    Remove {
        /// Issue ID
        id: String,
        /// Target issue ID
        target: String,
        /// Dependency type (blocks, related, parent_child, discovered_from)
        #[arg(short, long, default_value = "blocks")]
        dep_type: String,
        /// Actor name
        #[arg(long)]
        actor: Option<String>,
    },
    /// List dependencies
    List {
        /// Issue ID
        id: String,
        /// Show blockers (issues that block this one)
        #[arg(long)]
        blockers: bool,
        /// Show blocked (issues blocked by this one)
        #[arg(long)]
        blocked: bool,
    },
    /// Show dependency tree
    Tree {
        /// Issue ID
        id: String,
        /// Maximum depth (default: unlimited)
        #[arg(long)]
        max_depth: Option<usize>,
    },
}

pub fn run(repo: &impl IssueRepository, args: DepArgs) -> Result<()> {
    match args.command {
        DepCommand::Add {
            id,
            target,
            dep_type,
            actor: _,
        } => {
            let dtype: DependencyType = dep_type.parse().map_err(|e| {
                taskmaster::TaskmasterError::validation(format!("Invalid dependency type: {}", e))
            })?;
            use chrono::Utc;
            let dep = taskmaster::Dependency {
                issue_id: id.clone(),
                depends_on_id: target.clone(),
                dep_type: dtype,
                created_at: Utc::now(),
            };
            repo.add_dependency(&id, dep)?;
            println!(
                "{} {} {} {}",
                "Added:".green().bold(),
                id.cyan(),
                dtype,
                target.cyan()
            );
        }
        DepCommand::Remove {
            id,
            target,
            dep_type: _,
            actor: _,
        } => {
            repo.remove_dependency(&id, &target)?;
            println!(
                "{} {} -> {}",
                "Removed:".red().bold(),
                id.cyan(),
                target.cyan()
            );
        }
        DepCommand::List {
            id,
            blockers,
            blocked,
        } => {
            let issue = repo.get(&id)?;

            if blockers {
                let blockers = repo.blockers(&id)?;
                println!("{} {} blockers:", "Issues blocking".bold(), id.cyan());
                for blocker in blockers {
                    println!("  {} {}", "←".red(), blocker.id);
                }
            } else if blocked {
                let blocked = repo.blocked_by(&id)?;
                println!("{} {} blocks:", "Issues".bold(), id.cyan());
                for dep in blocked {
                    println!("  {} {}", "→".cyan(), dep.id);
                }
            } else {
                // Show all dependencies
                println!("{} {} dependencies:", issue.id.cyan(), "All".bold());
                if issue.deps.is_empty() {
                    println!("  {}", "No dependencies".yellow());
                } else {
                    for dep in &issue.deps {
                        println!("  {} {} ({})", "→".cyan(), dep.id, dep.dep_type);
                    }
                }
            }
        }
        DepCommand::Tree { id, max_depth } => {
            let tree = repo.dependency_tree(&id, max_depth.unwrap_or(100) as u32)?;
            println!("{} dependency tree:", id.cyan());
            print_tree(&tree.root, 0);
        }
    }

    Ok(())
}

fn print_tree(node: &taskmaster::DependencyTreeNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let dep_info = if let Some(dtype) = &node.dep_type {
        format!(" ({})", dtype)
    } else {
        String::new()
    };
    println!(
        "{}{} {}{}",
        indent,
        "→".cyan(),
        node.id,
        dep_info
    );
    for child in &node.children {
        print_tree(child, depth + 1);
    }
}
