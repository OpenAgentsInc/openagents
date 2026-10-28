# Public fixed-versus-random timing harness

Task `crypto-ct-timing-harness` (#0bd1). This is a first-order Welch t-test
harness inspired by [dudect](https://github.com/oreparaz/dudect), with its own
bounded Coax statistics policy. It is not a complete implementation of that
tool or an assertion of cryptographic constant time.

`model.coax` is `#authority`: Welford moments, admitted observations, sample
and tick bounds, sample variances, Welch arithmetic and outcome classification
all live there. C owns acquisition, buffers, callback dispatch, platform clocks
and files. Python independently recomputes statistics as explicit test tooling;
it adds no normal compiler, app, bootstrap or production dependency.

## Declared method

- Exactly 1,000,000 observations per fixture, with 500,000 per class. The
  model accepts at most 2,000,000 total and elapsed ticks in 0–1,000,000,000 ns.
  Its borrowed state view must refer to one valid writable `CtStats` record.
- Class 0 has fixed input zero; class 1 has seeded random public input. Each
  adjacent pair contains one of each class in a seeded random order. Input,
  class and table preparation happens before warmup and measurement.
- Three public fixtures: fixed work, a deliberately input-dependent branch
  with 256 extra rounds, and 64 dependent table loads with input-dependent
  indices into an 8 MiB table. These are positive/negative controls, not
  production algorithms. No keys or private account material are loaded.
- 20,000 calls warm the code/data before collection. Native collection calls
  through a volatile callback pointer once per sample and preserves an
  observable output. Browser collection uses eight such calls per observation
  to expose work above its quantized timer. Acquisition does not run the
  classifier between samples.
- Native elapsed time uses `CLOCK_MONOTONIC_RAW`, with observed resolution and
  minimum empty clock-pair duration reported. Browser uses isolated
  `performance.now()` rounded to ns. No baseline subtraction, outlier removal,
  cropping or favorable subset selection occurs.
- Welch `t = (mean0-mean1) / sqrt(var0/n0 + var1/n1)`, using sample variances.
  The predeclared detection threshold is **abs(t) >= 10**. Nonfinite or invalid
  observations create a sticky refusal. Short runs are `insufficient`. Zero
  variance is explicit: equal means yield `no-variation`, unequal means yield
  `detected` with a degenerate flag rather than NaN/infinity.

The output label `no-difference-detected` means only that this experiment did
not cross the threshold. It does not prove constant time. The harness does not
test all input distributions, second-order leakage, every instruction/JIT
tier, microarchitecture, compiler flag or power/cache observer. Repeated
searches, correlated noise, frequency scaling and background work can affect
results. A fixed-work alarm would be retained and investigated, not erased.

## Run and retain

```sh
coax/bin/coax test tools/coax-nostr/timing/fixtures.coax
coax/bin/coax mutate tools/coax-nostr/timing/model.coax
python3 tools/coax-nostr/timing/run.py --ssh-target USER@TRUSTED_LINUX
bash tools/coax-nostr/timing/browser.sh
python3 tools/coax-nostr/timing/test_timing.py
```

The native runner currently observes a native macOS arm64 host and optionally
an existing Linux x86-64 host. SSH uses batch mode, strict existing host keys
and a unique temporary directory; it never enrolls a machine, changes system
settings or touches a remote checkout. Only generated public test C/driver
files are transferred. It compiles with the existing C compiler, retrieves
observations/artifacts and removes its own temporary directory. Linux pins
only this process to the first allowed CPU and reports that CPU; macOS uses
normal scheduling. Other processes and governors remain unchanged.

Browser execution is headless Chrome with an isolated local HTTP server.
`--js-flags=--no-liftoff` requests optimizing-only Wasm; the report names the
actual Chrome/V8 versions and all flags. This is requested configuration,
not independent attestation of JIT tier. The default tool paths are explicit
in `browser.sh`; `COAX_WASM_CC`, `COAX_WASI_SYSROOT`, `COAX_WASI_RESOURCES` and
`CHROME` may select installed alternatives. Do not label Node/Bun execution
as a browser measurement.

Each retained private fixture file is `CT01`, little-endian u64 count, then
exactly count `(class:u8, elapsed_ns:u64)` records. This is a versioned local
test artifact, not a public OpenAgents protocol. Reader count, version, size,
class and tick checks refuse malformed data. The default ignored artifact
directory is `coax/bin/nostr-timing-2026-10-28/`; the native runner caps its
files at 128 MiB. Raw rows, binaries and Wasm are retained there, while hashes,
class counts, statistics, noise quantiles, compiler/flags and target metadata
are committed under `evidence/`. Hashes identify local artifacts; they do not
make those ignored files publicly downloadable. No hostname, SSH identity,
tailnet address or secret is included in the evidence.

An actual crypto adapter should preserve the same acquisition separation,
declare its public input distributions and memory representation, and pin
the exact source/object/compiler. These public controls are not timing
approval for later signing, ECDH, nonce generation or production key custody.

## Recorded run, 2026-10-28

Coax 0.16.0 release C, `-O2`, normal shared machines:

| Target | Fixed-work t | Planted branch t | Planted index t |
| --- | ---: | ---: | ---: |
| Apple M5 Max, Apple clang 21, arm64 | 0.1784 | -328.2741 | -771.7270 |
| Core i7-14700K, GCC 15.2, x86-64 | -0.1254 | -683.1609 | -1395.2560 |
| Chrome 156 / V8 15.6.75.19, Wasm, requested optimizing tier | 1.7954 | -327.2268 | -483.5863 |

Every cell represents a separate million-observation run with all raw rows
retained. The native clock reported 42 ns on this Mac and 1 ns on Linux;
Chrome's observed timer step was 5,000 ns. Both planted leaks were detected
on all three measured targets; none of these fixed-work runs crossed the
threshold. These are scoped observations, not universal timing guarantees.

Six Coax check groups kill **134/134 mutants** without exemptions; the loaded
fixture adds a seventh passing group. Five focused tooling/native tests cover
independent two-pass arithmetic, emitted checked/release NaN/infinity refusal,
raw file corruption, a planted classifier fault and actual browser/raw parity.
The actual million-sample index driver also passed ASan/UBSan. The latter's
instrumented timing is diagnostic only, not release timing evidence.
