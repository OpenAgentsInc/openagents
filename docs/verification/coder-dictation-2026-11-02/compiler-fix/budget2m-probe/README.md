# Private two-million-step diagnostic

This is a private diagnostic, not approval to raise the compiler budget and not publication of a new default. The archived current24 profile keeps its 1,048,576-step budget. Root and the user decide separately whether a different budget should be adopted.

The diagnostic keeps path24, exact summary256, BS origins64 and supplied-store RP64. Its shared work limit is 2,097,152. An independent read-only comparison found exactly 88 changed checker lines: the three production work-cap sites and paired work-boundary fixtures. Every numeric change adds 1,048,576, preserving fixture headroom and boundary offsets; all other bytes and the other 14 modules are identical. Unrelated arithmetic constants and the sum scanner's explicit one-million bound remain unchanged. The byte comparison is recorded in `independent-source-audit.json`.

The compiler agent reported successful C generation of the actual desktop settings source, exit zero at 22:50:33 UTC. Its generated C/header and empty stderr are preserved. This demonstrates source C generation under the private diagnostic limit; it does not demonstrate compiling or launching the UI, using a microphone, or inserting dictation.

The original diagnostic and correctly rooted canonical checker emissions are retained. The latter completed successfully with empty stderr. Removing only the literal snapshot prefix `/tmp/dictation-compiler-fix/probe2m-src/coax/` from the original C/header makes both byte-equal to the rooted outputs; this normalization was independently verified. That establishes the changed producer's emitted-output fixed point under the recorded path normalization. It does not establish a normal bootstrap or release.

`manifest.json` pins the 15-module diagnostic source closure, changed-line record, generated artifacts and this audit. Gzip uses mtime zero and records encoded/decompressed SHA identities. Producer executables are pinned by hash but are not included. No production files were edited and no tests were rerun to package this evidence.
