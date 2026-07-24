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

# stop the controlled instance
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

Keep [`262.md`](262.md) short.
Drive the Welcome screenshare with `welcome-setup` or `welcome-hold`.
Do not restore the longer stale clip-manifest wording into the spoken script.
See [`262-production-requests.md`](262-production-requests.md).

### Proven RC capture and edit (2026-07-24)

This path produced the local release candidate
`~/Desktop/Sarah/262/262-rc-no-music.mp4` (no background music).

1. **Spoken authority.** Use the short script in [`262.md`](262.md). Keep a
   paste-only copy at `~/Desktop/Sarah/262/262transcript.md` (spoken words
   only; no stage notes). Include the closing first-builds line.
2. **Sarah master.** One continuous Segmind `p-video-avatar` take of the full
   script with `scripts/sarah-avatar/sarah-direction.json`. Save as
   `~/Desktop/Sarah/262/262-sarah-master.mp4`. Prefer one long take (target up
   to 60 s). Do not split by sentence.
3. **Screenshare (prefer motion).** Record a zoomed Welcome walk with
   user-like clicks and scroll. Do not leave a single static hold as the only
   product picture:

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record-motion \
  --shot welcome-tour \
  --seconds 28 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

`record-motion` enlarges the Omega window, captures while the shot clicks
themes/keymaps and drag-scrolls the Welcome body, then crops/zooms that window
to 1920x1088. Do not click Create identity in unattended capture.

4. **Label.** Burn `OMEGA WELCOME - CURRENT` on the screenshare. If FFmpeg
   lacks `drawtext`, render a PNG label (Pillow) and use the `overlay` filter,
   or use `pnpm --dir apps/qa-runner run overlay-text`.
5. **Assemble (A / B / C).** Keep Sarah audio continuous. Cut picture only:

| Part | Picture | Timing (fraction of Sarah master duration D) |
| --- | --- | --- |
| A | Sarah | `0` to about `0.36 * D` |
| B | Labeled Welcome motion screenshare (loop or trim to fill) | about `0.36 * D` to `0.82 * D` |
| C | Sarah (name + first-builds close) | about `0.82 * D` to `D` |

Write the RC to `~/Desktop/Sarah/262/262-rc-no-music.mp4`.
Leave music for a later owner pass.
6. **Verify.** Sample frames near opening, mid-screenshare (confirm scroll or
   click motion), and close. Confirm the Welcome label and that no foreign side
   panel is in the product frame.
7. **Notes.** Keep local production notes beside the MP4s (for example
   `262-rc-notes.md`). Do not commit MP4s.


Full Segmind, GCS portrait URL, and concat detail:
[`../sarah/2026-07-22-segmind-talking-avatar-pipeline.md`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md)
(**Episode RC assembly (proven)**).

## Acting as Sarah

When a harness drafts Sarah's spoken words, follow
[`../sarah/ACTING_AS_SARAH_RUNBOOK.md`](../sarah/ACTING_AS_SARAH_RUNBOOK.md).
When a harness drives the product picture for a Sarah episode, follow this
document.
