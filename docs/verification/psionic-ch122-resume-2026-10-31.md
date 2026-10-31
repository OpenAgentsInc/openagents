# Chapter 122: packing, checkpoints and exact resume, 2026-10-31

Task #039b. Profile `psionic-resume-v1`, Apple clang arm64, Coax 0.38.0
(575b776fca). CPU only; no GPU, downloads or model weights.

- `psionic/lib/data/pack.coax` (`#authority`): admitted documents (ascending
  IDs, exact spans, tokens 0-14, a training split), training documents only,
  boundary token 15 at every document start, a seeded per-epoch Fisher-Yates
  order (`splitmix64-top53-v1` at counters `epoch*64+i`), a four-field cursor,
  all-or-nothing rows, and a SHA-256 dataset identity.
- `psionic/lib/checkpoint/record.coax` (`#authority`): the `PSCK` v1
  little-endian record (identities, parent, revision, cursor, optimizer
  counters, binary64 values, SHA-256) and the 80-byte `PSCP` pointer. Reading
  refuses truncated, extended, wrong-magic, wrong-version, corrupt (digest),
  wrong model/tokenizer/dataset/optimizer, impossible integer fields and
  non-finite or negative-moment values, writing nothing.
- `psionic/lib/checkpoint/resume.coax` (`#authority`): a 16x16 bigram trained
  with AdamW on rows of 8; resume admits the pointer, the record, its
  currency and its cursor against the dataset.
- `psionic/platform/checkpoint-files.c`: temporary file, fsync, rename,
  directory fsync. File effects only.
- Coax debug and release: all checks pass (72 in the resume unit including
  the loaded units). Independent oracle (`psionic/tests/resume_oracle.py`):
  epoch orders, the 48-token stream, dataset identity, a fixed record and
  pointer built with `struct.pack` (matched byte for byte), model/tokenizer/
  optimizer identities, and the 12-step loss stream and final weights
  (within 1e-13; loss 2.772589 -> 1.408927).
- Runs interrupted after steps 1, 5 and 11 resume in fresh buffers from the
  bytes alone and end bit for bit at the uninterrupted state.
- Mutation (`--root-only`; the loaded SHA-256, counter RNG and AdamW units keep
  their own gates): pack **218/218**, record **398/398**, resume **103/103**
  killed, zero survivors, no equivalents.
- Native -O0/-O3, plain and ASan/UBSan, through real files: step 5 published
  with its pointer; step 8's record published without its pointer (a crash);
  the restart resumes at 5 and reproduces all 12 losses and 768 values bit for
  bit. Step 8's record under step 5's name is stale; a truncated and a
  corrupted file refuse; no temporary file remains.

Not claimed: scheduler state, compilation identity, precision policy beyond
AdamW's declared compute precision, multi-file or sharded checkpoints, or GPU.
