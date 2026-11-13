# Fast shots

Issue: [#13139](https://github.com/OpenAgentsInc/openagents/issues/13139).
Code: `coax/web/resident.coax` (the resident browser, the settle check),
`coax/web/shot.coax`, `coax/web/clip.coax`, `coax/web/drive.coax`,
`coax/web/tool.sh` (the shared tool binaries).

`coax/web/tool.sh shot`, `clip` and `drive` use one headless browser per
checkout that stays up between calls. You don't change your commands to get
it. Add `--keep` when you take several pictures of one build.

```sh
coax/web/tool.sh shot URL a.png 1280x720                     # loads the page in the resident browser
coax/web/tool.sh shot URL b.png 1280x720 --keep --eval JS    # keeps the page for the next call
coax/web/tool.sh shot URL c.png 1280x720 --keep --eval JS    # no load: a few frames and a picture
coax/web/tool.sh shot URL d.png 1280x720 --times             # prints where the time went
coax/web/tool.sh clip URL a.mp4 --seconds 3 --keep           # the next --keep clip goes on from this one's end
coax/web/tool.sh shot --stop                                 # stops this checkout's resident browser
coax/web/tool.sh shot --compare a.png b.png                  # how much two pictures differ
```

## Why

In the 36 hours to 2026-11-10, agents made 621 calls to `coax/web/tool.sh`.
They averaged 29 s each, about 5 hours in all. Most were shots and clips. Each
call built the tool (in a new worktree), started a browser, loaded the page,
waited a fixed time, took the picture and closed the browser.

## Use `--keep` for the second picture onward

A call with `--keep` leaves its page loaded. The next `--keep` call for the
same URL uses that page as it stands: no load and no wait. It runs your
`--eval` script, waits for a few frames, and takes the picture.

The page's world carries over from call to call: where the last script left
the camera or your character is where the next script starts. Write scripts
that set what they need (go to a place, open a panel) instead of scripts that
assume a new world. A call without `--keep` always loads its page anew.

A kept page is loaded again when:

- the URL or the color scheme differs from the kept page's;
- a file of the page changed on the server (you rebuilt it);
- the resident browser stopped (3 minutes after the last call).

So `--keep` is safe after a rebuild: the first call loads the new build, and
the calls after it reuse it.

## What a call does

1. **Finds the resident browser.** Its files are in
   `~/.cache/coax-shots/CHECKOUT/`. With none running, the call starts one (a
   keeper process that owns Chrome) and waits for it, about as long as starting
   a browser took before.
2. **Opens a page.** Each call's page is a new tab in a new browser context:
   no cookies, storage or HTTP cache from an earlier call. This is what a new
   browser profile gave before. A `--keep` call first checks the kept page.
3. **Loads the URL and waits until it is ready,** as before: a Coax Native Web page
   has drawn its first scene and its loading card is gone.
4. **Waits until the page is settled** (below), at most `WAIT_MS`.
5. **Runs `--eval`,** waits for three frames and for the picture to stand
   still, at most 500 ms.
6. **Takes the picture.** The PNG holds the same pixels as before. Chrome
   compresses it less, which takes about half the time, and the file is about a
   tenth larger.
7. **Closes the page,** unless the call had `--keep`. A kept page's world loop
   is paused between calls, so an idle resident browser draws nothing.

### The settle check

`WAIT_MS` used to be a fixed wait after the page was ready. Agents passed
5,000 to 25,000 ms to be safe. Now it is the longest the call waits. The call
looks at a small picture of the page (an eighth of its size) every 200 ms and
takes the real picture when either holds:

- **The page stands still.** Two looks in a row change no pixel.
- **The scene moves steadily.** Six looks in a row change by amounts within a
  factor of two of each other, and the amounts don't fall off. Grass, water and
  a crowd move like this for as long as you watch.

A look starts over when the page loaded a file since the last look, still waits
for media, or runs a CSS animation or transition that ends (the loading card
fading out). Something coming to rest, such as a character taking its place,
changes less with each look, so the call waits until it stands still.

A scene that changes over time and never repeats (a slow camera move, a
staged fight) can be judged steady before the moment you want. For those, pass
`--exact-wait`: the call waits the whole `WAIT_MS`, and 500 ms after the
script, as before.

### When a call uses its own browser

A call starts a browser for itself alone, as every call did before, when:

- you pass `--fresh` (or set `SHOT_RESIDENT=0`; `DRIVE_FRESH=1` for `drive`);
- another call in the same checkout is using the resident browser;
- the resident browser doesn't start or doesn't answer. The call stops it, so
  the next call starts a new one.

`CHROME` and `CDP_WEBGPU` name different resident browsers, so a call never
gets a browser that was started with other settings.

The keeper stops Chrome 3 minutes after the checkout's last call
(`SHOT_RESIDENT_IDLE_S` sets another time, in seconds), when you run
`shot --stop`, or when Chrome quits. The time is short on purpose: many agents
share the Mac, and an idle browser holds memory and file handles. If a keeper is killed, the next
keeper stops the Chrome it left on that profile before it starts its own. It
finds that Chrome by its executable and profile directory, never by name.

## Shared tool binaries

`tool.sh` builds a tool into the checkout's `coax/web/build/tools/`. A new
worktree used to compile each tool again, and the C compiler takes most of
that time. Now the compiled tool is also kept in `~/.cache/coax-tools/` under
a hash of what it is made from: the tool's generated C, the platform sources,
and the C compiler with its flags. A build whose hash is already there copies
the binary. `COAX_TOOL_CACHE=0` turns this off. Entries go 14 days after
their last use.

## Clips

A clip uses the resident browser too, and its frames are the faster PNG. With
`--keep`, the page stays in clip mode, and the next `--keep` clip of the same
URL records on from where the last one ended, without a load. `--eval` and
`--skip` apply again from that point.

## Measured

2026-11-10, on the Mac while other agents were building (load average 12 to
15; 200 to 220 where a row says so). The page is
`/worlds/everglade-experimental` (a 90 MB pack), 1280x720, `WAIT_MS` 5000,
served by `coax/web/serve.sh --static`. Old and new calls alternated. At load
200 to 220 the old path took 16.6 to 17.6 s: browser start 1.9 to 2.6 s, load
7.1 to 7.3 s, the fixed wait 5.0 s and capture 2.3 to 2.6 s.

One shot, by phase, in seconds:

| Phase | Before | After, page loaded anew | After, `--keep` |
| --- | --- | --- | --- |
| Tool build in a new worktree | 3.0 (15 to 22 at load 200) | 0.8 | 0.8 |
| Browser start | 0.76 to 0.94 | 0 | 0 |
| New page, or the check of the kept one | 0 | 0.15 to 0.19 | 0.07 |
| Page load: the document, the scripts and every file's bytes | about 0.5 | about 0.5 | 0 |
| World and pack load, until the first scene | 5.1 to 5.9 | 4.5 to 5.3 | 0 |
| Settle | 5.0 (fixed) | 1.95 to 2.5 | 0 |
| Capture | 0.62 to 0.72 | 0.26 to 0.43 | 0.35 to 0.39 |
| Close | 0.15 to 0.32 | 0.01 | 0 |
| **Total, tool already built** | **12.3 to 13.1** | **7.3 to 9.0** | **0.6** |

Other cases:

| Case | Before | After |
| --- | --- | --- |
| The same shot with a script (`--eval`) | 12.3 to 13.1 s, and 0.5 s after the script | 1.3 to 1.5 s with `--keep` |
| `/worlds/garden` (it stands still once loaded) | 6.5 s | 1.7 s |
| A plain HTML page, `WAIT_MS` 1500 | 3.1 s | 0.9 s, and 0.4 s with `--keep` |
| A 3-second clip (90 frames) | 29.6 to 32.6 s | 19.8 to 21.3 s, and 15.1 s with `--keep` |
| One clip frame | 255 to 287 ms | 144 to 164 ms |
| A 1-second clip with `--keep` | | 4.8 s |
| Tool build on a build host (`oa-build-7`) | 6.4 s | 3.2 s |

### Do the pictures match?

`shot --compare A.png B.png` counts the pixels that differ by more than 6 of
255 in a channel.

| Page | Old against old | Old against new |
| --- | --- | --- |
| A plain HTML page | 0 | 0 |
| `/worlds/garden` | 0 | 0 |
| `/worlds/watchtower` | 1 in 1,000 | 2 in 1,000 |
| The home page (it never settles: it loads data every second) | 22 in 1,000 | 12 in 1,000 |
| `/worlds/everglade-experimental` (grass and leaves always move) | 46 in 1,000 | 327 in 1,000 |
| A 3-second clip of it, frame by frame | 75 of 90 frames identical | 75 of 90 frames identical |

The Everglade picture is the same scene, HUD and character. Its grass is at
another point of its sway, because the picture is taken about 2 s after the
scene is ready instead of 5 s. `--exact-wait` takes it at 5 s. The 15 clip
frames that differ are the same frames in both columns, by under 0.02 of 255
on average: the page itself varies there from run to run.

## What is left

- **World and pack load.** A page loaded anew spent about 5 s on this realm's
  world. That is now about 1.5 s ([fast loads](fast-loads.md), #13141), and
  `--keep` still skips it.
- **Clip frames.** A frame takes 144 to 164 ms: about 30 ms to draw it and the
  rest for Chrome to read it back and encode it. A clip that costs its own
  length needs a frame in 33 ms. That needs the page to hand over its frames
  itself in clip mode (`coax/web/host.js`), not a screenshot of each.
- **Pages with no scene.** A Coax Native Web page that draws no indexed scene (the
  home page, `/worlds/watchtower`) is taken as ready only after 10 s
  (`MEDIA_IDLE_MS` in `coax/lib/media/media.coax`). That wait is unchanged.
- **`drive`.** It uses the resident browser, which saves the browser start
  (about 0.5 s). It still starts a server for each call. A full drive session
  on the resident browser was not run here: no realm's build matched this
  checkout's sources.
- **Build hosts.** They have no Chromium and no GPU, so shots still run on
  the Mac ([remote-builds.md](remote-builds.md)).
