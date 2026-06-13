# AFK Autonomous Loop — standing instructions + live state

Date: 2026-06-13. Purpose: let the coordinator (Claude) drive the Autopilot
clients + self-driving-loop + OTA buildout autonomously for hours while the owner
is AFK, via `/loop`. This doc is the **source of truth** the loop re-reads each
iteration (so it survives context compaction). Append progress to §6.

## 0. The end-to-end goal + the long work queue (≥6h of runway)

The machine-economy loop, all on **our own machine + Cloud**, no Expo cloud:
phone-composed intent → Pylon hears it → coordinator fans out coding agents →
on merge, classify ship-mode (OTA vs rebuild) → ship back to the phone (OTA via
our own **OpenAgents Updates** server; native via a **local** build → App Store
Connect).

**Work through these PHASES in order. When a phase's fannable work is exhausted
or the only remainder is owner-gated, log it and MOVE TO THE NEXT PHASE — never
idle.** Keep a fanout running whenever fannable work exists.

- **Phase A — Mobile handshake + OTA (mostly done).** OTA server proven on our
  infra (real `expo export` served over HTTP) ✓; local `.ipa` build works ✓.
  Remainder: server signing-in-response + runnable `oa-update` CLI (fannable);
  the on-device update + a public host are **NEEDS-OWNER** (see §6). Also CL-7
  handshake demo (desktop+mobile show the same live session) is partly
  owner-gated (live boot).
- **Phase B — Desktop ↔ Mobile ↔ TUI parity.** Bring the new desktop app
  (`apps/autopilot-desktop`) AND mobile to **full functional parity with the
  current Pylon TUI** — this is **M3, issues #4921–#4930 (CL-15…CL-24)**:
  spawn/cancel, approvals (approve/deny/answer, exactly-once, read-only),
  steer/interrupt/pause/resume, accounts+quota, verify/dev-check status, node+
  provider status, artifacts+receipts, assignments, earnings (read-only), and
  the CL-24 parity conformance checklist. Build each across the shared protocol
  → `packages/autopilot-ui` components → desktop + mobile. Fan out the pure
  cores (protocol projections, view-models, components) the same way as CL-2;
  reconcile the live wiring as coordinator.
