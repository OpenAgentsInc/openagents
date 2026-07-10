# ChatGPT Desktop App (macOS) Teardown — 2026-07-10

Read-only inspection of the ChatGPT desktop app installed at
`/Applications/ChatGPT.app` (installed/updated 2026-07-09), plus public
corroboration. Purpose: competitive/architecture research to inform OpenAgents
Desktop (`apps/openagents-desktop`, Electron + Effect Native) decisions.

Every claim is tagged:

- **[bundle]** — observed directly in the installed app bundle
- **[runtime]** — observed from live process/filesystem state on this Mac
- **[public]** — publicly reported, with source

No secrets, credentials, or proprietary source were read or copied; inspection
was limited to bundle metadata, file listings, public strings, and one
non-sensitive manifest (`package.json`).

## TL;DR

The "ChatGPT" app on macOS is no longer the native Swift/AppKit app. As of
July 9, 2026 it is the **Codex desktop app rebranded** — an Electron-style
JavaScript app running on **"Owl"**, OpenAI's own Chromium-150-based,
Electron-compatible runtime — with a fleet of sidecar binaries: the
open-source Rust `codex` CLI as an `app-server` over stdio, a **second bundled
Node.js runtime** for computer-use sessions, a screen-memory capture daemon,
ripgrep, and half a dozen native `.node` addons. Total bundle: **1.4 GB**.
The bundle identifier is literally `com.openai.codex` shipping under the
display name "ChatGPT".

## 1. Identification

