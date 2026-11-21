# Recorded clips

Footage of GameBuilder recorded on 2026-11-17, kept for videos and the site.
The files are not in this repository. They live on the owner's Mac at:

```
~/work/film-out/gamebuilder-clips/2026-11-17/
```

That folder is local to that Mac and is not backed up anywhere else. Copy a
clip into a project before you change it.

All clips are 1920x1296 at 30 fps, H.264, with no sound, except the raw iPhone
recording. 1920x1296 is 3:2.

## Web, logged in

These were recorded from the local dev server (`scripts/web.sh`,
http://127.0.0.1:8810/) in headless Chrome with WebGPU, in real time. The login
is the test stand-in that `scripts/web-shots.sh` uses: the page's two login
calls are answered in the page, as `dev@example.test`. No real account was used.

| File | Length | What it shows | Best part |
| --- | --- | --- | --- |
| `web-village-sunrise.mp4` | 17 s | The medieval village from the picker: the grid floor, grass spreading from the character's feet, the sun coming up, then walking up the path into the village | 0–4 s the grass spreads; 8–16 s the walk into the village |
| `web-village-placing-houses.mp4` | 23 s | The camera turns to open grass, then houses and the market are placed from the hotbar (keys 1 to 3, then a click) | 9–14 s a round house goes down. After 15 s the camera is right up against a wall, so don't use it |
| `web-village-house-previews.mp4` | 21 s | The house and market previews in the village street, with "no room there" refusals | 9–15 s |
| `web-racing.mp4` | 20 s | Racing: start lights, then driving the track and a curve | 5–9 s the start, 11–16 s the curve |
| `web-drift.mp4` | 20 s | Drift: the ship flying through the rings in space | 4–10 s |
| `web-platformer.mp4` | 18 s | The platformer: running and jumping between floating platforms | It looks plain; use it last |
| `web-game-picker.mp4` | 9 s | The "pick a game." screen with all nine game types | The whole clip |

## Website, logged out

These were recorded from https://gamebuilder.org/ with `openagents labs clip`, with the page
zoomed so it fills the frame.

| File | Length | What it shows |
| --- | --- | --- |
| `site-home-hero.mp4` | 7 s | The home page hero: "make games. play them." with the match picture |
| `site-home-how.mp4` | 7 s | "idea to game. minutes.": build, play, together, with the two phone screens |

## iPhone app

These were recorded from the iOS Simulator ("GameBuilder iPhone", iOS 26.5) with
`xcrun simctl io booted recordVideo`.

| File | Length | What it shows |
| --- | --- | --- |
| `ios-editor-and-match-raw.mov` | 77 s | Raw portrait recording, 1206x2622. 0–16 s the level editor: orbiting Level 2, adding a block and a tree. About 20 s the mode picker (Knockouts, Last Stand, Scrapball, Hoops) and Bolt. 24–28 s the countdown. 28–34 s the start of a Scrappers Arena 3v3 match. After that, the robot mostly faces a wall, so don't use it |
| `ios-editor-and-match-framed.mp4` | 36 s | The first 36 s of the raw recording, with the phone centred on a blurred copy of itself, at 1920x1296 |

## Tools

`tools/` holds the scripts that made the web clips:

- `rec.py`: records a game type in headless Chrome over the DevTools protocol.
  It logs in with `login.js` and plays a list of timed keys, clicks and drags
  while it records. For example, `python3 rec.py OUT 20 8 '' '[[6,"down","w"],[19.5,"up","w"]]'`
  records racing (game type 8) and holds W. Pass game type -1 to stay on the picker.
- `login.js`: the test login stand-in, then picks the game type.
- `mk.py`: writes an ffmpeg concat list from the frame timestamps, so the clip
  plays at real speed. To make the clip, run
  `python3 mk.py OUT && ffmpeg -f concat -safe 0 -i OUT.ffc -vf fps=30,format=yuv420p OUT.mp4`.

`openagents labs clip` does not work for the logged-in world. It steps the
game's clock frame by frame, separately from the page's timers, so the
scripted login never gets past the login page. Use `rec.py`.
