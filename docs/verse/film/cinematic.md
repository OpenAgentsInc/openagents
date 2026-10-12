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
