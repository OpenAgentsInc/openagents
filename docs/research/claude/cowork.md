# Cowork

Cowork is a surface inside the same shell as chat and code, plus a Linux VM,
plus a document type the app serves itself. The route family is `cowork`,
`local_sessions`, `cowork_sessions`, `task`, `space`, and `epitaxy`. The VM
is how a task gets a Linux machine. The `cowork-artifact:` scheme is how a
task's rendered output is shown without giving that document the main view's
bridge. This chapter is the services and the document protocol. The boot
sequence of the guest is [the VM chapter](vm.md).

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
