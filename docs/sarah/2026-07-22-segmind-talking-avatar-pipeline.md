# Sarah talking-avatar pipeline (Segmind)

Date: 2026-07-22. Status: runbook. This document describes our own programmatic
way to make Sarah speak in a produced clip from one portrait image plus a line
of speech, using the Segmind AI Gateway.

## Why this exists

Episode 260 (`docs/transcripts/260.md`) introduced Sarah as an animated,
speaking presenter. That video was made by hand: a Midjourney-style portrait
animated by a hosted video-generation platform. Our existing owned-avatar docs
(`2026-07-09-owned-avatar-video-pipeline-spec.md`,
`2026-07-09-liveavatar-integration-assessment.md`,
`2026-07-09-oav-quality-strategy.md`,
`2026-07-09-pipecat-voice-infra-audit.md`) describe the **realtime** owned
avatar (MuseTalk lip-sync over owned footage on our own GPU, for live
conversation). Neither path covers the **produced-clip** case: take one image
plus a written line and get a short, high-quality talking-head video.

This pipeline fills that gap programmatically. It is for produced comms — a
tweet intro, an announcement, a "meet Sarah" clip — not live conversation. It
complements the realtime OAV, it does not replace it.

## What it does

Input:
- one **portrait image** (as a public URL),
- a **spoken line** — either a text script (built-in text-to-speech) or a
  pre-recorded audio URL.

Output: an **MP4** of that person speaking the line, with lip sync, natural head
motion, blinking, and expression.

## Model choice

Segmind hosts several talking-avatar models. For our stylized, sci-fi Sarah
portrait, the recommended test order is:

1. **VEED Fabric 1.0** — generative talking-avatar model, most natural facial
   motion. First choice when available on the account.
2. **Kling Avatar V2** (`kling-v2-standard-avatar`, also `kling-v2-pro-avatar`)
   — expressive, handles stylized faces well. Audio-driven (needs `image_url` +
   `audio_url`). Good A/B against Fabric.
3. **HeyGen Avatar V** (`heygen-avatar-v`) — polished but optimized for
   photorealistic humans and a "corporate" look. Not used for our first test.
4. **Pruna P Video Avatar** (`p-video-avatar`) — image plus a **text script or
   audio**, with **built-in TTS** and 30 voices. Simplest single-call path
   (no separate audio hosting). **Default for Episode RC masters** (proven on
   Episode 262, 2026-07-24). Also used for the first test below.

Do not use the cinematic video generators (Seedance, Veo, Wan, Luma, PixVerse)
for this — they animate a scene, they are not image-plus-voice avatar models.

We should A/B Fabric, Kling Avatar V2, and Pruna on the exact same image, line,
and voice, and compare lip sync, eye and head movement, emotional fidelity, and
temporal consistency, then standardize on one for Sarah's comms.

## Setup

### 1. API key (never committed)

The Segmind key lives in `~/work/.secrets/segmind.env` (gitignored), read as the
`x-api-key` header:

```
SEGMIND_API_KEY=SG_xxxxxxxx
```

The runner reads `SEGMIND_API_KEY` from the environment, or from the file named
by `SEGMIND_ENV_FILE` (default `~/work/.secrets/segmind.env`). Never hardcode it,
never print it, never commit it.

### 2. Host the portrait as a public URL

Segmind fetches the image server-side, so it must be a public URL. These avatar
models do not accept base64. Host the portrait on our own Google Cloud Storage
and pass a short-lived signed URL. Our `openagentsgemini` buckets use uniform
bucket-level access, so object ACLs are unavailable — use a V4 signed URL.

Upload with the automation service account (see the root `AGENTS.md` gcloud SA
section), then sign a short-lived read URL locally with the SA key (no IAM
`signBlob` needed):

```sh
CFG=~/work/.secrets/gcloud-sa-config
BUCKET=openagentsgemini-oa-artifacts
OBJ=sarah-avatar/src/<portrait>.png
CLOUDSDK_CONFIG=$CFG gcloud storage cp <local-portrait> "gs://$BUCKET/$OBJ"

# Sign a 6h read URL with the SA private key (Node, @google-cloud/storage):
node -e '
  import("@google-cloud/storage").then(async ({Storage})=>{
    const s=new Storage({keyFilename:process.env.HOME+"/work/.secrets/gcp-mvp-automation.json",projectId:"openagentsgemini"});
    const [url]=await s.bucket(process.env.B).file(process.env.O).getSignedUrl({version:"v4",action:"read",expires:Date.now()+6*3600*1000});
    console.log(url);
  })' 
# with B=$BUCKET O=$OBJ in the environment
```

