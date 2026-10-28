# secp256k1 differential oracle harness

Task `secp-libsecp-harness` (#2265). This is optional test tooling: Python
standard-library integer/affine references plus a pinned libsecp256k1 public-API
adapter. Nothing here is linked into the app, browser, compiler or production
services. The references are variable-time and handle **public throwaway test
keys only**. They are not a proposed production crypto implementation.

No Coax field/scalar/Schnorr/ECDH modules existed when this slice was written.
The retained smoke therefore checks the oracles and comparison plumbing. It
cannot establish Coax algorithm conformance, a one-hour fuzz milestone, constant
time, secure erasure or independent security approval.

## Source pin and inspected build boundary

`UPSTREAM.json` pins official libsecp256k1 release **v0.8.0**, its annotated tag
object, peeled source commit, codeload archive size/SHA-256, MIT license URL/hash
and inspected input hashes. The release/tag identity was read from the official
repository on 2026-10-28; this is not a claim of independently verifying the tag's
OpenPGP signature. Source is cached under ignored `.cache/`, never vendored into
this tree or shipped with a product.

Inspected before this build: `COPYING`, CMake options that select modules and
precomputed-table geometry, `src/secp256k1.c` includes/public entry points,
`src/util.h` arithmetic/configuration/erasure support, the ECDH callback path and
`include/secp256k1_schnorrsig.h` general-message API. This bounds our build/API
review; it is not an independent audit of every line of libsecp256k1.

The builder verifies the full archive first, rejects unsafe/duplicate/symlink
archive members, checks inspected file hashes, and compares every cached source
byte/inventory on reuse. It then invokes the host C compiler directly on exactly:

- `src/secp256k1.c`
- `src/precomputed_ecmult.c`
- `src/precomputed_ecmult_gen.c`
- our `oracle.c`

Only ECDH, extrakeys and Schnorr optional modules are enabled. Table geometry is
explicit (`ECMULT_WINDOW_SIZE=15`, `COMB_BLOCKS=11`, `COMB_TEETH=6`). No upstream
configure, CMake, installer, shell generator or package script is executed.
`.cache/build/build.json` records source-tree/adapter/unit/artifact identities,
compiler, flags and host. Loading a library checks its actual bytes against that
report and the current source pin/adapter before `ctypes` loads any native code.

After inspecting the exact pinned source/build inputs, explicitly build:

```sh
python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed
# Or reuse the same archive after review; no network:
python3 tools/coax-nostr/fuzz/build_oracle.py --archive /path/to/pinned.tar.gz --reviewed
```

`--reviewed` is an explicit operator assertion, not a substitute for the review.
There is no implicit download/build in tests, app generation or `run.sh`. A
changed source/archive/adapter fails closed until reviewed and rebuilt. The
optional `--sanitize` builds a separately instrumented library; the test host
must load the corresponding sanitizer runtime. No sanitizer run is claimed by
the retained normal-build smoke.

## Run the focused checks

```sh
python3 tools/coax-nostr/fuzz/test_harness.py
tools/coax-nostr/fuzz/run.sh --seed 20261028 --count 32 --report /tmp/secp-smoke.json
```

Tests cover all 19 official BIP-340 rows, including general-message lengths;
compare Python verification and libsecp verification, and reproduce every signing
row through `sign_custom`. Point multiplication and raw-x ECDH compare libsecp
against an independent Python affine double-and-add implementation. The integer
oracle uses arbitrary precision modulo p/n; separate bounded carry/borrow,
shift/add multiply and Fermat inverse paths smoke arithmetic identities. These
checks are explicitly not candidate conformance.

The tests execute a JSON-line fixture to prove the candidate transport works,
then inject a lost high-product carry and libsecp's wrong default ECDH hash.
Both must produce a recorded mismatch. Timeout, output-budget and changed-native-
artifact cases fail as well. The fixture shares the oracle deliberately; it must
never be presented as an independent Coax implementation.

## Candidate adapter contract

A candidate is an explicitly supplied local executable/command. It reads **one**
JSON request from stdin, writes one JSON result to stdout, and exits. No keys are
passed in argv. Each invocation has a wall timeout and 1 MiB regular-file output
limit; core dumps are disabled. This POSIX harness supports macOS/Linux. A
candidate that needs a different wire/limb representation supplies a thin test
adapter; it must not change expected mathematical results.

Unsigned integers are big-endian, 64-character lowercase hex. Byte arrays are
lowercase even-length hex. Generated messages are bounded by the corpus; native
sign/verify adapters cap messages at 1 MiB. Comparisons are exact, including
refusal/Boolean shape; diagnostic text is not an accepted substitute.

| Module | Request | Expected result |
| --- | --- | --- |
| `field` | `op`, `a`, optional `b` | `{"ok":true,"value":"<64 hex>"}` modulo p |
| `scalar` | Same | Same modulo n |
| `group` | `op:"mul"`, `scalar`, optional compressed `point` (default G) | Compressed 33-byte point; `"00"` denotes infinity; malformed point refuses |
| `ecdh` | `secret`, x-only `peer` | Raw 32-byte shared x; invalid scalar/peer/infinity refuses |
| `schnorr` | `op:"verify"`, `message`, `public`, `signature` | `{"ok":true,"valid":true/false}` |
| `schnorr` | `op:"sign"`, `message`, `secret`, `aux` | Exact deterministic BIP-340 signature for supplied auxiliary bytes; invalid key refuses |

Arithmetic operations are `normalize`, `add`, `sub`, `mul`, `negate`, `inverse`.
Inputs are unsigned 256-bit integers; normalization is explicit modulo p/n.
Inverse of zero refuses with `{"ok":false}`. This mathematical ABI does not
pretend to expose libsecp field/scalar internals. Representation/magnitude
preconditions need separate implementation checks in the Coax modules.

The public libsecp point adapter cross-checks finite multiplication outputs;
Python additionally defines infinity/curve arithmetic. The ECDH C adapter parses
an even-y compressed peer from its x coordinate and supplies a callback that
copies **x32**. Libsecp's default SHA-256(compressed shared point) is intentionally
exposed only to the negative test; it is the wrong NIP-44 oracle.

## Retained regressions and longer runs

```sh
tools/coax-nostr/fuzz/run.sh --seed 20261028 --count 32 \
  --report /tmp/field-run.json --regressions /tmp/field-regressions.jsonl \
  --candidate-artifact /path/to/generated.c \
  --candidate /path/to/candidate-adapter

# Explicit milestone run AFTER an actual independent Coax candidate exists:
tools/coax-nostr/fuzz/run.sh --seed 20261028 --count 64 --seconds 3600 \
  --report /path/to/retained/report.json --regressions /path/to/retained/regressions.jsonl \
  --candidate /path/to/candidate-adapter
```

The report pins seed, comparison count, actual elapsed time, source/build/native
artifact identity, harness hashes, corpus hash and candidate executable/extra
artifact bytes. Candidate arguments that name source/scripts are hashed too.
Each mismatch retains the complete small request and expected/actual result;
retain it as a regression and minimize further when a real algorithm exposes a
more complex failure. These records never contain production inputs.

Runs stop at the first mismatch. A timed run without a real candidate is refused.
One process per request favors simple bounds/isolation over throughput; record
actual comparisons/time rather than assuming a one-hour run proves coverage.
The committed `evidence/` records the bounded oracle smoke and seeded faults only.
Future field/scalar/point modules must run through this adapter before advertising
conformance. Independent arithmetic/protocol review remains a separate gate.
