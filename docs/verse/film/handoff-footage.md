# Launch film footage: handoff (#11432)

State on 2026-10-12 (evening, cut 3). All of it runs locally against the
1.0 stack on this Mac; nothing touches staging or production.

## What is recorded

`~/work/film-out/sync-v2/` (the "one conversation across devices" shot,
reshot with Coder and the iPhone app built from `origin/main` at
`b22acbdb31`, so the terminal shows "Answering a reply from openagents.com"
on the input's bottom rail rather than in the transcript). `sync-v1/` is
the earlier take; its terminal shows the old transcript line.

| File | Size | What |
|---|---|---|
| `terminal.mp4` | 3024×1964, 30 fps, 38.9 s | Coder in `~/code/slugkit`: the person asks why the tests fail, then sees the phone's and the website's replies arrive and Coder act on them |
| `web.mp4` | 3024×1964, same length | The signed-in website: the terminal chat appears in the sidebar, opens, and updates live; the person types the third message there |
| `phone.mp4` | 1206×2622 (iPhone 17 Pro simulator), same length | The iPhone app on Your chats: the chat lists, opens, updates live; the person types the second message there |
| `beats.json` | | Each beat's time in the clips (`clip`) and on the shared clock (`clock`), the cuts, and the three messages |

The three clips are cut alike on one wall clock: a stretch of more than
2 s where nothing changes on any surface keeps 0.7 s at each end and jumps
over its middle, on all three at once, so they play in sync side by side.
Spinners run at real speed (the web spinner changes on about 25 of 30
frames). The phone clip runs the full length: `simctl recordVideo` writes
frames only on change, so the cut holds its last frame to the end of the
shared clock (it was 1.4 s short in `sync-v1`). The terminal's cursor is
steady on every frame (checked by sampling its cell across the clip).

The messages: terminal "Why are the tests failing? Look, but don't change
anything yet."; phone "Fix it and run the tests."; web "Commit it with a
short message." All answers are real (Coder and the chat worker call real
models through the local gateway).

The boot clips are in `sync-v2/` too, on the same account:
`boot-terminal.mp4` (2.4 s: `coder` typed in `~/code/slugkit`, Coder's
home), `boot-web.mp4` (3.5 s: a dark page, then the signed-in website
home), `boot-phone.mp4` (6.4 s: the iPhone home screen, the OpenAgents
icon, the app opening to its signed-in home). `boot.mjs` removes the UI
tests' runner app from the home screen first. `FILM_BOOT=phone` (any of
`terminal,web,phone`) with `FILM_SETUP=WORK/setup.json` records a clip
again on the same account. There is no desktop clip (see below).

