# Rust Migration KPI Report Template

Date: YYYY-MM-DD  
Reporting window: YYYY-MM-DD to YYYY-MM-DD  
Prepared by: `<owner lane>`

## Snapshot

| KPI | Value | Target | Status (`Green`/`Yellow`/`Red`) | Trend (`Up`/`Flat`/`Down`) |
| --- | --- | --- | --- | --- |
| `KPI-01` Route parity progress |  |  |  |  |
| `KPI-02` Runtime parity progress |  |  |  |  |
| `KPI-03` WS reliability readiness |  |  |  |  |
| `KPI-04` Latency budget readiness |  |  |  |  |
| `KPI-05` Migration regression count |  |  |  |  |
| `KPI-06` Rollback incident count |  |  |  |  |

## Evidence

### Project and issue state

```bash
gh project item-list 12 --owner OpenAgentsInc --limit 500 --format json
```

```bash
gh issue list --state open --search "OA-RUST- label:bug" --json number,title
```

### KPI set calculations

Record counts for each KPI set and include exact issue IDs used for numerator/denominator.

## Breaches and Escalation

1. List KPI breaches (`Yellow` or `Red`) and impact.
2. Link blocker issue(s) opened/updated.
3. Record ETA and owner for recovery.

## Rollback Incidents (Window)

1. Incident ID/link.
2. Trigger.
3. Recovery status.
4. Follow-up OA-RUST issue.

## Decision Log

1. Go/no-go decisions made in this window.
2. Gates explicitly approved or denied.
3. Required follow-ups before next gate.

## Next Actions

1. Item 1.
2. Item 2.
3. Item 3.
