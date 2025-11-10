# Critique Tool (`critique.evaluate`)

## Purpose

Evaluate agent outputs (code, text, decisions, tool results) against specified criteria and provide structured feedback. Enables self-refinement loops, quality gates, and iterative improvement of agent work before finalization.

## Name

`critique.evaluate`

## Summary

- Accepts content to evaluate (text, code, tool result, or session snapshot) with evaluation criteria.
- Returns structured feedback: scores, identified issues, suggestions for improvement.
- Supports multiple critic providers (on-device Foundation Models, GPT-4, Claude, custom evaluators).
- Emits an ACP `tool_call` with compact feedback summary; full details in inspector.

## Arguments

- `content` (string, required)
  - The text, code, or structured data to evaluate. For tool results or session snapshots, pass a JSON string.
- `content_type` (string, optional; default: `auto`)
  - Allowed: `auto`, `code`, `text`, `markdown`, `tool_result`, `session_snapshot`
  - Notes: `auto` attempts to infer type from content; explicit types help the critic model focus.
- `criteria` ([string], required)
  - List of evaluation dimensions. Common values:
    - `correctness` — Logical accuracy, no bugs or errors
    - `completeness` — All requirements addressed
    - `quality` — Code/writing quality, clarity, maintainability
    - `security` — Security best practices, no vulnerabilities
    - `performance` — Efficiency, no obvious bottlenecks
    - `style` — Adherence to project conventions
    - `safety` — Workspace boundaries, safe operations
  - Custom criteria accepted (e.g., `accessibility`, `testability`).
- `provider` (string, optional; default: `auto`)
  - Allowed: `auto`, `local_fm`, `gpt4`, `claude`, `custom:<id>`
  - Notes: `auto` selects the best available critic model; explicit values force a specific provider.
- `evaluation_mode` (string, optional; default: `detailed`)
  - Allowed: `detailed`, `summary`, `score_only`
  - `detailed`: Returns scores, issues, and suggestions
  - `summary`: Returns scores and brief overview
  - `score_only`: Returns numeric scores per criterion only
- `scoring_scale` (string, optional; default: `0-10`)
  - Allowed: `0-10`, `0-100`, `letter` (A-F), `pass-fail`
  - Defines how scores are presented.
- `context` (string, optional)
  - Additional context to inform the critique (e.g., project goals, specific concerns, related files).
- `workspace_root` (string, optional)
  - If content references files, this sets the working directory for path resolution.
- `max_issues` (int, optional; default: 10, range: 1–50)
  - Maximum number of issues to report per criterion.
- `dry_run` (bool, optional; default: false)
  - If true, validate arguments and show what would be evaluated without running the critic.

## Result

Structured evaluation feedback:

```json
{
  "ok": true,
  "provider": "local_fm | gpt4 | claude | custom:<id>",
  "overall_score": 8.5,
  "scoring_scale": "0-10",
  "criteria_scores": [
    {
      "criterion": "correctness",
      "score": 9,
      "label": "Excellent"
    },
    {
      "criterion": "completeness",
      "score": 7,
      "label": "Good"
    }
  ],
  "issues": [
    {
      "criterion": "completeness",
      "severity": "medium",
      "location": "line 45-52",
      "description": "Missing error handling for network timeout case",
      "suggestion": "Add a timeout handler with user-friendly error message"
    }
  ],
  "summary": "Code is logically correct and mostly complete. Consider adding error handling for edge cases.",
  "suggestions": [
    "Add unit tests for the timeout scenario",
    "Consider extracting the retry logic into a separate function"
  ],
  "evaluation_time_ms": 1250
}
```

On failure:
```json
{
  "ok": false,
  "error": "Content exceeds maximum size (500KB)",
  "provider": null
}
```

## Routing Behavior

- `auto` provider selection:
  - For code: prefers `local_fm` if available (privacy), falls back to `gpt4` or `claude`
  - For security/safety: prefers specialized evaluators if configured
  - For general text: uses fastest available model
- Explicit `provider` forces routing when available; returns unavailability message otherwise.
- Evaluation runs asynchronously; progress shown in ACP stream.

