# JUNE 18 ROADMAP - launch rc.31 and test the real Tassadar run logic

Date: 2026-06-18, 07:18 CT. Carries forward the still-live launch/test work
from [`JUNE17_ROADMAP.md`](./JUNE17_ROADMAP.md), now that the Tassadar
LLM-computer roadmap is implemented on `main`.

## Status at morning handoff

- Public AGENTS reviewed at <https://openagents.com/AGENTS.md>. The Release
  Candidates forum is the right public feedback surface:
  <https://openagents.com/forum/f/release-candidates>. Do not publish the
  Raynor thread until the owner explicitly says to post it.
- Local release runbook reviewed: [`docs/DEPLOYMENT.md`](../DEPLOYMENT.md).
  RCs stay prerelease-only; npm `latest` must remain the stable launcher
  (`0.2.5` as of this morning).
- Current Pylon source version is `1.0.0-rc.31`
  (`apps/pylon/package.json`, `apps/pylon/src/version.ts`). `npm view
  @openagentsinc/pylon dist-tags` reports `rc: 1.0.0-rc.31`, `latest: 0.2.5`.
  GitHub prerelease `pylon-v1.0.0-rc.31` is published
  (2026-06-18 04:04 UTC). Verify signed OTA/feed state before claiming the
  auto-update surface is on rc.31.
- The Tassadar roadmap EPIC #5313 and child issues #5314-#5332 are closed as
  completed. Final commits on `main`:
  - `93b4ecbbf` through `657cd9270` in `openagents`: C1-C5, V1-V3, E1-E3, H1
    public execution, replay, settlement-simulation, marketplace, labor,
    adversarial verification, and gradient-window gates.
  - `8e74fc2d` in `psionic`: H1 frozen-core learned-interface validator over
    the W3 Baseline D evidence.
- The live run to test remains `run.tassadar.executor.20260615`. The point of
  today's RC test is not to claim a new trained model prematurely; it is to
  prove that current Pylon + current Worker can exercise the now-correct
  Tassadar construction/replay logic against the actual run path and produce
  public-safe receipts.

## Today's launch objective

Launch the current RC to testers, then run a receipt-first Tassadar test window
with the proper logic now present:

1. Start from a clean `origin/main` checkout.
2. Verify the published RC surfaces:
   - npm `rc` dist-tag resolves to `@openagentsinc/pylon@1.0.0-rc.31`;
   - GitHub prerelease `pylon-v1.0.0-rc.31` exists and is prerelease-only;
   - signed OTA/feed status is checked before any auto-update claim;
   - a fresh install reports `pylon --version` / `pylon status --json` as
     `1.0.0-rc.31`.
3. Deploy or verify the `openagents.com` Worker from clean `main` before the
   live run test if the latest Tassadar code has not already reached
   production. Do not call a local `check:deploy` pass a production deploy.
4. Smoke the public run surfaces:
   - `/api/training/runs/run.tassadar.executor.20260615`;
   - `/api/public/tassadar-run-summary`;
   - `/tassadar`;
   - `/api/public/product-promises`.
5. Run the Pylon contributor path against the actual run:
   - `pylon training status --base-url https://openagents.com`;
   - `pylon training claim`;
   - execute the assigned digest-pinned workload;
   - pair it with a separate validator device for `exact_trace_replay`;
   - record only public-safe contribution, replay, verifier, and receipt refs.
6. Confirm the run is exercising the new logic by evidence, not vibe:
   - C-track: real compiled-program corpus / dense module / linked module refs
     are present where the window expects them.
   - V-track: exact replay and construction-settlement simulation gates produce
     deterministic public-safe refs.
   - E-track: any labor/curation/adversarial market hooks stay typed and
     operator-gated; no Forum keyword routing.
   - H-track: learned-interface gradient windows remain quarantine/canary/
     promotion candidates only; no canonical checkpoint mutation or gradient
     payout claim without the full gate.
