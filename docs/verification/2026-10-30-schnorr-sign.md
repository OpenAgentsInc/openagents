# BIP-340 signer: `coax/lib/crypto/schnorr_sign.coax` (2026-10-30)

Board: #51f4 `schnorr-sign`. Psionic chapter 24 prerequisite 1.

## What it is

BIP-340 signing in six resumable stages over one caller-owned `SignWork`:
key point, nonce point and scalar equation, then three stages of the
fixed-work self-check. `schnorr_sign_step(key, aux, message, work, out, overlap)` does one
stage per call and returns `SIGN_MORE` until `SIGN_DONE` or `SIGN_REFUSED`.
`nostr_sign_step` is the same over a 32-byte event id. Nothing is written to
`out` unless the signature verifies, and the workspace is cleared whenever
signing ends, refused or done.

The self-check is `schnorr_self_verify.coax`'s `ssv_step` (#89db), one of
its stages per signer call:
- stage 3 prepares it (`sw_check`);
- stage 4 multiplies the key point (`sw_check`);
- stage 5 finishes (`sw_publish`).

The unpublished candidate never reaches the variable-time verifier. Only
ssv's final mask, with the three validity bits, decides publication. (Until
2026-10-30 this was one call to `schnorr_fast_verify_vartime` at
`0cc86015e7`.)

Stage 1 records a SHA-256 digest of the message its nonce came from. Stage
2, the scalar equation, refuses and clears unless the message has the same
digest. Later stages need no extra check: the self-check verifies against
each call's message, so a message changed after stage 2 fails it. One nonce over two messages would reveal the key, so this closes the
medium finding of the 2026-10-30 independent review
(`2026-10-29-schnorr-self-verify.md`). The final stage always runs the
self-check's last step before it reads the validity bits, so a refusal does
the same work as a publication.

Staging keeps every check under `coax mutate`'s 20M-step cap. One whole
signature through the fixed-work check is about 30.4M interpreter steps, so
the checks for stages 5 and 6 start from recorded states, as
`schnorr_self_verify.coax`'s own checks do. Stage 5 uses G and -e = 2 and
must produce 2G, at 19.0M steps. Stage 6 uses vector 0's recorded (-e)·P
(`ss_final`), at 11.7M. Whole signatures run in the native and Wasm harness
below.

## Evidence

- **Checks:** 256 of 256 pass, including the loaded suites. Earlier count, at `0cc86015e7`: 216 of 216 pass (`coax test coax/lib/crypto/schnorr_sign.coax
  --run-steps 250000000`, 34 s). Vectors 0–3 are checked stage by stage
  against independent pure-Python intermediates
  (`coax/tests/nostr/schnorr_sign_vectors.py`). The checks also cover a zero
  key, the group order as a key, an even key, an even nonce point, and refusal
  clearing the workspace.
- **Mutation:** 3,754 mutants, 3,754 killed, 0 survived, none marked
  equivalent, with the staged self-check and the message binding, on main at
  `84f1cb2222`. The staged self-check before the binding fix killed 3,742
  of 3,742 at `944651321f`. It ran as
  80 shards on the Mac (`--run-steps 250000000 --jobs 1`), merged. The
  earlier single-call version killed 3,384 of 3,384.
- **Native and Wasm:** `coax/tests/nostr/schnorr_sign.py` drives the signer
  and the event builder through the generated C. Checked and release, plain
  and ASan/UBSan, 44 requests each, all pass. The release build also runs as
  Wasm (wasm32-wasip1 under Node's WASI). The checked and release builds
  both run in the iOS Simulator (arm64, iOS 26.5, `COAX_IOS_SIMULATOR=<device>`
  on the Mac). All of them agree on the same 44 requests.
  The requests are:
  - all 8 official signing rows byte for byte, including rows 15–18 (0, 1,
    17 and 100-byte messages);
  - 24 random signatures (messages of 0, 1, 32, 33, 64 and 200 bytes)
    checked by an independent Python verifier;
  - 4 signed events and 8 refusals.

## Build used

The checks and mutation ran with the compiler built from `7baa083497`. The
files the signer loads (`coax/lib/crypto/`) are byte-identical at
`origin/main` when this landed. The only compiler changes since then are in
compute and Metal emission.

## Not done

- **The secret-flow audit of the signer's closure:** #4828.
- **Independent review.**

Until those are done, the signer is for test keys only. Moving real keys to
it needs the owner's approval and the review gate in `coax/nostr.md`.
