# Actual browser and native audio observations

The original browser graph and the first native shared-Coax PCM interpreter
were rendered against the same frozen AVFoundation-decoded 48 kHz PCM.
The browser invokes the real host.js offline clip graph, extracted into
`renderSoundLines` without changing its DSP. It renders silently through
OfflineAudioContext; no live speakers or GPU measurement are involved.
Coax authors each reset, command, exact frame time and retained render range.

The four simple-voice cases pass the inherited absolute 2^-20 sample gate,
with no time alignment or excluded samples. Max errors are zero (left),
2.083e-10 (right), 7.451e-9 (one-pole filter plus 960-frame delay) and
3.726e-9 (stereo pan). The two bed observations are also within that bound.
`pcm-compare.coax` kills all 98 root mutants, with no equivalents; it refuses
partial shapes and checks nonfinite raw bits before conversion.

The first mixed bus does **not** establish parity. Browser onset is frame
576 versus native frame zero; two Web Audio compressor nodes each retain
6 ms lookahead. Even the explicitly labelled 576-frame diagnostic alignment
leaves max errors of 0.01543 (rate), 0.01918 (room), 0.01411 (stop) and
1.50831 (loud). The raw loud native peak is 1.81611 versus browser 0.93499.
No alignment is applied to a sample gate. Complex cases have a null sample
verdict in `audio-iteration.json`; finite output is not audio parity.

SDK decoding is a distinct boundary. Chrome and AVFoundation each produce
24,960 samples from the same authored `fire-hit.wav`, but differ by up to
0.001902163; 15,178 samples exceed 2^-20. The shared-DSP comparison above
therefore reuses identical decoded input deliberately. Canonical bank decode
and resampling, a common Coax browser worklet/native DSP, actual callback
pause/resume and full three-host execution are still required.

Raw inputs, plans, decoded samples and outputs are retained under
`~/.openagents/aethyrforge/host-contract-12869/audio-native-first/` and
`audio-browser-first/`. These are frozen first-iteration observations. The
Metal owner has since corrected lookahead/ceiling behavior in its shared
audio foundation; its replacement observations will be recorded separately.

`check-browser-audio.coax LOCAL_URL PLAN.json AUDIO.json OUT_DIR` captures the
actual browser graph. `compare-audio-frames.coax BROWSER_DIR NATIVE_DIR
OUT.json DIAGNOSTIC_DELAY_FRAMES` analyzes saved samples without repeating
capture. Reports retain exact f64 metric bits alongside every raw f32 file.
