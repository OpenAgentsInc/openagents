# File Search and Ranking

Crate: `codex-rs/file-search`

Provides fast searching and ranking for project files so the model can fetch and
reference relevant code/text.

## Features

- Uses ripgrep-like matching via dependencies and Nucleo matcher for fuzzy match.
- Supports include/exclude patterns and prioritizes repo-local files.
- Integrates with the TUI `@` search behavior.

## Integration points

- The TUI invokes file search to attach citations or open files.
- The agent can propose opening files by path or by search keyword.

