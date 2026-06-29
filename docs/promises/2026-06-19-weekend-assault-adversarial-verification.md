# Weekend Promise Assault — Adversarial Verification (2026-06-19)

Independent attempt to REFUTE the honesty of everything the weekend promise
assault merged this session. Worktree off `origin/main` at merge `5a4dab35c`
(registry `2026-06-19.9`). Baseline for "pre-assault" comparisons:
`1ecb39683` (registry `2026-06-19.6`, the Episode 239 reconciliation just before
the assault wave began).

Default posture: skeptical — assume we over-claimed somewhere and go find it.

## Verdict summary

| # | Claim checked | Verdict |
| - | ------------- | ------- |
| 1 | Green count is exactly 20; no promise dishonestly flipped/advanced | HONEST |
| 2 | New scaffolds are truly inert (no money/settlement without flag + owner arm) | HONEST |
| 3 | "Green-ready" claims are honestly green-ready, real gates intact | HONEST |
| 4 | No over-claim in safeCopy/unsafeCopy on touched records | HONEST (one internal-contradiction wart fixed; see Fixes) |
| 5 | Cited receipts/docs dereference | HONEST (one cross-dir lookup confusion, no real dangling ref) |

Net: the assault was honest. One cheap honest fix made (an internal
numeric contradiction inside the green training record). A small set of
**under-stating** stale refs in red sibling records is FLAGGED (not fixed) for
owner review — they are conservative, not over-claims.

## Claim 1 — Green count exactly 20; green set unchanged by the assault

- `PublicProductPromisesVersion = '2026-06-19.9'` (product-promises.ts:7),
  matching the green-count test note and the brief.
- State distribution in the registry: **green 20**, yellow 30, red 19,
  planned 27, withdrawn 2.
- The green-count test asserts `=== 'green').length).toBe(20)`
  (product-promises.test.ts:119) and passes (3/3).
- **Green SET diff, pre-assault (`1ecb39683`) vs HEAD (`5a4dab35c`): NO DIFF.**
  Every green promise that is green now was already green before the assault
  session. The assault made ZERO changes to the green set.
- Going further back to `445d765d3` (registry `2026-06-16.9`, green=19): the only
  two green-set deltas since then both PREDATE this assault session — the
  `payments.offline_receive_spark_fallback.v1` flip and the documented
  identifier rename `training.monday_decentralized_training_launch.v1` ->
  `training.decentralized_training_launch.v1` (state preserved). Neither is
  attributable to the assault.

Verdict: HONEST. No green flip, no green-set swap, no dishonest advancement.

## Claim 2 — New scaffolds truly inert

Audited the seven new flag-gated surfaces against the question: can any path
charge / settle / decrement credits / pay out / leak without BOTH (a) a feature
flag on AND (b) an explicit owner-arm of a live adapter/hook/store?

Shared mechanism: flag parsers default OFF on unset (`config.ts:406-432`
`optionalBooleanFlag`; per-scaffold parsers return `false` on `undefined`,
enabling only on `1/true/yes/on`). Production `wrangler.jsonc` (`vars`) sets
NONE of the scaffold flags -> all OFF on prod. Staging arms only three
(`CLOUD_FINE_TUNING_ENABLED`, `CLOUD_SANDBOX_COMPUTE_ENABLED`,
`MARKETPLACE_COMPOSE_AND_LIST_ENABLED`) and even there the stub hooks / empty
stores make them exercisable-but-inert.

1. **Cloud fine-tuning** (`cloud/fine-tuning-service-routes.ts`): 404 when off.
   The real ledger-debit (`cloud-metering.ts settleCloudPrimitiveCharge`) is
   real and tested, but the route DEFAULTS to `stubFineTuningMeteringHook`
   (`metered:false`, no ledger). `index.ts` wires only `{authenticate,
   enabled}` — `makeLedgerFineTuningMeteringHook` is never instantiated in
   production. Arming the flag ALONE does not charge. Inert.
2. **Cloud sandbox** (`cloud/sandbox-compute-service-routes.ts`): same pattern;
   stub adapter + stub metering hook; live hook never wired; TTL abuse ceiling
   pre-dispatch. Inert.
