# Decentralized Training Participant & Scale Methodology

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19
Branch: `assault-pylon`
Owner sign-off: REQUIRED before any promise green flip (this doc is evidence
assembly only; it does not flip any promise).

## Purpose

Several Pylon product promises are blocked on a missing, written, dereferenceable
methodology for how participant / contributor counts are measured for public
decentralized-training claims. Specifically:

- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` →
  `blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`
- `pylon.largest_decentralized_training_claim.v1` →
  `blocker.product_promises.largest_training_participant_methodology_missing`

The counting rule is already enforced in code as a derived, provenance-labelled
metric. This document is the public, dereferenceable statement of that rule so
the methodology blocker has a citeable home. It documents the *rule and where it
lives*; it does NOT assert any scale claim, any "anybody earns" copy, or any
largest-run comparison. Those remain red and owner-gated.

## The qualified-contributor counting rule (authoritative)

A participant is counted as a **qualified contributor** on a public training run
**only if all of the following hold**:

1. The contributor was **admitted** to the run (holds or held a real window
   lease against the run), and
2. The contributor produced **accepted, replay-verified useful work** — i.e. a
   Worker-D1 `exact_trace_replay` verification challenge in state `Verified`,
   joined to one of that contributor's run leases, and
3. The contributor has a **public-safe, provider-confirmed settlement receipt
   ref** linked to that run.

The following are **never** counted:

- Raw registrations / first-run installs.
- Stale or live heartbeats with no accepted verified work.
- Pending, offered, claimed, wallet-side, or simulation-only
  (`realBitcoinMoved:false`) receipts when counting *real-paid* contributors.

This is the rule implemented for the live metric, verbatim from the metric's own
`description` field:

> "Qualified contributor count equals admitted contributors with accepted,
> replay-verified useful work and public-safe provider-confirmed settlement
> receipt refs linked to this run. It is derived from Worker D1 verified
> exact_trace_replay challenges joined to run leases plus provider-confirmed
> settled receipt projections; raw registrations and stale heartbeats never
> count."

## Where the rule is enforced in code (dereferenceable)

- `apps/openagents.com/workers/api/src/training-run-window-authority.ts`
  - `qualifiedContributorCount` metric construction (the description quoted
    above) and its `sourceRefs` (the contributor refs plus their settlement
    receipt refs).
  - `participantCountRule` manifest field on the window-seal contract, which lets
    each run carry its own per-run participant-count manifest text.
- `apps/openagents.com/workers/api/src/public-pylon-stats.ts`
  - The public aggregator that sums `summary.metrics.qualifiedContributorCount`
    across runs and carries the per-contributor `sourceRefs`.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts`
  - The standalone, pure **conformance verifier** that turns this prose rule
    into an auditable gate: `verifyQualifiedContributorMethodology` recomputes a
    run's qualified count from per-contributor evidence and confirms a published
    count is neither inflated nor under-counted. It explicitly rejects
    simulation-only (`realBitcoinMoved:false`), non-`settled`, and
    wallet-side/not-provider-confirmed receipts — closing the gap where the
    in-line `qualifiedContributorRefs` join trusts its caller to pre-filter the
    receipt-ref map. It also enforces **cross-contributor evidence integrity**
    across every prong: two counted contributors with distinct `pylonRef`s do not
    conform if they share the SAME admitted window lease
    (`shared-lease-across-contributors`), the SAME replay-verified exact_trace
    work challenge (`shared-verified-work-across-contributors`), or the SAME
    provider-confirmed real-bitcoin settlement receipt
    (`shared-settlement-receipt-across-contributors`). Distinct `pylonRef`s are
    necessary but not sufficient — a single lease, one piece of verified work, or
    one real Bitcoin movement cannot back two "distinct independent contributors".
    It also exposes `parseQualifiedContributorMethodologyInput`, the pure
    **untrusted-input parse boundary** an auditor uses to load a real captured
    evidence document (JSON) before verifying it: it validates structure/types
    and enforces a closed key allowlist at every level (document, contributor,
    settlement receipt), rejecting leak-prone extra fields (e.g. a raw address or
    balance) with path-qualified errors, and returns the typed input only when
    the document is sound. This makes the "run the verifier against the live
    run's real evidence" step safe to do from a file rather than hand-built
    objects. For that step it also exposes
    `verifyQualifiedContributorMethodologyDocument`, the single safe entry that
    **fuses parse → verify** over an untrusted document: it returns
    `{ ok:false, errors }` with path-qualified parse reasons (verifying nothing)
    or `{ ok:true, verdict }` with the conformance result. This removes the
    footgun of skipping the parse boundary by type-asserting a raw document
    straight into `verifyQualifiedContributorMethodology` — the boundary is
    unbypassable for the real-evidence run. The cross-contributor shared-ref
    checks compare each contributor's **distinct** refs (deduped within that
    contributor) before flattening, so a single legitimate contributor whose own
    evidence harmlessly lists a ref twice is never falsely flagged
    `*-across-contributors`, while genuine reuse across two contributors is still
    caught. A checked-in, public-safe **evidence-document SHAPE TEMPLATE**
    (`src/fixtures/qualified-contributor-methodology-evidence.template.json`, see
    its README) gives auditors the exact document shape the real run's evidence
    file must take, and the test suite exercises the actual **file → parse →
    verify** path against it (every other test builds the document in-memory) — so
    the "run against the live run's real evidence file" harness is proven before
    the real file is dropped in. The template is synthetic (placeholder refs only)
    and asserts no real claim. Covered by
    `qualified-contributor-methodology.test.ts` (39 tests, wired into
    `check:deploy`).

