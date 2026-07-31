# After-action reports

This directory holds dated, point-in-time forensic records of incidents and of
notable agent or fleet runs: what happened, what it cost, what the evidence
shows, and which process or contract changes follow from it.

## What belongs here

- Incident post-mortems for local applications, deployed services, and
  infrastructure.
- Spend, throughput, and token-burn audits of agent or fleet runs.
- Closeout records for bounded programs where the honest outcome, including a
  negative outcome, is worth preserving.

Every report is grounded in retrievable evidence — transcripts, logs, database
rows, commit hashes, receipts. A finding without evidence does not belong in one
of these documents. Where a number is an estimate or an inference, the report
says so in the same sentence.

## What does not belong here

- Current direction, sequencing, or priority. Those live in
  `docs/sol/MASTER_ROADMAP.md`.
- Product intent (`specs/`, `docs/mvp/`) or verification intent
  (`docs/assurance/`).
- Runbooks and operating procedure. Those live in `docs/ops/` and the
  surface-owning directories. An after-action may *propose* a runbook change;
  it is not the runbook.

## Conventions

- **Naming:** `YYYY-MM-DD-<short-slug>.md`, dated for the incident or run, not
  the writing.
- **Header:** open with a `## Document control` table (status, dates, incident
  class, impact, affected subsystem, scope) or an equivalent explicit status
  block. Several existing reports carry a `STATUS: HISTORICAL` banner; add one
  when a report could be mistaken for current direction.
- **Status:** these documents are historical the moment they land. They are not
  amended to track later developments — a later development gets a later report.
- **Neutral wording:** no personal names. Use product, team, role, or operator
  wording, per the repository contract.
- **Redaction:** no secrets, tokens, credentials, connection strings, wallet
  material, or private customer data, even when the source evidence contains
  them. Redact anything secret-shaped and say that you did.
- **Governance:** this directory is an internal working and audit surface. It is
  outside Simplified Technical English scope — `docs/ste/checker-config.v1.json`
  governs only the prefixes listed in its `governedPrefixes` field, which do not
  include `docs/afteraction/`. Write for precision.

## Index

| Date | Report | Subject |
| --- | --- | --- |
| 2026-06-26 | `2026-06-26-khala-glm-openrouter-fallback-afteraction.md` | Provider fallback behavior |
| 2026-06-26 | `2026-06-26-khala-pylon-codex-delegation-afteraction.md` | Pylon/Codex coding delegation path |
| 2026-06-26 | `2026-06-26-khala-roadmap-runbook-noncompliance-afteraction.md` | Roadmap and runbook non-compliance |
| 2026-06-29 | `2026-06-29-codex-fleet-throughput-collapse-after-action.md` | Fleet throughput collapse |
| 2026-06-29 | `2026-06-29-khala-2b-token-burn-and-pr-backlog-afteraction.md` | Two-billion-token burn and PR backlog |
| 2026-07-02 | `2026-07-02-fable-ws17-desktop-fleet-closeout.md` | Desktop fleet workstream closeout |
| 2026-07-19 | `2026-07-19-codex-desktop-git-review-oom-incident-afteraction.md` | Desktop Git review out-of-memory abort |
| 2026-07-20 | `2026-07-20-codex-desktop-main-process-sigtrap-incident-afteraction.md` | Desktop main-process SIGTRAP |
| 2026-07-31 | `2026-07-31-codex-019fb495-overnight-spend-audit.md` | Overnight LiveKit/Sarah run: 1.91B tokens, objective not met |
