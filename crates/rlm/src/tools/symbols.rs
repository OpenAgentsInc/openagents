//! Symbol extraction tool for RLM environment.
//!
//! Extracts symbols (functions, types, constants) from source files
//! using regex-based parsing. A tree-sitter implementation could be
//! added for more accurate parsing.

use super::{RlmTool, ToolConfig, ToolError, ToolResult, get_current_commit};
use crate::span::SpanRef;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

/// Kind of symbol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    Static,
    Type,
    Module,
    Class,
    Interface,
    Variable,
    Unknown,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::Type => "type",
            SymbolKind::Module => "module",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Variable => "variable",
            SymbolKind::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

/// Information about an extracted symbol.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// SpanRef pointing to the symbol definition.
    pub span: SpanRef,
    /// Parent symbol (for methods, nested items).
    pub parent: Option<String>,
    /// Signature/declaration line.
    pub signature: String,
    /// Documentation comment if present.
    pub doc: Option<String>,
}

/// Symbol extraction tool.
///
/// Extracts symbols from source files using language-specific
/// regex patterns. Provides SpanRefs for each symbol.
pub struct SymbolsTool {
    repo_root: PathBuf,
    config: ToolConfig,
}

impl SymbolsTool {
    /// Create a new SymbolsTool rooted at the given path.
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            config: ToolConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(repo_root: PathBuf, config: ToolConfig) -> Self {
        Self { repo_root, config }
    }

    /// Extract symbols from a file.
    pub async fn extract(&self, path: &str) -> ToolResult<Vec<SymbolInfo>> {
        let file_path = self.repo_root.join(path);

        if !file_path.exists() {
            return Err(ToolError::PathNotFound(path.to_string()));
        }

        let content = fs::read_to_string(&file_path).map_err(ToolError::Io)?;

        // Check file size
        if content.len() as u64 > self.config.max_file_size {
            return Err(ToolError::ExecutionError(format!(
                "File too large: {} bytes",
                content.len()
            )));
        }

        // Detect language from extension
        let language = path
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();

        let commit = self.config.commit.clone()
            .or_else(|| get_current_commit(&self.repo_root));

        let symbols = match language.as_str() {
            "rs" => self.extract_rust(&content, path, commit.as_deref()),
            "py" => self.extract_python(&content, path, commit.as_deref()),
            "ts" | "tsx" | "js" | "jsx" => self.extract_typescript(&content, path, commit.as_deref()),
            "go" => self.extract_go(&content, path, commit.as_deref()),
            _ => self.extract_generic(&content, path, commit.as_deref()),
        };

        Ok(symbols)
    }

    /// Extract symbols from Rust source.
    fn extract_rust(&self, content: &str, path: &str, commit: Option<&str>) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Patterns for Rust symbols
        let patterns = [
            (r"^\s*(?:pub\s+)?fn\s+(\w+)", SymbolKind::Function),
            (r"^\s*(?:pub\s+)?struct\s+(\w+)", SymbolKind::Struct),
            (r"^\s*(?:pub\s+)?enum\s+(\w+)", SymbolKind::Enum),
            (r"^\s*(?:pub\s+)?trait\s+(\w+)", SymbolKind::Trait),
            (r"^\s*impl(?:<[^>]+>)?\s+(?:(\w+)\s+for\s+)?(\w+)", SymbolKind::Impl),
            (r"^\s*(?:pub\s+)?const\s+(\w+)", SymbolKind::Const),
            (r"^\s*(?:pub\s+)?static\s+(\w+)", SymbolKind::Static),
            (r"^\s*(?:pub\s+)?type\s+(\w+)", SymbolKind::Type),
            (r"^\s*(?:pub\s+)?mod\s+(\w+)", SymbolKind::Module),
        ];

