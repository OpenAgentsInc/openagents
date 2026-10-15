# Process architecture

Strings, option keys, channel names, and file paths in this chapter are
observed in the shipped scripts. Control flow restated from those scripts is
derived. Nothing here was executed.

The desktop is one Electron main process, a set of renderer windows that are
not allowed to grow new privileges on their own, and a pool of utility
processes for terminals, indexing, MCP, and stall capture. Product work that
needs the machine — files, a virtual machine, computer use, Bluetooth — is
reached through those processes, not by giving the claude.ai page Node.

## Boot

`package.json` starts `.vite/build/index.pre.js`. That file sets the Sentry
release and installs a V8 compile cache, then loads `.vite/build/index.js`.
`index.js` is 638 bytes. It sets the same release id and requires
`index.chunk-CC6MOcaE.js` plus `index.chunk-BWk6pVUt.js` (6,983,168 bytes).
The second chunk is the main process.

The release id on every one of these files is the git commit
`ba51ab579cc4029765083ec9f9f0bd23d37a32ee`. A JSON object in the Buddy preload
records the same commit, `isNestBuild: false`, `buildType: "prod"`,
`commitTimestamp: 2026-10-13T14:04:25.000Z`, and `appVersion: 2.37337.1`.

When the main window's web contents are created, the main process passes two
extra arguments into the renderer:

- `--desktop-features=` followed by a JSON object from the main process
  feature snapshot
- a second argument produced by the same helper the quick-entry window uses
  for managed configuration (`--desktop-managed-config=` is the literal the
  Buddy and main-view preloads parse)

The main window is created hidden. It reports `window_ready_to_show` from
the shell preload and `main_view_ready_to_show` from the page. A
`chrome-error://` load is a named failure path. Redirects that look like a
Claude auth URL are withheld (`desktop_claude_auth_url_withheld`). The same
startup path names `staging.claude.ai` and `claude-ai.staging.ant.dev`.

Installing the `app://` protocol handler is allowed to fail; the log line is
`Failed to install app:// protocol handler`. Session-record retention is
initialized at ready and can fail independently of the window. Login-item
registration has its own failure log.

On Windows the same boot function still contains the Squirrel early-outs
`--squirrel-install`, `--squirrel-updated`, `--squirrel-uninstall`, and
`--squirrel-obsolete`. Uninstall tries to clear the jump list. A Windows app
job is checked (`desktop_windows_app_job_check`). Those branches are in the
macOS build's script. They are not a second macOS updater.

Updates that are already staged can quit and install after an idle countdown
(`stealth-update`). A `--stealth` dry run logs the decision and does not
quit. If the updater starts re-checking, the countdown retries. The about
window's `ancientJournal` channel is the same updater state, seen from the
renderer.

## Windows

Each window is a `BrowserWindow` plus a preload that is allowed to expose a
bridge only after a frame check. The HTML documents under `.vite/renderer/`
are the Electron shells. The claude.ai interface itself is the separate
`ion-dist` tree, loaded into the main view. See
[web and preview](web-and-preview.md).

