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
The record format, the trust file, the pty spawn, and the SSH config path
are the main-process implementation of those names, read from the session
manager chunk and the OpenSSH helper chunk.

## Where a code session record is stored

Observed. The code-session manager is constructed with the directory name
`claude-code-sessions`. Its storage root is

```text
<userData>/claude-code-sessions/<accountUuid>/<orgId>/<sessionId>.json
```

`<userData>` is `app.getPath("userData")`. The account id and the org id are
the ids the manager was initialized with. If either is missing,
`getStorageDir()` returns null and the save logs that the storage path is
not available. The session id is the file name. A legacy file
`<userData>/local-sessions.json` is an array. On load, each element whose
`sessionId` matches `^[a-zA-Z0-9_-]+$` is written to `<sessionId>.json` under
the account directory if that file is not already there, and the legacy file
is then unlinked. An id that fails the pattern is skipped.

Observed. The write is debounced: 1000 ms while the session is not running,
3000 ms while it is. The bytes are `JSON.stringify` of the serializer output,
written atomically (a sibling `.tmp`, then rename) with mode `0600`. The
parent directory is created as a private directory. If that atomic write
fails with `EPERM`, `EACCES`, `EBUSY`, or `ENOTEMPTY`, the same bytes are
written directly. Delete removes `<sessionId>.json` and `<sessionId>.json.tmp`.
A parked session of another account is addressed by that account's directory,
not the current one.

Observed. The object written for one session includes these fields when they
are set: `sessionId`, `cliSessionId`, `cwd`, `originCwd`, `worktreePath`,
`worktreeName`, `folderWorktrees`, `requestedAdditionalFolders`,
`worktreeLazy`, `worktreePinned`, `gitAnchors`, `gitAnchorsLookupOnly`,
`gitAnchorsFolderRealpath`, `lastFocusedAt`, `sourceBranch`, `branch`,
`createdAt`, `lastActivityAt`, `model`, `effort`, `effortInherited`,
`sessionSettings`, `agent`, `isArchived`, `title`, `titleSource`,
`previousTitles`, `permissionMode`, `enabledMcpTools`,
`remoteMcpServersConfig`, `withheldConnectorHosts`, `sshConfig`,
`wslConfig`, `sshRemoteProcessId`, `sshReattach`,
`sshRemoteTranscriptPath`, `prs`, `writtenBranches`, `autoArchiveExempt`,
`autoArchiveOnPrClose`, `isStarred`, `movedToCloud`, `keptDirtyWorktree`,
`keptDirtyAt`, `keptWorktreeLeftover`, `scheduledTaskId`, `spaceId`,
`contextRecovery`, `completedTurns`, `error`, `errorCategory`,
`tccFolderKind`, `forkedFromSessionId`, `forkedAtMessageUuid`,
`importedFrom`, `resumeConfirmed`, `spawnedFrom`, `bridgeSessionIds`,
`cloudSessionId`, `remoteControlSpawn`, `sessionPermissionUpdates`,
`terminalClaudeTabOrdinal`, `cliBinaryPin`, and the cut-turn, rewind, side
session, and scratch fields the serializer copies beside those. A field that
is empty is omitted rather than written as an empty list. The conversation
itself is not this file. The file stores `cliSessionId`. The transcript the
manager looks up is a `*.jsonl` under the Claude config projects directory:
`CLAUDE_CONFIG_DIR` when that variable is set, otherwise `~/.claude`, then
`projects/`. A handoff bundle adds a second file,
`local-session-state.json`, next to the metadata.

Derived. Queued composer input that has not been sent is a separate
waiting-input manifest under the same storage directory, not a field of the
session JSON. The manifest carries `v`, `sessionId`, `savedAt`,
`cleanShutdown`, and `entries`. An unreadable manifest is renamed to
`<name>.unreadable-<timestamp>` rather than parsed.

Third-party deployment shortens Cowork account directories (below). The code
session path above uses the full account uuid and org id. That split is the
two `join` calls, not a guess about which product is running.

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

## How trust is persisted

