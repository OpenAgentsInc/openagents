# Autopilot Clients Roadmap — Web, Desktop, Mobile

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


> **⚠️ BUILD/SHIP POLICY — UPDATED 2026-06-13.**
> **We have switched OFF Expo/EAS.** Native iOS `.ipa` is compiled **locally on
> our own Mac** (`expo prebuild` → `xcodebuild`/`fastlane`) and uploaded to
> TestFlight **Apple-native via `xcrun altool`** (ASC API key in
> `.secrets/appstoreconnect.env`). JS-only changes ship **OTA over our own
> server** `updates.openagents.com` (`apps/oa-updates/scripts/publish-ota.sh`),
> never `eas update`/u.expo.dev. **Do not use `eas build`, `eas submit`, `eas
> update`, or any Expo cloud service.** The `expo` CLI itself (`expo install`,
> `expo export`, `expo prebuild`) stays. Canonical runbook:
> `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md`.

Date: 2026-06-13
Status: implementation roadmap. Sequences the buildout of the three Autopilot
Coder client surfaces and the shared spine that unifies them, from first
local handshake through TUI parity and cloud integration. Planning doc; each
milestone's rungs are tracked as issues. No runtime invariant changes here.

## The three clients + one spine

- **Web** — Foldkit/Effect/Tailwind (`apps/openagents.com/apps/web`), exists.
- **Desktop** — Autopilot Desktop, Bun/Electrobun + Foldkit
  (`apps/autopilot-desktop`); audit
  `2026-06-13-autopilot-desktop-app-audit.md`.
- **Mobile** — Autopilot Remote Control, React Native/Expo (`clients/khala-ios`);
  audit `2026-06-13-autopilot-remote-control-mobile-app-audit.md`.

**Spine:** `packages/autopilot-control-protocol` (Effect Schema + typed client +
cursor/dedup/decision logic). All three clients speak the same control + bridge
vocabulary (`openagents.pylon.control.v0.3` today; the system-#39 bridge verbs
next). Desktop reaches the local node over loopback; mobile reaches it over the
bridge (and, dev-only, over the host network from a simulator); web over the
existing API. One protocol, implemented and tested once.

