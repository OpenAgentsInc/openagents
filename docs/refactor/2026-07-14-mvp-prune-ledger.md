# MVP Hygiene Prune Ledger — 2026-07-14

- Class: cleanup record
- Date: 2026-07-14
- Scope: owner-directed repo-hygiene sweep — delete, deprecate, or archive
  anything not used by the MVP or the current roadmap
- Authorities consulted: `docs/sol/MASTER_ROADMAP.md` (rev 110),
  `docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md`,
  root `CLAUDE.md`/`AGENTS.md`, root `INVARIANTS.md`
- Pre-deletion baseline commit (restores everything below):
  `1d7991fef7da16b34bf167ba821350c9c04c7dea` (origin/main at sweep time).
  Restore any deleted path with `git show 1d7991fef7:<path>`.

## Deliberately out-of-bounds for this sweep

These surfaces were excluded up front and were NOT evaluated for removal:

- `apps/openagents-desktop/**`, `apps/pylon/**`, `packages/khala-tools/**`,
  `packages/mcp-contract/**`, `packages/environment-auth/**`,
  `packages/portable-session-contract/**`, `packages/sqlite-runtime/**`,
  `packages/khala-sync*/**` — active concurrent agent lanes.
- `docs/transcripts/**` (preserved by repo law, `INVARIANTS.md`), the
  product-promise integrity chain (`docs/promises/**`, promise
  registry/report APIs), `packages/behavior-contracts/**`, all
  `AGENTS.md`/`INVARIANTS.md`/`GUARANTEES.md` files, `specs/**`.
- `clients/khala-mobile`, `clients/khala-ios`, `clients/khala-code-desktop` —
  deprecated but explicitly RETAINED frozen migration/contract sources per
  `CLAUDE.md` ("remove only after parity, migration, and release proof").
