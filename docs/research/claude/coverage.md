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

## Surfaces inventoried and not read

- `ion-dist` stopped after `assets/v1/cef1e6037-DkhsUYfO.js`. The HTML
  names are known. Their scripts are not.
- `.skill` zips, `office365-mcp-stdio.mjs`, `pdf.worker.mjs`,
  `pdfExtractorProcess.mjs`, and the locale JSON files were not opened as
  text.
- `compile-cache/*.jsc` was not decoded.
- `document.wflow` was not decoded.
- Electron Framework was not decompiled. The fuse record, version, exports,
  and linkage are known. The Chromium implementation is not.
- `swift_addon.node`, `github-mcp-server`, `claude-ios-sim`,
  `SimulatorFrameworkBridge-iOS`, and Squirrel have linkage and, where
  present, compiler metadata. They do not have procedure-level overviews.
- `pty.node` and `msal-node-runtime.node` have arm64 overviews and do not
  have a recovered Go or Rust metadata record. Their Mach-O type and full
  import tables were not normalized the way the other binaries were.
- The `smol-bin` images are exFAT volumes of about 23 and 24 MB. Their
  filesystems were not mounted, so a VM-disk conclusion stays an inference
  from the filename and from `swift_addon.node`'s Virtualization and
  `vmnet` linkage.

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

## Questions left open

1. Which of the 14 window constructions have no HTML file, and which build
   files are only modules loaded by the six documents.
2. The full strings of the dynamic IPC channels, and the names of the three
   main-process handlers.
3. Where `@ant/computer-use-mcp`, `@ant/claude-for-chrome-mcp`,
   `@ant/imagine-server`, and `@loc/electron-window` land inside the hashed
   bundles.
4. Whether `ion-dist` is the in-app browser pane, a Cowork frame, or both.
   The shim's `pane` reply and the `frame-*.html` names suggest a framed
   browser surface. The files were not tied together.
5. The guest image layout inside `smol-bin.*.img`, and whether
   `swift_addon.node` mounts those files.
6. The concrete Seatbelt allow-list's effect on CoreSimulator, which the
   profile's own comment says can only be checked by running it.
7. Whether the unpacked-metadata mismatches are the usual ASAR placeholder
   for files that also live in `app.asar.unpacked`, or a real content
   disagreement. The hashes differ. The cause was not proven by comparing
   the placeholder bytes inside the archive with the unpacked files.
