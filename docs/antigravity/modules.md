# Antigravity `dist/` module reference

Every JavaScript module inside `app.asar/dist`, grouped by responsibility. Sizes are the
uncompressed byte sizes recorded in the ASAR header.

## Root modules

| Module | Size | Responsibility |
| --- | --- | --- |
| `main.js` | 40 KB | App lifecycle, switches, window creation, protocol boot, quit handling |
| `preload.js` | 44 KB | `contextBridge` API for every BrowserWindow; WCV hole reporting |
| `ipcHandlers.js` | 52 KB | All `ipcMain.handle` channels |
| `languageServer.js` | 24 KB | Spawn/supervise `language_server`, port parsing, bootstrap token, TLS trust |
| `cablev2.js` | 82 KB | Device pairing / cable protocol v2 |
| `wcvBrowserRegistry.js` | 96 KB | Registry of WebContentsView browsers |
| `wcvZOrderController.js` | 160 KB | Z-order compositing for WCV layers |
| `wcvIpcHandlers.js` | 36 KB | IPC for the WCV layer |
| `webviewBrowser.js` | 124 KB | `<webview>`-based browser implementation |
| `guestDialogController.js` | 39 KB | Dialog interception for guests |
| `guestFrameBridge.js` | 23 KB | Frame-level messaging for guests |
| `guestFileAttachment.js` | 22 KB | File attachment handling |
| `guestFileChooserController.js` | 17 KB | File chooser interception |
| `guestNetworkController.js` | 17 KB | Guest network policy |
| `guestNetworkSanitizer.js` | 16 KB | Request/header sanitization |
| `guestWebMcpController.js` | 18 KB | WebMCP integration for guests |
| `guestCrossSiteCredentials.js` | 14 KB | Cross-site credential policy |
| `guestFramePolicy.js` | 14 KB | Frame navigation policy |
| `guestPromptController.js` | 12 KB | JS dialog/prompt handling |
| `guestAgentControl.js` | 3.6 KB | Agent control hooks for guests |
| `guestPermissionPolicy.js` | 2.7 KB | Permission decisions for guests |
| `guestRequestHeaders.js` | 2 KB | Header rewriting |
| `guestDebugger.js` | 1.7 KB | Debugger attachment |
| `scriptJail.js` / `scriptJailPreload.js` | 40 KB / — | Sandbox for executed scripts |
| `isolatedWorldExec.js` | 13 KB | Execution in an isolated world |
| `customScheme.js` | 4.4 KB | `plugin://`, `agy-favicon://`, `agy-browser-stream://` |
| `faviconStore.js` | 4.3 KB | Tokenized favicon cache |
| `redirects.js` | — | Redirect table |
| `authBootstrap.js` | 17 KB | Auth bootstrap flow |
| `beyondcorpPac.js` | 13 KB | BeyondCorp PAC proxy |
| `webauthn.js` | — | Session WebAuthn configuration |
| `hostBridgeServer.js` | 8.6 KB | Local host bridge server |
| `storage.js` | — | `StorageManager` (get/update + change events) |
| `paths.js` | — | On-disk path resolution |
| `windowState.js` | — | Window geometry persistence |
| `menu.js`, `tray.js`, `keybindings.js` | — | Native menu, tray icon, shortcuts |
| `updater.js` | — | `electron-updater` wrapper |
| `mendelFlagSync.js` | — | Feature-flag sync |
| `deviceChooser.js` | 16 KB | Device picker |
| `loginShellEnv.js` / `loginShellEnvReads.js` / `loginShellPolicy.js` | 11 KB / 2.3 KB / 1.5 KB | Login-shell environment capture |
| `externalUrl.js` | 4.3 KB | External URL handling |
| `loadingOverlay.js`, `provisionSplash.js`, `debugConsole.js` | — | UI affordances |
| `hubWindows.js`, `hubZoom.js` | — | Hub window management and zoom |
| `constants.js`, `types.js`, `utils.js`, `nodeWrapper.js`, `lsDefaults.js`, `wsl.js` | — | Shared support |
| `webviewDownloadRouting.js`, `webviewPopupSecurity.js`, `webviewPromptPreload.js`, `webviewSecurityState.js`, `webviewUploadValidation.js`, `webviewWebAuthnPreload.js` | — | `<webview>` hardening |

