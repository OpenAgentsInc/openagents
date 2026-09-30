# Pairing milestone check, 2026-09-29

The milestone of [#9965](https://github.com/OpenAgentsInc/openagents/issues/9965),
step 9 ([#9974](https://github.com/OpenAgentsInc/openagents/issues/9974)): a
phone pairs with the desktop app's code, sees the computer connected, starts
Coder there from chat, runs a read-only command card, and is cut off when the
computer removes it. Run on the owner's Mac, which already runs an old-style
Coder host with phones paired to it; that host was not touched.

## Setup

- **Computer.** Release builds of `coder`, `microcoder`, `openagents`, and
  `openagents-desktop` from this change, run from one folder (as
  `Contents/MacOS` holds them in the app). The host ran as
  `coder host serve --iroh --control --keys DIR --label "OA Test Mac"` with
  its own `HOME`, so its access store, task store, control socket, iroh key,
  and file key source were all new and separate from the owner's host. The
  window ran as `openagents-desktop --no-login-agent` with the same `HOME`,
  so it talked to that host's socket and registered no login agent.
- **Coding agents.** Codex and Claude Code are both signed in on this Mac.
  The Codex account was at its usage limit until 2026-10-03, so the runs
  used Claude Code (`claude:claude-opus-5-5`), the second route the desktop's
  switch now admits. The test `HOME` linked the owner's Codex folder, Claude
  Code binary, login keychain, and Jev key file; see the gaps below.
- **Phone.** A new iPhone 17 Pro simulator (iOS 26.5) on a debug build of
  this change, and a new Android 15 emulator (arm64) on a debug APK. Both
  were deleted afterwards. The simulator has no camera, so the code went in
  with **Connect a computer > Paste a code**, from the desktop's **Can't
  scan? Copy a code instead**.
- **Project.** A scratch Git repository with one commit.

## Results

| Step | iPhone simulator (direct iroh path) | Android emulator (iroh relay only) | Screenshots |
| --- | --- | --- | --- |
| The desktop app shows the code, with **Let this phone open a terminal on this Mac** set first | Pass | Pass | `desktop-01-code-terminal-allowed.png`; after ten idle minutes the code hides, `desktop-01-idle-code-hidden.png` |
| **Connect a computer** opens `SCR-22` with **Paste a code** and the desktop download line | Pass | Pass (camera permission refused, paste used) | `ios-01-computers-empty.png`, `ios-02-connect-a-computer.png` |
| Pasting the code pairs; the phone shows **Connected** and the computer's name | Pass (after fix 1) | Pass | `ios-03-connected.png`, `android-01-connected-over-relay.png`; before the fix, `ios-03-before-fix-done-invisible.png` |
| The phone lists the computer as online | Pass, **Online · Internet** | Pass, **Online · Internet** | `ios-04-computers-online.png`, `android-02-computers-online.png` |
| The desktop shows the phone | Pass (after fix 2): **Your phone is connected.**, then the phone under **Phones** with **terminal** | Pass | `desktop-02-your-phone-connected.png`, `desktop-03-home-phone-and-tasks.png`; before the fix, `desktop-02-before-fix-empty-name.png` |
| Picking the project and turning on **Let my phone start Coder here** | Pass (after fix 3) | Same host | `desktop-02-host-worktree-coder-on.png`; before the fix, `desktop-02-before-fix-project-refused.png` |
| A chat message offers **Run Coder on OA Test Mac**; the tap starts a task that runs on the Mac | Pass (after fix 4) | Pass | `ios-05-run-coder-offer.png` |
| Coder's steps and reply stream into the chat | Pass (after fix 5) | Pass | `ios-08-streaming-strip.png` (working, then done), `ios-07-coder-reply.png`, `android-05-run-coder-working.png`, `android-05-run-coder-done.png`; before the fix, `ios-06-before-fix-loading-the-chat.png` |
| A read-only command card (`openagents verse who`) runs on the Mac | Pass (after fix 6) | Pass | `ios-09-command-card-offer.png`, `ios-10-command-card-ran-on-mac.png`, `android-06-command-card-ran.png` |
| A terminal opens on the Mac | Pass, **Connected directly** | Not run | `ios-11-terminal-open.png` |
| **Remove** on the desktop cuts the phone off | Pass: the host records the grant revoked, the phone lists the computer as **Revoked**, and the next command card says **Connect a computer to run this.** | Pass | `desktop-03-confirm-remove.png`, `desktop-03-after-remove.png`, `ios-12-after-remove-card-refused.png`, `ios-13-after-remove-revoked.png`, `android-07-after-remove.png` |

### The relay-only path

The host ran with `--iroh-bind 127.0.0.1:0`, so its only iroh socket was on
the Mac's loopback, which the Android emulator cannot reach (its
`127.0.0.1` is its own). The host's only other connections were TCP to
`iroh.openagents.com` (34.136.30.163:443) and the Nostr relay. The emulator
still paired over iroh (the host issued the chat invitation, which only an
iroh redemption carries), ran Coder, and ran the command card, so every
byte between them went through `iroh.openagents.com`.

The iPhone run is the direct path: the simulator shares the Mac's network,
and the phone shows **Connected directly** on its terminal.

## What this change fixed

1. **Done was invisible** on the phone's **Connected** screen: a
   white-tinted prominent button with a white label
   (`bins/openagents-ios/host/App/ConnectView.swift`).
2. **The desktop showed " is connected."** with no name: a phone that pairs
   by scanning gives the host no name. It now says **Your phone**
   (`crates/openagents-desktop/src/screens.rs`), with new snapshots.
3. **A picked project could not run Coder.** The folder a person picks holds
   its own Git directory, and the auto-start policy refuses a writing
   workspace that is not an isolated worktree, so the switch failed with
   **Couldn't change that setting.** The host now admits a detached
   worktree of the folder's current commit under its root's `projects/`
   (`crates/coder-host/src/control/mod.rs`). The desktop also clears that
   message on the next try.
4. **Only Codex could run.** The switch admitted only `codex:gpt-6-luna`,
   so a Mac with only Claude Code signed in, or a Codex account at its
   limit, stopped every task with **No model capacity**. The switch now
   admits Codex then Claude Code; each start takes the first one signed in
   with capacity.
5. **The phone could not read the task.** A QR-paired phone had no chat
   pairing, so a Coder chat stayed on **Loading the chat** even after the
   task finished. The iroh enroll reply now carries a single-use
   `coder-pair:` chat invitation beside a grant, as tailnet admission's
   answer does, and the phone pairs it (`crates/openagents-connect`,
   `crates/coder-host`, `crates/coder-computers`, `crates/openagents-mobile`;
   NIP-HOST and `INVARIANTS.md` updated).
6. **Command cards need `openagents` on the Mac.** The app bundle now ships
   `openagents` beside `coder`, and a host with `openagents` beside it puts
   that folder first on its terminals' `PATH`
   (`scripts/desktop/package-macos.sh`, `bins/openagents-desktop-macos/bundle.sh`,
   `crates/coder-host/src/serve/mod.rs`). Before, the card ran whatever
   `openagents` the shell found, here an unrelated older binary.

## Gaps found and not fixed here

- **A fresh Mac also needs a Jev key.** `microcoder repository` builds its
  Jev judge from `TYPESAFE_API_KEY` or `~/.openagents/jev.json` and fails
  the launch without one (`no Jev key`); the task then stays **Waiting to
  start** on both screens. Codex or Claude Code being signed in is not
  enough. This check used the owner's key file.
- **A task whose owner process never admits it** stays queued forever on
  the phone and the desktop instead of ending with the diagnostic.
- **After the host restarts** (it does on every project change), a command
  card on an open phone says **Lost: the computer restarted** until the app
  is relaunched.
- **Claude Code outside `~/.local/bin`.** The engine finds `claude` on the
  launch `PATH` (`/usr/bin:/bin`) or in `~/.local/bin`, so an npm or Homebrew
  install is not found when the host runs as a login agent.
- The desktop shows the host's worktree path, not the folder the person
  picked.
- A QR-paired phone's chat grant lasts 29 days, and a code redeemed on the
  Nostr relay (when iroh cannot connect) carries no chat invitation. Nearby
  pairing ([#9975](https://github.com/OpenAgentsInc/openagents/issues/9975),
  landed while this ran) carries none either, so a phone paired that way
  cannot yet read the Coder chats it starts.
- Debug simulator builds only: a **Run Coder** that falls back to the Nostr
  relay overflowed the 512 KiB stack of the app's Rust dispatch queue in
  the WebSocket handshake. Release builds were not affected in earlier
  TestFlight runs of the same path.

## Checks run

- `cargo test -p coder-host --test control --test iroh`: 14 passed,
  including the new
  `a_phone_that_pairs_over_iroh_gets_a_chat_invitation_and_a_refused_one_does_not`
  and the extended `a_project_change_is_recorded_and_asks_the_host_to_start_again`
  and `auto_start_changes_run_the_hosts_own_command`. One earlier run had
  `a_terminal_opens_over_iroh_only_with_the_terminal_right` fail once under
  load and pass on three reruns.
- `cargo test -p openagents-connect -p openagents-desktop -p coder-computers`:
  all passed, with two new desktop snapshots.
- `bins/openagents-ios/build.sh sim` and
  `scripts/build-openagents-android.sh package` built this change.
