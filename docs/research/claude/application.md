# Application

The packaged product is `@ant/desktop` version `2.37337.1`, private, product
name Claude, description "Desktop application for Claude.ai", author
Anthropic PBC. `package.json` is 5,036 bytes inside the archive. Its `main`
is `.vite/build/index.pre.js`. `engines.node` is `>=22.0.0`.

The archive header lists **503** entries. Uncompressed sizes sum to
**139,011,220** bytes. Top-level members: `.vite` (428 files), `node_modules`
(51), `resources` (17), `compile-cache` (6), and `package.json`. Ten entries
are marked unpacked. Those ten are also the entries whose archive integrity
metadata does not match the bytes observed for them. See the table at the
end of this chapter.

## JavaScript coverage

A static parse of the archive JavaScript produced these counts. Graph
coverage is partial. Semantic expansion stopped while large hashed chunks
were still outstanding. Those files kept static or module facts, or were
not semantically started. The graph is not a complete call graph.

| Fact | Value |
| --- | --- |
| Relevant files | 445 |
| Parsed JavaScript files | 426 |
| Parse failures | 0 |
| Invalid UTF-8 files | 0 |
| Text bytes read | 40,483,669 |
| AST nodes visited | 9,502,839 |
| Findings | 6,905 |
| Modules | 78 |
| Browser windows | 14 |
| Explicit web preferences | 37 |
| Preload entrypoints | 0 |
| Context-bridge APIs | 52 |
| Exposed API members | 37 |
| IPC operations | 1,525 |
| Literal channels | 10 |
| Dynamic channel operations | 1,514 |
| Renderer transmissions | 1,292 |
| Renderer listeners | 230 |
| Main handlers | 3 |
| Paired renderer transmissions | 1 |
| Unpaired literal renderer transmissions | 8 |
| Sender-validation observations | 63 |
| Utility processes | 4 |
| Resolved utility entrypoints | 0 |
| Native-addon bindings | 0 |

The retained projection has 2,347 nodes and 2,855 edges. Semantic totals
recorded beside that projection are 262,663 nodes and 282,282 relations; the
smaller list is what fit after semantic expansion stopped. The extract
records 788 analysis limitations, 377 of them from that stop. The
heavy-work worker is named in that list.

Node kinds in the retained graph: 768 unknown, 445 asar-entry, 425
javascript-asset, 390 javascript-module, 114 ipc-channel, 64 endpoint, 43
source-map, 37 ipc-handler, 19 context-bridge-api, 14 artifact, 6
native-addon, 6 electron-renderer, 6 package, 5 javascript-expression, 3
worker, 1 javascript-chunk, 1 electron-main.

Edge relations: 1,368 contains, 667 imports, 496 maps_to, 141 exposes, 73
invokes, 59 calls, 37 handles, 10 loads, 4 sends.

`browser-window` is not itself a node kind here. The 14 windows are a
summary count. Six HTML documents were recovered as renderer nodes.

## Declared packages and what actually ships

Declared `dependencies`:

| Package | Range |
| --- | --- |
| `@ant/claude-for-chrome-mcp` | `*` |
| `@ant/claude-native` | `*` |
| `@ant/claude-swift` | `*` |
| `@ant/computer-use-mcp` | `*` |
| `@ant/imagine-server` | `*` |
| `@loc/electron-window` | `1.1.0` |
| `https-proxy-agent` | `^7.0.6` |
| `ws` | `^8.18.0` |

Optional dependency: `node-pty` `1.2.0-beta.14`.

Package trees present under `node_modules`, from each package's own
manifest:

| Name | Version | Main |
| --- | --- | --- |
| `@ant/claude-swift` | `1.0.0` | `js/index.js` |
| `@ant/claude-native` | `0.0.0` | `index.js` |
| `node-pty` | `1.2.0-beta.14` | `./lib/index.js` |
| `ws` | `8.21.0` | `index.js` |
| `agent-base` | `7.1.4` | `./dist/index.js` |

`@ant/claude-for-chrome-mcp`, `@ant/computer-use-mcp`, `@ant/imagine-server`,
`@loc/electron-window`, and `https-proxy-agent` are declared and have no
package directory in the archive. The Vite build is the remaining place they
could live. That bundling was not traced file by file.

`devDependencies` are manifest strings in the shipped `package.json`. They
are not proof those packages run. Names and versions that identify product
surface area:

