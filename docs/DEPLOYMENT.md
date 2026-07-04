# Deployment & Release Runbook — START HERE

**This is the single hub for every deploy / publish / release in this repo.** It
indexes the per-surface runbooks (the sources of truth) and gives the one-line
recipe, where the secrets live, and the conventions. When a surface's mechanics
change, update its linked runbook **and** fix the pointer here.

> Found this because you're about to ship something? Good. Read the linked runbook
> for the surface before running anything destructive/outward-facing.

## Conventions (all surfaces)

- **Deploy/publish only from a clean `origin/main`.** Parallel agent sessions leave
  uncommitted WIP in the shared working tree — never publish from it. Use an isolated
  `git worktree` off `origin/main` when the shared tree is dirty.
- **Release candidates are pre-releases.** Stable `latest` stays on the last stable
  (e.g. npm `latest: 0.2.5`); RCs go to the npm `rc` dist-tag and a GitHub
  **prerelease**. Never let an RC take the `latest`/"Latest" badge.
- **Sign, then verify.** Never publish unsigned wallet or desktop artifacts —
  clients fail closed on a bad/absent signature, and macOS Gatekeeper quarantines
  unsigned apps.
- **Secrets** live in `~/work/.secrets/` (workspace root, gitignored) and are mirrored
  in **GCP Secret Manager** (project `openagentsgemini`). Recovery steps are in the
  signing runbook. Never print secret values into tracked files, commits, or logs.

## Surfaces

