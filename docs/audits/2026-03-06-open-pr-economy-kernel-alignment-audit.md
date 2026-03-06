# 2026-03-06 Open PR Economy-Kernel Alignment Audit

- Author: Codex
- Status: complete
- Scope: review of open PRs `#2978`, `#2932`, and `#2933` against the active SA/SKL/AC NIP surface already written to power app features, with the broader economy-kernel docs treated as downstream integration targets rather than override authority for the NIPs

## Objective

Answer three questions:

1. Which of the three currently open PRs can merge cleanly now?
2. What should change in each PR before merge?
3. How should SA/SKL/AC follow-on work stay coherent with the broader economy-kernel vision without forcing kernel assumptions over the established NIP suite?

## Sources Reviewed

Primary repo authority:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PROTOCOL_SURFACE.md`
- `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`
- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`
- `crates/nostr/nips/AC.md`

Broader vision reference:

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

## Framing Correction

This audit has been updated to reflect the intended hierarchy more accurately:

1. The SA/SKL/AC NIPs in `crates/nostr/nips/` were written specifically to power parts of the app and are already partially implemented in `crates/nostr/core` and integrated in `apps/autopilot-desktop`.
2. The default review posture should therefore be: hold the NIPs relatively stable, evaluate PRs primarily as changes to those established NIPs, and adapt economy-kernel planning downstream where possible.
3. Nostr being a relay / websocket protocol is not itself a reason to reject NIP changes. The relevant review questions are whether a PR improves or degrades the NIP suite and whether it creates a coherent protocol surface across SA, SKL, and AC.

## Executive Recommendation

| PR | Standalone? | Merge now? | Recommendation |
| --- | --- | --- | --- |
| `#2978` | Yes | Yes | Merge first. The recommended cleanup has already been applied on the PR branch. |
| `#2932` | Only as a targeted SA revision or clearly isolated profile | No | Do not merge into the active NIP surface as-is. Narrow it to a coherent SA delta, or isolate it as an explicit optional profile. |
| `#2933` | No | No | Split it. It mixes several independent changes to the established NIPs and app-facing protocol surface in one branch. |

## Highest-Severity Findings

1. The repo's SA/SKL/AC NIPs are already app-facing protocol documents with partial implementation behind them, so `#2932` and `#2933` are protocol migrations, not greenfield exploration.
2. `#2932` and `#2933` overlap and compete to redefine SA / AC behavior rather than presenting one coherent NIP delta.
3. `#2933` defines `kind:39250` audit entries as addressable, which makes a supposedly forensic audit stream mutable even on its own Nostr-native terms.
4. Both protocol PRs add multiple new kinds and semantics at once, which makes it hard to judge whether each addition actually improves the NIP suite on its own merits.
5. Both protocol PRs expand alternative rails and settlement semantics well beyond the current MVP wallet integration, so merging them now would create a larger gap between the active protocol docs and the active app behavior.

## Alignment Rules From Current Source Of Truth

These are the rules the protocol work needs to obey if it is going to stay aligned with the current repo direction.

### 1. SA/SKL/AC NIPs are already part of the app surface

`docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md` is explicit that:

- SA, SKL, and AC modules exist in `crates/nostr/core`,
- tests exist around those modules,
- and `apps/autopilot-desktop` already includes typed SA/SKL/AC lanes.

So changes to these NIPs should be reviewed first as changes to the app's established protocol layer.

### 2. Hold the NIPs relatively stable and adapt the kernel downstream

The default posture should be:

- preserve the current NIP suite unless there is a strong protocol reason to change it,
- evaluate proposed deltas in terms of whether they improve the NIPs and the app-facing surface,
- and, once accepted, adapt the broader economy-kernel docs and proto plans to fit those accepted NIP shapes where possible.

The kernel planning docs are still useful as integration targets, but they should not be treated as an automatic veto over the NIP suite.

### 3. MVP payment truth is still Spark-first and Lightning-first

`docs/MVP.md` is explicit:

- provider earnings settle into the built-in Spark wallet first,
- wallet updates shown in UI must be authoritative,
- withdrawal flows through paying a Lightning invoice,
- MVP online mode is compute-provider only.

