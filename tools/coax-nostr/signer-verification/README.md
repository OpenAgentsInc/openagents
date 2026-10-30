# Independent signer measurements

This tooling measures the Coax signer without editing the signer, kernels,
analyzer or existing secret probes. It does not approve production crypto.
`compare.py` compiles the same generated release C at `-O1 -fno-inline` and
`-O2`, checks public BIP-340 fixtures, and records Memcheck XML contexts.
The Pylon's native release build uses `-O2`.

On Linux with GCC, Valgrind and its development headers:

```shell
python3 tools/coax-nostr/signer-verification/test_compare.py
python3 tools/coax-nostr/signer-verification/compare.py \
  --compiler coax/bin/coax \
  --valgrind /path/to/valgrind/bin/valgrind \
  --valgrind-include /path/to/valgrind-dev/include \
  --source-revision COMMIT \
  --out /tmp/new-signer-measurement
```

Use `--profile release-optimization` to measure only `-O2`. Output directories
must be new. Every subprocess has a ten-minute timeout. Nix users can obtain
the two Valgrind store paths with `nix build --no-link --print-out-paths
nixpkgs#valgrind.out` and `nixpkgs#valgrind.dev` respectively.

The eight official signing rows cover 0, 1, 17, 32 and 100-byte messages.
Negative cases cover zero/order/maximum keys, short key views, overlap,
changing the message after nonce preparation, and changing the unpublished
candidate. They check exact signatures, expected stages, actual workspace
clearing and untouched refused output. A wrong expected signature must fail
the native harness. A deliberate branch on the undefined key must register.

Only key and auxiliary randomness are undefined during signing. The harness
makes each returned status observable. It observes signature bytes after
signing, and defines the final workspace only for an actual zero-byte check.
It injects no intermediate declassification of P, R, parity or validity bits.
Its counts therefore include intentional observations pending source review.

`comparison.json` records all contexts, source/compiler/tool/artifact hashes,
flags, fixture results and per-process exit codes. Full XML and stdout/stderr
remain alongside it. Truncated reports, missing positive controls, unexpected
memory errors, changed inputs and fixture failures stop the measurement.
A successful **measurement** may contain secret-flow reports; neither zero
reports nor its exit status gives production approval. XML and text reports
can group contexts differently; compare counts only with the recorded format
and build settings in view. No timing test, arm64 trace or Wasm JIT claim is
made here. The source revision is a caller label; compiled source hashes are
the recorded identities.

## Negative-test the existing shell gate

`seed_gate.py` runs private copies of a supplied gate/probe with baseline
generated C and with one extra raw-key branch inside `sm_equation`. A small
compiler-output shim feeds those two C files and their header to the unchanged
gate. Only debug line directives are removed so the shell parser sees the
`release.c` locations it expects. No repository implementation is modified.

```shell
python3 tools/coax-nostr/signer-verification/seed_gate.py \
  --generated-c /tmp/new-signer-measurement/release.c \
  --header /tmp/new-signer-measurement/es.h \
  --gate-script coax/tests/nostr/schnorr_sign_secret_probe.sh \
  --gate-probe coax/tests/nostr/schnorr_sign_secret_probe.c \
  --out /tmp/new-signer-gate-negative
```

Exit zero requires the clean baseline to pass and the seeded branch to fail.
The pinned gate at `76e08098ca` fails this negative check: it accepts both.
Its whole-function exemptions are the finding; the crypto source stays with
its current owners. See [the coordination document](../../../docs/issue4828.md).
