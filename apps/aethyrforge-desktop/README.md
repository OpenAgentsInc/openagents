# Aethyrforge for Mac (experimental)

An AppKit shell around the website's Everglade 5e WebGPU engine in WKWebView,
with its own local room and page server. Requires an Apple Silicon Mac running
macOS 26 or later, Xcode and the existing Coax web build toolchain.

```sh
coax/build.sh
apps/aethyrforge-desktop/build.sh --open
```

The build tool is Coax. It builds the local stack when its source trees are newer,
then bundles the room, static server, engine, browser host, shaders and assets.
It ad-hoc signs the app and, with `--open`, copies it to
`~/Library/Caches/Aethyrforge/Aethyrforge.app` and opens that copy. The app works
independently of the checkout. No installation to `/Applications` or web deploy.

The app opens `/worlds/everglade?forge&room=pair-aethyrforge&relay=ws://localhost:PORT#gate`
with EDIT on. The existing named playtest-room profile takes a default character
and skips the first-visit character picker that would otherwise consume EDIT's
keys while hidden behind its panels. Its servers bind only `127.0.0.1`, on two free ports, and stop on app quit or
when either server fails. A lock prevents two instances writing the same world.
View → Reload Everglade reloads the page. Closing the window quits the app.
The relay uses `localhost`, the existing room client's required local override,
so the WebSocket connects straight to the room and leaves the HTTP page proxy
free to serve saves.

The local world is `~/Library/Application Support/Aethyrforge/everglade/`:

- `world-everglade.json`: edits saved by the room; retained across launches.
- `room-control.txt`: the local room's existing development controls.
- `room.log`, `serve.log`: the current launch's server output.
- `local-url.txt`: the current local page URL, for headless inspection.

On the first launch only, the app copies the repository's development world
snapshot and controls there. Existing files are preserved. It does not fetch the
published world. The existing editor's `1`–`5` keys place things, clicking selects
them, the arrows move, `R` rotates, `+`/`-` resize, Delete removes, and `Z`/`Y`
undo/redo during the session. Saving settles through the local room; this path's
word “publish” means the local document only. The undo stack is not durable yet.

Audio is muted. Voice, transcription, model loading, typed instructions, remote
publish/revert, bots and raid controls are future milestones in the
[desktop spec](../../docs/verse/2026-11-04-aethyrforge-desktop.md).

The native smoke check creates a hidden WKWebView and an isolated world next to
the report. It waits for a real WebGPU scene, places a thing through the editor's
keyboard path, checks the saved document, captures a frame, and quits both servers.
Run it again in the same directory to check the saved world survives reopening:

```sh
mkdir -p /tmp/aethyrforge-smoke
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --smoke-test /tmp/aethyrforge-smoke/first.json
apps/aethyrforge-desktop/build/Aethyrforge.app/Contents/MacOS/Aethyrforge --smoke-test /tmp/aethyrforge-smoke/reopened.json
```

Reports include the first-frame time and child PIDs; matching PNGs show the frame.
Use a fresh directory for a new edit test. This is milestone one's shell/save
check, not the future voice acceptance suite.
