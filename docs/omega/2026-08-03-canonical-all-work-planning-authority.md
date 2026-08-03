# Canonical All Work planning authority implementation

- Date: 2026-08-03
- Owner issue: [Omega #222](https://github.com/OpenAgentsInc/omega/issues/222)
- Accepted plan: `docs/omega/2026-08-02-v0.2.0-all-work-dogfooding-plan.md`
- Contract owner: `packages/all-work-contract`

## Landed boundary

The generated `openagents.all_work_boundary.v1` contract now contains a
`PlanningGraph` read projection and `planning.graph.read` protocol method. The
graph represents Organization, Team, Initiative, Project Status, Project,
Project Milestone, Cycle, Workflow State, Label, Document, Project Update,
Custom View, Notification, Release Pipeline, Release Stage, and Release
Planning Record resources. It also contains Work/Issue snapshots, typed Work
relations, planning links, label links, text records, Release Scope Links,
Source Coordinates, Projection Issues, freshness, completeness, revision,
cursor, and reconciliation digest.

These are planning records. A Release Planning Record cannot authorize a
Release, approve a Release Candidate, publish an artifact, or satisfy release
evidence.

## Canonical state and commands

The Effect runtime is the single writable authority. Its versioned state holds
the graph and idempotency receipts. The request processor implements native
Work creation and updates, triage through Work State, typed relations, comments,
and planning placement. Each command requires the current optimistic revision
and an idempotency key. A repeated identical command returns its first receipt;
reuse with different bytes fails closed.

Native Work has `effect_service` source authority and performs zero GitHub
writes. Imported GitHub Work has `imported_read_only` source authority and
cannot be changed through a native command. Assignee, Agent Delegate, command
authority, claim, delegation, verification, Release, and Owner Disposition
remain separate contracts.

The file adapter validates state on load, writes owner-only temporary bytes,
and atomically renames the next state. Revision comparison fences stale replay.
The process host remains responsible for its existing single-writer lease.

## Bootstrap and loss accounting

The checked-in v0.2.0 bootstrap input contains the accepted final snapshot's
exact 28 open rows and six closed foundation rows. It also contains 42 planning
resources and 46 typed relations. Stable Work identity derives from repository and issue
number; Issue is a same-identity projection.

Reconciliation sorts delivery order, merges duplicate issue deliveries and
comment pages, retains Source Coordinates, and records a digest. The second
identical import is a no-op. Incomplete or missing pages create explicit
`page_gap` Projection Issues and preserve last-known-good rows. A subsequent
complete observation can mark a missing source unavailable without deleting
the Work identity. Unsupported, malformed, private, and missing source data are
absent or represented as explicit Projection Issues; the adapter never invents
values.

## Verification state

`packages/omega-effectd` now opens the durable authority during negotiated v2
initialization and serves it through `planning.graph.read`. The method decodes
the generated request, loads the persisted graph, rejects an ahead-of-authority
revision, and encodes the generated result. Authored in-process and real-stdio
tests require all 34 Work rows, six completed rows, Source Coordinates, Release
Scope Links, and the reconciliation digest.

The implementation includes authored contract, golden bootstrap, duplicate and
pagination, gap and unavailability, native create/update/idempotency, and
durable restart tests. Per the sequential #208 execution instruction, these
tests, generated drift checks, cross-language conformance, the real
`omega-effectd` process read smoke, and the application build are deferred to
the single final verification run. This document is implementation status, not
test evidence, issue completion, Release evidence, or owner acceptance.