| Window | Preload | Document or protocol | What the creation site says |
| --- | --- | --- | --- |
| Main shell | `mainWindow.js` | `main_window/index.html` | Hidden until `ready-to-show`. Carries the title-bar bridge. |
| Main view | `mainView.js` | The claude.ai document inside that shell | Receives `--desktop-features=` and the managed-config argument. An unexpected `/code` child window is logged. |
| Quick entry | `quickWindow.js` | `quick_window/quick-window.html` | Frameless, transparent, always on top, hidden from Mission Control, not resizable, not minimizable, not maximizable, not closable, not fullscreenable, `skipTaskbar`, `show: false`. `setWindowOpenHandler` returns deny. |
| About | `aboutWindow.js` | `about_window/about.html` | Title `About Claude`. `titleBarStyle: hiddenInset`. Help opens `https://support.anthropic.com`. |
| Find in page | `findInPage.js` | `find_in_page/find-in-page.html` | Overlay. Load failure is logged as `find-in-page overlay load failed`. |
| Design / PDF | `designWindow.js` | A loaded document, not one of the six shell HTML files | `setDesignWindowNavigationHandlers` must run before `createDesignWindow`. `will-navigate` and `will-redirect` deny. An unparseable target is rejected. `http:` and `https:` need an explicit allow. |
| Cowork artifact | `coworkArtifact.js` | `cowork-artifact:` | The protocol handler serves the document and the artifact libraries. |
| Page preview | `claudePagePreview.js` | Preview partitions listed below | Top frame exposes the artifact-pane bridge. Subframes only forward pointer and pointer-lock events. |
| Computer-use teach | `computerUseTeach.js` | No shell HTML in the archive | Creation site names `darwin`, `panel`, and `screen-saver` beside the preload. |
| Watch-record chooser | `watchRecordChooser.js` | A temp file `wr-chooser-…/index.html` | A second open is refused: `chooser already open`. |
| Local-exec consent | `localExecConsent.js` | `local_exec_consent/localExecConsent.html` | Bridge exists only for a top frame on `localhost` or `file:`. |
| Buddy | `buddy.js` | `buddy_window/buddy.html` | Pairing UI. The preload is not referenced by the same path string as the other windows inside the main chunk; the HTML and the preload both exist. |
| Custom third-party setup | The main-view preload, with a restriction | Setup window | `Custom3pHelperRun` is restricted to this window. Device-code sign-in asks the user to confirm the account before anything is saved. |

Quick entry's `requestSkooch(width, height)` moves the panel so the requested
size stays inside the work area of the display nearest the window, leaving a
margin. `requestDismiss` hides it. Both are ignored once the window is no
longer in the open state the handler checks.

The design window's PDF bridge is the world `omeletteDesktopPdf`.
`printFrameToPdf` forwards `origin`, `nonce`, `docTitle`, and `paper` on
`claude.designPdf / DesignPdf / printFrameToPdf`. Any throw or a missing
bridge resolves `{ok: false, error: "refused"}` or `"bridge unavailable"`.
The bridge is installed only for `localhost`, `127.0.0.1`, or an `https`
origin of `claude.ai`, `preview.claude.ai`, `claude.com`,
`preview.claude.com`, or a host ending in `.ant.dev`.

Artifact pop-ups are a separate guest window, not a new shell HTML file. The
creation site refuses to proceed when there is no adoptable guest
(`no_guest`). It tracks `closed_before_verdict`, `opener_mismatch`,
`undecided_backstop`, and `destroyed`. Navigation is denied for `about:`,
`data:`, and `blob:`, and a `will-attach-webview` is part of the same guard.
Fullscreen from an artifact pop-up is blocked. The gate's own state names are
`passed_through_host_initiated`, `passed_through`, `denied_uninspected`,
`denied_not_top_frame`, `denied_cap`, `no_click`, `allow`, `reveal`,
`credited`, `allow_late`, and `deny_late`.

## The IPC bus

Renderer calls are not free-form channel strings. A preload builds:

```text
$eipc_message$_39b0efa9-376d-4ecf-a0af-4cd97a3d0235_$_<namespace>_$_<Type>_$_<method>
```

The UUID is a constant in every preload that was read. The namespace is a
short prefix such as `claude.web` or `claude.internal.computerUse`. `Type` and
`method` are the service and the operation. `invoke` is the usual verb.
Events use `on` with the same spelling, and the preload returns an unsubscribe
function.

A second bus, from the packaged `@loc/electron-window` dependency, uses a
different UUID and only the `WindowManager` type:

```text
$eipc_message$_fcb91d11-972d-4fd6-b90b-72739699bff5_$_electron_window_$_WindowManager_$_<method>
```

Observed methods: `RegisterWindow`, `UnregisterWindow`, `UpdateWindow`,
`DestroyWindow`, `WindowAction`, `GetWindowState`.

Namespaces observed as prefixes:

