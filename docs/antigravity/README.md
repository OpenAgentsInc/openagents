# Antigravity — Reference Documentation, Wrapper Audit & Desktop Teardown

## Directory Index

### OpenAgents Wrapper Audit & Complete Antigravity Universe Reference
| Document | Scope |
|---|---|
| **[`AUDIT.md`](./AUDIT.md)** | **Master forensic audit** of OpenAgents' iOS & macOS Antigravity wrapping stack against `#13993` & `#13992` — 7 live production breakages, 48 severity-ranked line-cited findings, buffer clash matrix, Google/T3 compliance boundary & `cloudcode-pa` token-extraction risk, Version Sync v2 blueprint, 7 strategic blind-spot registers, and phased remediation roadmap. |
| **[`cli-and-headless.md`](./cli-and-headless.md)** | Exhaustive `agy` CLI reference (verified against live `v1.5.14`): every global flag, subcommand, `agy acp` flag, headless `json`/`stream-json` contracts, exit codes `0–3` & `AGY_ERROR`, all `settings.json` / `admin_settings.json` keys, `statusLine`/`title` schemas, env vars, 50+ slash commands, and installer/updater. |
| **[`acp-and-endpoints.md`](./acp-and-endpoints.md)** | Live `agy acp` v1.5.14 JSON-RPC 2.0 wire reference (`initialize`, `session/list`, all methods & `session/update` variants, `_meta` extensions, `--process-per-session` router, `1.5.0–1.5.14` compatibility floor), every cloud/enterprise/local endpoint, and model/effort/Orchestrator/mode/auth matrices. |
| **[`sdk-and-ides.md`](./sdk-and-ides.md)** | Complete `google-antigravity` Python SDK (`v0.1.24`) reference (clients, local models, tools, policies, hooks, triggers, workflows, errors, OTel), 9 IDE extensions & binary sync mechanics, and 226+ release changelog velocity/drift analysis. |
| **[`platform-security-and-extensibility.md`](./platform-security-and-extensibility.md)** | Permissions & OS terminal sandbox, hooks (6 events + JSON contracts), MCP & MCP Apps (`ui://` SEP-1865), customization discovery hierarchy (skills/rules/agents/plugins/`AGY_ACCOUNT`/sidecars), orchestration (subagents/swarms/teamwork/boost/workflows/schedules/automations), Remote Control/Agents/SSH/Voice/Browser/Artifacts, and Enterprise/Quotas/ToS. |

### Antigravity Desktop Electron Teardown & MicroCoder Parity Notes
| Document | Scope |
|---|---|
| **[`README.md`](./README.md)** *(below)* | Structural teardown of `/Applications/Antigravity.app` (Electron 44.9.1 bundle, ASAR integrity, sidecars, IPC). |
| **[`modules.md`](./modules.md)** | Module-by-module inventory of `app.asar/dist/`. |
| **[`inference-and-agent-loop.md`](./inference-and-agent-loop.md)** | Language Server inference and agent execution loop analysis. |
| **[`gap_analysis.md`](./gap_analysis.md)** | Earlier Electron App vs. MicroCoder parity gap analysis. |

---

# Antigravity — Electron application teardown

A structural analysis of the Google **Antigravity** desktop application installed on this
machine (`/Applications/Antigravity.app`). Written from the shipped bundle only: the app
is closed-source, so everything below is read out of the packaging, the ASAR archive and
the JavaScript that Electron actually runs.

Companion: [`modules.md`](./modules.md) — a module-by-module reference of `dist/`.

---

## 1. Identity and packaging

| Property | Value | Source |
| --- | --- | --- |
| Display name | Antigravity | `Contents/Info.plist` |
| Bundle identifier | `com.google.antigravity` | `Contents/Info.plist` |
| Version | `2.34.0` (build `2.34.0`) | `Info.plist`, `app.asar/package.json` |
| Publisher | Google (`antigravity-support@google.com`) | `app.asar/package.json` |
| Homepage | <https://antigravity.google> | `app.asar/package.json` |
| Category | `public.app-category.developer-tools` | `Info.plist` |
| Minimum macOS | 13.0 | `LSMinimumSystemVersion` |
| URL scheme | `antigravity://` | `CFBundleURLTypes` |
| Electron runtime | **44.9.1** | `Electron Framework.framework/.../Info.plist` |
| Main entry | `dist/main.js` | `app.asar/package.json` |
| ASAR size | 6.7 MB on disk / 20.7 MB unpacked (1,166 files) | measured |