3. **Cloud coding-sessions** (`cloud/cloud-coding-session-routes.ts`): 404 when
   off; stub adapter/hook; live hook never wired; placement policy refuses
   inadmissible repo-trust lanes BEFORE dispatch; stub leases no VM. Not even
   armed on staging. Inert.
4. **Marketplace monetize-accrual bridge / cross-category referral ledger**
   (`marketplace-monetize-any-layer-accrual.ts`,
   `referral-cross-category-accrual.ts`): flag-gated AND has NO production
   caller (unreachable from any route). Even if reached + armed, writes an
   eligibility row only; payout stays on the separate
   `TREASURY_DISPATCH_ENABLED=false` owner-armed dispatch rail. Inert.
5. **composed-run** (`autopilot-composed-run*.ts`): GET-only projection; store
   never passed from `index.ts`, so listing is always empty / `inert:true` /
   `promiseState:'planned'` even if armed. Moves no money. Inert.
6. **spark-helper-autostart** (`apps/pylon/src/spark-helper-autostart.ts`): pure
   classifier, default off, no production caller; explicitly starts no helper,
   spawns no process, moves no funds; receipt redacted by construction. Inert.
7. **demand-provenance split** (`accepted-outcomes-per-kwh.ts`): read-only
   metric; the seed datapoint is hardcoded `internal`,
   `externalDemandClaimAllowed=false`; copy gate forbids external-market
   framing. Moves no money. Inert.

No bypass path found. Tests assert BOTH inert-when-off (404) and no-op defaults
when armed.

Verdict: HONEST — all seven truly inert.

## Claim 3 — "Green-ready" claims honestly green-ready

All four are non-green (3 yellow, 1 red) — consistent with "green-READY, not
green". File-path evidence refs all dereference. Cited pass-counts verified
exact. Remaining gates are real missing capabilities, NOT lowered bars.

- **proof.claim_upgrade_receipts.v1** (yellow): the audit panel
  (`apps/web/.../page/loggedOut/page/promises.ts`) is real (isGreenFlip, owner
  signoff, green-flip tally, "receipt is not the flip" rule), with substantive
  render tests. Registry record correctly stays yellow because the panel lives
  in the web app, not the API. Remaining gate (deploy + owner-signed
  receipt-first upgrade) is real.
- **autopilot.mission_briefing.v1** (yellow): risk + receipts rollups exist in
  `autopilot-mission-briefing.ts`; route `GET /api/autopilot/work/{ref}/briefing`
  registered + in OpenAPI + capability manifest; `autopilot-work-routes.test.ts`
  has exactly 37 test cases incl. rollup-shape + no-secret-leak assertions.
  Remaining gate ("at least one live mission citing this briefing JSON") is real.
- **autopilot_sites.native_email_sequences.v1** (yellow):
  `site-form-spec-registry.ts` + test (exactly 7 cases). Adversarially
  confirmed: NO send vendor wired (no resend/sendgrid/postmark/etc.) and the
  capture route is NOT mounted in `index.ts`. Remaining gates (send service,
  deliverability, mount route, self-serve authoring) genuinely missing.
- **training.public_distributed_training_run.v1** (RED): all 8 file refs exist;
  scale-methodology doc states a >=50-contributor threshold "(stated, not yet
  met)". The record is scrupulous that 5 canary-scale settlements satisfy
  "multi-contributor settlement exists" but NOT network scale. Bar not lowered —
  if anything the methodology formalized/raised it. (Minor framing note: this
  record is red describing a red->yellow path, not strictly "green-ready".)

Verdict: HONEST.

## Claim 4 — safeCopy/unsafeCopy over-claim scan (31 touched records)

Reviewed the added/changed copy across all records the assault touched
(`1ecb39683..HEAD`). The added copy is heavily caveated and conservative:
dominated by "do not claim", "not yet", "planned", "internal not external",
explicit scope-downs. No copy was found that EXCEEDS its evidence.

