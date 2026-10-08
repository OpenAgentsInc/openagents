# Corrected W11 checkpoint, 2026-10-08

Source `499b25d0839a01dbbbc524e1374b1ee5c3e52644` fixes the missing scene
bind group in a fresh blended pass and retains browser device/Log errors.
The WebGPU Water Lab dry/wet pair renders correctly with 726 valid wet GPU
samples. The three native reruns retain two old-budget failures and
zero/one valid GPU samples. Low waterline exits `0` without a GPU sample;
that exit does not establish a GPU-budget pass. Budget calibration and
acceptance remain incomplete; no constants, decoder, or production deploy
changed during this collection.

[receipt.json](receipt.json) records exact source, artifacts, input digests,
compiler and lease exits, commands, and raw-data hashes. Original Cargo
JSON and raw measurements are compressed byte-for-byte; decompression
reproduces each recorded original SHA-256. Native and browser summaries
include sample counts and scope limits. Captures and private kit bytes
remain outside Git at the paths recorded in the reports.

Native CPU values use thread clocks. Browser CPU values are elapsed only,
and waves run inline. Isolated Low timestamp probes split the normal fused
pass, so their cost is diagnostic. Fence estimates are separate from GPU
timestamps. Native timing validity still requires raw slot intervals,
submission errors, mapping status, and decoder reasons; no absent or
rejected interval is a measured zero cost. The test-compiled WASM derivative
is never a production artifact.

Resume through the [water handoff](../../../../../docs/verse/water-handoff.md#w11-checkpoint).
The coordinator stopped new jobs at the usage checkpoint. #10783 remains
open, its claim is released, and the project status is Todo.
