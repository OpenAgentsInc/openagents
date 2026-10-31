# 195. Check a peer's work

> **Status: unimplemented — design draft.** Nothing here runs yet.

A mesh must check that peers did the work they report. This chapter re-executes a sampled fraction of a peer's work on a trusted node and pays only for work that passes.

Psionic's fixed reduction order and batch invariance make outputs reproducible bit for bit under one backend profile. Across backends, the comparison uses that profile's stated tolerance, and an inexact match is evidence, not proof.

## What this chapter covers

- **Sampled re-execution.** A trusted node re-runs a sampled fraction of a peer's requests.
- **Exact or within tolerance.** Results match exactly only when both sides run the same backend profile. Otherwise they are compared within that profile's tolerance, using limits from [chapter 219](ch219-how-far-apart-may-two-honest-answers-be.md).
- **Signed usage receipts per stage.** Each stage signs a receipt for its share of the work.
- **A pricing policy.** Prices are an `#authority` file, as in [chapter 180](ch180-observe-meter-and-price.md).
- **Settlement outside Psionic.** Payment uses Bitcoin through the wallet. Paying peers is an owner decision.
- **The existing comparison.** `psionic/lib/mesh/compare.coax` already holds a pure comparison policy for a toy backend scope; this chapter extends that pattern to stage work.

## Dependencies

- [Chapter 26](ch026-check-the-mesh.md): receipts, sampled checks and recomputable totals for whole decisions.
- [Chapter 219](ch219-how-far-apart-may-two-honest-answers-be.md): honest distance limits between backends.
- [Chapter 234](ch234-exact-replay-as-a-verification-class.md): exact replay verdicts.
- [Chapter 180](ch180-observe-meter-and-price.md): usage records and pricing.
- Paying peers or accepting payment is an owner decision recorded in `NEEDS_OWNER.md`.

## Done means

- A peer that returns a deliberately wrong stage output fails the sampled check.
- Same-profile re-execution matches bit for bit on the fixture requests.
- A cross-backend comparison reports its distance and the tolerance it used.
- Every stage receipt verifies, and a reader can recompute totals from them.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [Private semantic comparison](../docs/serving/mesh-comparison.md)
