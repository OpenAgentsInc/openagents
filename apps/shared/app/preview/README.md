# The phone app's fast loops

Three loops for working on the iOS chat page (GitHub #13645). All three are thin
wrappers over one Coax tool, `loop.coax`, and all three generate only when a
Coax input changed.

| Command | What you get | Needs |
| --- | --- | --- |
| `apps/shared/app/preview/shot.sh OUT.png` | A PNG of the app's screen, drawn on the Mac | Metal; no Xcode, no simulator |
| `apps/shared/app/preview/watch.sh` | The same PNG, redrawn on every save | as above |
| `apps/openagents-ios/dev.sh` | The app rebuilt, installed and relaunched in the simulator, and a screenshot | Xcode, a simulator |

## The preview on the Mac

`carrier.c` is the iOS app's own carrier: it includes
`components/gallery/linux/main.c` with the macros the app builds with, the app's
`host.c` and the generated library `apps/openagents-ios/Generated/gallery.c`.
The page's state, layout and paint are the same compiled Coax and the same
display list the phone runs. Only the surface differs: the Mac's Metal device
draws into a texture at a phone's size and scale (3x by default), and the
picture is written as a PNG. `--shot` opens no window, no Dock icon and no menu.

```bash
apps/shared/app/preview/shot.sh /tmp/chat.png                    # iPhone 17 size, 393x852 points at 3x
apps/shared/app/preview/shot.sh --size small /tmp/se.png          # small, phone, large, or WxH in points
apps/shared/app/preview/shot.sh --large-text --text "Hello" /tmp/big.png
apps/shared/app/preview/shot.sh --keyboard 336 /tmp/kb.png        # the keyboard's height, in points
apps/shared/app/preview/shot.sh --case my.case --tree /tmp/tree.txt /tmp/after.png
apps/shared/app/preview/shot.sh --all /tmp/set                    # phone.png small.png large.png large-text.png
```

A case file holds one `coax drive` operation a line (`tap ID`, `type ID TEXT`,
`clock +MS`, `tree`, ...); `#` lines and blank lines are skipped. The page
answers an operation first; a `tap` or `type` it does not know goes the way the
phone sends it (a press on the row, the platform editor's whole text). `--tree`
writes the semantic table after the case, so a check reads meaning, not pixels.

`watch.sh` takes the same options plus `--out DIR` (default
`apps/shared/app/preview/build/watch`), `--all` and `--window`. It polls the
modification times of every source `coax deps` names for the app's root and
the shader, the carrier and the host files, four times a second. A save starts
a pass; a save during a pass starts the next one. `--window` also opens the
page live in a window on the Mac, where the pointer is the finger, and replaces
it after each pass. The window is a convenience: the PNG at the phone's scale is
the picture to compare.

## What runs, and when

- **Generate** (Coax to C and Coax to Metal) is `apps/openagents-ios/generate.sh`,
  one `remote.sh run` job: on the Mac unless it is busy (docs/metaverse/remote-builds.md, "Here first"). It runs only when the SHA-256 over every source `coax deps`
  names, `generate.sh` and the compiler's digest differs from
  `apps/openagents-ios/Generated/.inputs`. All three loops share that stamp, so
  a shot after a simulator build (or the other way round) does not generate
  twice.
- **Compile** (the preview's C on the Mac: the generated library, the shared
  Apple host and the carrier, about 1.7 s from nothing) recompiles only the
  object whose inputs changed, each by its own hash in `build/*.inputs`, and
  relinks only when an object changed.
- **dev.sh** also hashes `project.yml` and the file names beside it, and runs
  `xcodegen` only when they changed. `xcodebuild` builds incrementally through
  `coax/remote/remote.sh run --local` (the Mac's build slot), into a
  derived-data folder for each simulator under `~/.openagents/ios-dev/`, shared
  by every worktree. It waits for another `xcodebuild` on the Mac to end before
  it builds. Then `simctl install`, `simctl launch --terminate-running-process`,
  a settle of 1.5 s (`--settle MS`) and `simctl io screenshot` (`--shot OUT.png`;
  default `apps/openagents-ios/build/dev/screen.png`). No test suites run.

### Which simulator dev.sh uses

dev.sh uses the booted `Coax Native Starter` (the simulator
`apps/openagents-ios/README.md` names) unless it is contested, then the spare
`OpenAgents Dev`, then `OpenAgents Dev 2`. It makes a spare that is missing,
boots it, and prints the one it used (`dev: simulator NAME (UDID)`). A
simulator is contested when:

- another dev.sh holds its lock, `~/.openagents/ios-dev/sim-NAME.lock`, which
  each dev.sh holds until it ends;
- an `xcodebuild` names it, or a `simctl install` or `launch` names it or, while
  it is booted, `booted` (from `ps`; only the program counts, not a shell whose
  script mentions one);
- an app that is neither Apple's nor ours runs on it (`launchctl list` inside the
  simulator);
- our app runs on it and another worktree's dev.sh launched it in the last 15
  minutes (`~/.openagents/ios-dev/sim-NAME.owner` holds the worktree and the
  time of the last launch).

`--sim NAME` (or `--device NAME`) takes that simulator only: it says why it is
contested and uses it anyway, unless another dev.sh holds its lock.

Every loop prints each step and its time.

## Timings

Measured on 2026-11-16 on the owner's Mac (build host `oa-build-1`), on the current one-screen app. "Edit" is a change to a Coax source of the
app (`apps/openagents-ios/chat.coax`); each figure is from the save to a PNG of
the new screen.

| Loop | Edit to pixels | Nothing changed |
| --- | --- | --- |
| Before: `build.sh`, then `simctl install`, `launch`, `screenshot` by hand, derived data warm | 14 s | 5 s |
| Before, in a new worktree (derived data starts empty under `apps/openagents-ios/build`) | about 16 s (estimated: a cold `xcodebuild` is 5.3 s) | |
| `dev.sh`, derived data warm | 12.7 s | 3.3 s |
| `dev.sh`, first run (empty derived data) | 15.2 s | |
| `dev.sh` on the spare when the default is contested: boot 18 s the first time, then as above | | |
| `shot.sh` | 9.1 s | 0.4 s |
| `watch.sh`, a pass after a save that changes nothing generated | | 0.4 s |

Where the time goes in an edit:

| Step | Time |
| --- | --- |
| Generate, one build-host job | 7.6 to 7.8 s, of which the compile itself is about 1 s; the host's checkout and compiler setup is 5 s and sending the snapshot 1 to 2 s |
| Generate since #13645 (same day): on the Mac when it is not busy | 0.17 to 0.45 s; on a host when it is, 2.5 to 3.7 s |
| Preview compile on the Mac (the generated library) | 0.9 s |
| Preview draw | 0.3 to 0.5 s |
| `xcodebuild`, incremental | 2.9 s (1.1 s with nothing to compile) |
| Install, launch and settle | 2 s |

So the generate job is most of every loop, and most of the job is the build
host's per-job setup, not Coax. Splitting the C and Metal stamps buys nothing:
`coax metal` takes 0.03 s. Cutting the job's setup belongs to `coax/remote/`.

## Hot reload

The app is Coax compiled to C and linked in. There is no JavaScript, no
interpreter and no over-the-air code, and the loops add none. We looked at two
ways to keep the running page and swap its code:

- **Load a rebuilt library into the running preview** (`dlopen` the generated C
  as a dynamic library on each pass). It saves the preview's relaunch, about
  0.3 s of a 9 s loop. The page's state record is a C struct whose layout
  changes with most edits, so keeping state across a swap is unsafe without a
  migration step; a reload that drops state is what `watch.sh` already does.
- **Load new code into the app in the simulator.** That is runtime code
  loading in the iOS app, which the lean app forbids for the same reasons it
  forbids JavaScript and Updates (`apps/openagents-ios/check.sh`). We do not do
  it, even in Debug.

The state a picture needs comes from the case file and the options instead:
the text, the keyboard, the size, the text scale and the operations that led
there. That makes every picture reproducible, which a hot-reloaded session is
not. If the generate job's setup falls to near zero, the loop is about 2 s
without any reload.

## Files

| File | Role |
| --- | --- |
| `loop.coax` | The tool: input hashes, the three loops, step timings |
| `carrier.c` | The Mac carrier around the app's page (Metal texture or window) |
| `shot.sh`, `watch.sh` | Wrappers that run `loop.coax` through `coax/web/tool.sh` |
| `../../../openagents-ios/dev.sh` | The simulator loop's wrapper |
| `INTEGRATION.md` | What changes here when the app's root or carrier changes |
| `build/` | Objects, the `Preview` program and stamps (ignored) |
