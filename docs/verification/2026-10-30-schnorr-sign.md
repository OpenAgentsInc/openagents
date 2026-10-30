# BIP-340 signer: `coax/lib/crypto/schnorr_sign.coax` (2026-10-30)

Board: #51f4 `schnorr-sign`. Psionic chapter 19 prerequisite 1.

## What it is

BIP-340 signing in four resumable stages over one caller-owned `SignWork`:
key point, nonce point, scalar equation, then a self-check of the finished
signature. `schnorr_sign_step(key, aux, message, work, out, overlap)` does one
stage per call and returns `SIGN_MORE` until `SIGN_DONE` or `SIGN_REFUSED`.
`nostr_sign_step` is the same over a 32-byte event id. Nothing is written to
`out` unless the signature verifies, and the workspace is cleared whenever
signing ends, refused or done.

The self-check (`ss_self_check`) is one function, called only from
`sw_publish`. For now it uses the variable-time fast verifier. #89db
(`schnorr-self-verification-boundary`) replaces it with a fixed-work verifier
that returns a validity mask.

## Evidence

- **Checks:** 216 of 216 pass (`coax test coax/lib/crypto/schnorr_sign.coax
  --run-steps 250000000`, 34 s). Vectors 0–3 are checked stage by stage
  against independent pure-Python intermediates
  (`coax/tests/nostr/schnorr_sign_vectors.py`). The checks also cover a zero
  key, the group order as a key, an even key, an even nonce point, and refusal
  clearing the workspace.
- **Mutation:** 3,384 mutants, 3,384 killed, 0 survived, none marked
  equivalent. The run was 40 shards (`--run-steps 250000000 --shard I/40`),
  merged with `--merge`.
- **Native:** the end-to-end harness `coax/tests/nostr/schnorr_sign.py` lands
  with `event_sign.coax`, because it drives both. It runs checked and release
  builds, plain and ASan/UBSan, 44 requests each, and all 4 pass:
  - all 8 official signing rows byte for byte, including rows 15–18 (0, 1,
    17 and 100-byte messages);
  - 24 random signatures (messages of 0, 1, 32, 33, 64 and 200 bytes) checked
    by an independent Python verifier;
  - 4 signed events and 8 refusals.

## Build used

The checks and mutation ran with the compiler built from `7baa083497`. The
files the signer loads (`coax/lib/crypto/`) are byte-identical at
`origin/main` when this landed. The only compiler changes since then are in
compute and Metal emission.

## Not done

- **Wasm agreement:** not run.
- **iOS:** not run.
- **The fixed-work self-check:** #89db.
- **The secret-flow audit of the signer's closure:** #4828.
- **Independent review.**

Until those are done, the signer is for test keys only. Moving real keys to
it needs the owner's approval and the review gate in `coax/nostr.md`.
