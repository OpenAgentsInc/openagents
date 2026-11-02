# libsecp256k1 cross-check of chapters 11, 13 and 14

The Nostr book postponed three libsecp256k1 checks
([chapters 11 to 104 record](2026-11-01-nostr-book-chapters-11-104.md),
"Checks postponed"). This record runs them on 2026-11-01 against the tree
at `ada9f3767f`. **Result: no mismatch.** Every Coax result agreed with
libsecp256k1.

## Setup

- **Oracle:** libsecp256k1 `v0.8.0`, commit
  `6e2c8bc4ecdc6e71dbe7a368f360d8d453ce435d`. The archive's SHA-256 is
  `3fe9fd70…2dfc33` (2,440,876 bytes), as
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json) pins it. The
  oracle was built as chapter 7 describes:
  `CC=gcc python3 -I tools/coax-nostr/fuzz/build_oracle.py --archive DIR/secp256k1.tar.gz --reviewed`.
  The result was `libsecp_oracle.so` with SHA-256 `fd5af505…8822a6`.
  `CC=gcc` is needed on this machine because the system `clang` has no libc
  headers.
- **Compiler:** the cached release build `bcefdc7fa553…` (coax0 0.46.0,
  binary SHA-256 `738c1672…801898`). `coax/build.sh` was stopped after 20
  minutes in the stage-0 mutation gate, under the 15-minute rule. Compiler
  commits after 10:18 aren't covered; the library sources are the tree's.
- **Host:** Linux 6.18.49 x86_64, gcc 15.2.0, Python 3.13.15. Wasm builds
  used clang 21.1.8 from `~/.cache/coax-wasm-toolchain` and Node v24.19.0.
- **Wasm on Linux:** `schnorr.py`, `schnorr_fast.py` and `ecdh_vectors.py`
  hardcode Homebrew wasm paths. They ran through a scratch wrapper that
  rewrites `/opt/homebrew/opt/llvm/bin/clang`, the wasi sysroot and the
  resource directory to `~/.cache/coax-wasm-toolchain/{bin/clang,sysroot,resource}`,
  and puts its `bin` (with `wasm-ld`) on `PATH`. Nothing else changes.
  Later the same day the harnesses stopped needing it: every Wasm harness
  under `coax/tests/nostr/` now takes its clang, sysroot, resource directory
  and `wasm-ld` from `wasm_toolchain.py`, which uses `coax/web/toolchain.sh`
  (Homebrew on macOS, `~/.cache/coax-wasm-toolchain` on Linux,
  `COAXIAL_*` overrides). With no wrapper, `schnorr_fast.py` (790 cases in
  each of four builds) and `ecdh_vectors.py` (783 cases in each of four
  builds) passed again with 0 failures, in 33 s and 60 s.

## Chapter 13: Coax-signed signatures through libsecp256k1

A new harness,
[schnorr_sign_libsecp.py](../../coax/tests/nostr/schnorr_sign_libsecp.py),
compiles `coax/lib/nostr/event_sign.coax`, which includes
`coax/lib/crypto/schnorr_sign.coax`. It links the C with
`schnorr_sign_driver.c` and sends it seeded random test keys, aux and
messages. Messages are 0, 1, 31, 32, 33, 64, 100, 255, 1024 or 4096 bytes.
Each Coax signature must pass two checks:

1. libsecp256k1's `secp256k1_schnorrsig_verify` accepts it.
2. It is byte-for-byte equal to `secp256k1_schnorrsig_sign_custom` on the
   same key, message and aux. BIP-340 signing is deterministic given aux,
   so the two signers must agree.

Each event's id is recomputed with an independent NIP-01 serializer. Its
signature then gets the same two checks. Before any comparison, the oracle
must reject a signature with one changed bit and a changed message.

| Run | Build | Seed | Signatures | Events | Failures | Time |
| --- | --- | --- | --- | --- | --- | --- |
| `--shard I/8` for I = 1 to 8 | release, `-O2` | 13 | 10,000 | 300 | 0 | 56 to 75 s per shard |
| `--mode checked --signatures 1000 --events 100 --seed 14` | checked | 14 | 1,000 | 100 | 0 | 57 s |
| `--sanitize --signatures 1000 --events 100 --seed 15` | release, ASan/UBSan | 15 | 1,000 | 100 | 0 | 87 s |

The generated C has SHA-256 `4d780aab…dabd622` (release) and
`c4c9d03d…c014e09` (checked). The source hashes are `schnorr_sign.coax`
`3e9702566ce8892c…` and `event_sign.coax` `bf02052fb13343a3…`. All three
builds together sent 12,500 signatures from the Coax signer to libsecp256k1:
12,000 BIP-340 signatures and 500 Nostr events. None failed verification,
and none differed from libsecp256k1's own signature.

```shell
for i in 1 2 3 4 5 6 7 8; do python3 coax/tests/nostr/schnorr_sign_libsecp.py --shard $i/8 & done; wait
```

## Chapter 11: verification against libsecp256k1

- **`schnorr_fast.py`:** 19 official rows, 790 cases in each build. Native
  and Wasm, checked and release: 0 failures. Each case's expected answer
  comes from libsecp256k1 and agrees with the Python reference. 51 s.
- **`schnorr.py`:** 8 of 8 tests passed after a one-line harness fix,
  described below.
  - 76 official comparisons and 2,052 seeded comparisons against
    libsecp256k1.
  - 44 metadata and alias cases.
  - 174 Wasm cases in each build, 0 failures.
  - The ASan/UBSan probe passed.
  - All four planted faults in the emitted C were detected.

**Harness fix:** the planted-fault test in `schnorr.py` looked for
`(l_event_id).count` in the emitted C. The parameter of
`sv_prepare_nostr_vartime` is now `nostr_id`, so the C contains
`(l_nostr_id).count`. Only the test's search string changed. No crypto
code changed.

## Chapter 14: ECDH against libsecp256k1

`ecdh_vectors.py` ran 783 cases in each of four builds: native and Wasm,
checked and release, with 0 failures. The cases are:

- 35 NIP-44 rows
- 473 valid and 13 invalid-x Wycheproof cases (266 excluded by the x-only
  adaptation)
- 256 seeded random cases
- 4 invalid scalars and 1 invalid peer
- 1 seeded default-hash fault

Every valid result equals libsecp256k1's raw-x `secp256k1_ecdh` and the
Python computation. 59 s. These counts match the
[2026-10-29 acceptance record](2026-10-29-ecdh-acceptance.md).

## Not covered

- Mac, iOS Simulator and Chrome runs.
- Compiler commits after the cached 10:18 build.
- Timing, secret flow and use with production keys. All keys here are
  seeded test keys.
