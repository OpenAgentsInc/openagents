# 2026-03-06 Open PR Economy-Kernel Alignment Audit

Author: Codex
Status: complete
Scope: review of open PRs `#2978`, `#2932`, and `#2933` against the active MVP and broader economy-kernel source of truth

## Objective

Answer three questions:

1. Which of the three currently open PRs can merge cleanly now?
2. What should change in each PR before merge?
3. How should SA/SKL/AC follow-on work sync with the broader vision in `docs/plans/economy-kernel.md` rather than drifting into a second economic model?

## Sources Reviewed

Primary repo authority:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PROTOCOL_SURFACE.md`
- `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`

Broader vision authority:

- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`

Current open PRs:

- `#2978` `docs(nist): add NIST RFI NIST-2025-0035 supporting materials`
- `#2932` `Add SA-Cashu, SA-Fedimint, and SA-Guardian optional profiles`
- `#2933` `[NIP-AC] Add Cashu spending rail, SKL safety label revocation trigger, and Guardian gate integration`

Current in-repo protocol docs:

- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`
- `crates/nostr/nips/AC.md`

## Executive Recommendation

| PR | Standalone? | Merge now? | Recommendation |
| --- | --- | --- | --- |
| `#2978` | Yes | Yes, after small doc-hygiene edits | Merge first. Keep it explicitly archival and non-normative. |
| `#2932` | Only as a speculative profile | No | Do not merge into the active protocol surface as-is. Recast as a non-normative profile or redesign against kernel authority rules first. |
| `#2933` | No | No | Split it. It currently mixes rail expansion, trust/auth changes, audit/delegation semantics, and an ecosystem-specific profile in a way that conflicts with the kernel source of truth. |

## Highest-Severity Findings

1. `#2932` and `#2933` change normative SA/SKL/AC semantics without updating the repo's locked protocol-surface contract.
2. Both protocol PRs use Nostr events as if they can directly carry authority for approvals, revocations, spend gating, and cancellation. The economy-kernel plan explicitly forbids that.
3. `#2933` defines `kind:39250` audit entries as addressable, which makes a supposedly forensic audit stream mutable.
4. Both protocol PRs move the repo toward multi-rail spending before the active MVP wallet lane and custody model are ready for that complexity.
5. `#2978` is the only PR that is actually isolated from current implementation behavior, but it still needs framing so submission materials do not get mistaken for product or protocol source of truth.

## Alignment Rules From Current Source Of Truth

These are the rules the protocol work needs to obey if it is going to stay aligned with the current repo direction.

### 1. MVP payment truth is still Spark-first and Lightning-first

`docs/MVP.md` is explicit:

- provider earnings settle into the built-in Spark wallet first,
- wallet updates shown in UI must be authoritative,
- withdrawal flows through paying a Lightning invoice,
- MVP online mode is compute-provider only.

That means alternative rails may be future-compatible, but they are not current product truth.

### 2. Economy-kernel authority is HTTP-only

`docs/plans/economy-kernel.md` is explicit:

- all authority mutations happen over authenticated HTTP,
- Nostr and Spacetime are coordination/projection only,
- receipts are the source of truth for settlement, underwriting, and incident replay,
- wallet executor is the custody boundary.

So any SA/SKL/AC artifact published on Nostr can be:

- a coordination message,
- a public artifact,
- or evidence for a later authority action,

but it cannot itself be the canonical mutation of money, credit, verdict, warranty, or liability state.

### 3. Proto-first still applies

The kernel plan says externally observable fields, states, and receipts must be added in proto first and only then reflected in markdown. `#2932` and `#2933` do the reverse.

### 4. The protocol surface is currently frozen

`docs/PROTOCOL_SURFACE.md` and `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` together define a locked in-repo surface. New kinds or new required semantics need:

- explicit protocol-surface updates,
- implementation-plan updates,
- and a credible code/test follow-on path.

Neither protocol PR does that.

## PR-by-PR Analysis

## PR `#2978` `docs(nist): add NIST RFI NIST-2025-0035 supporting materials`

### What is good

- It is docs-only.
- It no longer carries the protocol-surface edits from `#2933`.
- It is genuinely reviewable on its own.
- Keeping the NIST submission bundle separate from live protocol debates is the right split.

### What should change before merge

1. Rename the added files to normal markdown filenames.

Current filenames have spaces and no `.md` suffix:

- `docs/nist/rfi-executive summary`
- `docs/nist/rfi-full-response`
- `docs/nist/rfi-iam-comparison`
- `docs/nist/technical-appendix`

They should become something like:

- `docs/nist/rfi-executive-summary.md`
- `docs/nist/rfi-full-response.md`
- `docs/nist/rfi-iam-comparison.md`
- `docs/nist/technical-appendix.md`

