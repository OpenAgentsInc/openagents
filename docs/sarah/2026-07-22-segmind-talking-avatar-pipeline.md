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
release-candidate MP4. Keep a clean plate without music and an optional
sibling with a documented music bed.

This is the default path for a short Sarah episode that needs one talking-head
master, a live product screenshare, and at least one second mid picture
(cutaway still or second shot). Read **Episode 262 lessons (2026-07-24)**
before you start a new RC.

### Folder contract

```text
~/Desktop/Sarah/<episode>/
  <episode>transcript.md          # spoken words only (paste into tools)
  <episode>-sarah-master.mp4      # Segmind full take
  <episode>-screenshare-….mp4     # controlled product capture
  cutaways/                       # stills (or ignored .artifacts/episode-N/cutaways/)
  <episode>-rc-no-music.mp4       # assembled RC (clean plate)
  <episode>-rc-with-music.mp4     # optional music mix
  <episode>-rc-notes.md           # local production notes (optional)
  rc-work/                        # intermediates (optional)
```

Episode 262 used:

```text
~/Desktop/Sarah/262/
  262transcript.md
  262-sarah-master.mp4
  262-screenshare-omega-welcome.mp4
  cutaways/ (or omega2 still)
  262-rc-no-music.mp4
  262-rc-with-music.mp4
  262-rc-notes.md
```

Do not commit those media files. Spoken authority stays in
`docs/transcripts/<episode>.md` (update that file to the final spoken text when
the script locks). The Desktop `*transcript.md` file is a paste helper: spoken
words only, no stage notes. Header metadata stays in the repository transcript.

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

Prefer `record-motion` with `welcome-tour`. Keep the Omega window near
`1280×900` (do not maximize). Use FIT decrease plus a dark pad
(`0x0E0E10`), then crop the full window. A static `welcome-hold` is only a
fallback. Keep Omega frontmost. Hide or close unrelated side panels before
capture. Do not click Create identity in unattended capture.