Observed. Workspace trust is not a field of `claude-code-sessions`. It is a
key in the Claude config JSON:

```text
~/.claude.json
```

If `CLAUDE_CONFIG_DIR` is set, the file is `<that directory>/.claude.json`
instead. The loader stats the file and reuses a frozen projection while the
mtime and size match. A file at or above the worker threshold is parsed off
the main thread by a request named `claudeJsonProjection`. A failed worker
falls back to a main-thread read. The projection the desktop keeps per
project path is `hasTrustDialogAccepted`, `remoteTrustGrant`,
`remoteControlSpawnMode`, the stdio MCP server names, and
`disabledMcpServers`. Other keys in the file are left in the on-disk object.
They are not all copied into that projection.

Observed. A check for an absolute directory resolves the path, then realpath,
then walks parents. Any `projects` entry on that walk with
`hasTrustDialogAccepted` true is trusted. A non-absolute key, including an
`ssh:` key that does not start with `ssh:[`, is trusted only when that exact
key has `hasTrustDialogAccepted` and the remote-grant check accepts
`remoteTrustGrant`. Saving trust sets `hasTrustDialogAccepted` true. When the
save is endpoint-keyed it also writes

```text
remoteTrustGrant: { keying: "endpoint", grantedAt: "<ISO time>" }
```

The write takes a directory lock `<file>.lock` (mode `0700`, treated as
stale after 10 seconds, given up after 15). It copies `<file>.backup` mode
`0600`, then atomically replaces the JSON. The in-memory projection is
invalidated before and after the write. The desktop checker memoizes a
result for 300,000 ms and clears that memo, the command memo, and the agent
memo on save.

Observed. Before the dialog, the checker lists dangerous features it could
read: `.claude/settings.json` and `.claude/settings.local.json` under the
directory, and the same two names under a root the path walk reached, when
those files are regular files inside the walk. If the walk itself cannot be
checked, both names are reported without opening them. A `cwd` that is only
a symlink of the real directory is refused with a message that tells the
caller to pass the real folder, so the approval is stored on the real path.
A warm start is skipped when the trust check throws, when the folder is not
trusted, or when the folder is a link. The log lines are `[CCD] Skipping
warm` and `desktop_ccd_trust_check_miss`. The prompt that is not answered in
time is `trust_prompt_timeout`.

The main process also refuses to treat a symlink as a CCD boundary
(`desktop_ccd_below_boundary_symlink_refused`) and has a separate event for a
hardlinked file read (`desktop_ccd_hardlinked_file_read`).

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

### How a pty is tied to a working tree

Observed. `ShellPtyManager.sessionCwd` starts from `worktreePath`, and if
that is absent, from `cwd`. Those are the fields on the session record. If
the session also has `harnessCwd` and the manager was given
`grantableHarnessCwd`, that function may replace the directory; a refusal
falls back to the worktree or `cwd`. `startShellPty` resolves the pty key to
that session. A missing session returns `{ok: false, error: "Session not found"}`.

Observed. The host process posts one message to the utility worker:

```text
{ type: "spawn", ptyId, file, args,
  opts: { name, cols, rows, cwd, env,
          windowsFallbackFile, scrollbackLogDir, scrollbackRedact } }
```

`cwd` is the directory from the paragraph above. `env` is the session shell
environment with undefined values removed. A local interactive shell is
started as the login shell (`-l`) with that `cwd`. The worker calls
`node-pty` `spawn(file, args, { name, cols, rows, cwd, env })`. If that
throws and a Windows fallback file was supplied, it posts `spawn-fallback`
and spawns that file instead. Otherwise it posts `spawn-error` and drops the
scrollback log. Later messages are `write`, `resize`, `kill`,
`read-scrollback`, `dispose-scrollback`, and `shutdown`. Data, a later pid,
and exit come back as `data`, `spawned`, and `exit`.

Observed. Scrollback is optional and gated by the flag whose id is
`1923867086`. When it is on, the directory is

```text
<userData>/terminal-scrollback/run-<pid>-<base36 timestamp>/<16 hex chars>/
```

