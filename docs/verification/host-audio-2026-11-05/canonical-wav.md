The canonical WAV source checkpoint adds `aethyr/audio/host-wav.coax`, SHA-256
`f31d8a5263ed0d81bf7ea8568a55baf7d9a120fa90b8a9eaea3e33b49d42c050`.
The niced root gate was `COAX_JOBS=2 COAX_CHECK_JOBS=2 nice -n 10
coax/bin/coax mutate aethyr/audio/host-wav.coax --library --root-only --jobs 2`.
Its retained `wav-mutation.log` reports 273/273 killed, no survivors or
equivalents. The compiler image is the unchanged admitted 598ddc2358 tool
identified in the foundation record. Passing unchanged audio gates were reused.

This source checkpoint makes the same decoder available to native and Wasm
adapters. It does not claim that an SDK's previous resampler had identical
samples, that adapters already use this decoder, or that either complete host
has passed. The actual old SDK mismatch and prior common DSP worklet results
remain retained independently. The ABI and numeric profile are documented in
[host-wav.md](../../../aethyr/audio/host-wav.md).
