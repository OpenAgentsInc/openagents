//! System and utility Tauri commands

use crate::error::CommandResult;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn get_project_directory() -> Result<CommandResult<String>, String> {
    // Try to find git repository root first
    if let Ok(output) = std::process::Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !git_root.is_empty() {
                return Ok(CommandResult::success(git_root));
            }
        }
    }
    
    // Fall back to current directory if not in a git repo
    match std::env::current_dir() {
        Ok(path) => Ok(CommandResult::success(path.to_string_lossy().to_string())),
        Err(e) => Ok(CommandResult::error(e.to_string())),
    }
}