That does not make alternative rails invalid, but it does mean they should be merged carefully and usually as future-facing or profile-level expansions until the app actually supports them.

### 4. Broader economy-kernel docs should be made to fit accepted NIP changes

Where the economy-kernel docs currently assume a different authority or settlement framing, that should generally be treated as a downstream integration problem unless there is a strong reason to change the NIP itself.

## PR-by-PR Analysis

## PR `#2978` `docs(nist): add NIST RFI NIST-2025-0035 supporting materials`

### What is good

- It is docs-only.
- It no longer carries the protocol-surface edits from `#2933`.
- It is genuinely reviewable on its own.
- Keeping the NIST submission bundle separate from live protocol debates is the right split.
- The recommended cleanup has already been applied on the PR branch.

### Cleanup applied on the PR branch

The following cleanup has already been applied:

1. The files were renamed to normal markdown filenames:
   - `docs/nist/rfi-executive-summary.md`
   - `docs/nist/rfi-full-response.md`
   - `docs/nist/rfi-iam-comparison.md`
   - `docs/nist/technical-appendix.md`
2. `docs/nist/README.md` was added to frame the bundle as archival submission material.
3. Each document now carries a short archival/non-normative note.

### Merge recommendation

Merge this one first.

## PR `#2932` `Add SA-Cashu, SA-Fedimint, and SA-Guardian optional profiles`

### What is useful in it

- It is directionally consistent with a future multi-rail world.
- It tries to keep most changes additive rather than breaking.
- It is closer to a profile than a core rewrite, which is the right instinct.

### Why it should not merge as-is

1. It expands the established SA surface very broadly in one pass.

It introduces new kinds and tags beyond the current SA kind table:

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

That is not automatically wrong, but it is a large SA revision and should be justified as one coherent protocol move.

2. It overlaps with `#2933` on guardian and envelope-related semantics, which means merging it independently would likely create two competing SA/AC stories.

Examples:

- tick requests carry approval tags here,
- `#2933` adds related guardian and envelope gating elsewhere,
- both branches touch the SA/AC boundary.

That needs one coherent protocol decision, not two partially overlapping ones.

3. It mixes unit semantics in a way that will cause ambiguity.

The same `budget` tag is described as:

- sats for Lightning,
- then "Cashu msats from mint",
- then Fedimint ecash,
- then envelope-linked spending.

If the rail changes the denomination or unit semantics, the tag schema needs an explicit amount/unit model. Otherwise the same numeric field means different things in different rails.

4. It pushes multi-rail provider settlement semantics much further than the current core SA text.

That may be a valid future direction, but in NIP terms it argues for profile-gating or narrower scope unless we explicitly want SA core to absorb that complexity now.

5. It adds more than the title suggests.

The SA change also expands `kind:39220` payment proof semantics and introduces additional rail-specific proof expectations, including `bolt12`. That is broader than "optional guardian/Cashu/Fedimint profiles."

### How to realign it

If we want to preserve the work, the cleanest path is:

1. Decide explicitly which parts are true SA core changes and which parts are optional profiles.
2. Reconcile it with `#2933` so there is one guardian / envelope / alternative-rail story across the NIP suite.
3. Normalize budget amount and unit semantics before merging any of the new `budget` forms.
4. Defer or clearly profile-gate provider-facing multi-rail settlement semantics until the app actually supports them.
5. Only merge after the core-vs-profile boundary is made explicit in the NIP text itself.

After that, any necessary economy-kernel planning changes should be made downstream to fit the accepted SA changes.

### Recommendation

Do not merge this PR as-is.

If you want to keep momentum, recut it as:

- a profile doc under `docs/` or a protocol appendix,
- clearly marked non-normative if it is not intended as a core SA revision,
- or a narrower SA-only protocol delta if it is intended to revise SA proper.

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

2. It introduces a large set of new kinds and semantics in one branch.

Examples include:

- `33410`
- `33411`
- `33420`
- `39246`
- `39250`
- `39260`

That breadth makes it hard to evaluate whether each new kind belongs in SKL, SA, or AC core.

3. `kind:39250` as an addressable audit trail entry is the wrong storage shape.

Audit history should be append-only. Making individual audit entries addressable means they can be replaced, which is a poor fit for the semantics the PR itself is trying to establish.

