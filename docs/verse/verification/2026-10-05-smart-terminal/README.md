# Shared smart terminal: Mac run and Linux checks

Status: on the Mac, the owner tested a local build and directed a refactor
before release: Retina text, one smart input line, and the binding
[design principles](../../../terminal/design-principles.md) (one fixed 3:2
sheet, anchored regions, function keys, ASCII only, no Markdown). The
refactor is implemented and checked below. **Signing, publication, the public
install, and the Mac stress run are paused** until the owner approves the
refactor; nothing was signed or published. The Linux checks and the original
handoff follow.

## Mac, 2026-10-05: the sheet

Machine: MacBook Pro, Apple silicon, built-in Liquid Retina XDR display
(backing scale 2). Release builds with the pinned toolchain from this
commit's tree. The local app for the owner to test is an ad hoc signed bundle
of `openagents-terminal` with the matching `openagents` helper beside it.

### What changed

- **Retina.** Before the change, a 2x window capture showed the standalone
  already drew text at the backing scale (glyph atlas at 16 x 2 px, surface in
  physical pixels). Text now also draws unhinted at Retina densities, as macOS
  draws text, and both surfaces rasterize again and refit when the scale
  changes (moving between displays). The Grid's atlas previously stayed at
  its first scale.
- **One input line.** The terminal owns the input line. At ENTER it
  classifies the line locally (`terminal_core::route`, from the shell's
  `PATH`, aliases, functions, builtins, and the line's structure) as a
  command or a question. The label before the line shows the decision as you
  type. No `#` prefix or mode key is needed; F5 and F6 override.
- **The sheet.** One fixed 1200 by 800 point window that cannot be resized;
  the Grid draws the same sheet anchored at the screen's center. Regions: a
  three-row status area, the transcript with a scroll bar always drawn, the
  input line, and the key strip. No floating preview, no automatic thread
  split, no blocking "finish or dismiss" line (a second question queues).
- **No leaked instructions or JSON.** How to answer moved into the system
  instructions (`basic_coder::INSTRUCTIONS_TERMINAL`, selected for OpenAgents
  Terminal's caller); the visible turn carries only the question and its
  attachment. The helper removes the typed plan from the answer, the terminal
  removes any plan line again, translates Markdown to plain text, and maps
  every character to ASCII. A plan shows as `PROPOSED: ...`, and ENTER and
  ESC confirm or reject it. The confirmed command's result returns to the
  same thread as "I ran `...` as you proposed; its output is attached."

### Live flow on both surfaces

[`terminal_sheet`](../../../../crates/verse/examples/terminal_sheet.rs) runs
the shared terminal on a real zsh in a scratch home, with the release
`openagents` helper answering live, and renders each stage offscreen at
scale 2. Keys are key events delivered in that process, labeled simulated:
the Mac was in use, so no window was brought to the front and no physical
key was pressed. Each run used a fresh
[`terminal-demo-fixture.sh`](../../../../scripts/release/terminal-demo-fixture.sh)
repository and an external Cargo target; the scratch home, its device key,
and its threads were deleted at the end, so nothing reached the owner's
thread lists.

| Stage | Standalone sheet | Grid sheet |
| --- | --- | --- |
| Idle | [window-idle.png](mac-sheet/window-idle.png) | |
| `cargo test` failed, exit 101 | [window-failed.png](mac-sheet/window-failed.png) | |
| `why did that fail`: answer and typed proposal | [window-asked.png](mac-sheet/window-asked.png) | |
| First ENTER on a command that may change files | [window-confirm-again.png](mac-sheet/window-confirm-again.png) | |
| Confirmed: new block and same-thread answer | [window-confirmed.png](mac-sheet/window-confirmed.png) | [grid-confirmed.png](mac-sheet/grid-confirmed.png) |
| Full-screen program (`top`) in the transcript region | [window-program.png](mac-sheet/window-program.png) | [grid-program.png](mac-sheet/grid-program.png) |

[window-run.json](mac-sheet/window-run.json) and
[grid-run.json](mac-sheet/grid-run.json) retain each run's transcript
entries and the scratch thread identities. In both runs the request carried
block 2 (`cargo test`, exit 101), the live answer explained the failed
assertion in plain ASCII, the proposal was `cat src/lib.rs`, the confirmed
command ran as block 3, and the same thread answered from its output (the
standalone run also proposed a fix, left pending). Requests went through the
local in-process client (`DOOR local`).

### Checks

- `terminal-core`: 16 tests, including the example table for the input line
  (`git status`, `ls -la`, `cargo test`, `./run.sh`, `cd ~/x`, `FOO=1 make`,
  `make it faster`, `why did that fail`, `how do I undo my last commit`, `fix
  the failing test`, `explain this error`, `find all TODOs in src`,
  `echo "hi"`, and `python3 -c '...'`), ASCII output from programs and the
  model, Markdown translation, the status area on every row of a 120 by 40
  sheet, no instructions or JSON in the request or the transcript, CONFIRM
  and REJECT, queued questions, and the earlier offline proposal flow.
- `terminal-gfx`: 27 tests (the panes view); `terminal-app`: the fixed,
  non-resizable 3:2 window; `openagents` helper: plan removal from the
  visible answer and the existing plan and deny-list rules; `openagents-chat`
  `basic_coder` tests; `verse-gfx` tests; release builds of the app, the
  helper, Verse, and both terminal examples.

### Latency

Key-to-glyph on the input line, release build, 400 keys each:

| Surface | p50 | p95 | max |
| --- | --- | --- | --- |
| Standalone sheet | 0.17 ms | 0.29 ms | 1.42 ms |
| Grid sheet | 0.12 ms | 0.15 ms | 0.47 ms |

The measure runs from the key event to the next frame's sheet drawn on the
CPU, every vertex built, before GPU submission; presentation adds the
display's refresh wait. The window build also records key-to-presented-frame
times with `--latency-out FILE` for a visible run.

### Release credentials found (not used)

Signing and publishing were stopped by the owner's direction, not by a
missing credential. On this Mac the login keychain holds a `Developer ID
Application: OpenAgents, Inc.` identity, `~/work/.secrets/appstoreconnect.env`
holds the `ASC_API_*` notary variables and `OA_DEVELOPER_ID_APPLICATION` (the
desktop release's `--notary-env` file), and `CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config`
lists the release bucket, whose `openagents-terminal/` prefix is still empty.
`native-terminal.py` needs Python 3.11 or later (`tomllib`); the system
`python3` is 3.9, so use `~/.local/bin/python3.12`. A pre-refactor stage
built from `fae8c18a22` was set aside unsigned and is not a release.

### Not done on the Mac

- Signing, notarization, publication, and the install from the public URL.
- The shared stress workload (8 busy panes) and repeated startup samples:
  both need a visible window on a computer someone is using.
- A physical key press on a live window, and window captures from the screen.

## Linux checks (before the Mac run)

The Grid adapter and native window mount `terminal-core` and `terminal-gfx`.
The standalone normal dependency graph contains no Verse application/world;
Verse's nonterminal surface graph contains no native terminal/window adapters.
The core contains no window, renderer, or network dependency.

- Core: 9 tests, including a failed block, scrubbed preview, one request, typed
  pending proposal, physical Enter, same-thread result, and duplicate acknowledgment.
  The bridge in that fixture is injected and offline, not a live model answer.
- Renderer/native adapter: 27 tests, including real temporary-HOME zsh hooks,
  physical approval, full-screen modes, Unicode, copy/selection, splits, and output budgets.
- `verse-gfx`: 26 tests; control/app test targets pass. Grid terminal-only and
  desktop consumers compile; app builds and opens a native window without Verse.
- Release/installer admission: 7 offline tests. They exercise missing verdicts,
  changed helpers/archive/checksums, existing versions, public-readback mismatch,
  and incomplete installation. Apple/cloud calls are mocked, not release evidence.
- The actual scratch Rust fixture exits 101; `linux-failing-cargo.txt` retains it.
  Formatting and diff checks pass. The seven-platform TUI release script is unchanged.

`linux-verification.json` records the base commit, changed source hashes, executable
SHA-256, toolchain, and omissions. The source hashes identify the issue-10644 build
above the recorded main commit; `compiled_commit: null` in the startup report is
expected for this development build.

## Linux measurements

The Wayland debug executable ran the shared stress driver with 8 busy panes,
15 recorded seconds, 4 seconds of warm-up, and a typing key every 150 ms.
The workloads remain `yes`, large-file replay, colored build-log replay, `seq`,
and full-screen `top`. Advisory block marks are enabled; Linux uses its `top`
flags, and the Mac uses the original BSD flags. Standalone has no world/spells.
The Grid run retains town, Wind Walls, and Meteor Swarm.

The existing p95 frame target remains **16.7 ms**; it is not relaxed. The Linux
report records whether it is met. This debug run is not the supported Mac release
verdict. `world_ms` in the shared report means all nonterminal frame time, including
GPU presentation; in standalone it does not imply a simulated world.

The new startup target was frozen before measurement at **2,000 ms** from process
entry before argument parsing to the first GPU frame submitted and presented.
`linux-standalone-startup.json` records one development sample, not a startup p95.
The Mac needs repeated release samples. No Grid performance run or capture was
performed here after extraction.

## Run on the Mac

Use macOS arm64, Python 3.12+, the pinned Rust toolchain, Xcode command-line tools,
Developer ID signing/notary credentials, and the existing bucket's authorized
`CLOUDSDK_CONFIG`. Fetch main first; build and both helpers come from its archived
commit. Set an absolute reusable `CARGO_TARGET_DIR` outside the checkout.

```sh
git fetch origin main
python3 scripts/release/native-terminal.py build --commit origin/main
python3 scripts/release/native-terminal.py sign --stage dist/releases/openagents-terminal/1.0.0-rc.2
python3 scripts/release/native-terminal.py publish --stage dist/releases/openagents-terminal/1.0.0-rc.2
```

Signing uses `OA_DEVELOPER_ID_APPLICATION` plus `NOTARY_KEYCHAIN_PROFILE`, or the
existing three `ASC_API_*` variables. It signs each helper and the app with the
existing hardened-runtime entitlements, notarizes, staples, validates, and assesses
Gatekeeper. Publishing refuses an existing version and creates objects with a
zero-generation precondition under `openagents-terminal/<version>/`; it reads the
archive, manifest, checksums, and installer back through public HTTPS. It does not
move a TUI channel. Keep `publication-receipt.json`, the signed manifest, and sums
beside the Mac evidence. Use a new committed version if a version is already present.

Download the versioned installer from that public prefix and run it with Python:

```sh
python3 install-native-terminal.py --version 1.0.0-rc.2 --destination /tmp/native-terminal-installed
fixture=$(scripts/release/terminal-demo-fixture.sh)
"/tmp/native-terminal-installed/OpenAgents Terminal.app/Contents/MacOS/openagents-terminal" --root "$fixture" --shell /bin/zsh
```

Use a fresh destination. The installer checks manifest/archive SHA-256, all three
executable hashes, signature validity, and Gatekeeper before installing the app.
Verse must be absent from the installation. In both this window and a Grid mount
with temporary HOME/state, run `cargo test` using an external Cargo target, inspect
and copy the failure block, type `# why did that fail`, inspect/remove the scrubbed
context, and submit. Request a typed command if the answer offers only prose.
Open the same observing thread; capture the pending proposal before physically
pressing Enter (and the second confirmation when required). Capture its new block
and same-thread continuation. Repeat an acknowledgment and confirm no second run.
Use separately authorized scratch credentials/host state; archive every smoke
chat/task afterward. Do not use or leave chats/services in the owner's normal lists.

Capture a split with a full-screen program, tabs, Grid/world focus, and hide/reopen.
Hiding retains local PTYs; process exit ends them. Retain source/executable hashes,
actual install command, captures, and live request/proposal/result identities with
secrets and private content removed.

For both surfaces, retain the shared workload reports (8 busy, 15 seconds, 4 warm-up):

```sh
openagents-terminal --stress-out native-busy8.json --startup-out native-startup.json --busy 8 --seconds 15 --warmup 4
cargo run --release -p verse --example terminal_stress -- --busy 8 --seconds 15 --warmup 4 --out grid-busy8.json
```

Run the installed native binary for its report. Repeat startup measurements, record
p95 against the frozen 2,000 ms target, and retain failed targets as failed. Keep the
original frame/typing targets from the [performance receipt](../2026-10-05-terminal-performance/README.md).
Update the machine-readable omissions only after each actual check runs; open a
new defect issue if Mac verification finds a problem.

The signing sequence follows [Apple's notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
Immutable upload uses [Cloud Storage generation preconditions](https://docs.cloud.google.com/sdk/gcloud/reference/storage/cp).
