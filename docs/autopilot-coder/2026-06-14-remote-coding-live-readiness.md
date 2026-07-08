# Remote Coding — Live Readiness (2026-06-14)

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Blunt, evidence-backed status of "can the owner code on OpenAgents Cloud from a
remote client RIGHT NOW." Written autonomously while the owner is traveling
(default-yes; reviewed post-hoc).

## TL;DR — can he code from his phone right now? **NO.**

The pieces exist and are individually proven, but they are **not wired together
in a deployed, network-reachable configuration**. Three gaps block the
phone→cloud→GCE loop, none of which are code defects — they are deploy/config:

1. The **deployed** `oa-codex-control` daemon (the only remotely reachable one,
   on the SHC host) is a **2026-06-07 binary with NO GCE config** → its
   `cloud-gcp` lane would silently run **fake/local**, not a real GCE VM.
2. The locally-running **Pylon control server binds to `localhost` only** (not
   its Tailnet IP) and has **no `OA_CLOUD_CONTROL_URL/TOKEN`** set → the phone
   can't reach it, and even if it could, `cloud-gcp` spawns fall back to local.
3. The **Expo app has never been built/shipped** to the owner's phone (no
   `ios/` native project, no `.ipa`, no TestFlight feedback, build scripts
   dated 2026-06-13).

Fastest path to "code from my phone" is in the last section. Several legs need
the owner (creds/phone). Marked `NEEDS-OWNER`.

---

## Per-leg verdict

| Leg | Status | One-line |
| --- | --- | --- |
| oa-codex-control daemon reachable | 🟢 GREEN | SHC daemon up, `/healthz` ok, token authenticates |
| GCE live provisioning (deployed) | 🔴 RED | Deployed daemon has no `gcloud`/ADC/`OA_CODEX_GCE_*`; runs fake provisioner |
| GCE live provisioning (capability) | 🟡 AMBER | Proven locally on this Mac (CND-054), but my creds can't reach the `openagents-bench-dev` project to re-prove it |
| Pylon node running | 🟢 GREEN | PID running, control server live on `localhost:4716` |
| Pylon bridge reachable from phone | 🔴 RED | Bound to loopback only, not Tailnet `100.127.107.31` |
| Pylon → cloud dispatch configured | 🔴 RED | `OA_CLOUD_CONTROL_URL/TOKEN` unset → cloud lanes fall back to local |
| Expo app on the phone | 🔴 RED | Never built/shipped; no `ios/`, no `.ipa`, no TestFlight feedback |
| Network (Tailnet) | 🟢 GREEN | This Mac is on Tailnet (`100.127.107.31`) |

---

## Evidence

### 1. oa-codex-control daemon — GREEN (reachable, authenticating)

- `curl http://23.182.128.195:8787/healthz` → `{"service":"oa-codex-control","status":"ok"}`.
- Token at `.secrets/shc-codex-control-token` authenticates: an authed GET to a
  bogus path returns `not_found` (passed auth) vs `unauthorized` unauthed.
- SSH (`ubuntu@23.182.128.195`) works; `systemctl is-active oa-codex-control`
  → `active` since 2026-06-07 20:43 UTC.
- API surface (from `cloud/crates/oa-codex-control/src/main.rs`): POST
  `/v1/codex-runs[/start]`, `/v1/placement[/start]`, `/v1/workrooms/codex/start`,
  `/v1/codex-runs/{continue,steer,cancel}`, `/v1/codex-runs/{id}/{turns,cancel}`;
  GET `/v1/codex-runs/{id}`, `/events?cursor=N`, `/stream` (SSE). There is **no
  list-all-sessions** route on the daemon; sessions are addressed by run id.

### 2. GCE live provisioning — RED on the deployed daemon

The deployed SHC daemon **cannot** drive a real GCE VM:

- `ExecStart=/home/ubuntu/openagents-cloud/target/release/oa-codex-control`,
  `EnvironmentFile=/home/ubuntu/.openagents-secrets/oa-codex-control.env`.
- That env file contains only `OA_CODEX_CONTROL_BIND` and the token — **no
  `OA_CODEX_GCE_PROVISIONER`, no `OA_CODEX_GCE_PROJECT_ID`**.
- Host has **no `gcloud`** (`which gcloud` → not found) and **no ADC**
  (`~/.config/gcloud/application_default_credentials.json` absent).
- Per `Config::from_env` + `ProvisionerKind::from_env_value`, with no
  `OA_CODEX_GCE_PROVISIONER` the daemon defaults to the **fake** provisioner;
  the live path additionally requires ADC + raw project id, so even setting the
  flag would fall back to fake without those. Net: a `cloud-gcp` placement to
  this daemon runs **local/fake**, not a billed GCE VM.
