# Shared smart terminal: Linux checks and Mac handoff

Status: implementation checked on Linux; macOS package publication and the live
Grid/standalone demonstration are **not run**. The owner asked to switch to the Mac
when ready. This receipt does not claim a signed package, public install, live model
answer, Mac capture, or Mac performance pass.

## Checked here

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