7. Only after the above, post the Release Candidates forum thread as Raynor.

## Release Candidates forum thread prep - Raynor

No post yet. Prepare this as a draft for the owner/posting step.

Suggested title:

```text
Pylon v1.0.0-rc.31 - Tassadar actual-run test window
```

Draft body shape:

```text
Raynor here. This is the rc.31 test window for the live Tassadar run.

What changed:
- Pylon rc.31 is the current RC install target.
- The Tassadar construction/verification roadmap is now on main: compiled
  program corpus, dense/loadable modules, linked module verification,
  construction settlement simulation, edge work directions, demand ranking,
  adversarial verification, and the frozen-core learned-interface quarantine
  gate.

What we need testers to try:
- Install/update to the RC.
- Confirm the local Pylon version.
- Check the live Tassadar run status.
- Claim and execute an assigned training window if admitted.
- Leave the node available for independent exact replay validation.
- Post only public-safe refs, version output, OS/platform, run/window refs,
  verifier verdict refs, and receipt refs.

Important caveats:
- Do not post wallet seeds, mnemonics, invoices, preimages, tokens, raw logs,
  raw traces, private prompts, provider material, or payout targets.
- Installing a node is not an earning claim.
- Accepted work and payouts require dereferenceable receipt evidence.
- Learned-interface gradient windows are candidate/quarantine-gated only; this
  RC does not claim public decentralized gradient training is live.
```

Before posting, replace the placeholders with verified live refs:

- exact RC install command/result;
- GitHub release URL;
- signed OTA/feed evidence if included;
- live `/api/training/runs/...` state;
- current known blockers;
- the first successful public-safe contribution/replay/receipt refs, if any.

## Carry-forward from June 17

### Gate 2 - real settlement / auto-payout dispatch (#5232)

Still the largest v1.0 release gate unless a newer receipt proves otherwise.
The default remains simulation/no-money movement. Real Bitcoin movement requires
the explicit owner gate, run allowlist, caps, payout target approval, idempotent
dispatch, reconciliation, and public-safe receipt refs. Do not arm or broaden
this gate as part of the docs/forum prep.

### Spark-native routing (#5225)

Verify final state before release copy. If complete, smoke that Spark-address
destinations use native Spark routing with zero Lightning fallback and honest
method labels. If incomplete, keep it as a non-blocking RC caveat unless the
specific RC test depends on internal Spark-to-Spark payout flow.

### Remaining wallet retests from June 17

Check whether #5208 Lightning Address send and #5194 helper-unavailable retests
have owner/contributor confirmation. Do not block the Tassadar logic RC unless
the failing path is part of the test install/run/receipt loop.

### Built-in hosted agent promise (#5063)

Still separate from today's Tassadar RC test. Green requires desktop executor
use of the live keyless grant route, a from-install go-online smoke, signed
recut evidence, and product-promise refs.

### Email strategy smoke

Still separate. Useful for operator notifications, not a blocker for the
Tassadar actual-run RC.

## Do not overclaim

- Do not say the public run has trained a new model until the run evidence
  proves model construction beyond the fixed executor workload.
- Do not say public gradient training is live. H1 is a quarantine/promotion
  gate around learned-interface candidate windows.
- Do not say a contributor earned Bitcoin until accepted-work and settlement
  receipts are dereferenceable.
- Do not present owner-operated nodes as independent contributor proof.
- Do not post as Raynor until the owner explicitly moves from roadmap prep to
  posting.

## Closeout for today

June 18 is done when:

- rc.31 (or a newer explicitly bumped RC, if code changes again) is verified
  across the intended install surfaces;
- the live Worker is verified to include the current Tassadar code;
- at least one actual-run Pylon path is exercised through contribution plus
  independent replay validation, or the blocker is captured with public-safe
  evidence;
- the Release Candidates thread is posted as Raynor only after owner approval;
- product promises and launch docs are updated to reflect only what receipts
  prove.
