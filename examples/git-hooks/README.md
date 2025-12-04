# Git Hooks for Task Validation

Sample hooks to keep `.openagents/tasks.jsonl` conflict-free.

## Hooks
- **pre-commit**: Runs `bun run tasks:validate --check-conflicts` to block commits when the tasks file has merge conflicts or schema errors.
- **post-merge**: Validates tasks immediately after merges to surface conflicts early.
- **pre-push**: Validates tasks before pushing.

All hooks are no-ops if `.openagents/tasks.jsonl` is missing or `bun` is not available.

## Install
```
cd "$(git rev-parse --show-toplevel)"
bash examples/git-hooks/install.sh
```

To remove, delete the installed hooks in `.git/hooks/`.