This matters for discoverability, consistent repo conventions, and GitHub rendering behavior.

2. Add a short `docs/nist/README.md` or top-of-file disclaimer that says these are archival submission materials, not normative repo spec.

Without that, readers can easily misread the bundle as current implementation or current protocol authority.

3. Soften or contextualize claims like "actively being implemented and deployed."

Those statements may be appropriate in a submitted RFI package, but inside this repo they should be read as submission-time claims, not as the current source of truth for the MVP repo.

### Merge recommendation

Merge this one first after the small doc-hygiene edits above.

If the goal is strict minimal churn, I would still consider it mergeable as-is because it is isolated from code and protocol behavior. But the rename + disclaimer pass is worth doing before merge.

## PR `#2932` `Add SA-Cashu, SA-Fedimint, and SA-Guardian optional profiles`

### What is useful in it

- It is directionally consistent with a future multi-rail world.
- It tries to keep most changes additive rather than breaking.
- It is closer to a profile than a core rewrite, which is the right instinct.

### Why it should not merge as-is

1. It expands the normative SA surface without updating the locked protocol-surface docs.

It introduces new kinds and tags, but `docs/PROTOCOL_SURFACE.md` still freezes SA at:

- `39200`
- `39201`
- `39202`
- `39203`
- `39210`
- `39211`
- `39220`
- `39221`
- `39230`
- `39231`

If we merge `#2932` directly, the repo immediately becomes internally inconsistent.

2. It treats guardian approval and envelope-linked spending as relay-native authority.

Examples:

- tick requests carry `approval_required`,
- runners must block on `kind:39213`,
- envelope-linked budgets are enforced from SA-layer tags.

That is not aligned with the kernel direction. In the kernel source of truth:

- approval thresholds,
- envelope commit/settle/revoke,
- and spend authorization effects

must live in authenticated authority actions with receipts. A Nostr event can be evidence or coordination, but not the mutation itself.

3. It mixes unit semantics in a way that will cause ambiguity.

The same `budget` tag is described as:

- sats for Lightning,
- then "Cashu msats from mint",
- then Fedimint ecash,
- then envelope-linked spending.

If the rail changes the denomination or unit semantics, the tag schema needs an explicit amount/unit model. Otherwise the same numeric field means different things in different rails.

4. It pushes multi-rail provider settlement ahead of the current MVP wallet truth model.

The active MVP is:

- Spark wallet first,
- truthful wallet-backed UI,
- Lightning withdraw path,
- compute-provider lane only.

The PR is useful as future design input, but not as current normative behavior.

5. It adds more than the title suggests.

The SA change also expands `kind:39220` payment proof semantics and introduces additional rail-specific proof expectations, including `bolt12`. That is broader than "optional guardian/Cashu/Fedimint profiles."

### How to realign it

If we want to preserve the work, the cleanest path is:

1. Move it out of core `SA.md` and into a non-normative profile doc.
2. Rephrase all guardian and envelope-linked behavior so Nostr events are coordination/evidence only.
3. Add the authority mirror explicitly:
   - approval receipt
   - envelope commit receipt
   - settlement receipt
   - typed denial/withhold reasons
4. Defer provider-facing multi-rail settlement semantics until the wallet-executor / TreasuryRouter model is ready for it.
5. Only merge into the active protocol surface after:
   - `docs/PROTOCOL_SURFACE.md` is updated,
   - `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` is updated,
   - and the code/test plan exists.

### Recommendation

Do not merge this PR as-is.

If you want to keep momentum, recut it as:

- a profile doc under `docs/` or a protocol appendix,
- clearly marked non-normative,
- with explicit kernel-authority caveats.

## PR `#2933` `[NIP-AC] Add Cashu spending rail, SKL safety label revocation trigger, and Guardian gate integration`

### What is useful in it

- It surfaces real design questions:
  - alternative spending rails,
  - safety-label-driven enforcement,
  - authorization grants,
  - reversible high-risk actions,
  - auditability and delegation.
- It also correctly noticed that the NIST bundle should be split out.

### Why it should not merge as-is

1. It is too many PRs in one.

This branch currently combines at least four different design tracks:

- AC rail expansion and guardian gating,
- NIST-driven SKL auth/trust additions,
- SA audit/delegation/hold-period behavior,
- and `docs/TRUE_NAME_INTEGRATION_PROFILE.md`, which is an ecosystem-specific profile.

That makes review shallow and merge sequencing unsafe.

2. It conflicts with the kernel authority model in multiple places.

The clearest example is `kind:39246` Cancel Spend. In the PR, cancellation is a relay event published by a guardian or issuer during a hold window. In the kernel plan, cancellation, refund, withhold, and rollback are authority actions with explicit receipts, deadlines, and reason codes.

The same problem appears in:

