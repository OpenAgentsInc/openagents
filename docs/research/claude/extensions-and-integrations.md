# Extensions, MCP, and plugins

MCP shows up in four places that do not share a process. A settings file the
`MCP` service edits. A session that already has servers attached. A direct
connection the local-agent session authorizes. A plugin or a `.dxt` / `.mcpb`
extension that brings its own server. The GitHub server and the Office server
ship as files. Chrome is a native host plus an extension id. The node host is
the utility process that runs a local server when the user has not pointed
the app at a system Node.

## The node host refuses to be Electron

`mcp-runtime/nodeHost.js` is the entry a utility process forks. After
`{type: "init", ports}` it imports the server entry as an ES module and
presents `argv` as `node` plus that entry. It patches `child_process`.
`spawn`, `fork`, `spawnSync`, `execFileSync`, `execSync`, and `execFile`
are refused when the target is the Electron binary, or another binary inside
a `.app` `MacOS` directory, including a helper's Frameworks path.

The refusal is `SelfExecBlockedError` with code `ERR_MCP_NODE_HOST_NOT_NODE`.
The message tells the user to install Node.js and to turn off "Use built-in
Node.js for MCP" under Settings, Extensions, Advanced. The first refusal posts
`{type: "self-exec-blocked", api}` once. Later refusals are quiet unless 20
happen within 5 seconds, and then the host throws. A marker file
`.built-in-node-refused` is written next to `process.execPath`.

`AppConfig.setIsUsingBuiltInNodeForMcp` is the setting that sentence names.
`setIsDxtAutoUpdatesEnabled` is the neighbor on the same service.

The direct-MCP host (`directMcpHost.js`, 613,700 bytes) is a second utility
entry. It contains a full MCP client: `initialize`,
`notifications/initialized`, `tools/list`, `tools/call`,
`notifications/tools/list_changed`, `resources/list`,
`resources/templates/list`, `resources/read`, `resources/subscribe`,
`resources/unsubscribe`, and the list-changed and updated notifications.
OAuth grant strings in the same file include `authorization_code` and
`refresh_token`, and the token type
`urn:ietf:params:oauth:token-type:id-jag`. Server identity is compared
against `io.modelcontextprotocol/serverInfo`. A call can be aimed at
`thisServer` or `allServers`. The file is mostly the MCP SDK and a schema
library. The desktop-specific policy around it is the session methods
`authorizeDirectMcpServer` and `authorizeDirectMcpServerWithAccountPicker`,
not a second copy of the protocol.

## Servers the app will not keep

A filter drops a server whose name collides with a trusted internal prefix.
The warning cites `reservedServerNames.ts` and a caller tag. A second filter
drops individual tools. The internal prefixes themselves were not recovered
as a string list; the log is the evidence the filter exists. Names that are
built in the scripts, and that this filter is shaped to protect, include
`mcp__<server>__<tool>`, `mcp__plugin_<plugin>_`, and
`official-plugin__<publisher>__<name>__<version>`. `plugin-shim:` is a
separate prefix on the plugin shim path. `sandbox.claudemcpcontent.com` is
the host string stored beside content sandboxing for MCP results.

The settings service `MCP` edits the config the filters read:
`getMcpServersConfig`, `getMcpServersConfigWithStatus`, `setMcpServerConfigs`,
`isLocalDevMcpEnabled`, `revealConfig`, `revealLogs`, `revealServerLog`, and
the events `mcpConfigChange` and `mcpStatusChanged`. Managed settings that
name the same lists, seen as keys in the main chunk, include
`allowedMcpServers`, `deniedMcpServers`, `strictKnownMarketplaces`, and
`blockedMarketplaces`. Those are key names in a policy object. The current
machine's policy was not read.

## Plugins

Two plugin services.

`LocalPlugins` is the installed set: `getPlugins`,
`getDownloadedRemotePlugins`, `syncRemotePlugins`,
`materializeEnabledPlugins`, `setPluginEnabled`, `deletePlugin`,
`uploadPlugin`, `listSkillFiles`. OAuth for a plugin:
`startPluginOAuthFlow`, `revokePluginOAuth`, `setPluginOAuthClient`. A CLI
the plugin ships: `getPluginCliStatus`, `getPluginCliBatch`,
`getPluginShimOps`, `setPluginShimPermission`, `onCliOpAlwaysAllowed`.
Environment for that CLI is `setPluginEnvVars`. Log tags that match:
`PluginOAuthFlow`, `PluginOAuthStorage`, `PluginEnvStorage`,
`PluginMcpHostConfig`, `PluginIpc`, `PluginScan`, `PluginManifest`,
`EnabledCliOpsStore`, `SkillsPlugin`.

`CustomPlugins` is marketplaces and org plugins: `addMarketplace`,
`removeMarketplace`, `refreshMarketplace`, `listMarketplaces`,
`listAvailablePlugins`, `listRemotePluginsPage`, `installPlugin`,
`updatePlugin`, `uninstallPlugin`, `checkPluginHasLocalChanges`,
`getAndClearMigrationIssues`, `listLocalOrgPlugins`,
`installLocalOrgPlugin`, `syncLocalOrgPlugins`, `localOrgPluginsSynced`.
A deep link to `customize/plugins/new?marketplace=&plugin=` lands in this
list. `PluginBridgeMcp` is the thin event the page uses to watch that bridge:
`listServers` and `changed`.

