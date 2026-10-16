# Cowork

Cowork is a surface inside the same shell as chat and code, plus a Linux VM,
plus a document type the app serves itself. The route family is `cowork`,
`local_sessions`, `cowork_sessions`, `task`, `space`, and `epitaxy`. The VM
is how a task gets a Linux machine. The `cowork-artifact:` scheme is how a
task's rendered output is shown without giving that document the main view's
bridge. This chapter is the services and the document protocol. The session
file, the folder grants, the sessions bridge, the Copper Heron hold, and
direct MCP are the sections after artifacts. The boot sequence of the guest
is [the VM chapter](vm.md).

## Spaces

`CoworkSpaces` is the project around a set of sessions. `createSpace`,
`updateSpace`, `deleteSpace`, `getSpace`, `getAllSpaces`,
`getSpacesStoreState`. Membership is folders, projects, and links, each with
an add and a remove. `classifySessions` and `summarizeSpace` /
`setAutoDescription` label a space. `createSpaceFolder` and
`copyFilesToSpaceFolder` write into the space folder. `listFolderContents`,
`readFileContents`, and `openFile` read it back.

A space can remember. `getAutoMemoryDir`, `readSpaceMemoryIndex`,
`getSpaceMemorySummary`, `getSpaceMemoryCapabilities`, `startSpaceMemoryCopy`,
`getSpaceMemoryCopyStatus`, `recordSpaceMigrationOutcome`,
`setSpaceMigration`, `requestSpacesInventoryReport`. The copy log tag is
`SpaceMemoryCopy`. Remote sessions attach to a space with
`getRemoteSessionSpaces`, `setRemoteSessionSpace`,
`appendRemoteSessionSpaceFolders`, `removeRemoteSessionSpace`. `onSpaceEvent`
is the subscription.

Account memory is a different service, `CoworkMemory`: read and write a
global memory, read and write an account memory, list, delete, reset. `Gorse`
is the migration off whatever the previous store was: `status`, `preview`,
`progress`, `migrateToProject`, `exportToFiles`, `openExportFallbackFolder`,
`decline`. The name is the service name. The format it migrates was not
opened.

`CoworkStorage` is usage, not memory: counts, usage, reveal in the file
manager, preview a cleanup, run it, cancel it, and export one task, one
project, or everything. `CoworkUserFiles` is `getInfo`, `migrate`,
`pickTarget`, `reveal`. The main process has helpers whose log names are
`ensureCoworkUserFilesRoot`, `ensureCoworkManagedFilesRoot`,
`resolveCoworkUserFilesPath`, and `isCoworkManagedDirsInAppStorage`. A path
classifier log is `classifyStoredPath`. Managed-directory migration has the
tag `managedDirsMigration`.

`CoworkFilePreview` is how a file is shown beside a task. `isEnabled`,
`isEpitaxyPreviewEnabled`, `isOpenInDefaultAppEnabled`, `isVmReady`,
`vmReadiness`, `show`, `hide`, `parkAndCapture`, `whenContentReady`. Epitaxy
is a preview mode and also a deep-link path (`/epitaxy`). The preview refuses
to show until `isVmReady` is meaningful; the method exists so the page can
ask. Parking and capturing is shared with artifacts
(`parkAndCaptureArtifact` on the artifact service).

## Artifacts

`CoworkArtifacts` on the main view manages the list. `getAllArtifacts`,
`getArtifactMetadata`, `getArtifactHtml`, `getArtifactThumbnail`,
`getArtifactRepublishContent`, `setArtifactName`, `setArtifactStarred`,
`setArtifactMcpTools`, `setArtifactLastModifiedSession`, `deleteArtifact`,
`showArtifact`, `hideArtifact`, `reloadArtifactView`, `printArtifactToPdf`,
`restoreArtifactVersion`, `importArtifact`, `refreshImportedArtifact`.
Sharing: `isSharingEnabled`, `isAutoPublishEnabled`, `setArtifactAutoPublish`,
`shareArtifact`, `unshareArtifact`, `isSharedContentCurrent`. A shared
artifact is addressed by uuid, and the deep link is
`claude://cowork/shared-artifact?uuid=…`. `onArtifactsChanged` is the
subscription.

The document the preview loads is not the main view. It is a
`cowork-artifact:` response from `protocol.handle("https")` on the artifact
session, or a navigation to that scheme. The HTML may contain one script tag:

```text
<script type="application/json" id="cowork-artifact-meta">…</script>
```