## Public dereference path

The count is enumerable and dereferenceable per run:

- `GET /api/public/training/runs/{runRef}` →
  `summary.metrics.qualifiedContributorCount` (value + description + sourceRefs).
- `GET /api/public/training/runs/{runRef}/settlements` → the enumerable per-run
  settled feed, where each counted contributor's `realBitcoinMoved:true`
  settlement row can be resolved.

For the live run `run.tassadar.executor.20260615` at the time of writing,
`qualifiedContributorCount` is exactly the small bounded set described by
`training.decentralized_training_launch.v1` (two distinct independent
contributors paid real Bitcoin, 1,005 sats real total). This methodology does
not change that number; it only documents how it is derived.

## What this does NOT establish (boundary)

- It does NOT assert network scale, "hundreds paid", "paid at scale", or any
  largest-run comparison.
- It does NOT widen install-platform coverage. Current install evidence is
  macOS + Linux; Windows/WSL is a deliberate owner scope-out (see
  `apps/pylon/docs/platform-support.md`), and broad "anybody on any platform"
  copy stays blocked.
- It does NOT flip any promise. A green flip for
  `pylon.consumer_compute_earns_bitcoin_self_serve.v1` or
  `pylon.largest_decentralized_training_claim.v1` still requires the remaining
  blockers cleared with their own receipts and an owner-signed, receipt-first
  upgrade per `proof.claim_upgrade_receipts.v1`.

## Green-readiness status for the methodology blockers

- `consumer_compute_self_serve_scale_methodology_missing`: the methodology is now
  written and dereferenceable here, enforced in code with a live public route,
  and additionally backed by a standalone conformance verifier
  (`qualified-contributor-methodology.ts`) that makes the rule independently
  auditable and rejects simulation/non-settled/wallet-side receipts. **Green-ready
  on this specific blocker pending owner review**; the honest remaining step is to
  run the verifier against the live run's actual per-contributor evidence bundle
  and cite its `conforms:true` verdict before any broad earning copy. The other
  two blockers on that promise (Windows/WSL coverage, Spark-helper autostart
  receipt) are tracked separately and stay listed.
- `largest_training_participant_methodology_missing`: the counting rule is
  written here. The *comparison* baseline is documented separately in
  `docs/training/2026-06-19-comparable-decentralized-training-runs-research.md`.
  Even with both docs, the largest-run promise stays red on
  `comparable_training_run_evidence_missing` (we do not yet have a comparable
  run at the benchmark count) and `public_training_contributor_receipts_missing`
  (we have two counted contributors, not a benchmark-beating count).