- **Phase C — Terminal-agent-systems roadmap to 100% parity.** Then work through
  `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`
  (Packs A–E / R0–R6, tracking issues **#4814–#4835**). Read it + the per-system
  audits in that folder; fan out the fannable pure cores (evidence/receipt
  schemas, event-log projections, permission/approval contracts, budget stops,
  scheduling/notification/task supervision projections, etc.), filing GitHub
  issues as needed, until parity. This is the deep well — plenty for hours.

## 1. Each loop iteration — do this, in order

1. **Reconcile in-flight work.** For any running `multi-session-run` (check
   `/tmp/*-proofs/multi-session-summary.json` + the runner PID): if its summary
   exists, review each session's diff, **run its verify from `main` yourself**
   (a session marked `failed` is often a transient runner/redaction hiccup — the
   code may be fine; trust the direct verify), then commit + merge the clean
   ones into `main`, run the touched tests on `main`, push, comment/close the
   issue, and remove the worktree + branch + `.pylon-*`/`/tmp` artifacts.
   **Never merge a red verify.**
2. **Monitor builds.** For any EAS/local build in flight: poll status. On
   `FINISHED`, proceed (e.g. it auto-submits). On `ERRORED`, fetch the build log
   (`mcp__expo__build_logs` or the local log), grep the bundle/error phase,
   diagnose, fix on `main`, push, and rebuild — the same self-correct loop that
   already fixed babel-preset-expo + metro hierarchical-lookup.
3. **Advance the OTA end-to-end.** Once the OTA pure cores merge
   (manifest-resolver, asset-store, publish-builder, code-signing), wire the
   `apps/oa-updates` Bun HTTP server (manifest endpoint + `/assets/:hash`),
   run it locally, do `expo export` → publish one update → curl the manifest
   endpoint with the right `Expo-*` headers and confirm a valid manifest. Log
   the result. (This is owner-visible proof the protocol works on our infra.)
4. **Keep the local build moving.** Get `eas build --platform ios --profile
   production --local` to produce an `.ipa`. Known prereqs installed this
   session: fastlane ✓. cocoapods (installing). Install any further missing
   local toolchain via `brew`. Run a local `expo export` pre-check before builds.
5. **Keep fanning out.** If no fanout is running and there is unblocked,
   bounded, file-disjoint work on the roadmap/CND backlog, launch the next batch
   (≤6 sessions, codex pool below, pre-provisioned worktrees off `main`,
   `noNetwork:false`, exact test-file verify, "keep edits to these files",
   "don't touch src/index.ts / root package.json"). Always pre-provision each
   worktree with `bun install`. This keeps the machine producing while AFK.
6. **Pace yourself.** Use ScheduleWakeup to re-enter: ~270s while actively
   watching a build/fanout, ~1200s when idle/waiting. When a tracked background
   watcher will re-invoke you on completion, prefer the long fallback.

## 2. Guardrails (hard)

- Work on `main` only; commit + push every merge (each repo separately:
  `openagents` and `cloud`). Co-author trailer as usual.
- **Cost:** prefer **local** builds; do NOT spin up more than one EAS *cloud*
  build per real need, and do not auto-submit repeatedly — only submit a build
  that's a meaningful new test target. Owning OTA is the whole point; lean on it.
- **Escalate, don't stall.** If blocked on something only the owner can do
  (Apple 2FA prompt, a spend/credential decision, a GitHub-org/connection,
  anything needing a human at apple.com), STOP that one thread, write a clear
  `NEEDS-OWNER:` line in §6, and keep working the other threads. Never idle.
- Never print raw tokens/secrets/credentials/mnemonics into files, commits,
  issues, or logs. Keep proof artifacts refs-only.
- Verify before merge; bounded fanout; clean up worktrees; don't create
  duplicate issues or re-do merged work.
- Don't change user-facing marketing copy.

## 3. Fanout mechanics (reference)

- Runner: `bun apps/pylon/scripts/multi-session-run.ts --plan <plan.json>
  --proofs-dir <dir> --pylon-home <dir> --concurrency N --run-id <id>` (nohup,
  background). Object-shaped plan with run-level `accountPool`.
- Codex account pool (all 7, for failover):
  `~/.codex`, `~/.codex-pylon-b`, `~/.codex-pylon-c`, `~/.codex-pylon-d`,
  `~/.codex-pylon-e`, `~/.codex-pylon-f`, `~/.codex-pylon-g`.
- Runbook: `docs/autopilot-coder/pylon-multi-session-agent-runbook.md`.
- A fresh worktree has no `node_modules` → `bun install` it before the run so
  `bun test` verifies pass.

## 4. Prioritized backlog (Phase A → B → C; fan out the pure cores)

**Phase A (finish):**
- OTA server: signing-in-response integration (server.ts uses
  `buildSignedManifestResponse`), runnable `oa-update` CLI bin. Cloud Rust port
  + CDN/multipart (cloud #82/#83/#85). On-device + public host = NEEDS-OWNER.
- M6 integration (#4940–4947): CL-34 RN compose screen, CL-36 coordinator
  dispatch wiring, CL-38 publish via our server, CL-39 local build, CL-40 status
  round-trip, CL-41 gating wiring.

**Phase B — TUI parity (#4921–#4930, M3):** for EACH, build protocol projection
+ `autopilot-ui` component (+ desktop render + mobile view-model) as fannable
pure cores, then wire:
- CL-15 spawn/cancel + list/detail · CL-16 approvals · CL-17 steer/interrupt/
  pause/resume · CL-18 accounts+quota · CL-19 verify/dev-check · CL-20 node+
  provider status · CL-21 artifacts+receipts · CL-22 assignments · CL-23
  earnings (read-only) · CL-24 parity conformance checklist (the gate).
- Cross-check against the real Pylon TUI surface in `apps/pylon` so "parity"
  means the desktop/mobile clients do what the TUI does today.

**Phase C — Terminal-agent-systems roadmap → 100%:** read
`terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`
+ `…-index.md` + the per-system audits; work the packs/issues #4814–#4835
(Pack A supervision/evidence, Pack B account/policy hardening, Pack C repo/
delivery, Pack D intake/market, Pack E polish) — fan out fannable pure cores
(schemas, projections, contracts, fixtures), filing issues as needed.

**Also:** #4949 (updates.url→ours + cert) once a public host exists; #4950
(local build pipeline — `eas build --local` works; prebuild+fastlane next).

## 5. Current state snapshot (update as it changes)

- TestFlight: build #4 (`46596803`) FINISHED + auto-submitting (new Control icon
  + OTA runtime `d36a2a5b`, channel `production`). Earlier #2/#3 errored
  (babel-preset-expo, metro hierarchical-lookup) — fixed.
- expo-updates wired on `main` (updates.url currently → `u.expo.dev`; switch to
  ours is #4949).
- OTA pure-core fanout RUNNING (run-id `run.local.ota`, watcher active).
- Local build: **WORKING** — `eas build --local` produced an 8.7 MB `.ipa` on
  this Mac (`/tmp/autopilot-local.ipa`), no Expo cloud. Prereqs: fastlane +
  cocoapods (both installed via brew). The "no Expo cloud build" half is proven.
- Merged so far: protocol (CL-0/1), autopilot-ui (CL-2) + web wiring, desktop
  P0 + RPC (CL-5), mobile control core + metro + token-store (CL-6), expo-updates
  config + policy (#4920), M6 cores (CL-34/35/36/37/41) + tokens (CL-42),
  CL-29/30/33 cores. Cloud OTA audit committed (`e083509`).

## 6. Progress log (append one line per iteration; newest at bottom)

> **NEEDS-OWNER (on-device OTA test) — non-blocking; lots of other work continues.**
> The whole OTA server pipeline is built + proven on our own infra (real
> `expo export` 2.1 MB Hermes bundle served as a manifest over HTTP, fingerprint
> gate + content-addressed assets + signing helper all green). To see an update
> land **on your phone**, two steps need you:
> 1. **A public host for the OpenAgents Updates server** — either a cloud deploy
>    target/domain (e.g. `updates.openagents.com`), or your OK to expose the
>    local server via a temporary tunnel (cloudflared/ngrok) for a throwaway
>    test. (I won't open a public tunnel without your say-so.)
> 2. **Installing the resulting local build** on your device (the build that
>    points `updates.url` at that endpoint).
> Everything else is ready: local `.ipa` builds work; the `updates.url` switch
> (#4949) is a one-line flip once the endpoint exists; `oa-update` publish works.
>
> **Deploy findings (2026-06-13) + plan:** owner wants `updates.openagents.com`
> on **GCloud** prod (cloudflare tunnel locally OK for the very first test).
> Access probe: `gcloud` authed (chris@openagents.com, project
> `openagentsgemini`) ✓; `cloudflared`+`docker` present ✓; the Cloudflare token
> in `.secrets/cloudflare-openagents.env` is **zone-READ only** — it can see the
> `openagents.com` zone (id `bd33d951ee951a7c18fa4ab2ddcbe3a7`) but **DNS write
> returns Authentication error**, and it has no account scope (so no cloudflared
> API tunnel). **Plan (autonomous):** containerize `apps/oa-updates` (seed a real
> `expo export` update at startup) → `gcloud run deploy` → use the stable
> `*.run.app` URL as `updates.url` for the initial on-device test (no DNS
> needed). **NEEDS-OWNER:** (1) **OWNER WILL ADD THE CNAME to Cloud Run** — so my
> deliverable is: deploy `oa-updates` to Cloud Run, create the domain mapping
> (`gcloud run domain-mappings create --domain updates.openagents.com --service
> oa-updates --region <r>`), and POST the **CNAME target + the `*.run.app` URL**
> right here in §6 for the owner to point DNS at; (2) confirm the right GCP
> project/billing if `gcloud run deploy` to `openagentsgemini` hits perms; (3)
> install the resulting build on your phone. Proceeding with the Cloud Run deploy
> after the containerize batch (p3) merges; URL/CNAME will be logged here.


- 2026-06-13: loop initialized; OTA pure-core fanout launched; cocoapods
  installing for local build; build #4 → TestFlight.
- 2026-06-13 iter 1: merged OTA pure cores CND-OTA-1..4 (`917e2bd16`, 14 tests),
  commented cloud #78–81; registered `apps/oa-updates` package (`8936bd4d2`);
  launched OTA server+CLI fanout (`run.local.ota2`); **LOCAL BUILD WORKS** —
  8.7 MB `.ipa` built on-machine (fastlane+cocoapods), no Expo cloud. Next:
  merge server/CLI, wire end-to-end manifest test, then a build with
  updates.url→ours.
- 2026-06-13 iter 2: merged OTA HTTP server + publishExport (`331e70563`, 18
  tests); registered `apps/oa-updates`; **E2E PASS** (`3ae455ee4`) —
  `scripts/e2e-local.ts` serves a real Expo-Updates manifest + asset over HTTP on
  our own infra (fingerprint gate works). OTA protocol proven, no Expo cloud.
  Remaining for on-device: sign manifests, deploy server publicly, switch
  updates.url (#4949), rebuild (local build works). Launching next OTA fanout:
  real-export CLI + signed manifest response.
- 2026-06-13 iter 3-4: merged OTA export-reader + signed-response + signing-in-server (real-export e2e PASS, 2.1MB Hermes bundle served over HTTP); merged CL-18 accounts + CL-20 node-status UI components (Phase B start). Deploy access probed (gcloud OK, CF token zone-read-only → DNS owner step). Launching: oa-updates containerize (Cloud Run) + CL-15/19/21/23 UI.

- 2026-06-13 iter 5 — **OTA SERVER DEPLOYED TO GCLOUD CLOUD RUN.**
  Live: `https://oa-updates-ezxz4mgdsq-uc.a.run.app` (also
  `https://oa-updates-157437760789.us-central1.run.app`), responds with
  expo-protocol-version:1 / noUpdateAvailable (empty registry; not seeded yet).
  Domain mapping created → **OWNER: add DNS `updates.openagents.com CNAME
  ghs.googlehosted.com`** (cert provisions once DNS is set). Merged Cloud Run
  containerize + CL-15/19/21/23 UI (39 tests). Next: bake a real expo-export
  update into the image + redeploy (OA_SEED_DIST/OA_SEED_RUNTIME) so the live
  server serves an actual update; then a build with updates.url→our endpoint.
  Launching p4: CL-16/17/22 parity components.

- 2026-06-13 iter 6 — **LIVE CLOUD RUN SERVER SERVES A REAL UPDATE.** Redeployed
  with a baked `expo export` seed (OA_SEED_DIST + OA_SEED_RUNTIME=d36a2a5b…);
  `https://oa-updates-ezxz4mgdsq-uc.a.run.app/autopilot/manifest` returns a real
  manifest (id seed-ios-…, 16 assets, content-addressed to our server). Full
  server-side OTA is operational in prod, no Expo cloud. Merged CL-16/17/22
  (M3 parity component set COMPLETE, CL-15..CL-23). Launching p5: CL-24
  conformance gate + mobile parity view-models. **On-device remaining (owner):**
  add the CNAME, then I build the app with updates.url→our endpoint (local
  build) and you install it. Next loop: Phase C (terminal-agent-systems roadmap).

- 2026-06-13 iter 7 — Phase B parity layer COMPLETE: merged CL-24 conformance
  gate + mobile parity view-models (47 tests). Starting **Phase C** (terminal-
  agent-systems roadmap): launched p6 = Pack A PA6 evidence-receipt (#4819),
  PA7 structured-event-log (#4820), PA1 task-supervision (#4814), PA2
  schedule-receipts (#4815) — each reads its audit + builds a pure schema/
  projection core in apps/pylon/src/tas/.