The handler parses that JSON and keeps `name`, `description`, `mcpTools`,
`mcpServerNames`, and `schemaVersion: 1`. It then strips the tag. A parallel
replacement walks a fixed list of HTML-sensitive tokens and their numeric
character references, which is a scrub of the document the guest will parse,
not a general sanitizer of every string. The guest session's permission
handlers always deny, and a request whose URL does not start with the
artifact origin is cancelled.

Responses that are not the document:

- `Not found`
- a tool name that is not `mcp__<server>__<tool>` is refused
- `resolved`, `refused`, `not-a-candidate`
- a connector that "has not finished connecting"
- `account_mismatch` when the request's `X-Expected-Account` does not match

Bundled libraries are served from `resources/artifact-libs` with
`Access-Control-Allow-Origin` set. The handler names Mermaid 11.17.2 and
Grid.js 5, including the Grid.js mermaid theme stylesheet, and attaches
`sha384` integrity to those responses. Chart.js is the third file in that
directory (208,337 bytes) and is not named in the slice that was read. The
same handler references the jsDelivr URLs for those versions as the identity
of the bytes it already has locally. It does not have to fetch them for the
response path that reads `artifact-libs`.

The artifact preload's world is `cowork`, with three calls:

- `askClaude(text)`
- `callMcpTool(serverTool, args)`
- `runScheduledTask(id)` and the preload adds
  `{hasUserActivation: navigator.userActivation.isActive}`

Clicks on anchors bubble through a capture listener. A trusted primary or
middle click on an anchor whose protocol is not `cowork-artifact:` is
prevented and sent as `openExternalUrl`. `navigateHost("back"|"forward")`
comes from mouse buttons 3 and 4 on macOS. Focusin, focusout, and keydown
recompute whether the focused node is an input, textarea, select, frame, or
contenteditable, including inside open shadow roots, and send
`setEditableFocused`. That is how the host knows a keyboard shortcut should
stay in the document.

The preview pane's other preload, `claudePagePreview.js`, is for a claude.ai
artifact frame rather than a `cowork-artifact:` document. Its handshake is in
[web and preview](web-and-preview.md). Both can be on screen. They are not the
same bridge. `cowork` can call a tool. `claudeDesktopArtifactPane` only
announces that the frame is embedded, on desktop, with `chrome: "none"`, and
optionally a `hostcaps` list.

## Where a Cowork session record is stored

Observed. The local-agent manager does not use `claude-code-sessions`. Its
account directory is

```text
<userData>/local-agent-mode-sessions/<account>/<org>/
```

`<userData>` is `app.getPath("userData")`. On a third-party deployment the
same helper may use the first eight hex characters of a uuid, lowercased,
for the account directory and the org directory. It does that only after the
deployment type has been read as `3p`, and only when the short directory is
the one that already exists or the full uuid directory is absent. A probe
error keeps the full path. A first-party run leaves the flag off, so the
full ids are used.

Observed. The session file is `<account dir>/<sessionId>.json`. When
`sessionType` is `agent`, the file is `<account dir>/agent/<sessionId>.json`
instead. The write is pretty-printed JSON (`JSON.stringify` with two-space
indent) through the same private atomic writer as the code sessions, mode
`0600`, after the directory has been created mode `0700`. If the session is
gone from memory before the write, the write is skipped. The log on success
is `Saved session <id> to storage`.

Observed. The object includes `sessionId`, `processName`, `cliSessionId`,
`cwd`, `createdAt`, `lastActivityAt`, `model`, `permissionMode`,
`isArchived` (true when the lifecycle state is `archived`), `title`,
`userApprovedFileAccessPaths`, `vmProcessName`, `hostLoopMode`,
`importedFrom`, `resumeConfirmed`, `spaceId`, `spaceIdSetBy`,
`sessionType`, `parentSessionId`, `scheduledTaskId`, `enabledMcpTools`,
`remoteMcpServersConfig`, `fsDetectedFiles`, `chromePermissionMode`,
`chromeAllowedDomains`, `chromeTabGroupId`, `cuAllowedApps`,
`cuGrantFlags`, `approvedToolNames`, `egressAllowedDomains`,
`memoryEnabled`, `skillsEnabled`, `pluginsEnabled`, `artifactHostGrant`,
`pluginInstallPaths`, and the prompt, error, and memory fields copied beside
those. Chat sessions also spread the folder record the manager kept for
them.

