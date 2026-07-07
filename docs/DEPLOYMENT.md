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
| **openagents.com monolith (Cloud Run)** | the SAME app on Google Cloud Run (CFG-9 #8524; the Cloudflare-exit origin — services `openagents-monolith` + `openagents-monolith-staging`, us-central1). Bun entry `apps/openagents.com/workers/api/src/cloudrun/server.ts` wraps the Worker's fetch/scheduled handlers; per-minute Cloud Scheduler drives `POST /internal/cron`; Postgres/LiveHub/GCS seams activate by config. Prod DNS flips in CFG-10 (#8525). | `apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh` (header documents the Secret Manager map) | staging first: `bash apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh staging --with-scheduler` → smoke; then `… production --with-scheduler` → smoke `curl -fsS https://openagents-monolith-ezxz4mgdsq-uc.a.run.app/internal/healthz` | — |
| **Forge UI** | separate `forge.openagents.com` Cloudflare Worker UI app | `apps/forge/README.md` | `bun run --cwd apps/forge typecheck && bun run --cwd apps/forge test` → `bun run --cwd apps/forge deploy` → live smoke `curl -fsS https://forge.openagents.com/` | — |
| **Aiur (owner admin panel)** | owner-only admin app on **Google Cloud Run** (service `openagents-aiur`, us-central1; CFG-11 #8526), TanStack Start SPA shell + Bun proxy server, manual credit grants + ops views for the Khala Code mobile MVP. Legacy Worker still holds `aiur.openagents.com` until the owner DNS flip (see `NEEDS_OWNER.md`) | `docs/khala-code/2026-07-06-aiur-admin-deploy-runbook.md` | `bun run --cwd apps/aiur typecheck && bun run --cwd apps/aiur test` → `bun run --cwd apps/aiur deploy:cloudrun` → live smoke `curl -fsS https://openagents-aiur-157437760789.us-central1.run.app/api/aiur/access` (`AIUR_OWNER_USER_IDS` comes from Secret Manager `aiur-owner-user-ids` — fails closed without it) | — |
| **Pylon (npm)** | `@openagentsinc/pylon` CLI/runtime | `apps/pylon/docs/npm-publishing-runbook.md` | publish leaf deps first → `cd apps/pylon && bun run release:gate` → `bun pm pack` → `npm publish <tgz> --tag rc --access public` (**not** `bun publish`; `--tag rc` keeps `latest` stable; corgi manifest lags minutes after publish) | `pylon-v<version>` (prerelease for rc) |
| **Pylon RC binaries / OTA** | signed standalone binaries → auto-update feed | `apps/oa-updates/docs/release-signing-runbook.md` | `bash apps/pylon/scripts/build-rc-binaries.sh <version>` (ed25519-signed) → publish to `updates.openagents.com` | — |
| **Autopilot Desktop (macOS DMG)** | Electrobun desktop app | `apps/autopilot-desktop/README.md` (Release Builds) + `apps/autopilot-desktop/scripts/notarize-macos.sh` + the signing runbook | bump `electrobun.config.ts` version → `bun run --cwd apps/autopilot-desktop build:stable` (unsigned `.app`+`.dmg`) → `notarize:macos` (codesign `--options runtime` + `notarytool --wait` + staple the `.app`) → **re-create the DMG from the stapled `.app`** then codesign/notarize/staple the DMG → `gcloud storage cp …dmg gs://openagentsgemini-oa-updates/desktop/` → GitHub release pointing to it | `autopilot-desktop-v<version>` (prerelease) |
| **Khala Code Desktop (macOS DMG)** | Electrobun Codex-wrapper desktop app | `clients/khala-code-desktop/README.md` + `apps/autopilot-desktop/scripts/notarize-macos.sh` + the signing runbook + `NEEDS_OWNER.md` (#8245 gate) | bump `clients/khala-code-desktop/electrobun.config.ts` version → `bun run --cwd clients/khala-code-desktop release:plan -- --version <version> --channel rc --artifact ./Khala-Code-<version>.dmg` → owner runs `bun run --cwd clients/khala-code-desktop release:macos -- --version <version> --channel rc` on macOS with Developer ID/notary env → script reuses `notarize-macos.sh` for the `.app`, **re-creates the DMG from the stapled `.app`**, signs/notarizes/staples the DMG, stages `desktop/khala-code-desktop/<channel>/feed.json`, and only uploads/GitHub-releases when owner env flags are set → clean-Mac first-run smoke proves the app boots and gives the honest Codex install/login hint | `khala-code-desktop-v<version>` (`--prerelease --latest=false` for RC; stable only for non-prerelease) |
| **oa-updates (Cloud Run)** | the `updates.openagents.com` feed service | `apps/oa-updates/scripts/deploy-cloudrun.sh` + the signing runbook | `bash apps/oa-updates/scripts/deploy-cloudrun.sh` (project `openagentsgemini`, `us-central1`) | — |
| **oa-queue-worker (Cloud Run)** | **The single queue execution model** (CFG-7 #8522, CFG-16 #8532): there are no Cloudflare Queues anymore — the 8 CF queues were deleted post-cutover (#8532). Producers `INSERT` into the Postgres `oa_infra_jobs` table (oa-infra JobQueue, `FOR UPDATE SKIP LOCKED`); this **separate** Cloud Run pump leases per topic and delivers each job over HTTPS to the monolith's admin-bearer `POST /api/internal/queue/deliver` (2xx→ack, else nack→retry→dead-letter). The pump is kept as a deliberate bulkhead (queue processing isolated from the request-serving monolith; matches the audit's "Postgres job queue worked by Cloud Run workers"; avoids autoscale-multiplied lease pollers) — NOT collapsed in-process (CFG-16 decision). | `apps/oa-queue-worker/README.md` + `apps/oa-queue-worker/scripts/deploy-cloudrun.sh` | prod: `bun run --cwd apps/oa-queue-worker deploy` (service `oa-queue-worker`, `min-instances=1`, `OA_QUEUE_DELIVERY_URL=https://openagents.com`; secrets `oa-queue-worker-database-url` / `oa-queue-worker-delivery-token`); staging: same script with `OA_QUEUE_WORKER_SERVICE=oa-queue-worker-staging OA_QUEUE_WORKER_DB_SECRET=oa-queue-worker-staging-database-url OA_QUEUE_WORKER_TOKEN_SECRET=oa-queue-worker-staging-delivery-token OA_QUEUE_DELIVERY_URL=https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app` (post-cutover: the old `openagents-staging.openagents.workers.dev` target was the now-deleted frozen Worker). Jobs table ships with `packages/oa-infra/migrations/` (`bun packages/oa-infra/scripts/migrate.ts --database-url <direct-url>`). **Known gap (CFG-16):** the `event-ledger-ingest` topic dispatch still fails on Cloud Run — `EVENT_LEDGER_OWNER` is a typed-unavailable DO stub with no oa-infra Mutex/advisory-lock replacement wired yet (see #8532); event-ledger jobs delivered to Cloud Run 500→nack→dead-letter until that lands. | — |
| **Mobile (AutopilotRemoteControl)** | iOS owner operator app | `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md` | **NO Expo/EAS cloud (owner mandate).** Native `.ipa` builds locally (`expo prebuild` → `xcodebuild`/`fastlane`), TestFlight via `xcrun altool`; JS-only ships OTA via `apps/oa-updates/scripts/publish-ota.sh` (never `eas build/submit/update`) | — |
| **Pylon Cloud node** | managed/cloud Pylon node | `apps/pylon/docs/cloud-node-deployment.md` | see runbook | — |
| **SHC agent** | SHC agent deploy | `apps/openagents.com/docs/2026-06-02-shc-agent-deployment-runbook.md` | see runbook | — |
| **Nostr relay** | `relay.openagents.com` Cloudflare Worker + Durable Object (market rails + gated general coordination) | `apps/nostr-relay/README.md` | `bun run --cwd apps/nostr-relay typecheck && bun run --cwd apps/nostr-relay test` → `bun run --cwd apps/nostr-relay deploy` (= `wrangler deploy`). Set general-kind authorized pubkeys via `OPENAGENTS_RELAY_AUTHORIZED_PUBKEYS` (#5537). | — |
| **Verse world service** | Cloudflare Worker + Region Durable Objects + D1 for live Verse presence, local interaction, interest-scoped fanout, and world WebSockets | `apps/openagents-world/README.md` + `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md` | preflight world contract/client/service tests → `cd apps/openagents-world && bunx wrangler d1 migrations apply openagents-world --remote` → `bun run deploy` | — |

## Cloud SQL security posture (CFG-14 #8530 — Postgres ingress hardening)

Project `openagentsgemini`, region `us-central1`. Verify live with
`gcloud sql instances describe <inst> --format="value(settings.ipConfiguration.authorizedNetworks[].value,settings.ipConfiguration.sslMode)"`.

| Instance | Consumers & connection path | Posture (2026-07-07) | Remaining lockdown |
|---|---|---|---|
| **`khala-sync-pg`** (POSTGRES_17, primary Khala DB, public IP `34.70.178.7`; hosts BOTH `khala_sync_prod` and `khala_sync_staging`) | PROD `openagents-monolith`, `khala-live-hub`, `oa-queue-worker` connect over the **direct public IP** in their `*-database-url*-prod` secrets (`sslmode=require`). **STAGING peers cut over to the Cloud SQL Auth Connector 2026-07-07** (`--add-cloudsql-instances`, unix socket, no public IP) — see the CFG-14 cutover runbook below. | **SSL enforced** (`sslMode=ENCRYPTED_ONLY`). `ipv4Enabled=true`, `authorizedNetworks=[0.0.0.0/0]` still present (PROD still uses the public IP; staging + prod share this instance, so ingress can't close until BOTH are on the connector). | **Staging connector path PROVEN; PROD cutover is the remaining owner-confirmable step** (real prod-DB change — not done unattended). Run the **"CFG-14 prod cutover runbook"** below: connector + socket DSN on each of the 3 prod services, verify, then `--clear-authorized-networks`. Do **not** delete `0.0.0.0/0` before all 3 prod services are on the connector. |
| **`l402-aperture-db`** (POSTGRES_15, aperture LSAT store, public IP `34.46.174.166`) | Only consumer is Cloud Run `l402-aperture`, which connects via the **Cloud SQL connector** (`run.googleapis.com/cloudsql-instances=openagentsgemini:us-central1:l402-aperture-db`, private Google-internal socket — independent of `authorizedNetworks`). | **Locked down 2026-07-07:** `authorizedNetworks` cleared (public ingress closed) and `sslMode` raised to `ENCRYPTED_ONLY` (was `ALLOW_UNENCRYPTED_AND_ENCRYPTED`). Verified: `l402-aperture` reconnected to Postgres via the connector post-patch ("Using postgres as database backend", clean startup, no DB errors). | None — public ingress is closed; the connector path is unaffected. |

Rollback for either instance if serving breaks: re-add the network with
`gcloud sql instances patch <inst> --authorized-networks=0.0.0.0/0` (and, if needed,
`--ssl-mode=ALLOW_UNENCRYPTED_AND_ENCRYPTED`). After any change, verify
`curl -fsS https://openagents-monolith-ezxz4mgdsq-uc.a.run.app/internal/healthz`,
the Postgres-served counter `curl -fsS https://openagents.com/api/public/khala-tokens-served`,
and khala-live-hub `…/health`.

### CFG-14 prod cutover runbook — close `khala-sync-pg` public ingress (staging-proven 2026-07-07)

Goal: move the 3 prod consumers of `khala-sync-pg` off the direct public IP and
onto the **Cloud SQL Auth Connector** (unix socket at
`/cloudsql/openagentsgemini:us-central1:khala-sync-pg`), then remove
`0.0.0.0/0`. This was **rehearsed and PROVEN end-to-end on the `-staging`
peers** (evidence in #8530). **This changes the LIVE prod DB the mobile app
depends on — run it deliberately, verify after each service, roll back on any
failure. Do NOT touch `khala-sync-pg` ingress until all 3 prod services are
verified on the connector.**

**Two driver-specific socket forms (both PROVEN against a real socket+scram
Postgres and on staging Cloud Run — the naive `@/db?host=/cloudsql/…` form does
NOT work with our clients; see #8530 for the driver analysis):**

- **postgres.js** (`openagents-monolith`, `postgres@3.4.9`): postgres.js does
  **not** honor `?host=` or `?path=` in a connection string, and an
  authority-with-credentials + empty-host URL throws `Invalid URL`. The ONLY
  connection-string-compatible socket form is an **authority-less URL + libpq
  `PG*` env**:
  - `KHALA_SYNC_DATABASE_URL = postgres:///khala_sync_prod`
  - env `PGHOST=/cloudsql/openagentsgemini:us-central1:khala-sync-pg`,
    `PGUSER=khala_app`, and `PGPASSWORD` from a NEW password-only secret.
- **Bun.SQL** (`khala-live-hub` + `oa-queue-worker`, `new SQL({ url })`): Bun.SQL
  honors the **`?path=` query param** for the socket dir while taking
  credentials from the URL authority (the `localhost` host is ignored once
  `path` is set). Single self-contained secret, no `PG*` env:
  - `postgres://<user>:<pass>@localhost/khala_sync_prod?path=/cloudsql/openagentsgemini:us-central1:khala-sync-pg`

Both drivers dial `<path>/.s.PGSQL.5432`. IAM: the runtime SA
`157437760789-compute@developer.gserviceaccount.com` (all 3 prod services)
**already has `roles/cloudsql.client`** project-wide — no grant needed.
All `*-database-url*-prod` secrets are `:latest` refs, so adding a new version +
redeploying picks it up; **keep the old IP-DSN version as the rollback.**

```sh
INST=openagentsgemini:us-central1:khala-sync-pg
REGION=us-central1

# ---- 1) oa-queue-worker (Bun.SQL, ?path= single secret) ----
PW="$(gcloud secrets versions access latest --secret=oa-queue-worker-database-url \
      | python3 -c 'import sys,urllib.parse as u;print(u.urlparse(sys.stdin.read().strip()).password)')"
printf '%s' "postgres://khala_app:${PW}@localhost/khala_sync_prod?path=/cloudsql/${INST}" \
  | gcloud secrets versions add oa-queue-worker-database-url --data-file=-
gcloud run services update oa-queue-worker --region "$REGION" --add-cloudsql-instances "$INST"
#   VERIFY: new revision logs show ZERO `oa_queue_backend_error` and health
#   `cycles` advance (authed): TOKEN=$(gcloud auth print-identity-token);
#   curl -H "Authorization: Bearer $TOKEN" https://oa-queue-worker-ezxz4mgdsq-uc.a.run.app/

# ---- 2) khala-live-hub (Bun.SQL, ?path= single secret) ----
PW="$(gcloud secrets versions access latest --secret=khala-live-hub-database-url-prod \
      | python3 -c 'import sys,urllib.parse as u;print(u.urlparse(sys.stdin.read().strip()).password)')"
printf '%s' "postgres://khala_capture:${PW}@localhost/khala_sync_prod?path=/cloudsql/${INST}" \
  | gcloud secrets versions add khala-live-hub-database-url-prod --data-file=-
gcloud run services update khala-live-hub --region "$REGION" --add-cloudsql-instances "$INST"
#   VERIFY: curl -fsS https://khala-live-hub-ezxz4mgdsq-uc.a.run.app/health  -> {"ok":true,...}
#   and an authed DB-backed /log read returns a clean app response (410
#   cursor_behind), NOT a 5xx: HUBTOK=$(gcloud secrets versions access latest --secret=khala-live-hub-token);
#   curl -H "Authorization: Bearer $HUBTOK" ".../log?scope=scope.public.cfg14probe"

# ---- 3) openagents-monolith (postgres.js, authority-less URL + PG* env) ----
PW="$(gcloud secrets versions access latest --secret=openagents-monolith-database-url-prod \
      | python3 -c 'import sys,urllib.parse as u;print(u.urlparse(sys.stdin.read().strip()).password)')"
gcloud secrets create openagents-monolith-pgpassword --replication-policy=automatic --data-file=- <<<"$(printf '%s' "$PW")" 2>/dev/null \
  || printf '%s' "$PW" | gcloud secrets versions add openagents-monolith-pgpassword --data-file=-
printf '%s' "postgres:///khala_sync_prod" | gcloud secrets versions add openagents-monolith-database-url-prod --data-file=-
gcloud run services update openagents-monolith --region "$REGION" \
  --add-cloudsql-instances "$INST" \
  --update-env-vars "^@^PGHOST=/cloudsql/${INST}@PGUSER=khala_app" \
  --update-secrets PGPASSWORD=openagents-monolith-pgpassword:latest
#   VERIFY (admin bearer): db-smoke round-trip returns ok:true, khalaSyncTables>0:
#   curl -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
#     https://openagents-monolith-ezxz4mgdsq-uc.a.run.app/api/internal/khala-sync/db-smoke
#   plus /internal/healthz and https://openagents.com/api/public/khala-tokens-served.

# ---- 4) ONLY after all 3 verified on the connector: close public ingress ----
gcloud sql instances patch khala-sync-pg --clear-authorized-networks
#   (equivalently drop just the one entry; --clear-authorized-networks removes all).
#   Re-run the 3 smokes above; all must still pass with NO public IP anywhere.
```

**Rollback (per service, if its smoke fails — do this immediately, before moving
on):** re-add the original IP DSN as a new `:latest` version and revert the
service, e.g. for the monolith:
`ORIG=$(gcloud secrets versions access <prev-version> --secret=openagents-monolith-database-url-prod);
printf '%s' "$ORIG" | gcloud secrets versions add openagents-monolith-database-url-prod --data-file=-;
gcloud run services update openagents-monolith --region us-central1
--remove-cloudsql-instances openagentsgemini:us-central1:khala-sync-pg
--remove-env-vars PGHOST,PGUSER --remove-secrets PGPASSWORD`.
For the Bun.SQL services, just re-add the IP DSN version and
`--remove-cloudsql-instances`. **If ingress was already cleared**, restore it
with `gcloud sql instances patch khala-sync-pg --authorized-networks=0.0.0.0/0`.
The old IP-DSN secret versions are the canonical rollback — never destroy them
during the cutover.

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
