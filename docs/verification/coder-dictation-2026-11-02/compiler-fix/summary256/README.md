# Historical path16 / summary256 compiler slice

This preserves a completed intermediate profile: 16 projection steps and 256 distinct canonical retained effects per procedure. BS origin sets and the supplied-store RP kernel remain 64; the compiler work budget remains1,048,576. Later path24 qualification is separate.

Four focused summary groups passed. Native probes passed 11 cases each at O0, O2 and under ASan/UBSan, including complete 256-effect application and 257 refusal. Four authored sensitivity controls were killed. Canonical checker emission passed its source checks, and its emitted C/header are byte-equal to the candidate's self-emitted outputs. These results do not claim a full mutation run.

The full compiler completed at 1,039,516 work steps, leaving 9,060 (0.864%) of the unchanged budget. Its phase log includes a successful exit; emitted/header artifacts are retained.

The desktop did **not** pass this profile: `actual256.stderr` records E0381 at Desk523. That new failure is unresolved here and was investigated separately. The earlier 128-effect failure is preserved in `effects128.json.gz` and `actual128.stderr`:128 exact relations, 1,147 global effects and 394,525 work steps. Neither diagnostic is an application success.

The frozen 15-module Coax closure, generated/probe C and headers, actual logs, and result metadata are pinned in `manifest.json`. Gzip uses mtime zero and records both encoded/decompressed hashes. `record.json` states the proof scope and native stack increase. Existing checks were not repeated to package this record. No binaries, microphone or end-to-end dictation claim are included.