## `actuator/`

Connect-RPC client to the language server: `mainClient.js`, `nodeTransport.js`,
`pinnedLsCert.js` (cert pinning), `testCerts.js`, `index.js`.

## `browserEngine/`

`browserEngine.js`, `cdpPipe.js`, `chromeDiscovery.js` — discover a local Chrome and drive
it over a CDP pipe.

## `browserStream/`

`browserViewerHandler.js` (41 KB), `captureWindow.js` (25 KB), `capturePage.js` (24 KB),
`browserViewerTabSnapshot.js` (24 KB), `browserViewerTabDriver.js` (18 KB),
`browserViewerUrlPolicy.js` (16 KB), `capturePreload.js`.

## `computer/`

`macComputerDriver.js` (22 KB), `index.js` (17 KB), `hud.js` (15 KB),
`desktopHelper.js` (12 KB), `overlay.js` (10 KB), `previewIpc.js`, `previewViewers.js`,
`permissions.js`, `permissionsIpc.js`, `hudAgents.js`, `hudState.js`, `gate.js`,
`screenLock.js`.

## `computerActuator/`

`computerActuatorHandler.js` (114 KB — largest JS module), `scriptRunner.js` (26 KB),
`scriptMediator.js` (26 KB), `computerOps.js` (23 KB), `fakeComputerDriver.js` (12 KB),
`registerComputerActuator.js`, `index.js`, and `driver/` (`computerDriver.js`,
`backgroundDriver.js`, `errors.js`, `index.js`).

## `desktopHost/`

WebRTC desktop host: `previewManager.js` (29 KB), `host.js` (18 KB), `rtcSender.js`
(27 KB), `viewerHandler.js` (13 KB), `pairing.js`, `frames.js`, `main.js`,
`rtcTransport.js`, `scriptSelftest.js`, `protocol.js`, `config.js`, `colors.js`,
`takeovers.js`, `activityThrottle.js`, `index.js`.

## `ideInstall/`

Install wizard for the IDE integration: `constants.js`, `index.js`, `service.js`,
`wizard.js`, `wizardHtml.js`, `wizardPreload.js`.

## `proto/`

Generated protobuf/Connect stubs: `actuator_pb.js`, `browser_viewer_pb.js`,
`computer_payload_pb.js`, `computer_viewer_pb.js`, `host_bridge_pb.js`.

## `remote/` (34 modules, ~476 KB)

SSH and remote-host lifecycle: `remoteManager.js`, `transport.js`, `sshConnection.js`,
`sshBridge.js`, `sshAskpass.js`, `sshArgs.js`, `sshConfigHosts.js`, `sshDestination.js`,
`sshErrorText.js`, `sshPrompts.js`, `sshPromptPreload.js`, `sshRecentHosts.js`,
`sshWindowRestore.js`, `remoteIpcChannels.js`, `remoteLsArgs.js`, `remoteSidecars.js`,
`remoteWindowSecurity.js`, `serverArtifacts.js`, `serverVersion.js`,
`bootstrapScript.js`, `bootstrapProtocol.js`, `connectionInputs.js`, `localNetwork.js`,
`privateDir.js`, `pausableTimeout.js`, `signInUrls.js`, `signInCallback.js`,
`signInReturn.js`, `disconnectedPage.js`, `hostQuestions.js`, `hostStores.js`,
`windowBackends.js`, `types.js`.

## `services/`

`settingsService.js` — typed settings (`runInBackground`, `keepComputerAwake`,
`autoCheckForUpdates`) with reactive side effects.

## `vendor/`

`actuator/` — vendored actuator code.

## `__mocks__/` and `test/`

`electron.js` (14 KB), `electron-updater.js`, `electron-log/main.js`; `test/` helpers.
Shipped in production.