- `@anthropic-ai/claude-agent-sdk` `0.3.295`
- `@anthropic-ai/mcpb` `2.1.2`
- `@anthropic-ai/sdk` `catalog:`
- `@modelcontextprotocol/client`, `@modelcontextprotocol/core`, and
  `@modelcontextprotocol/sdk`, each `catalog:`
- `@ant/chrome-native-host` `*`, `@ant/claude-ssh` `*`,
  `@ant/app-cu-helper` `workspace:*`, `@ant/disclaimer` `*`,
  `@ant/cowork-win32-service` `*`, `@ant/ssh-askpass` `*`,
  `@ant/rfb-client` `*`, `@ant/clawd-quest` `workspace:*`,
  `@ant/session-halo-helper` `workspace:*`
- `electron` `catalog:`, `@electron/fuses` `^2.1.3`, `@sentry/electron` `^7.12.0`
- `@electron-forge/*` `8.0.0-alpha.10`
- `playwright-core` `1.57.0`, `ssh2` `^1.16.0`

`@ant/cowork-win32-service` is a Windows service name in a macOS build's
manifest. The macOS bundle does not contain that service.

## Windows and processes

The main-process node is the package entrypoint
`.vite/build/index.pre.js`.

HTML renderer documents:

- `.vite/renderer/main_window/index.html`, whose assets include
  `MainWindowPage-*.js` and `clawdQuestEngine-*.js`
- `.vite/renderer/buddy_window/buddy.html`
- `.vite/renderer/about_window/about.html`
- `.vite/renderer/find_in_page/find-in-page.html`
- `.vite/renderer/quick_window/quick-window.html`
- `.vite/renderer/local_exec_consent/localExecConsent.html`

Main-build modules whose names match windows or hosts. These are archive
paths. Each name is not, by itself, proof of a `BrowserWindow` constructor:

`aboutWindow.js`, `buddy.js`, `claudePagePreview.js`,
`computerUseTeach.js`, `computerUseWatchRecord.js`, `coworkArtifact.js`,
`designWindow.js`, `findInPage.js`, `localExecConsent.js`, `mainView.js`,
`mainWindow.js`, `quickWindow.js`, `watchRecordChooser.js`, `index.js`,
`index.pre.js`.

Worker-named files:

- `.vite/build/file-index-worker/fileIndexWorker.js`
- `.vite/build/heavy-work-worker/heavyWorkWorker.js`
- `.vite/build/pty-host/ptyHostWorker.js`
- `.vite/build/shell-path-worker/shellPathWorker.js`
- `.vite/build/stall-sampler-worker/stallSamplerWorker.js`
- `.vite/build/transcript-search-worker/transcriptSearchWorker.js`
- `.vite/build/mcp-runtime/directMcpHost.js`
- `.vite/build/mcp-runtime/nodeHost.js`

The three worker nodes in the graph are dynamic constructors: `new Worker(f)`,
`new Worker(n)`, and `new Worker(t, {type: "module"})`. Their labels are
empty. The summary counted 4 utility processes and 0 resolved entrypoints.
The file names above were not joined to those constructors.

`compile-cache/` holds six `.jsc` files, arm64 and x64, for `index.js` and
two chunks. They were not decoded. One webpack bootstrap chunk was
recovered (`webpack-bootstrap@481603`). Most of `.vite/build` is
content-hashed `index.chunk-*.js` plus source maps (43 source-map nodes).

## Context bridge

Nineteen context-bridge nodes were retained, all in world `main`. The
summary count is 52. The difference sits in the truncated semantic analysis.
Preload entrypoints recovered: 0.

Literal keys with object members:

| Key | Members |
| --- | --- |
| `buddy` | `getPathForFile` |
| `cowork` | `askClaude`, `callMcpTool`, `runScheduledTask` |
| `claudeDesktopArtifactPane` | `chrome`, `embedded`, `platform`, plus one member whose name was not recovered |
| `omeletteDesktopPdf` | `printFrameToPdf` |

`desktopEssentialTelemetryDisabled` and `process` appear as dynamic keys.
Several further bridges survive only as the expression `t`.

## IPC

Of 114 retained channel nodes, 113 are dynamic concatenations and one is a
literal. The summary's 10 literal channels are therefore not all present as
nodes. The literal node is a renderer invoke:

`$eipc_message$_39b0efa9-376d-4ecf-a0af-4cd97a3d0235_$_claude.designPdf_$_DesignPdf_$_printFrameToPdf`

The other retained channels are a minified prefix (`n`, `d`, or `s`) plus a
literal suffix. They are renderer invokes unless noted. The suffix is not
the full channel string.

