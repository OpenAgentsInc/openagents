# Cinematic launch film (#11432)

The V1 launch film is built from code in three stages. First, real footage of the product is recorded. Second, three back ends render the shots:

- **Studio**: headless Blender puts that footage on 3D device models.
- **World**: the Verse renderer shoots in-world scenes with a keyframed camera.
- **Titles**: generated as their own clips.

Third, a cut list assembles the shots with music and sound effects. `inspect` checks a cut before any final render.

## Output layout

Rendered media stays out of git, under `~/work/film-out/`:

| Dir | Who writes it | Contents |
|---|---|---|
| `footage/` | `scripts/film/footage/` | Real screen recordings: `web-chat.mp4`, `web-sync.mp4`, `phone-approve.mp4`, `terminal-coder.mp4` |
| `studio/` | `scripts/film/studio/` (Blender) | Device shots: `<shot>.mp4` |
| `verse/` | `crates/verse/examples/film_*` | In-world shots: `<shot>.mp4` |
| `titles/` | `scripts/film/cut.py` | Title and end cards |
| `audio/` | `scripts/film/music.py` | Music bed and sound effects |
| `cut/` | `scripts/film/cut.py` | Contact sheets, previews, `openagents-v1-launch.mp4` |

## Format

- Every shot clip is 1920×1080 at 30 fps, H.264 `yuv420p`, CRF ≤ 14 (or ProRes 422), with no audio.
- The first cut is 1080p. The same scripts render 4K by changing a single resolution flag.
- A clip may run longer than its slot. The cut trims it from its `in` point.
- Screen footage is recorded at the device's own pixel size, with no browser chrome:
  - Desktop web: 1512×982 CSS px at 2×.
  - Phone: 393×852 CSS px at 3×.
  - Terminal: 1512×982 at 2×.

## The cut (first version, about 56 s)

| # | Slot | Source | Shot |
|---|---|---|---|
| 1 | 0–6 s | verse | Cold open: a slow crane through the ritual chamber. |
| 2 | 6–9 s | titles | "OpenAgents 1.0" |
| 3 | 9–17 s | studio + `web-chat` | MacBook orbit while a real answer streams in. |
| 4 | 17–24 s | studio + `terminal-coder` | Push in on Coder working a task in the terminal. |
| 5 | 24–30 s | studio + `web-sync` | Match cut: the same chat appears live in the web sidebar. |
| 6 | 30–36 s | studio + `phone-approve` | iPhone approves the six-letter code, shallow depth of field. |
| 7 | 36–48 s | verse | World montage: druid, meteor, Lagrange 1, crypt, about 3 s each. |
| 8 | 48–56 s | titles | `openagents.com/download` and the install one-liner. |

Claims follow `docs/launch/1.0/claims.md`: web and terminal are the story, and the Verse is the visual hook.

## The Verse trailer (#12458)

A 72 s cut, [`verse-trailer.json`](verse-trailer.json), in Video Lab as
`verse-trailer`. Its media goes under `~/work/film-out/verse-trailer/`, and
the render to `~/work/film-out/cut/verse-trailer.mp4`.

| Part | Made by |
|---|---|
| Grid shots with people and agents walking | `film_shot` with the Grid scene's `walkers` (`verse-shots/grid-agents.json`, `grid-reveal.json`) |
| The Grid turning into a world | `toggle-grid.json` and `toggle-grove.json`: one rig in both scenes, so the cut's `circleopen` crossfade changes the world without moving the camera |
| Real footage of the Grid, its zones, and a game | `scripts/film/footage/verse-grid.mjs` over `verse-trailer-takes.json` |
| The phone | `scripts/film/footage/concept-voice.mjs` (a mock-up screen), then `studio/shots/iphone-voice.json` |
| CONCEPT tags | `scripts/film/tag-overlay.py`, as a shot's `over` |
| Music | `scripts/film/music.py OUT --seconds 72 --cuts 8,20,32,45,58 --end 64` |

Building a world by voice, shops, publishing a world, and earning from one
aren't built yet. Their shots carry a CONCEPT tag or say so on a title card.

### Second cut: Kiki's storyboard

[`verse-trailer-kiki.json`](verse-trailer-kiki.json), 72 s, in Video Lab as
`verse-trailer-kiki`, follows Kiki's six beats, voiceover lines, and timings
literally. It's a vision trailer: the Grid as the monochrome layer that
connects everything, and each world as its own colorful layer at the same
coordinates. Its media goes under `~/work/film-out/verse-trailer-kiki/`, and
the render to `~/work/film-out/cut/verse-trailer-kiki.mp4`.

| Part | Made by |
|---|---|
| The astral layer: a world as the Grid draws it, then lit and colored at the same camera | `film_shot`'s `astral` scene (`examples/film/astral.rs`): `layer` `grid` or `world`, and a `reveal` that spreads the world over the Grid from a point |
| Ghost agents | `walkers` in an astral scene: see-through wireframes on the Grid, solid characters in the world |
| The Haunted House, the Camping Adventure, Blue Rush's main street, home plots, and the galaxy | the `haunt`, `camp`, `street` (with the greco kit's models and their footprints), `plots`, and `galaxy` worlds (`verse-shots/kiki-*.json`) |
| The phone | `concept-voice.mjs --variant haunt`, then `studio/shots/iphone-voice-kiki.json` |
| VISION tags | `tag-overlay.py --tag VISION` |
| Music | `music.py OUT --seconds 72 --cuts 8,20,32,45,58 --end 66 --reveal 58 --mood warm` |

Every astral shot is a concept made in our engine and carries a VISION tag.
The Grid with its walkers, the Everglade, The Ice Cream Man, and the ritual
chamber are real and untagged.