- `crates/oa-node`, `crates/oa-codex-control`, `crates/oa-workroomd`,
  `crates/openagents-cloud-contract`, `docs/cloud/**`, `fixtures/cloud/**` —
  first-class in-repo Cloud infrastructure per `CLAUDE.md` (#8591); no
  retirement statement exists in the roadmap.

## Why the deletion set is small

`docs/sol/MASTER_ROADMAP.md` (rev 110) states directly: "Work claimed only
under a closed lane stops; code already landed on `main` remains dormant
substrate" and "Proven contracts and tombstones are not deleted merely to
express closure." Additionally, the admitted MVP AssuranceSpec
(`docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md`) pins the exact
full-repo test sweep command, which names nearly every workspace package
(`test:forge`, `test:sarah-take-scoreboard`, `test:autopilot-ui`, ...).
Removing any package named in that pinned command would invalidate the
admitted 18-obligation evidence index without a new owner decision. The
SOL-DOC-01..11 documentation cleanup (closed) already archived the historical
doc tree to Backroom with bidirectional receipts, so the doc surface was
recently curated by its own program.

Net honest result: 7 deletions, 0 backroom moves, 10 named candidates left in
place with blocking reasons.

## Disposition table

| Path | Disposition | Reason | Restoration pointer |
| --- | --- | --- | --- |
| `a.txt` | deleted | Git-workflow test detritus (content `local-edit`; commits "one"/"remote edit a"/"local edit a"). Zero repo references (all `a.txt` grep hits are temp-dir test fixtures inside `packages/khala-tools` tests). Not in roadmap. | `git show 1d7991fef7:a.txt` (last change `f87555fea0`) |
| `b.txt` | deleted | Test detritus (content `two`, commit "two"). Zero references. | `git show 1d7991fef7:b.txt` (last change `7a5c80cd1d`) |
| `config.txt` | deleted | Test detritus (content `safe`, commit "initial"). Zero references. | `git show 1d7991fef7:config.txt` (last change `f1a8e45cf9`) |
| `local-change.txt` | deleted | Test detritus (content `local`, commit "local change"). Zero references. | `git show 1d7991fef7:local-change.txt` (last change `cf15883e6a`) |
| `remote-change.txt` | deleted | Test detritus (content `remote`, commit "remote change"). Zero references. | `git show 1d7991fef7:remote-change.txt` (last change `95c136712e`) |
| `review.txt` | deleted | Test detritus (content `base`, commit "base"). Zero references. | `git show 1d7991fef7:review.txt` (last change `ce2d31b1f9`) |
| `seed.txt` | deleted | Test detritus (content `x`, commit "seed"). Zero references. | `git show 1d7991fef7:seed.txt` (last change `efc2561807`) |
| `.agents/skills/khala-fleet/` | candidate-not-removed | Owner said "probably can delete", but removal is blocked twice: (1) `clients/khala-code-desktop/tests/khala-bundled-skills.test.ts` byte-pins the canonical `SKILL.md` at `<repo root>/.agents/skills/khala-fleet/SKILL.md` against the generated embedded copy, and that frozen client is out-of-bounds for edits; (2) the product promise in `apps/openagents.com/workers/api/src/product-promises.ts` (~line 4970) cites `.agents/skills/khala-fleet/SKILL.md` as dereferenceable evidence — the promise integrity chain is protected. Removal needs a change inside the frozen client plus a promise-evidence update, i.e. its own bounded issue. | `git show 1d7991fef7:.agents/skills/khala-fleet/SKILL.md` (last change `214217cd44`) |
| `crates/oa-cloud-run-bridge/` | candidate-not-removed | `CLAUDE.md` labels it "Historical Cloud Run bridge — not new prod paths", but it is still live-wired: Cargo workspace member (root `Cargo.toml`), `docker/cloud/oa-cloud-run-bridge.Dockerfile`, live Terraform module `module.oa_cloud_run_bridge` (`infra/README.md`), and the production API deploy mounts its control token (`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`: `oa-cloud-run-bridge-control-token:latest`) with a live Cloud Run URL default in `.../scripts/cloudrun/render-env-yaml.ts`. Decommissioning is an infra change (Cloud Run service + Secret Manager + Terraform), not a repo prune. | crate at `1d7991fef7` (`crates/oa-cloud-run-bridge/`, landed `033386dc75`, `HISTORICAL.md` in-crate) |
| `runners/py-bench-runner/` | candidate-not-removed | Python benchmark runner migrated with the deliberate #8591 Cloud consolidation (`033386dc75`); referenced by protected `docs/cloud/BENCHMARK_CLOUD.md` and `docs/cloud/bootstrap/CND-048`/`CND-054`. No roadmap retirement statement; the Cloud lane is first-class per `CLAUDE.md`. Not in any bun/cargo build path, so it is inert but documented infra. | `git show 1d7991fef7 -- runners/py-bench-runner` (landed `033386dc75`) |
| `ops/owned-runner/` | candidate-not-removed | systemd unit+timer for the Khala Code QA nightly matrix (`bun run qa:nightly`, still a live root script). Documented by `docs/qa/khala-code-nightly-matrix.md`, and the units are plausibly deployed on the owned runner box (`/srv/openagents/openagents`). Removing the checked-in units without confirming the runner box is drained risks orphaning a live deployment. | `git show 1d7991fef7 -- ops/owned-runner` (last change `78a4aaff17`) |
| `packages/sarah-take-scoreboard/` | candidate-not-removed | Sarah surface is removed (#8610), but this package is named in the root `package.json` test aggregate AND in the pinned full-sweep command inside the admitted MVP AssuranceSpec (`docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md`). `docs/sarah/QUALITY_SCOREBOARD.md` is a retained historical record referencing it. Removal invalidates admitted assurance evidence — needs a new bounded owner decision. | `git show 1d7991fef7 -- packages/sarah-take-scoreboard` |
| `apps/autopilot-desktop/` | candidate-not-removed (frozen evidence) | DEPRECATED/FROZEN per `docs/DEPLOYMENT.md` ("DO NOT RELEASE... parity/extraction/migration evidence"); same retained class as `clients/khala-*`. Wired into root `package.json` verify scripts and referenced by `INVARIANTS.md` (legacy desktop lockout paths). | `git show 1d7991fef7 -- apps/autopilot-desktop` |
| `apps/forge/` | candidate-not-removed | "Legacy/postponed implementation source" per `CLAUDE.md`; still in the root test aggregate (`test:forge`) and the pinned AssuranceSpec sweep command. | `git show 1d7991fef7 -- apps/forge` |
| `clients/khala-mobile`, `clients/khala-ios`, `clients/khala-code-desktop` | candidate-not-removed (explicitly retained) | Deprecated frozen migration/contract/native-module extraction sources per `CLAUDE.md`: "remove the old clients only after parity, migration, and release proof." No such proof exists. | current `main` |
| Audio substrate (`crates/oa-desktop-audio`, `packages/audio-contract`, `apps/openagents-audio`, `apps/openagents-audio-edge`) | evaluated-kept | AUDIO-0..8 are closed not-planned, but MASTER_ROADMAP decision 23 explicitly "preserve[s] landed implementation evidence"; dormant substrate law forbids deletion to express closure. | current `main` |
| `infra/` (Terraform), `docker/cloud/`, `config/cloud/` | evaluated-kept | Live GCP production infrastructure after the Cloudflare exit: Terraform for Cloud Run/Cloud SQL/LB/secrets, Dockerfiles and env examples for the in-repo Cloud crates (#8591). This is the honest answer to the owner's "cloud/runner/infra shit?" — it is current production, not residue. | current `main` |
| `docs/reference/mpp/` (6.3 MB vendored mirror) | evaluated-kept | Named the "authoritative protocol spec... local mirror" by `docs/mpp/README.md` and the MPP launch runbook; MPP payments are live in production. | current `main` |

## Counts

- Deleted: 7 (all root-level git-test detritus files)
- Moved to backroom: 0 — every retired-but-historical candidate is blocked by
  live production references, frozen-client byte-pin tests, promise-evidence
  refs, or the pinned admitted AssuranceSpec sweep; each needs its own bounded
  issue and owner decision before it can move
- Candidates recorded, not removed: 10 rows above (including the owner-named
  `.agents/skills/khala-fleet` and `crates/oa-cloud-run-bridge`)

## How to restore

Git history is the archive for this sweep. Every deleted path exists at
baseline commit `1d7991fef7da16b34bf167ba821350c9c04c7dea`:

```sh
git show 1d7991fef7:<path> > <path>
```

No backroom intake was created by this sweep (nothing qualified for the
move-to-backroom tier).

## Part 2 — owner-directed supersession removals (2026-07-14)

Owner statement (verbatim, 2026-07-14):

> khala-code-desktop must itself be deprecated and all relevant promises
> removed (OpenAgents desktop supercedes it). ditto for
> apps/autopilot-desktop. sarah get rid of that too etc - i dont give a shit
> wut u do just get that shit cleared out

This statement is the new bounded owner decision Part 1 said the blocked
candidates needed. It explicitly supersedes the "remove only after parity,
migration, and release proof" retention clause for the named surfaces.

- Pre-removal recovery commit (origin/main immediately before this change):
  `c7044f5a2870110b331c5a7288caceb85488290a`. Restore any removed path with
  `git show c7044f5a28:<path>`.
- Backroom intake: `openagents-supersession-prune-2026-07-14/` in
  `OpenAgentsInc/backroom` (registry/contract/charter docs worth archaeology;
  bulk code recovery relies on git history at the commit above).
- Promise transitions: registry pass `2026-07-14.1` withdraws seven promises
  with successor `promise:openagents.desktop_app.v1` (green 34 -> 33); full
  record in `docs/promises/2026-07-14-owner-supersession-removals.md`.
  Withdrawals are downgrades — no `promise_transition` receipt required per
  the `mobile.autopilot_remote_control.v1` precedent.
- AssuranceSpec: proposed revision 3 created as
  `docs/mvp/openagents-codex-workroom-mvp.rev3-proposed.assurance-spec.md`
  (sweep command of record re-pinned without `test:sarah-take-scoreboard`;
  admitted rev-2 bytes, evidence index, and receipts untouched because they
  are digest-pinned admitted proof; admission of rev 3 is an owner/gate act).

### Part 2 disposition table

| Path / surface | Disposition | Reason | Restoration pointer |
| --- | --- | --- | --- |
| `apps/autopilot-desktop/` | **deleted** | Owner-named ("ditto for apps/autopilot-desktop"); OpenAgents Desktop supersedes it. No external package imported it; root scripts/workspace entry and perimeter-allowlist entries removed; CUT-26 `410` lockout routes in `apps/oa-updates` retained as the serving tombstone; promises withdrawn in `2026-07-14.1`. | `git show c7044f5a28 -- apps/autopilot-desktop`; charter/docs in backroom intake |
| `packages/sarah-take-scoreboard/` | **deleted** | Owner-named sarah cleanup; clean leaf (no runtime importers — only root scripts, the perimeter allowlist, docs, and the admitted-spec inventory named it). Root script + allowlist entries removed; proposed AssuranceSpec rev 3 re-pins the sweep. | `git show c7044f5a28 -- packages/sarah-take-scoreboard`; README+src in backroom intake |
| `.agents/skills/khala-fleet/` | **deleted** | Owner named it (via khala-code-desktop bundling) and Part 1's two blockers are cleared by this decision: the frozen client's byte-pin test was reduced to embedded-copy checks with a dated note, and `khala_code.bundled_fleet_skill.v1` is withdrawn with the evidence path tombstoned to git history. No live fleet-dispatch runtime reads the skill dir. | `git show c7044f5a28:.agents/skills/khala-fleet/SKILL.md`; copy in backroom intake |
| `clients/khala-code-desktop/` | **retained (deprecation stands; removal blocked)** | Owner ordered removal, but live code imports the tree: `apps/pylon/scripts/fleet-run-{live,sustained}-smoke.ts` (dynamic import of `src/bun/khala-fleet-tools.ts`; pylon is an active concurrent lane and live fleet production), `packages/khala-qa-harness` (9 files), `packages/harness-conformance` (4 files), `scripts/qa-nightly-matrix.ts`. Deleting it breaks live fleet smoke + QA lanes. Deprecation/frozen status recorded in `AGENTS.md`; physical removal needs a bounded dependent-migration issue. One edit made: the khala-fleet byte-pin test. | tree still on `main` |
| `packages/autopilot-ui/` | **retained** | Not autopilot-desktop-only: `apps/openagents.com/apps/web/package.json` depends on it and `apps/web/src/styles.css` `@import`s its stylesheet; token-parity tests in `packages/ui` and `packages/autopilot-control-protocol` read its source. Removing it breaks the live web build, so the owner's "etc" does not reach it. | tree still on `main` |
| `/api/sarah/fleet-runs` (FleetRun authority route) | **aliased (neutral canonical path added)** | Shipped OpenAgents desktop/mobile binaries hardcode `GET /api/sarah/fleet-runs` (`packages/khala-sync-client`), so the path cannot 410 without breaking fielded clients. `/api/fleet-runs` added as the neutral canonical path on the identical handler; the client helper now targets it for future builds; both paths tested. sarah_* DB tables/schema refs are durable authority vocabulary — renaming them is its own bounded issue. | `apps/openagents.com/workers/api/src/sarah-fleet-run-routes.ts` |
| `/api/operator/business/sarah-checkout-links` + `crm-sarah-handoff` store | **retained, ledger-noted** | Live CRM machinery consumes the handoff store (`crm-reply-routes.ts`, `crm-command.ts`, `crm-mcp.ts`) and D1 migration 0311 backs it. A rename/removal cascades through production CRM paths — deferred to its own bounded issue. | current `main` |
| Sarah-named promise IDs | **none exist** | No promiseId contains "sarah"; the FleetRun/CRM Sarah references in live promise/registry text describe retained authority surfaces. `khala_code.bundled_fleet_skill.v1` (the one promise whose evidence was a removed sarah-adjacent surface) is withdrawn. | — |

### Verification (Part 2)

- `bun test apps/openagents.com/workers/api/src/product-promises.test.ts` — pass.
- `bun run test:assurance-spec` — pass (compiler snapshot updated for the
  post-removal `bun.lock` dependency-lock digest).
- `bun packages/product-spec/src/cli.ts validate --specs-root specs` — pass.
- `bun run test:khala-code-desktop` — pass after the byte-pin edit.
- `bun run test:behavior-contracts`, `test:qa-pre-push-smoke`,
  `test:qa-nightly-matrix`, `test:harness-conformance`, `test:autopilot-ui`,
  khala-sync-client + FleetRun route/alias tests — pass.
- Pre-existing reds at the base commit (NOT introduced here, verified against
  pristine HEAD): `test:bun-api-perimeter` (unallowlisted `Bun.serve` in
  `apps/pylon/src/harness-mcp-server.ts` from FEED-1 `c7044f5a28`),
  `test:khala-qa-harness` (10 failures), the `worker-exact-routes` manifest
  order test (`/observer` + `/observer/traces/...` registered in `index.ts` by
  the Observatory change without approved-list entries; not part of
  `check:deploy`), and `typecheck:khala-code-desktop` (9 errors in
  `apps/pylon/src/orchestration/work-planner.ts`, `packages/pylon-core`,
  `apps/openagents.com/packages/effect-native-render-dom`, and
  `clients/khala-code-desktop/src/bun/claude-harness-status.ts` — byte-identical
  on a pristine HEAD worktree with its own fresh install). All belong to the
  pylon/QA/observatory/render-dom lanes; every other root typecheck lane passes
  post-removal.
