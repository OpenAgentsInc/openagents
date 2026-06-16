# Forge Factory Metric Definitions

Date: 2026-06-16
Status: Locked metric definitions and dashboard instrumentation for #5090
Related: #5088, #5090, #5091, #5101

## Purpose

The Forge factory dashboard at `/forge` must be auditable: each number needs a
clear definition, source, and provenance. This document locks the current metric
contract for the software-factory dashboard shipped in #5088 and records which
values are live today versus intentionally seeded placeholders.

## Current Sources

The dashboard currently reads two projections from the logged-in Autopilot model:

| Source                              | Loader/API boundary           | Used for                                                                  |
| ----------------------------------- | ----------------------------- | ------------------------------------------------------------------------- |
| Runs projection                     | `LoadAutopilotWorkList({})`   | stage counts, scheduled backlog, accepted outcomes, pass rate, cycle time |
| Provider-account pool projection    | `LoadProviderAccountPool({})` | Code Gen capacity and eligible-node count                                 |
| Verification, token, power, MTTR DB | not wired to `/forge` yet     | shown as seeded/absent, never as live                                     |

The UI tags every displayed number as either `live` or `seeded`. A seeded number
is a placeholder with no production authority.

## Stage Bucketing

Run states are mapped into the canonical Forge production line:

| Run state(s)                                                             | Forge stage |
| ------------------------------------------------------------------------ | ----------- |
| `scheduled`                                                              | Triage      |
| `queued_or_running`, `access_required`, `payment_required`, `paid_ready` | Code Gen    |
| `delivered`, `revision_required`                                         | Validate    |
| `accepted`, `accepted_free_slice`                                        | Release     |
| `blocked`, `rejected`, `invalid`                                         | Monitor     |

Signal, Document, and Deploy do not yet have dedicated historical projections.
Deploy currently mirrors accepted outcomes because accepted receipts are the
live delivery evidence available in the Runs projection.

## Locked Metrics

| Metric               | Definition                                                                                                                   | Current `/forge` status                                                                                                                      |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Throughput           | Accepted outcomes per loaded period: count of `accepted` + `accepted_free_slice`.                                            | Live as Release/Deploy accepted counts and "Outcomes Shipped" when Runs projection is loaded.                                                |
| Stage throughput     | Count of work orders currently bucketed in a stage.                                                                          | Live for Triage, Code Gen, Validate, Release, and Monitor. It is a snapshot count, not historical transition throughput yet.                 |
| Cycle time           | Median `updatedAt - createdAt` minutes across loaded work orders.                                                            | Live when timestamps exist. This is whole-run median cycle time until stage entry/exit event receipts exist.                                 |
| Pass rate            | `accepted / (accepted + rejected + invalid)`.                                                                                | Live when at least one accepted/rejected/invalid decision exists. Blocked, delivered, revision-required, and scheduled are excluded.         |
| Token efficiency     | Useful accepted output per model token or credit spend, reconciled with accepted-outcomes-per-kWh when compute power exists. | Seeded/absent. `/forge` has no token/power join yet.                                                                                         |
| MTTR                 | Median time from incident signal to accepted correction/deploy receipt.                                                      | Seeded/absent. Incident-resolution receipts are not wired yet.                                                                               |
| Backlog / queue burn | Scheduled backlog is `scheduled`; triage count is the Triage stage bucket, which already includes scheduled runs.            | Live. The "Runs Triaged" panel now uses the Triage stage bucket directly to avoid double-counting scheduled backlog.                         |
| Week-over-week delta | `(last 7 days created - prior 7 days created) / prior 7 days created`, rounded to percent.                                   | Live for the intake band when Runs projection has created timestamps. Direction is neutral: up means more intake; down can mean load easing. |

## Instrumentation Notes

- `dailyCreated` is a trailing 14-day created-at histogram, oldest to newest.
- The Triage and Signal sparklines use `dailyCreated` because that is the only
  real historical series currently available.
- Other per-stage sparklines remain seeded until stage transition receipts exist.
- Provider pool capacity is live only when the pool projection has loaded; until
  then Code Gen capacity remains seeded.
- Power/kWh efficiency must not be inferred from tokens alone. It needs a joined
  compute-source estimate or measurement and must preserve the uncertainty range
  from the accepted-outcomes-per-kWh methodology.

## Non-Authority Boundary

These metrics do not create deployment, billing, settlement, payout, legal,
customer-communication, or production-write authority. Forge can display
operational evidence, but the existing authority receipts remain the source of
truth for accepting work, spending credits, deploying artifacts, or paying
contributors.
