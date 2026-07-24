# Episode 262 Production Requests

Status: production can start now. One final-narration gate and non-blocking
asset replacements apply.
Applies to: [`Project Omega`](262.md).
Requested by: Episode 262 production packet.

Episode 262 is the nontechnical product introduction.
Technical repository footage belongs in
[`Episode 263`](263.md).

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

The Sarah picture master is optional.
Use cutaways and title cards when it is not available.
Do not wait for historical masters, fresh product recordings, a named Buzz
source, a branded Omega build, or Episode 263 build proof.

## Request 262-01: final Sarah narration

Priority: required before final publication.

Provide or generate the approved final Sarah voice track.
Include its generation method and rights status.
Use it to set the final shot timing.

An approved Sarah picture master is recommended but optional.

Default if unavailable: create an editorial cut with temporary narration.
Label it `EDITORIAL ASSEMBLY - NOT FINAL SARAH PERFORMANCE`.
Do not publish that cut as the final Sarah performance.

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

Default if unavailable: use a static repository capture.
Label it `OMEGA SOURCE - CURRENT`.
Do not imply that the source is already a branded build.

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