The run directory is one per manager lifetime. Each pty gets its own 8-byte
random subdirectory, created mode `0700`. The worker appends numbered
segments `<generation>.log`, opened `ax` mode `0600`. A segment holds at most
4,194,304 characters. At most 16 segments are kept. A backlog over 4,194,304
characters is dropped and the read inserts `[... earlier output not kept ...]`.
Strings in `scrollbackRedact` are replaced with `*` before the write. Reads
strip terminal control sequences, return at most 102,400 characters, and can
grep with two lines of context, at most 40 matches. Dispose deletes the pty
directory. When a shell command finishes, the session manager emits a
working-tree change with cause `terminal_command` (or `bash_mode_command`
for the one-shot bash pty). That is the link from the pty back to git
refresh. The pty does not write the session JSON.

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

## How an SSH config line becomes a connection

Observed. The desktop does not parse `~/.ssh/config` as the only source of
truth. It asks the ssh binary which files that binary would read:

```text
ssh -vvv -G -- <host>
```

Stderr must contain `debug1: Reading configuration data`. Each such line's
path is the file OpenSSH opened. If the command fails, is killed, or never
prints that line, the result is `system_file_unknown`. The user's own
`~/.ssh/config` path is built from the home directory. A path whose UTF-8
length is 200 bytes or more, or that contains a replacement character, is
the same `system_file_unknown` error. The home path is not used when it
cannot be read.

Observed. For a connection that must carry a user and must include the
user's config, the helper builds one buffer:

```text
Host <host>
  User <user>
Host *
Include <config path>
```

`<config path>` is the `-F` file when the spawn already has one, otherwise
`~/.ssh/config`. The host and the user are passed through the same escape
used for the generated file. The write is refused when the host matches
`[*?!,@]`, or when an explicit `-F` path fails
`^[A-Za-z0-9_./+:@=, -]+$`. Included file bytes are appended under further
`Host *` lines after a check that the include is not a system file the
helper refuses to copy. A separate probe,
`ssh -F <sentinel> -G -l <user> -- localhost`, detects an ssh that rejects
`-F`.

Observed. `ssh -G` stdout is read as `key=value` lines. The keys the helper
keeps include `user`, `hostname`, `port`, `identityfile`,
`identitiesonly`, `identityagent`, `proxycommand`, `proxyjump`,
`certificatefile`, `knownhostscommand`, `verifyhostkeydns`,
`revokedhostkeys`, `hostkeyalias`, `checkhostip`, `hashknownhosts`,
`updatehostkeys`, and `nohostauthenticationforlocalhost`. An `identityagent`
value of `SSH_AUTH_SOCK` is not stored as a path. Any other value is stored
as `identityAgent`. `proxycommand` and `proxyjump` values of `none` are
dropped. `knownhostscommand` of `none` is dropped.

Observed. The agent socket is chosen as follows. If `identityAgent` is
absent, `$SSH_AUTH_SOCK`, or `SSH_AUTH_SOCK`, the helper uses the probed
socket. If the value is `none`, the result is `config_none` and no socket is
used. Any other value is a configured path (`config`). A path is accepted
only when `stat` says it is a socket. The probe of the login shell is

```text
$SHELL -l -c 'printf %s "$SSH_AUTH_SOCK"'
```

with `SHELL` defaulting to `/bin/zsh` and a 5 second timeout. If that prints
a path different from the inherited `SSH_AUTH_SOCK` and the path is a
socket, the login-shell path is preferred. The log tag is
`[sshAgentSocket]`. A failed `env -0` from the shell-path worker is split on
newlines so those bytes are not handed to ssh as environment entries.

Observed. The keyword table that decides what may be copied onto the ssh
command line marks `IdentityFile` and `CertificateFile` as path lists,
`IdentityAgent` as an expanded path, `ProxyJump` as a token, and
`ProxyCommand` as raw. `KnownHostsCommand` is in the `never` class. If the
command line would skip `Match` host blocks that an interactive terminal
would apply, and those blocks change identity files or the proxy, the helper
logs `[OpenSSHConnection] the command line skips N Match host block(s)` and
does not apply the skipped lines.