One internal-contradiction WART found and FIXED (see Fixes): the green training
record stated its run total as 1,020 sats throughout but a trailing
simulation-exclusion sentence still read "excluded from the 1,005 real total" —
a missed find-replace from the destale, not an over-claim. Fixed to 1,020 for
self-consistency with the record's own already-merged figure.

Verdict: HONEST.

## Claim 5 — Cited receipts/docs dereference

- The destale doc `docs/promises/2026-06-19-training-live-run-evidence-destale.md`
  EXISTS (an initial lookup in `docs/launch/` was the wrong directory; the
  registry cites `docs/promises/...` and it dereferences). Content is
  internally consistent with the 5-contributor / 1,020-sat claim and the
  three new receipt ids.
- The three new receipt ids (`...ao6.final...`, `...ao6.patched...`,
  `...ao6.patched2...`) are anchored in the registry, the destale doc, and the
  June 18/19 roadmaps. They are described as LIVE production receipts served via
  `GET /api/public/nexus-pylon/receipts/<id>` and the per-run settlements feed.
  Their live dereference is a production-data claim that was intentionally NOT
  hit from this offline verification run; the on-disk evidence is internally
  consistent and the verification block gives runnable dereference commands.
- All file-path evidence across the four green-ready promises and the green
  training record dereferences on disk (the `2026-06-19` training methodology
  docs, the autostream capture doc + clip manifest, etc.).

Verdict: HONEST (no real dangling file refs).

## Fixes made (cheap, honest, no green flip, no gate weakened)

1. `product-promises.ts` (green record
   `training.decentralized_training_launch.v1`, safeCopy): corrected the
   trailing simulation-exclusion sentence from "excluded from the **1,005**
   real total" to "excluded from the **1,020** real total". This removes an
   internal contradiction (the rest of the same paragraph already states 1,020)
   and does NOT broaden any claim — 1,020 was already the record's stated total
   from the merged destale.

`check:deploy` GREEN after the fix (typecheck pass; web 203/203; api targeted
17/17; product-promises green-count test 3/3 with `toBe(20)`).

## Flagged for owner review (NOT fixed — conservative/under-stating, not over-claims)

After the `2026-06-19.7` destale moved the GREEN
`training.decentralized_training_launch.v1` to "five distinct independent
contributors / 1,020 sats", several RED sibling records still describe the live
run as "two distinct independent contributors / 1,005 sats" in their current
(non-dated-note) copy:

- `claims.world_first_ai_training_paid_bitcoin.v1` — safeCopy "two independent
  contributors" (line ~604); unsafeCopy "two bounded canary-scale settlements
  (1,005 sats real total)" (line ~606).
- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` — safeCopy "two distinct
  independent contributors ... (1,005 sats real total)" (line ~664); unsafeCopy
  "two counted run settlements" (line ~666); authorityBoundary "two counted
  bounded settlements" (line ~696).
- `pylon.largest_decentralized_training_claim.v1` — verification "the live run
  has two counted contributors" (line ~591); and registry note `2026-06-19.8`
  (line ~3323) "two counted contributors".

These are RED records using the count in conservative "do not extrapolate"
framing, so saying "two/1,005" UNDER-states the (now-five/1,020) live evidence —
the safe direction, not an over-claim. They were deliberately left unchanged
here rather than bumped to five/1,020, because raising them would propagate the
5/1,020 figure (backed by production receipts not dereferenced in this offline
run) into additional public records. Recommend the owner either (a) destale them
to five/1,020 once the receipts are re-confirmed live, or (b) leave them as the
conservative floor. Dated registry notes (lines ~175, ~184, ~3305, ~3322)
correctly retain 1,005 as point-in-time history and should NOT be changed.

## Method notes

- Green-set comparison via `awk` extraction of `promiseId` nearest each
  `state: 'green'`, diffed across `git show <ref>:<file>`.
- Touched-record mapping via unified-diff line numbers mapped to enclosing
  `promiseId`.
- Inert-scaffold and green-ready evidence audits run as parallel read-only
  sub-investigations.
- No production endpoints were called; no secrets printed; neutral commit
  metadata; never GitHub Actions.
