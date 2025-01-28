# Repository Map Generation Tool

This tool generates a visual representation of the repository's file structure, saving it to `docs/repomap.md`. It creates a hierarchical view of files and directories, with built-in exclusions for common build artifacts and additional custom exclusions.

## Usage

The tool can be run using cargo:

```bash
# Use current branch (if in a git repo)
cargo run --bin generate-repomap

# Specify a branch explicitly
cargo run --bin generate-repomap --branch handshake
# or
cargo run --bin generate-repomap -b handshake
```

## Features

- Automatically detects current git branch if not specified
- Falls back to 'main' branch if not in a git repository and no branch specified
- Built-in exclusions for common paths:
  - `target/` (build artifacts)
  - `.git/` (git internals)
  - `node_modules/` (npm dependencies)
- Additional custom exclusion:
  - `assets/main.css`
- Creates a hierarchical view with proper indentation

## Requirements

- Rust toolchain
- Git installed and available in PATH
- Environment variables:
  - `DEEPSEEK_API_KEY` (required)
  - `GITHUB_TOKEN` (optional)

## Output

The tool generates a markdown file at `docs/repomap.md` containing the repository structure. The output uses a tree-like format with indentation to show the hierarchy of files and directories in the repository.