Observed. A shell pty on an SSH session is refused before spawn when
`sshHost` is not `^[A-Za-z0-9.@_:-]+$` or starts with `-`, when `sshPort` is
present and not an integer, or when the remote cwd contains a control byte,
a backslash, a Windows drive or UNC prefix, or a smart quote. An identity
file that starts with `-` or contains a control byte is dropped, and the
refusal is logged. The session record stores the resulting `sshConfig` on
the JSON described above. Reconnect uses that stored config, expands a
leading `~` with the remote home, and ensures the remote controller is ready
for `terminal_open` or `bash_command` before the pty spawn.

Observed. A saved password is not a field of the session JSON and not a line
of `~/.ssh/config`. The store is

```text
<userData>/ssh-remembered-passwords.json
```

mode `0600`, tab-indented JSON. The key is `ssh:<host>:<port>`, with port
22 when the session did not set one, or `wsl:<distro>` in lower case. Under
`entries`, each key holds `c` and `savedAt`. `c` is the base64 form of
`safeStorage.encryptString` applied to
`{secret, kind, prompt, hostKeyFingerprints, savedAt}`. `kind` is
`password` or `keyboard-interactive`. That encryption path runs only when
`safeStorage.isEncryptionAvailable()` is true and the selected backend is
not `basic_text`. The variable `CLAUDE_E2E_SSH_PLAINTEXT_SAVED_PASSWORDS`
is the path that writes the object in the clear, under
`e2ePlaintextEntries`, and only when encryption is off.
`getSSHSavedPassword` returns `{saved, savedAt}`. It does not return
`secret`. A remembered secret is replayed only when this connection's
verified host-key fingerprint is in `hostKeyFingerprints` and the prompt
text and kind match. Otherwise the log says the remembered password is not
used, and the person is asked. `forgetSSHSavedPassword` deletes that key.
Managed settings turn the feature off with `disableSshSavedPasswords`.

Observed. Each connection gets its own control directory. The candidate
roots are the process temp directory and `/tmp`. Under a root the parent is
`ccd-ssh.<uid>`, or `ccd-ssh` when the process has no uid. That parent is
accepted only when it is a directory owned by this uid and its permission
bits are exactly `0700`. The connection directory is created with the prefix
`ccd-ssh-`. The control socket is the file `ctl` inside it. A root is
skipped when that socket path would be longer than 86 bytes, and the log
names the length and the limit. An `owner` file, mode `0600`, is created
exclusive and holds `{pid, instanceId}`. A sweep removes stale directories
whose marker is at least 15 seconds old. The sweep does not run when the
process has no uid. The directory is removed when the connection ends.

Observed. OpenSSH is pointed at an askpass helper written into that
directory as `askpass.sh`, mode `0700`, beside fifos `p.fifo` and `a.fifo`
created mode `0600`. The spawn sets `SSH_ASKPASS` to the script and
`SSH_ASKPASS_REQUIRE` to `force`. If `DISPLAY` is unset it becomes
`none:ccd-ssh-askpass`. The script treats `SSH_ASKPASS_PROMPT` as
`confirm`, `none`, or `prompt`. It writes its pid, that kind, and the
prompt text to `p.fifo` as NUL-separated fields, and reads answers from
`a.fifo`. A line `<pid> Y…` is the answer. `<pid> N` is a refusal. A missing
fifo on a prompt sends `SIGTERM` to the parent ssh and exits 1. The same
directory also gets `ap.sock` and a 32-byte hex token for the relay helper.
A pipe frame that is not JSON is dropped, and the log is
`[OpenSSHAskpass] unparseable pipe frame`.

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

`LocalAgentModeSessions` is the Cowork session bridge. The session file, the
folder-grant file, the sessions-bridge process, the Copper Heron hold, and
the direct-MCP grant file are in [Cowork](cowork.md). Lifecycle mirrors the
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