- DesktopIntl: `getInitialLocale`, `requestLocaleChange`
- AboutWindow: `openHelp`, `getBuildProps`, `ancientJournal`, `getSupport`, `getAppName`
- QuickWindow: `requestSkooch`, `requestDismiss`
- MainWindowTitleBar: `requestMainMenuPopup`, `requestRestartApp`, `requestReloadMainView`, `titleBarReady`
- FindInPage: `findInPage`, `stopFindInPage`, `endFindSession`, and `density_$store$_getState` (also a `send-sync` variant)
- Buddy: `deviceStatus`, `pickDevice`, `install`, `status`, `pickFolder`, `scanDevices`, `pairDevice`, `preview`, `submitPin`, `setName`, `forgetDevice`, `cancelScan`
- CuTeach: `mouseEnter`, `mouseLeave`, `exit`, `next`
- CuWatchRecordChooser: `choose`, `setMicPreview`, `stopMicPreview`, `reportContentShortfall`
- CuWatchRecordPill: `toggleMic`, `done`, `discard`, `reportProcessingWidth`
- SableOtterHud: `button`, `paste`, `escape`, `submit`, `resize`, `fieldChanged`, `settings`, `tab`, `removePill`
- CoworkArtifactBridge: `callMcpTool`, `runScheduledTask`, `openExternalUrl`, `navigateHost`, `setEditableFocused`, `askClaude`

The 37 handler nodes in the extract are renderer `on` listeners whose
channel expression is a single identifier (`i`, `r`, or `n`). The summary's
3 main-process handlers were not recovered under names.

`CoworkArtifactBridge` suffixes match the `cowork` context-bridge members
`askClaude`, `callMcpTool`, and `runScheduledTask`. That is a reading of the
two name lists. The summary counted one paired renderer transmission.

Buddy's suffixes are a device lifecycle: scan, pick, pair, PIN, name,
preview, install, forget. SableOtterHud's suffixes are a composer overlay:
field changes, pills, paste, submit, resize. CuTeach and CuWatchRecord are
the computer-use teaching and recording chooser. About's `ancientJournal`
and `getBuildProps` are build and support surfaces. None of these were
executed.

## Endpoint-shaped strings

Sixty-four endpoint nodes were retained. Some were classified from a
`path` property and some from a call such as `.get` or `.open`. A route or
call candidate is not an observed host. Many `.get` hits are ordinary
property reads (`aria-label`, `Content-Length`, pdf.js strings, `tokens`).

Route-shaped names that match product settings rather than URL paths:

`sandbox.credentials`, `sandbox.credentials.sigv4`,
`sandbox.credentials.allowPlaintextInject`, `sandbox.credentials.awsPairs`,
`availableModels`, `deniedModels`, `modelPricing`, `strictKnownMarketplaces`,
`blockedMarketplaces`, `hooks`, `policyHelper`, `policyHelpers`,
`policySettings`, `forceLoginOrgUUID`, `enabledPlugins`,
`deniedMcpServers`, `allowedMcpServers`, `strictPluginOnlyCustomization`,
`gatewayInternalNetworks`, `wslInheritsWindowsSettings`.

Call-shaped names that remain property or method names: `workspace`, `vm`,
`hostFacts`, `pendingDaemonUpgrades`, `coworkArchive`, `ppkConvert`,
`proxyusefdpass`, `e2ePlaintextTokens`, and Sentry fields `sentry.dsc`,
`sentry.sample_rand`, `sentry.sample_rate`. No hostname was established
from these nodes.

`sandbox.credentials.allowPlaintextInject` and `e2ePlaintextTokens` are
names only. This pass did not find an implementation that injects
credentials, and it did not find a token value.

## Packaged resources

| Path | Unpacked size | Lives outside the archive payload |
| --- | --- | --- |
| `resources/artifact-libs/npm_mermaid@11.17.2_dist_mermaid.min.js` | 3,572,661 | no |
| `resources/artifact-libs/npm_chart.js@4.5.0_dist_chart.umd.js` | 208,337 | no |
| `resources/artifact-libs/npm_gridjs@5.0.2_dist_gridjs.umd.js` | 60,552 | no |
| `resources/bundled-skills/pptx.skill` | 173,007 | no |
| `resources/bundled-skills/docx.skill` | 163,372 | no |
| `resources/bundled-skills/xlsx.skill` | 155,709 | no |
| `resources/bundled-skills/pdf.skill` | 21,600 | no |
| `resources/bundled-skills/frontend-design.skill` | 8,125 | no |
| `resources/bundled-skills/pdf-reading.skill` | 7,997 | no |
| `resources/bundled-skills/manifest.json` | 4,628 | no |
| `resources/github-mcp/github-mcp-server` | 39,731,138 | yes |
| `resources/office365-mcp/office365-mcp-stdio.mjs` | 7,288,531 | no |
| `resources/office365-mcp/libmsalruntime_arm64.dylib` | 7,134,816 | yes |
| `resources/office365-mcp/pdf.worker.mjs` | 1,089,813 | no |
| `resources/office365-mcp/pdfExtractorProcess.mjs` | 425,295 | no |
| `resources/office365-mcp/msal-node-runtime.node` | 359,456 | yes |

