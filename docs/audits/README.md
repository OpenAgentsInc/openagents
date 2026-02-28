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
2. Run the standard audit command:
   - `./scripts/audits/run-architecture-audit.sh`
   - Optional output path override: `./scripts/audits/run-architecture-audit.sh /tmp/audit.json`
3. Review the machine-readable snapshot:
   - Default path: `docs/audits/snapshots/YYYY-MM-DD-architecture-audit.json`
   - Per-check logs: `docs/audits/snapshots/YYYY-MM-DD-architecture-audit.json.logs/`
4. Run any additional checks listed in the template if needed for deeper investigation.
5. Write findings and trend deltas vs prior audit.
6. Open follow-up cleanup issues for concrete actions.
7. Close the monthly audit issue only after the report is committed.

## Scheduling Rule

- Maintain at least one next-month audit issue in `OPEN` state at all times.
- Suggested title format: `[cleanup][P3] Monthly architecture hygiene audit - YYYY-MM`.
- Current scheduled issue: `#2403` (`[cleanup][P3] Monthly architecture hygiene audit - 2026-04`).
