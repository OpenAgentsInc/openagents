use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolve Codex agent configuration
pub async fn resolve_codex_config(
    codex_home: Option<PathBuf>,
) -> Result<(String, Vec<String>, HashMap<String, String>)> {
    let codex_acp_path = find_codex_acp().await.context(
        "codex-acp not found. Install it from https://github.com/zed-industries/codex-acp",
    )?;

    let mut env = HashMap::new();

    // Pass through authentication environment variables
    if let Some(ref path) = codex_home {
        env.insert("CODEX_HOME".to_string(), path.to_string_lossy().to_string());
    } else if let Ok(val) = std::env::var("CODEX_HOME") {
        env.insert("CODEX_HOME".to_string(), val);
    } else if let Ok(home) = std::env::var("HOME") {
        let default_codex_home = Path::new(&home).join(".codex");
        if default_codex_home.exists() {
            env.insert(
                "CODEX_HOME".to_string(),
                default_codex_home.to_string_lossy().to_string(),
            );
        }
    }

    // Pass through API keys
    if let Ok(val) = std::env::var("CODEX_API_KEY") {
        env.insert("CODEX_API_KEY".to_string(), val);
    }
    if let Ok(val) = std::env::var("OPENAI_API_KEY") {
        env.insert("OPENAI_API_KEY".to_string(), val);
    }

    Ok((codex_acp_path, vec![], env))
}

/// Resolve Gemini agent configuration
pub async fn resolve_gemini_config() -> Result<(String, Vec<String>, HashMap<String, String>)> {
    let gemini_path = find_gemini_cli()
        .await
        .context("gemini CLI not found. Install it via 'npm install -g @google/gemini-cli'")?;

    let mut env = HashMap::new();
    if let Ok(val) = std::env::var("GOOGLE_API_KEY") {
        env.insert("GOOGLE_API_KEY".to_string(), val);
    }

    // Gemini requires --experimental-acp for ACP mode
    Ok((gemini_path, vec!["--experimental-acp".to_string()], env))
}

const CODEX_ACP_REPO: &str = "zed-industries/codex-acp";

async fn find_codex_acp() -> Option<String> {
    if let Ok(path) = which::which("codex-acp") {
        return Some(path.to_string_lossy().to_string());
    }

    let home = std::env::var("HOME").ok()?;
    let common_paths = vec![
        format!("{}/.local/bin/codex-acp", home),
        format!("{}/.cargo/bin/codex-acp", home),
        "/usr/local/bin/codex-acp".to_string(),
        "/opt/homebrew/bin/codex-acp".to_string(),
    ];

    for path in common_paths {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    None
}

async fn find_gemini_cli() -> Option<String> {
    if let Ok(path) = which::which("gemini") {
        return Some(path.to_string_lossy().to_string());
    }

    let home = std::env::var("HOME").ok()?;
    let common_paths = vec![
        format!("{}/.npm-global/bin/gemini", home),
        "/usr/local/bin/gemini".to_string(),
        "/opt/homebrew/bin/gemini".to_string(),
    ];

    for path in common_paths {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    None
}
