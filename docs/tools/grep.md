# Grep Tool (`search.grep`)

## Purpose

Bounded workspace search for a regex pattern. Optimized for speed and safety. Used by the runtime and models to quickly locate code/text across the repository.

## Name

`search.grep`

## Summary

Find lines matching a regex pattern within the workspace. Skips binary files and common build/dependency directories. Respects time and result caps for responsiveness.

## Implementation Notes

- Uses ripgrep (`rg`) when available for performance; falls back to a native Swift implementation otherwise.
- Always enforces workspace boundaries; paths outside the workspace are rejected.
- Applies a short time cap (≈1.5s) and a match cap to keep results snappy.

## Arguments

- `pattern` (string, required)
  - Regex to match. Defaults to case-sensitive unless `case_insensitive` is set.
- `path_prefix` (string, optional)
  - Workspace-relative directory or file to scope the search.
- `case_insensitive` (bool, optional; default: false)
  - If true, uses case-insensitive matching.
- `max_results` (int, optional; default: 200, range: 1–200)
  - Maximum number of matches to return; total matches may be higher.
- `context_lines` (int, optional; default: 0, range: 0–3)
  - Lines of context to include before/after each match. When 0, no context fields are returned.

## Result

Structured output with a compact, model-friendly shape.

- `total_matches` (int): Count of all matches encountered (may exceed `matches.length`).
- `truncated` (bool): True if `max_results` or time cap was hit.
- `matches` (array): List of match objects:
  - `path` (string): Workspace-relative file path.
  - `line_number` (int): 1-based line index.
  - `line` (string): The matching line.
  - `context_before` ([string], optional): Present only when `context_lines` > 0.
  - `context_after` ([string], optional): Present only when `context_lines` > 0.

## Safety & Guardrails

- Skips hidden files and common large/vendor/build directories (e.g., `.git`, `node_modules`, `DerivedData`, `build`, `dist`, `.build`).
- Skips binary files.
- Enforces workspace boundaries on `path_prefix` and results.
- Emits progress and completion via ACP `tool_call` events.

## Examples

1) Minimal search
```json
{
  "tool": "search.grep",
  "arguments": {
    "pattern": "\\bOpenAgents\\b"
  }
}
```

2) Scoped search with case-insensitive matching
```json
{
  "tool": "search.grep",
  "arguments": {
    "pattern": "swiftui view",
    "path_prefix": "ios/OpenAgents/Views",
    "case_insensitive": true,
    "max_results": 100
  }
}
```

3) With context lines
```json
{
  "tool": "search.grep",
  "arguments": {
    "pattern": "NavigationSplitView",
    "context_lines": 2
  }
}
```

## UI Integration

- The chat timeline shows a tool call row named `search.grep` with an inline pattern summary (e.g., "🔍 NavigationSplitView").
- Tapping the row opens a detail sheet with pretty-printed Arguments and Result.