- permission grants being treated as enforceable authorization contracts,
- guardian gating being treated as relay-native spend control,
- and sub-agent delegation carrying economic scope without kernel contract hooks.

3. `kind:39250` as an addressable audit trail entry is the wrong storage shape.

Audit history should be append-only. Making individual audit entries addressable means they can be replaced, which is the opposite of the kernel's receipt/evidence discipline.

If audit artifacts are going to live on Nostr at all, they should be append-only and clearly separated from canonical authority receipts.

4. It introduces new kinds and semantics with no protocol-surface synchronization.

Examples include:

- `33410`
- `33411`
- `33420`
- `39246`
- `39250`
- `39260`

Those do not appear in the current frozen protocol surface or implementation plan.

5. It introduces trust-profile vocabulary that the active repo does not yet own.

`docs/TRUE_NAME_INTEGRATION_PROFILE.md` is Satnam-specific and ecosystem-specific. Nothing else in the active MVP repo currently defines or implements that profile. In this pruned MVP repo, that belongs in backroom or in a clearly external profile package unless and until there is an active implementation owner.

6. It overreaches the current custody model.

The PR is trying to specify reversibility windows, permission grants, and sub-delegation economics before the kernel-side authority path exists in this repo. That is backwards relative to the current full-vision rule: proto and authority semantics first, relay artifacts second.

### How to salvage it

Split it into separate follow-ons:

1. A docs-only trust-metadata/profile PR:
   - optional organizational identity metadata,
   - optional assurance-tier metadata,
   - optional challenge-response profile,
   - all clearly non-authoritative.

2. A kernel-alignment design PR:
   - how hold periods, cancels, reversals, and revokes map to HTTP authority actions,
   - which receipt families are needed,
   - how Nostr artifacts reference those receipts instead of replacing them.

3. A provenance/audit PR:
   - how delegation, audit chain, and parent/child execution map to kernel evidence and contract lineage,
   - whether any of that needs new Nostr kinds at all.

4. A separate ecosystem-profile PR, if still desired:
   - keep `TRUE_NAME` / Satnam-specific conventions out of the core MVP docs unless there is explicit adoption and ownership.

### Recommendation

Do not merge this PR as-is.

It should be split, and the economic-state parts should be redesigned from the kernel authority model outward.

## Relationship Between `#2932` And `#2933`

They should not both move forward unchanged.

Practical reading:

- `#2932` is a speculative SA profile branch for alt rails and guardian behavior.
- `#2933` is a larger AC/SKL/SA redesign branch that reaches even further into authority and trust semantics.

So the right sequencing is not "merge both in order." The right sequencing is:

1. merge `#2978`,
2. decide which small, non-authoritative parts of `#2932` or `#2933` are worth salvaging,
3. rewrite those pieces against the kernel authority model before they touch the active protocol surface.

## Concrete Change List By PR

## For `#2978`

- Rename files to `.md` with no spaces.
- Add `docs/nist/README.md`.
- Add one sentence that these are archival submission materials, not repo source of truth.
- Optionally add a note that protocol/product authority remains in `docs/MVP.md` and `docs/plans/economy-kernel.md`.

## For `#2932`

- Move profile semantics out of core `SA.md`, or explicitly label them non-normative.
- Recast `kind:39212` / `kind:39213` as coordination/evidence rather than authority.
- Remove or defer rail-specific payment-proof requirements that imply canonical settlement truth on Nostr.
- Normalize amount-unit semantics across Lightning/Cashu/Fedimint.
- Update `docs/PROTOCOL_SURFACE.md` and `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` if any part becomes active surface.

## For `#2933`

- Split the branch by concern.
- Remove `docs/TRUE_NAME_INTEGRATION_PROFILE.md` from the MVP protocol merge path.
- Redesign cancel/hold/revoke semantics as kernel authority actions plus receipts.
- Redesign audit artifacts as append-only evidence, not addressable mutable entries.
- Update protocol-surface and implementation-plan docs before any new kinds become active.

## Recommended Merge Order

1. `#2978`, after the small doc-hygiene changes.
2. No merge for `#2932` or `#2933` until there is a kernel-aligned redesign.
3. After that redesign, re-open smaller PRs in this order:
   - doc framing / optional profile docs
   - protocol-surface update
   - code/test support

## Bottom Line

The NIST PR is the only one that is truly standalone, and I would merge it first after a quick cleanup pass.

The other two PRs are not wrong in spirit, but they are currently written as if Nostr markdown is the economic source of truth. The active repo and the broader economy-kernel plan say the opposite:

- Nostr carries public artifacts, coordination, and evidence.
- Authority lives in authenticated HTTP services.
- Wallet executor is the custody boundary.
- Receipts are the canonical economic truth.

So the correct move is not to reject the ideas. It is to re-express them in that architecture before they land in the active protocol surface.
