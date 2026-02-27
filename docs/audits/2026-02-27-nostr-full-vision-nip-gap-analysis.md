# 2026-02-27 Nostr Full-Vision NIP Gap Analysis

## Scope

Assess whether additional NIPs are needed to realize the full OpenAgents vision described in:

- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/core/SYNTHESIS.md`
- `/Users/christopherdavid/code/nips/README.md`

Context baseline in this repo:

- `docs/MVP.md`
- `docs/PROTOCOL_SURFACE.md`
- `crates/nostr/core/src/lib.rs`
- `crates/nostr/nips/{SA.md,SKL.md,AC.md}`

## Current Baseline (In-Repo)

Implemented protocol modules in `crates/nostr/core` today:

- NIP-01, NIP-06, NIP-09, NIP-26, NIP-28, NIP-32, NIP-40, NIP-44, NIP-59, NIP-90, NIP-99
- OpenAgents drafts: NIP-SA, NIP-SKL, NIP-AC

Not implemented in this repo (but relevant to full vision from SYNTHESIS):

- NIP-11, NIP-17, NIP-34, NIP-42, NIP-47, NIP-57, NIP-60, NIP-61, NIP-65, NIP-66, NIP-69, NIP-77, NIP-78, NIP-87, NIP-89, NIP-98

## Executive Assessment

SA/SKL/AC are necessary but not sufficient for the full architecture in `SYNTHESIS.md`.

Main conclusion:

1. Most remaining capability can and should be built on existing canonical NIPs.
2. We should avoid adding new large custom NIPs unless an interoperability gap is truly uncovered.
3. Two narrow gaps likely justify future new draft NIPs (listed below), but only after shipping the missing canonical NIPs first.

## Capability Map: Vision vs NIP Coverage

## 1) Agent Runtime, Compute, and Skill Economy

Already covered well:

- SA lifecycle and trajectory model (NIP-SA)
- Skill registry/trust/revocation (NIP-SKL + NIP-32/NIP-09/NIP-99 linkage)
- Bounded credit lifecycle (NIP-AC)
- Compute request/response core (NIP-90)

Gaps:

- Provider/service discovery standardization for agent services is missing without NIP-89.
- Payment linking in public market flows is weaker without NIP-57 (zaps) and/or NIP-47 wallet control.

Recommendation:

- Add NIP-89 and NIP-57 in near-term core roadmap.

## 2) Payments, Treasury, and Exchange Vision

`SYNTHESIS.md` explicitly leans on:

- NIP-47 (wallet control)
- NIP-60, NIP-61, NIP-87 (Cashu/nutzap/mint discovery rails)
- NIP-69 (P2P order events)
- NIP-32 (already present) and NIP-90/NIP-89 (partially present)

Gaps:

- Exchange and treasury flows in SYNTHESIS are not interoperable without NIP-60/61/87/69/47 support.
- AC can model credit, but cannot by itself replace wallet-state and multi-rail settlement standards.

Recommendation:

- Treat NIP-47 + NIP-60/61/87 + NIP-69 as required for the "Treasury Agent / Exchange" part of the vision.

## 3) Communication, Privacy, Relay Ops

Gaps:

- NIP-17 private messaging is absent (NIP-04 is deprecated in the canonical README and should not be adopted).
- NIP-42 relay auth and NIP-65 relay list metadata are absent.
- NIP-11 relay capability discovery and NIP-66 liveness/discovery are absent.
- NIP-77 negentropy is absent (important for large-scale replay/sync correctness).

Recommendation:

- Add NIP-17, NIP-42, NIP-65 as priority for secure reliable agent operation.
- Add NIP-11/NIP-66/NIP-77 as scale and operability enhancements.

## 4) Git-Native Collaboration Vertical

`SYNTHESIS.md` calls out NIP-34 as a core substrate for GitAfter.

Gap:

- No in-repo NIP-34 support today.

Recommendation:

- If GitAfter remains in retained roadmap, NIP-34 becomes required, not optional.

## 5) HTTP and External API Commerce

`SYNTHESIS.md` references L402/pay-per-call flows.

Gap:

- NIP-98 HTTP auth is absent, which limits clean authz/authn interop for paid API calls.

Recommendation:

- Add NIP-98 as a required dependency for robust L402-style external commerce.

## Prioritized Roadmap (Existing NIPs First)

## Tier A: Required for Full Vision Coherence

- NIP-89 (service/handler discovery)
- NIP-57 (zaps)
- NIP-47 (wallet connect/control)
- NIP-17 (private DMs)
- NIP-42 (relay auth)
- NIP-65 (relay list metadata)
- NIP-98 (HTTP auth)

## Tier B: Required for Treasury/Exchange Realization

- NIP-60 (Cashu wallet events)
- NIP-61 (nutzaps)
- NIP-87 (mint discoverability)
- NIP-69 (P2P order events)

## Tier C: Required for Scale and Ecosystem Breadth

- NIP-11 (relay information documents)
- NIP-66 (relay liveness/discovery)
- NIP-77 (negentropy syncing)
- NIP-34 (Git-native collaboration rail)
- NIP-78 (app-specific data, if needed for portable runtime state/config)

## Should We Write New NIPs Beyond SA/SKL/AC?

Short answer: not yet for broad product surfaces. Existing NIPs already cover most of the remaining vision.

Potential narrow exceptions worth drafting only after Tier A/B:

1. **Bifrost Threshold Coordination Profile/NIP**
- Problem: no canonical NIP for threshold signing coordination/session semantics across operators.
- Why it matters: true sovereign identity interop depends on reproducible threshold coordination, not app-private wire formats.

2. **Trajectory Proof Interop Profile/NIP**
- Problem: SA defines trajectory event semantics, but not a broader canonical replay bundle/proof envelope that third-party tools can verify uniformly.
- Why it matters: cross-client auditability and objective proof portability are central to OpenAgents' trust model.

Both should be treated as profile-first efforts; promote to new NIP drafts only if profile constraints prove insufficient.

## Recommendation Summary

1. Do not create another large custom "marketplace" NIP now.
2. Implement the missing canonical NIPs above in priority order.
3. Keep SA/SKL/AC focused on agent lifecycle, skill trust, and bounded credit.
4. Re-evaluate new NIP drafting only after Tier A/B lands and real interop pain is observed.

This path keeps us aligned with Nostr ecosystem gravity while still preserving OpenAgents-specific innovation where standards are genuinely missing.