Observed. Beside the JSON, `primeSessionDirs` creates two directories under
the session storage directory: `outputs` and `.claude`. The host process
cwd recorded for the session is the helper's host cwd, or
`<session dir>/host-cwd` when that helper has none. A `local_<uuid>` session
directory may be addressed by the first eight hex digits of that uuid when
the full directory is not already a directory and the short name is free. If
both names are occupied by something that is not a directory, the resolver
throws `session storage folder name is occupied`.

The "Open Cowork session data folder" action uses
`getAccountStorageDir()`, and if that is null it opens
`<userData>/local-agent-mode-sessions` directly. An import scan classifies a
directory as user data when it contains `local-agent-mode-sessions` or
`claude-code-sessions`, and as projects when it contains `projects` or
`.claude/projects`.

## Folder grants

Observed. Grants that outlive one session are keys of `preferences` inside

```text
<userData>/claude_desktop_config.json
```

The file is read synchronously at startup. A missing file becomes an empty
object. A read or JSON error marks the load degraded. While the load is
degraded, later writes are refused and the log says that persisting defaults
could destroy the original. A successful write replaces the whole JSON
atomically, mode `0600`. The preferences object is one key of that JSON.
Setting one preference reads the current object, replaces that key, and
writes the file. After a write, open windows get `dispatchPreferencesChanged`
with a copy that always clears a fixed set of grant fields before they are
shown to the page (the lists below are sent empty in that dispatched copy
even when the file has them). The on-disk keys are not cleared by that
dispatch.

Observed. The grant keys in that preferences object are:

| Key | Empty value in the defaults |
| --- | --- |
| `routineFolderGrants` | `{}` |
| `remoteSessionFolderGrants` | `{}` |
| `remoteChatFolderGrants` | `{}` |
| `remoteFolderConsentFromSettings` | `[]` |
| `remoteChatFolderAccessReports` | `{}` |
| `remoteChatFolderAdoptionChecked` | `[]` |
| `ccSshHostToolGrants` | `[]` |
| `dispatchTrustedCodeWorkspaces` | `[]` |
| `coworkLegacyRootGrantsPruned` | `false` |
| `selfHostedGrantsHost` | `""` |
| `localAgentModeTrustedFolders` | `[]` |

`dispatchCodeTasksPermissionMode` defaults to `acceptEdits` in the same
object. The values are not all empty objects of the same shape.

`localAgentModeTrustedFolders` is an array of path strings. A one-time
prune drops entries the helper treats as never served, including `/` and a
home directory, and then sets `coworkLegacyRootGrantsPruned`. The log is
`[cowork] one-time prune of never-served "/" / home grant entries`.

`routineFolderGrants` and `remoteChatFolderGrants` are records keyed by
string. Each value is `{orgUuid, accountUuid, folders}`. A folder entry is
either a string or `{display, unc?, letter?}`. `remoteSessionFolderGrants`
is a record of those folder arrays, without the org and account wrapper.
`remoteChatFolderAccessReports` values are
`{orgUuid, accountUuid, generation, digest, settled}`. `generation` is a
positive integer and `settled` is a boolean.
`remoteFolderConsentFromSettings` and `remoteChatFolderAdoptionChecked` are
arrays of strings. `dispatchTrustedCodeWorkspaces` is an array of strings.

`ccSshHostToolGrants` is an array of `{account, endpointKey, serverHostKey,
toolName, sshHost, serverLabel, toolLabel, grantedAt}`. `grantedAt` is a
number. A row is kept only when every one of those fields has that type.
`folderTccProbeResults` is `{desktop?, documents?, downloads?, recordedAt?,
appVersion?}`. The three folder fields are `granted` or `denied`.

Per session, the session JSON holds `userApprovedFileAccessPaths`, and the
loader also accepts `fileDeleteApprovedMounts` and `folderMountNames`. Those
three are the session's own grant lists. They are not a second file.

Observed. Copying folder grants onto a finished scheduled task is refused,
and the task id is logged, when any of these is true: there is no revocation
epoch, a revocation landed after the seal, there is no recorder, the account
or org changed after completion, the task is no longer in the live store, or
a compliance verdict engaged during the copy. The log prefix is the scheduled
task copier. A Cowork scheduled-task directory is created with
`taskDirBoundaryUntrusted: true`, which is the boundary flag on that store,
not a bit inside the grant JSON.

Derived. Inside a VM session the prompt text tells the model that connected
folders are mounted at `/sessions/<session>/mnt/<name>`. The host path stays
the path in the grant. A tool that is told a `/sessions/...` path while it
is running on the host is denied, with a message that the VM path does not
exist on the host. A path outside the connected folders is denied and the
model is told to request the folder. A path that resolves to a protected
location inside a connected folder is denied without asking the user to
grant the folder again.

