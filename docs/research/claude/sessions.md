# Sessions, code, and the local agent

Two session services sit on `claude.web`. `LocalSessions` is the Claude Code
desktop session: a working tree, a shell, git, and ssh. `LocalAgentModeSessions`
is the Cowork / local-agent session: folders the user has trusted, a bridge
back to a remote or VM-side agent, skills, and the watch-record hold called
Copper Heron. They share MCP, transcript, and permission verbs. They do not
share a store. A third, smaller surface, `ClaudeCode`, reads sandbox and
workflow flags without owning a transcript.

The preload lists 337 methods on `LocalSessions` and 130 on
`LocalAgentModeSessions`. The groups below are those names, read from the
preload, arranged by the job they do. Calling one of them was not attempted.

## A code session

Start and recovery: `start`, `retryStart`, `warmSession`, `prewarmAuth`,
`prewarmLocal`, `prewarmFolderless`, `discardPrewarm`, `canAutoResume`,
`confirmImportedSessionResume`. Stop and interrupt: `stop`, `interrupt`,
`stopTask`, `getTaskOutput`. A session can be forked (`forkSession`,
`detachFromParent`), archived, unarchived, deleted, or updated. Side work is
`startSideChat`, `sendSideChatMessage`, `stopSideChat`, `interruptSideChat`,
`startBtwChild`, `closeBtwChild`, `acceptSideSessionOffer`,
`muteSideSessionOffers`.

The composer is `sendMessage`, `cancelQueuedMessage`, `reorderQueuedMessage`,
`promoteQueuedMessage`, `sendSteeredNow`. Model controls are `setModel`,
`setEffort`, `getDefaultEffort`, `setFastMode`, `applyModelByRestart`,
`setThinkingSummariesWanted`, `setAvailableNextActions`,
`setAvailableCodeModels`, `resolveCodeSessionModel` (the last of those is a
log tag in the main process, not a preload method). Context is
`getContextUsage` and `getContextUsageSummary`.

Permission mode is `setPermissionMode`, `getDefaultPermissionMode`,
`getChipLevelPreference`, `setChipLevel`, `setChipLevelPreference`. Trust of
the working directory is `checkTrust`, `checkStoredTrust`, `saveTrust`,
`changeCwd`, `answerPendingCwdTrust`. A folder the OS will not let the app
read is `classifyTccFolderKind` and `openMacFilesAndFoldersSettings`. Tool
prompts come back through `respondToToolPermission`,
`respondToRefusalFallbackPrompt`, `respondToAutoModeServerFallbackPrompt`,
`respondToLanyardConsent`, and `respondToViolinBowPrompt`. `ViolinFret` is a
separate two-method service (`setViolinFretChoice`, `violinFretStatus_`).
The name is only a name; the prompt it answers is the violin-bow consent.

Rewind is `rewind`, `rewindV2`, `rewindToRefusedPrompt`, `rewindFiles`,
`undoRewind`, `switchRewindMark`, `discardPendingTurn`, `clearSession`,
`resumePreClearSession`. Worktrees: `getUncommittedChanges`,
`getUncommittedChangesByFolder`, `discardKeptWorktree`, and a startup call
`startKeptWorktreeReaper` in the main process. Scratch workspaces are
`createScratchWorkspace`.

The transcript is paged: `getTranscript`, `getTranscriptTail`,
`getTranscriptPage`, `readTranscript`, `requestTranscriptPort`,
`searchSessions`. The port is a `MessagePort`, not a second IPC channel.
List and identity: `getSession`, `getAll`, `getSessionList`,
`loadArchivedSessions`, `findLocalSessionIdForBridgeId`,
`getSessionsForScheduledTask`, `setVisibility`, `setFocusedSession`.

UX timing is reported rather than computed in the page: switch timing, switch
initiated, switch abandoned, session-list hint, composer INP, transcript
backfill, stream render, run follow-up, dead end, artifacts reset, session
list load. Those methods are counters. They do not change the session.

## Files and the shell inside a session

