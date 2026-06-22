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
| **openagents.com Worker** | Cloudflare Worker + web app + API (product promises, Forum, Autopilot/Forge UI) | `apps/openagents.com/docs/2026-06-15-openagents-web-deploy-runbook.md` | `bun run build:web` → `cd workers/api && npx wrangler deploy --assets ../../apps/web/dist` (the `--assets` is **mandatory** or you ship stale UI; worker-only: add `--containers-rollout=none`) | — |
| **Pylon (npm)** | `@openagentsinc/pylon` CLI/runtime | `apps/pylon/docs/npm-publishing-runbook.md` | publish leaf deps first → `cd apps/pylon && bun run release:gate` → `bun pm pack` → `npm publish <tgz> --tag rc --access public` (**not** `bun publish`; `--tag rc` keeps `latest` stable; corgi manifest lags minutes after publish) | `pylon-v<version>` (prerelease for rc) |
| **Pylon RC binaries / OTA** | signed standalone binaries → auto-update feed | `apps/oa-updates/docs/release-signing-runbook.md` | `bash apps/pylon/scripts/build-rc-binaries.sh <version>` (ed25519-signed) → publish to `updates.openagents.com` | — |
| **Autopilot Desktop (macOS DMG)** | Electrobun desktop app | `apps/autopilot-desktop/README.md` (Release Builds) + `apps/autopilot-desktop/scripts/notarize-macos.sh` + the signing runbook | bump `electrobun.config.ts` version → `bun run --cwd apps/autopilot-desktop build:stable` (unsigned `.app`+`.dmg`) → `notarize:macos` (codesign `--options runtime` + `notarytool --wait` + staple the `.app`) → **re-create the DMG from the stapled `.app`** then codesign/notarize/staple the DMG → `gcloud storage cp …dmg gs://openagentsgemini-oa-updates/desktop/` → GitHub release pointing to it | `autopilot-desktop-v<version>` (prerelease) |
| **oa-updates (Cloud Run)** | the `updates.openagents.com` feed service | `apps/oa-updates/scripts/deploy-cloudrun.sh` + the signing runbook | `bash apps/oa-updates/scripts/deploy-cloudrun.sh` (project `openagentsgemini`, `us-central1`) | — |
| **Mobile (AutopilotRemoteControl)** | iOS owner operator app | `clients/mobile/AutopilotRemoteControl/TESTFLIGHT.md` | **NO Expo/EAS cloud (owner mandate).** Native `.ipa` builds locally (`expo prebuild` → `xcodebuild`/`fastlane`), TestFlight via `xcrun altool`; JS-only ships OTA via `apps/oa-updates/scripts/publish-ota.sh` (never `eas build/submit/update`) | — |
| **Pylon Cloud node** | managed/cloud Pylon node | `apps/pylon/docs/cloud-node-deployment.md` | see runbook | — |
| **SHC agent** | SHC agent deploy | `apps/openagents.com/docs/2026-06-02-shc-agent-deployment-runbook.md` | see runbook | — |
| **Nostr relay** | `relay.openagents.com` Cloudflare Worker + Durable Object (market rails + gated general coordination) | `apps/nostr-relay/README.md` | `bun run --cwd apps/nostr-relay typecheck && bun run --cwd apps/nostr-relay test` → `bun run --cwd apps/nostr-relay deploy` (= `wrangler deploy`). Set general-kind authorized pubkeys via `OPENAGENTS_RELAY_AUTHORIZED_PUBKEYS` (#5537). | — |
| **Verse world service** | Cloudflare Worker + Region Durable Objects + D1 for live Verse presence, local interaction, interest-scoped fanout, and world WebSockets | `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md` until `apps/openagents-world/README.md` exists | scaffold `packages/world-contract`, `packages/world-client`, and `apps/openagents-world`; run the P1-P13 gates in the audit before any production cutover | — |

## Verse World Service Gate

The new Verse world backend is the planned `apps/openagents-world` Cloudflare
Worker + Region Durable Object service, with shared schemas in
`packages/world-contract` and the desktop/web mirror in `packages/world-client`.
Until those packages and the service README exist, the canonical cutover plan is
`docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`.

Deploy readiness for this surface means the audit's P1-P13 sequence is green:
Effect Schema contracts, service/client authorization boundaries, Region Durable
Object hibernatable WebSockets, D1 projection tables, Queue/alarm expiry,
interest-scoped snapshots/deltas, handshake buffering, command sequence
receipts, moderation, two-client desktop smoke, web smoke, and public-safety
redaction tests.

`apps/openagents-world-spacetimedb/` and its generated TypeScript bindings are
historical source material during the ripout. Do not regenerate or extend them
for new production world behavior. Any useful schema or reducer pattern should
be ported into the Cloudflare/Effect contracts and then the old binding path
deleted as part of the mandatory decommission issue.

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
render queue freshness, and R2 clip manifest/artifact availability. Any
remaining SpacetimeDB bridge observation in this smoke is legacy cutover evidence
only and must be removed with the old backend path. It exits nonzero for
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
