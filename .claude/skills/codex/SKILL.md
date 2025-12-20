---
description: Delegate complex code analysis or generation tasks to Codex (OpenAI's coding agent). Use when the task would benefit from a different AI perspective or when user explicitly requests Codex.
allowed_tools:
  - Bash
---

# Codex Delegation Skill

You can delegate tasks to Codex, OpenAI's AI coding agent, when appropriate.

## When to Use Codex

- Complex multi-file refactoring that would benefit from a fresh perspective
- Code generation in languages where Codex may have different strengths
- When the user explicitly requests Codex ("use codex for this", "ask codex")
- Performance-critical code optimization
- Alternative implementation approaches

## How to Delegate

Run Codex with the exec command:

```bash
codex exec --sandbox workspace-write "Your detailed prompt here"
```

### Options:
- `--sandbox read-only` - For analysis tasks (safer)
- `--sandbox workspace-write` - For tasks that modify files
- `--model <model>` - Specify model (e.g., gpt-4o)

### Important Notes:
1. Give Codex clear, specific instructions
2. Include relevant file paths in your prompt
3. Review Codex's changes before presenting to user
4. If Codex makes errors, you can correct them or retry

## Example

```bash
codex exec --sandbox workspace-write "Refactor the authentication module in src/auth/ to use JWT tokens instead of sessions. Update all related tests."
```
