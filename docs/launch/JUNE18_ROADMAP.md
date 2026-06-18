# JUNE 18 ROADMAP - launch rc.31 and test the real Tassadar run logic

Date: 2026-06-18, 07:18 CT. Carries forward the still-live launch/test work
from [`JUNE17_ROADMAP.md`](./JUNE17_ROADMAP.md), now that the Tassadar
LLM-computer roadmap is implemented on `main`.

## END-OF-DAY UPDATE (2026-06-18) — what shipped (authoritative current state)

The morning plan (launch the RC, self-test, invite testers) is **done**, and the
RC moved rc.31 → **rc.33** as self-testing and a real tester (Trigger) shook out
bugs. All on npm `rc` + signed OTA 4-platform rollout 100 + GitHub prereleases;
`latest` stays `0.2.5`.

**RC progression (why each cut):**

- **rc.32** — pre-invite self-test fixes so basic commands don't break for
  testers: `pylon --version`/`-V` and bare `--help`/`-h` were booting the node and
  crashing on the control port (now short-circuit + exit); raw port-in-use crash →
  clear actionable error; stale "Pylon v0.3" crash banner → real version; the
  bundled Breez SDK storage banner was corrupting `wallet status --json` /
  `backup-status --json` stdout (guard added).
- **rc.33** — the Breez guard installed too late vs module-eval order in the
  compiled binary, so `wallet status --json` still leaked the banner (Trigger hit
  it). Fixed by installing the stdout guard eval-first (top-level side effect on the
  first import). Verified on the signed darwin-arm64 binary; **Trigger confirmed
  `wallet status --json` parses clean on rc.33.**

**RC thread — POSTED (owner approved).** Release Candidates thread:
<https://openagents.com/forum/t/6cb2d165-7a65-495d-a21c-6a3a546ad759> (title
corrected to rc.32; consolidated to one clean OP; my redundant reply deleted —
which required building real post-deletion + a topic-rename endpoint, below).

**Self-tested the full path before inviting anyone.** On the live run
`run.tassadar.executor.20260615`: worker `claim` → `submit-trace` (pinned
loop-sum, digest match, 80 steps) → an independent validator (`validate --auto`,
distinct device) replayed (exact digest match) → challenge **`Verified`**. Caught
5+ tester-facing bugs ourselves first.

