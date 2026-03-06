# 2026-03-06 PR NIP Content Audit

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


- Author: Codex
- Status: complete
- Scope: review of PRs `#2978`, `#2932`, and `#2933` against the active SA/SKL/AC NIP surface and established NIP conventions from `/Users/christopherdavid/code/nips/`

## Objective

Answer three questions:

1. Which substantive changes in these PRs belong in the NIPs?
2. What should change in each PR before merge?
3. Which substantive parts of `#2932` and `#2933` belong in the NIPs, which should be changed, and which do not fit current NIP conventions?

## Sources Reviewed

Primary repo authority:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PROTOCOL_SURFACE.md`
- `docs/NIP_SA_SKL_AC_IMPLEMENTATION_PLAN.md`
- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`
- `crates/nostr/nips/AC.md`

Established NIP convention references:

- `/Users/christopherdavid/code/nips/README.md`
- `/Users/christopherdavid/code/nips/01.md`
- `/Users/christopherdavid/code/nips/31.md`
- `/Users/christopherdavid/code/nips/32.md`
- `/Users/christopherdavid/code/nips/40.md`
- `/Users/christopherdavid/code/nips/42.md`
- `/Users/christopherdavid/code/nips/60.md`
- `/Users/christopherdavid/code/nips/61.md`
- `/Users/christopherdavid/code/nips/87.md`
- `/Users/christopherdavid/code/nips/89.md`
- `/Users/christopherdavid/code/nips/90.md`

Broader vision reference:

- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`

Reviewed PRs:

- `#2978` `docs(nist): add NIST RFI NIST-2025-0035 supporting materials`
- `#2932` `Add SA-Cashu, SA-Fedimint, and SA-Guardian optional profiles`
- `#2933` `[NIP-AC] Add Cashu spending rail, SKL safety label revocation trigger, and Guardian gate integration`

Current status:

- `#2978` merged at `5388cdb786f53ea73d70feef2efecc3b98976438`
- `#2932` merged at `674bfa363ba7376332399008c166a4700679691c`
- `#2933` merged at `fb42f9f5a937aa6dce46fcf076b1a37b2007edb2`

Current in-repo protocol docs:

- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`
- `crates/nostr/nips/AC.md`

## Framing Correction

This audit has been updated to reflect the intended hierarchy more accurately:

1. The SA/SKL/AC NIPs in `crates/nostr/nips/` were written specifically to power parts of the app and are already partially implemented in `crates/nostr/core` and integrated in `apps/autopilot-desktop`.
2. The default review posture should therefore be: hold the NIPs relatively stable and evaluate PRs primarily as changes to those established NIPs.
3. Nostr being a relay / websocket protocol is not itself a reason to reject NIP changes. The relevant review questions are whether a PR is backwards-compatible, whether it creates a single clear way of doing the thing, and whether the new semantics actually belong in that NIP.

## Executive Recommendation

| PR | Standalone? | Merge now? | Recommendation |
| --- | --- | --- | --- |
| `#2978` | Yes | Merged | Merged after doc cleanup. |
| `#2932` | Yes | Merged | Merged after SA revisions: durable approval proof, aligned guardian naming, explicit sat-denominated budget semantics, federation-as-hint wording, and removal of duplicated rail-proof hash definitions. |
| `#2933` | Partly | Merged | Merged after AC/SA/SKL revisions: `cancel_until` instead of `hold_period_secs`, `33410`/`33411` moved to an optional SKL auth profile with ephemeral semantics, `33420` removed, `39250` dropped in favor of richer `39231` trajectory tags, `39260` cleaned up, and the `TRUE_NAME` profile deleted. |

## Substantive NIP Findings

1. `#2932` and `#2933` initially overlapped on guardian and rail semantics, so the final merge path needed one converged vocabulary across SA and AC.
2. `#2933` initially defined `kind:39250` audit entries as addressable, which was the wrong storage shape for forensic history and was revised away before merge.
3. `#2932` initially used the same `budget` tag for multiple rails without a stable unit model, which was revised before merge.
4. `#2933` initially put live-auth and permission-grant semantics into SKL core even though current SKL scope is registry/trust, not general authorization.
5. `#2933` initially used `hold_period_secs` plus over-specific proof-hash definitions; the merged version instead uses absolute `cancel_until` timing and leaves detailed proof construction out of AC core.

## NIP Conventions Used For Review

These are the conventions used for the substantive review below.

### 1. Backwards-compatible and optional

From `/Users/christopherdavid/code/nips/README.md`:

- new NIPs should be optional,
- they should be backwards-compatible,
- clients/relays that ignore them should still interoperate.

Both `#2932` and `#2933` are mostly additive, which is good.

### 2. One clear way of doing each thing

Also from `/Users/christopherdavid/code/nips/README.md`:

- there should be no more than one way of doing the same thing.

