# Episode 262 Production Requests

Status: local release candidate assembled (2026-07-24). Owner music pass and
publication gates remain. Non-blocking asset replacements still apply.
Applies to: [`Project Omega`](262.md).
Requested by: Episode 262 production packet.
Screenshare control:
[`Sarah video screenshare`](SARAH_VIDEO_SCREENSHARE.md).
Produced-clip pipeline:
[`Segmind talking-avatar pipeline`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md)
(**Episode RC assembly (proven)**).

Episode 262 is the nontechnical product introduction.
Technical repository footage belongs in
[`Episode 263`](263.md).

The short spoken script in [`262.md`](262.md) is the word authority.
Local clip manifests and long-take manifests may be stale. Do not restore
longer spoken wording from them.
Paste-only spoken copy (local, not committed):
`~/Desktop/Sarah/262/262transcript.md`.

## Local RC deliverable (2026-07-24)

Proven path completed for a no-music release candidate:

| File | Role |
| --- | --- |
| `~/Desktop/Sarah/262/262transcript.md` | Spoken words only |
| `~/Desktop/Sarah/262/262-sarah-master.mp4` | Segmind `p-video-avatar` full take |
| `~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4` | Controlled Welcome hold |
| `~/Desktop/Sarah/262/262-rc-no-music.mp4` | A/B/C edit, continuous Sarah audio |
| `~/Desktop/Sarah/262/262-rc-notes.md` | Local production notes |

Assembly: Sarah picture through the fork beat → labeled
`OMEGA WELCOME - CURRENT` screenshare → return to Sarah for the close.
Screenshare re-recorded with Omega frontmost so foreign side panels stay out
of frame. Do not commit these MP4s.

## Recording and publication gate

No additional owner-supplied input is necessary before production starts.
Use the current Sarah direction and generation runner.
Make the final narration before you lock the picture edit.

These items are necessary before publication:

- Use the approved final Sarah narration and confirm its rights status.
- Burn each product-state and evidence label into the video.
- Review the rights and privacy state of each asset that appears.
- Show visible support for the public Omega fork claim.
  A current static repository capture is sufficient.
- Prefer a live Omega Welcome screenshare over a still of the same frame
  (Request 262-08). The local RC already includes this capture.
- Owner music pass on the RC (RC ships without music by design).

The Sarah picture master is required for this release candidate.
Generate it with the current Sarah direction and Segmind runner.
Use the longest practical Sarah generations, up to 60 seconds for each
generation. Split only at a full-screen cutaway, a service limit, or a verified
quality defect.
Do not wait for historical masters, a named Buzz source, a fully branded
Omega release build, or Episode 263 build proof.
A current Omega Welcome screenshare is welcome now.

## Request 262-01: final Sarah narration

Priority: required before final publication.
Local RC status: Sarah master generated with Segmind `p-video-avatar` and
`scripts/sarah-avatar/sarah-direction.json` (2026-07-24). File:
`~/Desktop/Sarah/262/262-sarah-master.mp4`. Confirm rights before public
release.

Provide or generate the approved final Sarah voice track.
Include its generation method and rights status.
Use it to set the final shot timing.

An approved Sarah picture master is required.
Do not substitute temporary narration or unrelated Sarah footage.

## Request 262-02: historical episode masters

Priority: recommended and replaceable.

Provide source masters for these moments:

- Episode 183: the zero-base reset
- Episode 196: the one-product consolidation
- Episode 237: “we have covered a lot of ground”
- Episode 251: the Desktop base-hit decision
- Episode 257: “your last agent IDE”
- Episode 258: reliability and crash isolation
- Episode 259: verifiable software and accepted outcomes

Default if unavailable: use published cuts when reuse rights are clear.
Otherwise, use checked transcript title cards.
Do not fabricate historical product footage.

## Request 262-03: archive montage

Priority: recommended and replaceable.

Provide or approve short clips for:

- document chat
- Faerie coding
- agent graphs
- plugins and the agent store
- Onyx mobile
- AutoDev
- Pylon and the Tassadar run board
- OpenAgents Desktop
- ProductSpec, approval, and receipt views

Default if unavailable: use repository screenshots and episode-title cards.
Keep each product and fixture label visible.

## Request 262-04: current product captures

Priority: recommended and replaceable.

