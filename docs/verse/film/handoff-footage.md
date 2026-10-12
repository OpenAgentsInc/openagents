# Launch film footage: handoff (#11432)

State on 2026-10-12, when work paused. All of it runs locally against the
1.0 stack on this Mac; nothing touches staging or production.

## What is recorded

`~/work/film-out/sync-v1/` (the "one conversation across devices" shot):

| File | Size | What |
|---|---|---|
| `terminal.mp4` | 3024×1964, 30 fps, 42.8 s | Coder in `~/code/slugkit`: the person asks why the tests fail, then sees the phone's and the website's replies arrive and Coder act on them |
| `web.mp4` | 3024×1964, same length | The signed-in website: the terminal chat appears in the sidebar, opens, and updates live; the person types the third message there |
| `phone.mp4` | 1206×2622 (iPhone 17 Pro simulator), same length | The iPhone app on Your chats: the chat lists, opens, updates live; the person types the second message there |
| `beats.json` | | Each beat's time in the clips (`clip`) and on the shared clock (`clock`), the cuts, and the three messages |

The three clips are cut alike on one wall clock: a stretch of more than
2 s where nothing changes on any surface keeps 0.7 s at each end and jumps
over its middle, on all three at once, so they play in sync side by side.
Spinners run at real speed. Checked: at clip time 29 s all three show the
same answer to "Fix it and run the tests."

The messages: terminal "Why are the tests failing? Look, but don't change
anything yet."; phone "Fix it and run the tests."; web "Commit it with a
short message." All answers are real (Coder and the chat worker call real
models through the local gateway).

Not yet recorded: the boot clips (`boot-{terminal,web,phone}.mp4`).
`boot.mjs` is written but has not been run. There is no desktop clip (see
below).

Earlier footage: `~/work/film-out/footage/`, `footage-v2/` (including
`phone-app.mp4`, the app's Running screen), `footage-v3/`.

## Scripts (`scripts/film/footage/`)

| Script | What |
|---|---|
| `record-all.sh` | The single-surface clips (web-chat, phone-approve, terminal-coder, web-sync, phone-app). Rebuilds the binaries unless `--no-build`, restarts a stale stack, `FILM_IOS=0` skips the phone |
| `boot.mjs` | Makes the test account, the demo repo, and Coder's sign-in (writes `FILM_WORK/setup.json`), then the three boot clips. **Untested** |
| `sync-shoot.mjs` | The sync shot. Uses `FILM_SETUP` (a `setup.json`) or makes its own account. Keeps the take in `FILM_WORK/take.json`; `node sync-shoot.mjs --edit WORK` re-cuts it |
| `setup.mjs` | Account, demo repo, `coder login` approved on the website, simulator status bar and `recordVideo`, video change detection, cutting a simulator video at given times |
| `phone-app.mjs` | The app's Running screen while Coder works (`--edit RAW` re-cuts) |
| `cdp.mjs`, `web.mjs` | Headless Chrome over CDP: screencast recorder, encoding at shared-clock times |
| `terminal.mjs`, `ptyrun.py`, `replay-terminal.mjs` | Real PTY sessions kept as asciicast (with their wall-clock start), rendered frame by frame in xterm.js with Paper Mono; `replayAt` renders at shared-clock times |
| `make-demo-repo.sh` | The slugkit repository with three failing tests |

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
  FILM_OUT=$HOME/work/film-out/sync-v1 FILM_CODER=$CARGO_TARGET_DIR/debug/coder-new"
env $common node scripts/film/footage/boot.mjs                                # boot clips (untested)
env $common FILM_SETUP=$W/setup.json node scripts/film/footage/sync-shoot.mjs # the sync shot (about 4 min)
node scripts/film/footage/sync-shoot.mjs --edit ~/work/film-out/.work/sync-162719 # re-cut the kept take
```

The kept take is `~/work/film-out/.work/sync-162719` (300 MB: web frames,
the phone's raw video, the terminal cast, `take.json`). Older scratch
frames are deleted. The iOS app must be built once with
`bins/openagents-ios/build.sh sim` (`OPENAGENTS_IOS_DEVICE=$SIM`); the
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
- The terminal's notice for a phone reply says "from openagents.com".
- The web chat answers coding questions with a closing line about Coder
  and refuses "write code" asks (the web policy), which shows in web-chat
  takes.
- The phone's Running card timer only advances when the list is read
  again (it can sit at "2s" for several seconds).

## What's left

1. Run `boot.mjs` and check the three boot clips (fix what breaks).
2. Desktop: account sign-in and synced chats in the desktop app (product
   work), and the Screen Recording permission to film its window.
3. Approve on the phone: needs Coder to ask for a risky ability (a
   production deploy, a PR merge, DNS, secrets, payments). None can be
   staged honestly for the film yet.
4. Second-person editing: needs shared chats in the product.