## The sessions bridge

Observed. The sessions bridge in this build is an object in the main
process, not a second executable on the path that registers it. The class
extends `EventEmitter`. It keeps `environmentId`, `environmentSecret`, a
poll abort controller, a backoff attempt, and counters for registration
responses 401 and 403. Its persisted state is one JSON file:

```text
<userData>/bridge-state.json
```

The key is `<orgUuid>:<accountUuid>`. Callers pass the org id first. The
fields read back are `userConsented` (true only when the boolean is true),
`enabled` (on unless the boolean is false), `environmentId`,
`dispatchAgentName`, `remoteSessionId`, `localSessionId`,
`localSessionGen` (used only when `localSessionId` is set and the generation
is a non-negative number), and `lastDispatchMessageAt`. A missing file reads
as `{}`. A read error other than `ENOENT` also becomes `{}` after a warning.
JSON that does not parse yields no persisted session id. Writes use the same
atomic pretty-printer as the session files. `environmentSecret` is kept on
the object in memory. The register write does not put it in this file.

Observed. `start` reads that record, forces `userConsented` true, and
registers with `POST /v1/environments/bridge`. The JSON body is
`machine_name` (the host name), `directory` `/cowork`, `metadata`
`{worker_type: "cowork"}`, and `environment_id` when one was already stored.
The bearer on that post is the OAuth access token. A trusted-device token is
added as `X-Trusted-Device-Token` when one exists. The other headers are
`anthropic-version: 2023-06-01`, `anthropic-beta: managed-agents-2026-04-01`,
`x-environment-runner-version` (the app version), and
`Anthropic-Worker-ID` (a UUID chosen for this client). The request times out
after 15 seconds. A 401 retries once with a fresh token. A 403 enrolls the
device and retries once when `error.details.error_code`, `error.resource`,
or `resource` is `untrusted_device`. Credential invalidation is not repeated
inside 5 minutes. The response fields kept are
`environment_id` and `environment_secret`. The id is written back into
`bridge-state.json` with `userConsented` and `enabled`. If the server
returns a different id than the one that was sent, the persisted remote
session id is cleared. The log says the max lifetime may have been exceeded.

Observed. The poll loop is `GET /v1/environments/<id>/work/poll`, bearer
equal to the environment secret, with the same version and worker headers
as registration, and times out after 15 seconds. The default interval is 15
seconds. Flag `1978029737` can replace it through
`sessionsBridgePollIntervalMs`, and, while a dispatch message has arrived
within the last 604,800,000 ms (7 days), through
`sessionsBridgeActivePollIntervalMs`. Otherwise the interval is
`sessionsBridgeInactivePollIntervalMs`, which defaults to the active
interval and is capped at one hour. `kickBridgePoll` aborts the sleep
between polls. A change to flag `1978029737` does the same. A poll status
of 401, 403, 404, or 409 tears down the active sessions, forgets the id and
the secret, and calls `start` again, up to 3 times. After that the client
sets `startFailedPermanently` and logs
`lam_bridge_poll_gave_up`. Other failures back off from 1 second, doubling,
capped at 30 seconds. Work from a poll is acknowledged at
`POST /v1/environments/<id>/work/<workId>/ack`. The other paths beside
register are

```text
/v1/environments/bridge/<id>
/v1/environments/bridge/<id>/offline
/v1/environments/<id>/bridge/reconnect
```

`getSessionsBridgeEnabled` and `setSessionsBridgeEnabled` are the preload
names for the `enabled` flag. `getBridgeConsent` is the `userConsented`
flag. `resetBridge`, `resetBridgeSession`, `abandonBridgeEnvironment`,
`kickBridgePoll`, and `deleteBridgeSession` are the other preload methods on
that object. If dispatch asks to set the agent name and the client is not
running, the throw is `Sessions bridge client is not running`. The agent
name itself is behind the flag id `3558849738`. The analytics name for this
client is `sessionsBridgeClient`.

Observed. A dispatch child that starts a code session calls
`checkWorkspaceTrust` on the folder and, when the folder is not trusted,
`saveWorkspaceTrust` with `withoutDialog: true`, then `startSession` with
`permissionMode` from the dispatch default, `dispatchParentOrigin: "local"`,
and `exclusiveCwd: true`. That is the script. It is not a second trust file.

## Copper Heron

Observed. Copper Heron is one in-memory hold in the main process. It is not
a file under user data. Taking a new hold clears the previous one. The hold
object is:

```text
createdAtMs, holdEpoch, requesterWebContentsId, sessionId,
recording: { startedAt, durationMs, frameIntervalMs, frames, events, apps },
transcript, computerUseEnabled,
summaryTakes, thumbnailTakes, thumbnails,
lastStageFrame, lastStageFrameAtMs,
timer, ttlDeadlineMs, finalizedContentHash
```

`recording.frames` is the frame list from the capture. `recording.events` is
`rawEvents` mapped with the display context. `recording.apps` is the app
list from the capture. `transcript` is built from narration snapshots and
the duration. `requesterWebContentsId` defaults to `-1` when the capture did
not name a web contents.

Observed. Each of summary, thumbnails, and the staged frame may be taken at
most three times (`COPPER_HERON_MAX_TAKES`). A fourth take logs
`context recording <kind> exhausted` and returns null. Staged frames are
also limited to one per 100 ms. A thumbnail strip resizes each chosen frame
to a width of 240 and encodes JPEG quality 60. Frames closer than
`max(1000, ceil(durationMs / 120))` milliseconds, or whose encoded bytes
repeat the previous choice, are skipped. A single frame preview resizes so
the long edge is at most 1280, encodes JPEG quality 80, and returns a
`data:image/jpeg;base64,...` URL plus `tMs`. The summary returned to the
page is `durationMs`, a copy of the transcript, and the apps as
`{tMs, name}`.

Observed. The hold expires on a timer. The deadline is `createdAtMs` plus
one hour, and the timer delay is the remaining time clamped to at most 15
minutes. A send may extend that delay by up to five more minutes. On expiry
the log is `context recording hold dropped — expired`, the pill is
dismissed, and the renderer is told `copperHeronHoldDropped` with
`holdEpoch`. An account change increments an identity generation and drops
the hold with `account identity changed`. A HIPAA gate on `watch_record`
drops it with reason `compliance_block`. A kill switch or a platform block
on `cowork_watch_record` refuses the read and keeps the hold. The refusal
strings are `kill switch off` and `my-access platform block`.

Observed. Finalize stores `finalizedContentHash`, the SHA-256 hex of
`JSON.stringify` of the content blocks. A text block contributes
`["text", text]`. Any other block contributes
`[type, source.media_type, source.data]`. `start` on the local-agent service
loads `verifyCopperHeronFinalizedBlocks` and refuses the start unless the
trailing blocks hash to that value for the same `webContents.id`. The errors
are `trailingContentBlocks failed shape validation` and `trailingContentBlocks
do not match the finalized hold`. Discard clears the hold. The bytes of a
frame are not written to `bridge-state.json` or the session JSON by the
functions above.

## Direct MCP

Observed. `authorizeDirectMcpServer` is handled in the direct-MCP chunk, not
by editing `claude_desktop_config.json` in that function. The order is:

1. If the name is in the built-in registry, transition that server to
   `user-connect` and connect it. The log is
   `LocalAgentModeSessions.authorizeDirectMcpServer: <name> — connecting (builtin)`.
   Microsoft 365 is the built-in that
   `authorizeDirectMcpServerWithAccountPicker` forces through the account
   picker (`accountPicker: true`). A missing built-in of that name returns
   `No built-in Microsoft 365 server named "<name>"`.
2. Otherwise, if a pending OAuth config exists, reconnect it with the
   session home. If the pending config changes while that is in flight, the
   new connection is disposed, the result is `{ok: false, cancelled: true}`,
   and a grant clear runs against `home.grants` for that name when the
   config was the kind that owns grants. Success calls
   `addConnectedDirectMcp` and returns `{ok: true, tools}` from
   `rendererTools`. The log includes the tool count.
3. If there is no pending config, a server already in `directMcpServers()`
   under that name returns its current tools. Otherwise the error is
   `No pending MCP server named "<name>"`.

Observed. Cancel and failure are separate. User sign-in canceled, and
disconnect during connect, return `{ok: false, cancelled: true}`. A newer
OAuth attempt that supersedes this one is the string
`custom3p-oauth-cancelled-by-newer` or the same cancelled result. Any other
throw returns `{ok: false, error, failureKind}`. The error string is passed
through a redaction helper before the log line.

