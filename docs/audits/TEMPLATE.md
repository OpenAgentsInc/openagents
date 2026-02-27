# YYYY-MM-DD Full Codebase Architecture Audit

## Scope

- `apps/autopilot-desktop`
- `crates/nostr/*`
- `crates/spark`
- `crates/wgpui*`
- lint/build guardrails

## Method

- Review against:
  - `docs/MVP.md`
  - `docs/OWNERSHIP.md`
- Run:
  - `cargo check --workspace --tests`
  - `scripts/lint/ownership-boundary-check.sh`
  - `scripts/lint/clippy-regression-check.sh`
  - `cargo fmt --all --check`

## Ownership-Boundary Drift

- Changes since prior audit:
- Violations found:
- Remediation actions:

## Largest-File Trend

- Top file sizes (current):
- Delta vs previous month:
- Concentration risks:

## Dead-Code Warning Trend

- Current warning set:
- Delta vs previous month:
- Highest-priority removals/wiring:

## Lint-Gate Trend

- `clippy-regression-check`: pass/fail + notes
- `fmt --check`: pass/fail + notes
- Any lane blockers:

## Findings

- Ranked findings with evidence and impact.

## Recommendations

- Actionable cleanup recommendations with priority.

## Follow-On Issues

- List created issues and owners.
