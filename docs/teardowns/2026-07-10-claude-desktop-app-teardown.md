# Claude Desktop App (macOS) Teardown — 2026-07-10

Read-only inspection of the Claude desktop app installed at
`/Applications/Claude.app` (installed 2026-07-07), plus public corroboration.
Purpose: competitive/architecture research to inform OpenAgents Desktop
(`apps/openagents-desktop`, Electron + Effect Native) decisions. This is the
Claude counterpart to
[`docs/teardowns/2026-07-10-chatgpt-desktop-app-teardown.md`](2026-07-10-chatgpt-desktop-app-teardown.md).

Every claim is tagged:

- **[bundle]** — observed directly in the installed app bundle
- **[runtime]** — observed from live process, UI, or filesystem state on this Mac
- **[public]** — publicly documented, with source

No credentials, conversation content, source maps, or user-data file contents
were read or copied. Inspection was limited to bundle metadata and file
listings, public strings, package manifests, names-only user-data listings, and
a sanitized live UI/process observation.

## TL;DR

Claude is a **stock Electron 42.5.1 / Chromium 148** application, not a custom
runtime. The 718 MB universal macOS bundle contains a small native window shell,
a complete 102 MB build of Claude's **Ion** web SPA, Rust and Swift native
addons, a direct MCP host, Office 365 MCP support, packaged skills, and two
small architecture-specific disk images used by its local-agent VM machinery.

The main product UI is hybrid: Electron loads a local title/error shell from
`app.asar`, then places the live `claude.ai` app in a `WebContentsView` (observed
at `claude.ai/epitaxy`). A full copy of the web SPA is also bundled and served
through the private `app://localhost` protocol for pinned/local operation. The
web surface is React 19; the Electron shell is React 18.

For Code, the desktop app uses the Claude Agent SDK to spawn a separately
downloaded, versioned **Claude Code** executable over newline-delimited
`stream-json` on stdin/stdout. That executable is 232 MB on this machine. For
local Cowork, the agent loop runs natively while shell commands and generated
code run inside a downloaded, hash-verified Linux root filesystem under Apple
`Virtualization.framework`. In other words: remote web UI, native Electron
orchestrator, closed CLI sidecar, MCP utility processes, and a hardware VM —
four execution planes behind one window.

## 1. Identification

| Field | Value | Evidence |
|---|---|---|
| Path | `/Applications/Claude.app` | [bundle] |
| `CFBundleIdentifier` | `com.anthropic.claudefordesktop` | [bundle] `Info.plist` |
| Display/product name | `Claude` | [bundle] |
| App version | `1.19367.0` | [bundle] |
| Available staged update | `1.20186.0` | [runtime] app UI, “Relaunch to update” |
| Runtime | Electron `42.5.1`, Chromium `148.0.7778.271` | [bundle] framework `Info.plist` + framework strings |
| `NSPrincipalClass` | `AtomApplication` | [bundle] |
| Total bundle size | 718 MB (`du -sh`) | [bundle] |
| Architectures | universal `arm64` + `x86_64` | [bundle] `file` |
| Minimum macOS | 12.0 | [bundle] |
| Signing | Developer ID `Anthropic PBC (Q6L2SF6YDW)`, stapled notarization ticket, hardened runtime | [bundle] `codesign -dvvv` |

