# Rust Migration KPI Baseline Report

Date: 2026-02-21  
Reporting window: 2026-02-21 to 2026-02-21  
Prepared by: `owner:contracts-docs`

## Snapshot

| KPI | Value | Target | Status | Trend |
| --- | --- | --- | --- | --- |
| `KPI-01` Route parity progress | `0/9` (`0.0%`) | `100%` at cutover gate | `Yellow` | `Baseline` |
| `KPI-02` Runtime parity progress | `0/8` (`0.0%`) | `100%` at runtime cutover gate | `Yellow` | `Baseline` |
| `KPI-03` WS reliability readiness | `0/13` (`0.0%`) | `100%` at WS-only gate | `Yellow` | `Baseline` |
| `KPI-04` Latency budget readiness | `0/2` (`0.0%`) | `100%` at web cutover gate | `Yellow` | `Baseline` |
| `KPI-05` Migration regression count | `0` open bug issues | `0` | `Green` | `Baseline` |
| `KPI-06` Rollback incident count | `0` in trailing 14d | `0` | `Green` | `Baseline` |

## Evidence

### Project and issue state

Project 12 summary (captured 2026-02-21):

- Total OA-RUST items: `107`
- `Done`: `5`
- `Backlog`: `102`

Command used:

```bash
gh project item-list 12 --owner OpenAgentsInc --limit 500 --format json | jq '{total:.totalCount, migration_status_counts:(.items | map((.["migration Status"] // "(unset)")) | group_by(.) | map({status:.[0], count:length}))}'
```

Regression query result:

- Open OA-RUST bug issues: `0`

Command used:

```bash
gh issue list --state open --limit 200 --search "OA-RUST- label:bug" --json number,title | jq '{open_bug_count:length}'
```

### KPI set calculations

Route parity set (`KPI-01`):
- OA-RUST-031, OA-RUST-032, OA-RUST-059, OA-RUST-060, OA-RUST-061, OA-RUST-062, OA-RUST-063, OA-RUST-064, OA-RUST-100
- Closed: `0/9`

Runtime parity set (`KPI-02`):
- OA-RUST-033, OA-RUST-034, OA-RUST-035, OA-RUST-036, OA-RUST-037, OA-RUST-038, OA-RUST-039, OA-RUST-040
- Closed: `0/8`

WS readiness set (`KPI-03`):
- OA-RUST-042, OA-RUST-043, OA-RUST-044, OA-RUST-045, OA-RUST-047, OA-RUST-048, OA-RUST-070, OA-RUST-071, OA-RUST-087, OA-RUST-088, OA-RUST-089, OA-RUST-092, OA-RUST-093
- Closed: `0/13`

Latency set (`KPI-04`):
- OA-RUST-065, OA-RUST-094
- Closed: `0/2`

## Breaches and Escalation

No `Red` KPI breaches in this baseline window. Migration remains pre-cutover.

## Rollback Incidents (Window)

No rollback incidents recorded in this reporting window.

## Decision Log

1. KPI framework is adopted as the Rust migration gate mechanism.
2. Weekly KPI reporting cadence starts now.
3. Cutover/no-go decisions must reference the latest KPI report artifact.

## Next Actions

1. Land OA-RUST-007 and OA-RUST-008 to unblock contract-first implementation lanes.
2. Keep Project 12 `Migration Status` synchronized with issue state on each closure.
3. Publish next weekly KPI report with trend deltas from this baseline.
