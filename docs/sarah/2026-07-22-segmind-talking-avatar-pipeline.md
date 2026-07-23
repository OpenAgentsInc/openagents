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
   (no separate audio hosting). **This is the model used for the first test
   below.**

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

- The **video output stays local** (kept under `~/Downloads/sarah/`). Do not
  commit generated clips.
- The **API key** stays in `~/work/.secrets/segmind.env`. Never commit or print.
- Only this runbook and `scripts/sarah-avatar/segmind-talking-avatar.mjs` are
  committed.

## What to decide next

- Which model to standardize on for Sarah's comms after the A/B (Fabric vs Kling
  vs Pruna).
- Where produced clips are stored and how they attach to a comm (the owner tweet
  queue `docs/sarah/SARAH_TWEET_QUEUE.md`, a blog post, or the timeline once the
  outward web-communications broker is admitted — see
  `docs/sarah/2026-07-22-sarah-company-command-analysis.md`).
- Voice source: Segmind built-in TTS vs our own voice (the OAV/pipecat voice
  stack) fed in as `--audio`.
- Cost and rate posture for routine comms generation.