The bundle declares handlers for `.mcpb` and legacy `.dxt` Desktop Extensions,
`.skill` files, folders, and arbitrary files, plus `claude:` and Microsoft MSAL
URL schemes [bundle]. Publicly, Anthropic positions the app as one surface for
Chat, Cowork, and Code, with local files, local extensions, Chrome, computer
use, SSH, and cloud sessions [public:
[Claude download](https://claude.com/download),
[Claude Code Desktop docs](https://code.claude.com/docs/en/desktop)].

## 2. Framework fingerprint: stock Electron, extended above the runtime

This is conventional Electron packaging:

- `Contents/Frameworks/Electron Framework.framework` identifies itself as
  `com.github.Electron.framework`, version `42.5.1` [bundle].
- The framework reports `Chrome/148.0.7778.271 Electron/42.5.1`, contains
  `electron/js2c/browser_init`, `contextBridge`, and standard V8 snapshot
  artifacts [bundle].
- Standard helper applications ship as `Claude Helper`, `Claude Helper
  (Renderer)`, `(GPU)`, and `(Plugin)`, alongside `chrome_crashpad_handler`
  [bundle].
- Standard Electron/Squirrel macOS frameworks are present: `Squirrel`,
  `ReactiveObjC`, and `Mantle` [bundle].
- `app.asar`, `app.asar.unpacked`, `electron.icns`,
  `ElectronAsarIntegrity`, Electron Forge, Vite, and the Electron fuses plugin
  complete the normal Electron fingerprint [bundle].

Anthropic did not fork Chromium to add agent capability. It extended stock
Electron at the application boundary:

- a private Rust N-API module, `@ant/claude-native` (4.2 MB), built in the
  internal CI checkout `apps/packages/desktop/claude-native` [bundle];
- Swift N-API modules, `swift_addon.node` (36 MB) and
  `computer_use.node` (906 KB), linking AppKit, ScreenCaptureKit, Speech,
  SwiftUI, ServiceManagement, `Virtualization.framework`, and `vmnet`
  [bundle];
- a 2 MB `chrome-native-host` helper for the Claude-in-Chrome bridge and a
  small native `disclaimer` helper [bundle];
- Electron utility processes for Node/MCP work, rather than a second complete
  Node distribution [bundle + runtime];
- a host-native Claude Code sidecar and a hardware-virtualized Linux guest for
  agent execution [runtime + public].

Verdict: **stock Electron as the trusted orchestrator, with native modules and
separate execution substrates where the browser runtime is the wrong trust or
capability boundary.**

## 3. Bundle anatomy (718 MB, plus downloaded components)

| Component | Size | What it is |
|---|---|---|
| `Frameworks/Electron Framework.framework` | 478 MB | Universal Electron 42 / Chromium 148 runtime |
| `Resources/ion-dist/` | 102 MB | Full local build of the Claude/Ion web SPA: 1,022 files, 909 versioned assets |
| `Resources/app.asar.unpacked/` | 48 MB | Native addons and unpacked MCP/runtime pieces |
| `Resources/smol-bin.arm64.img` + `smol-bin.x64.img` | 22 MB + 23 MB | exFAT boot images containing the architecture-specific host/guest RPC and mount helper used by the local VM path |
| `Resources/app.asar` | 34 MB | Electron main process, preloads, small renderer shells, direct MCP hosts, and packaged metadata (213 entries / 175 files) |
| `@ant/claude-swift/swift_addon.node` | 36 MB | Swift bridge for macOS integration, VM lifecycle, speech, notifications, permissions, and related native services |
| `@ant/claude-native/claude-native-binding.node` | 4.2 MB | Rust N-API bridge for native desktop behavior |
| `resources/office365-mcp/` | 6.8 MB | Built-in Office 365 MCP, PDF extraction worker, and native Microsoft MSAL runtime |
| `Resources/fonts/` | 1.5 MB | Anthropic Sans and Serif variable fonts |
| `Resources/bundled-skills/` (inside asar) | 524 KB extracted | Zip-packaged `docx`, `pdf`, `pdf-reading`, `pptx`, `xlsx`, and `frontend-design` skills plus a manifest |
| `Helpers/chrome-native-host` | 2.0 MB | Native host for the Claude-in-Chrome bridge |
| `Frameworks/Squirrel.framework` | 616 KB | macOS updater and `ShipIt` helper |

All [bundle]. The app also carries Windows and Linux tray assets, both macOS
architectures, makers for DMG/PKG/ZIP, Squirrel/MSIX, and DEB, and per-platform
VM bundle definitions [bundle]. The public product now ships on macOS, Windows,
and Linux beta [public: [Claude download](https://claude.com/download)].

Two large components are deliberately **not** in the signed app bundle:

- `~/Library/Application Support/Claude/claude-code/2.1.202/claude.app` is a
  232 MB, arm64, separately signed and notarized Claude Code executable. Its
  `--version` is `2.1.202 (Claude Code)` [runtime].
- The local Cowork Linux root filesystem is downloaded on demand from
  `downloads.claude.ai/vms/linux/<arch>/<sha>` and validated against an
  embedded SHA-256 manifest before promotion. This machine did not have a VM
  rootfs cached at inspection time [bundle + runtime].

The signed Electron bundle is therefore only the first layer of the effective
installation footprint.

## 4. Main-process app: `@ant/desktop`

From `package.json` inside `app.asar` [bundle]:

- `name: "@ant/desktop"`, `productName: "Claude"`, version `1.19367.0`,
  description “Desktop application for Claude.ai.” The package is marked
  `private`.
- Entry: `.vite/build/index.pre.js`. The Vite output includes the main process,
  code-split chunks, preloads for `mainView`, `mainWindow`, quick entry, file
  search, transcript search, shell-path discovery, direct MCP hosts, computer
  use teaching/recording, Cowork artifacts, and file previews.
- Build/tooling: **Yarn workspace**, **Electron Forge 7.8**, **Vite 6.4**,
  React plugin, Electron fuses/notarization, **TypeScript 6** plus
  `@typescript/native-preview`, **oxlint/oxfmt**, **Vitest 3**, **Playwright
  1.57**, Knip, and Sentry's Vite plugin.
- Main-process/runtime dependencies exposed by the manifest include internal
  `@ant/claude-native`, `@ant/claude-swift`, Claude-in-Chrome MCP,
  computer-use MCP, image-generation service, `ws`, and `https-proxy-agent`.
  Build-time dependencies compiled into the app include the Claude Agent SDK
  `0.3.202`, Anthropic SDK, MCP SDK `1.29`, `node-pty`, `ssh2`, `sharp`, Zod,
  RxJS, Electron Store, Winston, and Sentry Electron.
- The CI/build path embedded in the Rust native addon is
  `/Users/runner/work/apps/apps/packages/desktop/claude-native/...`, indicating
  an internal repository named `apps` with the desktop product under
  `packages/desktop` [bundle].

The most important engine seam is explicit in the compiled SDK surface
[bundle]:

- Desktop resolves a versioned Claude Code executable, then spawns it with
  `--output-format stream-json --input-format stream-json --verbose`.
- Permission callbacks use `--permission-prompt-tool stdio`.
- Sessions can add directories, plugins, MCP configuration, models, thinking
  controls, resume/fork state, and partial-message streaming as CLI arguments.
- Environment attribution distinguishes `claude-desktop`, `local-agent`, and
  SDK entrypoints.

This is a sidecar protocol even though it is not named `app-server`: the public
Agent SDK is a process supervisor and typed message adapter over the proprietary
Claude Code CLI's JSON stream.

## 5. Renderer: remote Claude.ai plus a complete bundled SPA

The renderer is deliberately two-layered [bundle + runtime]:

1. `app.asar/.vite/renderer/main_window/index.html` is a local shell for the
   title bar and error UI. Its own source comment says: “everything else gets
   loaded from claude.ai.”
2. A separate `WebContentsView` hosts the product UI. Live accessibility state
   identified its URL as `claude.ai/epitaxy`, while the outer shell remained the
   local `file://.../main_window/index.html` [runtime].
3. `Resources/ion-dist/` contains a complete build of the same web product.
   Electron registers it behind `app://localhost`, pins bundled-SPA navigation
   to that origin, serves local assets, and proxies selected API paths under a
   generated CSP [bundle].

So Claude is neither a pure web wrapper nor a fully local renderer. It reuses
the live web product for normal operation while carrying a local SPA snapshot
as a controlled application origin.

The local Electron shell uses React 18.3 and Tailwind 3.4 [bundle manifest and
CSS]. The Ion SPA identifies React **19.2.4** and exposes named vendor chunks for
React, router, query, Radix, Base UI, Zustand, Motion, Zod, GrowthBook,
internationalization, virtualization, icons, lodash, Luxon, and Anthropic's own
design system [bundle]. Other fingerprints include:

- ProseMirror/Tiptap for rich composition;
- CodeMirror and Monaco for file/code editing;
- xterm.js and `node-pty` for integrated terminals;
- Shiki, KaTeX, Mermaid 11.15, and rich Markdown rendering;
- PDF, Office document, image, SVG, HTML, React, CSV/TSV, and interactive
  artifact surfaces;
- Sentry debug IDs/telemetry and GrowthBook feature gates.

The renderer contains several generations of the web stack — React 19 for Ion,
React 18 for the Electron shell, and an older React 16 attribution inside an
embedded subcomponent — but the split follows product boundaries more clearly
than the ChatGPT/Codex bundle's duplicated state-library accretion [bundle].

## 6. Runtime shape (observed live)

With the app idle on its Code home [runtime]:

- Main `Claude` process (a small universal launcher) with
  `chrome_crashpad_handler`.
- Standard Electron multiprocess tree: GPU process, NetworkService, three
  renderer processes, AudioService, VideoCaptureService, and a
  `node.mojom.NodeService` utility process.
- Renderers ran with `--enable-sandbox` and declared the app's custom schemes:
  `cowork-artifact`, `cowork-file`, `claude-media`, `claude-simulator`, `app`,
  and `sentry-ipc`.
- Squirrel's `ShipIt` helper remained resident for update staging.
- No Claude Code child was active while the Code home was idle. The desktop
  downloads and launches the CLI for a session rather than keeping it resident.
- No Claude-owned TCP listener was present at observation time. Normal UI
  traffic is outbound HTTPS; local subprocess control uses Electron IPC,
  stdio, OS services, or VM sockets rather than a permanent localhost control
  port.

User data [runtime, names only]:

- `~/Library/Application Support/Claude/` is a conventional Electron/Chromium
  profile: Cookies, IndexedDB for `https_claude.ai`, Local/Session Storage,
  Cache/Code Cache, GPU/Dawn caches, Crashpad, Trust Tokens, partitions for
  Cowork file and launch previews, and Sentry queue/state.
- The same directory also holds desktop-specific control data: versioned
  `claude-code/`, code/local-agent session directories, desktop extension
  configuration, device identity/registry, enabled CLI operations, extension
  blocklist, git-worktree registry, feature cache, and window state.
- `~/.claude/` remains the shared Claude Code configuration/session ecosystem:
  settings, projects, plugins/marketplaces, skills, session environment,
  history, file-history, caches, and debug records. No contents were read.

Anthropic's public architecture description matches the bundle: for local
Cowork, the agent loop and folder/web/MCP access run natively, while code and
shell execution run in a Linux VM under Apple Virtualization or Hyper-V, with
guest egress, syscall, and per-session isolation [public:
[Claude Cowork architecture overview](https://support.claude.com/en/articles/14479288-claude-cowork-architecture-overview)].

## 7. Security and signing posture

[bundle: `codesign`, entitlements, Electron fuses, embedded provisioning
profile]

- Developer ID signed, stapled notarization ticket, hardened runtime.
- **Not App-Sandboxed**: no `com.apple.security.app-sandbox` entitlement.
- Renderer helpers carry `allow-jit`; live renderer processes use Chromium's
  renderer sandbox.
- Main/helper entitlements include audio input, camera, Bluetooth, USB,
  printing, location, Photos, and `com.apple.security.virtualization`.
- Keychain groups cover Claude WebAuthn/hardware keys plus Microsoft identity
  storage. The embedded Developer ID provisioning profile is named
  `Claude - Developer ID - 20260410`.
- Info.plist usage strings cover microphone/dictation, camera, audio capture,
  Bluetooth hardware, Desktop/Documents/Downloads folder access, speech
  recognition, and local networking. The Documents string explicitly names
  scheduled tasks, live artifacts, and Cowork files.
- Computer use is implemented through a dedicated Swift addon linked to
  ScreenCaptureKit and ApplicationServices. Public docs require explicit macOS
  Accessibility and Screen Recording grants and per-app session approval
  [public:
  [Computer use in Claude Desktop](https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork)].

Electron fuse state is comparatively hardened [bundle]:

- `RunAsNode`: disabled
- cookie encryption: enabled
- `NODE_OPTIONS`: disabled
- Node CLI inspect arguments: disabled
- embedded asar integrity validation: enabled
- only load application code from asar: enabled
- WebAssembly trap handlers: enabled
- file-protocol extra privileges: **enabled**

`Info.plist` pins the SHA-256 hash of `Resources/app.asar`; `codesign --verify
--deep --strict` passes. The strongest residual boundary is therefore not code
signature integrity but the deliberate breadth of the host app: it is an
unsandboxed, networked automation process with filesystem, VM, browser, screen,
audio, camera, Bluetooth, and local-extension reach. The generated IPC layer
does perform sender-origin validation for `claude.ai`, preview origins, and
`app://localhost` before exposing privileged handlers [bundle].

## 8. Update mechanism: three independently moving planes

### Desktop shell

Claude uses **Squirrel.Mac**, not Sparkle [bundle]:

- `Squirrel.framework`, `ReactiveObjC.framework`, `Mantle.framework`, and the
  resident `ShipIt` helper are present.
- Electron `autoUpdater` uses a JSON feed at
  `https://api.anthropic.com/api/desktop/darwin/universal/squirrel/update`
  with a per-device query parameter [bundle].
- Managed settings can disable auto-updates or set an enforcement window.
- At inspection time the installed `1.19367.0` app had `1.20186.0` downloaded
  and ready to relaunch [runtime].

### Claude Code engine

The desktop manifest pins Claude Agent SDK `0.3.202`; its embedded release
manifest downloads a separately versioned Claude Code build from
`https://downloads.claude.ai/claude-code-releases`, checks its size and SHA-256,
and installs it under Application Support [bundle + runtime]. The installed
engine was `2.1.202`.

### Cowork VM

The app embeds VM bundle version
`6d1538ba6fecc4e5c5583993c4b30bb1875f0f5a` and per-architecture SHA-256
checksums. macOS/Linux hosts download `rootfs.img`; Windows downloads a VHDX,
kernel, and initrd. Downloads are staged, checksum-verified, and atomically
promoted; background warming can prefetch the next app-version mapping
[bundle].

The update story is therefore not “update the app.” It is a compatibility
matrix across Electron shell, live/bundled Ion SPA, Claude Agent SDK, Claude
Code executable, desktop extensions, and VM rootfs.

## 9. Open/closed split

- **Claude Desktop is closed**: `@ant/desktop`, `@ant/claude-native`, and
  `@ant/claude-swift` are private internal packages; the product is emitted
  from Anthropic's internal `apps` repository [bundle].
- **Claude Code's core is closed**: the desktop downloads the same versioned
  binary used by the public Agent SDK, but the `anthropics/claude-code` license
  is “All rights reserved” under Anthropic's Commercial Terms [public:
  [Claude Code license](https://github.com/anthropics/claude-code/blob/main/LICENSE.md)].
- **The orchestration wrapper is public**: the Claude Agent SDK repositories
  expose the typed API and subprocess adapter, while resolving/spawning the
  proprietary CLI [public + bundle].
- **The extension boundary is intentionally open**: MCP is an open protocol;
  MCPB/Desktop Extensions are zip archives with `manifest.json`, local MCP
  servers, dependencies, and stdio transport. Anthropic published the format
  and toolchain for other desktop apps to implement [public:
  [Desktop Extensions architecture](https://www.anthropic.com/engineering/desktop-extensions),
  [MCPB repository](https://github.com/modelcontextprotocol/mcpb)].
- **The hardware edge is open**: the BLE “Hardware Buddy” protocol and ESP32
  reference implementation are public even though the desktop bridge is not
  [public:
  [anthropics/claude-desktop-buddy](https://github.com/anthropics/claude-desktop-buddy)].

The split is almost the inverse of ChatGPT/Codex: OpenAI ships an open Rust
agent engine beneath a closed desktop and plugin layer; Anthropic ships a closed
agent engine beneath public SDK, MCPB, plugin, and hardware integration
boundaries.

## 10. SO WHAT for OpenAgents Desktop

Context: OpenAgents Desktop is Electron + Effect Native
(`apps/openagents-desktop`; Effect Native per `docs/effect-native/`, Sol master
roadmap §EN).

**1. Electron is now validated by both frontier desktop strategies.** OpenAI
uses an Electron-compatible Chromium fork; Anthropic gets Chat, Cowork, Code,
computer use, Chrome control, terminals, native file access, Bluetooth, and a
hardware VM from stock Electron. The question is not whether Electron can host
an agentic operating surface. The question is how disciplined its trust and
state boundaries are.

**2. Anthropic is the stronger argument against forking the runtime.** Claude
achieves the capabilities that supposedly justify Owl — browser integration,
computer use, native permissions, device bridges, and deep agent execution —
with upstream Electron plus small Swift/Rust modules. OpenAgents should stay on
stock Electron, take upstream Chromium security releases, and put exceptional
capabilities behind typed host services.

**3. Their best seam is the versioned CLI over stream JSON.** Claude Agent SDK
spawns Claude Code with explicit stdin/stdout JSON modes and stdio permission
callbacks. This validates the same core direction as Codex `app-server`: one
engine shared by CLI and desktop, a process boundary, no permanent localhost
port, resumable sessions, and a UI-independent event stream. For OpenAgents,
make that contract an Effect Schema package, version it independently, and test
replay/compatibility across engine releases.

**4. Their VM split is more important than their web stack.** The host agent
owns conversation, selected-folder file access, web fetch, and MCP; untrusted
code runs in a Linux guest under the platform hypervisor. That is a crisp
authority boundary. OpenAgents Desktop should preserve the same conceptual
split even when the first implementation is a lighter sandbox: host authority,
guest execution, explicit mounts, explicit egress, and receipts crossing the
boundary. Do not let “local agent” collapse into unrestricted renderer or main
process execution.

**5. Do not copy the live-site-as-main-renderer dependency.** Reusing
`claude.ai` gives Anthropic immediate web/desktop parity and lets the product
change without an app release. It also means the trusted desktop surface is
partly a remote deployment, with a second bundled SPA and routing/proxy logic to
keep the two coherent. OpenAgents should share renderer packages between web
and desktop, but ship a locally versioned renderer whose behavior can be tested
against the installed host protocol. Predictability and offline inspectability
are product advantages.

**6. Treat component updates as one compatibility ledger.** Claude has a
Squirrel shell feed, live Ion deploy, bundled Ion snapshot, Claude Code release
manifest, extension updates, and VM-rootfs manifest. OpenAgents will acquire the
same problem as agents, plugins, native helpers, and sandboxes separate. Define
one signed component manifest with minimum/maximum compatible protocol versions,
hashes, rollback rules, and a user-visible receipt before multiplying release
planes.

**7. Copy the concrete Electron hardening.** Disable RunAsNode,
`NODE_OPTIONS`, and inspect flags; enable cookie encryption, asar integrity, and
asar-only loading; keep renderers sandboxed; validate every privileged IPC
sender origin; use dedicated schemes and partitions for file/artifact previews;
sign/notarize every downloaded executable. Claude's file-protocol extra
privileges and unsandboxed host are reasons to make OpenAgents' exceptions
explicit, not defaults.

**8. MCPB compatibility is more valuable than a parallel package format.** The
app associates `.mcpb`, legacy `.dxt`, and `.skill`; ships a direct MCP host and
built-in Node execution; and exposes extensions, plugins, skills, connectors,
and hardware through one product. OpenAgents' typed catalog should ingest the
open MCPB format, add signatures/provenance/Effect schemas around it, and avoid
inventing an incompatible zip-with-manifest convention.

**9. Native capability modules should stay narrow.** Anthropic's Rust N-API
module and Swift bridge are large but conceptually bounded: OS integration,
permissions, capture, VM lifecycle, hardware, and update cleanup. Effect Native
should expose comparable capabilities as typed services with mock
implementations; the renderer should never import native addons or own TCC/VM
authority directly.

**10. Stack overlap is broad; architecture is the differentiator.** React 19,
Radix/Base UI, TanStack-style query/router layers, Zustand, Tailwind, Motion,
ProseMirror/Tiptap, CodeMirror, Monaco, xterm, Shiki, KaTeX, Mermaid, Vite,
Forge, Playwright, Oxc, and native TypeScript tooling are all conventional
choices. Claude's advantage comes from how those shelves are composed around a
real CLI, VM, MCP, and native boundary. OpenAgents' counter-position is one
typed Effect model spanning those boundaries, open engine/protocol authority,
and receipts rather than opaque product state.

## Sources

- Bundle/runtime evidence: `/Applications/Claude.app` as installed 2026-07-07
  (version `1.19367.0`), the staged `1.20186.0` update indicator, live process
  and sanitized accessibility state, and names-only listings of
  `~/Library/Application Support/Claude` and `~/.claude`.
- [Claude download — all desktop surfaces](https://claude.com/download)
- [Claude Code Desktop documentation](https://code.claude.com/docs/en/desktop)
- [Claude Cowork architecture overview](https://support.claude.com/en/articles/14479288-claude-cowork-architecture-overview)
- [Let Claude use your computer in Cowork](https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork)
- [Anthropic — Desktop Extensions architecture](https://www.anthropic.com/engineering/desktop-extensions)
- [modelcontextprotocol/mcpb](https://github.com/modelcontextprotocol/mcpb)
- [anthropics/claude-desktop-buddy](https://github.com/anthropics/claude-desktop-buddy)
- [anthropics/claude-code license](https://github.com/anthropics/claude-code/blob/main/LICENSE.md)
