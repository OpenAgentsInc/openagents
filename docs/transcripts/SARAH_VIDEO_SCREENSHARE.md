# Sarah video screenshare control

Date: 2026-07-24.
Status: production runbook.
Audience: harnesses that record or edit Sarah episodes with live product
picture.

Use this document when a Sarah episode needs a live product screen in the
picture, not only a still image. Prefer a controlled screenshare. Keep the
spoken script short. Move the picture with a shot script.

## Rule

1. The spoken transcript is the authority for words.
2. Local clip manifests and long-take manifests may be stale. Do not let them
   lengthen the spoken text.
3. Prefer a live Omega window over a PNG or JPG of the same frame.
4. Burn an honest product-state label into the edit.
5. Do not present a planned feature as a shipped feature.

## When to cut

Use a screenshare when Sarah points at a product surface that exists now.
Examples:

- Omega Welcome setup (`Welcome to Omega`)
- A public repository page
- A Desktop or mobile attention surface that is real and redacted

Use a still only when the live surface is unavailable, private, or unstable.
Label the still honestly.

## Interleave model

Record two tracks, then cut:

1. **Sarah picture** — talking-head generation from the short script.
2. **Product screenshare** — a live Omega (or other) window driven by a shot.

In the edit:

| Spoken beat | Picture |
| --- | --- |
| Opening strategy lines | Sarah |
| Fork / Zed decision | Optional brief Cursor or Zed reference, or stay on Sarah |
| "Out of the box…" through naming Omega | Live Omega Welcome screenshare |
| Closing name / hold | Hold Welcome, or return to Sarah |

Do not invent extra spoken lines only to fill a long screenshare. Hold the
screen under the existing short lines, or add silence under the screenshare
with a lower-third label.

## Control Omega on macOS

Tool:

```sh
node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-setup
```

What it does:

- Launches Omega with a fresh `--user-data-dir` so the window is clean
- Sets `ZED_STATELESS=1` so a second instance does not hand off to an
  already-open Omega window
- Sets `ZED_EXPERIMENTAL_A11Y=1` for AccessKit when labels exist
- Opens Welcome through `Help → Editor Onboarding` when first-run does not
  auto-open
- Clicks theme and keymap with window-relative coordinates when AX names are
  sparse

If machine Keychain custody is still Ready, Welcome will not auto-open for a
fresh data dir. Reset the channel identity with the Omega CLI (Ready UI only
shows Protect, not Reset):

```sh
cargo run -p omega_identity --bin omega-identity -- --channel rc status
cargo run -p omega_identity --bin omega-identity -- --channel rc wipe --yes
```

Useful commands:

```sh
# launch, open Editor Onboarding, hold
node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-hold

# record a static hold (legacy / fill)
node scripts/omega-screen-control/omega-screen-control.mjs record \
  --shot welcome-hold \
  --seconds 20 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4

# preferred for Episode 262 mid-section: zoomed window + live clicks/scroll
node scripts/omega-screen-control/omega-screen-control.mjs record-motion \
  --shot welcome-tour \
  --seconds 28 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4

# open Welcome on an already-running controlled instance
node scripts/omega-screen-control/omega-screen-control.mjs menu \
  --bar Help --item "Editor Onboarding"

# window-relative click (0..1 fractions of the front Omega window)
node scripts/omega-screen-control/omega-screen-control.mjs click-at --x 0.22 --y 0.42

# Finish Setup when identity is ready (cmd-enter)
node scripts/omega-screen-control/omega-screen-control.mjs key --combo cmd+enter

# stop the controlled instance (SIGTERM only — never Cmd+Q)
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

Episode working media stays local under `~/Desktop/Sarah/<episode>/`. Do not
commit those MP4s.

Set the binary:

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
# or
export OMEGA_APP="/path/to/Omega.app"
```

Owner gates:

- Grant Accessibility permission to the terminal or agent host.
- Grant Screen Recording if the host will capture the Omega window.
  After a new grant, restart Cursor (or the terminal app) before capture APIs
  work.
- `Create identity` may open a Keychain or secure-input prompt once.
- `Finish Setup` stays disabled until the local Nostr identity is ready.
- First-run Welcome auto-opens only when custody is not Ready. Otherwise use
  `Help → Editor Onboarding` or `Help → Show Welcome`.

## Labels

| Surface | Label |
| --- | --- |
| Fresh Welcome from a current Omega build | `OMEGA WELCOME - CURRENT` |
| Public fork / README capture | `OMEGA SOURCE - CURRENT` |
| Inherited Zed-looking build | `OMEGA FORK BUILD - UPSTREAM ZED IDENTITY` |
| Stock Zed reference | `STOCK ZED - CURRENT REFERENCE` |
| Planned UI not running | `ACCEPTED ARCHITECTURE - NOT RUNNING` |

