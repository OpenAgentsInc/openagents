# Architecture Audit Cadence

## Cadence

- Run one full architecture hygiene audit every month.
- Naming convention: `YYYY-MM-DD-full-codebase-architecture-audit.md`.
- Store each report in `docs/audits/`.

## Required Sections

Every monthly audit must include:

1. Ownership-boundary drift
2. Largest-file trend and module concentration
3. Dead-code warning trend
4. Lint-gate status trend

Use `docs/audits/TEMPLATE.md` as the baseline structure.

## Minimum Workflow

1. Create/confirm the monthly audit issue in GitHub.
2. Run the checks listed in the template.
3. Write findings and trend deltas vs prior audit.
4. Open follow-up cleanup issues for concrete actions.
5. Close the monthly audit issue only after the report is committed.

## Scheduling Rule

- Maintain at least one next-month audit issue in `OPEN` state at all times.
- Suggested title format: `[cleanup][P3] Monthly architecture hygiene audit - YYYY-MM`.
- Current scheduled issue: `#2389` (`[cleanup][P3] Monthly architecture hygiene audit - 2026-03`).