`Info.plist` also pins **`ElectronAsarIntegrity`** — SHA-256 hashes for `app.asar` and
`default_app.asar`. Electron refuses to load an archive whose hash does not match, so the
payload is tamper-evident without relying solely on the code signature.

The app declares `NSAllowsArbitraryLoads` plus local-network exceptions for
`localhost` and `127.0.0.1`, which is what an app needs when the renderer talks to a
local companion server over plain HTTP.

### Bundle layout

```
Antigravity.app/Contents/
├── MacOS/Antigravity                     # native launcher (Electron main process)
├── Info.plist                            # identity, URL scheme, asar integrity hashes
├── CodeResources, _CodeSignature/        # Apple code signature
├── Frameworks/
│   ├── Electron Framework.framework      # Electron/Chromium 44.9.1
│   ├── Antigravity Helper.app            # renderer/utility helper processes
│   ├── Antigravity Helper (GPU).app
│   ├── Antigravity Helper (Renderer).app
│   ├── Antigravity Helper (Plugin).app   # PPAPI-style plugin host
│   ├── Mantle.framework, ReactiveObjC.framework, Squirrel.framework
│   └── …                                 # macOS auto-update (Squirrel) stack
└── Resources/
    ├── app.asar                          # the entire JS application (6.7 MB)
    ├── app.asar.unpacked/node_modules/   # chrome-devtools-mcp (must stay on disk)
    ├── app-update.yml                    # electron-updater feed config
    ├── bin/                              # native sidecars (see §6)
    ├── default_app.asar                  # stock Electron fallback app
    └── *.lproj/                          # ~60 locales
```

The presence of the Squirrel/Mantle/ReactiveObjC frameworks alongside
`electron-updater` means the app carries **two update paths**: Squirrel for the macOS
shell and `electron-updater` for the JS payload.

---

## 2. Process architecture

Classic Electron three-tier model, with an unusually large main process.

```
┌────────────────────────────── main process (Node.js) ──────────────────────────────┐
│  dist/main.js            app lifecycle, window creation, protocol registration      │
│  dist/ipcHandlers.js     IPC surface (window/storage/shell/notification/mic)        │
│  dist/languageServer.js  spawns + supervises language_server sidecar                │
│  dist/remote/*           SSH / remote-host backends (34 modules)                    │
│  dist/computerActuator/* desktop automation + script mediation (12 modules)         │
│  dist/browserStream/*    CDP-driven browser streaming                              │
│  dist/guest*             <webview>/guest WebContents policy, dialogs, network       │
│  dist/wcv*               WebContentsView compositing + z-order                     │
└───────────────┬─────────────────────────────────────────┬──────────────────────────┘
                │ contextBridge (preload.js)              │ child_process / stdio
        ┌───────▼────────┐                        ┌───────▼─────────────┐
        │  Renderer      │                        │  Native sidecars    │
        │  (Hub UI)      │                        │  language_server    │
        │  + <webview>s  │                        │  agy-desktop-helper │
        └────────────────┘                        │  cable_ble_scanner  │
                                                  └─────────────────────┘
```

### Boot sequence (`dist/main.js`)

1. **Command-line switches** are appended before `app.whenReady()`:
   `--disable-blink-features=AutomationControlled`, `--disable-features=FedCm`, plus a
   computed list that always contains `CalculateNativeWinOcclusion`, and
   `--disable-backgrounding-occluded-windows`. In headless/WSL mode it also appends
   `--ozone-platform=headless`, `--headless`, `--disable-gpu`, `--no-sandbox`.
2. **Single-instance lock** — a second launch is redirected to the first through
   `app.on('second-instance')`.
3. **Custom schemes** are registered as privileged *before* `ready`.
4. `app.whenReady()` → `registerCustomSchemeHandlers()`, session WebAuthn configuration
   (`configureSessionWebAuthn(session.defaultSession)`), BeyondCorp PAC proxy
   pre-configuration for known `<webview>` partitions, then `createWindow(entryUrl)`.
5. **Lifecycle** is fully handled: `window-all-closed` (quits unless the “run in
   background” setting is on, which defaults to true only on macOS), `before-quit`
   (graceful shutdown, `session.closeAllConnections()`, sidecar kill), `will-quit`,
   `activate` (re-create window on dock click), and `open-url` for the
   `antigravity://` scheme.
6. **Global listeners** on `session-created` and `web-contents-created` attach policy to
   every new session and WebContents — the app does not trust per-window setup to be
   complete.

---

## 3. Renderer and preload security model

`dist/preload.js` is described in its own header as running “in every BrowserWindow
before the page loads” and exposing “a minimal, secure API via contextBridge … without
nodeIntegration.” The observable posture:

- **`contextIsolation` on, `nodeIntegration` off** — the renderer only sees the objects
  the preload explicitly publishes.
- **Narrow, namespaced bridge objects** rather than a generic `invoke`: `updaterAPI`
  (`onStateChanged`, `applyUpdate`, `quitAndInstall`, `checkForUpdates`, `getState`),
  `dialogAPI` (`showOpenDialog`, `showOpenMultipleFolderDialog`), `notificationAPI`,
  a device-chooser API with subscriber counting, and WCV (WebContentsView) probe/cursor
  channels.
- **Subscription discipline** — event bridges return an unsubscribe function and some
  track subscriber counts, so listeners are removed when the renderer goes away.
- **Guest policy modules** in the main process (`guestFramePolicy`, `guestPermissionPolicy`,
  `guestNetworkSanitizer`, `guestPromptController`, `webviewPopupSecurity`,
  `webviewUploadValidation`, `webviewSecurityState`, `guestCrossSiteCredentials`) enforce
  what embedded `<webview>` guests may do. Security is centralized in main, not delegated
  to page code.

---

## 4. Privileged custom schemes

`dist/customScheme.js` registers three schemes via
`protocol.registerSchemesAsPrivileged`, each with the minimum privileges it needs:

| Scheme | Privileges | Purpose |
| --- | --- | --- |
| `plugin://` | standard, secure, fetch, CORS, service workers, code cache | Fronts a localhost extension server; authority = hash(extension id + port + project id) |
| `agy-favicon://` | standard, secure only | Serves cached favicon bytes to the Hub renderer’s `<img>` tags; no fetch/CORS/SW |
| `agy-browser-stream://` | standard, secure only | Serves the isolated capture page/script in its own partition; never bound on `defaultSession` |

The comment in `handleFaviconProtocolRequest` is explicit that the favicon scheme
**never fetches** — unknown tokens and all non-`/` paths return 404, and non-GET returns
405. That is a deliberate SSRF guard.

---

## 5. IPC surface

`dist/ipcHandlers.js` (1,124 lines, the second-largest root module) registers a small,
tightly scoped set of `ipcMain.handle` channels:

```
window:close            window:minimize          window:maximize
window:unmaximize       window:is-maximized      window:set-title-bar-overlay
window:show-context-menu  window:close-context-menu  window:toggle-devtools
window:zoom-in          window:zoom-out          window:reset-zoom
storage:get-items       storage:update-items
shell:open-external     notification:send        notification:open-system-preferences
mic:get-access-status   mic:open-system-preferences
agent:update-active-count   extensions:send-authorities
```

Notable design choices:

- **No generic `ipc:invoke`** — every capability has its own named channel and handler.
- **`shell:open-external`** is mediated in main rather than handed to the renderer.
- **`mic:*`** channels are separate from generic permissions, so microphone access is
  gated and can deep-link into System Settings.
- **`storage:*`** exposes the app’s settings store (see §7) rather than allowing filesystem
  writes from the renderer.
- Much of the heavier traffic does **not** use `ipcMain.handle` at all — the browser-stream,
  computer-actuator, desktop-host and remote modules use **Connect/gRPC over a local
  transport** (see §8).

---

## 6. Native sidecars (`Contents/Resources/bin`)

| Binary | Size | Role |
| --- | --- | --- |
| `language_server` | **196.9 MB** | The real backend: language services, agent runtime, HTTP/HTTPS server |
| `agy-desktop-helper` | 1.3 MB | Native desktop helper (window/input/screen bridging) |
| `agy-desktop-script` | 129 KB | Script executor used by the computer-actuator |
| `cable_ble_scanner` | 115 KB | BLE scanner for the “cable” device-pairing flow |

The language server dominates the bundle. `dist/languageServer.js` spawns it, parses its
stdout with the regex `/listening on \w+ port at (\d+) for HTTP(S)?/i` to learn the port,
mints a bootstrap token, monitors for crashes, supports a **stamp/self-restart** protocol
(exit code constant for “immediate self-restart”), and can trust a local TLS certificate
(`setupLocalCertTrust`) — i.e. the renderer talks to it over HTTPS on loopback.

---

## 7. State, settings and storage

- `dist/storage.js` provides a `StorageManager` with `getItems`/`updateItems` and a
  `onDidChange` event, surfaced over `storage:get-items` / `storage:update-items`.
- `dist/services/settingsService.js` layers typed settings on top:
  `runInBackground` (default: `process.platform === 'darwin'`), `keepComputerAwake`
  (default `false`), `autoCheckForUpdates` (default `true`). Side effects are applied
  reactively via `onDidChange` — e.g. `keepComputerAwake` drives a `SleepBlocker`
  singleton.