        for (pattern, kind) in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_idx, line) in lines.iter().enumerate() {
                    if let Some(caps) = regex.captures(line) {
                        // Get the symbol name (last capture group)
                        let name = caps
                            .iter()
                            .skip(1)
                            .filter_map(|m| m)
                            .last()
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_default();

                        if name.is_empty() {
                            continue;
                        }

                        let line_num = line_idx as u32 + 1;

                        // Look for doc comments above
                        let doc = self.extract_doc_comment(&lines, line_idx);

                        // Calculate byte offset
                        let byte_offset: u64 = lines[..line_idx]
                            .iter()
                            .map(|l| l.len() as u64 + 1)
                            .sum();

                        let span = SpanRef::with_range(
                            format!("sym-{}-{}", path.replace('/', "-"), name),
                            path.to_string(),
                            line_num,
                            line_num,
                            byte_offset,
                            byte_offset + line.len() as u64,
                        )
                        .with_content(line);

                        let span = if let Some(c) = commit {
                            span.with_commit(c)
                        } else {
                            span
                        };

                        symbols.push(SymbolInfo {
                            name,
                            kind: kind.clone(),
                            span,
                            parent: None,
                            signature: line.trim().to_string(),
                            doc,
                        });
                    }
                }
            }
        }

        symbols
    }

    /// Extract symbols from Python source.
    fn extract_python(&self, content: &str, path: &str, commit: Option<&str>) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let patterns = [
            (r"^def\s+(\w+)", SymbolKind::Function),
            (r"^class\s+(\w+)", SymbolKind::Class),
            (r"^\s+def\s+(\w+)", SymbolKind::Method),
            (r"^(\w+)\s*=", SymbolKind::Variable),
        ];

        for (pattern, kind) in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_idx, line) in lines.iter().enumerate() {
                    if let Some(caps) = regex.captures(line) {
                        if let Some(name) = caps.get(1) {
                            let name = name.as_str().to_string();
                            let line_num = line_idx as u32 + 1;

                            let doc = self.extract_python_docstring(&lines, line_idx);

                            let byte_offset: u64 = lines[..line_idx]
                                .iter()
                                .map(|l| l.len() as u64 + 1)
                                .sum();

                            let span = SpanRef::with_range(
                                format!("sym-{}-{}", path.replace('/', "-"), name),
                                path.to_string(),
                                line_num,
                                line_num,
                                byte_offset,
                                byte_offset + line.len() as u64,
                            );

                            let span = if let Some(c) = commit {
                                span.with_commit(c)
                            } else {
                                span
                            };

                            symbols.push(SymbolInfo {
                                name,
                                kind: kind.clone(),
                                span,
                                parent: None,
                                signature: line.trim().to_string(),
                                doc,
                            });
                        }
                    }
                }
            }
        }

        symbols
    }

    /// Extract symbols from TypeScript/JavaScript source.
    fn extract_typescript(&self, content: &str, path: &str, commit: Option<&str>) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let patterns = [
            (r"(?:export\s+)?(?:async\s+)?function\s+(\w+)", SymbolKind::Function),
            (r"(?:export\s+)?class\s+(\w+)", SymbolKind::Class),
            (r"(?:export\s+)?interface\s+(\w+)", SymbolKind::Interface),
            (r"(?:export\s+)?type\s+(\w+)", SymbolKind::Type),
            (r"(?:export\s+)?const\s+(\w+)", SymbolKind::Const),
            (r"(?:export\s+)?(?:let|var)\s+(\w+)", SymbolKind::Variable),
            (r"^\s+(?:async\s+)?(\w+)\s*\([^)]*\)\s*[:{]", SymbolKind::Method),
        ];

        for (pattern, kind) in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_idx, line) in lines.iter().enumerate() {
                    if let Some(caps) = regex.captures(line) {
                        if let Some(name) = caps.get(1) {
                            let name = name.as_str().to_string();
                            let line_num = line_idx as u32 + 1;

                            let doc = self.extract_jsdoc(&lines, line_idx);

                            let byte_offset: u64 = lines[..line_idx]
                                .iter()
                                .map(|l| l.len() as u64 + 1)
                                .sum();

                            let span = SpanRef::with_range(
                                format!("sym-{}-{}", path.replace('/', "-"), name),
                                path.to_string(),
                                line_num,
                                line_num,
                                byte_offset,
                                byte_offset + line.len() as u64,
                            );

                            let span = if let Some(c) = commit {
                                span.with_commit(c)
                            } else {
                                span
                            };

                            symbols.push(SymbolInfo {
                                name,
                                kind: kind.clone(),
                                span,
                                parent: None,
                                signature: line.trim().to_string(),
                                doc,
                            });
                        }
                    }
                }
            }
        }

        symbols
    }

    /// Extract symbols from Go source.
    fn extract_go(&self, content: &str, path: &str, commit: Option<&str>) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let patterns = [
            (r"^func\s+(?:\([^)]+\)\s+)?(\w+)", SymbolKind::Function),
            (r"^type\s+(\w+)\s+struct", SymbolKind::Struct),
            (r"^type\s+(\w+)\s+interface", SymbolKind::Interface),
            (r"^const\s+(\w+)", SymbolKind::Const),
            (r"^var\s+(\w+)", SymbolKind::Variable),
        ];

        for (pattern, kind) in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_idx, line) in lines.iter().enumerate() {
                    if let Some(caps) = regex.captures(line) {
                        if let Some(name) = caps.get(1) {
                            let name = name.as_str().to_string();
                            let line_num = line_idx as u32 + 1;

                            let doc = self.extract_doc_comment(&lines, line_idx);

                            let byte_offset: u64 = lines[..line_idx]
                                .iter()
                                .map(|l| l.len() as u64 + 1)
                                .sum();

                            let span = SpanRef::with_range(
                                format!("sym-{}-{}", path.replace('/', "-"), name),
                                path.to_string(),
                                line_num,
                                line_num,
                                byte_offset,
                                byte_offset + line.len() as u64,
                            );

                            let span = if let Some(c) = commit {
                                span.with_commit(c)
                            } else {
                                span
                            };

                            symbols.push(SymbolInfo {
                                name,
                                kind: kind.clone(),
                                span,
                                parent: None,
                                signature: line.trim().to_string(),
                                doc,
                            });
                        }
                    }
                }
            }
        }

        symbols
    }

    /// Generic symbol extraction for unknown languages.
    fn extract_generic(&self, content: &str, path: &str, commit: Option<&str>) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Try common patterns
        let patterns = [
            (r"(?:function|def|fn|func)\s+(\w+)", SymbolKind::Function),
            (r"(?:class|struct|type)\s+(\w+)", SymbolKind::Struct),
        ];

        for (pattern, kind) in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for (line_idx, line) in lines.iter().enumerate() {
                    if let Some(caps) = regex.captures(line) {
                        if let Some(name) = caps.get(1) {
                            let name = name.as_str().to_string();
                            let line_num = line_idx as u32 + 1;

                            let byte_offset: u64 = lines[..line_idx]
                                .iter()
                                .map(|l| l.len() as u64 + 1)
                                .sum();

                            let span = SpanRef::with_range(
                                format!("sym-{}-{}", path.replace('/', "-"), name),
                                path.to_string(),
                                line_num,
                                line_num,
                                byte_offset,
                                byte_offset + line.len() as u64,
                            );

                            let span = if let Some(c) = commit {
                                span.with_commit(c)
                            } else {
                                span
                            };

                            symbols.push(SymbolInfo {
                                name,
                                kind: kind.clone(),
                                span,
                                parent: None,
                                signature: line.trim().to_string(),
                                doc: None,
                            });
                        }
                    }
                }
            }
        }

        symbols
    }

    /// Extract doc comments (/// or /** style) above a line.
    fn extract_doc_comment(&self, lines: &[&str], target_idx: usize) -> Option<String> {
        let mut doc_lines = Vec::new();
        let mut idx = target_idx;

        while idx > 0 {
            idx -= 1;
            let line = lines[idx].trim();

            if line.starts_with("///") {
                doc_lines.push(line.trim_start_matches('/').trim());
            } else if line.starts_with("//!") {
                // Inner doc comment, stop
                break;
            } else if line.ends_with("*/") || line.starts_with("/**") || line.starts_with("/*") {
                // Block comment handling would go here
                break;
            } else if line.starts_with("//") {
                // Regular comment, continue looking
                continue;
            } else if line.is_empty() {
                // Empty line, continue looking
                continue;
            } else {
                // Non-comment, non-empty line, stop
                break;
            }
        }

        doc_lines.reverse();
        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join(" "))
        }
    }

    /// Extract Python docstring below a definition.
    fn extract_python_docstring(&self, lines: &[&str], target_idx: usize) -> Option<String> {
        if target_idx + 1 >= lines.len() {
            return None;
        }

        let next_line = lines[target_idx + 1].trim();
        if next_line.starts_with("\"\"\"") || next_line.starts_with("'''") {
            let quote = if next_line.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };

            if next_line.ends_with(quote) && next_line.len() > 6 {
                // Single line docstring
                return Some(next_line.trim_matches(|c| c == '"' || c == '\'').trim().to_string());
            }

            // Multi-line docstring
            let mut doc = Vec::new();
            doc.push(next_line.trim_start_matches(quote).trim());

            for i in (target_idx + 2)..lines.len() {
                let line = lines[i].trim();
                if line.ends_with(quote) {
                    doc.push(line.trim_end_matches(quote).trim());
                    break;
                }
                doc.push(line);
            }

            return Some(doc.join(" ").trim().to_string());
        }

        None
    }

    /// Extract JSDoc comments above a line.
    fn extract_jsdoc(&self, lines: &[&str], target_idx: usize) -> Option<String> {
        if target_idx == 0 {
            return None;
        }

        let mut idx = target_idx - 1;
        let mut in_jsdoc = false;
        let mut doc_lines = Vec::new();

        while idx > 0 || (idx == 0 && !in_jsdoc) {
            let line = lines[idx].trim();

            if line.ends_with("*/") {
                in_jsdoc = true;
                if line.starts_with("/**") {
                    // Single line JSDoc
                    let content = line
                        .trim_start_matches("/**")
                        .trim_end_matches("*/")
                        .trim();
                    return Some(content.to_string());
                }
            } else if in_jsdoc {
                if line.starts_with("/**") {
                    break;
                }
                let content = line.trim_start_matches('*').trim();
                if !content.starts_with('@') {
                    doc_lines.push(content);
                }
            } else {
                break;
            }

            if idx == 0 {
                break;
            }
            idx -= 1;
        }

        doc_lines.reverse();
        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join(" ").trim().to_string())
        }
    }
}