| Prefix | Who exposes it |
| --- | --- |
| `claude.web` | Main view. Account, windows, files, sessions, Cowork, VM, plugins, updater. |
| `claude.settings` | Main view. Preferences, extensions, MCP config, third-party setup, support. |
| `claude.hybrid` | Main view, about, buddy, find, quick, main shell. `DesktopIntl` locale. |
| `claude.telemetry` | Main view. Renderer memory and print-call reports. |
| `claude.buddy` | Buddy window. Device pairing. |
| `claude.skills` | Main view. `Skills.previewSkillFile`. |
| `claude.simulator` | Main view. Simulator service. |
| `claude.officeAddin` | Main view. Office files and the Porter handoff. |
| `claude.engineSurface` | Main view. A pane/input surface with its own render and focus events. |
| `claude.coworkArtifact` | Artifact preload. |
| `claude.designPdf` | Design preload. |
| `claude.internal.ui` | Title-bar preloads. |
| `claude.internal.findInPage` | Find overlay. |
| `claude.internal.computerUse` | Teach, watch-record pill, watch-record chooser. |
| `claude.internal.localExecConsent` | Consent window. |
| `claude.internal.sableOtter` | Teach preload, and only for a top `file:` frame. |

The main view assigns services to worlds with `contextBridge.exposeInMainWorld`.
The world is the first path segment. The service is a property on that world.

| World | Services |
| --- | --- |
| `claude.settings` | `AppConfig`, `Startup`, `CcdCli`, `GlobalShortcut`, `MCP`, `Extensions`, `FilePickers`, `AppPreferences`, `AppFeatures`, `DesktopInfo`, `Custom3pSetup`, `Custom3pHelperRun`, `ClaudeAiImport`, `HistoryImport`, `SupportBundle`, `WakeScheduler`, `Custom3pUsage` |
| `claude.web` | `WindowControl`, `QuickEntry`, `Account`, `PlanUsage`, `WebBuild`, `Navigation`, `OsOpenHandlers`, `DeepLink`, `MenuEvents`, `ContentStampReporter`, `WindowState`, `DeviceStorage`, `Toast`, `Auth`, `DesktopNotifications`, `AutoUpdater`, `BrowserNavigation`, `FindInPageProvider`, `ManagedConfig`, `CoworkArtifacts`, `CoworkFilePreview`, `ClaudeCode`, `CliSessionImport`, `CloudMove`, `ChromeExtension`, `ComputerUseTcc`, `ComputerUseLivePreview`, `BuddyRemoteFeed`, `FramebufferPreview`, `LocalAgentModeSessions`, `CoworkStorage`, `CoworkUserFiles`, `CoworkMemory`, `Gorse`, `CoworkSpaces`, `AgentModeFeedback`, `CoworkFeedback`, `DeviceRegistry`, `Dictation`, `DocumentFunnel`, `FileSystem`, `Resources`, `ForgeState`, `GitState`, `GrandPrix`, `Launch`, `LocalSessions`, `LocalSessionEnvironment`, `LocalSessionPresets`, `LocalPlugins`, `PluginBridgeMcp`, `CustomPlugins`, `RemoteControlServing`, `CoworkScheduledTasks`, `CCDScheduledTasks`, `SessionPopouts`, `ViolinFret`, `ClaudeVM`, `WindowLayout` |
| `claude.simulator` | `Simulator` |
| `claude.skills` | `Skills` |
| `claude.officeAddin` | `OfficeAddinFiles` |
| `claude.engineSurface` | `EngineSurface` |
| `claude.hybrid` | `DesktopIntl` |
| `claude.buddy` | `Buddy` |

Separate `exposeInMainWorld` calls, not properties of those worlds:

- `electronWindowControl` and `electronIntl`, the same objects as
  `WindowControl` and `DesktopIntl`
- `claudeAppBindings`, `claudeAppSettings`, a reduced `process`
- `buddyBle` on the main view, which is the Nordic UART transport, not the
  Buddy window service
- `desktopBootFeatures`, `desktopManagedConfig`, `desktopEnterpriseConfig`,
  `desktopTelemetryConfig`, `desktopDeviceClass`, `desktopPreferredLanguages`
- `cowork` on the artifact preload
- `omeletteDesktopPdf` on the design preload
- `claudeDesktopArtifactPane` on the preview preload
- `desktopEssentialTelemetryDisabled` on the Buddy preload