## Episode 262

Keep [`262.md`](262.md) short. Status: final script.
Drive the Welcome screenshare with `welcome-tour` (preferred) or
`welcome-hold`.
Do not restore the longer stale clip-manifest wording into the spoken script.
See [`262-production-requests.md`](262-production-requests.md).

### Episode 262 lessons (do not repeat)

Full detail:
[`../sarah/2026-07-22-segmind-talking-avatar-pipeline.md`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md)
(**Episode 262 lessons (2026-07-24)**). Short form for screenshare agents:

- **Finder leak.** Stop capture before teardown. Verify frames at
  `T2 − 1 s`, `T2 − 0.5 s`, and `T2`. Reject Finder/Desktop pixels in the mid
  tail.
- **FIT pad and window size.** Window about `1280×900` (not maximized). FIT
  decrease + dark pad `0x0E0E10`. Crop the full window. Avoid cover-fill zoom
  that crushes Welcome.
- **Quit with SIGTERM only.** Use `omega-screen-control quit`. Never Cmd+Q
  during capture.
- **Second mid picture.** Welcome motion alone is not enough. Episode 262
  finder-fix v2 used Welcome motion, then an `omega2` README still for the
  last about 6 s of the mid.
- **Music.** Proven ElevenLabs Music generate + mix lives in the Segmind
  pipeline doc (**Music bed (ElevenLabs)**). Secret:
  `~/work/.secrets/elevenlabs.env`. Do not invent a key.

### Proven RC capture and edit (2026-07-24)

This path produced the local release candidates
`~/Desktop/Sarah/262/262-rc-no-music.mp4` (clean plate) and
`~/Desktop/Sarah/262/262-rc-with-music.mp4` (optional music sibling).

1. **Spoken authority.** Use the short final script in [`262.md`](262.md).
   Keep a paste-only copy at `~/Desktop/Sarah/262/262transcript.md` (spoken
   words only; no stage notes). Write lowercase `zed` for British TTS.
   Include the closing first-builds line.
2. **Sarah master.** One continuous Segmind `p-video-avatar` take of the full
   script with `scripts/sarah-avatar/sarah-direction.json`. Save as
   `~/Desktop/Sarah/262/262-sarah-master.mp4`. Prefer one long take (target up
   to 60 s). Do not split by sentence.
3. **Screenshare (prefer motion).** Record a FIT-padded Welcome walk with
   user-like clicks and scroll. Keep a second mid still ready (`omega2` or
   similar). Do not leave a single static hold as the only product picture:

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record-motion \
  --shot welcome-tour \
  --seconds 28 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

Prefer a `1280×900` window, FIT pad, full-window crop to 1920x1088. Do not
click Create identity in unattended capture. Quit only through the control
script.

4. **Label.** Burn `OMEGA WELCOME - CURRENT` on the screenshare. If FFmpeg
   lacks `drawtext`, render a PNG label (Pillow) and use the `overlay` filter,
   or use `pnpm --dir apps/qa-runner run overlay-text`.
5. **Assemble (A / B / C).** Keep Sarah audio continuous. Cut picture only.
   Verify T1 on the silence after spoken `zed` (about 17.50 s). Set T2 near
   `0.78 * D`. Do not copy older fractions when they disagree with the audio.

| Part | Picture | Timing |
| --- | --- | --- |
| A | Sarah | `0` to T1 |
| B | Welcome motion, then second beat (`omega2` still) | T1 to T2 |
| C | Sarah (name + first-builds close) | T2 to `D` |

Write the clean plate to `~/Desktop/Sarah/262/262-rc-no-music.mp4`.
Optional music sibling: `262-rc-with-music.mp4` via the pipeline music
section.
6. **Verify.** Sample frames near opening, mid-screenshare (confirm scroll or
   click motion), and close. Also check `T2 − 1 s`, `T2 − 0.5 s`, and `T2`
   for Finder/Desktop leak. Confirm the Welcome label and that no foreign side
   panel is in the product frame.
7. **Notes.** Keep local production notes beside the MP4s (for example
   `262-rc-notes.md`). Do not commit MP4s.

Full Segmind, GCS portrait URL, music, and concat detail:
[`../sarah/2026-07-22-segmind-talking-avatar-pipeline.md`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md)
(**Episode RC assembly (proven)** and **Episode 262 lessons**).

## Acting as Sarah

When a harness drafts Sarah's spoken words, follow
[`../sarah/ACTING_AS_SARAH_RUNBOOK.md`](../sarah/ACTING_AS_SARAH_RUNBOOK.md).
When a harness drives the product picture for a Sarah episode, follow this
document.