Observed. Connect and disconnect can open a consent window,
`localExecConsent.html`, width 460. Connect is modal, takes focus, and times
out after 600,000 ms. Disconnect uses the same rules at height 300. An
apply dialog is height 400, not modal, and does not time out. The only
choices the bridge accepts are `allow` and `cancel`. The consent channel is
the app IPC bus with type `LocalExecConsent` and method `choose`. The
sender must be a top frame whose URL is `file:` in a packaged app, or
`localhost` when the app is not packaged. Chat grants are a third operation,
`confirmDirectMcpChatGrant` / `revokeDirectMcpChatGrant` /
`clearAllDirectMcpChatGrants`. The clear runs on the identity-change hook
named `direct-mcp-chat-grants`. The grant list those functions mutate is
`home.grants`.

Observed. `home.grants` is a connector store. Its directory is

```text
<userData>/connectors/<ownerId>/
```

`<ownerId>` matches `u-` plus 32 hex characters. That id is the first 32 hex
digits of SHA-256 over `namespace`, a NUL, `issuer`, a NUL, and `subject`.
The pointer to the current owner is the key `custom3pConnectorOwner` in
`<userData>/config.json`. That file is the electron-store the app resets by
renaming it `config.json.corrupt-<timestamp>` when it cannot be read. It is
not `claude_desktop_config.json`.

The grants file is `<ownerId>/mcp-oauth.json`, mode `0600`. The object key
is `custom3pMcpOAuth`. A server entry can hold `tokens`, `expiresAt`,
`hosted`, `urlKey`, and `client`. Clearing a server deletes `tokens`,
`expiresAt`, `hosted`, and `urlKey`, and deletes `client` when the clear is
a revoke. The log is `[custom3p-mcp] cleared OAuth tokens`. The same
directory holds `builtin-desired.json`, `mcp-schemas.json`, and a `builtin`
cache directory from the same factory. Those are not the folder-grant file.

## Scheduled tasks

Two stores share a class and a directory layout. The factory calls them
`coworkScheduledTasks` and `ccdScheduledTasks`. Records live under
`app.getPath("userData")/db/<id>/…`. Cowork's instance sets
`taskDirBoundaryUntrusted: true`. CCD's log prefix is `[CCDScheduledTasks]`,
its preference key is `ccdScheduledTasksEnabled`, and a dispatch name beside
it is `claude_code_routines` / `codeRoutines`.

Shared operations, from the preload and from the store methods in the main
chunk: list, read a task file, create, update, update status, remove one
approved permission, get watcher history, mark the listener ready, ack an
event, release a dispatch claim, subscribe to `onScheduledTaskEvent`. The
store persists with `persistToDisk` and emits the task. Adding a folder
dedupes `userSelectedFolders`. Chrome permissions on a task are a mode plus a
domain list (`updateChromePermissions`, `getChromePermissions`,
`clearChromePermissions`). `getNotifySessionId` is the session a task pings.

Cowork adds migration and a bound signature the CCD service does not have:
`beginManualMigration`, `endManualMigration`, `revertRemoteMigration`,
`answerBoundCreateSign`, `boundCreateSignRequested`. CCD adds
`setHostRunsSupported` instead. Dispatch waits on the VM. The wait is capped
by `scheduledTaskVmWaitMaxTicks`. If Yukon Silver's status is not the ready
value the store checks, or the policy helper `ry()` says restrictions apply,
local dispatch is off. Approval lifetime is a day count, and a separate
"stood in" lifetime exists for a person acting for someone else
(`approvalLifetimeDays`, `approvalLifetimeStoodIn`). The exact integers were
not lifted out of the minified constants `IBr` and `LBr`.

`ccd-session-secrets` and `ccdCredential` are log tags on the code side of
the same scheduler. Error categories observed as tag suffixes include
`credential_needs_auth` and `models_not_discovered`.

## What the page can ask an artifact to do

From the artifact document's point of view the host is the `cowork` bridge.
From the host's point of view a tool call is accepted only when the name is
`mcp__<server>__<tool>` and the connector is up. `askClaude` and
`runScheduledTask` are the other two verbs. There is no file picker and no
`LocalSessions` method on that preload. A link that leaves the artifact
scheme is an `openExternalUrl` request the host may refuse. The preload
cannot tell the difference until the invoke returns `refused`.

Office files are a neighbor, not a Cowork service. `OfficeAddinFiles` tracks
connected documents, focuses and selects one, and runs a Porter handoff:
`queuePorterHandoff`, `downloadAndQueuePorterHandoff`, `cancelPorterHandoff`,
`probePorterOpenTarget`, `onPorterHandoffOutcome`. Sensitivity-label rules
that can turn those actions off are in
[enterprise configuration](enterprise.md).