- Window geometry persistence lives in `dist/windowState.js`; the Hub remembers window
  state across launches.
- `dist/paths.js` centralizes on-disk locations, and `dist/loginShellEnv.js`,
  `loginShellEnvReads.js`, `loginShellPolicy.js` read the user’s login-shell environment
  once and reuse it — necessary so spawned sidecars see the same `PATH` a terminal would.

---

## 8. The interesting subsystems

> **Inference, the agent loop and tool calls have their own page:**
> [`inference-and-agent-loop.md`](./inference-and-agent-loop.md) — how a prompt reaches
> Gemini (Cloud Code `v1internal:streamGenerateContent` vs `generativelanguage`), why the
> loop lives inside the Go `language_server` rather than the Electron JS, the
> `CortexStep` / `ToolCallStep` wire format, the planner/executor split, the
> `EXECUTOR_TERMINATION_REASON_*` contract, the tool vocabulary, and the Actuator
> round-trip that lets the Go loop drive the host's `computer_use` and `browser_*` tools.

### 8.1 Browser engine and streaming (`browserEngine/`, `browserStream/`)

`browserEngine.js` discovers a local Chrome, connects over a **CDP pipe**
(`cdpPipe.js`, `chromeDiscovery.js`) and drives it. `browserStream/` then streams that
browser into an in-app view: `captureWindow.js`, `capturePage.js`, `capturePreload.js`,
`browserViewerHandler.js` (41 KB), `browserViewerTabDriver.js`, `browserViewerTabSnapshot.js`
and `browserViewerUrlPolicy.js`. There is a dedicated URL policy module, so navigation
inside the streamed browser is filtered in main.

### 8.2 Desktop host (`desktopHost/`, 176 KB)

A WebRTC-based remote-desktop host: `rtcSender.js` (27 KB), `rtcTransport.js`,
`frames.js`, `host.js`, `previewManager.js` (29 KB), `pairing.js`, `takeovers.js`,
`protocol.js`, `activityThrottle.js`, `scriptSelftest.js`. It can pair a peer, send
frames, hand over control (“takeover”) and throttle activity — the plumbing for letting
another agent or device drive this Mac.

### 8.3 Computer control (`computer/`, `computerActuator/`)

- `computer/` — OS-level integration: `macComputerDriver.js`, `desktopHelper.js`,
  `permissions.js` / `permissionsIpc.js` (TCC prompts), `hud.js` / `hudAgents.js` /
  `overlay.js` (on-screen agent HUD), `screenLock.js`, `previewIpc.js`.
- `computerActuator/` — the RPC-facing layer: `computerActuatorHandler.js` (114 KB, the
  single largest JS module), `computerOps.js`, `scriptMediator.js` and `scriptRunner.js`
  (which pair with the `agy-desktop-script` sidecar and `dist/scriptJail.js` /
  `scriptJailPreload.js` to sandbox executed scripts), and a `driver/` abstraction
  (`computerDriver`, `backgroundDriver`, `fakeComputerDriver` for tests).

### 8.4 Remote development (`remote/`, 476 KB, 34 modules)

Full SSH story: `sshConnection.js`, `sshBridge.js`, `sshAskpass.js`, `sshConfigHosts.js`,
`sshPrompts.js` / `sshPromptPreload.js`, `sshRecentHosts.js`, `sshWindowRestore.js`,
`transport.js`, `remoteManager.js`, `remoteSidecars.js`, `serverArtifacts.js`,
`serverVersion.js`, `bootstrapScript.js` / `bootstrapProtocol.js`, plus sign-in plumbing
(`signInUrls.js`, `signInCallback.js`, `signInReturn.js`) and `localNetwork.js`,
`privateDir.js`, `remoteWindowSecurity.js`. This is the machinery that installs and runs
the language server on a remote host.

### 8.5 Guest/WebContents policy

Large, security-focused modules that police embedded content:
`guestDialogController.js` (39 KB), `guestFrameBridge.js` (23 KB),
`guestFileAttachment.js` (22 KB), `guestFileChooserController.js` (17 KB),
`guestNetworkController.js` (17 KB), `guestNetworkSanitizer.js` (16 KB),
`guestWebMcpController.js` (18 KB — **WebMCP**), `guestCrossSiteCredentials.js` (14 KB),
`guestFramePolicy.js` (14 KB) and `webviewPopupSecurity.js`.

---

## 9. Networking and RPC

Dependencies in `package.json` tell most of the story:

