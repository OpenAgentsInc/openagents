# Native audio acceptance

The native scene ran for 100 seconds on Apple M5 Max at the existing full-resolution, four-sample rendering settings. The device callback produced 5,046,272 stereo frames at 48 kHz, with 10,088,151 nonzero samples and a peak amplitude of 0.21346. It reported no device errors, refused commands, or dropped capture blocks. The scene queued 383 footsteps, 23 fire launches, 97 impacts, and one ritual ambience loop. This fixture did not exercise the shield cue.

`window/audio.wav.gz` contains the float32 stereo mixer output supplied to the device callback, before device sample conversion. It is not acoustic loopback. Decompress it to play the WAV file. The capture contains original procedural audio; it includes no imported recordings.

The frame budget failed: work p95 was 21.068584 ms and delivered p95 was 21.556583 ms. The profile reports 23 stress casts and no dropped simulation time. The audio issue remains open pending performance investigation and shield cue evidence. `window/frames.ndjson.gz`, `window/budget.json`, and the native screenshots retain this failed run.

The follow-up `no-capture` run kept device audio enabled and disabled WAV recording. It also failed: work p95 29.763042 ms, delivered p95 30.471125 ms, 23 casts, and zero dropped time. A retained process snapshot taken during the run shows another session’s Rust compiler using over 1,300% CPU. These results do not establish an audio regression or isolate recording cost; a contention-free run is still required.

The programmatic `--audio-window-proof OUTPUT 100` run covers all five cues: 430 footsteps, 21 fire launches, 98 impacts, seven shields, and one ambience loop. Device output remained error-free with no refused commands or dropped capture blocks. Audio routing measured 0.005166 ms p95. `all-cues/audio-proof.json` records counters, full-resolution 3456 × 2104 rendering with four samples, and source digests; `all-cues/preview.wav` contains the first 20 seconds of callback output. The full WAV and profile are retained as gzip files.

This run also fails the established budget: work p95 16.697917 ms, delivered p95 17.055542 ms, delivered maximum 113.479 ms, and 0.013479 seconds of dropped simulation time. The renderer reload preserves the world checkpoint and presents the new catalog. The screenshot was inspected. No claim of passing performance acceptance is made, and #10491 remains open.

## Passing acceptance with grounded shadow reuse

Commit `01567c75a9` caches grounded casters only after two identical full-pose and identity samples. Pose, model, life, eligibility, visible-caster membership, and light-matrix changes invalidate depth before drawing. The renderer still draws dynamic casters and preserves shadow geometry and resolution. The `cached-shadow-pixels/shadow-cache.json` proof records identical cold/cached pixels, one admitted corpse caster, and zero unchanged face refreshes. The residency proof also preserves pixels after rebuilding and rejects stale catalog handles.

The final `cached-shadows` native run passes the established budget: 100 seconds, 21 completed fireballs, work p95 15.443959 ms / maximum 26.404417 ms, delivered p95 15.7725 ms / maximum 26.735 ms, and zero dropped simulation time. Rendering remains 3456 × 2104 with four samples. Up to 11 grounded casters reuse shadow depth. Audio routing is 0.005125 ms p95. These are CPU frame-work and delivered-interval measurements, not GPU timestamps or a guarantee for every machine and workload.

The device callback produces 5,046,272 stereo frames at 48 kHz, including 10,088,232 nonzero samples, with peak amplitude 0.31770. The scene queues 428 footsteps, 21 fire launches, 94 impacts, seven shields, and one ambience loop. Device errors, refused commands, and dropped capture blocks are all zero. The full callback PCM, compressed frame profile, source digests, native screenshots, budget, and unchanged-world reload receipt are retained under `cached-shadows`. The final screenshot was inspected. Earlier failed runs remain as evidence; #10491's native audio and performance acceptance is now complete.

Verification: 26 imported-scene tests, three final cache-invalidation tests, the focused native example test, formatting, pixel/residency proof, and the full native frame-budget check pass.