(`@google-cloud/storage` is not a workspace dependency — install it in a scratch
dir for signing, or use any equivalent V4 signer. The plain `gcloud storage
sign-url` path needs pyOpenSSL in gcloud's Python and is less convenient.)

## Direction profile (how director notes persist)

Owner director notes — a deeper and less cheery voice, and a less toothy, more
sly smile — must shape every future Sarah clip, not just one call. They
live in a canonical profile, `scripts/sarah-avatar/sarah-direction.json`, which
the runner loads and applies by default. It sets Sarah's voice, `voice_prompt`
(tone), `video_prompt` (expression and framing), `negative_prompt`, and
resolution. An explicit `--flag` overrides one field for one call. Pass
`--direction none` to disable the profile, or `--direction <path>` to use
another one.

To act on a new director note, translate it into these fields and append a dated
entry to `director_notes` in the profile (verbatim note plus how it maps to
parameters). Every generation after that carries the change, so the character
stays consistent. Example current mapping: the note above moved the voice from
`Zephyr` (bright) to `Kore` (firm, deeper), rewrote `voice_prompt` toward a
lower, calm, dry, faintly sly register, rewrote `video_prompt` toward a
closed-lip knowing half-smile with minimal teeth, and added big toothy grins and
over-cheerful expressions to `negative_prompt`.

## Run

```sh
node scripts/sarah-avatar/segmind-talking-avatar.mjs \
  --model p-video-avatar \
  --image "<signed portrait URL>" \
  --script "Hi! I'm Sarah. Shall we begin?" \
  --voice-prompt "Warm, confident, friendly — with a genuine smile." \
  --resolution 1080p \
  --out ~/Downloads/sarah/sarah-intro.mp4
```

Kling A/B (audio-driven — supply a hosted voice URL):

```sh
node scripts/sarah-avatar/segmind-talking-avatar.mjs \
  --model kling-v2-standard-avatar \
  --image "<signed portrait URL>" \
  --audio  "<signed voice URL>" \
  --out ~/Downloads/sarah/sarah-intro-kling.mp4
```

## Generation length and edit points

Episode 261 proved that a long, continuous Sarah performance gives a better
result than many short clips. Use the longest practical generation. Target a
maximum duration of 60 seconds for each generation.

Do not split a performance at each sentence. Split only when one of these
conditions applies:

- A full-screen cutaway gives the editor a natural place to join two
  performances.
- The model or the service requires a shorter input.
- A verified sync or quality defect makes a shorter generation necessary.

When a full-screen cutaway exists, put the join below that cutaway. This method
keeps Sarah's performance continuous and prevents an unnecessary visible cut.
If a section is longer than 60 seconds, use the nearest full-screen cutaway as
the preferred boundary. If there is no suitable cutaway, use the nearest
natural change in the subject.

## Product screenshare (second picture track)

The Segmind runner makes the **Sarah talking-head** track. Many episode beats
also need a **product screenshare** track: a live Omega (or other) window under
the same short spoken lines.

Rules:

1. The spoken transcript remains the word authority. Do not lengthen Sarah's
   lines to cover a long screenshare.
2. Prefer a live product window over a still of the same frame.
3. Burn an honest product-state label into the edit (for example
   `OMEGA WELCOME - CURRENT`).
4. Keep generated talking-head clips and recorded screenshares **local**. Do
   not commit them.

Control and shot scripts live in
[`docs/transcripts/SARAH_VIDEO_SCREENSHARE.md`](../transcripts/SARAH_VIDEO_SCREENSHARE.md)
and `scripts/omega-screen-control/`.

### Capture into the episode folder

Episode working media for Sarah lives on the owner Desktop:

```text
~/Desktop/Sarah/<episode>/
```

For Episode 262:

```text
~/Desktop/Sarah/262/
```

Record a Welcome hold (example):

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
OUT="$HOME/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4"

# Drive the window (Help → Editor Onboarding when custody is Ready).
node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-hold &
CTRL_PID=$!
sleep 4

# Capture the main display. Device index comes from:
#   ffmpeg -f avfoundation -list_devices true -i ""
# Use the "Capture screen N" index (often 4 on this Mac). No mic.
ffmpeg -y -f avfoundation -framerate 30 -i "4:none" \
  -t 20 -c:v libx264 -pix_fmt yuv420p -an "$OUT"

wait "$CTRL_PID" 2>/dev/null || true
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

Or use the bundled record helper (same destination contract):

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record \
  --shot welcome-hold \
  --seconds 20 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
```

Owner gates for capture:

- Accessibility for the control host.
- Screen Recording for the capture host. After a new grant, restart Cursor or
  the terminal before `ffmpeg` / `screencapture` can read the display.
- If Nostr custody is Ready, reset with Omega's `omega-identity` CLI before a
  true first-run Welcome, or open `Help → Editor Onboarding`.

### Interleave in the edit

| Track | Source | Typical use |
| --- | --- | --- |
| Sarah picture | Segmind runner output under `~/Desktop/Sarah/<episode>/` | Opening and closing on-camera beats |
| Product screenshare | `ffmpeg` capture of a controlled Omega window | Mid-episode product beats and fill over salvage gaps |
| Still cutaway | Only when live capture is unavailable | Labeled honestly. Replace when a live shot exists |

Join Sarah generations under a full-screen screenshare or cutaway. Do not invent
extra spoken text only to fill the screenshare. Hold the screen under the
existing short lines, or use silence with a lower-third label.

## Episode RC assembly (proven)

Date proven: 2026-07-24. Episode: 262 (Project Omega). Result: local
release-candidate MP4 with **no background music**.

This is the default path for a short Sarah episode that needs one talking-head
master and one live product screenshare. Music stays for a later owner pass.

### Folder contract

```text
~/Desktop/Sarah/<episode>/
  <episode>transcript.md          # spoken words only (paste into tools)
  <episode>-sarah-master.mp4      # Segmind full take
  <episode>-screenshare-….mp4     # controlled product capture
  <episode>-rc-no-music.mp4       # assembled RC
  <episode>-rc-notes.md           # local production notes (optional)
  rc-work/                        # intermediates (optional)
```

Episode 262 used:

```text
~/Desktop/Sarah/262/
  262transcript.md
  262-sarah-master.mp4
  262-screenshare-omega-welcome.mp4
  262-rc-no-music.mp4
  262-rc-notes.md
```

Do not commit those media files. Spoken authority stays in
`docs/transcripts/<episode>.md`. The Desktop `*transcript.md` file is a
copy-paste helper with no stage notes.

### Step 1 — Lock spoken words

1. Follow [`ACTING_AS_SARAH_RUNBOOK.md`](ACTING_AS_SARAH_RUNBOOK.md).
2. Keep the repository script short. Do not restore stale clip-manifest lines.
3. Write `~/Desktop/Sarah/<episode>/<episode>transcript.md` with spoken text
   only (no speaker labels, no cut notes).

### Step 2 — Generate one Sarah master

Use Pruna P Video Avatar (`p-video-avatar`) with the direction profile. Host
the portrait as a short-lived GCS V4 signed URL (see Setup above). Pass the
full short script in one call:

```sh
node scripts/sarah-avatar/segmind-talking-avatar.mjs \
  --model p-video-avatar \
  --image "<signed portrait URL>" \
  --script "$(cat ~/Desktop/Sarah/262/262transcript.md)" \
  --resolution 1080p \
  --out ~/Desktop/Sarah/262/262-sarah-master.mp4
```

Proven Episode 262 result: one continuous take, about 40.8 s, 1920x1088,
24 fps, AAC mono 24 kHz. Inference about 283 s submit → COMPLETED. Direction
file applied by default (`scripts/sarah-avatar/sarah-direction.json`). No
fallback master was required.

### Step 3 — Record a clean product screenshare

Prefer `welcome-hold` (Help → Editor Onboarding). Keep Omega frontmost. Hide
or close unrelated side panels before capture. Re-record if a foreign UI
(for example a Gemini panel) is in frame.

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record \
  --shot welcome-hold \
  --seconds 20 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

Burn an honest label (`OMEGA WELCOME - CURRENT`). If local FFmpeg has no
`drawtext`, render a PNG with Pillow and composite with `overlay`, or use:

```sh
pnpm --dir apps/qa-runner run overlay-text \
  --input /path/to/screenshare.mp4 \
  --cues /path/to/cues.json \
  --out /path/to/screenshare-labeled.mp4
```

### Step 4 — Assemble A / B / C (audio continuous)

Keep Sarah's spoken audio end to end. Cut **picture** only:

| Part | Picture | Timing |
| --- | --- | --- |
| A | Sarah master | `0` → about `0.38 * D` |
| B | Labeled screenshare (loop or trim) under Sarah audio | about `0.38 * D` → `0.85 * D` |
| C | Sarah master | about `0.85 * D` → `D` |

`D` is the Sarah master duration. Episode 262 used `D ≈ 40.76 s`,
`T1 = 15.49 s`, `T2 = 34.65 s`. Concatenate to
`~/Desktop/Sarah/<episode>/<episode>-rc-no-music.mp4`.

Optional fork stills (`OMEGA SOURCE - CURRENT`) may sit ready as b-roll. The
proven 262 RC did not need them when the Welcome screenshare filled Part B.

### Step 5 — Verify and stop

- Sample frames at opening, mid-screenshare, and close.
- Confirm the product label and a clean product frame.
- Confirm no music track on the RC.
- Record local notes. Do not treat the RC as a public post until publication
  gates in the episode production-requests file are green.

Screenshare shot detail:
[`../transcripts/SARAH_VIDEO_SCREENSHARE.md`](../transcripts/SARAH_VIDEO_SCREENSHARE.md).

## How it works (Segmind Async Inference V2)

Video models must use the async API for reliability. The runner:

1. `POST https://api.segmind.com/v2/<model>` with the inputs and the `x-api-key`
   header. The response is JSON: `{ request_id, status: "QUEUED", status_url,
   response_url, poll_url }`.
2. Polls `status_url` every 10 s until `status` is `COMPLETED` (or `FAILED`).
3. Reads `response_url` for the output and downloads the MP4 to `--out`.

The synchronous `/v1/<model>` endpoint holds the connection for the whole
generation and is not reliable for video — always use V2.

## First test result

Model `p-video-avatar`, our Sarah portrait
(`~/Downloads/sarah/v2_1783572530279-62463022.png`), line "Hi! I'm Sarah. Shall
we begin?", voice `Zephyr (Female)`, 1080p, seed 4242.

- The async V2 path is proven: submit returned `request_id` + `QUEUED`, polled
  through `PROCESSING`, reached `COMPLETED`.
- Inference time about 96 s. Cost 0.225 credits.
- Output: an H.264 + AAC MP4, 1920x1088, 24 fps, about 3.4 s, about 2.7 MB.
- Quality: Sarah's identity is preserved on the stylized sci-fi portrait. She
  smiles and speaks the line with natural mouth motion, lit to match the source.
- The clip is saved at
  `~/Downloads/sarah/sarah-intro-pruna-p-video-avatar.mp4`. It is kept local and
  is not committed, per owner direction.

Next: A/B the same input against VEED Fabric and Kling Avatar V2 before
standardizing a model for Sarah's comms.

## Output handling and secrets

- The **talking-head video output stays local** (kept under
  `~/Downloads/sarah/` or `~/Desktop/Sarah/<episode>/`). Do not commit
  generated clips.
- The **screenshare captures** stay under `~/Desktop/Sarah/<episode>/`. Do not
  commit them.
- The **API key** stays in `~/work/.secrets/segmind.env`. Never commit or print.
- Only this runbook, the screenshare control doc, and the committed scripts
  under `scripts/sarah-avatar/` and `scripts/omega-screen-control/` are
  committed.

## What to decide next

- Whether to keep `p-video-avatar` as the default after A/B with Fabric and
  Kling Avatar V2 (RC default today: Pruna).
- Where produced clips are stored and how they attach to a comm (the owner tweet
  queue `docs/sarah/SARAH_TWEET_QUEUE.md`, a blog post, or the timeline once the
  outward web-communications broker is admitted — see
  `docs/sarah/2026-07-22-sarah-company-command-analysis.md`).
- Voice source: Segmind built-in TTS vs our own voice (the OAV/pipecat voice
  stack) fed in as `--audio`.
- Cost and rate posture for routine comms generation.
- Owner music pass and publication gates for each episode RC under
  `~/Desktop/Sarah/<episode>/`.