| Surface | What | Canonical runbook | One-line recipe | GitHub release tag |
|---|---|---|---|---|
| **openagents.com Worker** | Cloudflare Worker + web app + API (product promises, Forum, Autopilot, current product API) | `apps/openagents.com/docs/2026-06-15-openagents-web-deploy-runbook.md` | **`bun run --cwd apps/openagents.com/workers/api deploy:safe`** (the ONLY sanctioned path — see the Worker deploy safety gate below). The final Worker upload uses `--containers-rollout=none` so local Docker/container probes cannot stall the safe deploy. | — |
| **Forge UI** | separate `forge.openagents.com` Cloudflare Worker UI app | `apps/forge/README.md` | `bun run --cwd apps/forge typecheck && bun run --cwd apps/forge test` → `bun run --cwd apps/forge deploy` → live smoke `curl -fsS https://forge.openagents.com/` | — |
| **Pylon (npm)** | `@openagentsinc/pylon` CLI/runtime | `apps/pylon/docs/npm-publishing-runbook.md` | publish leaf deps first → `cd apps/pylon && bun run release:gate` → `bun pm pack` → `npm publish <tgz> --tag rc --access public` (**not** `bun publish`; `--tag rc` keeps `latest` stable; corgi manifest lags minutes after publish) | `pylon-v<version>` (prerelease for rc) |
| **Pylon RC binaries / OTA** | signed standalone binaries → auto-update feed | `apps/oa-updates/docs/release-signing-runbook.md` | `bash apps/pylon/scripts/build-rc-binaries.sh <version>` (ed25519-signed) → publish to `updates.openagents.com` | — |
| **Autopilot Desktop (macOS DMG)** | Electrobun desktop app | `apps/autopilot-desktop/README.md` (Release Builds) + `apps/autopilot-desktop/scripts/notarize-macos.sh` + the signing runbook | bump `electrobun.config.ts` version → `bun run --cwd apps/autopilot-desktop build:stable` (unsigned `.app`+`.dmg`) → `notarize:macos` (codesign `--options runtime` + `notarytool --wait` + staple the `.app`) → **re-create the DMG from the stapled `.app`** then codesign/notarize/staple the DMG → `gcloud storage cp …dmg gs://openagentsgemini-oa-updates/desktop/` → GitHub release pointing to it | `autopilot-desktop-v<version>` (prerelease) |
| **Khala Code Desktop (macOS DMG)** | Electrobun Codex-wrapper desktop app | `clients/khala-code-desktop/README.md` + `apps/autopilot-desktop/scripts/notarize-macos.sh` + the signing runbook + `NEEDS_OWNER.md` (#8245 gate) | bump `clients/khala-code-desktop/electrobun.config.ts` version → `bun run --cwd clients/khala-code-desktop release:plan -- --version <version> --channel rc --artifact ./Khala-Code-<version>.dmg` → owner runs `bun run --cwd clients/khala-code-desktop release:macos -- --version <version> --channel rc` on macOS with Developer ID/notary env → script reuses `notarize-macos.sh` for the `.app`, **re-creates the DMG from the stapled `.app`**, signs/notarizes/staples the DMG, stages `desktop/khala-code-desktop/<channel>/feed.json`, and only uploads/GitHub-releases when owner env flags are set → clean-Mac first-run smoke proves the app boots and gives the honest Codex install/login hint | `khala-code-desktop-v<version>` (`--prerelease --latest=false` for RC; stable only for non-prerelease) |
| **oa-updates (Cloud Run)** | the `updates.openagents.com` feed service | `apps/oa-updates/scripts/deploy-cloudrun.sh` + the signing runbook | `bash apps/oa-updates/scripts/deploy-cloudrun.sh` (project `openagentsgemini`, `us-central1`) | — |
| **Mobile (AutopilotRemoteControl)** | iOS owner operator app | `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md` | **NO Expo/EAS cloud (owner mandate).** Native `.ipa` builds locally (`expo prebuild` → `xcodebuild`/`fastlane`), TestFlight via `xcrun altool`; JS-only ships OTA via `apps/oa-updates/scripts/publish-ota.sh` (never `eas build/submit/update`) | — |
| **Pylon Cloud node** | managed/cloud Pylon node | `apps/pylon/docs/cloud-node-deployment.md` | see runbook | — |
| **SHC agent** | SHC agent deploy | `apps/openagents.com/docs/2026-06-02-shc-agent-deployment-runbook.md` | see runbook | — |
| **Nostr relay** | `relay.openagents.com` Cloudflare Worker + Durable Object (market rails + gated general coordination) | `apps/nostr-relay/README.md` | `bun run --cwd apps/nostr-relay typecheck && bun run --cwd apps/nostr-relay test` → `bun run --cwd apps/nostr-relay deploy` (= `wrangler deploy`). Set general-kind authorized pubkeys via `OPENAGENTS_RELAY_AUTHORIZED_PUBKEYS` (#5537). | — |
| **Verse world service** | Cloudflare Worker + Region Durable Objects + D1 for live Verse presence, local interaction, interest-scoped fanout, and world WebSockets | `apps/openagents-world/README.md` + `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md` | preflight world contract/client/service tests → `cd apps/openagents-world && bunx wrangler d1 migrations apply openagents-world --remote` → `bun run deploy` | — |

## openagents.com Worker deploy safety gate (AAR 2026-06-25 — read before deploying)

**The ONLY sanctioned way to deploy the `openagents.com` Worker is
`bun run --cwd apps/openagents.com/workers/api deploy:safe`.** It runs, IN ORDER:

1. `check:deploy-from-main` — local HEAD must equal `origin/main` (no stale ship).
2. `check:deploy` — typecheck:web + typecheck:api + the real web/worker test
   suites + the contract-drift / architecture / effect-topology /
   public-projection guards + the deploy-guard self-tests
   (`test:pending-migrations-guard`). It does **NOT** depend on the flaky
   `verse-launch-smoke` (that desktop smoke was removed from `check:deploy`,
   #6234), so there is **no reason to ever bypass it** with a raw deploy.
3. **Staging first (#6409)** — applies
   `openagents-autopilot-staging` migrations, builds the web assets, deploys the
   staging Worker, then runs `predeploy:parallel-dispatch-smoke`. That smoke
   registers a staging-only dummy Codex Pylon, advertises five distinct dummy
   account slots, and dispatches five no-spend coding assignments concurrently.
   If any request is rejected with `duplicate_active_assignment` (or any other
   non-2xx dispatch failure), prod promotion stops.
4. **`wrangler d1 migrations apply openagents-autopilot --remote`** — migrations
   are applied to remote D1 **before** the worker is uploaded, always.
5. **`check:pending-migrations`** — runs `wrangler d1 migrations list … --remote`
   and **fails the deploy if ANY migration is still pending**, naming the files.
   This is the guard that makes "code shipped ahead of its schema" impossible.
6. `wrangler deploy --containers-rollout=none --assets …` — the production
   worker is uploaded last, without Wrangler container rollout probing.

#6409 canary/rollback posture: production promotion should use Cloudflare
Worker versions / Gradual Deployments for a 10% canary when the operator is
rolling a risky dispatch-gate change. Keep a Tail Worker attached to the
production Worker that watches for `duplicate_active_assignment` 409s during the
canary and triggers the operator rollback path immediately. Cloudflare documents
Worker gradual deployments as traffic splitting across versions, and Tail
Workers attach through `tail_consumers` to a Worker with a `tail()` handler.

**Raw `bunx wrangler deploy` / `npx wrangler deploy` is FORBIDDEN as a deploy
path** because it skips both `migrations apply` and `check:pending-migrations`.
That exact shortcut (`build:web && bunx wrangler deploy --assets`, taken to
dodge the flaky `verse-launch-smoke`) shipped the worker ahead of migration
`0234_pylon_openauth_links.sql` and caused the **2026-06-25 gateway-wide 500
outage** (every `POST /api/v1/chat/completions` returned 500 for all keys —
full AAR: `docs/incidents/2026-06-25-khala-500-completions-outage-aar.md`).

`deploy:safe` needs the owner OAuth env (`wrangler login` /
`CLOUDFLARE_API_TOKEN`) to reach remote D1; it cannot run unattended in CI. The
pure logic of `check:pending-migrations` IS unit-tested in CI
(`scripts/check-pending-migrations.test.ts`, wired into `check:deploy`).

## Verse World Service Gate

The Verse world backend is `apps/openagents-world`, a Cloudflare Worker +
Region Durable Object service with shared schemas in `packages/world-contract`
and the desktop/web mirror in `packages/world-client`. The canonical migration
ledger remains `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`.

Deploy readiness for this surface means the audit's P1-P13 sequence is green:
Effect Schema contracts, service/client authorization boundaries, Region Durable
Object hibernatable WebSockets, D1 projection tables, Queue/alarm expiry,
interest-scoped snapshots/deltas, handshake buffering, command sequence
receipts, moderation, two-client desktop smoke, web smoke, and public-safety
redaction tests.

The old self-hosted world module and generated TypeScript bindings have been
deleted. Do not regenerate or reintroduce them for new production world
behavior. Any useful schema or reducer pattern belongs in the Cloudflare/Effect
contracts and Worker service.

Operational details live in `apps/openagents-world/README.md`: D1 migrations
use Wrangler against `openagents-world` or `openagents-world-staging`, DO class
migrations use unique tags in `wrangler.jsonc`, `WORLD_BRIDGE_QUEUE` carries
compact bridge retry markers, and DO alarm expiry is validated through the
Effect-clock expiry tests plus two-client live smoke.

## Autopilot Desktop Verse smoke is the desktop release lane, NOT the Worker deploy gate

The interactive Verse / mouselook desktop UI smoke
(`apps/autopilot-desktop/scripts/verse-launch-smoke.ts`, run via
`bun run --cwd apps/autopilot-desktop smoke:verse-launch` and inside
`verify:deploy`) belongs to the **Autopilot Desktop release lane**, not the
`openagents.com` Worker/web deploy gate. It launches an Electrobun build + a
headless Chrome (CDP) and validates desktop UI; it does **not** validate the
Worker/web change.

- It is **removed from `check:deploy`** (the Worker deploy + pre-push gate). The
  Worker gate still runs the real checks: typecheck:web/api, the web + worker
  test suites, and the contract-drift / architecture / effect-topology /
  public-projection guards. Putting the desktop smoke on that critical path
  previously hung / OOM'd in headless/CI and SIGKILLed the whole deploy
  (`DEPLOY_EXIT=137`) before `wrangler deploy` ran (issue #6234).
- Run the desktop smoke on the desktop build/release path — see the
  **Autopilot Desktop (macOS DMG)** row above — or standalone with
  `bun run --cwd apps/autopilot-desktop verify:deploy` (full desktop lane) or
  `bun run --cwd apps/autopilot-desktop smoke:verse-launch` (smoke only).
- To gate only when desktop files changed, use
  `bun run verify:autopilot-desktop:if-changed` (root). Force with
  `OA_FORCE_DESKTOP_VERIFY=1`.
- It is **hard-bounded**: `smoke:verse-launch` runs through
  `scripts/run-bounded.ts` with a wall-clock timeout (default 480s, override
  with `OA_VERSE_SMOKE_TIMEOUT_MS`). On timeout it SIGTERM→SIGKILLs the whole
  child process group (Electrobun + Chrome) and exits non-zero (124) — fail
  fast and loud, never an unbounded hang. Use
  `smoke:verse-launch:unbounded` only for local debugging.

## Owned Visibility Freshness Smoke

Full visibility/replay operations runbook:
`docs/launch/2026-06-19-visibility-replay-operations-runbook.md`.

Run the Pylon visibility/replay freshness smoke from owned local, CI, or
container infrastructure, not GitHub Actions:

```sh
node apps/openagents.com/scripts/visibility-freshness-smoke.mjs \
  --base-url https://openagents.com
```

The smoke checks public route status, activity-timeline `generatedAt` and
`projection_staleness.v1`, source-lag rows, the SSE stream route, replay-clip
render queue freshness, and R2 clip manifest/artifact availability. It exits nonzero for
alerting unless `--warn-only` is used to collect an evidence report during a
known incident. Failures name the stale source or broken route so the operator
can route the fix without manual page refreshes.

### Pylon npm vs signed OTA feed

The Pylon npm RC and signed standalone RC feed are independent release surfaces.
Publishing `@openagentsinc/pylon@rc` with `npm publish --tag rc` only updates the
npm install path. The signed auto-update feed at `updates.openagents.com/pylon/rc/...`
only moves after the signed binary flow (`apps/pylon/scripts/build-rc-binaries.sh`)
and the `oa-updates` publish/deploy path run. Do not infer one surface's current
version from the other.

## Signing & secrets — the trust layer

**Runbook: `apps/oa-updates/docs/release-signing-runbook.md` — read before any signed release.**

- **ed25519 release key** (signs Pylon binaries + OTA manifests; clients pin + fail
  closed): `~/work/.secrets/openagents-release-signing.env`; public key pinned at
  `apps/oa-updates/keys/release-pubkey.json`; GCP backup secret
  `openagents-release-signing-key`. Sign with
  `bun apps/oa-updates/scripts/sign-release.ts <artifact>`, verify with
  `verify-release.ts`.
- **Apple Developer ID** — `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`,
  team `HQWSG26L43` — signs/notarizes the desktop `.app`/`.dmg`. Material: login
  keychain + `~/work/.secrets/developer-id/` (`.p12` + key + chain) + notary creds in
  `~/work/.secrets/appstoreconnect.env` (`ASC_API_KEY_ID`/`ASC_API_ISSUER_ID`/
  `ASC_API_PRIVATE_KEY_PATH`, `OA_DEVELOPER_ID_APPLICATION`). GCP backup:
  `developer-id-application-p12` (+ `-password`). `notarize-macos.sh` reads
  `OA_ASC_ENV` (point it at `~/work/.secrets/appstoreconnect.env`). Confirm the cert:
  `security find-identity -v -p codesigning | grep HQWSG26L43`.
- **npm automation token** — `~/work/.secrets/npm-publish.env` (`NPM_PUBLISH_TOKEN`) +
  `~/.npmrc`. `npm whoami` must return `openagentsinc`. Scope is `@openagentsinc/`
  only (never `@openagents/`).
- **Recovery on a fresh machine** is `gcloud secrets versions access …` per the signing
  runbook (gcloud must be authed to project `openagentsgemini`).

## Worked example — Pylon + Autopilot Desktop v1.0.0-rc.3 (2026-06-16)

1. Published the new leaf dep `@openagentsinc/autopilot-control-protocol@0.1.0`, then
   `@openagentsinc/pylon@1.0.0-rc.3` to the npm `rc` tag (`latest` stayed `0.2.5`),
   then the GitHub prerelease `pylon-v1.0.0-rc.3`.
2. Autopilot Desktop: bumped `electrobun.config.ts` → `1.0.0-rc.3` → `build:stable` →
   `notarize:macos` (.app accepted "Notarized Developer ID") → re-created + signed +
   notarized the DMG → uploaded to `gs://openagentsgemini-oa-updates/desktop/` →
   GitHub prerelease `autopilot-desktop-v1.0.0-rc.3`.