4. It overlaps with `#2932` rather than cleanly superseding it.

The branch touches SA, SKL, and AC at once, including guardian and rail semantics that are also being changed in `#2932`. That makes it hard to tell what the intended stable NIP suite actually is.

5. It introduces trust-profile vocabulary that the active repo does not yet own.

`docs/TRUE_NAME_INTEGRATION_PROFILE.md` is Satnam-specific and ecosystem-specific. Nothing else in the active MVP repo currently defines or implements that profile. In this pruned MVP repo, that belongs in backroom or in a clearly external profile package unless and until there is an active implementation owner.

6. It overreaches the scope of a clean single NIP revision.

The PR is trying to specify reversibility windows, permission grants, and sub-delegation economics all at once. Those may each be worthwhile, but they should be justified independently rather than bundled together.

### How to salvage it

Split it into separate follow-ons:

1. An AC-focused PR:
   - alternative spend rails,
   - hold period / cancel semantics,
   - and nothing else.

2. A SKL-focused PR:
   - optional organizational identity metadata,
   - optional assurance-tier metadata,
   - optional challenge-response profile,
   - permission grants if they are truly part of SKL core.

3. An SA-focused PR:
   - how delegation, audit chain, and parent/child execution map to kernel evidence and contract lineage,
   - whether any of that needs new Nostr kinds at all,
   - and whether `39250` should be regular rather than addressable.

4. A separate ecosystem-profile PR, if still desired:
   - keep `TRUE_NAME` / Satnam-specific conventions out of the core MVP docs unless there is explicit adoption and ownership.

5. Once those NIP deltas are accepted, update the economy-kernel planning docs downstream to fit them.

### Recommendation

Do not merge this PR as-is.

It should be split, and the accepted pieces should be treated as intentional NIP revisions with synchronized app-facing follow-on work.

## Relationship Between `#2932` And `#2933`

They should not both move forward unchanged.

Practical reading:

- `#2932` is a speculative SA profile branch for alt rails and guardian behavior.
- `#2933` is a larger AC/SKL/SA redesign branch that reaches even further into authority and trust semantics.

So the right sequencing is not "merge both in order." The right sequencing is:

1. merge `#2978`,
2. decide which intentional NIP deltas from `#2932` and `#2933` are actually wanted,
3. reconcile overlaps so there is one coherent SA/SKL/AC story,
4. then adapt the economy-kernel planning docs downstream to match the accepted NIP changes.

## Concrete Change List By PR

## For `#2978`

- Cleanup is already applied on the PR branch.
- Merge as the standalone docs-only PR.

## For `#2932`

- Decide core-SA vs optional-profile status explicitly.
- Reconcile guardian and envelope semantics with `#2933` before merging either branch.
- Remove or defer rail-specific payment-proof requirements that the app does not yet consume.
- Normalize amount-unit semantics across Lightning/Cashu/Fedimint.
- Keep the resulting SA delta narrow enough that it reads as one coherent revision.

## For `#2933`

- Split the branch by concern.
- Remove `docs/TRUE_NAME_INTEGRATION_PROFILE.md` from the MVP protocol merge path.
- Revisit `39250` storage semantics so audit entries are append-only if kept.
- Decide whether permission grants and challenge/response are SKL core or optional profile material.
- Keep each accepted NIP delta narrow enough to justify itself on content, not bundle momentum.

## Recommended Merge Order

1. `#2978`, with the already-applied cleanup.
2. No merge for `#2932` or `#2933` until there is a coherent NIP-level redesign and synchronization pass.
3. After that redesign, re-open smaller PRs in this order:
   - targeted NIP deltas
   - downstream economy-kernel doc updates

## Bottom Line

The NIST PR is the only one that is truly standalone, and I would merge it first.

The other two PRs are not wrong in spirit, but they are currently too broad and too overlapping to serve as clean revisions to the established SA/SKL/AC NIPs that already power parts of the app.

So the right move is not to reject them because they use Nostr transport. It is to:

- decide which NIP changes are actually wanted,
- merge those as intentional, synchronized revisions to the active NIP suite,
- and then adapt the broader economy-kernel planning docs downstream to fit the accepted NIP surface.