Earlier footage: `~/work/film-out/footage/`, `footage-v2/` (including
`phone-app.mp4`, the app's Running screen), `footage-v3/`.

## Feature takes for cut 5 (2026-10-13)

`~/work/film-out/footage-v8/`, 3024×1964, recorded live from `coder-new`
built at `e991114008`+ in `~/code/slugkit` (made afresh by
`make-demo-repo.sh`) at `FILM_TERM_FONT=26`, so the text reads on a MacBook
in a short beat:

| File | Length | What |
|---|---|---|
| `terminal-plugins.mp4` | 12.6 s | `/plugins`, the selection moving down, the ACP Subagents page (seven agents found here), Esc (`coder-plugins.mjs`, `FILM_PLUGIN_DOWNS=4 FILM_PLUGIN_STEP=1.0`) |
| `terminal-harness.mp4` | 13.1 s | Text in the input; Shift+Tab steps the rail Auto → Claude (Opus 5.5 · 5h · week) → Codex → OpenCode → Grok Build → Devin → Cursor → Oh My Pi → Auto. Nothing is sent |
| `terminal-tray.mp4` | 14.9 s | The chat on OpenCode (free `opencode/big-pickle`), `/agent opencode Run the tests…`; the rail row shows the model and each step (`Bash · ls -la…`, `Bash · npm test 2>&1`) |
| `terminal-rewind.mp4` | 22.6 s | A turn on OpenCode edits `src/slugify.js`; `/rewind` lists it with "1 file changed"; Enter restores it and puts the message back |

`coder-features.mjs` (`FILM_TAKE=harness|tray|rewind`) runs them: real HOME,
an isolated `--state` copy without `account.json`/`sync.json`,
`OPENAGENTS_TASKS` under `FILM_WORK`, and `CODER_OPENCODE_MODEL` plus
`OPENCODE_CONFIG_CONTENT` pinning OpenCode to its free model. A finished
background agent's report starts a parent turn, so the tray take pins the
chat to OpenCode first (an Auto chat would answer on Claude Code).

Product fixes found while shooting: `a16d25c70f` (an ACP agent's step on
the rail, not "bash · cwd: /Users/…"), `64e41eb7e0` (/rewind records
OpenCode, Codex and agent turns), `e991114008` (another harness's edit no
longer reads "No changes"), `ad8d018c87` (OpenCode's `filePath` reads as a
path, home as `~`).

## Scripts (`scripts/film/footage/`)

| Script | What |
|---|---|
| `record-all.sh` | The single-surface clips (web-chat, phone-approve, terminal-coder, web-sync, phone-app). Rebuilds the binaries unless `--no-build`, restarts a stale stack, `FILM_IOS=0` skips the phone |
| `boot.mjs` | Makes the test account, the demo repo, and Coder's sign-in (writes `FILM_WORK/setup.json`), then the three boot clips. `FILM_SETUP` reuses an account, `FILM_BOOT` picks clips |
| `sync-shoot.mjs` | The sync shot. Uses `FILM_SETUP` (a `setup.json`) or makes its own account. Keeps the take in `FILM_WORK/take.json`; `node sync-shoot.mjs --edit WORK` re-cuts it |
| `setup.mjs` | Account, demo repo, `coder login` approved on the website, simulator status bar and `recordVideo`, video change detection, cutting a simulator video at given times |
| `phone-app.mjs` | The app's Running screen while Coder works (`--edit RAW` re-cuts) |
| `cdp.mjs`, `web.mjs` | Headless Chrome over CDP: screencast recorder, encoding at shared-clock times |
| `terminal.mjs`, `ptyrun.py`, `replay-terminal.mjs` | Real PTY sessions kept as asciicast (with their wall-clock start), rendered frame by frame in xterm.js with Paper Mono; `replayAt` renders at shared-clock times |
| `coder-plugins.mjs` | `terminal-plugins.mp4`: Coder's Plugins screen browsed live (`/plugins`, the list, the ACP agents page, Esc) on the person's real installs, with an isolated `--state` copy of their settings and no account file, so nothing is changed or synced. Pacing is the recording's own (no waits shortened). Kept in `~/work/film-out/footage-v7/` |
| `make-demo-repo.sh` | The slugkit repository with three failing tests |
| `coder-plugins.mjs`, `coder-features.mjs` | Coder's Plugins screen; Shift+Tab, the agent tray, /rewind (cut 5) |

The phone is driven by `bins/openagents-ios/host/UITests/FilmUITests.swift`
(opt-in): `testRunningForTheFilm` and `testSyncForTheFilm`, the latter
steered by files (`open`, `reply`, `finish`) the shoot writes.

### Resume

```sh
cd ~/work/openagents   # at or after the commit that adds this file
export CARGO_TARGET_DIR=~/work/openagents-target-film
SIM=$(cat ~/work/film-out/.film-sim-udid)
W=~/work/film-out/.work/sync-$(date +%H%M%S); mkdir -p $W
common="FILM_BASE=http://127.0.0.1:4301 FILM_GATEWAY=http://127.0.0.1:8791 FILM_SIM=$SIM FILM_WORK=$W \
  FILM_OUT=$HOME/work/film-out/sync-v2 FILM_CODER=$CARGO_TARGET_DIR/debug/coder-new"
env $common node scripts/film/footage/boot.mjs                                # boot clips
env $common FILM_SETUP=$W/setup.json node scripts/film/footage/sync-shoot.mjs # the sync shot (about 4 min)
FILM_OUT=~/work/film-out/sync-v2 node scripts/film/footage/sync-shoot.mjs --edit ~/work/film-out/.work/sync-170709 # re-cut the kept take
```

The kept take is `~/work/film-out/.work/sync-170709` (470 MB: web frames,
the phone's raw video, the terminal cast, `take.json`), shot on the boot
account in `.work/boot-165735/setup.json` (which also keeps
`boot-terminal.cast`). Older takes are deleted. The iOS app must be built once with
`bins/openagents-ios/build.sh sim` (it also generates the Xcode project the
shoot's UI test needs) (`OPENAGENTS_IOS_DEVICE=$SIM`); the
shoot's UI test builds its own Debug app into
`$CARGO_TARGET_DIR/openagents-ios/DerivedData-film`.

## What is running

- The local stack (`scripts/dev/full-local.sh`, state
  `~/.openagents/film-local`): website `127.0.0.1:4301`, gateway `:8791`,
  chat worker. Restarted 16:24 with binaries built from `origin/main` at
  `9f2a916688`. Left running.
- The simulator: "iPhone" (iPhone 17 Pro, iOS 26.5), UDID in
  `~/work/film-out/.film-sim-udid` (`3E794F7C-1CBE-4842-9CC3-F35D17C20B66`),
  booted.

## Sync paths

| Path | State |
|---|---|
| Terminal → web | Works. The synced chat appears in the sidebar 1.2 s after the message is sent and its thread updates live |
| Terminal → phone | Works. The chat listed in Your chats 1.8 s after the message was sent, and the open chat updates live (the phone reads again every few seconds, so it can trail the web by a second or two) |
| Phone → terminal and web | Works. The reply shows on the web at once and in the terminal about 2 s later. Coder says "Answering your reply from openagents.com." even when the reply came from the phone (small wording issue, not fixed) |
| Web → terminal and phone | Works. Coder took the web reply about 9 s after it was sent in this take, the slowest hop |
| Desktop | **Missing.** The desktop app (`crates/openagents-desktop`) does not sign in to an openagents.com account and does not list account or synced chats; it keeps its own local chats. Its terminal pane can run `coder`, which syncs, but the app itself doesn't show the conversation. Recording its window also needs macOS Screen Recording permission for the process that records (`screencapture` fails with "could not create image from display" here): an owner step |
| Editable by others | **No path.** A chat belongs to one account (`crates/openagents-web/src/chat_owner.rs`); there is no sharing, collaborator, or second-account reply. The only sharing is read-only: a Coder session uploaded as a trace (`coder trace upload`) can be shared at `/trace/{id}` for anyone with the link to read, not to add to. "UI-14" (#11028) is the theme-alignment work, not shared chats. Nothing was faked |

## Product changes made along the way

- `7c6c049f3f`: Coder's "Saving your chats…" notice gave way once saved
  (later replaced by `8b479fd4c4`, which removes the line).

Found, not fixed:
- The terminal's notice for a phone reply says "from openagents.com" (the
  phone does reply through openagents.com, so it's not false).
- The iPhone app draws Coder's `ℹ` test-summary marks as blue emoji
  (U+2139 without a text presentation selector).
- The web chat answers coding questions with a closing line about Coder
  and refuses "write code" asks (the web policy), which shows in web-chat
  takes.
- The phone's Running card timer only advances when the list is read
  again (it can sit at "2s" for several seconds).

## What's left

1. Desktop: account sign-in and synced chats in the desktop app (product
   work), and the Screen Recording permission to film its window.
2. Approve on the phone: needs Coder to ask for a risky ability (a
   production deploy, a PR merge, DNS, secrets, payments). None can be
   staged honestly for the film yet.
3. Second-person editing: needs shared chats in the product.
