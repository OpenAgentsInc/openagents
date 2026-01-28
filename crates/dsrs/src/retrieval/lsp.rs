//! LSP-based retrieval backend.
//!
//! Uses Language Server Protocol for symbol-aware code navigation:
//! - Go to definition
//! - Find references
//! - Find implementations
//! - Document symbols

use super::{RepoIndex, RetrievalConfig, RetrievalResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// LSP-based retrieval backend.
pub struct LspIndex {
    /// Root path of the repository.
    repo_path: PathBuf,

    /// Language server command (e.g., "rust-analyzer", "typescript-language-server").
    server_command: Option<String>,

    /// Cached symbol index.
    #[allow(dead_code)]
    symbol_cache: HashMap<String, Vec<SymbolInfo>>,
}

/// Information about a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,

    /// Symbol kind (function, struct, trait, etc.).
    pub kind: SymbolKind,

    /// File path.
    pub path: String,

    /// Line number.
    pub line: usize,

    /// Column number.
    pub column: usize,

    /// Container (parent symbol, if any).
    pub container: Option<String>,
}

/// Symbol kinds supported by LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Interface,
    Trait,
    Enum,
    Module,
    Constant,
    Variable,
    Field,
    Property,
    Type,
    Unknown,
}

impl From<u32> for SymbolKind {
    fn from(value: u32) -> Self {
        match value {
            1 => SymbolKind::Module,
            2 => SymbolKind::Module, // Namespace
            3 => SymbolKind::Module, // Package
            4 => SymbolKind::Class,
            5 => SymbolKind::Method,
            6 => SymbolKind::Property,
            7 => SymbolKind::Field,
            8 => SymbolKind::Function, // Constructor
            9 => SymbolKind::Enum,
            10 => SymbolKind::Interface,
            11 => SymbolKind::Function,
            12 => SymbolKind::Variable,
            13 => SymbolKind::Constant,
            14 => SymbolKind::Constant, // String
            15 => SymbolKind::Constant, // Number
            16 => SymbolKind::Constant, // Boolean
            17 => SymbolKind::Variable, // Array
            18 => SymbolKind::Variable, // Object
            22 => SymbolKind::Struct,
            23 => SymbolKind::Constant, // Event
            24 => SymbolKind::Method,   // Operator
            25 => SymbolKind::Type,     // TypeParameter
            _ => SymbolKind::Unknown,
        }
    }
}

