# P0 FC-4: explicit owner-local and managed-remote work routing

- Issue: #8636
- Parent: #8638
- Mobile consumer: #8597
- Status: active P0 target/workroom contract; advanced placement follows R7
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md), Revision 25

## Outcome

One owner-scoped run can place independent work units on owner-local Pylons or
managed remote Agent Computers/workrooms under an explicit typed target policy.
Mobile and Desktop select, observe, and reconcile the same target and fallback
history without owning placement or claim authority.

## Dependencies

- closed FC-1 durable run contract and FC-2 mixed-harness execution;
- #8547 real brokered Codex Agent Computer/workroom proof;
- shared identity, repository/thread, Fleet, and Khala Sync refs from R1/R2.

## Scope

1. Add per-work-unit `owner_local | managed_remote | auto` target policy with
   typed eligibility, selection, denial, and fallback history.
2. Keep authority/economic rails distinct. Owner-local subscription accounts
   are never pooled or resold; managed remote uses brokered owner grants and
   separate compute receipts.
3. Complete the minimum Codex remote-workroom path first. Additional providers
   use the same work-unit/closeout contract and require separately accepted
   capacity and isolation evidence.
4. Let placement consume explicit capacity, quota, cost class, data posture,
   repository, and task constraints. V1 may be deterministic and simple.
5. Preserve one claim registry across targets so local and remote workers
   cannot take the same unit.
6. Normalize safe progress and closeout projections while retaining target-
   specific private evidence and exact isolation/compute rung.
7. Share target policy/outcome/action IDs through Effect Native and Khala Sync;
   no mobile-only routing schema or silent automatic substitution.

## Exit

One owner-scoped run completes at least two real units concurrently: one on an
owner-local Pylon and one in a managed Agent Computer. Mobile starts or manages
the remote unit, Desktop observes/resumes it, and both clients show the same
target/fallback/claim/outcome refs. Broker refs, exact token truth or
`not_measured`, compute lifecycle, verification, safe writeback, stop, and
reclaim reconcile with zero duplicate claims or silent provider/target changes.

This exit is required by R3/R6/R7. Additional providers, elastic placement, and
cost optimization may remain explicit post-R7 follow-ons.