Reads stay in the session root unless a method says otherwise: `readFileAtCwd`,
`readSessionFile`, `readSessionFileWithStatus`, `resolveSessionFile`,
`isSessionPathGone`, `listSessionDirectory`, `watchSessionDirectories`,
`pickSessionFile`, `pickFileAtCwd`. Writes are `writeSessionFile` and
`createSessionFile`. Placement suggestions are `suggestFolderForSessionFile`,
`addSuggestedFolderForSessionFile`, `getSessionFilePlacement`. Reveal and
open: `showSessionFileInFolder`, `showSessionFilePreview`,
`openSessionFileInDefaultApp`, `openInEditor`, `getInstalledEditors`.

Images and other media have two representations. `readSessionImageAsDataUrl`
and `readSessionMediaAsDataUrl` return bytes. `getSessionImageStreamUrl` and
`getSessionMediaStreamUrl` return a URL, which is what `claude-media:` is
for. Panel variants of the same four methods exist for the side panel.

The terminal is not a utility-process secret from the page's point of view.
`startShellPty`, `writeShellPty`, `resizeShellPty`, `stopShellPty`,
`getBusyShellPtyKeys`, and `setShellPtyTitle` are `LocalSessions` methods.
The process that actually owns the pty is `pty-host/ptyHostWorker.js`. One-shot
commands are `runBashCommand` and `interruptBashCommand`, separate from the
pty. `getSupportedCommands` and `getSSHDraftCommandNames` tell the composer
which slash commands exist for this session.

`DocumentFunnel` is the scratch-document path beside a session:
`ensureScratchRoot`, `writeScratchFile`, `listScratchWorkingFiles`,
`ingestSessionDocument`, `injectDocumentContext`, `runClarkdownConvert`,
`runClarkdownDownloadExport`, `revealDownloadExport`, `openDownloadExport`.
Clarkdown is the name of the convert/export step in the method list. The
converter's implementation was not read.

`FileSystem` is the app-wide picker, not the session root: browse file or
folder, list a directory, read and write a local file, thumbnail, "show in
folder", which application owns an extension, save a paste, write a download
and optionally open it, export a local file to Google Drive, and
`promoteScratchpadFile`.

## Git and GitHub

Local git on `LocalSessions`: `getGitInfo`, `getLocalBranches`, `getGitDiff`,
`getGitDiffStats`, `getDiffFileContent`, `getDiffFileImage`,
`getGitDiffFilePatch`, `getGitCommits`, `getCommitDiff`, `refreshGitState`,
`invalidateGitDiffCache`, `isWorkingTreeDirty`, `getWorkingTreeDirtyState`,
`getWorkingTreeStatus`, `stashWorkingTree`, `discardWorkingTree`,
`commitWipForBranchSwitch`, `checkBranchSwitchConflict`, `isBranchOnOrigin`.
`GitState` on the same world is the small snapshot subscription
(`getGitStateSnapshot`, `gitStateChanged`) used by the sidebar, not the full
diff API.

GitHub, through the `gh` CLI rather than the bundled GitHub MCP server:
`checkGhAvailable`, `installGh`, `listGhIssues`, `getGhIssue`,
`getGhRefSummary`, `getGhUserSummary`, `getPrStateForBranch`, `getPrOverview`,
`getPrTimeline`, `getPrFiles`, `getPrCommits`, `getPrFileContent`,
`getPrChecks`, `getCheckRunAnnotations`, `rerunPrCheck`, `getPrReviewComments`,
`replyToPrReviewComment`, `resolvePrReviewThread`, `updatePrComment`,
`createPrReviewComment`, `submitPrReview`, `updatePrBody`, `mergePr`,
`markPrReady`, `markPrDraft`, `enableAutoMerge`, `disableAutoMerge`,
`dequeuePr`, `setBoundPrAutoFix`, `armAutoFixOnNextPr`,
`setAutoFixOwnPrsWanted`, `setSessionAutoArchiveOnPrClose`, `dismissBoundPr`,
`invalidatePrCaches`, `fetchGithubImage`. A second forge is named on
`getCrRefSummary` and `getCrCliStatus`. `ForgeState` is the sidebar snapshot
for that (`getForgeStateSnapshot`, `refreshForgeStateRow`,
`setForgeStateVisibleKeys`, `forgeStateChanged`).

