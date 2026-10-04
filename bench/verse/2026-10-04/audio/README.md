# Native audio acceptance

The native scene ran for 100 seconds on Apple M5 Max at the existing full-resolution, four-sample rendering settings. The device callback produced 5,046,272 stereo frames at 48 kHz, with 10,088,151 nonzero samples and a peak amplitude of 0.21346. It reported no device errors, refused commands, or dropped capture blocks. The scene queued 383 footsteps, 23 fire launches, 97 impacts, and one ritual ambience loop. This fixture did not exercise the shield cue.

`window/audio.wav.gz` contains the float32 stereo mixer output supplied to the device callback, before device sample conversion. It is not acoustic loopback. Decompress it to play the WAV file. The capture contains original procedural audio; it includes no imported recordings.

The frame budget failed: work p95 was 21.068584 ms and delivered p95 was 21.556583 ms. The profile reports 23 stress casts and no dropped simulation time. The audio issue remains open pending performance investigation and shield cue evidence. `window/frames.ndjson.gz`, `window/budget.json`, and the native screenshots retain this failed run.

The follow-up `no-capture` run kept device audio enabled and disabled WAV recording. It also failed: work p95 29.763042 ms, delivered p95 30.471125 ms, 23 casts, and zero dropped time. A retained process snapshot taken during the run shows another session’s Rust compiler using over 1,300% CPU. These results do not establish an audio regression or isolate recording cost; a contention-free run is still required.

The programmatic `--audio-window-proof OUTPUT 100` run covers all five cues: 430 footsteps, 21 fire launches, 98 impacts, seven shields, and one ambience loop. Device output remained error-free with no refused commands or dropped capture blocks. Audio routing measured 0.005166 ms p95. `all-cues/audio-proof.json` records counters, full-resolution 3456 × 2104 rendering with four samples, and source digests; `all-cues/preview.wav` contains the first 20 seconds of callback output. The full WAV and profile are retained as gzip files.

This run also fails the established budget: work p95 16.697917 ms, delivered p95 17.055542 ms, delivered maximum 113.479 ms, and 0.013479 seconds of dropped simulation time. The renderer reload preserves the world checkpoint and presents the new catalog. The screenshot was inspected. No claim of passing performance acceptance is made, and #10491 remains open.
