# Autopilot

Autopilot is the Electrobun desktop shell for the Autopilot Coder GUI.
It is planned as a Bun-native sibling to the Pylon TUI: the Bun main process
will own local Pylon control access over loopback, while the webview renders the
operator interface.

The UI is intended to reuse the same Effect and Foldkit stack as
`apps/openagents.com`, with typed bun<->webview RPC as the boundary between
credentialed local control and public-safe projections.

## Composer Pane (the day-to-day coding loop, #5355)

The **Composer** pane is the foreground "code in the app" surface — the
Claude-Code / Codex-CLI replacement loop — built on the EXISTING control
protocol (`session.spawn` / `session.events` / `session.cancel` + the approvals
queue), with no new wire contract:

1. **Start a coding session** — pick a runtime (`codex` / `claude_agent` /
   `apple_fm`), the provider account to run under (CS-A1, below), an execution
   lane, optionally a repo/worktree path, type an objective, and spawn the first
   turn via the desktop's `spawnSession` Bun function.
2. **Live streamed transcript** — the polled per-session event tail (the same
   `/events` content the node emits) renders as a readable turn/diff view; the
   agent's tool calls and file edits appear as transcript rows.
3. **Inline approvals** — the node's pending exactly-once decisions surface in
   the pane (Approve / Deny) wired to the existing `resolveApproval` flow.
4. **Reply / continue** — once a turn is terminal, a follow-up turn continues the
   thread. Because the protocol has no `session.reply` verb, a follow-up is a
   continuation `session.spawn` whose objective carries the prior turns as
   context (same repo/worktree). No new contract.
5. **Cancel** — `session.cancel` on the active turn.

When a local node is reachable, this pane is a usable coding loop; without a
node it shows a "start `pylon dev`" hint rather than hiding.

Prove the loop against a real loopback control server (a `pylon dev`-equivalent
node, no paid provider accounts required):

```sh
bun run --cwd apps/autopilot-desktop proof:composer
```

It drives spawn → live session-event tail (incl. a tool/diff line) → approval
projection → continuation (reply) spawn → cancel through the desktop's own Bun
control functions.

## Provider / account picker + multi-account management (CS-A1, #5361)

The Composer is account-aware on the EXISTING control protocol (no new wire
verb):