- Binary is dated 2026-06-07 — **predates** the GCE-live provisioner work
  (CND-054, 2026-06-14). The deployed daemon does not even contain the live
  provisioner improvements.

The live GCE capability itself **is** real but was proven **on this Mac**, not
on SHC: CND-054 (`cloud/docs/bootstrap/CND-054-...md`) drove
`gcloud compute instances create/delete` for `e2-small` in `us-central1-a` with
guaranteed teardown and a final empty `instances list`. I could not re-run it
live this session: the canonical project `openagents-bench-dev` is **not
accessible** from my active credentials (`chris@openagents.com`,
`gcloud projects list` does not include it; direct list → "resource not found").

Teardown hygiene I **did** verify live: across the accessible Compute-enabled
projects (`openagentsgemini`, `openagents-lyra`) there are **zero** leftover
`oa-codex-sess*` instances and **zero** `oa-codex-sess*` firewall rules. No GCE
VM was created or left running by me this session.

### 3. Pylon node + bridge — RED for phone reach

- A Pylon node is running: `bun apps/pylon/src/index.ts node` (PID 91129); its
  control server listens on `localhost:4716` (returns `unauthorized` unauthed →
  alive). Config `~/.pylon/config.json` (`@openagentsinc/pylon 0.3.0-rc2`).
- **Bind is loopback-only.** `lsof` shows `TCP localhost:4716 (LISTEN)` (and two
  more Pylons on 4721/4722, also loopback). Per `apps/pylon/src/node/bind-config.ts`,
  non-loopback binds need a bearer token; the phone needs a Tailnet (`100.x`)
  bind to connect at all. This Mac's Tailnet IP is `100.127.107.31`.
- **No cloud dispatch configured.** `OA_CLOUD_CONTROL_URL`/`OA_CLOUD_CONTROL_TOKEN`
  are unset in the running process and nowhere in `.secrets/`. Per
  `apps/pylon/src/cloud-control-client.ts` (`resolveCloudControlConfig`) +
  `index.ts` (`cloudExecutorFactory`) + `node/control-sessions.ts`
  (`selectExecutor`), an unconfigured Pylon **silently runs cloud lanes
  locally** — no error. So `spawn{lane:cloud-gcp}` on the current node = local.
- Bridge pairing + verbs exist and are real: `/bridge/pair` (bootstrap exchange,
  pre-bearer) and `/bridge` (verbs `capability.list`, `decision.resolve`,
  `session.list`, `session.snapshot|history|subscribe`, `artifact.read`,
  `session.cancel`) in `apps/pylon/src/node/control-server.ts`. The bridge-native
  **`session.list` is fully implemented** (control-server.ts ~L451 →
  `control-sessions.ts list()` ~L925).

### 4. Expo app on the phone — RED (never shipped)

- `clients/khala-ios/AutopilotRemoteControl`: bundle id `com.openagents.autopilot-mobile`,
  version `0.1.0` (from `app.config.ts`).
- **No native project** (`ios/` absent → `expo prebuild` has not been run here)
  and **no `.ipa`** (`/tmp/oa-autopilot-local.ipa` absent). Build/ship scripts
  (`scripts/build-and-submit.sh`) are dated 2026-06-13.
- `testflight_feedback` for the bundle id → none (not proof of absence, but
  consistent with "not yet on the phone").
- Build path is **local, no EAS** (per repo policy + `scripts/build-and-submit.sh`):
  `expo prebuild` → `pod install` → `fastlane gym` → `xcrun altool` upload.
  ASC key is at `.secrets/appstoreconnect.env`.
- Client capabilities (per source): pair, list (currently via dev-token
  `/command`, not the bridge credential), stream, approve (offline-queued),
  cancel, spawn, deploy-to-cloud — all implemented and UI-wired. Two honest
  follow-ups: (a) the **session LIST is wired to the dev-token path, not the
  bridge `session.list`**; (b) **no QR camera** — pairing is text/URI input.

---

## Single fastest path to "code from my phone"

Ordered. Items needing the owner are flagged.

1. **Redeploy oa-codex-control on a GCE-capable host with live GCE.** Either
   (a) install `gcloud` + ADC on the SHC host and set
   `OA_CODEX_GCE_PROVISIONER=live` + `OA_CODEX_GCE_PROJECT_ID=<bench project>`
   in `/home/ubuntu/.openagents-secrets/oa-codex-control.env`, rebuild from
   current `main` (the deployed binary predates CND-054), and restart; or
   (b) run the daemon on this Mac (which already has `gcloud`+ADC) bound to
   Tailnet. `NEEDS-OWNER:` the GCP project + a service identity with Compute
   create rights — my `chris@openagents.com` creds cannot reach
   `openagents-bench-dev`.