`Extensions` is the desktop-extension package, as opposed to a marketplace
plugin. The file types in the bundle's document registration are `.dxt` and
`.mcpb`. Methods: `isExtensionsEnabled`, `isDirectoryEnabled`,
`isLocalExtensionInstallAllowed`, `isDesktopExtensionDirectoryEnabled`,
`isDesktopExtensionSignatureRequired`, `getInstalledExtensionsWithState`,
`getExtension`, `getExtensionVersions`, `getManifestCompatibilityResult`,
`getAvailableExtensionRuntimes`, `getExtensionSettings`,
`setExtensionSettings`, `installDxt`, `installDxtUnpacked`,
`installDxtFromDirectory`, `installExtensionFromPreview`,
`previewExtensionInstallation`, `handleDxtFile`, `showInstallDxtDialog`,
`deleteExtension`, `openExtensionsFolder`, `openExtensionSettingsFolder`,
`getDirectoryUrl`, `refreshAllowlistCheck`, `getIsUpdateAvailable`. Events:
`extensionsChanged`, `extensionSettingsChanged`, `extensionDownloadProgress`.
Signature required and allowlist refresh are both methods, so an install can
be refused after the file is parsed. `DxtPaths`, `McpbParsing`, and
`ManifestParsing` are the log tags.

Bundled skills ship as zip files registered as `com.anthropic.skill`:
`pptx`, `docx`, `xlsx`, `pdf`, `frontend-design`, `pdf-reading`, plus
`manifest.json`. The archives were not extracted. `SkillParsing` is the log
tag for reading one. Session methods `saveLocalSkill` and
`saveLocalSkillArchive` are how a session writes a skill the user built,
which is a different directory from the bundled zips.

## GitHub, Office, and Chrome

`resources/github-mcp/github-mcp-server` is a universal executable, unpacked
from the archive, 39,731,138 bytes. It is the Go module
`github.com/github/github-mcp-server` at `v1.6.0`. The session git methods
and this binary are different stacks. Pull-request review in
[sessions](sessions.md) goes through `gh`. This process is an MCP server the
extension host can spawn. The spawn arguments were not recovered, and the
binary was not run.

Office is `resources/office365-mcp/`. The stdio entry
`office365-mcp-stdio.mjs` is 7,288,531 bytes. Beside it: `pdf.worker.mjs`,
`pdfExtractorProcess.mjs`, `msal-node-runtime.node`, and
`libmsalruntime_arm64.dylib`. The URL scheme
`msauth.com.anthropic.claudefordesktop` is the MSAL reply. A second scheme
string in the broker error path is `msauth.com.msauth.unsignedapp://auth`,
used when the embedded provisioning profile is what the broker sees. The
broker error text points at the Company Portal SSO extension log under
`~/Library/Containers/com.microsoft.CompanyPortalMac.ssoextension/…`.
`microsoftAuthBroker` is a managed setting whose values include `disabled`,
`auto`, and `required`. `microsoftAuthDefaultAccount` is the switch for
signing in with the Windows account. Both records are in the macOS build's
settings table.

Chrome is three binaries' worth of JavaScript. `ChromeExtension`:
`isInstalled`, `installExtension`, `restartChrome`, `launchBrowser`,
`getBrowserActivity`, `getConnectedExtensions`, `connectedExtensionsChanged`.
The native host is `chrome-native-host`, covered in the native chapter.
`ChromeImport` and `Launch.getChromeImportAvailability` /
`acceptChromeImportSharedOffer` / `getCoworkChromeImportAvailability` are the
import of Chrome's existing state into Cowork. A remote session calls
`callChromeToolForRemoteSession` rather than `ChromeExtension.launchBrowser`.
Log tags: `Chrome Extension`, `Chrome Extension MCP`,
`Chrome Extension Install`, `chrome-mcp`.

`AgentModeFeedback` can `openFeedbackAndConfirmReinstall` and
`reportErrorToSlack`. That is the feedback path for this stack, not a second
Slack integration. `slackWatch` remains a Swift group name on the addon.

## Local execution consent

`LocalExecConsent.choose` is the one method on the consent window. The
preload forwards the argument unchanged and only exists for a top frame on
`localhost` or `file:`. The main-process log tag is `local-exec-consent`.
The prompt's choices and the default were not in the preload. The window
exists so a local tool run can ask in a window that does not have the main
view's 300-method bridge.

## CCD, the code-side host

CCD is the Claude Code desktop host that is not the Cowork VM. It has its
own scheduled-task store, its own CLI installer, and its own session
recovery. `CcdCli` is `getCcdCliStatus`, `installCcdCli`, `uninstallCcdCli`.
Log tags around it: `CCD`, `CCD-SessionRecovery`, `CCD-autoupdate`,
`CCDMarketplacePluginManagerCLI`, `HostCLIRunner`, `cliStubWriter`,
`cliTempRootsStore`, `codeSurfaceGate`. A boundary walk refuses a symlink
below the CCD root and records a hardlinked read. Shell environment captured
for ssh is newline-split and dropped when `env -0` fails. Those rules are
the host's path safety. They are not the VM guest's mount table.

`HistoryImport` and `ClaudeAiImport` bring older transcripts in.
History import: enumerate sources, scan one, import, list and import extras,
progress, history, remove one run or all of them, auto-import status.
Claude.ai import: device-code style auth (`startAuth`, `onAuthUserCode`,
`getAuthState`), a remote export (`fetchRemoteExport`, progress, status,
cancel, discard), local zips (`pickLocalZips`, `peekLocalZips`,
`runImportFromLocalZips`), space twins (`listImportSpaceTwins`,
`mergeImportedSpace`), and a session export to a folder
(`previewSessionExport`, `runSessionExport`, `showExportInFolder`).
`registerDesktop3pBundle` attaches a third-party desktop bundle to that
import. The export container name in the main process is
`claude-desktop-export`, written under user data with roots for projects,
code, chat, and cowork. `importThirdPartyHistoryIntoOrgUUID` is a managed
setting whose title is "Import earlier third-party sessions into this
organization".
