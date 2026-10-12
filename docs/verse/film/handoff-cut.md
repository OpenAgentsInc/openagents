# Launch film handoff: the edit (#11432)

Written 2026-10-12, when work paused at a capacity limit. Two companion notes cover the other parts: `handoff-footage.md` (recording) and `handoff-studio.md` (Blender device shots).

## Where it stands
- **Cut 3** (for owner review): `~/Downloads/openagents-v1-launch-cut3.mp4`, 45.5 s at 1080p, built from `~/work/film-out/cut/cut3.json` (`docs/verse/film/cut.json` with the studio shots pointed at `~/work/film-out/studio-1080/`) and the score `~/work/film-out/audio/score-cut3.wav` (`music.py --seconds 45.5 --cuts 10.5,18,25.5,32.5`).
  - Title over the Grid (0–4) → `suite-boot` (4–10.5) → `sync-terminal-web` (10.5–18) → `sync-phone` (18–25.5) → `sync-web-back` (25.5–32.5) → `trio-wide` (32.5–37.5) → end card (37.5–45.5).
  - Every shot has two or three devices on screen; the conversation runs terminal → web and phone → reply from the phone → reply from the web → the commit on all three.
  - No Verse footage; the single-device demos from cut 2 are out.
- **Cut 2** (owner reviewed): `~/Downloads/openagents-v1-launch-cut2.mp4`, 42 s at 1080p: title → MacBook web chat → MacBook terminal → MacBook sync → iPhone app → end card. Verdict: the demos are good, but the film has to show **sync** (2+ devices at once; the suite booting; one conversation moving live; "editable by others" only if the product supports it).
- **Gaps in cut 3:** the desktop app isn't in it (it doesn't sign in to an account or list synced chats, and filming its window needs Screen Recording permission); a second person continuing the chat isn't in it (chats belong to one account; no shared chats in the product).

## Owner rules for this film
- Review cuts are **1080p until the owner approves one**; render 4K after that. Always check a file's duration and content before opening it for the owner.
- The stage is the black-and-white Grid with neutral light. Show only real app UI, recorded live.
- No Verse or game footage featured: no ritual, meteor or montage. The crypt-lab interior is liked, but at most as a brief beat.
- No flickering cursors and no strobing spinners: shorten a wait by cutting it, never by speeding it up.
- No machine-talk copy on screen; stale copy gets fixed in the product.

## Next steps
1. Owner review of cut 3 (1080p). Changes go into the shot specs (`scripts/film/studio/shots/`) and `docs/verse/film/cut.json`; every slot stays on the 120 BPM grid (0.5 s steps), and the score is re-timed with `music.py` when the cuts move.
2. A review cut: `scripts/film/cut.py inspect CUT.json` must pass (contact sheets in `~/work/film-out/cut/`; a dark-by-design shot gets `"dark_ok": true`), then `cut.py render CUT.json --preview` (half of 3840×2160, so 1080p), copy to `~/Downloads/`, check duration and content, open it.
3. After approval: render the 4K studio shots (`render-all.sh` without `--resolution`, `OUT=~/work/film-out/studio`, `SYNC=~/work/film-out/sync-v2`), then `cut.py render docs/verse/film/cut.json --master`.

## Product fixes made along the way (all on main)
- `fac7f6dd7c`: web tool steps no longer carry a "Status" heading.
- `8b479fd4c4`: Coder's `/sync on` no longer shows a "Saving your chats" line.
- `4cdfed71b8`: the phone's empty Running line shows no raw backticks.

## Disk
The disk was nearly full during renders (one 4K render failed for lack of space). Check `df -h ~` before rendering. A 4K shot needs 1–2 GB of scratch space.
