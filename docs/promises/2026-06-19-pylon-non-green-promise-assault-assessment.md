# Pylon Non-Green Promise Assault — Green-Readiness Assessment

Date: 2026-06-19
Branch: `assault-pylon`
Scope: the ten non-green `pylon.*` product promises.
HARD RULE: no promise is flipped green here. Green requires a dereferenceable
receipt AND owner sign-off, recorded receipt-first per
`proof.claim_upgrade_receipts.v1`. This doc assembles evidence and records the
exact remaining receipt + owner gate for each promise.

Source registry: `apps/openagents.com/workers/api/src/product-promises.ts`
(state derived from the `state` field on each promise record).

## Summary table

| Promise | State | Built / assembled this pass | Receipt needed for green | Owner-gated? |
| --- | --- | --- | --- | --- |
| `pylon.v03_release_candidate.v1` | yellow | npm latest=1.0.5 verified; release-signing + feed publish path documented | live-network smoke receipt + signed-binary feed rollout receipt (GCP publish) | YES (feed publish + flip) |
| `pylon.release_tomorrow.v1` | yellow | npm latest=1.0.5; platform scope (macOS/Linux) documented | signed-binary feed rollout receipt | YES (feed publish + flip) |
| `pylon.first_real_model_training_run.v1` | yellow | bounded two-device A1 run evidence confirmed live | a network-rung `training.model_ladder.v1` run receipt on real devices | YES (run + flip) |
| `pylon.largest_decentralized_training_claim.v1` | red | participant methodology + comparable-runs research written | a run at comparable contributor scale w/ public per-contributor receipts | YES (scale + flip) |
| `pylon.consumer_compute_earns_bitcoin_self_serve.v1` | red | scale methodology doc; INERT flag-gated Spark autostart capability + receipt builder | autostart receipt for ≥1 normal contributor + copy narrowed to macOS/Linux + scale | YES (flip) |
| `pylon.v0_3_multi_earning_node.v1` | red | per-mode earning inventory recorded | settled receipts for ≥2 earning modes in one install + safe projection | YES (flip) |
| `pylon.five_bitcoin_revenue_streams.v1` | planned | per-stream readiness inventory recorded | one settled receipt per stream (compute/data/labor/referral) | YES (flip) |
| `pylon.compute_revenue_modes.v1` | planned | no-spend GEPA loop confirmed; gap is paid settlement | one settled paid GEPA assignment receipt | YES (spend + flip) |
| `pylon.data_trace_revenue.v1` | planned | redaction core + marketplace gate confirmed | one settled, redacted trace-sale receipt | YES (flip) |
| `pylon.gepa_worker_loop_v03.v1` | planned | no-spend production smoke confirmed | one settled paid GEPA assignment receipt | YES (spend + flip) |

## Per-promise detail

### 1. `pylon.v03_release_candidate.v1` (yellow)

- Done: `@openagentsinc/pylon` package metadata is `1.0.5`
  (`apps/pylon/package.json`); the promise verification already records
  `npm view ... latest=1.0.5` on 2026-06-19. The install guide is Pylon-first
  (`apps/openagents.com/apps/web/public/INSTALL.md`).
- Remaining: two blockers — `pylon_v1_live_network_smokes_incomplete` and
  `pylon_v1_signed_binary_feed_rollout_incomplete`. The signed-binary feed
  publish is a GCP-only, secret-key-signed deploy
  (`apps/oa-updates/docs/release-signing-runbook.md`,
  `apps/oa-updates/scripts/publish-pylon-release.ts`) — an owner-gated action,
  not performed here.
- Receipt for green: (a) a dated live-network smoke receipt against production
  OpenAgents endpoints, and (b) a signed-feed rollout receipt naming the feed
  URL, v1.0.5 present for all four targets, signature verified, and a fresh
  install from the feed completing end to end.
- Owner-gated: YES (the feed publish and the green flip).

### 2. `pylon.release_tomorrow.v1` (yellow)

- Done: npm latest=1.0.5; platform scope is macOS + Linux only and is documented
  as a deliberate owner scope-out for Windows/WSL
  (`apps/pylon/docs/platform-support.md`).
- Remaining: `pylon_v1_signed_binary_feed_rollout_incomplete` — same owner-gated
  feed publish as #1.
