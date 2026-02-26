# Lint Posture

## Decision

OpenAgents uses a strict baseline lint posture:

- Workspace lint policy is authoritative.
- `clippy::unwrap_used`, `clippy::expect_used`, and `clippy::panic` remain denied.
- Crate-wide suppression for those lints is not allowed.

## Exception Policy

Temporary exceptions are allowed only when all of the following are true:

- The exception is scoped to the smallest possible item (statement/function/module).
- The exception has an inline `reason` that states the invariant.
- The code path has no practical fallible alternative without harming API clarity or performance.
- The exception is reviewed and removed when the surrounding code is refactored.

## Enforcement

- Run `scripts/lint/clippy-regression-check.sh` before commit.
- Use `scripts/lint/touched-clippy-gate.sh` (clean-on-touch gate) for changed files.
- Track pre-existing debt in `scripts/lint/clippy-debt-allowlist.toml`.
- Allowlist entries must include `path | owner:<lane> | added:<YYYY-MM-DD> | reason:<...>`.
