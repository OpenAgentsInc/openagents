# Launch film handoff: the edit (#11432)

Written 2026-10-12, when work paused at a capacity limit. Two companion notes cover the other parts: `handoff-footage.md` (recording) and `handoff-studio.md` (Blender device shots).

## Where it stands
- **Cut 2** (owner reviewed): `~/Downloads/openagents-v1-launch-cut2.mp4`, 42 s at 1080p. It runs title over the Grid → MacBook web chat → MacBook terminal → MacBook sync → iPhone app → end card.
  - It was built from `~/work/film-out/cut/cut2.json`, which is `docs/verse/film/cut.json` with the three MacBook shots pointed at `~/work/film-out/studio-1080/`.
- **Owner verdict on cut 2:** the demos are good, but the film has to show **sync**:
  - 2+ devices on screen at once;
  - the three-device suite (desktop app, iPhone app, web) booting to their home screens;
  - one conversation moving live: typed in the terminal → seen on the web → reply from the phone → and so on;
  - "editable by others", only if the product really supports it.
- **Reshoot in progress when work paused:**
  - Synchronized footage is going to `~/work/film-out/sync-v1/` (see `handoff-footage.md`).
  - Multi-device studio shots are going to `~/work/film-out/studio-1080/`: `suite-boot`, `sync-terminal-web`, `sync-phone`, `sync-web-back`, `trio-wide` (see `handoff-studio.md`).

## Owner rules for this film
- Review cuts are **1080p until the owner approves one**; render 4K after that. Always check a file's duration and content before opening it for the owner.
- The stage is the black-and-white Grid with neutral light. Show only real app UI, recorded live.
- No Verse or game footage featured: no ritual, meteor or montage. The crypt-lab interior is liked, but at most as a brief beat.
- No flickering cursors and no strobing spinners: shorten a wait by cutting it, never by speeding it up.
- No machine-talk copy on screen; stale copy gets fixed in the product.

## Next steps once the reshoot lands
1. Rewrite `docs/verse/film/cut.json` around the new beats (in-points come from `~/work/film-out/sync-v1/beats.json`):
   - title, then `suite-boot`, then the sync shots in story order, then `trio-wide`, then the end card;
   - every slot on the 120 BPM grid (0.5 s steps).
2. Re-time the music: `scripts/film/music.py OUT.wav --seconds N --cuts a,b,c`. It puts the impacts on the title and on the end card at N−8 s.
3. Build the review cut:
   - `scripts/film/cut.py inspect CUT.json` writes the contact sheets to `~/work/film-out/cut/` and must pass. A shot that is dark by design gets `"dark_ok": true`.
   - Then run `scripts/film/cut.py render CUT.json --preview`, copy the result to `~/Downloads/`, check its duration, and open it.
4. After approval: render the 4K studio shots (`render-all.sh` without `--resolution`, `OUT=~/work/film-out/studio`), then `cut.py render CUT.json --master`.

## Product fixes made along the way (all on main)
- `fac7f6dd7c`: web tool steps no longer carry a "Status" heading.
- `8b479fd4c4`: Coder's `/sync on` no longer shows a "Saving your chats" line.
- `4cdfed71b8`: the phone's empty Running line shows no raw backticks.

## Disk
The disk was nearly full during renders (one 4K render failed for lack of space). Check `df -h ~` before rendering. A 4K shot needs 1–2 GB of scratch space.