- **`@connectrpc/connect`, `@connectrpc/connect-node`, `@bufbuild/protobuf`** — the app
  speaks **Connect (gRPC-compatible) with protobuf** to its sidecars. Generated stubs live
  in `dist/proto/`: `actuator_pb.js`, `browser_viewer_pb.js`, `computer_payload_pb.js`,
  `computer_viewer_pb.js`, `host_bridge_pb.js`.
- **`dist/actuator/`** — `mainClient.js`, `nodeTransport.js`, `pinnedLsCert.js` (certificate
  pinning for the language-server connection), `testCerts.js`.
- **`dist/hostBridgeServer.js`** — a local server that bridges hosts.
- **`dist/beyondcorpPac.js`** — BeyondCorp PAC-proxy configuration, which is why
  `session-created` pre-configures proxies on `<webview>` partitions.
- **`dist/webauthn.js`** — per-session WebAuthn configuration.
- **`dist/isolatedWorldExec.js`** — script execution isolated from the page’s world.

---

## 10. Updates and distribution

`Contents/Resources/app-update.yml`:

```yaml
provider: generic
url: https://antigravity-hub-auto-updater-974169037036.us-central1.run.app/manifest/
updaterCacheDirName: com.google.antigravity
```

A **generic** provider pointed at a Google Cloud Run host. `dist/updater.js` wraps
`electron-updater`; the renderer only sees the `updaterAPI` bridge in `preload.js`
(state subscription, apply, quit-and-install, check, get-state). Update state is pushed
over `updater:state-changed`. The Squirrel frameworks in `Frameworks/` handle the native
shell’s own update path, and `main.js` reacts to `before-quit-for-update` by freezing SSH
windows so a restart does not lose remote sessions.

---

## 11. Testing hooks shipped in production

`dist/__mocks__/` contains `electron.js` (14 KB), `electron-updater.js` and
`electron-log/main.js`, and `dist/test/` exists alongside `dist/computerActuator/fakeComputerDriver.js`.
The build ships its test doubles. This is common in Electron apps compiled with
`tsc`/esbuild where `__mocks__` sits next to source, but it does mean the production
archive carries a fake Electron implementation and a fake computer driver.

---

## 12. Observations

**Strengths**

- Security is centralized: one preload, narrow named IPC channels, three privileged
  schemes each with least-privilege flags, URL policies for streamed browsers, and
  dedicated guest/WebContents policy modules rather than ad-hoc checks.
- Certificate pinning (`pinnedLsCert.js`) and a bootstrap token for the loopback language
  server, plus a self-restart protocol, show the sidecar link was designed adversarially.
- `ElectronAsarIntegrity` pins the payload hash at the OS level.
- Process-wide hooks (`session-created`, `web-contents-created`) mean new windows cannot
  silently skip policy.

**Costs / risks**

- The main process is very large: `computerActuatorHandler.js` alone is 114 KB,
  `cablev2.js` 82 KB, `wcvBrowserRegistry.js` 96 KB, and `remote/` is 476 KB across 34
  files. A single `main.js` failure mode can take down remote, automation and browser
  features at once.
- The renderer is opaque: the actual Hub UI is fetched from a remote origin (the app
  opens `entryUrl`, not a bundled `index.html`), so the shipped bundle contains the
  *host*, not the *UI*. `default_app.asar` and `__mocks__` add surface area.
- `NSAllowsArbitraryLoads = 1` is broad; the localhost exceptions alone would usually
  suffice.
- Debug switches (`--disable-blink-features=AutomationControlled`) are present in the
  shipping binary, which is worth knowing when reasoning about bot-detection behaviour.
- Two parallel update mechanisms (Squirrel + electron-updater) mean two failure modes to
  reason about during rollout.

---

## 13. How this was produced

Read-only inspection of the installed bundle:

```sh
# identity
/usr/libexec/PlistBuddy -c 'Print' /Applications/Antigravity.app/Contents/Info.plist
cat '/Applications/Antigravity.app/Contents/Frameworks/Electron Framework.framework/Versions/A/Resources/Info.plist'
cat /Applications/Antigravity.app/Contents/Resources/app-update.yml

# ASAR listing and extraction (no external deps; parses the archive header directly)
node -e '…read app.asar header, walk files, write to /tmp/ag…'

# targeted reading
grep -n 'whenReady\|new BrowserWindow\|registerSchemesAsPrivileged' /tmp/ag/dist/main.js
grep -o "ipcMain.handle('[^']*'" /tmp/ag/dist/ipcHandlers.js
du -sh /tmp/ag/dist/* | sort -h
```

No application code was executed and the app was not launched during the analysis.
