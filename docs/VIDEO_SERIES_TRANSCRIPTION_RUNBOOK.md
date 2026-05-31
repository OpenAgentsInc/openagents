# Video Series Download And Transcription Runbook

Status: operator runbook

Last updated: 2026-05-31

## Purpose

This runbook defines the retained process for turning the OpenAgents GitHub wiki
video-series index into local episode transcripts.

The workflow supports two operator modes:

- transcribe one specific episode by number
- transcribe every wiki episode that does not yet have
  `docs/transcripts/<episode>.md`

The script intentionally keeps large downloaded media and raw transcription JSON
out of Git. Only reviewed markdown transcripts should be committed.

## Sources

The canonical video-series index is the GitHub wiki page:

- <https://github.com/OpenAgentsInc/openagents/wiki/Video-Series>

For automation, use the raw wiki markdown:

- <https://raw.githubusercontent.com/wiki/OpenAgentsInc/openagents/Video-Series.md>

The raw markdown is easier to parse because each episode is a numbered markdown
link:

```markdown
224. [Distributed Training 101](https://x.com/OpenAgents/status/2044890647342027072)
```

As of this runbook, the raw wiki contained 227 episode links.

## Local Layout

Tracked outputs:

- `docs/transcripts/<episode>.md`

Ignored local working files:

- `var/video-series-transcripts/<episode>/episode.info.json`
- `var/video-series-transcripts/<episode>/media-*`
- `var/video-series-transcripts/<episode>/audio-part-*.mp3`
- `var/video-series-transcripts/<episode>/transcription-part-*.json`

`var/` is already ignored by this repo. Do not commit downloaded video, audio,
or raw API JSON unless a future task explicitly asks for an archival artifact.

## Dependencies

Required local commands:

- `python3`
- `ffmpeg`
- `ffprobe`
- either `yt-dlp` or `uvx`

The script prefers a local `yt-dlp` binary. If that is missing, it runs
`uvx yt-dlp`, which downloads and runs an ephemeral/cached copy without adding a
repo dependency.

Transcription requires an OpenAI API key. The script reads either:

- `OPENAI_API_KEY`
- `PROBE_OPENAI_API_KEY`

For this workspace, the reusable local fallback secret is outside this repo:

```bash
../.secrets/probe-openai.env
```

Do not print or commit the key.

## Transcription API Notes

The default model is:

```text
gpt-4o-transcribe-diarize
```

The script requests:

- `response_format=diarized_json`
- `chunking_strategy=auto`
- `language=en`

This produces speaker-labeled segments when the model can infer speakers. The
script converts those segments into markdown lines:

```markdown
**[00:00] Speaker 0:** Text.
```

Use `--speaker` mappings when a reviewed episode has known speakers:

```bash
--speaker speaker_0="Christopher David" --speaker speaker_1="Car Gonzalez"
```

Speaker labels are model-derived. Review and correct them before treating the
transcript as quote-grade source material.

## Basic Commands

List missing transcripts without downloading:

```bash
python3 scripts/transcribe-video-series.py --missing --dry-run
```

Transcribe one episode:

```bash
python3 scripts/transcribe-video-series.py \
  --episode 1 \
  --env-file ../.secrets/probe-openai.env
```

Transcribe one episode with known speaker names:

```bash
python3 scripts/transcribe-video-series.py \
  --episode 224 \
  --env-file ../.secrets/probe-openai.env \
  --speaker speaker_0="Christopher David" \
  --speaker speaker_1="Car Gonzalez"
```

Transcribe all missing episodes:

```bash
python3 scripts/transcribe-video-series.py \
  --missing \
  --keep-going \
  --env-file ../.secrets/probe-openai.env
```

Limit a missing run to a small batch:

```bash
python3 scripts/transcribe-video-series.py \
  --missing \
  --limit 5 \
  --keep-going \
  --env-file ../.secrets/probe-openai.env
```

Overwrite an existing transcript:

```bash
python3 scripts/transcribe-video-series.py \
  --episode 224 \
  --overwrite \
  --env-file ../.secrets/probe-openai.env
```

## Process Details

### 1. Fetch The Wiki Index

The script downloads the raw wiki markdown and parses numbered markdown links.
It does not scrape GitHub HTML.

The parsed record is:

- episode number
- episode title
- episode URL

### 2. Decide The Episode Set

With `--episode <number>`, the script selects exactly that episode.

With `--missing`, it selects every wiki episode where
`docs/transcripts/<episode>.md` is absent.

Existing transcripts are skipped by default. Use `--overwrite` only when the
operator deliberately wants to regenerate a transcript.

### 3. Download Metadata

For each selected episode, the script runs:

```bash
yt-dlp --dump-single-json --skip-download <episode-url>
```

The metadata is retained as:

```text
var/video-series-transcripts/<episode>/episode.info.json
```

Twitter/X posts often appear as playlists because a single post can contain
multiple video parts. The metadata records each media entry and duration.

### 4. Download Media

The script downloads low-resolution media by default:

```text
bestvideo[height<=270]+bestaudio/best[height<=270]/worst
```

This keeps operator tests cheap while still preserving usable audio. The media
is written under the ignored episode work directory.

### 5. Extract Normalized Audio

Each downloaded media file is converted with `ffmpeg` to:

- mono
- 16 kHz
- MP3
- 64 kbps

This keeps files small enough for normal transcription requests and removes
video payload from the API call.

### 6. Transcribe Each Part

Each audio part is sent independently to the transcription API. The raw JSON
response is saved in the ignored episode work directory.

If `yt-dlp`, `ffmpeg`, or transcription fails for one episode in a batch run
with `--keep-going`, the script records:

```text
var/video-series-transcripts/<episode>/failure.json
```

and continues. This is expected for deleted posts, posts without downloadable
media, or temporary X/Twitter extractor failures.

For multi-part X posts, the script offsets later segment timestamps by the
duration of previous parts before writing the final markdown transcript.

### 7. Write Markdown

The final transcript is written to:

```text
docs/transcripts/<episode>.md
```

The generated file includes:

- title
- source URL
- wiki source URL
- model
- generated timestamp
- transcript body with timestamped speaker lines

The body follows the same broad structure as the existing retained transcripts:
markdown, speaker names in bold, timestamps, then utterance text.

## Review Checklist

Before committing a generated transcript:

1. Confirm the source URL and episode number match the wiki.
2. Confirm no raw API key or local secret path was written into the transcript.
3. Scan speaker labels and rename obvious speakers.
4. Spot-check timestamps near the start, middle, and end.
5. Confirm downloaded media and JSON stayed under ignored `var/`.
6. Stage only the script, runbook, and intended markdown transcript.

## Known Limitations

- X/Twitter extraction can break when X changes its guest-token or media APIs.
  `yt-dlp` updates usually fix this.
- Some wiki links may no longer expose downloadable video to `yt-dlp`. Batch
  runs should use `--keep-going` and inspect `failure.json` files afterward.
- Diarization is model-derived. It is useful for structure, not final speaker
  authority.
- The script does not commit or archive media files.
- The script does not yet create pull requests or update the wiki.
- The script does not try to infer canonical human speaker names. Use
  `--speaker` mapping or edit the generated markdown after review.

## Current Proof

The first retained smoke test should be a missing short episode, not an existing
214-227 transcript, so the test proves both download and new markdown output
without overwriting prior work.

The recommended smoke command is:

```bash
python3 scripts/transcribe-video-series.py \
  --episode 1 \
  --env-file ../.secrets/probe-openai.env
```