`claudeAppBindings.registerBinding` accepts only `cmdK` and `googleAuthCode`.
Any other name throws `unsupported Claude App binding`. The same object lists
MCP servers, connects one, and opens MCP settings. Alongside the bridge, the
preload posts these `postMessage` events on a `MessageChannel` port, target
`*`: `list-mcp-servers`, `connect-to-mcp-server`, `mcp-server-connected`,
`request-open-mcp-settings`, `mcp-server-auto-reconnect`,
`cowork-session-folders-updated`, `cowork-session-directories-staged`.

`claudeAppSettings` is only `filePickers.getPathForFile`, bound to Electron's
`webUtils.getPathForFile`. The reduced `process` object exposes `arch`,
`platform`, `type`, and `versions`. `env` is `{CI}` when `CI` is set, and
otherwise empty. Buddy synthesizes `versions` from the build JSON instead of
forwarding the real environment.

Mouse buttons 3 and 4, on a trusted event, are history back and forward in
the main view and in a Cowork artifact. The artifact preload ignores clicks
that are not trusted, and it only calls `openExternalUrl` for an anchor whose
protocol is present and is not `cowork-artifact:`.

## Who may hold a bridge

A preload installs its world only when both checks pass.

The frame check: `webFrame.frameToken` equals `webFrame.top.frameToken` when
those fields exist, otherwise `routingId` equals the top frame's `routingId`.
Subframes do not get the service objects. The preview preload is the
exception that still does work in a subframe, and that work is pointer
forwarding, not a service bridge.

The origin check on the main view allows `https://claude.ai`,
`https://preview.claude.ai`, `https://claude.com`,
`https://preview.claude.com`, a host ending in `.ant.dev`, `localhost`, and
`file:`. Some call sites also allow `app://localhost` and `127.0.0.1`.
Local-exec consent allows only `localhost` or `file:`. Sable Otter's HUD
allows only a top `file:` frame. The design PDF bridge is the https list
above, plus `localhost` and `127.0.0.1`, and it does not use the frame-token
helper the other internal preloads use; it checks `window.location` once at
load.

`Custom3pHelperRun` is additionally restricted to the setup window even after
the origin check. The log line is `Custom3pHelperRun is restricted to the
setup window`.

## Utility processes and workers

The main process forks utility processes with `utilityProcess.fork(entry, [],
{serviceName, stdio: "pipe"})`. It opens a `MessageChannelMain`, waits for the
`spawn` event, and posts `{type: "init"}` with the second port. A reply that
the host treats as proof marks the child healthy. Exit before that proof, or
exit with a request in flight, rejects pending calls with `worker_exit`. A
kill uses `worker_killed`. Stdout and stderr are logged as
`UtilityProcess stdout` and `UtilityProcess stderr` unless the fork was asked
to be quiet.

Archive files that are those entry points, or workers beside them:

| File | Role derived from its own messages |
| --- | --- |
| `mcp-runtime/nodeHost.js` | Parent says `{type: "init", ports}`. The child imports an ES module as if it were `node`, wraps stdin, and refuses to spawn Electron itself. See [extensions](extensions-and-integrations.md). |
| `mcp-runtime/directMcpHost.js` | An MCP client session. The file speaks `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, and the matching notifications. It also contains the OAuth token-type `urn:ietf:params:oauth:token-type:id-jag`. |
| `pty-host/ptyHostWorker.js` | Messages `spawn`, `resize`, `data`, `exit`, `spawned`, `spawn-error`, `spawn-fallback`, `scrollback-read`. A spawn may name `scrollbackLogDir`. |
| `shell-path-worker/shellPathWorker.js` | One parent-port round trip. The reply is `{type: "envResult"}` or `{type: "error"}`. This is the login-shell environment capture. A CCD log says a failed `env -0` is split on newlines so those values are not handed to ssh. |
| `stall-sampler-worker/stallSamplerWorker.js` | A worker thread, not a utility process. It reads a `BigInt64Array` heartbeat with `Atomics` and, after `stallThresholdMs`, starts the V8 profiler. It posts `{type: "stall_capture", resumeSeq, frames, startLagMs}`. Captures are token-bucketed by `maxBurstCaptures` and `minCaptureIntervalMs`. |
| `file-index-worker/fileIndexWorker.js` | 895,160 bytes. Named by the file-index log tag. The body was not reconstructed operation by operation. |
| `heavy-work-worker/heavyWorkWorker.js` | 689,648 bytes, plus `dist-CI9lzb1v.js`. Named as the heavy-work worker. The body was not reconstructed operation by operation. |
| `transcript-search-worker/transcriptSearchWorker.js` | Named search worker for transcripts. |

The node host's parent protocol, read in full: stdout and stderr become
`{type: "stdout"|"stderr", content}`. A fatal error is
`{type: "fatal-error", kind, message, stack}` and then exit 1. Kinds are
`uncaughtException`, `unhandledRejection`, and `import-failed`. Stdin is a
fake stream fed by `{type: "stdin", data}` plus a newline. `argv` is rewritten
to `node` or `node.exe`, then the entry URL, then the caller's args.
`SIGTERM` and `SIGINT` exit 0.

## Schemes and partitions

`protocol.registerSchemesAsPrivileged` is called with at least these schemes.
A further array is spread into the same call and was not recovered as
literals.

| Scheme | Privileges |
| --- | --- |
| `cowork-artifact` | standard, secure, `supportFetchAPI` |
| `cowork-file` | standard, secure, `supportFetchAPI`, stream |
| `claude-media` | standard, secure, stream, `bypassCSP` |
| `claude-simulator` | standard, secure, stream, `bypassCSP`, `supportFetchAPI`, `corsEnabled` |

`protocol.handle` is installed for `app` and for `https`. The `https` handler
is the artifact and preview path that can answer a document itself; the
permission and account checks on that path are in
[Cowork](cowork.md) and [web and preview](web-and-preview.md).

Session partitions observed as literals:

- `persist:cowork-artifact`
- `persist:cowork-file-preview`
- `persist:launch-preview-`
- `persist:launch-preview-cowork-shared`
- `persist:launch-preview-session-`
- `persist:launch-preview-static`

`in-app-browser` is a partition name in the webauthn and preview code. It is
not spelled with the `persist:` prefix at the site that was read.

## Permission decisions

One session permission callback is the default for Claude-owned contents.
The failure logs are specific:

- `Blocked permission request from Claude-page preview`
- `Blocked fullscreen request from artifact pop-up window`
- `Blocked permission request from origin-less frame`
- `Blocked permission request`
- `Blocked permission check`

Allows that are visible in the same function: the main frame of a window the
process already tracks; a media request (`camera`, `microphone`,
`camera+microphone`) for a launch preview, with a denial event
`desktop_launch_preview_media_permission_denied`; a
`clipboard-sanitized-write` after a clipboard guard; webauthn only when the
requesting frame passes the frame gate. A webauthn ceremony whose frame fails
that gate is cancelled. If the platform returns more than one discoverable
credential and there is no picker UI, the ceremony is cancelled rather than
choosing one. The user-facing sheet name in that path is
`select-webauthn-account`.

The artifact-document session is stricter. Its permission request handler
always calls the callback with false, its permission check handler always
returns false, and `webRequest.onBeforeRequest` cancels any URL that does not
start with the artifact origin.

When `CLAUDE_DESKTOP_LOCAL_FRAME_SHELL` is set, a response-header hook rewrites
a `frame-ancestors` CSP directive away from `none` for the local frame shell.
That environment variable is a development switch in the shipped script, not
a default.

Model ids that sit in the main process next to the effort levels are
`sonnet`, `opus`, `haiku`, `fable`, and `mythos`. The effort names beside them
are `low`, `medium`, `high`, `xhigh`, and `max`, with the descriptions
"Fastest for quick answers", "Most efficient for everyday tasks", "Most
capable for ambitious work", and "For your toughest challenges". Those are
copy strings in the main chunk, not proof of which ids the signed-in account
is offered.