This matters for:

- overlapping guardian / approval semantics across `#2932` and `#2933`,
- separate audit-trail kinds when SA already has trajectory events,
- and duplicated payment-proof semantics across SA and AC.

### 3. Storage type should match semantics

From NIP-01 conventions:

- regular events are for durable records,
- ephemeral events are not expected to be stored,
- addressable events are replaceable by `(kind, pubkey, d)`.

If a new event is meant to be durable proof or append-only audit history, addressable or ephemeral storage may be the wrong choice.

### 4. Reuse existing NIP patterns where they already fit

Examples used here:

- NIP-32 for labels and local-quorum interpretation,
- NIP-40 for absolute expiry timestamps,
- NIP-42 for live proof-of-possession challenge/response,
- NIP-61 and NIP-87 for ecash mint / federation discoverability patterns,
- NIP-89 for handler / capability advertisement,
- NIP-90 for request / result / feedback marketplace flows.

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

### Result

Merged after the filename/disclaimer cleanup.

## PR `#2932` `Add SA-Cashu, SA-Fedimint, and SA-Guardian optional profiles`

### What is useful in it

- It is directionally consistent with a future multi-rail world.
- It tries to keep most changes additive rather than breaking.
- It is closer to a profile than a core rewrite, which is the right instinct.

### What substantively belongs

These parts fit current NIP conventions reasonably well:

- `cashu_mint` tags on `kind:39200` as optional inbound-rail hints,
- a `federation` hint on `kind:39200`,
- explicit guardian-related tags and events as an optional SA profile,
- back-references from tick results to AC settlement receipts and NIP-90 results.

### Issues identified before merge

1. `kind:39213` is defined as ephemeral even though later events are expected to reference it as proof of approval.

That is a storage-type mismatch. If the approval is meant to be durable evidence, `39213` should be regular. If it is meant to be ephemeral only, later events should not treat its event id as durable proof.

2. The extended `budget` tag is ambiguous about units.

Using one `budget` tag for:

- Lightning sats,
- Cashu msats,
- Fedimint ecash,
- and envelope-linked spending

without an explicit unit convention is not a good NIP shape. This should be normalized, for example by keeping one base amount unit and moving rail-specific details into additional tags, or by explicitly adding a unit field.

3. The `federation` tag format is not well aligned with existing ecash discoverability conventions.

NIP-87 uses mint/federation announcement events with explicit `d` and `u` semantics. A value like `<federation-id>@<domain>` may still be useful as a hint, but it should be framed as a convenience identifier, not as the canonical federation locator.

4. The `payment_rail` / `payment_proof` expansion on `kind:39220` belongs only if SA wants skill-license settlement proof to live in SA rather than AC.

That is a real protocol choice. If kept, SA should reference one canonical proof-hash definition source instead of defining rail-proof semantics in parallel with AC.

5. It overlaps with `#2933` on guardian and envelope-related semantics, which means the two PRs together currently violate the “single way of doing each thing” rule.

Examples:

- tick requests carry approval tags here,
- `#2933` adds related guardian and envelope gating elsewhere,
- both branches touch the SA/AC boundary.

That needs one coherent protocol decision, not two partially overlapping ones.

### How to realign it

If we want to preserve the work, the cleanest path is:

1. Change `39213` to regular if it is meant to be referenced as proof.
2. Fix the `budget` tag so amount and unit semantics are unambiguous.
3. Clarify that `federation` is a hint / identifier, not a replacement for NIP-87-style federation discovery.
4. Keep the guardian semantics in one naming scheme that can also coexist with AC.
5. Choose one place for rail-proof hash definitions and make the other spec reference it.

### Applied revisions before merge

The merged SA text now:

1. makes `39213` regular rather than ephemeral,
2. uses `guardian` + `approval_threshold` naming,
3. makes `budget` explicitly sats-denominated across all forms,
4. reframes `federation` as a hint rather than canonical federation discovery,
5. removes duplicated rail-proof hash construction from SA core and leaves that to rail/profile-specific specs.

### Result

Merged after the revisions above.

## PR `#2933` `[NIP-AC] Add Cashu spending rail, SKL safety label revocation trigger, and Guardian gate integration`

### What is useful in it

- It surfaces real design questions:
  - alternative spending rails,
  - safety-label-driven enforcement,
  - authorization grants,
  - reversible high-risk actions,
  - auditability and delegation.
- It also correctly noticed that the NIST bundle should be split out.

### What substantively belongs

These parts fit well enough and should likely be kept:

- `spend_rail` as distinct from `repay` in AC,
- explicit Fedimint support in `repay` examples,
- SKL safety-label revocation as an AC policy hook,
- guardian-gated envelopes in AC if the naming is aligned with SA,
- optional `security_posture` metadata in SA profile content.

### Issues identified before merge