#[async_trait]
impl RlmTool for SymbolsTool {
    fn name(&self) -> &str {
        "symbols"
    }

    fn description(&self) -> &str {
        "Extract symbols (functions, classes, types) from a source file. Returns symbol info with SpanRefs."
    }

    fn args_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to repository root"
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by symbol kind (function, struct, class, etc.)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult<Value> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::ParseError("Missing 'path' argument".to_string()))?;

        let mut symbols = self.extract(path).await?;

        // Filter by kind if specified
        if let Some(kind_str) = args["kind"].as_str() {
            let kind_lower = kind_str.to_lowercase();
            symbols.retain(|s| s.kind.to_string() == kind_lower);
        }

        Ok(json!({
            "symbols": symbols,
            "count": symbols.len()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_extract_rust_symbols() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "/// A test function").unwrap();
        writeln!(file, "pub fn hello() {{}}").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "struct Point {{ x: i32, y: i32 }}").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "impl Point {{").unwrap();
        writeln!(file, "    fn new() -> Self {{ Self {{ x: 0, y: 0 }} }}").unwrap();
        writeln!(file, "}}").unwrap();

        let tool = SymbolsTool::new(temp.path().to_path_buf());
        let symbols = tool.extract("test.rs").await.unwrap();

        assert!(symbols.iter().any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "new" && s.kind == SymbolKind::Function));

        // Check doc comment was extracted
        let hello = symbols.iter().find(|s| s.name == "hello").unwrap();
        assert!(hello.doc.as_ref().map_or(false, |d| d.contains("test function")));
    }

    #[tokio::test]
    async fn test_extract_python_symbols() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.py");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    \"\"\"A hello function\"\"\"").unwrap();
        writeln!(file, "    pass").unwrap();
        writeln!(file, "").unwrap();
        writeln!(file, "class Point:").unwrap();
        writeln!(file, "    def __init__(self):").unwrap();
        writeln!(file, "        pass").unwrap();

        let tool = SymbolsTool::new(temp.path().to_path_buf());
        let symbols = tool.extract("test.py").await.unwrap();

        assert!(symbols.iter().any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Class));
    }

    #[tokio::test]
    async fn test_extract_typescript_symbols() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.ts");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "export function hello() {{}}").unwrap();
        writeln!(file, "export class Point {{}}").unwrap();
        writeln!(file, "export interface Shape {{}}").unwrap();
        writeln!(file, "export type ID = string;").unwrap();

        let tool = SymbolsTool::new(temp.path().to_path_buf());
        let symbols = tool.extract("test.ts").await.unwrap();

        assert!(symbols.iter().any(|s| s.name == "hello" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "Shape" && s.kind == SymbolKind::Interface));
        assert!(symbols.iter().any(|s| s.name == "ID" && s.kind == SymbolKind::Type));
    }
}
