# Coverage

This chapter records what the installed build did not yield, so a later
reader can tell a missing feature from an unfinished look. Counts here are
the same counts as [application](application.md). They are repeated because
each zero is a limit on the claim, not a product fact by itself.

## Not established, because nothing was run

- No process was started. No window was opened, no IPC message was sent,
  and no computer-use, Cowork, Buddy, or Simulator session was observed.
- `~/Library/Application Support/Claude` was not opened. Conversations,
  tokens, cookies, and keychain items are outside this report.
- The browser-shim socket was not contacted. The protocol in
  [bundle](bundle.md) is the script's behavior on paper.
- The iOS Simulator seatbelt profile was not applied. Its comments describe
  an intended sandbox and say the rules were fitted by running the helper.
- The Developer ID signature was read from the main Mach-O snapshot. The
  whole bundle was not re-checked as a filesystem trust evaluation. The
  plist integrity hash and the whole-file ASAR hash differ; both are in
  [scope](scope-and-evidence.md). Which bytes the fused integrity check
  hashes was not executed.
- The ten unpacked natives whose archive metadata disagrees with observed
  bytes are described from the observed files. Which of the two digests
  Electron loads at runtime was not observed.

## JavaScript graph is partial

The archive parse covered 426 files with zero parse failures, then stopped
expanding semantics while hashed bundles were still outstanding.
Consequences:

| Summary count | What was actually recovered |
| --- | --- |
| 14 browser windows | 6 HTML renderer documents, plus window-named build files |
| 52 context-bridge APIs | 19 nodes, four of them with literal member lists |
| 10 literal IPC channels | 1 literal channel node |
| 3 main-process handlers | 37 retained handler nodes, all renderer listeners |
| 4 utility processes | 0 resolved entrypoints; worker file names were not joined to the 3 dynamic `new Worker` nodes |
| 0 preload entrypoints | No preload graph. The absence is coverage, not a proof that no preload exists |
| 0 native-addon bindings | 6 native-addon file nodes, with no resolved `require` |

Endpoint nodes are candidates. They did not produce a hostname. Hashed
chunks and `heavyWorkWorker.js` kept static facts only.

## Read as text after the graph stopped

The preload files, the `claude://` handler, the VM controller, the artifact
protocol, the frame-shell HTML, and the managed-config schema were read from
the shipped bytes. So were the code-session manager, the local-agent
manager, the pty host worker, the OpenSSH helper, and the direct-MCP
authorize handler. Those chapters replace the "not read" status of the
window, IPC, session-file, trust, pty, SSH, grant, bridge, and hold
questions below. They do not replace a run.

Still not read as documents:

- Hashed modules under `ion-dist/assets/v1/`. The HTML that loads them, and
  the inline boot scripts, were read.
- `.skill` zips, `office365-mcp-stdio.mjs`, `pdf.worker.mjs`,
  `pdfExtractorProcess.mjs`, and the locale JSON files.
- `compile-cache/*.jsc`.
- `document.wflow`.
- `file-index-worker` and `heavy-work-worker` past their file names and sizes.
- The body of `directMcpHost.js` past the MCP method strings and the OAuth
  token type. The file is mostly an SDK.
- Electron Framework's Chromium and V8. The fuse record, version, and linkage
  are known.
- Procedure-level contents of `swift_addon.node`, `github-mcp-server`,
  `claude-ios-sim`, `SimulatorFrameworkBridge-iOS`, and Squirrel.
- The files inside the exFAT `smol-bin.*.img` volumes. The controller copies
  a `smol-bin.<arch>.vhdx` by that name. No `.vhdx` was in the bundle listing
  this pass walked. The images were not mounted.

## Inferences that should stay labeled

- The rustc commit `48a229ceaefd4985c50990b14116b6d856af0985` is one path
  string shared by `claude-native-binding.node`, `chrome-native-host`, and
  `app-cu-helper`. It is not a crate version and not a single build id.
- The Go module `usernet` is a `c-archive` record inside the Swift
  `swift_addon.node`. The addon remains a Swift dylib that contains that
  record.
- Go module versions in `github-mcp-server` (`v1.6.0`, MCP Go SDK
  `v1.7.0-pre.1`, `go-github` `v89.0.0`) are embedded build info, with VCS
  revision `7629b6e7337e1fc4b11a0ca5fdea4af4edd21f6e`.
- `devDependency` names such as `claude-agent-sdk`, `ssh2`, and
  `playwright-core` are manifest text. The running surface is the files in
  the archive and the native images, not the dev-dependency list.
