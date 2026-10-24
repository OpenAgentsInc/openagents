# Medieval Town showcase reconstruction

Status: full reconstruction captured and reviewed, 2026-10-23. Visual parity
remains incomplete.
The source is Hivemind's
[Modular Medieval Town showcase](https://www.youtube.com/watch?v=QDKrtfXxdG0),
published on April 15, 2023. This is an Aethyrforge reconstruction study of the
purchased pack, alongside the [still-image shot guide](medieval-town-shots.md).
Camera fit, image quality, and animation fidelity are separate measurements.
None of the measurements below establish complete visual parity.

## Source and inspection

The private reference download contains 10,907 H.264 frames at 1920 × 1080 and
60 frames per second. The video stream lasts 181.783 seconds; the audio/container
lasts 181.859 seconds. Its SHA-256 is
`8eb40bca6c21f387ab51cf6b338ca8bc03056606faf0831ec3e21b33eacd5e8d`.
The download, decoded images, licensed assets, recovered poses, and generated
movies stay outside Git.

Every decoded frame has an indexed presentation timestamp and a 480 × 270
analysis image. Optical flow and image-change measurements cover all 10,906
consecutive intervals. A reviewer inspected the overview, each shot's arc, and
before/after images around the 14 largest early motion spikes. Those spikes are
continuous motion and near-camera occlusion, not 14 additional cuts. This is
frame-by-frame computational analysis plus selected visual inspection; it is not
a claim that a person visually inspected every frame.

The first continuous flight commonly holds an image for two or three output
frames, consistent with approximately 24 fps motion placed in a 60 fps file.
Later feature shots show motion on consecutive 60 fps frames. A reconstruction
should preserve the observed cadence per shot, rather than apply one global
motion rate. Compression, animated objects, and exposure changes prevent
treating every small image difference as a new camera pose.

## Shot and motion map

Times describe the reference video. The first flight's phases are analytical
subdivisions of one continuous movement. Fade boundaries are approximate; a
nearly black interior does not uniquely identify an editorial cut.

| Reference time | Subject | Reconstruction work |
| --- | --- | --- |
| 0–4.033 s | Intro graphic | Use an Aethyrforge title, with its own attribution |
| 4.033–12.167 s | Interior window reveal | Window geometry, shutter motion, exposure transition, forward camera |
| 12.167–19.500 s | Fast plaza flight | Translation and turns; lens estimates are ambiguous in part of this interval |
| 19.500–26.917 s | Bunting and flowers | Continue the same flight through near foreground objects |
| 26.917–31.333 s | Green-shuttered interior | Pass through the opening and turn toward the sky |
| 31.333–51.333 s | Sky and birds | Match view rotation and flock motion; sky alone cannot recover camera translation |
| 51.333–57.567 s | Day plaza | Forward/up dolly and tilt, with a stable lens |
| 58.367–64.467 s | Café and fountain | Slow diagonal push, stable lens, narrow letterbox |
| 65.483–74.050 s | Flowering trees and pavilion | Foreground foliage, lateral motion, focus and exposure |
| 75.083–83.083 s | Waterwheel | Camera push, wheel rotation, canal water, and spray |
| 84.100–91.517 s | Day street | Slow rising push and tilt, stable lens |
| 92.550–119.183 s | Blue entry and green stairs | Door movement, interior route, exposure and focus changes |
| 120.667–135.600 s | Night interior and door | Interior lighting, opening door, and forward camera |
| 136.917–143.317 s | Night plaza | Slow push and tilt, narrow letterbox |
| 144.383–149.267 s | Night street | Warm local lights and diagonal tracking |
| 150.350–173.200 s | Night window, moon, and chimney | Window approach, upward tilt, clouds, and chimney smoke |
| 176.183–179.100 s | Outro | Use Aethyrforge attribution, followed by black |

Letterboxing varies between shots. Do not apply a global crop. Source logos,
feature captions, and soundtrack are not rendered scene geometry.

## Camera evidence

Reference and earlier Aethyrforge renders provide feature descriptors. Their
pixels are mapped to the owned scene's static geometry, then matched to video
images. Robust perspective fitting estimates camera position, orientation, and
lens; tracked points extend a fit through adjacent frames. Vegetation and effect
cards are excluded from architectural anchors. Invalid or invisible ray hits
must be rejected rather than treated as recovered landmarks.

A fixed lens resolves the main plaza, café, and day-street intervals well.
At 960-pixel analysis width, initial tracked-camera median reprojection errors
are approximately 1.21, 0.68, and 0.83 pixels respectively. Their held-out point
errors are approximately 1.40, 0.81, and 0.84 pixels. The night plaza's medians
are approximately 0.86 and 0.95 pixels. Fitted full-frame vertical fields of view
are approximately 54.5°, 36.4°, 35.1°, and 54.4° respectively.

These are geometric consistency checks, not image similarity scores. Raw
per-frame estimates can jump despite small projection errors. The recording
uses stable camera keys and smooth rails, with separate rails across cuts.
White exposure transitions, heavy blur, near occlusion, planar architecture,
and unlit interiors reduce observability. Unsupported views require an explicitly
authored reconstruction; they must not be described as recovered source cameras.

The strongest fits cover the day plaza, café, day street, and night plaza.
The approximately 15–19.5 second flight uses provisional fixed-lens keys:
consistent point subsets fit closely, but many held-out matches disagree under
blur and occlusion. It is not qualified to the same standard as the four slow
exterior shots. This does not recover the whole film. The remaining camera paths
use source geometry and selected visual
landmarks, followed by rendered framing checks. The opening and closing window
shots use the actual open green shutters, balcony, and scalloped awning of
`BP_residential_house_02`. The blue entry and stair shots use the actual door and
staircase of `BP_residential_house_41`.

The middle flight's original window identity remains unresolved. Its replacement
uses two real lower-floor doorways in `SM_wall_A_2x4_01_99_Blueprint`, with their
independent leaves opened for capture. This is an owned enter-room-exit stand-in;
its height, paint, framing, and route differ from the reference's upper windows.
It does not create an opening by hiding a merged wall or window surface. Sky-only
intervals use an authored view rotation and carried position because clouds and
birds do not establish camera translation.

## Implemented capture path

`packages/renderer/camera-rail.ts` adapts the public OpenAgents Film Lab camera
path from commit `83fe8b7f6b3cd437ae9b7af213cf9f04875dd793`:
[`camera_rig.rs`](https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/crates/verse-engine/src/camera_rig.rs)
and [`film_shot.rs`](https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/crates/verse/examples/film_shot.rs).
Apache-2.0 attribution is in `third_party/openagents-camera/`.
Centripetal Catmull–Rom paths interpolate position and target. Lens interpolation
uses focal length in logarithmic space. Transported up vectors and explicit roll
are owned additions. This is not an arc-length or constant-world-speed solver.

A manual film session stops the live animation loop and input. Each requested
frame supplies its own world time and optional camera pose, including up vector,
lens, and vignette. An optional absolute render exposure from 0.01 to 64 applies
to that frame, including exposure-aware source effects; it never changes saved
lighting. The kernel's player position and live camera are independent of the
capture pose. The session pins its resolution, allows one capture in
flight, and reuses readback storage. Cancellation rejects pending work and
suppresses late downloads. Ending the session restores live size and world age.

A short, isolated 640 × 360 browser qualification made 11 world submissions.
Raw RGBA and decoded PNG bytes matched exactly; time 0 → 3 → 0 replay matched
exactly. Roll, wider field of view, pinned dimensions, idle/manual scheduling,
error rejection, cancellation, and live-state restoration passed. It reported
no GPU or page errors and left no test browser running. This qualifies the
capture mechanism at that workload, not the final movie or all camera paths.

`capturePixels()` returns packed RGBA8 as a transferable `ArrayBuffer`. Both
message hops transfer ownership; this path creates no Blob. The recorder uses
this API because sustained full-resolution Blob uploads exhausted Chrome
storage after approximately 390 frames in the initial long-run check.

With `--codec h264`, `tools/record-film.mjs` encodes in the browser through
WebCodecs. It applies editorial fades and black bars to the captured sRGB pixels,
then submits one RGBA `VideoFrame` at the exact output timestamp. The encoder
requests H.264 High Profile, Annex B output, a 16 Mbit/s bitrate, and preferred
hardware acceleration. Browser support and actual hardware use are separate;
unsupported configurations fail rather than silently switch codecs. One frame
finishes encoding before the next begins, and each encoded upload is capped at
16 MiB.

Only the compressed H.264 bytes reach the local receiver. FFmpeg copies that
stream into MP4, assigns the rational frame timestamps, and moves the MP4 index
to the front; it does not re-encode the video. The mux process uses at most two
threads. Output contains no audio. The recorder does not upload raw full-size
pixels in this mode or hold an entire uncompressed film in memory. Held frames
reuse the preceding scene capture but still receive an encoded output frame.
Each newly rendered frame is followed by a bounded pause.

The report beside the private MP4 records output timestamps, hashes of the edited
pixels before encoding, encoded upload sizes, capture durations, Node process
memory samples, and failures. H.264 is lossy, so these pixel hashes do not imply
that decoding the MP4 reproduces the RGBA bytes exactly. A retry resends the same
encoded bytes; the receiver checks their hash and acknowledges the most recent
frame without muxing it twice. The legacy `--codec rgba` path remains available
and is still the command's default; it uploads raw frames and runs FFmpeg's H.264
encoder instead. Choose `h264` explicitly for browser-side encoding.

```sh
openagents browser run -- node --import tsx tools/record-film.mjs \
  --timeline /private/path/timeline.json \
  --out /private/path/aethyrforge-medieval.mp4 \
  --codec h264 \
  --pause-ms 150
```

The timeline format is `aethyrforge.film.v1`; admission and editorial operations
are in `tools/film/timeline.ts`. `--validate true` checks a timeline without opening
a browser. Output and partial filenames must be unused and outside the checkout.


## Temporary source motion

A frame can rotate up to 16 source instances around local hinges or hubs.
Selectors bind the day/night mode, instance index, mesh ID, and a SHA-256 of the
saved model matrix (16 little-endian Float64 values). Repeated labels alone do
not select an object. Admission rejects unsupported scale, shear, wind-driven
geometry, and instances absent from the prepared draw ranges.

The capture updates CPU and GPU transforms and bounds, clears view and reflection
caches, and redraws the shadows. It restores transforms and clears caches again
when the capture succeeds or fails. This deliberately pays for full shadow
refreshes during offline capture; it is not a claim of an optimized live rigid
animation subsystem. Original baked GI still describes the saved geometry, so
newly opened doors do not yet update indirect light transport.

The private scene includes the actual waterwheel, blue entry leaf, and open
green shutters. Their model geometry and saved transforms are recovered facts.
The film's wheel rotates at an owned +7.5° per second around its local Z axis;
that rate, sign, and phase are not recovered source animation. Door-opening
curves and unsupported camera paths are also owned reconstruction choices.
The actual green shutters remain open; their presence does not establish an
opening animation in the reference. Rotating a doorway wall instead of its leaf
is not an acceptable substitute.

A private film snapshot can also select the optional foliage material and
light-transport profile `bounded-forward-sheet-v1`, with scattering parameter
`g = 0.5`. It duplicates the selected leaf material without changing the original
and rebuilds the 3,315-probe, 128-ray GI field for both scenarios. Wind is
unchanged. Fixed-view checks leave untagged MT-13 pixels exactly unchanged and
show darker leaf interiors closer to the references. This is an owned
approximation, not a recovery of the source leaf-scattering model, and it does
not replace the interactive world's saved profile. Keep its identity with the
recording evidence. Rigid door and wheel transforms remain temporary; static
baked GI and the unknown original temporal history remain limitations.

## Final capture results

The completed 829-frame exterior capture and 7,990-frame remaining reel are
1920 × 1080, H.264 High Profile, with exact 60 fps timestamps. Both report no
page or GPU errors and no transport failures. The remaining reel makes 6,513
world captures and preserves 1,477 near-duplicate intervals from the first
flight as held frames. It takes 1,351 seconds with a 150 ms pause after each
world capture; median capture/readback time is 27.1 ms, with a 117.2 ms p95.
These timings include the capture API and readback, not only GPU execution.

The remaining reel uploads 264.04 MB of compressed video versus 66.27 GB of
equivalent raw pixels, about 251 times less. Warm Node process samples range
from 595 to 628 MB, with nonmonotonic buffer sizes. These samples do not measure
Chrome or GPU memory. The separate 829-frame check uses 27.18 MB of uploads and
samples 349–355 MB of warm Node memory; its shorter workload is not a
whole-film memory bound.

The first assembled film passes complete frame-count and timestamp checks:
10,907 frames at 60 fps, lasting 181.783333 seconds, with no audio or decoding
errors. A reviewer inspected 20 source/render pairs. The first full movie stays private
as a comparison artifact.

That review led to seven targeted replacement sections: a brighter opening
window, an unobstructed sky view toward the source gull emitters, the tighter
pavilion lens and upward tilt, a wider and brighter waterwheel view, a floor-level
interior tour, a later night door opening, and a tighter night street view.
A separate 150-frame replacement smooths the turn after entering the house;
interpolating a nearby target through the camera had caused a sharp rotation.
The corrected interval interpolates unwrapped yaw and elevation while preserving
eye position, lens, exposure, world time, and door pose.

The revised private output is `aethyrforge-medieval-reconstruction-final.mp4`:
1920 × 1080, 10,907 H.264 frames, exactly 60 fps, 181.783333 seconds, and no
audio. It contains newly rendered Aethyrforge scenes and owned title cards;
no source video frames or soundtrack are assembly inputs. Its size is
404,199,090 bytes and its SHA-256 is
`a602086d20c4f49899ca031850ff3b19c65d1f8188d5ede141f273d7fdcc36ef`.
The source order, clip lengths, black gaps, and shot-specific letterboxing are
preserved. All decoded frame timestamps are checked at the exact 60 Hz grid.
A final metadata-only copy declares BT.709 primaries and matrix, the sRGB
transfer curve, and limited range in H.264. It does not re-encode the images.
All 10,907 decoded YUV frame hashes match the preceding version, and the final
file passes complete frame, packet timestamp, and decoding checks.

The revision reel has 5,469 frames, including 842 holds, with 4,627 world
captures. It reports no page, GPU, or transport errors. Its median capture and
readback time is 31 ms, with a 100.3 ms p95, across a 976.972-second paced run.
The 150-frame turn replacement also completes without capture errors. Final
visual review covers 20 source/render pairs and 23 path samples: 40 distinct
output frames. The revised file is opened in QuickTime Player. The capture
browser and film server are stopped. The private evidence directory retains
both movie versions, input hashes, camera keys, frame ledgers, capture reports,
full timestamp checks, and comparison sheets.

A 60 fps presentation rate does not establish 60 fps live rendering.
Remaining parity work includes the unrecovered middle window route, authored
flight and interior camera framing, pedestrian animation, indoor lighting, cloud
appearance, water and particle motion, depth of field and motion blur, and
indirect light changes when doors open.