Provide fresh, redacted captures of:

- OpenAgents Desktop chat and Full Auto
- files and review
- a plan or approval
- a receipt
- the current mobile attention surface

Default if unavailable: use the existing real Full Auto frame and named
deterministic fixtures.
Do not edit separate captures into one claimed live journey.

## Request 262-05: Cursor and Zed source footage

Priority: recommended and replaceable.

Record:

- Cursor's first-party `0.2.0` changelog
- Cursor's first-party fork explanation
- a clean local Zed project
- file open, Git, terminal, agent thread, and split panes

Keep `ZED - CURRENT` visible.
Do not present Zed footage as Omega footage.

Default if unavailable: use slow browser moves across the cited first-party
pages.

## Request 262-06: public Omega fork reveal

Priority: required claim support. Recorded footage is optional.

Record the public `OpenAgentsInc/omega` repository.
Show the parent repository line and the Omega README.

Use `omega1.jpg` for the public repository and fork line.
Use `omega2.jpg` for the Omega README and product direction.
Label it `OMEGA SOURCE - CURRENT`.
Do not imply that the source is already a branded build.

## Request 262-08: live Omega Welcome screenshare

Priority: recommended. Preferred over a Welcome still.
Local RC status: captured and used in Part B of
`~/Desktop/Sarah/262/262-rc-no-music.mp4` (2026-07-24). Prefer
`record --shot welcome-hold` with Omega frontmost. Re-record if a foreign side
panel appears in frame.

Record a live Omega Welcome setup window during the short "out of the box /
Omega" beat in [`262.md`](262.md).

Drive the window with:

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record \
  --shot welcome-hold \
  --seconds 20 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
```

Show real UI motion:

- `Welcome to Omega` / `Your last IDE.`
- Theme family `Aiur` selected
- Base keymap `VS Code` selected
- Optional: `Create identity` click (owner may need one Keychain approval)

Label it `OMEGA WELCOME - CURRENT`.
Do not present Welcome as a finished product release.
Do not lengthen Sarah's spoken lines to cover the screenshare.
Hold the screen under the existing short lines, or cut with silence and a
lower-third.

Default if a live build is unavailable: use one labeled still of Welcome, then
replace it when the live shot exists.

## Request 262-07: Buzz feedback attribution

Priority: optional.

Confirm whether the Buzz user can be named.
Provide exact approved attribution and quote permission.

Default if unavailable: use `A NEW BUZZ USER`.
Paraphrase the “one home screen for the company” insight.
Do not show a private message or identifying account details.

## Timed text overlays

Truth-state labels are necessary before publication.
Slogan and transition overlays are optional polish.

The repository now includes a timed text-overlay command.
It accepts an arbitrary video, preserves its optional source audio, and does
not require the FFmpeg `drawtext` filter:

```sh
pnpm --dir apps/qa-runner run overlay-text \
  --input /path/to/episode-262-picture-lock.mp4 \
  --cues src/compose/timed-text.example.json \
  --out /path/to/episode-262-labeled.mp4
```

Copy the
[`example cue sheet`](../../apps/qa-runner/src/compose/timed-text.example.json)
and set its time ranges after the narration is final.
Use `title`, `center`, `lower-third`, and `state-label` styles.
Use `--audio aac` when the MP4 container does not support the source audio
codec.
The command refuses to replace an existing output unless `--force` is present.

## Delivery checklist

For each supplied asset, include:

- the absolute source path or stable URL
- the owner and rights status
- the capture date and product version
- the evidence label that must remain visible
- private details that the editor must mask

The editor can replace a placeholder only when the asset proves the same claim.

## RC edit checklist (no music)

Use after Requests 262-01 and 262-08 exist locally:

1. Spoken paste file: `~/Desktop/Sarah/262/262transcript.md`
2. Sarah master under `~/Desktop/Sarah/262/`
3. Clean Welcome screenshare with burn-in `OMEGA WELCOME - CURRENT`
4. A/B/C concat with continuous Sarah audio (see
   [`SARAH_VIDEO_SCREENSHARE.md`](SARAH_VIDEO_SCREENSHARE.md)
   **Proven RC capture and edit**)
5. Output `262-rc-no-music.mp4` and sample three verification frames
6. Leave music and public post for the owner publication pass
