# Echo native recording audit — #12984

The parent issue is closed by the [afternoon follow-up](performance.md#afternoon-follow-up-who-uses-the-gpu-and-the-scenes-own-cost):
the live misses come from another app's GPU work, and the scene needs at most
about 8 ms of GPU a frame. This audit found and fixed a commons-spawn gameplay
regression and made the performance qualification reject missing characters.
Historical R1–R4 captures and passing measurements remain useful evidence;
they are not a fresh performance pass.

The [rendering follow-up](performance.md) adds conservative grass ranges,
shader work rejection and a deterministic 217-frame image comparison. Its
source/build receipts supersede this initial audit for changed files; live
60 FPS qualification remains pending a controlled GPU window.

Audit source base: `4522812690`. Historical R4 implementation and qualification:
`e1af503e1de3cdf0d403dd017bd17636d2eb4c20`.

## November 7 fixes and current measurements

The player spawn moved from z=−84 to the commons at z=−16 after R4. Friendly
Echo's initial (1, −16) position then fell inside the cult's 12 m spawn exclusion.
The generic raid routing and final position correction pushed her about 11 m
on her first step; the live wide-shot timer could wait three minutes without
her casting. The exclusion now applies only to hostile actors. Friendly town
actors retain normal walls/buildings and collision routing. Tests cover her
first walking step and her natural first Meteor Swarm, plus the existing watch
combat and hostile siege regressions. The rebuilt worker is 1,068,308 bytes.

A fresh native raid capture passes all 246 frames with no console errors.
Its H.264 video is 1920×1080 at 24 fps, 10.25 s; inspected frames 30 and 180
show the commons walk and subsequent cast pose. Capture stepping waits for the
GPU per frame and is not a real-time 60 FPS result.

The performance timer now waits for Echo's pass when requested, fails promptly
on a terminal character-load error, and qualifies only a ready character with
nonzero groom draws at the exact requested raster. Suppressed, absent or
reduced-raster runs cannot qualify. The two new authority helpers kill all
25 mutants with zero survivors. Red/green check records are retained here.

All fresh measurements below used muted headless Chrome, hardware Metal WebGPU,
DPR 1 and an actual 1920×1080 raster on this Mac, on AC power. Existing owner
applications were left running; competing GPU load has not been ruled out.
A controlled baseline uses the original R4 Everglade Wasm and host.js with the
unchanged current character runtime/shader and matching avatar asset pack.
It is not a complete historical checkout. No thermal or performance warning
was recorded by `pmset -g therm`.

| Fresh run | Browser | Frames / FPS | p95 / max, ms | Held | Result |
| --- | --- | --- | --- | ---: | --- |
| Current medium | 157 | 259 / 51.8 | 33.330 / 33.335 | 41 | Fail |
| Current wide, after spawn fix | 157 | 213 / 42.6 | 33.335 / 50.000 | 87 | Fail; natural first cast now reached |
| Controlled R4 medium | 157 | 264 / 52.8 | 33.335 / 33.335 | 36 | Fail |
| Current medium | 156 | 264 / 52.8 | 33.335 / 33.340 | 36 | Fail |

The fresh no-character run is explicitly diagnostic and exits 1; a missing
private pack also exits 1 promptly. Neither can be used to close the issue.
The timing rule is unchanged: p95 ≤17.5 ms and no frame over 33 ms, with more
than 30 frames. A controlled qualification without competing 3D previews is
still needed; the cause of the current miss is not established.

## Historical delivery evidence

| Parent requirement | Delivered evidence |
| --- | --- |
| One renderer, frame and depth buffer; no composite | [R1 #12985](https://github.com/OpenAgentsInc/openagents/issues/12985), `5a8378b397`; the character pass shares Everglade's colour/light/aux targets, depth and shadow map. [R3 #12987](https://github.com/OpenAgentsInc/openagents/issues/12987), `a88dad32b0`, retires the old matte/composite controls. |
| Echo occludes and is occluded by the world | R1 and R3's actual Mac captures cover player, posts, grass and rain. R4 retains 217 orbit and 246 raid frames, including the complete projected-head review. This audit also inspected retained full orbit frames 50 and 170. |
| World's lamps and two-way shadows | R1 verifies SceneLights key/ambient/nearby lamp/fog/wetness inputs, a caster in the world's shadow map and character sampling of that map. Its day and night/rain captures and debug shadow-term observation are recorded in the R1 closure. |
| Shared CPU character runtime and idle face | [R2 #12986](https://github.com/OpenAgentsInc/openagents/issues/12986), `ac3f365193`, moves the runtime to `aethyr/character/`; R3 adopts it in the native staged page. |
| 1920×1080 orbit recorded directly from Everglade | The retained `orbit.mp4` is H.264, 1920×1080, 24/1 fps, 217 frames, 9.041667 s. The original capture reports `PASS 217 orbit frames, no console errors` from `?stage=echo&clock=0.95&weather=rain` and `coaxialClipStep`. |
| 60 FPS outside the close-up | [R4 #12988](https://github.com/OpenAgentsInc/openagents/issues/12988), `e1af503e1d`, qualifies full-raster wide and medium shots at 1080p and 720p on Apple M5 Max, hardware Metal WebGPU. The four final-source windows each draw 300 frames at 60.0 FPS with p95/max 16.670 ms and zero held frames. |

## Retained measurements

The R4 plan section contains the three-run AC table and the separate
final-source battery measurements. Copies of the four final-source timing logs
are retained here; their values are historical R4 observations, not a fresh
performance claim for this audit. The unchanged rule requires p95 ≤17.5 ms and
no frame over 33 ms. Close-ups are recorded separately and are not the parent
issue's 60 FPS exit gate.

| Shot | Actual raster | Frames | FPS | p95 / max, ms | Held |
| --- | --- | ---: | ---: | --- | ---: |
| Wide raid | 1920×1080 | 300 | 60.0 | 16.670 / 16.670 | 0 |
| Medium orbit end | 1920×1080 | 300 | 60.0 | 16.670 / 16.670 | 0 |
| Wide raid | 1280×720 | 300 | 60.0 | 16.670 / 16.670 | 0 |
| Medium orbit end | 1280×720 | 300 | 60.0 | 16.670 / 16.670 | 0 |

The close-up's retained 1080p result is 59.2 FPS, p95 16.670 ms, max 33.330 ms,
one held frame. It is preserved here with the 720p close-up rather than reported
as a passing wide/medium result. The retained `raid.mp4` is also 1920×1080 at
24/1 fps, 246 frames and 10.25 s. Both videos' metadata were read with `ffprobe`
during this audit. The original capture's 24 fps clock and the real-time
performance timer's 60 FPS measurements are separate observations.

## Source and scope

`source-audit.json` records source and built-artifact hashes. In this initial audit the character
runtime, character shader, orbit and capture tool remained byte-identical to R4.
The world's host character draw/depth/shadow section also remains unchanged.
Surrounding world/arrival/network sources changed after R4; the fresh audit
adds the raid spawn policy, regression checks and stricter performance proof.
No character renderer or quality/frame-admission threshold is changed here.

The H-series production/mobile/native-host work and exact-render follow-up
#12962 retain their separate scopes. No production deployment is implied.

Historical private videos, source packs and rendered frames remain under
`~/.openagents/verse/private-build/echo-r4-finish/`. Fresh capture output stays
under `~/.openagents/verse/private-build/echo-12984-2026-11-07/`. They are not committed.
The public record contains only code identities, counts, timings and video
metadata. Log copies normalize trailing whitespace only. Reproduction commands and the original implementation checks are in
[the R1–R4 plan sections](../../verse/2026-11-06-native-hero-characters.md#first-echo-in-the-raid-scene-for-recording).
