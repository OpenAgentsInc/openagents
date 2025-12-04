# Linting & Formatting Baseline

We use [Biome](https://biomejs.dev/) as the unified formatter/linter for this repo. The configuration lives in `biome.json` (with ignores in `.biomeignore`) and is intentionally gentle to avoid churn while remaining automation-friendly and Effect-compatible.

## Commands

- `bun run lint` – run Biome lint (currently a minimal rule-set; fails on debuggers and reserved files).
- `bun run lint:fix` – apply safe lint fixes.
- `bun run format` – format files with Biome.
- `bun run format:check` – verify formatting without writing.

## Notes

- The linter is tuned to avoid common false positives with Effect TypeScript and `exactOptionalPropertyTypes`. Tighten rules incrementally as we harden the codebase.
- Vendored/worktree artifacts (`src/harbor/**`, `.worktrees`, `.venv`, `node_modules`, `.git`) are excluded to keep lint fast and noise-free.
- CI can call `bun run lint` to keep a consistent baseline; formatting commands are available for bulk cleanups when ready.