- Receipt for green: the signed-binary feed rollout receipt (shared with #1).
- Owner-gated: YES.

### 3. `pylon.first_real_model_training_run.v1` (yellow)

- Done: the bounded two-device CS336 A1 real-gradient run is live and evidenced
  (`apps/openagents.com/docs/2026-06-11-cs336-a1-multi-device-real-gradient-evidence.md`;
  routes `/api/training/runs/run.cs336.a1.real_gradient.demo` and
  `/api/training/leaderboards/a1`).
- Remaining: `model_ladder_network_rungs_not_run` — the honest green path is a
  network-rung `training.model_ladder.v1` run on real contributor devices with
  commitment-backed verification and paid closeouts.
- Receipt for green: a network-rung execution receipt (multiple distinct
  devices; shard → commit → cross-device verify → merge/eval → payment →
  settlement with `realBitcoinMoved:true`).
- Owner-gated: YES (running the rung + flip).

### 4. `pylon.largest_decentralized_training_claim.v1` (red)

- Built this pass:
  `docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md`
  (the qualified-contributor counting rule, as enforced in
  `training-run-window-authority.ts`) and
  `docs/training/2026-06-19-comparable-decentralized-training-runs-research.md`
  (cited comparable runs: Templar Covenant-72B ~70 contributors; ~200 is the
  transcript target). These clear the "we never wrote the methodology /
  comparables" gap for `largest_training_participant_methodology_missing` and
  `comparable_training_run_evidence_missing`.
- Remaining: `public_training_contributor_receipts_missing` — OpenAgents has two
  counted contributors, far below ~70/~200. The methodology + research do NOT
  manufacture scale.
- Receipt for green: an actual run at comparable-or-greater verified-contributor
  scale with public per-contributor receipts.
- Owner-gated: YES.

### 5. `pylon.consumer_compute_earns_bitcoin_self_serve.v1` (red)

- Built this pass:
  - `apps/pylon/src/spark-helper-autostart.ts` — an INERT, flag-gated
    (`PYLON_SPARK_AUTOSTART`, default off) classifier + receipt builder that
    decides Spark-helper autostart readiness for a normal contributor and emits a
    public-safe `receipt.pylon.spark_helper_autostart.*` ref, with no live
    behavior change and no raw target/balance/credential leakage. Tests:
    `apps/pylon/src/spark-helper-autostart.test.ts` (9 pass).
  - The scale-methodology doc (#4 above) covers
    `consumer_compute_self_serve_scale_methodology_missing`.
- Remaining:
  - `spark_helper_autostart_receipt_missing` — the capability now exists INERT;
    green still needs a real autostart-ready receipt captured for ≥1 normal
    contributor on the self-serve path (the flag must be enabled and the helper
    must actually reach readiness).
  - `windows_wsl_consumer_install_coverage_missing` — Windows/WSL is a
    deliberate owner scope-out; the honest path is to NARROW the broad "anybody
    on any platform" copy to macOS/Linux, not to build Windows support.
- Receipt for green: an autostart-ready receipt for a normal contributor, copy
  narrowed to supported platforms, and a scale methodology applied.
- Owner-gated: YES.

### 6. `pylon.v0_3_multi_earning_node.v1` (red)

- Inventory: earning modes that currently settle = Tassadar executor training +
  Forum tips (`forum.content_tipping.v1` green). NIP-90 compute/data/labor rails
  exist in history and as schema (`nip90-market-receipts.ts`) but have no live
  settled receipts.
- Remaining: settled receipts across ≥2 modes in one install +
  modeled/observed/pending/paid/settled-distinguishing public projection.
- Receipt for green: a single Pylon identity with settled receipts from ≥2
  streams + a safe public projection.
- Owner-gated: YES.

### 7. `pylon.five_bitcoin_revenue_streams.v1` (planned)

- Inventory: tips live (green). Compute/data/labor/referral each need one live
  settled receipt; the porting plan is recorded in
  `apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`.
- Receipt for green: one settled receipt per stream (4) + a stacking proof.
- Owner-gated: YES.

### 8. `pylon.compute_revenue_modes.v1` (planned)

- Done: GEPA capability envelope + assignment runtime + 2026-06-11 live no-spend
  endpoint smoke (`apps/pylon/docs/2026-06-10-v03-live-worker-loop-smoke.md`).
- Remaining: `live_gepa_network_missing` reduces to a paid GEPA settlement.
- Receipt for green: one settled paid GEPA assignment receipt.
- Owner-gated: YES (operator spend approval + flip).

### 9. `pylon.data_trace_revenue.v1` (planned)

- Done: redaction core (`apps/pylon/src/proof-redaction.ts`) + marketplace gate
  (`apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md`,
  8-state lifecycle).
- Remaining: `settled_trace_sale_missing` — one public-safe settled trace sale.
- Receipt for green: a settled, redacted trace-sale receipt ref.
- Owner-gated: YES.

### 10. `pylon.gepa_worker_loop_v03.v1` (planned)

- Done: full v0.3 assignment/GEPA runtime + 2026-06-11 operator-closed no-spend
  production assignment smoke.
- Remaining: `paid_gepa_settlement_v03_missing` — identical real blocker to #8.
- Receipt for green: one settled paid GEPA assignment receipt from the v0.3 loop.
- Owner-gated: YES (operator spend approval + flip).

## What did NOT change

- No promise `state` was flipped. Green count stays 20.
- No npm package was published. No signed binary was published to the feed.
- No GCP deploy, no money movement, no owner-gated spend.
- The new Spark autostart capability is INERT (default off) and changes no live
  earning path.
