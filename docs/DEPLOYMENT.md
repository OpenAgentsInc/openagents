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
