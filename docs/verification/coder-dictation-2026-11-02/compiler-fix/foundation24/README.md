# Path24 foundation qualification

The supplied-fact borrow kernel now represents 24 projection steps. Its only production changes from path16 are the path bound and inline step-array width. Set64, bindings64, pool256 and candidate-work128 remain unchanged. This independent kernel proof does not qualify compiler lifetime analysis or the desktop.

All 29 Coax checks passed. The independent literal C oracle passed 230 API cases, including 75 refusals, at O0, O2 and under fatal ASan/UBSan. The oracle checks LP64 Path392/Origin424/Binding464 and binding field offsets. It covers 24/25, every high identity slot 8–23, inactive tails, full-depth inline rebasing, shorter formal mismatch and the existing set/work boundaries. Sanitizer stderr was empty; leak detection was disabled.

Source, generated C/header and actual logs are pinned in `manifest.json`. Gzip is deterministic with mtime zero and both encoded/decompressed SHA identities. The producer hash, flags, results and scope are in `record.json`. No full mutation run, binaries or application/microphone claim is included. The adjacent historical foundation16 proof remains unchanged.