1. **Per-session account picker** — when a codex/claude account is registered on
   the node, the composer spawn form lists it; selecting it threads
   `accountRef` through `session.spawn` (the runtime already accepts it, #4868),
   so the coding turn runs under that account. "Default" leaves the node's own
   account selection.
2. **Apple FM as a spawn adapter** — `apple_fm` joins the runtime toggle
   alongside codex/claude. It uses its own control verb
   (`apple_fm.session.start`), so it is its own bounded spawn path (local-only,
   no per-account selection), not a separate Agent-pane card.
3. **Account management** — the Composer's **Accounts** card turns the old
   read-only list into add / select / set-priority / remove against the node's
   local `dev.accounts` config — the exact file the runtime reads
   (`loadPylonAccountRegistry`). The Bun host owns the home/config path; the
   webview only sees public-safe refs. Live readiness/quota still comes from the
   `accounts.list` projection (shared `AccountList` component). Priority orders
   dispatch (lower runs first).

Prove the picker + management end-to-end against a real loopback control server:

```sh
bun run --cwd apps/autopilot-desktop proof:account-picker
```

It drives accounts.list populating → a session spawning under a SELECTED account
(asserted by the session's `accountRefHash`) → add / set-priority / remove
mutations round-tripping through the node's `dev.accounts` config.

Scope: this sits inside the yellow, local-only promise
`autopilot.desktop_gui_client.v1`; a swarm/multi-session view and a richer
diff/transcript are later phases (see
`docs/launch/2026-06-18-autopilot-desktop-coding-surface-audit.md`).

## Training Pane Verification

Run the focused Training cockpit gate from the repo root with:

```sh
bun run verify:autopilot-desktop:training
```

or inside this app with:

```sh
bun run verify:training
```

That command runs the Foldkit reducer/view tests, CSS build, browser and Bun
bundle checks, and the Chrome-backed `oa-training-run` canvas-pixel smoke. The
smoke auto-detects common Chrome, Chromium, and Edge install paths; set
`CHROME_PATH=/path/to/chrome` if the browser binary is somewhere else.

The webview build consumes the centralized token/class component styles.
`build:css` generates Tailwind output, prepends the `@openagentsinc/design-tokens`
`:root { --oa-* }` block, appends shared workspace component CSS, writes
`src/ui/styles.out.css`, and emits `resources/ui/main.js`. Electrobun consumes
that generated browser entry so packaged browser code receives the same
token-backed classes used by the headless replica harness.

## Release Builds (macOS + Linux)

`bun run build` runs `electrobun build` with no channel, which produces a **dev**
`.app` under `build/dev-macos-arm64/` (no signing, no release artifacts). For a
distributable release build use a channel:

```sh
# Canary channel (unsigned by default unless signing env is present):
bun run --cwd apps/autopilot-desktop build:canary

# Stable channel:
bun run --cwd apps/autopilot-desktop build:stable
```

A channel build emits, into `artifacts/` (gitignored):

- the `.app` (under `build/<channel>-<os>-<arch>/`),
- a `.dmg` installer (macOS),
- a compressed `*.app.tar.zst` for the OTA feed,
- `<platform>-update.json` (version + content hash) for the feed.

Signing/notarization is **skipped** unless the relevant env is present (the build
log prints `skipping codesign` / `skipping notarization`). An unsigned `.app`
runs locally but Gatekeeper will quarantine it on a stranger's machine; ship the
signed + notarized build for the public install path (see below).

**Linux** uses the same `build:canary` / `build:stable` channels (electrobun
produces a Linux `.app`-equivalent bundle + tarball under
`build/<channel>-linux-<arch>/`); the launched node and the bundled-Bun path are
platform-agnostic. The bundled launcher sets `LD_PRELOAD` for the CEF libs (see
the generated `run.sh`). Linux has no codesign/notarization step.

## Packaged-app node bring-up (#5027, Phase 2)

In a packaged `.app` the dev launcher cannot walk to `apps/pylon/src/index.ts`,
so it looks for a **bundled** Pylon node entry the build step copied into the
app's Resources at `Resources/app/pylon-node/` (electrobun `build.copy` lands
sources under `Resources/app/`). When present, `node-launcher.ts` launches it
with the **bundled Bun** (`process.execPath` inside the `.app`) in headless
`node` mode, into a per-user managed home at
`~/.openagents/autopilot-desktop/.pylon-local` (a packaged install has no
writable repo root). Supervision, readiness, and stop/restart are identical to
the dev path; `PYLON_HOME` is forced to the managed home so discovery + the
poller pick the launched node up unchanged.

The current build path creates that artifact before Electrobun packaging:

```sh
bun run --cwd apps/autopilot-desktop build:pylon-node
```

`electrobun.config.ts` copies `resources/pylon-node/index.js` to
`pylon-node/index.js`, which lands in the runtime payload at
`Resources/app/pylon-node/index.js`. `node-launcher.ts` resolves that path from
`PATHS.RESOURCES_FOLDER` and launches it in headless `node` mode. If the bundle
is absent or the runtime payload is not extracted, the packaged app still stays
honest `unavailable` / offline (no false "online").

CS-B1 proof for the signed/notarized packaged-node path is recorded at
`docs/launch/2026-06-18-autopilot-desktop-cs-b1-proof.md`.

## Auto-onboarding smokes (AO-1..AO-6, EPIC #5441)

A fresh install converges, headlessly (no GUI, no terminal, no env vars), to a
registered → presence-live → payout-target-registered → Tassadar-joined →
earning-ready node. Two proofs cover this:

- **Phase 1 — convergence proof** (AO-1/AO-2, #5442/#5443):
  `bun run proof:auto-onboarding`. Drives the real Pylon node through the launcher
  into a fresh managed home against a mock `openagents.com`, asserting the 9-gate
  chain (identity → register → token persisted → presence (bearer) → payout
  target → assignment poll).
- **AO-6 — end-to-end first-run smoke** (#5447): `bun run smoke:auto-onboarding-e2e`.
  Extends Phase 1 with the AO-3 identity-choice gates (both paths; never
  overwrites a home), asserts the AO-3 chosen name flows into registration, and
  asserts the AO-4 wizard projects each step from real state (incl. the
  not-complete-until-settled and offline → retry behaviors). The from-DMG
  rendered window, production pylon detail, and real settled Bitcoin receipt
  are recorded in the live-production verification record; the smoke still
  never fakes those physical gates.

Black-screen regression guard (commit `73cada159`): `view`/`crashView` must
return a Foldkit `Document` (`{ title, body }`), not a bare `Html`, or the window
never mounts. `bun test tests/black-screen-guard.test.ts` fails on a regression.

The AO-6 verification record + the from-DMG rerun runbook live at
`docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md`.

## Built-in Agent (#5063)

The desktop source includes the no-user-key built-in agent flow. The first
screen has **Go online**, and the Agent pane shows the same readiness state. The
webview only sees public-safe readiness fields; the Bun host keeps cloud control
credentials and starts a fixed, bounded `codex` session on OpenAgents compute.

Runtime/package env:

- `OA_CLOUD_CONTROL_URL` and `OA_CLOUD_CONTROL_TOKEN` — required for hosted
  OpenAgents compute. These are package/runtime secrets, not user-supplied API
  keys and not projected into the webview.
- `OPENAGENTS_BUILTIN_AGENT_ENABLED=0` — disables the built-in agent path.
- `OPENAGENTS_BUILTIN_AGENT_LANE=cloud-gcp|cloud-shc` — default `cloud-gcp`.
- `OPENAGENTS_BUILTIN_AGENT_MODEL_SET` — display label, default
  `openagents-hosted-gemini`.
- `OPENAGENTS_BUILTIN_AGENT_MAX_SESSION_SECONDS` — bounded to `60..1200`,
  default `600`.
- `OPENAGENTS_BUILTIN_AGENT_DAILY_SESSION_CAP` — bounded to `1..20`, default
  `3`.
- `OPENAGENTS_BUILTIN_AGENT_WORKTREE` — optional managed scratch workspace; by
  default the app creates `workrooms/builtin-agent-workspace` under the managed
  Pylon home.

The desktop host records successful built-in-agent starts in
`builtin-agent-usage.json` under the managed Pylon home and blocks new starts
after the configured daily cap for that UTC day. Hosted compute should still
enforce its own server-side entitlement and metering; the local cap is a
client-side fail-closed bound, not billing authority.

The source path is yellow, not a green public promise, until a signed/notarized
desktop recut ships with the hosted-compute runtime configuration and a public
from-install **Go online** smoke shows a session running without a user API key.

## First-run Health (#5064)

The desktop source includes a public-safe first-run health projection for normal
installs. Bun owns the private checks and exposes `installReadiness` to the
webview; the webview shows a compact line on the home screen and the full
breakdown under **Settings → First-run Health**.

The projection composes:

- local Pylon lifecycle (`launching`, `online`, `adopted`, `failed`,
  `unavailable`) plus readable home/control-token discovery,
- built-in-agent readiness from #5063,
- platform + runtime (`source` or `packaged`),
- default-on auto-update status.

The webview receives only booleans, labels, and blocker refs such as
`blocker.autopilot.install.local_pylon_failed` or
`blocker.autopilot.builtin_agent.hosted_compute_unconfigured`; it never receives
control tokens or hosted-compute credentials. The highest-ROI action is explicit
(`Go online`, `Wait for local node`, `Restart Autopilot or install a newer
build`, or `Install the hosted-compute desktop recut`) so the normal path does
not require reading the full public `AGENTS.md`.

## Promise Surfacing (#5065)

The Agent pane includes **Surface Promise Gap**, an Orrery-style public report
flow for Product Promises. It accepts exact report fields (`promiseId`, surface,
claim text, expected/observed behavior, evidence or steps, environment, impact,
and suggested state), fetches the live `/api/public/product-promises` ledger,
checks Product Promises Forum topics for exact promise-id matches, and builds a
public-safe Forum report.

Bun owns the registered-agent token. Configure either:

- `OPENAGENTS_PROMISE_SURFACING_AGENT_TOKEN`
- `OPENAGENTS_AGENT_TOKEN`

When a token is present, Bun posts to:
`POST /api/forum/forums/product-promises/topics` with an idempotency key. When
no token is present, the UI still produces the exact draft and returns the
`env.OPENAGENTS_AGENT_TOKEN` blocker ref. The generated report explicitly says
**Surface only. Do not ship code from this report.** Maintainers can triage,
reproduce, patch copy/projections/product behavior, and open a strict GitHub
issue only if the report becomes a concrete reproducible bug.

## macOS Signing And Notarization

Sign and notarize a built (`build:stable`) `.app` with Apple Developer ID:

```sh
OA_DESKTOP_APP_PATH="/path/to/Autopilot.app" \
OA_DEVELOPER_ID_APPLICATION="Developer ID Application: OpenAgents, Inc. (...)" \
bun run --cwd apps/autopilot-desktop notarize:macos
```

`scripts/notarize-macos.sh` signs with hardened runtime, submits the archive to
`xcrun notarytool`, staples the ticket, and verifies with `spctl`. Notary auth
uses either `OA_NOTARY_KEYCHAIN_PROFILE` or the App Store Connect API key in the
workspace `.secrets/appstoreconnect.env`.

## Desktop Update Feed

After notarization, stage a full artifact plus an optional BSDIFF delta for
`updates.openagents.com`:

```sh
bun run --cwd apps/oa-updates desktop:publish -- \
  --channel stable \
  --version 0.0.2 \
  --artifact ./AutopilotDesktop-0.0.2.zip \
  --previous-version 0.0.1 \
  --previous-artifact ./AutopilotDesktop-0.0.1.zip
```

The script writes content-addressed artifacts and `desktop-dist/releases.json`.
Deploy `apps/oa-updates` with `OA_DESKTOP_RELEASES_DIST=/app/desktop-dist`, and
desktop clients read `https://updates.openagents.com/desktop/stable/feed.json`.

Pricing is TBD.