1. `hold_period_secs` was a weak fit for NIP time semantics.

NIPs more commonly use explicit timestamps, for example NIP-40 `expiration`. For AC, an absolute `hold_until` / `cancel_until` style field would be clearer and easier to interpret than a relative duration that must be combined with `created_at`.

2. `33410` / `33411` did not clearly belong in SKL core, and regular storage was a questionable choice for live challenge-response.

Substantively, this looks more like:

- a live proof-of-possession flow similar to NIP-42,
- or an optional auth profile,

than part of SKL core registry/trust semantics. If kept, I would not put it in SKL core as regular events.

3. `33420` Permission Grants did not clearly belong in SKL core.

Current SKL scope is canonical skill identity, manifests, trust signals, revocation, and discovery. Operator- or guardian-issued action grants are a different concern. They may be useful, but they read more like SA or a separate auth/capability profile than SKL core.

4. `39250` did not belong as a new addressable audit kind because SA already has trajectories.

SA already defines:

- `39230` Trajectory Session
- `39231` Trajectory Event

Those are already described as audit trail, live coordination, training data, and debugging artifacts. The more NIP-like move is to enrich `39231` content/tags for auditable actions, not to create a second replaceable audit log primitive.

5. `39260` may belong in SA, but it needed cleanup against existing trajectory events and merged guardian vocabulary.

Delegation is plausibly SA content. The open question is not "can SA have delegation?" but "does this need a new kind, or should delegation be modeled as a typed trajectory event first?" The PR should answer that explicitly.

6. The branch also carried `docs/TRUE_NAME_INTEGRATION_PROFILE.md`, which was ecosystem-specific and not needed for OpenAgents.

### Applied revisions before merge

The merged `#2933` branch was revised as follows before merge:

1. AC kept `spend_rail`, guardian-gated envelopes, and the SKL safety-label revocation trigger.
2. AC replaced `hold_period_secs` with absolute `cancel_until` timing.
3. AC renamed `spend_rail_keyset` to Cashu-specific `spend_cashu_keyset`.
4. AC dropped the canonical proof-hash appendix and now leaves detailed proof construction out of core.
5. SKL kept `assurance_tier` and optional `nip05` organizational identity.
6. SKL moved `33410` / `33411` into an optional `SKL-Auth Challenge Profile` with ephemeral semantics.
7. SKL removed `33420` from core.
8. SA kept `security_posture`.
9. SA dropped `39250` and instead extended `39231` trajectory events with audit-friendly tags.
10. SA kept `39260`, but aligned it to the merged vocabulary (`credit`, `scope`, `approval_threshold`, `guardian`) and softened it to a `SHOULD` publish requirement.
11. The `docs/TRUE_NAME_INTEGRATION_PROFILE.md` file was deleted from the PR branch.

### Result

Merged after the revisions above.

## Relationship Between `#2932` And `#2933`

The key substantive issue across both PRs is this:

- the SA guardian vocabulary,
- envelope / approval terminology,
- and alternative-rail proof semantics

need one coherent final form across SA and AC. That is required by the standard NIP criterion that there should not be more than one way of doing the same thing.

## Concrete Change List By PR

## For `#2978`

- Cleanup is already applied on the PR branch.
- Merge as the standalone docs-only PR.

## For `#2932`

- Make `39213` regular if it is referenced as durable approval proof.
- Fix `budget` amount/unit semantics.
- Clarify `federation` as a hint rather than canonical federation discovery.
- Keep guardian terminology aligned with AC.
- Normalize amount-unit semantics across Lightning/Cashu/Fedimint.
- Avoid duplicating rail-proof definitions already better housed in AC.

## For `#2933`

- Keep AC `spend_rail` and SKL safety-label revocation.
- Replace relative `hold_period_secs` with absolute hold/cancel timing.
- Move or profile-gate `33410` / `33411`.
- Move or drop `33420` from SKL core.
- Replace `39250` with richer `39231` trajectory semantics.

## Current Outcome

1. `#2978` merged with the doc cleanup.
2. `#2932` merged after the substantive SA revisions listed above.
3. `#2933` merged after the substantive AC/SA/SKL revisions listed above.

## Bottom Line

For the substantive NIP content:

- `#2978` is done.
- `#2932` was substantially sound and is now merged after targeted SA fixes.
- `#2933` was mergeable after targeted narrowing: the good AC/SA/SKL additions stayed, while the misplaced core-auth, permission-grant, mutable-audit, and `TRUE_NAME` content was removed or reshaped.

The main rule I used is the canonical NIP one from `/Users/christopherdavid/code/nips/README.md`: there should not be more than one way of doing the same thing. The merge path should therefore keep the good ideas, remove the misplaced ones, and converge SA/SKL/AC on one vocabulary for guardian approvals, rails, auditability, and proof semantics.
