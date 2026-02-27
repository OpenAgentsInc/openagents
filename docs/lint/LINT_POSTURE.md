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
- Enforce strict production hardening lanes with `scripts/lint/strict-production-hardening-check.sh`.
- Track pre-existing debt in `scripts/lint/clippy-debt-allowlist.toml`.
- Validate allowlist structure with `scripts/lint/clippy-debt-allowlist-check.sh`.
- Enforce high-churn warning ceilings with `scripts/lint/clippy-warning-budget-check.sh`.
- Validate repo-managed Agent Skills with `scripts/skills/validate_registry.sh`.

## Strict Production Lanes

- Required strict commands:
  - `cargo clippy -p nostr --lib --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic`
  - `cargo clippy -p autopilot-desktop --bin autopilot-desktop --no-deps -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic`
- These are run via `scripts/lint/strict-production-hardening-check.sh` and are part of `scripts/lint/clippy-regression-check.sh`.

## Allowlist Rules

- Allowlist entries must include:
  - `path`
  - `owner:<lane>`
  - `added:<YYYY-MM-DD>`
  - `reason:<short reason>`
  - one time-bound field:
    - `review_cadence:<...>` (for recurring review), or
    - `expiry_issue:<#...>` (for tracked removal), or
    - `expires_on:<YYYY-MM-DD>` (for hard expiry).
- Entries that are not time-bounded are invalid and should fail lint posture checks.
- Entries added on/after `2026-02-27` must include `expiry_issue` or `expires_on` (review cadence alone is no longer sufficient for new debt).

## Warning Budgets

- High-churn files are tracked in `scripts/lint/clippy-warning-budgets.toml`.
- Budget entries must include:
  - `path`
  - `budget:<non-negative integer>`
  - `owner:<lane>`
  - `added:<YYYY-MM-DD>`
  - `reason:<short reason>`
  - time-bound metadata (same as allowlist policy)
- Budgets are exact:
  - If current warnings exceed a budget, the gate fails.
  - If current warnings fall below a budget, the gate also fails until the budget is lowered in the same change.

## Workflow

- If you touch a Rust file with existing warnings, either clean them up or add a debt entry that follows the allowlist rules.
- If you add debt, include a clear owner and time-bound metadata so removal is auditable.
- Keep debt entries temporary and delete them as soon as warnings are removed.
- If a budgeted file warning count drops, lower the budget in `clippy-warning-budgets.toml` before merge.