## Safety & Guardrails

- Content size limit: 500KB to prevent excessive processing time
- Evaluation timeout: 30s default, configurable via runtime
- Workspace-scoped: If file paths are referenced, they must be within `workspace_root`
- No execution: Critique is read-only; it analyzes content but never modifies files or executes code
- All evaluations logged to ACP timeline for auditability
- Privacy: `local_fm` provider runs entirely on-device; external providers may send content to API

## Examples

1) Minimal code critique
```json
{
  "tool": "critique.evaluate",
  "arguments": {
    "content": "func fetchData() async throws -> Data {\n  let url = URL(string: \"https://api.example.com\")!\n  let (data, _) = try await URLSession.shared.data(from: url)\n  return data\n}",
    "content_type": "code",
    "criteria": ["correctness", "quality", "security"]
  }
}
```

2) Session snapshot evaluation with specific focus
```json
{
  "tool": "critique.evaluate",
  "arguments": {
    "content": "{\"session_id\": \"abc123\", \"messages\": [...], \"tool_calls\": [...]}",
    "content_type": "session_snapshot",
    "criteria": ["completeness", "safety"],
    "context": "User requested a refactor of ChatAreaView. Verify all requirements met and no workspace violations.",
    "provider": "local_fm",
    "evaluation_mode": "detailed"
  }
}
```

3) Quick quality check before commit
```json
{
  "tool": "critique.evaluate",
  "arguments": {
    "content": "// ... git diff output ...",
    "content_type": "code",
    "criteria": ["correctness", "style", "security"],
    "provider": "auto",
    "evaluation_mode": "summary",
    "scoring_scale": "pass-fail",
    "max_issues": 5
  }
}
```

4) Documentation review
```json
{
  "tool": "critique.evaluate",
  "arguments": {
    "content": "# New Feature Guide\n\nThis guide explains...",
    "content_type": "markdown",
    "criteria": ["completeness", "quality"],
    "context": "README for new Critique tool. Should explain purpose, usage, and examples clearly.",
    "evaluation_mode": "detailed"
  }
}
```

## UI Integration

- The chat timeline shows a `critique.evaluate` tool call with a compact summary (e.g., "✓ 8.5/10 · 3 issues · 2 suggestions").
- Tapping the row opens a detail sheet with:
  - **Arguments**: Pretty-printed evaluation request
  - **Result**: Structured feedback with expandable issue list
  - Scores visualized as progress bars or color-coded badges
  - Issues grouped by criterion with severity indicators
- Issues may be actionable: tapping suggests a follow-up prompt or opens a file at the referenced location.

## Implementation Notes

- **Provider Integration**:
  - `local_fm`: Uses Apple Foundation Models with system prompts tailored for critique tasks
  - `gpt4` / `claude`: Calls respective APIs with structured output formatting
  - `custom:<id>`: Extensible for domain-specific evaluators (security scanners, linters, etc.)

- **Criteria System**:
  - Standard criteria have predefined evaluation prompts
  - Custom criteria accepted; runtime generates evaluation prompts from criterion names
  - Future: Support for user-defined evaluation rubrics (JSON schema)

- **Self-Refinement Loops**:
  - Agents can call `critique.evaluate` on their own outputs
  - Use feedback to iterate: `delegate.run` → `critique.evaluate` → revise → repeat
  - Future: Built-in refinement loop orchestration

- **Quality Gates**:
  - Can be used as a pre-commit check (e.g., "critique my changes before committing")
  - Runtime may enforce minimum scores for automated commits (configurable)

- **Performance**:
  - On-device critics (local_fm) typically run in 1-3s for code snippets
  - External API critics may take 3-10s depending on content size
  - Results are not cached by default; future may add caching for identical content+criteria

## Notes

- Critique does not modify content; it only provides feedback. Use in conjunction with `edit.patch` or `delegate.run` for actual changes.
- For multi-file evaluations, consider calling `critique.evaluate` per file or using `session_snapshot` mode.
- Privacy-conscious users should prefer `local_fm` provider for sensitive code.
- Critique quality depends on the underlying model; complex criteria may require more capable models (GPT-4, Claude).