2. **Point Pylon at the cloud + bind to Tailnet.** Start the node Pylon with
   `OA_CLOUD_CONTROL_URL=http://<daemon-host>:8787`,
   `OA_CLOUD_CONTROL_TOKEN=<shc-codex-control-token>`, and a Tailnet bind
   (`100.127.107.31`) + bearer. Then `spawn{lane:cloud-gcp}` actually leaves the
   Mac. (Config only; no code change.)
3. **Ship the Expo app to the phone (local build, one command):**
   ```
   cd /Users/christopherdavid/work/openagents/clients/khala-ios/AutopilotRemoteControl
   bun run ship:testflight
   ```
   (= `expo prebuild` → `pod install` → `fastlane gym` → `xcrun altool`.)
   `NEEDS-OWNER:` an iOS distribution cert + provisioning profile in the login
   keychain, a populated `.secrets/appstoreconnect.env` (currently the keys are
   empty), and the owner accepting the TestFlight build on the device.
4. **Pair the phone to the Tailnet-bound bridge** with a bootstrap code, then
   spawn `cloud-gcp`. Loop proven when the phone sees events + the artifact +
   the resource receipt for a GCE-backed run.

---

## NEEDS-OWNER

- GCP project + Compute-capable identity for a live GCE run (bench project not
  reachable from current creds).
- iOS signing assets + filled `.secrets/appstoreconnect.env` for the TestFlight
  upload; physical device to accept the build.
- Decision on daemon home (redeploy SHC with gcloud+ADC vs run the daemon on
  the GCE-capable Mac).

---

## Task 2 — shipped while offline (running log)

Bounded, verifiable items shipped to `main` (each: scoped change, tsc + tests
green, committed + pushed). Both are the honest Coder-Cloud follow-ups noted in
the brief. No tracked issues existed for these (repo policy keeps such items in
the Forum, not GitHub issues), so none to close.

1. **Mobile: poll session list over the bridge credential** (`60b35d5b4`).
   The session-list poll used the long-lived dev-token `/command` path even
   when a capability-scoped bridge credential was established. Added
   `fetchSessionRowsViaBridge` (projects the bridge-native `session.list`
   `SessionSummary` into the `ControlSessionRow` shape the screens consume) and
   made `ConnectionContext.poll` prefer it, falling back to the dev-token path
   when no bridge credential is ready yet or the bridge read fails — the same
   bridge-prefer pattern already used for cancel/events/decision dispatch.
   Files: `src/control/control-client.ts`,
   `src/connection/ConnectionContext.tsx`. `tsc --noEmit` clean; `bun test`
   green (56 pass).

2. **Mobile: dev-token-free bridge pairing from a bootstrap code** (`8628d12da`).
   Added `connectBridgeWithBootstrap`: pair onto `/bridge` using a single-use
   bootstrap decoded from a QR/pasted pairing code or URI (the
   `autopilot://pair` URI, rendered text block, or raw `bootstrapId:secret`),
   instead of minting the bootstrap over a dev token. Resolves the bootstrap's
   tailnet/LAN/loopback address (tailnet-first), exchanges at `/bridge/pair`,
   returns the paired session + resolved baseUrl with no dev token on the wire.
   Makes the previously-dead `createPairingFlow` machinery reachable and gives a
   QR-scanner UI a concrete API to call. Refactored `connectBridge` to share a
   `pairBridgeWithBootstrap` tail (no behavior change on the dev-token path).
   New unit tests mock global `fetch` (no node/device): success returns the
   tailnet base + pairing ref; undecodable code → null without any fetch;
   rejected exchange → null. Files: `src/control/control-client.ts`,
   `src/control/bridge-bootstrap.test.ts`. `tsc --noEmit` clean; `bun test`
   green (59 pass, +3).

   Remaining for full dev-token-free QR pairing (NEEDS-DEVICE, not shipped):
   a camera QR-scanner component (no camera dep installed) + a `ConnectionContext`
   path that connects from a stored pairing credential rather than a dev token.
   Deferred deliberately — that change needs on-device verification I cannot do
   while the owner is traveling, and risks breaking the working connect flow.

### Repo-health notes (no change shipped)

- `packages/autopilot-control-protocol`: `bun test` green (748 pass). Its
  `npm run typecheck` (`tsc -p tsconfig.json`) reports many **pre-existing**
  `node16` "needs `.js` extension" errors across test files — a repo-wide
  config state, not introduced here, and out of scope to flip (bun runs the
  `.ts` directly). Left untouched.
- `cloud/openagents-cloud-contract`: `cargo fmt/check/test` green (32 pass)
  after the Task 0 SHC-invoice change.
- GCE hygiene re-verified at finish: zero `oa-codex-sess*` instances in the
  accessible Compute-enabled projects (`openagentsgemini`, `openagents-lyra`).
  No GCE VM created this session.