**Settlements (precise, per Orrery's dereference):** two real-Bitcoin settled
receipts on the run —

- **Orrery 1,000 sats** (`pylon.448ba824…`, ~01:34Z) — the owner-armed canary;
  **first real settlement, full stop** (it proved the *rail*).
- **Trigger 5 sats** (`pylon.81f0facfe…`, ~14:13Z) — **first independent
  contributor through the rc.32 self-serve public path** (install → register →
  claim → submit → independent validation → paid). It proved the *door*.
  **Operator-retro-settled** via the admin settlement endpoint because the
  auto-stream *skipped* at verdict time (the payout-target resolution bug, fixed
  below).
- **The first fully-autonomous auto-stream settlement (gate firing at verdict, no
  operator POST) has NOT happened yet** — the next Verified pair should be it, now
  the resolver is fixed. Flag it explicitly when it lands.
- **Real settled total = 1,005 sats** (1000 + 5). The public aggregate
  `providerConfirmedSettledPayoutSats` currently reads 1,010 because it counts
  `state:settled` without filtering movement, so it includes an old 5-sat
  **simulation** receipt (`…59ba1f30…`, `mdk_agent_wallet`,
  `realBitcoinMoved:false`). Reconcile fix in flight (below).

**Bugs fixed + deployed today (openagents.com Worker):**

- **Settlement payout-target resolution** — Verified contributors with a *ready*
  Spark target weren't paid because settlement resolved by the lease's worker
  device-ref while the target is registered under the pylonRef; added an
  owner-scoped canonical fallback (fail-closed). Unblocked Trigger's payout.
- **Real forum post deletion** — tombstoned posts were rendering a broken
  `content.forum.post.<id>` placeholder; now excluded from the topic projection
  (audit row kept, counts corrected). Cleared the bad post.
- **#5333 self-serve agent displayName** — `PATCH /api/agents/me` so an agent can
  rename itself (propagates live to `/api/pylons`; Forum author names are per-post
  snapshots — flagged, no risky backfill). Closed.
- **Forum topic-title rename** — `PATCH /api/forum/topics/{id}` (author-only),
  built to fix the stale RC thread title.

**In flight (last open item):** reconcile the run-level settled state from settled
receipts (stop showing stale manifest `settlementState:pending`), add an
enumerable REST settled-feed endpoint keyed by run, and **fix the real-vs-sim
total so the run reads 1,005 real, not 1,010** — all per Orrery's reconciliation
asks. On deploy, post the corrected numbers + the feed URL for re-dereference.

**Monitoring:** all-day forum-reply + GitHub-issue watchers armed; regular
public-safe progress updates posted as Raynor as each fix lands. Two independent
contributors are actively stress-testing — Trigger (client + settlement path),
Orrery (receipt-trail + projection reconciliation, with sha256/Nostr/OTS
pre-commitments).

**Overnight (context):** the Tassadar roadmap EPIC #5313 + child issues
#5314–#5332 were built (C/V/E/H tracks) and the run-state audit was reframed to the
Percepta **LLM-computer** paradigm — training = compiling programs into
transformer weights, verified by exact replay, not gradient descent.

**Morning closeout criteria — MET:** RC verified across npm/OTA/GitHub (rc.33);
the live Worker includes the current Tassadar code (deployed); an actual-run Pylon
path was exercised through contribution + independent replay → `Verified` +
settled; the RC thread is posted (owner-approved); launch docs updated (this
section). The remaining reconciliation fix is tracked above.

## Product-promise status (end of day)

Audit of the product-promises registry (`apps/openagents.com/workers/api/src/product-promises.ts`)
and `docs/promises/` against today's dereferenceable receipts. Registry bumped
`2026-06-18.2 → 2026-06-18.3` (copy/evidence accuracy upgrade only — no gate flip).

### Q1 — is "paying people for verified work" fully + accurately reflected?

**Verdict: covered, and now accurate (after this update).** Paid-verified-work /
streaming-settlement / paid-contributions is reflected by a *set* of promises, not
one named "streaming settlement" promise (the registry models claims/capabilities,
not mechanisms — that is correct):

- `training.decentralized_training_launch.v1` (**green**) — the primary one for
  today: install Pylon → verified Tassadar exact-trace-replay work → independent
  validator replay → real Bitcoin settlement over Spark. This is where the
  streaming-settlement gate (run-scoped, 5w/5v, 100/payout, 50k/day) and the
  real settled receipts live.
- `labor.forum_work_requests.v1` + `labor.nostr_negotiation_market.v1` (**green**)
  — the live labor/work-request market (credit-ledger settlement; external-wallet
  labor payout still gated under `provider.compliant_usage_labor.v1`, yellow).
- `payments.accepted_outcome_economics.v1` (**red**) — the formal
  paid/accepted/payable/dispatched/settled state-machine gate (correctly red).
- `pylon.five_bitcoin_revenue_streams.v1`, `pylon.compute_revenue_modes.v1`,
  `pylon.data_trace_revenue.v1` (**planned**) — broad earning, correctly planned.

**Gap found and corrected (under-statement):** the green
`training.decentralized_training_launch.v1` and the registry notes were frozen at
the Orrery-canary state — they said "**one** contributor has now been paid real
Bitcoin" and cited only the 1,000-sat canary receipt + the simulation receipt.
Today's reality is **two** distinct real settled receipts to **two** distinct
independent contributors, and a live enumerable settled feed. The copy understated
the proof. Corrected to "two distinct independent contributors paid", 1,005 real
sats total, with the second (self-serve) receipt + the settled feed added as
evidence, and the operator-retro-settled / no-auto-stream-yet caveat made explicit.
No scope widened; codebase-contribution paid work ("soon") remains a future claim
and was not added.

Dereferenceable evidence used:
- `GET /api/training/runs/run.tassadar.executor.20260615/settlements` — three rows:
  1,000-sat **real** (canary, `pylon.448ba824…`), 5-sat **real** (self-serve,
  `pylon.81f0facfe…`, `…retro.10c3b01b.trigger.v1`), 5-sat **simulation**
  (`realBitcoinMoved:false`, excluded). Real total = **1,005**.
- Second real receipt `receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1`
  (`realBitcoinMoved:true`, `state:settled`, `adapter:spark_treasury`), backed by
  Verified challenge `training.verification.challenge.10c3b01b-c781-4a03-a8ed-4ae6c6195fe4`.

Honesty caveats preserved: the **first fully-autonomous auto-stream settlement
has not happened** (the 5-sat self-serve payout was operator-retro-settled because
the auto-stream skipped at verdict, payout-target bug since fixed); and the
run-summary `settledPayoutSats` / `/api/public/pylon-stats` 24h aggregate still
read **1,010** because they have not yet excluded the simulation row — the
`/settlements` feed (1,005) is the reconciled per-run truth and the aggregate
reconciliation is tracked separately (Orrery is actively dereferencing it).

### Q2 — today's gates: flip-status + evidence

| Promise | State | Flip today? | Evidence / why |
| --- | --- | --- | --- |
| `training.decentralized_training_launch.v1` | green | **No flip — copy upgrade only (done)** | Already green. Two real settled receipts (1,005 sats) + live settled feed. Updated copy/evidence in registry `2026-06-18.3`. State unchanged green→green, so no `promise_transition` required; an optional exception receipt for the copy upgrade is **owner-gated** per `proof.claim_upgrade_receipts.v1`. |
| `training.public_distributed_training_run.v1` | red | **No** | Correctly red. Two bounded settlements + one verified pairing do not prove network-scale participation, a participant-count methodology, or broad multi-contributor accepted-work receipts. Stays red. |
| `pylon.first_real_model_training_run.v1` | yellow | **No** | Unrelated evidence base (CS336 A1 two-device real-gradient run). Today's Tassadar receipts do not bear on its model-ladder-network-rungs blocker. Stays yellow. |
| `training.public_gradient_windows.v1` | planned | **No** | H1 has code-backed psionic frozen-core validation + quarantine→recompute→canary→promotion gate, but no public contributor gradient window has been accepted/promoted/paid/settled. Public devices do generation/validation/eval only. Stays planned. |
| `payments.accepted_outcome_economics.v1` | red | **No** | The formal settlement state-machine + contributor-ledger + gross-margin gates are not met. Stays red. |
| `autopilot.repo_study_packets.v1` / `autopilot.external_repo_studying_pilot.v1` (studying→Autopilot-coder) | yellow | **No** | Internal-dogfood / refs-only pilot. Customer-repo studying, marketplace packaging, pricing, payout, and settlement remain blocked. Stays yellow. |

**No gate is a warranted flip today.** The single warranted registry change was the
green-promise copy/evidence accuracy upgrade above (no state change). Conservative
per the 2026-06-18 read: the run constructs **no new capability** beyond the fixed
executor workload, and the first fully-autonomous auto-stream settlement has not
landed — so no red/yellow/planned promise advances on today's evidence.

### Owner sign-off needed

- **Optional:** record a `promise_transition` *exception* receipt for the
  `2026-06-18.3` green→green copy upgrade via the operator route (per
  `proof.claim_upgrade_receipts.v1`), if the owner wants the copy upgrade itself
  dereferenceable as a transition. Not required for correctness (state unchanged).
- **Worker redeploy required:** `product-promises.ts` changed, so
  `/api/public/product-promises` will not serve `2026-06-18.3` (Trigger receipt,
  settled feed, two-contributor copy) until the `openagents.com` Worker is
  redeployed. **Not deployed by this change** — deploy is owner-gated.
- **Watch item (not a registry change):** flip-to-1,005 of the run-summary
  `settledPayoutSats` field and the pylon-stats 24h aggregate (currently 1,010,
  simulation row not yet excluded) is the in-flight reconciliation already tracked
  in the END-OF-DAY UPDATE; the registry now points readers to the `/settlements`
  feed as the reconciled truth in the meantime.

> The sections below are the **morning handoff record** (07:18 CT) and are
> superseded by the END-OF-DAY UPDATE above where they differ.

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

> POSTED (owner-approved) — live at
> <https://openagents.com/forum/t/6cb2d165-7a65-495d-a21c-6a3a546ad759>, now
> rc.33 + consolidated. The draft below is the historical morning prep.

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

> Status: criteria MET (see END-OF-DAY UPDATE). One reconciliation fix
> (real-only settled total + run-level reconcile + enumerable settled feed)
> remains in flight; settlement gate stays armed as-is.

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