Background tasks the session offers the user: `backgroundTasks`,
`setBackgroundTasksShortcutArmed`, `popBackgroundTaskSuggestion`,
`popBackgroundTaskSuggestionById`, `getTaskBacklog`, `dismissTaskBacklogItem`,
`notifySpawnedTaskStarted`, `notifySpawnedTaskUnstarted`. Output styles and
agents: `listGlobalOutputStyles`, `createOutputStyle`,
`validateNewOutputStyleName`, `cancelCreateOutputStyle`, `revealOutputStyle`,
`getAgents`, `createAgent`. `launchUltrareview` and `reviewRepository` are
their own entry points. `submitFeedback` and `respondToTitleOffer` close the
session-level loops that are not git.

## SSH, WSL, and a remote machine

SSH is a group of `LocalSessions` methods, not a separate service.

Connection: `testSSHConnection`, `probeSSHConnection`, `reconnectSessionHost`,
`forceReconnectSSH`, `getSSHProxyOutput`, `getSSHLocalClientCapture`,
`getSSHAuthSummary`, `getSessionSSHRemedies`, `getRemoteSSHRemedies`,
`runSSHRemedy`. Config: `getSSHConfigs`, `setSSHConfigs`, `resolveSSHSettings`,
`getTrustedSSHHosts`, `setTrustedSSHHosts`, `getSSHWorktreeLocation`,
`setSSHWorktreeLocation`, `getSSHWorktreeDefaults`. A saved password:
`getSSHSavedPassword`, `forgetSSHSavedPassword`, and the prompt events
`onSSHPasswordRequired`, `respondToSSHPassword`,
`getPendingSSHPasswordRequests`, `ackSSHPasswordRequestShown`,
`reportSSHPasswordPromptHidden`, `declareSSHPromptKinds`,
`onSSHPasswordRequestWithdrawn`, `onSSHPasswordQueueChanged`. Per-host tool
grants: `getSSHHostToolGrants`, `removeSSHHostToolGrant`,
`clearSSHHostToolGrants`.

The main process refuses to treat a symlink as a CCD boundary
(`desktop_ccd_below_boundary_symlink_refused`) and has a separate event for a
hardlinked file read (`desktop_ccd_hardlinked_file_read`). SSH config lines
that say `identityagent` are parsed. A value that is `$ENV` is expanded from
the environment; an unknown agent name is dropped. `SSH_AUTH_SOCK` is stated
and accepted only when it is a socket. The log when shell-environment
extraction fails says the values are split on newlines so ssh is not handed
them. An error string `cannot_scrub_all_ssh_hosts` is in the same chunk. The
scrubber's full rule was not reconstructed. `sshPolicy` is a log tag beside
it.

Remote git mirrors the local git methods with `getRemote*` and `getSSH*`
prefixes, plus `listRemoteDirectory`, `listSSHDirectory`, `checkRemoteTrust`,
`checkRemoteTargetTrust`, `saveRemoteTargetTrust`, `revealRemotePath`.
WSL: `listWslDistros`, `canStartWslSession`. Those names are in the macOS
build. Whether a macOS process can start WSL was not tested; the methods
exist on the bridge either way.

Cloud move is `getTeleportReadiness`, `getTeleportPlan`, `teleportToCloud`,
`resolveCloudBranch`, `resolveGithubRepoIds`, plus the small `CloudMove`
service (`setCloudMoveListening`, `cloudMoveCommand`,
`reportCloudMoveResult`). Remote control of a session the desktop is serving
is `RemoteControlServing`: `requestEnable`, `disable`, `requestAddFolder`,
`requestServeSessionFolder`, `requestServeGrantedFolder`, `removeFolder`,
`setFolderPinned`, `setFolderWorktree`, `disconnectCloudSession`,
`remoteControlServeStatus_`. `toggleRemoteControl` and
`getRemoteControlStartState` live on `LocalSessions` itself.
`dropsHostReplyStamp` and `shareSession` are there too.

