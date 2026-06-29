# Autopilot Desktop — Availability + First-Run + Auto-Onboarding Audit

Date: 2026-06-18
Scope: `apps/autopilot-desktop` (Electrobun), `apps/pylon` (the launched node),
`apps/oa-updates`, `apps/openagents.com` (homepage / register endpoint), the
`docs/launch/` and `docs/DEPLOYMENT.md` release set.
Type: audit + doc only. **No app code changed.**
Base: worktree off `origin/main` at `d290e73a0` (refactor(api): register worker
exact routes (#5440)).

The vision being audited (owner's words): the friendliest possible **one-click
desktop app** — a non-technical user installs it, opens it, and it does
EVERYTHING automatically (generate identity → self-register the agent → bring up
the node → connect a receive-ready wallet + register a payout target → register
presence → join the Tassadar run + claim work → start earning), "literally on
Autopilot," no CLI.

## TL;DR verdict

- **Can a human download + run it today?** *Yes, with friction, macOS Apple
  Silicon only, and it is not surfaced on the homepage.* A signed + notarized
  `.dmg` exists (GitHub prerelease `autopilot-desktop-v1.0.0-rc.3` + the update
  bucket), but the homepage download CTA was **removed** on 2026-06-18 to focus
  on Pylon, so the only way a human finds the app is the GitHub release or
  `INSTALL.md` — not a discoverable product surface. Intel macOS, Windows, and
  Linux are **not published**.
- **Black screen:** *root-caused and fixed* (commit `73cada159`, 2026-06-14).
  The current `view.ts`/`main.ts` reflect the fix. First launch now renders the
  immersive `network` (Tassadar proof-replay) scene, not a black window.
- **Auto-onboarding (the core ask):** *Mostly MISSING at the desktop layer, and
  partly AUTO inside the Pylon node but un-triggered by the desktop.* The
  desktop launches `pylon node` headless and supervises it — but it passes **no
  onboarding configuration**. The Pylon node auto-generates identity and
  auto-provisions a Spark wallet, but presence/payout/Tassadar work-claim are
  all gated on env (`PYLON_OPENAGENTS_BASE_URL`, `PYLON_ASSIGNMENT_WORKER=1`,
  `OPENAGENTS_AGENT_TOKEN`) that the desktop **never sets**, and **nothing
  anywhere calls `POST /api/agents/register`** to mint the agent token. So the
  end-to-end "install → earning" chain is broken at registration, presence, and
  Tassadar join.
- **Top 3 gaps to one-click-does-everything:** (1) no agent self-registration
  (no `/api/agents/register` call → no `OPENAGENTS_AGENT_TOKEN`); (2) desktop
  does not configure the node for presence + payout + Tassadar assignment work
  (the three env switches are unset); (3) no onboarding UX/orchestration in the
  Electrobun app at all (the old Rust onboarding flow was deleted with the
  Rust app; the Electrobun rewrite never reimplemented it). Plus owner-gated:
  Windows Authenticode cert, Intel/Linux publication.

---

## 1. Availability — can a user get it today?

### Where the code is + the stack

The app lives at **`apps/autopilot-desktop`**. The stack is **Electrobun**
(Bun main process + native webview), **not** WGPUI/Rust and **not** Tauri:

- `apps/autopilot-desktop/electrobun.config.ts` (Electrobun build config,
  `version: "1.0.0-rc.3"`).
- `apps/autopilot-desktop/src/bun/` — the Bun main process (plain TS): owns the
  control token, node discovery/launch, polling, the typed RPC bridge.
- `apps/autopilot-desktop/src/ui/` — the webview, a **Foldkit** (Effect TEA)
  app (`view.ts`, `model.ts`, `update.ts`, `main.ts`); visuals use
  `@openagentsinc/three-effect` per `apps/autopilot-desktop/AGENTS.md`.

There was a previous **Rust/WGPUI** Autopilot desktop app. It was renamed/retired
(`081a843f7 Rename legacy Autopilot desktop app path`) and rebuilt as the
Electrobun app under CL-3..CL-59 (`a548b5c13 Electrobun app scaffold (CL-3)` …
`70deb8107 CL-53: convert Autopilot Desktop webview to Foldkit (#4966)`). This
rename is load-bearing for §3 below: the old Rust onboarding flow does not exist
in the current app.

### Built, signed, notarized macOS build — yes

Per `docs/launch/2026-06-18-autopilot-desktop-cs-b1-proof.md` (issue #5364) and
the release `autopilot-desktop-v1.0.0-rc.3`:

- Source commit `e125ad853…`, channel `stable`, config `1.0.0-rc.3`.
- Apple Developer ID `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
- `.app`: `codesign --verify --deep --strict` passed; `spctl -a -vvv -t exec`
  → `accepted`; Gatekeeper source `Notarized Developer ID`. Notarization
  submission `2de6437f-…`.
- `.dmg`: recreated from the stapled app, signed, notarized (submission
  `75f3362e-…`), `xcrun stapler validate` passed. Digest
  `cc1d7876b70fded1ad5b15743bdcf2ee4e72fd8c5ca6dedd3d1d73f037db41db`.

Signing/notarization mechanics: `apps/autopilot-desktop/scripts/notarize-macos.sh`,
`apps/autopilot-desktop/README.md` (macOS Signing And Notarization),
`apps/oa-updates/docs/release-signing-runbook.md` (Apple Developer ID section,
`HQWSG26L43`, valid 2026-06-15 → 2031), `docs/DEPLOYMENT.md` (Autopilot Desktop
row + worked example).

### Downloadable DMG + OTA feed — yes

- The signed `.dmg` is attached to GitHub prerelease
  `autopilot-desktop-v1.0.0-rc.3`
  (asset `AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg`, verified via
  `gh release view`).
- Mirrored on the update bucket:
  `https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/AutopilotDesktop-1.0.0-rc.3-macos-arm64.dmg`.
- OTA feed: `apps/oa-updates` serves `updates.openagents.com`; desktop clients
  read `https://updates.openagents.com/desktop/stable/feed.json`
  (`apps/autopilot-desktop/README.md` "Desktop Update Feed"; publish via
  `bun run --cwd apps/oa-updates desktop:publish`). Auto-update is on by default
  (`apps/autopilot-desktop/src/bun/auto-update.ts`, surfaced in first-run
  health).

### Platforms

- **macOS Apple Silicon (arm64): shipped** (signed, notarized, downloadable).
- **macOS Intel: not published** (release body: "Intel macOS … not published
  yet").
- **Linux: not published** for users (the build path supports a Linux bundle —
  `apps/autopilot-desktop/README.md` "Release Builds (macOS + Linux)" — but no
  Linux installer is published, and Linux has no codesign/notarization step).
- **Windows: not present** (no Windows build path in the repo; gated on the
  Authenticode cert — owner-gated, *unverified that the cert exists*).

### Is the download surfaced anywhere a human can find it?

**No — not on the product surface.** The homepage DMG CTA was deliberately
**removed** on 2026-06-18:

- `b85391e2b feat(web): focus homepage install on Pylon, remove Autopilot DMG`
  — "The logged-out homepage … replaces the 'Download Autopilot' macOS DMG CTA
  with a Pylon-only install block … `npx @openagentsinc/pylon` … rather than a
  human-downloads-a-DMG flow." It removed the DMG download CTA + its constant.
- `grep -rni "\.dmg|download.*autopilot"` over `apps/openagents.com/apps/web/src`
  returns nothing — there is no desktop download link in the web app today.
- An earlier `1843494bd Expose direct Autopilot desktop download` did surface
  it; that was undone by `b85391e2b`.

So today a human only gets the app via: the GitHub release page, the update
bucket URL, or `https://openagents.com/INSTALL.md`. There is **no friendly
product-page download**. This directly contradicts the "non-technical user
installs it" framing — the current discoverable install path is Pylon-via-CLI,
not the desktop app.

### Issues cited

- **#5360** (CLOSED) — EPIC: make the Autopilot Desktop coding service
  operational. Scope is the *coding* surface (composer/sessions/approvals), not
  the contributor auto-onboarding chain in §3.
- **#5364** (CLOSED) — CS-B1: packaged headless node + signing/notarization
  gate; proof at `docs/launch/2026-06-18-autopilot-desktop-cs-b1-proof.md`.
- **#5027** (CLOSED) — Tassadar Launch Step F2: package the desktop, bundle +
  launch the node in the signed `.app` (+Linux), smokes.
- Commit `e125ad853…` is the CS-B1 source; the README change recording the
  CS-B1 packaged-node proof is in `apps/autopilot-desktop/README.md`
  ("Packaged-app node bring-up (#5027, Phase 2)" + "CS-B1 proof … recorded at
  …").

Caveat: #5360/#5364/#5027 being CLOSED means the *coding service + packaged
signed node + DMG publication* are done. They do **not** cover the contributor
auto-onboarding chain (identity→register→wallet→presence→Tassadar→earn), which
is the owner's core ask here and is **not** delivered (§3).

---

## 2. First-run — what happens when the user opens it?

### Black screen: root-caused and FIXED

- **Status: fixed.** Commit **`73cada159`** (2026-06-14)
  *"fix(desktop): view/crashView must return a Foldkit Document (fixes blank
  screen)."*
- **Root cause (from the commit):** Foldkit's runtime renders
  `view(model).body` — `view` must return a `Document` (`{ title, body }`). The
  desktop `view` returned a bare `Html` (`h.div`), so `.body` was `undefined`
  and nothing ever mounted → blank window. The crash boundary had the same bug,
  so even errors rendered blank.
- **Fix:** both `view` and `crashView` now return `{ title, body }`. Verified in
  the current tree: `apps/autopilot-desktop/src/ui/view.ts:4894`
  (`export const view = (model: Model): Document => ({ title: "Autopilot",
  body: sanitizeTree(rootView(model)) as Html })`) and `main.ts` `crashView`
  returns a Document. A `sanitizeTree` guard (strips `undefined`/`false`
  children) was kept as defense-in-depth.
- A related follow-up `b694661c1 Fix desktop webview module script`
  (`src/ui/index.html`, 2026-06-14) corrected the webview module-script load.

So the black/blank screen the owner remembers is fixed in source on `main`.
*Unverified:* whether a clean-machine launch of the **published rc.3 `.dmg`**
visually renders for an external user — the CS-B1 proof exercised the packaged
*node launcher* smoke (`{"statuses":["launching","online"]}`) and the composer
loop, not a screenshot of the rendered first-run window on a stranger's Mac.

### What the user actually sees on first launch

From `apps/autopilot-desktop/src/ui/initial-state.ts` + the pane router in
`view.ts`:

- The default pane is **`network`** — an immersive, full-screen `three-effect`
  Tassadar proof-replay scene with no sidebar chrome. Initial commands are
  `LoadInstallReadiness`, `LoadProofReplayBundle` (catalog), and
  `LoadPublicActivityTimeline` — all **public Worker / replay data**, fetched
  with no local node required.
- A sidebar exposes other panes: `builtin-agent` (Agent, with a **Go online**
  button), `nodes`, `training`, `training-fullscreen` (Training Live),
  `sessions`, `decisions`, `spawn`, `settings`, `session-detail`.
- **First-run Health** (#5064) shows a compact readiness line on home and a full
  breakdown under Settings (`apps/autopilot-desktop/src/shared/install-readiness.ts`):
  local Pylon lifecycle, built-in-agent readiness, platform/runtime, auto-update.
- In parallel, the Bun host runs `superviseManagedNode`
  (`src/bun/node-launcher.ts`): it adopts a running node or launches the bundled
  headless `pylon node`, into a managed home
  `~/.openagents/autopilot-desktop/.pylon-local`, and waits ≤30s for the control
  token + a reachable control server before reporting honest `online`/`failed`.

Net first-run experience: a polished public visualization renders immediately;
the local node comes up in the background; the **coding** panes are dark until
the node is online (see `docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`).
Crucially, **no onboarding wizard runs** — there is no UI step that says
"creating your identity / registering you / setting up your wallet / joining the
run." The app is an observe-and-steer cockpit, not an onboarding funnel.

---

## 3. Auto-onboarding — the core ask, mapped AUTO / MANUAL / MISSING

How the app is wired: the desktop's only contribution to onboarding is launching
`pylon node` headless and supervising it (`src/bun/node-launcher.ts`). It passes
the inherited environment plus a forced `PYLON_HOME`; it sets **none** of the
onboarding env switches. So each step's status is determined by (a) what the
Pylon node does automatically on `runHeadlessNode` boot
(`apps/pylon/src/index.ts:904`) and (b) whether the desktop supplies the
required configuration. Verified facts:

- **The desktop sets none of `PYLON_OPENAGENTS_BASE_URL`,
  `PYLON_ASSIGNMENT_WORKER`, or `OPENAGENTS_AGENT_TOKEN`.**
  `grep` over `apps/autopilot-desktop/src` finds these only as *read-only
  display* fields (`agentTokenPresent` in `promise-surfacing.ts`/`rpc.ts`), never
  as values the launcher injects into the node child env.
- **Nothing in `apps/pylon/src` or the desktop calls `POST /api/agents/register`.**
  `grep -rn "agents/register" apps/pylon/src` → no `fetch`/POST. The endpoint
  exists only server-side (`apps/openagents.com/workers/api/.../agent-registration-routes`,
  capability manifest, OpenAPI) and is documented as **self-service** in
  `apps/openagents.com/workers/api/src/openagents-agent-onboarding.ts`. The
  agent token is read from env everywhere (`OPENAGENTS_AGENT_TOKEN` /
  `--agent-token`) and Pylon **throws** when it is required and absent
  (e.g. `apps/pylon/src/wallet.ts:834`, `agent-surface.ts:24`, `tips.ts:19`).

| # | Step | Status | Evidence + what is/ isn't automated |
|---|------|--------|-------------------------------------|
| a | **Generate Nostr/agent identity** | **AUTO** (in the node) | `runHeadlessNode` calls `ensurePylonLocalState(...)` and logs `[Identity] Pylon Nostr npub: …` (`apps/pylon/src/index.ts:960-963, :1027`). A fresh managed home gets an identity with no user/CLI step. |
| b | **Self-register the agent (`POST /api/agents/register` → `OPENAGENTS_AGENT_TOKEN`)** | **MISSING** | No code calls the register endpoint (see above). The token must be supplied via env, and the desktop never supplies or mints it. This is the **central break** in the chain: without this token, steps e/f/g cannot authenticate, and the wallet payout-target registration (d) has no `agentToken`. NB: identity (a, a Nostr keypair) is *not* the same as an agent account/token (b); having a npub does not authorize the agent against the Worker. |
| c | **Start the bundled Pylon node** | **AUTO** | The Bun host launches + supervises the bundled headless node (`src/bun/node-launcher.ts`, `superviseManagedNode`, restart-on-crash). Packaged-node bring-up proven in the CS-B1 proof (`statuses: ["launching","online"]`). |
| d | **Set up / connect MDK/Spark wallet (receive-ready) + register payout target** | **PARTIAL** — wallet provisioning AUTO; payout-target registration **effectively MISSING** | Wallet: `startSparkBackupProvisioning(localState, …)` runs on boot, **default-ON** (#5304) — "a fresh node is payable out of the box with no manual command" (`apps/pylon/src/index.ts:1037-1040`), plus a warm Spark session (#5207) and `wallet.status` served warm (#5306). The desktop reads this read-only (`pylon-control.ts` `fetchWalletRow`, `wallet.status`). **Payout target:** `ensureSparkPayoutTargetRegistered(...)` (#5305, auto/idempotent/fail-soft) is wired into the heartbeat path — but it lives **inside the heartbeat block that only runs when `presenceBaseUrl` (`PYLON_OPENAGENTS_BASE_URL`) is set** and it needs `agentToken` (`apps/pylon/src/index.ts:1042-1075`). Since the desktop sets neither, payout-target registration does not fire. |
| e | **Register presence (bearer-token path, not NIP-98)** | **MISSING in the desktop path** | `forkNodeServices(... heartbeat: { baseUrl: presenceBaseUrl, register: () => registerPylon(...), heartbeat: () => sendHeartbeat(...) })` is wired (`apps/pylon/src/index.ts:1042-1075`), and `presenceClientOptionsFromEnv` uses `OPENAGENTS_AGENT_TOKEN` as the bearer (`apps/pylon/src/presence.ts:62-77`). But presence is gated on `presenceBaseUrl = PYLON_OPENAGENTS_BASE_URL`, which the desktop never sets → presence never registers. (`registerPylon` is presence registration under schema `openagents.pylon.register.v0.3`; it is **not** agent-account registration — that is step b, which has no caller at all.) |
| f | **Join the Tassadar run + claim work** | **MISSING in the desktop path** | The assignment worker (claim/work loop) starts only when `presenceBaseUrl && PYLON_ASSIGNMENT_WORKER === "1"` (`apps/pylon/src/index.ts:1076-1083`, `runHeadlessAssignmentWorkerLoop`). The desktop sets neither switch, so the node boots as a presence-less, non-claiming node. No desktop UI triggers a Tassadar join. |
| g | **Start earning** | **MISSING (consequence of b/e/f)** | Earning requires a registered agent (b), live presence (e), and claimed/settled work (f). None fire in the desktop-launched node, so an out-of-the-box install does not earn. |

### Honest summary of the chain

On a fresh desktop install, the node *will* generate an identity and provision a
receive-ready Spark wallet locally — but it boots **isolated**: it does not
register the agent, does not announce presence to `openagents.com`, does not
register a payout target, and does not join Tassadar or claim work. The three
switches that would light up presence/payout/Tassadar
(`PYLON_OPENAGENTS_BASE_URL`, an `OPENAGENTS_AGENT_TOKEN`, `PYLON_ASSIGNMENT_WORKER=1`)
are all unset by the desktop, and the one truly absent primitive — minting the
agent token via `POST /api/agents/register` — has **no caller anywhere**.

The closest thing to "one click → working" today is the **Built-in Agent
"Go online"** button (#5063, `apps/autopilot-desktop/src/shared/builtin-agent.ts`,
`src/bun/` builtin-agent host). But that is a *coding* path: it runs a bounded
codex session on hosted OpenAgents compute using package-level
`OA_CLOUD_CONTROL_URL`/`OA_CLOUD_CONTROL_TOKEN` secrets (README "Built-in Agent")
— it is **not** the contributor onboarding chain and does not register the
agent, join Tassadar, or earn. It is also explicitly yellow/unshipped until a
hosted-compute recut ships.

### Why the owner may remember onboarding existing

The old **Rust** Autopilot app *did* have an onboarding flow:
`cbf1df04d autopilot-desktop: add first-run onboarding flow` (2026-03-17) added
`src/onboarding.rs` (1456 lines) + `src/app_state.rs`/`input.rs`, and
`95ba8fd78 Fix first-run Spark wallet bootstrap` patched it. **Those `.rs` files
no longer exist** — the Rust app was retired and replaced by the Electrobun app
(CL-3..CL-59), and the Electrobun rewrite **never reimplemented an onboarding
flow** (`grep -rli "onboard" apps/autopilot-desktop/src` → nothing). So the
auto-onboarding the owner remembers was real, in a now-deleted codebase, and has
not been ported.

---

## 4. Gap analysis + path to one-click-does-everything

Prioritized, reusing what already exists. The runtime primitives mostly exist in
Pylon; the missing pieces are **registration**, **desktop configuration of the
node**, and **onboarding orchestration/UX**.

### P0 — Agent self-registration (closes step b; unblocks d/e/f/g)
The single highest-leverage gap. On first run, the Bun host must call
`POST /api/agents/register` (self-service, documented in
`openagents-agent-onboarding.ts`) using the node's generated identity, persist
the returned `OPENAGENTS_AGENT_TOKEN` to the managed home, and inject it into the
node child env. Reuse: the existing identity (`ensurePylonLocalState`), the
existing server endpoint, the existing env contract (`OPENAGENTS_AGENT_TOKEN`
read everywhere). Build: one Bun-side register-on-first-run step + secure token
persistence (keep the token in the Bun host, never the webview, per the secrets
boundary in `AGENTS.md`).

### P0 — Desktop configures the node for presence + payout + Tassadar (closes d/e/f)
Have the desktop launcher set, in the node child env: `PYLON_OPENAGENTS_BASE_URL`
(= `https://openagents.com`), the freshly minted `OPENAGENTS_AGENT_TOKEN`, and
`PYLON_ASSIGNMENT_WORKER=1`. With those three set, the **already-built** Pylon
code does presence registration (`registerPylon`/`sendHeartbeat`), auto payout
target (`ensureSparkPayoutTargetRegistered`, #5305), and the Tassadar assignment
work loop (`runHeadlessAssignmentWorkerLoop`) — no new runtime needed. Build:
inject env in `node-launcher.ts` (currently it only forces `PYLON_HOME`). Gate
behind explicit consent so it stays honest about going online + earning.

### P1 — Onboarding UX + status surface in the Electrobun app
There is currently no onboarding pane. Build a first-run wizard/status surface
(Foldkit + `autopilot-ui`) that shows each step (identity → registered → node
online → wallet receive-ready → presence → joined Tassadar → first work claimed)
with live status, reusing the **First-run Health** projection
(`install-readiness.ts`) extended with onboarding items (it currently tracks
node/built-in-agent/apple-fm/auto-update only — no identity/register/wallet/
presence/Tassadar items). This is the visible "literally on Autopilot" surface.

### P1 — First-run identity choice: detect existing vs create new + name it (owner requirement)
On first launch, **before** the node auto-generates a fresh identity (step a), the
app should **auto-detect whether the user already has a Pylon identity** — scan the
known node homes for a seed marker (`~/.openagents/pylon`, `~/.pylon`, the
historical-config identity path). The detection logic already exists: reuse Pylon
v1.0.3's `selectPylonHomeResolution` (`apps/pylon/src/bootstrap.ts`), which already
prefers the seed-bearing home and never reads it (marker-file presence only).

Then present a clear choice:
- **"Use your existing Pylon identity"** (show the detected npub / `pylon.<short>` so
  they recognize it) — boot the app against that home, so an existing contributor's
  wallet, payout target, and history carry over instead of being forked.
- **"Create a new Autopilot identity"** — and let the user **name it** (a display
  name for the agent). This mints a fresh `PYLON_HOME` + identity under that name and
  runs the clean from-scratch chain (self-register → wallet → presence → join). It
  must be available **even when an existing Pylon is detected**, so (1) a user can
  run more than one identity, and (2) the from-scratch onboarding is demoable/testable
  on a machine that already has a Pylon.

Hard rule (the v1.0.3/Orwell lesson): **never silently overwrite or adopt the wrong
home** — detect, ask, and only then act on the chosen home. This decision is the very
front of the onboarding wizard (the first screen, ahead of identity→register→join).
The product currently hides the desktop app (homepage is Pylon-CLI-only after
`b85391e2b`). For a "non-technical user installs it" story, re-expose a
discoverable, signed-DMG download on a product page — *after* the onboarding
chain works, so the download leads to a real one-click experience rather than an
isolated node.

### Owner-gated / platform
- **Windows:** no build path; needs the Authenticode code-signing cert
  (owner-gated). *Unverified whether the cert exists.*
- **Intel macOS + Linux:** build paths exist (Linux per README) but no published
  installers; publication is a release decision.
- **Built-in-agent hosted-compute recut:** the "Go online" coding path is yellow
  until a signed recut ships with `OA_CLOUD_CONTROL_*` configured (README).

### Verification that should exist but doesn't (mark unverified)
- A **clean-machine, from-DMG** first-run smoke that (1) screenshots the
  rendered window (confirms no black screen for an external user) and (2) drives
  the full identity→register→presence→payout→Tassadar→first-claim chain and
  shows a settled receipt. The CS-B1 proof only covered packaged node bring-up +
  the composer loop, not the contributor onboarding chain.

---

## Source references

- Desktop: `apps/autopilot-desktop/electrobun.config.ts`,
  `apps/autopilot-desktop/src/bun/node-launcher.ts` (launch/supervise; sets only
  `PYLON_HOME`), `src/bun/pylon-control.ts` (read-only `wallet.status`),
  `src/bun/promise-surfacing.ts` (`agentToken` read-only),
  `src/ui/initial-state.ts`, `src/ui/view.ts:4894` (Document fix),
  `src/ui/main.ts` (crashView Document), `src/ui/index.html`,
  `src/shared/install-readiness.ts`, `src/shared/builtin-agent.ts`,
  `apps/autopilot-desktop/README.md`, `apps/autopilot-desktop/AGENTS.md`,
  `apps/autopilot-desktop/scripts/notarize-macos.sh`.
- Node (Pylon): `apps/pylon/src/index.ts:904` (`runHeadlessNode`), `:960-963`
  (identity), `:1027` (npub log), `:1037-1040` (Spark provisioning #5304),
  `:1042-1075` (presence/heartbeat/payout, gated on `PYLON_OPENAGENTS_BASE_URL`),
  `:1076-1083` (assignment worker, gated on `PYLON_ASSIGNMENT_WORKER=1`),
  `apps/pylon/src/presence.ts` (bearer-token presence options),
  `apps/pylon/src/wallet.ts:832-834` (agent-token required).
- Registration endpoint (server only, no Pylon/desktop caller):
  `apps/openagents.com/workers/api/src/openagents-agent-onboarding.ts`,
  `…/agent-registration-routes.test.ts`, capability manifest + OpenAPI tests.
- Release / signing: `docs/DEPLOYMENT.md` (Autopilot Desktop row + worked
  example), `apps/oa-updates/docs/release-signing-runbook.md` (HQWSG26L43),
  `docs/launch/2026-06-18-autopilot-desktop-cs-b1-proof.md`,
  GitHub prerelease `autopilot-desktop-v1.0.0-rc.3`.
- Homepage download removal: commit `b85391e2b`
  ("focus homepage install on Pylon, remove Autopilot DMG"); prior
  `1843494bd` exposed it.
- Black-screen fix: commit `73cada159`; follow-up `b694661c1`.
- Old Rust onboarding (deleted): commit `cbf1df04d` (`src/onboarding.rs`),
  `95ba8fd78` (Spark bootstrap), retired by `081a843f7` + CL-3..CL-59.
- Coding-surface context: `docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`.
- Issues: #5360, #5364, #5027 (all CLOSED — coding service + packaged signed
  node + DMG, *not* the auto-onboarding chain).
