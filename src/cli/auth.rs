//! Auth CLI commands

use anyhow::Result;
use clap::Subcommand;

use openagents::auth;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Check authentication status
    Status,

    /// Import credentials from OpenCode
    Import,
}

pub fn run(cmd: AuthCommands) -> Result<()> {
    match cmd {
        AuthCommands::Status => status(),
        AuthCommands::Import => import(),
    }
}

fn status() -> Result<()> {
    println!("Checking authentication status...\n");

    // Check OpenAgents auth
    println!(
        "OpenAgents auth ({}):",
        auth::openagents_auth_path().display()
    );
    match auth::check_openagents_auth() {
        auth::AuthStatus::Found { providers } => {
            println!(
                "  Found {} provider(s): {}",
                providers.len(),
                providers.join(", ")
            );
        }
        auth::AuthStatus::NotFound => {
            println!("  Not configured");
        }
        auth::AuthStatus::Error(e) => {
            println!("  Error: {}", e);
        }
        auth::AuthStatus::Copied { .. } => unreachable!(),
    }

    println!();

    // Check OpenCode auth
    println!("OpenCode auth ({}):", auth::opencode_auth_path().display());
    match auth::check_opencode_auth() {
        auth::AuthStatus::Found { providers } => {
            println!(
                "  Found {} provider(s): {}",
                providers.len(),
                providers.join(", ")
            );
        }
        auth::AuthStatus::NotFound => {
            println!("  Not found");
        }
        auth::AuthStatus::Error(e) => {
            println!("  Error: {}", e);
        }
        auth::AuthStatus::Copied { .. } => unreachable!(),
    }

    println!();

    // Check Anthropic specifically
    if auth::has_anthropic_auth() {
        println!("Anthropic auth: Ready");
    } else {
        println!("Anthropic auth: Not configured");
        println!("  Run 'openagents auth import' to import from OpenCode");
    }

    Ok(())
}

fn import() -> Result<()> {
    println!("Importing credentials from OpenCode...\n");

    // First check if OpenCode has auth
    let opencode_status = auth::check_opencode_auth();

    match opencode_status {
        auth::AuthStatus::NotFound => {
            println!(
                "OpenCode auth not found at {}",
                auth::opencode_auth_path().display()
            );
            println!("Please run 'opencode auth login' first to configure credentials.");
            return Ok(());
        }
        auth::AuthStatus::Error(e) => {
            println!("Error reading OpenCode auth: {}", e);
            return Ok(());
        }
        auth::AuthStatus::Found { ref providers } => {
            println!("Found OpenCode credentials for: {}", providers.join(", "));
        }
        auth::AuthStatus::Copied { .. } => unreachable!(),
    }

    // Copy the credentials
    match auth::copy_opencode_auth() {
        Ok(auth::AuthStatus::Copied { providers }) => {
            println!(
                "\nSuccessfully copied {} provider(s) to OpenAgents:",
                providers.len()
            );
            for provider in &providers {
                println!("  - {}", provider);
            }
            println!(
                "\nCredentials saved to: {}",
                auth::openagents_auth_path().display()
            );
        }
        Ok(auth::AuthStatus::NotFound) => {
            println!("No credentials found to copy.");
        }
        Ok(_) => unreachable!(),
        Err(e) => {
            println!("Error copying credentials: {}", e);
        }
    }

    Ok(())
}
