# Episode video stitch runbook

Date: 2026-08-21
Status: required procedure for every `NNNa` + `NNNb` X-HD stitch
Failure this documents: Episode 274 audio desync after the part-A/part-B join

Use this file before stitching the next episode. Do not reuse the 270–273
concat-filter command.

## What failed on 274

The first `274.mp4` used the same FFmpeg **concat filter** recipe as 270–273:

1. Scale/pad each clip to 1920×1080.
2. Force video to `fps=30`.
3. Resample audio 48 kHz mono → 44.1 kHz stereo.
4. `setpts=PTS-STARTPTS` / `asetpts=PTS-STARTPTS` on each stream.
5. `[v0][a0][v1][a1]concat=n=2:v=1:a=1`.

That concat filter appends **video independently of audio**. It does not keep
the A/V pair of clip A glued together, then glue clip B after that pair. It
glues all video, then glues all audio.

Measured on the 274 sources:

| File | Video duration | Audio duration | Audio minus video |
| --- | --- | --- | --- |
| `274a.mp4` | 990.316 s | 990.244 s | −72 ms |
| `274b.mp4` | 148.985 s | 148.885 s | −100 ms |

After concat, clip B started ~72 ms earlier on the video timeline than on the
audio timeline. The offset is locked in from the join at **16:30** through the
end of the file. That is why it looked fine early and “massively out of sync
later.”

Two extra steps made the gap worse than a raw 72 ms source mismatch:

- `fps=30` rewrites the video clock (duplicate/drop) without stretching audio
  to match.
- `aresample` 48 kHz → 44.1 kHz can change audio duration again.

The broken encode was moved to
`~/Downloads/274-desync.mp4`. The replacement `~/Downloads/274.mp4` used the
procedure below.

270–273 used the same filter and did not blow up as badly because their
per-clip A/V gaps were smaller and their joins were a smaller fraction of
runtime. The recipe was still wrong. Do not use it again.

## Do not do this either

`apad` plus `-shortest` on AAC is not a fix. On the 274b retry it left audio
~10 s **longer** than video (`A-V delta ≈ +10 s`) because `-shortest` did not
cut the padded AAC at the video end. If audio must be extended, pad to an
**exact** `whole_dur=` equal to the video duration, or cap both streams with
`-t <video_duration>`.

## Required stitch procedure

Probe first. If video duration and audio duration differ by more than a few
milliseconds, say so and use this procedure anyway.

```sh
# 1. Encode each source alone. Lock audio to that clip's video clock.
#    Do not fps=30. Do not resample 48k -> 44.1k. Do not concat yet.
vdur=$(ffprobe -v error -select_streams v:0 -show_entries stream=duration -of csv=p=0 NNNa.mp4)

ffmpeg -y -fflags +genpts -i NNNa.mp4 -t "$vdur" \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease:flags=lanczos,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,format=yuv420p,setpts=PTS-STARTPTS" \
  -af "asetpts=PTS-STARTPTS,aresample=48000:async=1:first_pts=0,aformat=sample_fmts=fltp:channel_layouts=stereo" \
  -c:v libx264 -profile:v high -level 4.1 -pix_fmt yuv420p \
  -preset medium -crf 18 -bf 0 \
  -x264-params "keyint=60:min-keyint=60:scenecut=0" \
  -maxrate 10M -bufsize 20M \
  -c:a aac -b:a 160k -ac 2 -ar 48000 \
  -muxpreload 0 -muxdelay 0 \
  -movflags +faststart -brand mp42 \
  -video_track_timescale 15360 \
  /tmp/nna.mp4

# Repeat for NNNb.mp4 -> /tmp/nnb.mp4

# 2. Concat at the file level so each clip stays an A/V pair.
printf "file '%s'\nfile '%s'\n" /tmp/nna.mp4 /tmp/nnb.mp4 > /tmp/list.txt
ffmpeg -y -f concat -safe 0 -i /tmp/list.txt -c copy \
  -movflags +faststart \
  ~/Downloads/NNN.mp4
```

Then check A/V duration on **each** normalized clip and on the final file.
Audio minus video should stay under ~200 ms (AAC priming). If it is seconds,
stop and do not publish.

Playback-check the join: start ~10 s before part B and watch through the end.
Lips and UI clicks must stay with the voice after the cut.

## Rules

- Never use the `concat` **filter** to join episode parts.
- Never force `fps=30` on these screen recordings.
- Never resample 48 kHz → 44.1 kHz as part of a multi-clip stitch.
- Never concat two sources whose video and audio durations differ without
  locking each clip first.
- Blackouts, if any, apply on the **normalized** part-A file, not inside a
  two-input concat filter.
- Keep this runbook next to the transcripts. The next stitch looks here first.