| Field | Value | Evidence |
|---|---|---|
| Path | `/Applications/ChatGPT.app` | [bundle] |
| `CFBundleIdentifier` | `com.openai.codex` | [bundle] `Info.plist` |
| `CFBundleDisplayName` | `ChatGPT` | [bundle] |
| `CFBundleAlternateNames` | `["Codex"]`; `BundleSigningBaseName: Codex` | [bundle] |
| App version | `26.707.31428` (build `5059`) | [bundle] |
| Runtime framework | `Codex Framework.framework` `150.0.7871.101` (= Chromium 150) | [bundle] framework `Info.plist` |
| `NSPrincipalClass` | `BrowserCrApplication` (Chromium's NSApplication subclass) | [bundle] |
| Total size | 1.4 GB (`du -sh`) | [bundle] |
| Min macOS | 12.0 | [bundle] |
| Signing | Developer ID `OpenAI OpCo, LLC (2DC432GLL2)`, notarized, hardened runtime | [bundle] `codesign -dv`, `spctl -a` → `accepted, source=Notarized Developer ID` |

The old native app survives separately as "ChatGPT Classic" and is being
phased out; existing Codex app installs were auto-updated into this app
[public: [Michael Tsai roundup](https://mjtsai.com/blog/2026/07/10/chatgpt-work-and-chatgpt-classic/),
[OpenAI Help Center](https://help.openai.com/en/articles/20001276-moving-to-the-new-chatgpt-desktop-app),
[Daring Fireball](https://daringfireball.net/linked/2026/07/09/todays-the-day-openai-fucked-up-the-chatgpt-mac-app)].
A stale `browser_crashpad_handler` from a now-deleted `/Applications/Codex.app`
(framework `149.0.7827.197`) was still running on this machine — the rename
happened in place via the updater [runtime].

## 2. Framework fingerprint: not stock Electron — "Owl"

It quacks like Electron but isn't stock Electron:

- `Contents/Resources/app.asar` (195 MB) + `app.asar.unpacked` +
  `electron.icns` + `default_app` — Electron packaging artifacts [bundle].
- `ElectronAsarIntegrity` key in `Info.plist` [bundle].
- Framework binary contains `electron/js2c/browser_init`,
  `electron/js2c/renderer_init`, `contextBridge` strings, and
  `Chrome/150.0.7871.101` [bundle: `strings` on `Codex Framework`].
- BUT there is no `Electron Framework.framework`. The renamed
  `Codex Framework.framework` uses **Chrome's bundle layout**, not Electron's:
  helpers live inside the framework at
  `Versions/150.0.7871.101/Helpers/` as `Codex (Renderer).app`,
  `Codex (GPU).app`, `Codex (Service).app`, `Codex (Alerts).app`
  (`…framework.AlertNotificationService`), plus Chrome-browser-only artifacts
  Electron never ships: `app_mode_loader`, `web_app_shortcut_copier`,
  `browser_crashpad_handler`, `MEIPreload`, `IwaKeyDistribution`,
  `PrivacySandboxAttestationsPreloaded` [bundle].
- Non-stock strings such as `Electron node main process disconnected with
  code` and `Electron app main process was not started before browser
  startup.` suggest the main JS process is decoupled from the Chromium browser
  process, unlike stock Electron [bundle].
- `Resources/owl-app.ini` → `[Owl] UserDataDirectoryName=Codex`;
  `owl-electron-app.json` → `"runtimeName": "owl"`, plus the CI packaging
  path: `packagedFrom: /Users/runner/work/openai/openai/codex/codex-apps/electron/out/ChatGPT-darwin-arm64/ChatGPT.app`
  [bundle]. So the desktop app lives in OpenAI's internal `openai/openai`
  monorepo under `codex/codex-apps/electron` and is built in CI (GitHub
  Actions runner path), on a runtime named **owl**.
- The app's own `package.json` (see §4) has `owl` scripts
  (`owl-shell.mjs run/package`, `ensure-owl-electron-types.mjs`) while pinning
  `electron: 42.1.0` as a devDependency — i.e. stock Electron for local dev,
  Owl for shipping.
- Community analysis corroborates: Owl is a Chromium runtime with an
  Electron-compatibility layer (`require("electron")` still works; CDP is
  available but off by default) [public:
  [codex-plusplus OWL-RUNTIME doc](https://raw.githubusercontent.com/b-nnett/codex-plusplus/master/docs/OWL-RUNTIME.md)].

The framework links a very wide native surface — AppKit, Metal/MetalKit,
ScreenCaptureKit, AVFoundation, CoreML, Vision, CoreBluetooth, IOBluetooth,
GameController, CoreMIDI, LocalAuthentication, SafariServices, CryptoTokenKit,
UserNotifications, and more [bundle: `otool -L`]. There is also an AppleScript
dictionary (`Resources/scripting.sdef`, Chromium's standard suite bound to
`BrowserCrApplication`) and a native dock-tile plugin
(`PlugIns/CodexDockTilePlugin.plugin`) [bundle].

Verdict: **an Electron-API app on a first-party Chromium fork** — the runtime
consequence of OpenAI owning a browser engine (Atlas) and wanting
browser/computer-use control the stock Electron surface doesn't give them.

## 3. Bundle anatomy (1.4 GB)

| Component | Size | What it is |
|---|---|---|
| `Frameworks/Codex Framework.framework` | 346 MB | Owl/Chromium 150 runtime |
| `Resources/cua_node/` | 358 MB | A complete second **Node.js distribution** (`bin`, `lib`, `include`, `share`, `CHANGELOG.md`) for computer-use agent sessions |
| `Resources/codex` | 248 MB | The Rust **codex CLI**, `codex-cli 0.144.0-alpha.4` (`--version` output) |
| `Resources/app.asar` | 195 MB | Main + renderer JS app (6,275 entries; 4,819 under `/webview`) |
| `Resources/plugins/openai-bundled/` | 187 MB | Bundled Codex plugins: `latex`, `deep-research`, `visualize`, `sites`, `browser`, `chrome`, `computer-use`, `record-and-replay` — each with `.codex-plugin/plugin.json` + `.mcp.json` (MCP servers), `license: "Proprietary"` |
| `Resources/codex-code-mode-host` | 44 MB | Mach-O sidecar ("code mode" host; also spawned by the npm codex CLI) |
| `Resources/codex_chronicle` | 4.4 MB | Mach-O; `--help`: "Capture sparse screen-memory frames until interrupted" — ambient screen-memory capture daemon |
| `Resources/rg` | 3.9 MB | ripgrep, arm64 |
| `Resources/native/` | 1.9 MB | Native addons: `sky.node`, `devicecheck.node`, `remote-control-device-key.node`, `browser-use-peer-authorization.node`, `input-monitoring-permission.node`, `avatar-overlay.node`, `sparkle.node`, plus helper binaries `bare-modifier-monitor`, `launch-services-helper`, `remote-hosted-pip` |
| `Resources/skills/` | 372 KB | Skills tree with `.curated/hatch-pet` and `.curated/onboard-new-user` |
| `Frameworks/Sparkle.framework` | — | Sparkle **2.9.1** updater |
| 55× `*.lproj` + `Assets.car` | — | Native localization + asset catalog |

All [bundle]. `app.asar.unpacked` holds the compiled native modules:
`node-pty`, `better-sqlite3`, `node-mac-permissions`, `objc-js`,
`@worklouder/device-kit-oai` [bundle].

`sky.node` and the entitlement app group `com.openai.sky.CUAService` (§7)
trace to Software Applications Incorporated's "Sky" macOS agent product,
which OpenAI acquired [public: widely reported acquisition, Nov 2025;
`~/.codex/computer-use/Codex Computer Use.app/Contents/MacOS/SkyComputerUseService`
was running on this machine [runtime]].

## 4. Main-process app: `openai-codex-electron`

From `package.json` inside `app.asar` (read via `asar extract-file`, single
metadata file) [bundle]:

- `"name": "openai-codex-electron"`, `"productName": "Codex"`,
  `"codexAppBrand": "chatgpt"` — **one codebase, brand switch**. Also
  `codexBuildFlavor: prod`, `codexBuildNumber: 5059`.
- Entry: `.vite/build/early-bootstrap.js` → Vite-bundled main process
  (`main-*.js`, `preload.js`, `sandbox-preload.js`, `worker.js`,
  `codex-micro-service-*.js`, `sqlite-*.js`, `trace-recording-upload-*.js`).
- Build/tooling: **pnpm workspace, Electron Forge 7.11.1**
  (`plugin-vite`, `plugin-fuses`, `plugin-auto-unpack-natives`,
  `@electron/notarize`), **Vite 8 / Rolldown**, **tsgo** (TypeScript native
  compiler) instead of tsc, **oxlint/oxfmt** (Oxc), **Vitest 4**,
  **Playwright** (an `playwright-electron-agent-cdp` script drives the app via
  CDP for agentic e2e), `fb-dotslash`. Makers: dmg, zip, **deb, rpm, msix** —
  Linux and Windows-Store targets are wired even though only mac/Windows have
  shipped.
- Notable runtime deps: `better-sqlite3` (local persistence), `node-pty`
  (terminals), **`objc-js`** (calling Objective-C APIs from JS rather than
  writing a Swift shell), `ws` + `bufferutil`, **`capnweb`** (Cap'n-Web
  RPC), `zod` 4, `electron-context-menu`, `mdast-*`, `smol-toml` +
  `ssh-config` + `shlex` + `which` (dev-environment introspection),
  `@sentry/electron` + `@sentry/node` (crash/error telemetry),
  `@oai/integrity-state` (internal, `lib/applied/js/oai_js_integrity_state`),
  `@worklouder/*device-kit*` (hardware device integration, vendored from an
  internal registry), and internal workspace packages `app-server-types`,
  `protocol`, `commands`, `shared-node`, `browser-api`/`browser-common`
  (from `lib/browser_use/` in the monorepo).

Electron fuse wire is present in the framework binary (sentinel
`dL7pKGdnNz…`, version 1, 9 fuses, raw states `101100011`) [bundle]. Exact
enum mapping is unverifiable against their fork, so no per-fuse claims;
`ElectronAsarIntegrity` hashes in `Info.plist` indicate asar integrity
checking is wired regardless.

## 5. Renderer: React SPA, no web wrapper

The UI is a local Vite-built SPA in `app.asar` under `/webview` (4,819
files) — this is not a wrapped chatgpt.com [bundle]. Stack, confirmed via
`THIRD_PARTY_NOTICES.txt` versions (1,145 attributed packages) and asset
fingerprints [bundle]:

- **React 19.2.5** + `react-dom`, **react-router 7.13.1**
- **TanStack**: react-query 5.90, react-form, react-table 8, react-store
- State: **jotai 2.19** (+ `jotai-effect`, `jotai-tanstack-query`) and
  **zustand 5** (both, in one app), immer
- **Radix UI** (full primitive set) + **Tailwind CSS 4** (`@layer theme, base,
  components, utilities` inline in `index.html`) + `lucide-react` +
  **framer-motion 12**
- Content rendering: `react-markdown`, **shiki 3** (syntax highlighting),
  **KaTeX**, **mermaid 11**, `pdfjs-dist`/`react-pdf`, **three 0.179**,
  `3Dmol` (molecule viewer!), **ProseMirror** (full suite — the composer is
  ProseMirror, same family as our editor references)
- Terminal: **@xterm/xterm 5.5** + fit/clipboard/web-links addons
- Telemetry: Sentry browser + **Session Replay** (`@sentry-internal/replay`,
  `replay-canvas`)
- Duplicate versions throughout (two tailwinds, three zods, two shikis, two
  mermaids…) — multiple sub-apps bundled together.

Chunk names expose the product surface: `chatgpt-conversation-page`,
`hotkey-window-*`, `new-thread-panel-page`, `appgen-library-page`
(app generation), `pull-request-route`, `codex-micro-onboarding-host`,
`avatar-overlay-page`, `pets-settings` (the "hatch-pet" skill has UI),
`business-checkout-switch-workspace-dialog`, `remote-conversation-*`
[bundle]. `webview/apps/` ships icons for every editor/terminal it can hand
off to (VS Code, Cursor, Windsurf, Zed, Ghostty, Warp, JetBrains suite,
Xcode, Antigravity…) [bundle].

## 6. Runtime shape (observed live)

With the app running [runtime]:

- Standard Chromium multiprocess: main `ChatGPT` binary (a 70 KB stub linking
  only `libSystem` — the real work is in the framework), `Codex (Renderer)`
  ×2, `Codex (Service)` as `network.mojom.NetworkService`,
  `storage.mojom.StorageService`, and gpu-process, plus two
  `browser_crashpad_handler`s.
- **`codex app-server --listen stdio://`** ×2 — the Rust agent core runs as a
  child process speaking the app-server protocol over stdio (same protocol
  surface the open-source CLI exposes). A third instance:
  `codex -c features.code_mode_host=true app-server --analytics-default-enabled`.
- `codex-code-mode-host` sidecar.
- `cua_node/bin/node --experimental-vm-modules …/kernel.js --session-id <id>`
  ×3 plus `node_repl` ×4 — per-session computer-use "kernels" on the bundled
  Node, running out of temp dirs.
- `native/bare-modifier-monitor --key DoubleCommand --immediate` — dedicated
  tiny process watching the global double-Command hotkey.
- `SkyComputerUseService` from `~/.codex/computer-use/Codex Computer Use.app`.
- A Chrome extension host: `ChatGPT for Chrome chrome-extension://…` from
  `~/.codex/plugins/cache/openai-bundled/chrome/latest/extension-host/`.
- **No TCP listeners** owned by these processes (`lsof -iTCP -sTCP:LISTEN`):
  IPC is stdio/domain-socket, not localhost HTTP.

User data [runtime, names only]:

- `~/Library/Application Support/Codex/` is a **full Chromium browser
  profile** — `Default/`, `Local State`, `Cookies`, `component_crx_cache`,
  `extensions_crx_cache`, `first_party_sets.db`, `Crashpad/`, Dawn/GPU
  caches, safe-browsing-style component dirs (`CertificateRevocation`,
  `Crowd Deny`, `CaptchaProviders`…), plus `codex-browser-app/`. Far heavier
  than a stock Electron profile; this is a browser.
- `~/.codex/` is **shared with the CLI**: `auth.json`, `config.toml`,
  `history.jsonl`, `sessions`-style archives, and the agent-OS layer —
  `memories_1.sqlite`, `goals_1.sqlite`, `logs_2.sqlite`,
  `ambient-suggestions/`, `automations/`, `computer-use/`, `browser/`,
  `skills`/plugins caches, `mcp-oauth-locks/`.

## 7. Security & signing posture

[bundle: `codesign -d --entitlements`, `security cms -D` on
`embedded.provisionprofile`]

- Developer ID signed + notarized, hardened runtime flag set.
- **Not App-Sandboxed** (`com.apple.security.app-sandbox: false`).
- `allow-jit` and `allow-unsigned-executable-memory` (V8 requirements).
- `com.apple.security.automation.apple-events: true` — it scripts other apps.
- Camera + microphone entitlements.
- App groups: `…com.openai.codex.notifications`,
  `2DC432GLL2.com.openai.sky.CUAService`; keychain groups `2DC432GLL2.*`,
  `…com.openai.shared`. An embedded provisioning profile
  ("Codex Desktop AKV 2026-05-24") carries these — unusual for Developer ID
  apps, needed for DeviceCheck/app-group entitlements (`devicecheck.node`).
- Native permission brokers as dedicated addons
  (`input-monitoring-permission.node`, `node-mac-permissions`).

## 8. Update mechanism

**Sparkle 2.9.1** (native macOS updater), not Squirrel [bundle:
`Frameworks/Sparkle.framework`, `native/sparkle.node` bridge]. Feed and key
are in `package.json` [bundle]:

- `codexSparkleFeedUrl: https://persistent.oaistatic.com/codex-app-prod/appcast.xml`
- `codexSparklePublicKey` (EdDSA) pinned in the app manifest.

This is how Codex.app became ChatGPT.app in place on July 9 [runtime +
public].

## 9. Open/closed split

- The **codex CLI is open source** (Apache-2.0, `github.com/openai/codex`,
  Rust) — and it is literally the agent engine inside the desktop app
  (`Resources/codex`, `app-server` protocol) [bundle + public].
- The **desktop app is closed**: built from the internal `openai/openai`
  monorepo (`codex/codex-apps/electron` per `packagedFrom`), bundled plugins
  are marked `"license": "Proprietary"` with `"repository":
  "https://github.com/openai/openai"` (private) [bundle]. This matches the
  split discussed in episode 248: open terminal/CLI, closed desktop surface.
- Owl itself is unreleased; even `@electron/fuses` tooling fails on the bundle
  because the framework path is nonstandard [bundle].

## 10. SO WHAT for OpenAgents Desktop

Context: OpenAgents Desktop is Electron + Effect Native
(`apps/openagents-desktop`; Effect Native per `docs/effect-native/`,
MASTER_ROADMAP §EN).

**1. Electron is validated at the highest possible level.** OpenAI killed
their flagship native Swift/AppKit app and replaced it with an
Electron-architecture app — on the same week Gruber was campaigning for
native Mac apps. Whatever taste objections exist, the company with the most
resources on earth decided the web-stack desktop shell is the right substrate
for an agentic super-app. Our Electron choice needs no apology; the
differentiation battle is elsewhere.

**2. The differentiation is exactly where the owner's thesis says it is.**
This app is observably a rebrand under pressure: `com.openai.codex` shipping
as "ChatGPT", `codexAppBrand: "chatgpt"` as a build flag, duplicate copies of
tailwind/zod/shiki/mermaid from glued-together sub-apps, jotai AND zustand AND
TanStack Store in one renderer, a 1.4 GB bundle, and a public backlash about
chat being "a sub section of a sub pane" (Tsai roundup). Predictable, open,
typed software — one schema layer (Effect) instead of three state libraries
and boundary zod — is a real, visible contrast, not marketing.

**3. Their best architectural idea is one we already hold: the agent core as
a typed sidecar protocol.** The single most load-bearing seam in the app is
`codex app-server --listen stdio://` — open-source Rust engine, closed UI on
top, talking a versioned protocol (`app-server-types`, `protocol` workspace
packages) over stdio with no localhost port. That is the same shape as our
desktop↔agent seam and it's the piece of their design worth taking seriously:
version the protocol package explicitly, keep transport stdio (no open
ports), and let CLI and desktop share one engine.

**4. Don't fork the runtime.** Owl (a Chromium fork with an Electron compat
layer) only makes sense because OpenAI owns a browser (Atlas) and wants
browser-grade computer-use, extensions, and profile machinery inside the app.
The cost is visible: Chrome-layout helpers, component-updater caches, a full
browser profile in Application Support, and tooling breakage. For us, stock
Electron + hardened boundary is the right cost point; we get to ride upstream
security updates instead of staffing a browser team.

**5. Sidecar sprawl is the anti-pattern to avoid.** Five+ Mach-O sidecars,
a second complete Node distribution (358 MB) beside the Electron runtime's
own Node, per-session kernel processes in temp dirs, and a screen-memory
capture daemon (`codex_chronicle`) — three languages, two Node runtimes, one
bundle. Effect Native's whole-app-one-runtime premise is the direct
counter-position. Where we need native capability, their `objc-js` pattern
(script ObjC from JS) and dedicated tiny permission/hotkey brokers
(`bare-modifier-monitor`, permission `.node` addons) are cheaper than shipping
extra runtimes.

**6. Update + integrity checklist worth copying.** Sparkle 2.x with a pinned
EdDSA key and a static appcast (they renamed an entire product in place with
it), `ElectronAsarIntegrity`, Forge fuses plugin, notarization in CI, and
auto-unpacked native modules. Also note what they gave up: no App Sandbox,
plus apple-events + input-monitoring + screen-capture — the permission
surface of an OS agent. If OpenAgents Desktop stays sandbox-friendlier with a
narrower entitlement set, that's a trust story against a competitor whose app
is a notarized-but-unsandboxed automation engine with Sentry Session Replay
compiled into the renderer.

**7. Their plugin/skill packaging is convergent with ours.** Bundled plugins
are directories with `plugin.json` + `.mcp.json` (MCP servers) and a
`marketplace.json`; skills are a curated directory tree. This is the same
shape as our typed catalog direction — except theirs is `"license":
"Proprietary"` and unsigned-by-scheme, while a typed, open, verifiable
catalog remains open ground.

**8. Stack overlap map** (for calibration, not imitation): React 19,
Tailwind 4, Radix, TanStack Query, xterm, shiki, ProseMirror, framer-motion,
better-sqlite3, node-pty, Vite/Forge, pnpm — largely the same shelf we build
from. They also run oxlint/oxfmt and tsgo at scale, which is a useful
existence proof for adopting Oxc-family tooling on a large TS desktop
codebase.

## Sources

- Bundle/runtime evidence: `/Applications/ChatGPT.app` as installed
  2026-07-09 (version 26.707.31428), live process table, and
  `~/Library/Application Support/Codex` + `~/.codex` directory listings
  (names only).
- [OpenAI Help Center — Moving to the new ChatGPT desktop app](https://help.openai.com/en/articles/20001276-moving-to-the-new-chatgpt-desktop-app)
- [Michael Tsai — ChatGPT Work and ChatGPT Classic (2026-07-10)](https://mjtsai.com/blog/2026/07/10/chatgpt-work-and-chatgpt-classic/)
- [Daring Fireball — "Today's the Day OpenAI Fucked Up the ChatGPT Mac App" (2026-07-09)](https://daringfireball.net/linked/2026/07/09/todays-the-day-openai-fucked-up-the-chatgpt-mac-app)
- [Neowin — OpenAI launches ChatGPT Work and unveils unified desktop app with Codex built in](https://www.neowin.net/news/openai-launches-chatgpt-work-and-unveils-unified-desktop-app-with-codex-built-in/)
- [codex-plusplus — Owl Runtime Surface notes](https://raw.githubusercontent.com/b-nnett/codex-plusplus/master/docs/OWL-RUNTIME.md)
- [openai/codex — open-source CLI (Apache-2.0)](https://github.com/openai/codex)