CLI sessions on the same machine: `listCliSessions`, `readCliTranscript`,
`listResumableCliSessions`, `resumeCliSession`, `previewCliResume`,
`importAllCliSessions`, `probeImportableCliSessions`, `enableCliRemoteControl`,
`openCliTail`, `closeCliTail`. The menu-driven import is `CliSessionImport`
(`claimMenuStart`, `startFromMenu`, `submitAction`, `cliImport_`).
`createLogsHandoffBundle` packages logs for that session.

Tear-off halo, a Swift group on the native addon as well as three session
methods: `beginTearOffHalo`, `setTearOffHalo`, `endTearOffHalo`. The native
chapter lists the Swift group. The session methods are how the page asks for
it. `SessionPopouts` stores and restores the layout of those torn-off panes
(`reportOpenLayout`, `reportOpenPopouts`, `takeRestorableLayout`,
`takeRestorablePopouts`).

MCP inside a code session: `setMcpServers`, `replaceRemoteMcpServers`,
`replaceEnabledMcpTools`, `getLocalMcpServers`, `getSessionCliMcpServers`,
`mcpAuthenticate`, `mcpReconnect`, `mcpRetryConnection`,
`mcpSubmitOAuthCallbackUrl`, `mcpAuthenticateConfigServer`, `mcpClearAuth`,
`mcpEnableServer`, `mcpCallTool`, `mcpReadResource`, `mcpListResources`,
`getProxiedConnectorCall`, plus the connector-toggle and remote-start
handshakes (`sessionConnectorToggleStarted`, `respondSessionConnectorToggle`,
`remoteStartMcpConfigStarted`, `respondRemoteStartMcpConfig`).

Events the page subscribes to: `onEvent`, `sidebarCommand`,
`sessionViewRequest`, `codeSettingRequest`, `startTargetsRequest`,
`sessionDirectoryChanged`, `onToolPermissionRequest`. The matching completions
are `completeSessionViewRequest`, `completeCodeSettingRequest`,
`completeStartTargetsRequest`.

`LocalSessionEnvironment` stores env for a session (`get`, `save`, `getScoped`,
`saveScoped`, `moveIntoScopes`, `removeScoped`). `LocalSessionPresets` is
`load` and `replaceAll`. `Resources.setFocusedCwd`, `setFocusedSshTarget`,
and `setFocusedRemoteTarget` tell search which root is current.

## The local-agent session

`LocalAgentModeSessions` is the Cowork session bridge. Lifecycle mirrors the
code session where the job is the same: `start`, `sendMessage`, `setModel`,
`setEffort`, `setExtendedThinking`, `setPermissionMode`, `stop`, `stopTask`,
`rewind`, `rewindV2`, `archive`, `delete`, `updateSession`, `getSession`,
`getAll`, `getTranscript`, `searchSessions`, `shareSession`. `getSessionMissReason`
explains a get that returned nothing.

Folders are the trust boundary. `getDefaultWorkspaceFolders`,
`setDraftSessionFolders`, `addFolderToSession`, `addTrustedFolder`,
`browseAndAddTrustedFolder`, `removeTrustedFolder`, `isFolderTrusted`,
`requestFolderTccAccess`. Remote sessions have a second grant list:
`grantRemoteSessionFolders`, `browseAndGrantRemoteSessionFolder`,
`grantRemoteSessionFolder`, `clearRemoteSessionFolderGrants`,
`replayChatFolderAccessReports`, `ackChatFolderAccessReport`, and the events
`onChatFolderAccessChanged`, `onRemoteFolderConsentDialogShown`. Project
upload is `grantProjectUploadFolder`, `listProjectUploadFolder`,
`readProjectUploadFile`, `releaseProjectUploadFolder`,
`stageRemoteSessionAttachmentsViaFilesApi`,
`uploadRemoteAttachmentsToFilesApi`, `releaseRemoteAttachmentUploads`.
Routine folders, the ones a scheduled task may reuse, are
`grantRoutineFolders`, `allowRoutineFolders`, `revokeRoutineFolderGrant`,
`getRoutineFolderGrant`. Delivered files can be auto-saved
(`autoSaveDeliveredFiles`).

