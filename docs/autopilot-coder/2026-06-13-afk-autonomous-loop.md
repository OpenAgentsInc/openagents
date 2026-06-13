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
   ones into `main`, run the touched tests on `main`, push, and remove the
   worktree + branch + `.pylon-*`/`/tmp` artifacts. **Never merge a red verify.**
   **CLOSE the issue (with a commit-citing comment) the moment the merged work
   fully satisfies its stated acceptance — do NOT leave completed issues open.**
   If only a core/partial landed, comment what merged + what remains and keep it
   open. Don't just comment-and-leave-open on done work; that is how stale
   open-issue piles form.
1b. **Clarity sweep (every few iterations).** Run `gh issue list --state open`
   and for each: close any whose acceptance is now genuinely met (citing comment);
   for partials, make sure the latest status comment is CURRENT (what's merged +
   what remains) so nothing is stale or forgotten. Partials are fine — staleness
   isn't. Keep the open list a true reflection of remaining work.
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
5. **NEVER IDLE — always have a fanout in flight.** The instant you finish
   merging a batch, **launch the next batch in the SAME turn** (≤6 sessions,
   codex pool below, pre-provisioned worktrees off `main`, `noNetwork:false`,
   exact test-file verify, "keep edits to these files", "don't touch
   src/index.ts / root package.json"; `bun install` each worktree first). Do NOT
   end a turn with zero fanouts running. There is always more backlog
   (Phase A→B→C; the deep TAS well) — if one phase is owner-gated, pull the next.
   Chain it every iteration: merge → **launch next** → arm watcher → yield.
   **Partials are fine — the bar is CLARITY + nothing stale/forgotten:**
   (a) every batch maps to ≥1 **OPEN** issue it advances (don't build cores for
   already-closed or out-of-scope issues — that's wasted motion);
   (b) every issue a batch touches gets a **current status comment** ("merged X
   on `<sha>`; remains Y") so its true state is always visible and never goes
   stale;
   (c) **close** when the acceptance is genuinely met; partials stay open with the
   up-to-date comment;
   (d) when an issue's only remainder is **live integration** (not a fannable
   core), do that integration yourself as coordinator.
   Keep the open list honest: each issue reflects exactly what's done vs left.
6. **Pacing = the fanout watcher, not a timer.** Arm a background watcher on each
   launched fanout; its completion re-invokes you immediately — that IS the
   heartbeat, no fixed delay. ScheduleWakeup is ONLY a long safety net (~1800s)
   in case a watcher dies; it is NEVER the primary cadence, and you must never
   set a short wakeup and sit idle. While a fanout runs + its watcher is armed,
   work is continuous.

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
- **Close issues on completion** (commit-citing comment) — never accumulate
  done-but-open issues; the open list must reflect only real remaining work.
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
- 2026-06-13 iter 8 — merged TAS Pack A PA6/PA7/PA1/PA2 cores (16 tests, apps/pylon/src/tas/). Launched p7: PA9 approval-contract (#4822), PA8 budget (#4821), PA5 smoke-proof (#4818), PA10 non-interactive (#4823) — Pack A nearly complete (PA3 notifications done earlier as CL-30; PA4 mobile companion = the parity work).
- 2026-06-13 iter 9 — TAS **Pack A COMPLETE** (PA1/2/5/6/7/8/9/10 + PA3=CL-30; 31 TAS tests). Launched Pack B: PB1 credential-store (#4825), PB2 effective-config (#4826), PB5 retention (#4829), PB6 managed-policy (#4830).
- 2026-06-13 iter 10 — TAS **Pack B COMPLETE** (PB1/2/5/6; 52 TAS tests). Launched Pack C: PC1 repo-identity (#4832), PC2 diff-review (#4833), PC3 workspace-boundary (#4834), PC4 delivery-receipt (#4835).
- 2026-06-13 iter 11 — TAS **Pack C COMPLETE** (PC1/2/3/4; 67 TAS tests, 16 files). Launched Pack D + cross-cutting: work-intake, error-taxonomy, tool-registry, multi-agent coordination.
- 2026-06-13 iter 12 — merged Pack D + cross-cutting (work-intake, error-taxonomy, tool-registry, coordination; 85 TAS tests, 20 files). Launched memory/context family: context-assembly, compaction, session-memory, semantic-retrieval (cosine).
- 2026-06-13 iter 13 — merged memory/context family (context-assembly, compaction, session-memory, semantic-retrieval/cosine; **100 TAS tests, 24 files**). Launched protocol/extensibility family: mcp-client, mcp-server, hook-event, command-system.
- 2026-06-13 iter 14 — merged protocol/extensibility (mcp-client/server, hook-event, command-system; **117 TAS tests, 28 files**). Launched model/config family: model-provider, prompt-layering, skill-system, plugin-system.

- 2026-06-13 OWNER-FLAG FIX — owner saw build #4 still empty. ROOT CAUSE: all the
  parity/OTA work was libraries+backend, never wired into mobile SCREENS, and no
  OTA was ever pushed to build #4. FIX: rewrote app/nodes.tsx (real dark screen +
  node-status view-model + honest empty state + live expo-updates OTA badge in
  footer), JS-only (fingerprint still d36a2a5b == build #4), then `eas update
  --branch production` (update 019ec158) + LINKED channel production→branch
  production (was unlinked — the missing piece). Build #4 now pulls the OTA on
  next cold start. LESSON: wire screens + push OTA, don't just build libraries.
- 2026-06-13 iter 15 — OWNER: OTA auto-updater (5s poll→download→reload + overlay) + white navbar removed, OTA 019ec15d to build #4; in-app name Autopilot. Home-screen name deferred to next build (keeps build #4 OTA-able). Merged p13 3/4 (prompt-layering/skill/plugin; 128 TAS tests, mprov cancelled→requeued). Launched p14: model-provider(retry), telemetry, resume-rewind, migration.
- 2026-06-13 — BUILD 5 shipped: name=Autopilot, built LOCALLY (eas build --local, 8.8MB .ipa, no Expo cloud) + submitted to TestFlight (submission b7e4705c, ASC app 6779949704). New runtime (name change); future OTAs from main target build 5. Merged p14 (143 TAS tests, 35 files).

- 2026-06-13 iter 16 — OWNER clarified pairing transport: NOT Tailnet-only —
  support BOTH same-Wi-Fi (QR→LAN ip, no Tailnet) AND Tailnet, same handshake,
  Tailnet-first. Updated roadmap CL-8 (bind loopback+LAN+tailnet, auth off-loopback)
  + CL-13 (QR payload carries reachable address(es)+bootstrap). Merged p15 bridge
  client cores (bridge-client, pairing-client, mobile pairing-view-model; 26 tests).
  Launched dual-transport cores: bootstrap-payload codec, address-resolution
  (tailnet-first/LAN-fallback), Pylon bind-config. NEXT integration (mine/operator):
  CL-8 socket bind + node QR render + live handshake → real paired session on phone.
- 2026-06-13 iter 17 — OWNER: close issues on completion (not just comment). Added close-on-complete mandate + an issue-close sweep step to §1/§2. Closed the 4 fully-done: #4919 (TestFlight), #4920 (OTA), #4948 (dark tokens), #4950 (local build). Open 44→40 (remaining = live-wiring partials + M4/not-started + ~8 older-program issues #4749-4786).
- 2026-06-13 iter 17b — merged dual-transport pairing cores (bootstrap-payload, address-resolution tailnet-first, pylon bind-config; 35 tests). Pairing PROTOCOL+CLIENT layer now built (p15+p16). Remaining for live pairing = INTEGRATION (mine): CL-8 socket bind to LAN/tailnet, node QR render, live handshake, mobile pairing screen + subscription render. Open issues 40 (close-on-completion now mandated).
- 2026-06-13 iter 18 — merged p17 pairing live-path cores (pairing-offer/QR, session-subscription, pairing-flow; 23 tests). Pairing pure-core layer COMPLETE; live integration is coordinator work. NO-IDLE: launched p18 same turn — TAS team-memory, repo-memory, eval-regression, performance.
- 2026-06-13 iter 18b — OWNER: no issue closes visible. Root cause: merging partial CORES (feed already-closed CL-9/10/13 or not-yet-live CL-6/14), not COMPLETING issues. Fix: runbook now requires every batch map to an OPEN issue it CLOSES; live integration done by coordinator. Closed CL-24 #4930 (conformance gate met) → 39 open. PIVOT: next focus = live bridge/pairing integration to actually close CL-14/#4908/#4909 + render the new autopilot-ui components in web/desktop to close the M3 component issues.
- 2026-06-13 iter 19 — **CL-6 #4908 CLOSED (auto-detect handshake working end-to-end).** Fixed RN bundle (pure base64url + Symbol.for inspect, no node:util/Buffer; babel unstable_transformImportMeta for effect's import.meta); shipped auto-detect + live session-detail timeline OTA (group 1581c9cc, runtime 302fb167). Built node self-registration → broker (startDiscoveryHeartbeat; buildBrokerRegistrationBody/postNodeRegistration) + made broker self-pruning (ISO-timestamp fix, prune-on-list) + redeployed Cloud Run (rev 00004). session.events now returns inline recentEvents tail (RN can't stream SSE). Fanout merged CL-40 ship-status-roundtrip #4946 + CL-41 ship-spend-gate #4947 cores (11 tests, commented partial). Open 39→38.
- 2026-06-13 iter 19 OWNER-LIVE — owner on cellular, NEEDS tailnet. Plain-http tailnet connect failed ("Network request failed"). Diagnosis: build 9 has ATS exception (received 302fb167 OTA whose fingerprint includes NSAllowsArbitraryLoads), so likely the phone's Tailscale wasn't routing; node now bound 0.0.0.0 + advertised tailnet-first http://100.127.107.31:4716 for retry. Added OA_DISCOVERY_PUBLIC_URL so node can advertise an HTTPS `tailscale serve` MagicDNS endpoint (ATS-safe, cellular-reachable). **NEEDS-OWNER:** (1) ensure phone Tailscale connected + reopen app; (2) if still failing = ATS → enable Tailscale HTTPS at admin/settings/features so I front the node with HTTPS over tailnet (node already wired via OA_DISCOVERY_PUBLIC_URL=https://macbook-pro-m5.tailaeab8f.ts.net), OR cut a fresh local build with ATS guaranteed.
