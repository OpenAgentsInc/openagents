# Autopilot

Autopilot is the Electrobun desktop shell for the Autopilot Coder GUI.
It is planned as a Bun-native sibling to the Pylon TUI: the Bun main process
will own local Pylon control access over loopback, while the webview renders the
operator interface.

The UI is intended to reuse the same Effect and Foldkit stack as
`apps/openagents.com`, with typed bun<->webview RPC as the boundary between
credentialed local control and public-safe projections.

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

Until the bundle is shipped, the packaged app resolves no entry and stays honest
`unavailable` / offline (no false "online"). **Remaining dependency (Pylon-side
follow-up, NOT done here):** the desktop build needs a bundled, headless Pylon
node artifact to copy. Pylon's `src/index.ts` cannot be `bun build`-bundled today
because `@opentui/core` pulls platform-native optional deps via cross-platform
dynamic `import()` that the bundler cannot resolve. Pylon must publish a
headless, no-OpenTUI bundle (e.g. a `bun build`-able `node`-mode entry with the
TUI externalized, or a `bun build --compile` single binary). Once that exists,
add a `build.copy` entry in `electrobun.config.ts` mapping it to `"pylon-node"`
and the packaged path activates with no further desktop changes.

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