impl LspIndex {
    /// Create a new LSP index for a repository.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            server_command: None,
            symbol_cache: HashMap::new(),
        }
    }

    /// Set the language server command.
    pub fn with_server(mut self, command: impl Into<String>) -> Self {
        self.server_command = Some(command.into());
        self
    }

    /// Query using ctags as a fallback when LSP is unavailable.
    fn query_ctags(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        // Try universal-ctags
        let output = Command::new("ctags")
            .args(["-R", "-x", "--output-format=json", "--fields=+n"])
            .arg(&self.repo_path)
            .output()
            .context("Failed to execute ctags")?;

        if !output.status.success() {
            // Fall back to basic grep for symbol-like patterns
            return self.query_symbol_grep(query, config);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let name = json["name"].as_str().unwrap_or("");
                if name.to_lowercase().contains(&query.to_lowercase()) {
                    let path = json["path"].as_str().unwrap_or("");
                    let line_num = json["line"].as_u64().unwrap_or(1) as usize;
                    let kind = json["kind"].as_str().unwrap_or("unknown");

                    results.push(
                        RetrievalResult::new(path, line_num, line_num, name)
                            .with_lane("lsp")
                            .with_score(if name == query { 1.0 } else { 0.8 })
                            .with_metadata("kind", kind.to_string())
                            .with_metadata("symbol", name.to_string()),
                    );
                }
            }
        }

        results.truncate(config.k);
        Ok(results)
    }

    /// Grep for symbol-like patterns (function/struct/class definitions).
    fn query_symbol_grep(
        &self,
        query: &str,
        config: &RetrievalConfig,
    ) -> Result<Vec<RetrievalResult>> {
        // Build regex patterns for common symbol definitions
        let patterns = vec![
            format!(r"fn\s+{}\s*[<(]", regex::escape(query)), // Rust function
            format!(r"struct\s+{}\s*[<{{]", regex::escape(query)), // Rust struct
            format!(r"trait\s+{}\s*[<{{]", regex::escape(query)), // Rust trait
            format!(r"impl\s+.*{}", regex::escape(query)),    // Rust impl
            format!(r"function\s+{}\s*\(", regex::escape(query)), // JS function
            format!(r"class\s+{}\s*[<{{]", regex::escape(query)), // Class
            format!(r"interface\s+{}\s*[<{{]", regex::escape(query)), // Interface
            format!(r"def\s+{}\s*\(", regex::escape(query)),  // Python function
            format!(r"type\s+{}\s*=", regex::escape(query)),  // Type alias
        ];

        let pattern = patterns.join("|");
        let output = Command::new("rg")
            .args(["-n", "-e", &pattern])
            .arg(&self.repo_path)
            .output()
            .context("Failed to execute ripgrep for symbols")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() >= 3
                && let Ok(line_num) = parts[1].parse::<usize>()
            {
                let content = parts[2].trim();
                let kind = self.infer_symbol_kind(content);

                results.push(
                    RetrievalResult::new(parts[0], line_num, line_num, content)
                        .with_lane("lsp")
                        .with_score(0.9)
                        .with_metadata("kind", format!("{:?}", kind))
                        .with_metadata("symbol", query.to_string()),
                );
            }
        }

        results.truncate(config.k);
        Ok(results)
    }

    /// Infer symbol kind from definition line.
    fn infer_symbol_kind(&self, line: &str) -> SymbolKind {
        let trimmed = line.trim();
        if trimmed.starts_with("fn ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("def ")
        {
            SymbolKind::Function
        } else if trimmed.starts_with("struct ") {
            SymbolKind::Struct
        } else if trimmed.starts_with("class ") {
            SymbolKind::Class
        } else if trimmed.starts_with("trait ") || trimmed.starts_with("interface ") {
            SymbolKind::Trait
        } else if trimmed.starts_with("enum ") {
            SymbolKind::Enum
        } else if trimmed.starts_with("type ") {
            SymbolKind::Type
        } else if trimmed.starts_with("const ") {
            SymbolKind::Constant
        } else if trimmed.starts_with("impl ") {
            SymbolKind::Method
        } else {
            SymbolKind::Unknown
        }
    }
}

#[async_trait]
impl RepoIndex for LspIndex {
    async fn query(&self, query: &str, config: &RetrievalConfig) -> Result<Vec<RetrievalResult>> {
        // For now, use ctags/grep fallback
        // Full LSP integration would require running a language server
        // and communicating via JSON-RPC
        self.query_ctags(query, config)
    }

    fn lane_name(&self) -> &str {
        "lsp"
    }

    fn supports_semantic(&self) -> bool {
        // LSP has some semantic understanding via types
        true
    }

    async fn build_index(&self, repo_path: &PathBuf) -> Result<()> {
        // Generate ctags index
        let status = Command::new("ctags")
            .args(["-R", "--fields=+n", "-f", ".tags"])
            .current_dir(repo_path)
            .status()
            .context("Failed to run ctags")?;

        if !status.success() {
            anyhow::bail!("ctags indexing failed");
        }

        Ok(())
    }

    async fn is_available(&self) -> bool {
        // Check for ctags or rg
        Command::new("ctags")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            || Command::new("rg")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_lsp_availability() {
        let index = LspIndex::new(env::current_dir().unwrap());
        let available = index.is_available().await;
        println!("LSP/ctags available: {}", available);
    }

    #[test]
    fn test_symbol_kind_inference() {
        let index = LspIndex::new(".");

        assert_eq!(index.infer_symbol_kind("fn main() {"), SymbolKind::Function);
        assert_eq!(index.infer_symbol_kind("struct Foo {"), SymbolKind::Struct);
        assert_eq!(index.infer_symbol_kind("trait Bar {"), SymbolKind::Trait);
        assert_eq!(index.infer_symbol_kind("enum State {"), SymbolKind::Enum);
        assert_eq!(
            index.infer_symbol_kind("class MyClass {"),
            SymbolKind::Class
        );
    }
}