`.skill` matches the exported zip type `com.anthropic.skill`. The archives
were not opened. The GitHub and MSAL binaries are described in
[native modules](native.md).

## Unpacked integrity metadata

Ten entries are marked unpacked, and the same ten have archive integrity
metadata that does not match the bytes observed for that logical path.
Mismatched content cannot support equivalence between the metadata and the
observed file. This is an archive-metadata fact. It is not a statement that
the Developer ID signature failed, and the application was not run to see
which bytes Electron loads.

| Logical path | Declared SHA-256 | Observed SHA-256 |
| --- | --- | --- |
| `node_modules/@ant/claude-native/claude-native-binding.node` | `6edd538da90aabf1b6e474e478931816ff60bbf6cf696d734df5b6a0ee870d09` | `bf61b0f31cea501ea2d79ef41ca13f28570c57022586498d662c2ea8f61554f7` |
| `node_modules/@ant/claude-swift/build/Release/computer_use.node` | `0153757d0ca4ac45d05867f4b7747cb5d23cfab6d43a0f477ba907cfd74a841f` | `b11aa4b2a03bf8bc92d2129bbf0aa60c512d1b2914782149b450573d3eb809ec` |
| `node_modules/@ant/claude-swift/build/Release/swift_addon.node` | `74d055b92489ea147621ef0547f94689db1fb76bb5ef8829822a4993d3016d60` | `802ed30cac1242094481d28ceb85682ab72a488833f5ce2a7719923e862f902e` |
| `node_modules/node-pty/prebuilds/darwin-arm64/pty.node` | `72ea0c3d8bdaa7fdb850ab28d09caa900b653f6b3a6db3db243624c671047e37` | `a8887e12780b6176c6584146053844a52fa95dd61677cb617d26bc64e0d1c26c` |
| `node_modules/node-pty/prebuilds/darwin-arm64/spawn-helper` | `926dfd78b6a4967a40f1e2fe85f6978237096ecfd55735c29de1ad38bba34deb` | `9a7ff3d8a3af40b7de70e3b65a8ef022326d7f97917ebb7af1ab8f6c13d9a458` |
| `node_modules/node-pty/prebuilds/darwin-x64/pty.node` | `102fe7e2e9ee4e28d22761cd94b45c4eff6f6a181ced914a3fc01320ea83706a` | `97143b0b74ace5e2ce1fcf50b6967b6be01b8e2ad01e6cebc0357a4593264982` |
| `node_modules/node-pty/prebuilds/darwin-x64/spawn-helper` | `d262f35a9017eff5d466612a59f2ce1f86ea3e930d793130365ba0ef43fad2f6` | `2c1d364b1e2ed6eddeca804b012af4ee87fda94b534dcf5560f7e2c2a2101e8a` |
| `resources/github-mcp/github-mcp-server` | `bbe4f3da24b4167889563e1cfd6776aebb772a2d06a0c5e91b2cf33031e6b478` | `beb8c09ddcaa00234b88bc670b142e0060d0c9fe5cde20e9774ee51c2c1d3d7e` |
| `resources/office365-mcp/libmsalruntime_arm64.dylib` | `24e90a68a669f59430e982e9f628d4fb8878c134da25272468e51daa9f498c19` | `9258fb9a53c676365425e755b52359916d958b6124a4d5d0d57a23d1105fe159` |
| `resources/office365-mcp/msal-node-runtime.node` | `0139daad3da5282cb14ae3d78198f59b02cae124cbafd42d64f94ecb4b05fa05` | `31c7a3cb54b0f72d235788ecc07197f33e206da16a3ebc42492f3f8f51fdadb9` |

The native-addon nodes in the JavaScript graph use the observed digests.
No JavaScript `require` binding to those addons was resolved. The files are
still in the archive inventory, and the native chapters describe the
observed binaries.
