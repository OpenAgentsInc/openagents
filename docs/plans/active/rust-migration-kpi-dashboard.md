# Rust Migration KPI Dashboard (OA-RUST-006)

Status: Active  
Last updated: 2026-02-21  
Owner lane: `owner:contracts-docs`

## Purpose

Define objective migration KPIs, data sources, owners, cadence, and breach escalation so Rust cutover decisions are gate-driven instead of subjective.

## KPI Dictionary

| KPI ID | KPI | Formula | Green Gate | Yellow Gate | Red Gate | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `KPI-01` | Route parity progress | `closed(route_set) / total(route_set)` where route_set = OA-RUST-031, 032, 059, 060, 061, 062, 063, 064, 100 | `100%` at cutover gate | `<100%` before cutover gate | `<100%` at/after cutover gate | `owner:openagents.com` |
| `KPI-02` | Runtime parity progress | `closed(runtime_set) / total(runtime_set)` where runtime_set = OA-RUST-033..040 | `100%` at runtime cutover gate | `<100%` before runtime cutover gate | `<100%` at/after runtime cutover gate | `owner:runtime` |
| `KPI-03` | WS reliability readiness | `closed(ws_readiness_set) / total(ws_readiness_set)` where ws_readiness_set = OA-RUST-042, 043, 044, 045, 047, 048, 070, 071, 088, 089, 092, 093, 087 | `100%` at WS-only gate and no unresolved WS incidents >24h | `<100%` before WS-only gate with no unresolved WS incidents >24h | `<100%` at/after WS-only gate or any unresolved WS incident >24h | `owner:khala` |
| `KPI-04` | Latency budget readiness | `closed(latency_set) / total(latency_set)` where latency_set = OA-RUST-065, 094 | `100%` at web cutover gate | `<100%` before web cutover gate | `<100%` at/after web cutover gate | `owner:openagents.com` + `owner:khala` |
| `KPI-05` | Migration regression count | `count(open issues matching "OA-RUST- label:bug")` | `0` open regressions | `1-2` open regressions | `>=3` open regressions | `owner:contracts-docs` |
| `KPI-06` | Rollback incident count | `count(rollback incidents in reporting window)` | `0` in trailing 14d | `1` in trailing 14d | `>=2` in trailing 14d | `owner:infra` |

## Data Sources and Commands

| KPI ID | Source | Collection command |
| --- | --- | --- |
| `KPI-01` | GitHub issue state (route parity set) | `gh issue view <id> --json state --jq .state` |
| `KPI-02` | GitHub issue state (runtime parity set) | `gh issue view <id> --json state --jq .state` |
| `KPI-03` | GitHub issue state (WS readiness set) + incident runbooks | `gh issue view <id> --json state --jq .state` |
| `KPI-04` | GitHub issue state (latency set) + perf signoff artifacts | `gh issue view <id> --json state --jq .state` |
| `KPI-05` | GitHub issue search | `gh issue list --state open --search "OA-RUST- label:bug" --json number,title` |
| `KPI-06` | Weekly migration report entries (`rollback incidents` section) | Read `docs/plans/active/rust-migration-reports/*.md` |

## Ownership Matrix

| KPI | Primary owner | Backup owner | Review forum |
| --- | --- | --- | --- |
| Route parity (`KPI-01`) | `owner:openagents.com` | `owner:contracts-docs` | Weekly migration report |
| Runtime parity (`KPI-02`) | `owner:runtime` | `owner:infra` | Weekly migration report |
| WS reliability (`KPI-03`) | `owner:khala` | `owner:runtime` | Weekly migration report + incident review |
| Latency readiness (`KPI-04`) | `owner:openagents.com` | `owner:khala` | Weekly migration report |
| Regression count (`KPI-05`) | `owner:contracts-docs` | `owner:openagents.com` | Daily triage + weekly report |
| Rollback incidents (`KPI-06`) | `owner:infra` | `owner:contracts-docs` | Incident review + weekly report |

## Reporting Cadence (Adopted)

1. Weekly: publish a KPI report every Friday in `docs/plans/active/rust-migration-reports/` using `docs/plans/active/rust-migration-kpi-report-template.md`.
2. Daily: update Project 12 (`Migration Status` field) for all touched OA-RUST issues.
3. Milestone: before any cutover gate (Phase 5, 9, 13), attach latest KPI report link to the gate issue.

## Go/No-Go Gates Tied To KPIs

1. No Phase 9 cutover approval unless `KPI-01 >= 100%`, `KPI-03 >= 100%`, and `KPI-04 >= 100%`.
2. No runtime authority cutover (OA-RUST-040) unless `KPI-02 >= 100%`.
3. No closure approval (OA-RUST-106/107 lane) if `KPI-05` is red or `KPI-06` is red.

## Breach Escalation Path

1. Breach detected in weekly report.
2. Open/update blocker issue with labels: `roadmap`, `risk:high` (or `risk:critical`), and relevant `owner:*` + `area:*`.
3. Move affected Project 12 items to `Blocked` within 24h.
4. Add mitigation plan and target recovery date in blocker issue comment.
5. A KPI remains red until next report shows recovery and linked blocker is closed.

## Verification Checklist

1. Run KPI collection commands and paste outputs in weekly report.
2. Link report in issue `#1821` and Project 12 notes.
3. Verify `Migration Status` updates match closed/open issue state.

## First Report Requirement

First baseline report generated at:

- `docs/plans/active/rust-migration-reports/2026-02-21-oa-rust-kpi-baseline.md`