Stop recording before app teardown. Prefer
`record-motion --safe-tail-seconds 1` (default). Do **not** use Cmd+Q during
capture. Cmd+Q can quit the wrong app. `omega-screen-control quit` is
SIGTERM-only (#9233).

Also prepare a **second mid picture** (cutaway still or second shot). One
continuous screenshare alone looks flat and leaves no cover for a bad tail.
Episode 262 used Welcome motion, then an `omega2` README still for the last
about 6 s of the mid section.

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
node scripts/omega-screen-control/omega-screen-control.mjs record-motion \
  --shot welcome-tour \
  --seconds 28 \
  --safe-tail-seconds 1 \
  --out ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4
```

Burn an honest label (`OMEGA WELCOME - CURRENT`). If local FFmpeg has no
`drawtext`, render a PNG with Pillow and composite with `overlay`, or use:

```sh
pnpm --dir apps/qa-runner run overlay-text \
  --input /path/to/screenshare.mp4 \
  --cues /path/to/cues.json \
  --out /path/to/screenshare-labeled.mp4
```

Keep cutaway stills under the episode Desktop folder or ignored
`.artifacts/episode-N/cutaways/` with a manifest.

### Step 4 — Assemble A / B / C (audio continuous)

Keep Sarah's spoken audio end to end. Cut **picture** only:

| Part | Picture | Timing |
| --- | --- | --- |
| A | Sarah master | `0` → T1 |
| B | Mid product picture (motion, then second beat / cutaway) | T1 → T2 |
| C | Sarah master | T2 → `D` |

`D` is the Sarah master duration. Do not copy old T1/T2 fractions blindly.
Derive T1 from the silence after the spoken pause (for Episode 262, after
`zed`, about 17.50 s). Set T2 near `0.78 * D` only as a start value, then
confirm it on the audio. Older notes that disagree with the audio are wrong
for that cut.

Use the one-command helper (#9234). It writes the no-music RC, derives T1 from
silence in a search window, supports a second mid cutaway still, checks the
Desktop paste against `docs/transcripts/<episode>.md`, and fails when
Finder-like frames appear near T2:

```sh
node scripts/sarah-avatar/assemble-rc.mjs \
  --sarah-master ~/Desktop/Sarah/262/262-sarah-master.mp4 \
  --screenshare ~/Desktop/Sarah/262/262-screenshare-omega-welcome.mp4 \
  --cutaway /path/to/omega2.jpg \
  --cutaway-seconds 6.3192 \
  --desktop-transcript ~/Desktop/Sarah/262/262transcript.md \
  --repo-transcript docs/transcripts/262.md \
  --require-transcript-lock \
  --out ~/Desktop/Sarah/262/262-rc-no-music.mp4
```

Pass `--t1` only after you verify the pause on the audio. Pass `--t2` when
`0.78 * D` is wrong for the close. Tests:
`node --test scripts/sarah-avatar/assemble-rc.test.mjs`.

Optional music mix writes `<episode>-rc-with-music.mp4` (see **Music bed
(ElevenLabs)** below). Keep `*-rc-no-music.mp4` as the clean plate.

### Step 5 — Verify and stop

- The assemble helper already samples the mid tail at `T2 − 1 s`,
  `T2 − 0.5 s`, and `T2 − 1 frame` (still Part B) and exits non-zero on
  Finder/Desktop-like pixels. Do not skip that QC for a release candidate.
- Also sample frames at opening, mid-screenshare, and close by eye.
- Confirm the product label and a clean product frame.
- Confirm the clean plate has no music. If you mix music, keep
  `*-rc-no-music.mp4` as the clean plate.
- Record local notes. Do not treat the RC as a public post until publication
  gates in the episode production-requests file are green.

Screenshare shot detail:
[`../transcripts/SARAH_VIDEO_SCREENSHARE.md`](../transcripts/SARAH_VIDEO_SCREENSHARE.md).

## Episode 262 lessons (2026-07-24)

Use this section before every new Sarah episode RC. It records failures that
must not repeat, the proven music path, and tooling work that is still open.

### Circles and failures to never repeat

1. **Screenshare leak into Finder/Desktop.** Recording past Omega into Finder
   or Desktop at the end of Part B spoiled about the last 0.9 s of the mid on
   Episode 262. Always verify frames at `T2 − 1 s`, `T2 − 0.5 s`, and `T2`
   before you declare the RC good. Mitigation shipped in #9233:
   `omega-screen-control` hard-stops capture before quit, defaults
   `--safe-tail-seconds 1` on `record-motion`, and trims Finder/Desktop-like
   tails when the frame heuristic fires.
2. **Maximized Omega / Retina crop / cover-fill zoom.** Maximized windows and
   cover-fill zoom crushed Welcome. The winning path: window about
   `1280×900` (not maximized), FIT decrease, dark pad `0x0E0E10`, crop the
   full window.
3. **Never quit with Cmd+Q during capture.** Use SIGTERM only through
   `omega-screen-control quit`. Cmd+Q can quit the wrong app (Cursor or
   another frontmost app). The tool refuses `key --combo cmd+q` (#9233).
4. **One continuous mid looked like one still.** Need a second mid beat
   (cutaway still or second shot) for visual variety and to cover a bad
   tail. Episode 262: Welcome motion, then `omega2` README still for the
   last about 6 s of the mid.
5. **Music had no recipe.** Older notes said "owner music pass" but gave no
   generate or mix steps. Agents searched the wrong places. The proven
   ElevenLabs Music path is below. Episode 261 bed reuse is a fallback only.
6. **TTS product name `zed`.** For British pronunciation `/zed/`, write
   lowercase `zed` in the paste transcript. Uppercase `Zed` can become
   "Zeed".
7. **Spoken authority.** Paste-only file:
   `~/Desktop/Sarah/<ep>/<ep>transcript.md`. When the script locks, update
   `docs/transcripts/<ep>.md` to the final spoken text. Header metadata stays
   in the repository transcript. The Sarah body is spoken words only.
8. **A / B / C.** Continuous Sarah audio. Picture cuts only. Episode 262
   proven cuts: T1 after `zed` (about 17.50 s), T2 ≈ `0.78 * D`. Verify on
   the audio. Do not copy older fractions when they disagree.
9. **Omega reopen mess (product capture).** For the owner, Help → Editor
   Onboarding and Welcome "Return to Onboarding" appeared to do nothing.
   `⌘P` is the file picker, not the command palette (`⌘⇧P`). Treat this as
   known product failure plus the code improvements below. Do not claim it
   is fixed until it is fixed.

### Music bed (ElevenLabs)

Secret file: `~/work/.secrets/elevenlabs.env` with `ELEVENLABS_API_KEY`.
Never print the key. Never invent a key. If the file or key is missing, say
so and stop the music path (or fall back only when the owner accepts an
existing bed).

On 2026-07-24 this machine had no `~/work/.secrets/elevenlabs.env`. Create
that file before you generate a new bed.

Generate:

```sh
# Load key from ~/work/.secrets/elevenlabs.env (do not echo it).
# music_length_ms = RC duration in milliseconds.
curl -sS -X POST \
  "https://api.elevenlabs.io/v1/music?output_format=mp3_48000_192" \
  -H "xi-api-key: ${ELEVENLABS_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "model_id": "music_v2",
    "force_instrumental": true,
    "music_length_ms": <RC_DURATION_MS>,
    "prompt": "Instrumental cinematic sci-fi underscore. Sparse midrange. No vocals. Calm pulse under spoken narration."
  }' \
  -o ~/Desktop/Sarah/<episode>/<episode>-music-bed.mp3
