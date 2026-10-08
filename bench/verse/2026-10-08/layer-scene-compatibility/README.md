# Cross-platform layer compatibility

The release Mac, Linux, and browser scenes have identical ordered indices,
batches, materials, UVs, vertex colors, and licensed image bytes. Maximum
position differences are 0.031 mm; maximum normal differences are 0.00000191.
Only the procedural dirt image differs, by one channel value in nine pixels
at most. `receipt.json` records the exact scene identities and comparisons.

The compatibility record accepts only these exact digests and the complete
SHA-256-pinned layer artifact. Unknown scenes, changed recipes, changed layer
bytes, and the different debug-build topology remain rejected. No artifact
was rebaked or rewritten. Raw geometry and image evidence stays in private
scratch; the browser proof uses test-only exports and is not a deployable build.