The bridge to the agent environment: `getBridgeConsent`,
`getSessionsBridgeEnabled`, `setSessionsBridgeEnabled`, `resetBridge`,
`resetBridgeSession`, `abandonBridgeEnvironment`, `kickBridgePoll`,
`deleteBridgeSession`, `deleteBridgeAgentMemory`, `refetchMyAccess`,
`respondBridgePermissionPreflight`, `onBridgePermissionPreflight`,
`sessionsBridgeStatus_`. `signalSessionIntent` and `setSpSectionPrompts` set
prompt state. `onCoworkFromMain`, `onRemoteSessionStart`, and
`onCuEmergencyStop` are the events a Cowork or computer-use run pushes.

Skills on disk: `syncSkills`, `listLocalSkills`, `getLocalSkillFiles`,
`saveLocalSkill`, `saveLocalSkillArchive`, `renameLocalSkill`,
`setLocalSkillEnabled`, `deleteLocalSkill`, `revealLocalSkill`,
`respondSlashMenuSkills`, `onFolderSkillsCapped`. Directory servers and
plugin search have their own respond methods
(`respondDirectoryServers`, `respondPluginSearch`).

Direct MCP from this session, as opposed to the config file the settings
service edits: `authorizeDirectMcpServer`,
`authorizeDirectMcpServerWithAccountPicker`, `disconnectDirectMcpServer`,
`disconnectDirectMcpServerV2`, `getDirectMcpServerStatuses`,
`directMcpCallTool`, `directMcpReadResource`, `directMcpListResources`,
`getManagedAskToolNames`, `onDirectMcpServerStatusesChanged`,
`onManagedAskToolNamesChanged`. Interactive auth is
`triggerInteractiveAuth`, `revokeInteractiveAuth`, `cancelInteractiveAuth`,
`interactiveAuth_`. Chrome for a remote session is
`callChromeToolForRemoteSession` and `closeRemoteSessionTabGroup`.
`setChromePermissionMode` is the per-session switch the scheduled-task store
also keeps.

Watch recording on this service is the hold the teach window's pill refers
to. `startWatchRecording`, `takeHeldWatchRecording`, `dismissWatchRecordPill`,
`watchRecordingComplete`. Copper Heron is the same hold seen as frames:
`takeCopperHeronSummary`, `takeCopperHeronThumbnails`, `takeCopperHeronFrame`,
`touchCopperHeron`, `finalizeCopperHeron`, `discardCopperHeron`,
`copperHeronHoldDropped`. `releaseRemoteCuLock` drops a computer-use lock held
for a remote session. `getAppIconForBundleId` is how that UI shows the app
it recorded.

`ClaudeCode`, the small third service: `prepare`, `getStatus`,
`checkGitAvailable`, `resolveLocalSettings`, `patchUserSettings`,
`setEnableWorkflows`, `getPeriodUsage`, `setLocalSandboxEnabled`,
`setStrictSandboxMode`, `getLocalSandboxStatus`. Sandbox is a local setting
with a strict mode, not a second VM. The VM is `ClaudeVM`, described in
[the VM chapter](vm.md).

`DeviceRegistry` signs the stamps a remote session is asked to present:
attestation preimage, session header, create-session bind, session bind,
conversation bind, project-thread link, trigger op, trigger digest, session
request, session file copy. It also rotates a key, reports the local device,
and can mark a row keyless (`setKeylessBridgeRowPk`) or tell the server the
bridge is absent. The signatures are method names. The key material was not
read, and no signature was produced.