```

Mix (audible Episode 261 louder path):

1. `loudnorm` the bed.
2. Apply volume about `-3 dB`.
3. Fade in 1.5 s. Fade out 3 s.
4. `amix` with Sarah voice, then `alimiter`.
5. Write `*-rc-with-music.mp4`. Keep `*-rc-no-music.mp4` as the clean plate.

### Code and tooling improvements (tracked candidates)

Items 1 and 2 shipped (#9233, #9234). The rest remain future fix work. Do not
treat open items as done.

1. **`scripts/omega-screen-control` (shipped #9233):** Hard-stops recording
   before app teardown. Rejects Cmd+Q key quit. Optional
   `--safe-tail-seconds` freezes the last clean frame (default `1` on
   `record-motion`). Trims Finder/Desktop-like tails when the frame heuristic
   fires. Tests:
   `node --test scripts/omega-screen-control/recording-lifecycle.test.mjs`.
2. **RC assemble helper (shipped #9234):**
   `scripts/sarah-avatar/assemble-rc.mjs`. One command that takes
   sarah-master + screenshare + optional cutaway stills + T1/T2, writes the
   no-music RC, derives T1 from the post-spoken silence window (not folklore
   fractions alone), supports a second mid cutaway beat, checks Desktop paste
   against `docs/transcripts/<episode>.md`, verifies frames at the cut
   points, and exits non-zero if Finder-like pixels appear near T2. Tests:
   `node --test scripts/sarah-avatar/assemble-rc.test.mjs`.
3. **`scripts/sarah-avatar/elevenlabs-music-bed.mjs` (or similar):** Read
   `elevenlabs.env`, generate a bed into the episode folder, optional
   `--mix` onto the RC. Document that script here when it lands.
4. **Segmind/TTS paste lint:** Warn if uppercase `Zed` appears in the spoken
   paste when the British voice is selected.
5. **Omega product (`OpenAgentsInc/omega`):** Make Help → Editor Onboarding
   and Welcome "Return to Onboarding" reopen first-run/editor onboarding
   reliably (fix silent no-op when `with_local_workspace` fails or a
   completion KVP blocks). Add matching command-palette strings. Document
   `⌘⇧P` vs `⌘P` in Omega help. Add a reset-onboarding action that clears
   completion KVPs for development builds.
6. **Cutaway inventory:** Keep cutaway stills under the episode Desktop
   folder or ignored `.artifacts/episode-N/cutaways/` with a manifest.
   Do not rely on one screenshare duration alone. Tracked as #9237.

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
- The **Segmind API key** stays in `~/work/.secrets/segmind.env`. Never commit
  or print.
- The **ElevenLabs API key** stays in `~/work/.secrets/elevenlabs.env`. Never
  commit or print.
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
- Wire the music and RC helper scripts listed in **Episode 262 lessons**. Fix
  the Omega onboarding reopen bug. Keep publication gates for each episode RC
  under `~/Desktop/Sarah/<episode>/`. The music generate/mix path is documented
  in this file. It is not yet a committed helper script.
