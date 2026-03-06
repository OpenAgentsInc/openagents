# 2026-03-06 PR NIP Content Audit

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
- `#2933` remains open

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
| `#2933` | Partly | Not as-is | Some content clearly belongs, especially AC spend-rail and SKL-revocation linkage. Other pieces should be revised or removed from core NIP text because they do not fit current NIP scope or duplicate existing mechanisms. |

## Substantive NIP Findings

1. `#2932` and `#2933` overlap and compete to redefine SA / AC behavior rather than presenting one coherent NIP delta.
2. `#2933` defines `kind:39250` audit entries as addressable, which makes a supposedly forensic audit stream mutable even on its own Nostr-native terms.
3. `#2932` uses the same `budget` tag for multiple rails without a stable unit model, which is not a good NIP convention.
4. `#2933` puts live-auth and operator-grant semantics into SKL core even though current SKL scope is registry/trust, not general authorization.
5. `#2933` adds audit and delegation primitives without first proving they cannot be expressed more cleanly as extensions of SA's existing trajectory primitives.

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

### Why it should not merge as-is

1. `hold_period_secs` is a weak fit for NIP time semantics.

NIPs more commonly use explicit timestamps, for example NIP-40 `expiration`. For AC, an absolute `hold_until` / `cancel_until` style field would be clearer and easier to interpret than a relative duration that must be combined with `created_at`.

2. `33410` / `33411` do not clearly belong in SKL core, and regular storage is a questionable choice for live challenge-response.

Substantively, this looks more like:

- a live proof-of-possession flow similar to NIP-42,
- or an optional auth profile,

than part of SKL core registry/trust semantics. If kept, I would not put it in SKL core as regular events.

3. `33420` Permission Grants do not clearly belong in SKL core.

Current SKL scope is canonical skill identity, manifests, trust signals, revocation, and discovery. Operator- or guardian-issued action grants are a different concern. They may be useful, but they read more like SA or a separate auth/capability profile than SKL core.

4. `39250` does not belong as a new addressable audit kind because SA already has trajectories.

SA already defines:

- `39230` Trajectory Session
- `39231` Trajectory Event

Those are already described as audit trail, live coordination, training data, and debugging artifacts. The more NIP-like move is to enrich `39231` content/tags for auditable actions, not to create a second replaceable audit log primitive.

5. `39260` may belong in SA, but it should be justified against existing trajectory events.

Delegation is plausibly SA content. The open question is not "can SA have delegation?" but "does this need a new kind, or should delegation be modeled as a typed trajectory event first?" The PR should answer that explicitly.

6. `docs/TRUE_NAME_INTEGRATION_PROFILE.md` is ecosystem-specific.

That is fine as a non-normative profile document. It does not belong as normative core SA/SKL/AC content.

7. It overlaps with `#2932` on guardian and rail semantics, so the combined result still violates the “single way of doing each thing” rule unless the vocabulary is unified.

### How to salvage it

If you want the content merged, I would revise it in-place as follows:

1. Keep AC `spend_rail`, `repay` rail clarification, Fedimint support, and SKL safety-label revocation.
2. Replace `hold_period_secs` with an absolute-timestamp formulation.
3. Either move `33410` / `33411` out of SKL core or explicitly mark them as an optional auth profile, ideally using ephemeral semantics if they remain live challenge-response events.
4. Move `33420` out of SKL core unless you explicitly want SKL to own operator/guardian authorization contracts.
5. Drop `39250` as a new addressable kind and instead extend `39231` trajectory events with the needed audit tags.
6. Keep `39260` only if you conclude delegation really needs a first-class SA kind instead of a typed trajectory event.
7. Keep `TRUE_NAME` as a non-normative profile document only.

### Recommendation

This PR should not merge as-is, but several of its substantive protocol ideas do belong and are worth keeping after the revisions above.

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
- Keep `TRUE_NAME` non-normative only.

## Current Outcome

1. `#2978` merged with the doc cleanup.
2. `#2932` merged after the substantive SA revisions listed above.
3. `#2933` remains the unresolved protocol PR and should be judged against the substantive NIP points in this audit.

## Bottom Line

For the substantive NIP content:

- `#2978` is done.
- `#2932` was substantially sound and is now merged after targeted SA fixes.
- `#2933` contains both good additions and some content that does not currently belong in core SKL/SA form.

The main rule I used is the canonical NIP one from `/Users/christopherdavid/code/nips/README.md`: there should not be more than one way of doing the same thing. The merge path should therefore keep the good ideas, remove the misplaced ones, and converge SA/SKL/AC on one vocabulary for guardian approvals, rails, auditability, and proof semantics.