- Bridge hypotheses that pair every helper with hundreds of scripts are a
  projection rule. They are not a call graph.

## Questions the later reading closed

1. The six shell HTML files are the Electron windows. Computer-use teach,
   the watch-record chooser, the design window, Cowork artifacts, and the
   page preview are further `BrowserWindow`s whose documents are a temp file,
   a `cowork-artifact:` response, or a preview partition. Buddy has its own
   HTML. The graph's "14 windows" count was a constructor count, not 14 HTML
   files.
2. IPC channels are the `$eipc_message$_<uuid>_$_…` spelling in
   [architecture](architecture.md). Two UUIDs exist: the app bus and the
   `electron_window` bus. The graph's three unnamed main-process handlers
   were not given those names by the graph. The handlers that were read are
   the service methods on the far side of that spelling.
3. `ion-dist/index.html` is the claude.ai page inside the main view.
   `frame-shell.html` is the artifact frame. Launch-preview partitions and
   the shim's `pane` reply are the in-app browser. They are three surfaces,
   described in [web and preview](web-and-preview.md).
4. `@loc/electron-window` is the second IPC UUID and the `WindowManager`
   methods. `@ant/computer-use-mcp`, `@ant/claude-for-chrome-mcp`, and
   `@ant/imagine-server` still have no package directory. Computer use and
   Chrome are native addons plus the services in
   [computer use](computer-use.md) and
   [extensions](extensions-and-integrations.md). An imagine server was not
   found as a named process.
5. A code session record is
   `<userData>/claude-code-sessions/<account>/<org>/<sessionId>.json`.
   Trust for that folder is `hasTrustDialogAccepted` in `~/.claude.json`
   (or `$CLAUDE_CONFIG_DIR/.claude.json`). The pty cwd is the session
   worktree, then `cwd`, unless a grantable harness cwd replaces it.
   Scrollback, when flag `1923867086` is on, is under
   `<userData>/terminal-scrollback/`. SSH config is what `ssh -vvv -G`
   prints, then a generated `Host` / `Include` buffer. Saved passwords are
   `safeStorage` ciphertext in `<userData>/ssh-remembered-passwords.json`.
   The askpass helper is `askpass.sh` in a per-connection `ccd-ssh-`
   directory. Details are in [sessions](sessions.md).
6. A Cowork session record is
   `<userData>/local-agent-mode-sessions/<account>/<org>/<sessionId>.json`,
   or `agent/<sessionId>.json` when `sessionType` is `agent`. Folder grants
   are keys of `preferences` in `<userData>/claude_desktop_config.json`.
   The sessions bridge is an in-process client. It persists
   `<userData>/bridge-state.json` keyed `<orgUuid>:<accountUuid>` and polls
   `GET /v1/environments/<id>/work/poll`. Copper Heron is one in-memory hold,
   hashed with SHA-256 over the finalized blocks, not a frame file. Direct
   MCP OAuth state is `<userData>/connectors/<ownerId>/mcp-oauth.json`.
   Details are in [Cowork](cowork.md).
7. `cannot_scrub_all_ssh_hosts` is a failure while building a Cowork
   feedback log bundle, not a trust decision. The reason tags beside it are
   `ssh_configs`, `merged_settings`, `user_settings`,
   `user_settings_hosts`, `device_managed_settings`, `name_turned_away`,
   and `listing_failed`. On that error the bundle is built again without
   logs, and `desktop_cowork_feedback_logs_left_out` records which tag was
   present.

## Questions still open

1. The guest filesystem inside `smol-bin.*.img`, and whether the `.vhdx` the
   controller copies is produced from those images at runtime or downloaded
   inside `claudevm.bundle`.
2. The Seatbelt profile's effect on CoreSimulator. The profile's own comment
   says that can only be checked by running it.
3. Whether the unpacked-metadata mismatches are the usual ASAR placeholder
   for files that also live in `app.asar.unpacked`, or a real content
   disagreement. The hashes differ. The placeholder bytes inside the archive
   were not compared with the unpacked files.
4. Which `https` requests `protocol.handle("https")` answers itself, and
   which it forwards. The artifact-document branch was read. The rest of the
   handler was not.
5. The enforcement branch for each managed-config key that is only named in
   [enterprise](enterprise.md). The keys the boot path consults are the ones
   that chapter says the boot path consults.
6. Swift method names inside each addon group. The loader lists the groups.
   The TypeScript declarations are not in the archive. Call sites in the main
   chunk were not enumerated per group.