Authority and projection rules come from the remote session bridge audit
(system #39) and the mobile/web companion audit; this roadmap is the *sequencing*
of building against them.

## Milestones at a glance

| Milestone | Theme | Headline outcome |
| --------- | ----- | ---------------- |
| **M0** | Shared spine | `packages/autopilot-control-protocol` + shared fixtures, tested once |
| **M1** | Scaffolds + first handshake | Desktop *and* mobile-in-emulator both show the **same live Autopilot Coder session** running on this Mac |
| **M2** | The bridge (#39) | Proper pairing/scoped-credential/cursor-resumable transport for mobile + remote desktop |
| **M3** | Actions + TUI parity | Each client does what the TUI does today: spawn, approve, steer, cancel, accounts/quota, artifacts |
| **M4** | Cloud integration | Clients deploy/observe OpenAgents Cloud sessions (BYO-key / credits, quota/failover) |
| **M5** | Cross-client polish | Perfect interop: dedup across clients, notifications, theming parity, distribution, conformance suite |
| **M6** | **The Self-Driving Loop** ◀ **ASAP** | A message composed on the **phone** is heard by **Pylon**, a **coordinator agent fans it out** to coding agents, and at the right moment it **auto-ships back to the phone** — an **OTA update** through our **OpenAgents Updates** server for JS/mobile changes, or a **local binary build** on our own Mac (`expo prebuild` → `xcodebuild`/`fastlane`) uploaded **straight to Apple** for native changes. No Expo cloud, no GitHub |

> **Issue tracker:** the full roadmap is now filed. M0–M2 = **#4902–#4916** (CL-0…CL-14); M3 = **#4921–#4930** (CL-15…CL-24); M4 = **#4931–#4934** (CL-25…CL-28); M5 = **#4935–#4939** (CL-29…CL-33); **M6 (self-driving loop) = #4940–#4947 (CL-34…CL-41)**; theming fast-track CL-42 = **#4948**. Plus fast-track #4919 (TestFlight) and #4920 (self-hosted Expo Updates).

> **Fast-track (out of milestone order, owner request):** ship the mobile
> **shell to TestFlight ASAP** so it's on a personal device now — local build +
> Apple-native upload (#4919). Distribution normally lives in M5 (CL-32), but TestFlight for
> the shell is pulled forward and does not block the M1–M4 sequence. iOS bundle
> `com.openagents.autopilot-mobile`, Apple team OpenAgents, Inc. `HQWSG26L43`;
> runbook `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md`. The desktop
> equivalent (sign/notarize + BSDIFF feed) remains M5.
>
> **Fast-track follow-on:** once on TestFlight, adopt self-hosted
> **Expo Updates** so JS-only fixes ship OTA without a rebuild — #4920.
> Formal home is M5 (CL-32); pulled forward with TestFlight, also non-blocking.

> **★ ASAP critical path (owner priority) — M6, the Self-Driving Loop.** The
> headline goal, prioritized ahead of finishing M2–M5: **I compose a message in
> the mobile app → Pylon hears it → a coordinator agent (the productized version
> of the loop a human coordinator runs today) fans it out to coding agents
> across the account pool → and at the appropriate time it ships the result back
> to my phone automatically** — an **OpenAgents Updates OTA** when the change is
> JS/mobile-only, or a **local binary build** (on our own Mac) submitted
> **straight to Apple** when there are native/config changes. **No Expo cloud,
> no GitHub:** OTA publishes through our self-hosted
> **OpenAgents Updates** server (cloud audit
> `cloud/docs/2026-06-13-openagents-updates-and-deployment-infra-audit.md`);
> binaries build locally (`expo prebuild` → `xcodebuild`/`fastlane`) and upload directly to App Store Connect. Apple
> is the only external party. The MVP runs on the **M1 dev transport + local
> tooling + #4920 fingerprinting** — it does **not** wait for the full M2 bridge.
> Rungs **CL-34…CL-41 (#4940–#4947)**; see **M6** below. Theming the three
> clients off the website's dark mode is pulled forward as **CL-42 (#4948)**.

> **Dark-mode theming (owner request):** the website's dark theme is the single
> source of truth for look-and-feel across **all three** clients. Desktop and web
> share the canonical dark **tokens** (`cssVars`/`darkTokens` from
> `@openagentsinc/autopilot-ui`); mobile (RN) maps the same tokens. UPDATE
> (2026-06-13): desktop now **also renders through the shared Foldkit
> components** — the webview was converted from hand-DOM to a Foldkit TEA app
> (CL-53 #4966), so web + desktop share one component library and look-and-feel;
> mobile (RN) still maps the same tokens. The desktop webview is Foldkit-only
> going forward (`apps/autopilot-desktop/AGENTS.md`). Tokens tracked as the
> fast-tracked **CL-42 (#4948)**.

---

## M0 — Shared spine (foundation, do first)

Everything else imports this. Build and test it once.

- **CL-0** (#4902) Extract `packages/autopilot-control-protocol`: Effect Schema for the
  control surface (`openagents.pylon.control.v0.3`: `session.spawn/list/events/
  cancel`, node snapshot, event frames) plus the planned bridge verbs/events
  (`bridge.pair/revoke/clients.list`, `session.subscribe/snapshot/history`,
  `turn.steer/interrupt`, `decision.resolve`, `artifact.read`, and the event
  names). Include the typed client, the cursor/sequence/dedup logic, and the
  decision exactly-once state. Repo: openagents. **Foundational.**
- **CL-1** (#4903) Shared node-fixture set (health, `/command` results,
  `/sessions/:ref/events` frames, decision request/resolve/cancel) used by
  vitest across web/desktop/mobile. Repo: openagents. *Parallel after CL-0 shape.*
- **CL-2** (#4904) `packages/autopilot-ui`: shared Foldkit components (session rows,
  decision cards, event timeline, status/staleness/lag chips) for web + desktop.
  Repo: openagents. *Parallel after CL-0.*

## M1 — Scaffolds + first handshake (the "get the picture" demo)

Goal: a concrete, local, on-this-Mac demo where **both new clients render the
same live Autopilot Coder session** from the shared protocol — no bridge yet,
dev-only transport.

How the local handshake works (no bridge required for the demo):

- Run a local Pylon node (`pylon node`) — control server on `127.0.0.1:4716`
  with its `control-token`.
- **Desktop** (same machine) connects over **loopback** directly.
- **Mobile in a simulator/emulator** reaches the *host* machine: iOS Simulator
  shares the host network (`localhost`/`127.0.0.1` → host), Android emulator
  uses `10.0.2.2`. So the emulator can hit the host's control server with the
  dev bearer token — enough to prove the protocol before the bridge exists.
- Start a real bounded Autopilot Coder session (e.g. the multi-session runner or
  `session.spawn`) and watch it appear, with a **live event timeline**, in both
  clients at once.

Rungs:

- **CL-3** (#4905) Desktop scaffold: `apps/autopilot-desktop` Electrobun app
  (`electrobun.config.ts`, Bun main wiring the Pylon control client over
  loopback, Foldkit webview via Vite → `views://`), added to workspace globs.
  Repo: openagents. *Parallel.*
- **CL-4** (#4906) Mobile scaffold: `clients/khala-ios` vanilla Expo app (SDK 55 / RN 0.81 /
  React 19 / React Navigation v7; MMKV, edge-to-edge, EAS, Maestro patterns
  borrowed from Ignite) consuming `packages/autopilot-control-protocol`. Repo:
  openagents. *Parallel.*
- **CL-5** (#4907) Desktop P0: connect to local node, render session list + live
  session-detail timeline from the shared protocol. Repo: openagents. *After
  CL-0, CL-3.*
- **CL-6** (#4908) Mobile P0 (dev transport): connect from the simulator to the host
  control server (localhost / `10.0.2.2`, dev token in `expo-secure-store`),
  render session list + live timeline. Explicitly dev-only; real transport is
  M2. Repo: openagents. *After CL-0, CL-4.*
- **CL-7** (#4909) **Handshake demo + doc:** a scripted local demo — start a bounded
  Autopilot Coder session; show it live in desktop and the mobile emulator
  simultaneously (same session ref, same event timeline; bonus: a decision
  request appears on both). Capture a short runbook + screenshots. Repo:
  openagents. *After CL-5, CL-6.* **This is the M1 milestone gate.**

## M2 — The remote session bridge (system #39)

Replace the dev-only loopback/host-network access with the real, secure
transport so mobile (and remote desktop) work off the local machine.

- **CL-8** (#4910) Pylon control/bridge binding reachable over **both transports**
  (same handshake, only the address differs): **(a) same-LAN** — bind the LAN
  interface so a device on the same Wi-Fi reaches `http://<lan-ip>:4716`; **(b)
  Tailnet** — reachable at `http://<tailnet-ip>:4716` across networks
  (**prioritize Tailnet**, both required ASAP). Loopback still default; explicit
  refusal of **unauthenticated** non-loopback traffic (pairing credential
  required off-loopback). The reachable address(es) are what the QR/bootstrap
  payload carries (CL-13); the client resolves them tailnet-first, LAN fallback.
  Repo: openagents/apps/pylon. *Parallel.*
- **CL-9** (#4911) Bridge pairing + scoped credentials: short-lived one-time QR/bootstrap
  secret exchanged for a capability-scoped credential (issuer/audience/expiry/
  client id/device class/pairing id/`jti`/projection level); node stores hash +
  revocation, client stores the credential. + `bridge.clients.list`/`revoke`.
  Repo: openagents/apps/pylon. *After CL-0.*
- **CL-10** (#4912) Cursor-resumable streams: event ids + sequences, lossless/best-effort
  tiers, duplicate-safe replay, lag caveats, bounded backpressure; atomic
  subscribe that replays pending decisions first. Repo: openagents/apps/pylon.
  *After CL-0.*
- **CL-11** (#4913) Decision relay: server-originated decision requests, single-use
  exactly-once binding, cancellation/resolution broadcast (reuses the
  decision-queue spine #4765). Repo: openagents/apps/pylon. *After CL-10.*
- **CL-12** (#4914) Action receipts + typed failures for every remote verb (duplicate,
  expired, cancelled, revoked, stale, unauthorized, unsupported, overloaded).
  Repo: openagents/apps/pylon. *After CL-11.*
- **CL-13** (#4915) Pairing UX (render a **QR + copyable bootstrap**). The QR
  payload encodes the **reachable address(es)** (tailnet first, then LAN) +
  bootstrapId + secret + projectionLevel/capabilities, so a phone scan works on
  same-Wi-Fi (no Tailnet) OR over Tailnet with the identical handshake. Node side
  in the Pylon TUI/CLI; the mobile client gets a QR-scan + paste pairing screen.
  Repo: openagents/apps/pylon (+ clients/khala-ios). *After CL-9.*
- **CL-14** (#4916) Wire desktop + mobile clients onto the bridge transport (replacing
  the M1 dev transport), behind the same client interface. Repo: openagents.
  *After CL-9..CL-12, CL-5, CL-6.*

## M3 — Actions + parity with the TUI

Bring each client to functional parity with what the Pylon TUI / control surface
does today. Parity dimensions (map of TUI capability → client work):

| TUI capability today | Client target |
| -------------------- | ------------- |
| session spawn/list/events/cancel | CL-15 (#4921) spawn/cancel + list/detail (all clients) |
| approvals / decision queue | CL-16 (#4922) approve/deny/answer, exactly-once, read-only enforced |
| steer / interrupt / pause / resume | CL-17 (#4923) bounded steering + interrupt + pause/resume as separate verbs |
| accounts list/usage, quota | CL-18 (#4924) account/quota panel (incl. quota-block/failover state from #4884) |
| dev-loop check / verify status | CL-19 (#4925) verify/dev-check status + required-artifact view |
| node status / go-online/offline / heartbeat | CL-20 (#4926) node + provider online/offline status surface |
| artifacts / closeout / receipts | CL-21 (#4927) artifact + receipt review (refs/projection-gated) |
| assignments poll/accept/progress | CL-22 (#4928) work/assignment view (read-first) |
| wallet/balance/earnings (read) | CL-23 (#4929) earnings/balance read-only panel (no spend authority) |

- Each rung implements its capability across the **shared protocol** first, then
  web/desktop/mobile UI. Read surfaces (CL-18..CL-23) are largely parallel; the
  effectful ones (CL-15..CL-17) depend on M2 (CL-11/CL-12) and honor read-only
  capability gating. Repo: openagents.
- **CL-24** (#4930) Parity conformance checklist: a doc + test asserting each
  client can do (or explicitly, honestly cannot yet do) each TUI capability — the
  gate for claiming "parity with the TUI."

## M4 — Cloud integration

Per the cloud commercial plan
(`2026-06-13-cloud-remote-execution-commercial-plan.md`): clients become the
operator console for OpenAgents Cloud.

- **CL-25** (#4931) Cloud coordinator client in the shared package (deploy/list/
  observe cloud sessions). Repo: openagents (+ cloud contracts). *After cloud C-5/C-14.*
- **CL-26** (#4932) "Deploy to Cloud" from desktop + mobile (BYO-key vs OpenAgents
  credits selection), reusing the M3 spawn surface. Repo: openagents.
- **CL-27** (#4933) Cloud quota/cost/failover view: surface compute metering, credit
  balance, and account-quota failover (#4884) across clients. Repo: openagents.
- **CL-28** (#4934) Unified multi-origin session list: local + remote(bridge) + cloud
  in one view, with origin badges and per-origin authority. Repo: openagents.

## M5 — Cross-client polish ("all three working perfectly together")

- **CL-29** (#4935) Cross-client decision consistency: if two clients answer the
  same prompt, exactly-once wins and the others show "resolved elsewhere";
  cancellation propagates. Repo: openagents (+ pylon).
- **CL-30** (#4936) Notifications: `expo-notifications` (mobile) + desktop OS
  notifications + web, all projections of the #4765 spine, refs-only payloads,
  quiet hours; notification-open routes to the right node/session/decision.
- **CL-31** (#4937) Theming/UX parity (full): shared design tokens so web/desktop
  (Foldkit) and mobile (RN) feel consistent; status/staleness/lag chips identical
  in meaning. Builds on the fast-tracked dark-mode tokens **CL-42 (#4948)**.
- **CL-32** (#4938) Distribution: desktop code-sign/notarize + BSDIFF auto-update
  feed; mobile local build/TestFlight + owner-approved App Store submission
  ($4.99); mobile OTA JS updates via self-hosted `expo-updates` (#4920,
  fast-tracked after #4919). Repo: openagents.
- **CL-33** (#4939) **Cross-client conformance suite**: one fixture-driven test
  matrix (the CL-1 fixtures) every client runs, proving identical protocol
  behavior — cursor resume, dedup, exactly-once decisions, read-only gating,
  projection levels. The objective definition of "working perfectly together."

## M6 — The Self-Driving Loop (★ ASAP critical path)

The headline outcome, prioritized ahead of finishing M2–M5: **the phone commands
the factory, and the factory ships itself back to the phone.** Concretely — I
compose a message in Autopilot Remote Control; Pylon hears it; a coordinator
agent (the productized version of the loop a human coordinator runs today) fans
it out to coding agents across the account pool; and at the appropriate time the
result is shipped back to my device automatically.

**MVP runs now, no full bridge required:** it builds on the **M1 dev transport**,
local Expo tooling, the **#4918 campaign scheduler**, and the
**#4920 Expo Updates fingerprinting** — then hardens onto the **M2 bridge**
(CL-9..CL-12) for off-LAN/secure use.

- **CL-34** (#4940) **Mobile intent capture** — a compose surface in the mobile
  app to submit a natural-language work request ("build X / fix Y") to Pylon,
  with live status. Dev transport now, bridge later. Repo: openagents.
- **CL-35** (#4941) **Pylon intent intake** — an `intent.submit` control verb +
  durable intent queue + status projection (received → planning → fanning_out →
  shipping → shipped/failed), refs-only, exactly-once, cursor-resumable. The
  thing the phone's "ask" is heard by. Repo: openagents/apps/pylon.
- **CL-36** (#4942) **Coordinator agent runtime** — consume an intent, plan
  rungs, and drive the campaign scheduler (#4918) / `multi-session-run` to fan
  out coding agents across the account pool — exactly today's manual loop, made
  autonomous, with escalation hooks (CL-41). Repo: openagents/apps/pylon (+ forge).
- **CL-37** (#4943) **Ship-mode classifier (Expo fingerprint)** — after the
  fan-out merges, compute the Expo Updates runtime fingerprint of `clients/khala-ios`
  vs the deployed build: unchanged ⇒ **OTA-eligible** (JS only), changed ⇒
  **rebuild required** (native/config). Pure, testable; decides CL-38 vs CL-39.
  Depends on #4920. Repo: openagents.
- **CL-38** (#4944) **Auto OTA publish** — when OTA-eligible, the coordinator
  publishes a JS-only update to the build's channel so it reaches the phone with
  no rebuild by shelling out to **`apps/oa-updates/scripts/publish-ota.sh`**.
  The self-hosted "OpenAgents Updates" service on Cloud serves the signed
  manifest and assets; `app.config.ts updates.url` points at
  `updates.openagents.com`. Emits an update-id receipt. Repo: openagents (+ cloud).
- **CL-39** (#4945) **Auto local build + submit (no Expo cloud)** — when a
  rebuild is required, the coordinator runs a **local build on our own Mac** and
  uploads the binary **straight to Apple** through
  `clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh`
  (`expo prebuild` → `xcodebuild`/`fastlane gym` + `xcrun altool`/ASC API).
  Run a local `expo export` bundle pre-check before building. Apple is the only
  external dependency. Repo: openagents.
- **CL-40** (#4946) **Ship-status round-trip** — the originating mobile client
  sees live coordinator progress + the final ship outcome ("OTA published / build
  N on its way to TestFlight"), as a projection of the intent status. Closes the
  loop visibly on the phone. Repo: openagents.
- **CL-41** (#4947) **Loop safety** — authority/spend gating for autonomous,
  cost-bearing EAS builds and for OTA pushes to a live device; escalation for
  native builds; per-intent receipts (what changed, ship mode, artifact/update/
  build id), honoring the default-yes-with-escalation autonomy model. Repo:
  openagents (+ pylon).

## Theming fast-track

- **CL-42** (#4948) **Shared website dark-mode tokens across desktop + mobile** —
  the openagents.com web app's dark theme is the single source of truth; extract
  its tokens (the `--bg`/`--text`/`--outline`/`--primary` palette the
  `autopilot-ui` components already reference) into one shared source, ensure
  desktop (Foldkit) sets those CSS vars, and map them into the React Native theme
  so all three clients match. Pulls the full CL-31 (#4937) parity work forward.
  Repo: openagents.

## Dependency spine (critical path)

```
CL-0 ─┬─ CL-1/CL-2 (parallel)
      ├─ CL-3 (desktop scaffold) ─ CL-5 ─┐
      ├─ CL-4 (mobile scaffold)   ─ CL-6 ─┴─ CL-7  ◀ M1 demo gate
      ├─ CL-9 ─ CL-10 ─ CL-11 ─ CL-12 ─┐
      └─ CL-8 ─────────────────────────┴─ CL-14    ◀ M2 (real transport)
                                          │
                 M3 actions/parity (CL-15..CL-24, mostly parallel after M2)
                                          │
                 M4 cloud (CL-25..CL-28, after cloud C-5/C-14)
                                          │
                 M5 polish (CL-29..CL-33)

★ M6 — Self-Driving Loop (ASAP; runs on the M1 dev transport + local Expo/OpenAgents Updates tooling, not gated on M2):
   CL-34 (phone intent) ─ CL-35 (Pylon intake) ─ CL-36 (coordinator fan-out, uses #4918)
        └─ on merge ─ CL-37 (fingerprint classify) ─┬─ OTA-eligible ─ CL-38 (publish-ota.sh) ─┐
                                                     └─ rebuild ───── CL-39 (local build/altool) ┘
        └─ CL-40 (status back to phone)   CL-41 (spend/authority gating) wraps CL-36/38/39
   CL-42 (shared dark-mode tokens) — fast-track, parallel
```

Parallelizable now (fan-out friendly): CL-0 first, then CL-1/CL-2/CL-3/CL-4 and
the M2 pylon rungs CL-8/CL-9/CL-10 can run concurrently; M3 read surfaces
(CL-18..CL-23) are broadly parallel once M2 lands. **M6's MVP can start
immediately** — CL-34/CL-35/CL-36 on the dev transport, with CL-37/CL-38/CL-39
on the Expo EAS MCP — and CL-42 is independent.

## How this connects to the existing plans

- The desktop and mobile audits define *what* each client is; this roadmap
  defines *the order* and the shared spine that makes them one system.
- M2 here is the same "remote session bridge (system #39)" the audits depend on.
- M4 consumes the cloud commercial plan's coordinator + the quota-aware routing
  (#4884) already shipped.
- M0's `packages/autopilot-control-protocol` is the single most important rung — it
  is what makes "one implementation, three clients" real and is why the mobile
  app is React Native rather than Swift.

## Open decisions

1. **First-demo session content for CL-7** — simplest compelling demo is a
   bounded `session.spawn` whose live timeline + a single approval prompt render
   on both clients. (Recommended.)
2. **Workspace integration of `clients/khala-ios`** — RESOLVED: `clients/khala-ios` is a
   bun workspace member with a `workspace:*` dep on
   `packages/autopilot-control-protocol`; Metro is monorepo-aware (CL-6 follow-on:
   `watchFolders`/`nodeModulesPaths` to the repo root). Standalone publish not used.
3. **When to claim "TUI parity"** — gate on CL-24's conformance checklist, not
   vibes.
4. **Desktop pricing/channel** — still open (mobile is $4.99 App Store; desktop
   TBD per its audit).
