# Coax entropy request and completion contract

Task `crypto-entropy-contract` (#da6b), worker `codex-nostr-codecs`, 2026-10-28.

## Boundary

`coax/lib/crypto/entropy.coax` owns the pure, checked request and completion
policy. Entropy itself remains an OS/browser effect. Requests are explicit
counts from zero through 1 MiB, with caller-owned, disjoint output and scratch
prefixes and explicit capacities. Native callers remain responsible for real
allocated spans; checking nulls, address wrap and overlap does not prove object
ownership. Browser callers are checked against the actual Wasm memory size.

Admission refuses invalid counts, short/negative capacities, invalid physical
span facts and overlap before requesting bytes. Completion releases output only
after an available provider reports full success with exactly the requested
count. Missing, failed, short, negative and excessive completion counts report
zero written and preserve the entire output. Zero-byte requests need no RNG.

After an admitted provider attempt, Coax copies successful bytes and resets the
requested scratch prefix; output/scratch tails remain untouched. Admission
failures preserve both buffers. This reset is an ordinary observable memory
postcondition, **not** optimization-resistant secure erasure. Original output
lifetimes, secret destruction and the later secret-lifecycle gate remain
separate responsibilities. No production key, storage, permissions or UI was
added. This boundary makes no constant-time claim.

`coax/platform/entropy-native.c` performs only address observation and platform
effects: Apple `SecRandomCopyBytes`, or Linux `getrandom` with EINTR/partial-read
handling and flags zero. A missing/failed OS source has no PRNG, clock or other
fallback. The production API accepts no seed or injected provider. A provider
symbol is compiled only by explicitly defining `COAX_ENTROPY_TEST_PROVIDER` in
the test adapter build.

`coax/web/entropy.js` observes memory and calls WebCrypto using the actual
compiled Coax policy exported by `coax/platform/entropy-wasm.c`. Coax supplies
the full-chunk/tail counts for WebCrypto's 65,536-byte request limit. Throws,
missing sources and a wrong returned view fail without partial output. WebCrypto
defines successful calls to fill the whole passed view, so there is no native
short-count result; browser tests exercise partial writes followed by an error.
Deterministic replacement occurs only in an isolated test page, not through a
production adapter argument. The host import wiring remains the following
`host-random-import` task; this change does not edit `host.js` or an app screen.

## Verification

Checks preceded implementation; surviving zero-boundary mutants led to explicit
checks. There are no mutation exemptions.

```sh
coax/bin/coax test coax/lib/crypto/entropy.coax
coax/bin/coax mutate coax/lib/crypto/entropy.coax
python3 coax/tests/entropy.py
```

- Pure contract: 2 check groups, **57/57 mutants killed**. Native harnesses also
  assert buffer postconditions and call counts through the generated C ABI.
- Actual macOS `SecRandomCopyBytes` and explicitly compiled test-provider
  adapters pass clang ASan/UBSan. Cases cover zero, 1, 32, 65,535, 65,536,
  65,537 and 1,048,576-byte requests; invalid counts/capacities/null/address-wrap
  spans; aliases; absent/partial/short/negative/excessive completions; output
  preservation, scratch reset and unchanged tails. Leak detection is disabled
  because unsupported on this host; no leak-check claim is made.
- The same real-OS and test-provider C executables run successfully in the
  **iOS 26.5 simulator**, rather than merely compiling an iOS target. The real
  policy and native adapter also compile for physical arm64 iOS 26.0; no
  physical-device execution is claimed.
- **Chrome 156.0.8078.48** executes the production browser module with the actual
  compiled Coax Wasm and real WebCrypto. All **21** browser cases pass, including
  exact chunk boundaries, the maximum request, missing/throwing/wrong-return
  providers, an exception after partial scratch writes, invalid spans and
  admission without any RNG call. The test uses an isolated temporary browser
  profile and local HTTP server. No entropy bytes are logged.
- Production native object symbols name the platform CSPRNG and contain no test
  provider symbol. The Wasm face imports memory only; it supplies no substitute
  source of randomness.

Compiler: existing `coax0 0.13.0`; C toolchain: Apple clang 21.0.0. The focused
adapter harness passes all 3 tests. No compiler bootstrap, full app suite or
TestFlight build was repeated. Linux's `getrandom` branch has not been executed
on this Mac (Docker daemon unavailable); the portable harness can execute it on
Linux. Safari/Firefox and physical-phone conformance remain later target gates.

Verified authority source SHA-256:

```text
entropy.coax 7cbaed5034107afcfe8e3174795cafb9fc2692f230757e6849abeced69a1c5a1
```

References: [Apple SecRandomCopyBytes](https://developer.apple.com/documentation/security/secrandomcopybytes(_:_:_:)),
[Linux getrandom](https://man7.org/linux/man-pages/man2/getrandom.2.html),
[Web Cryptography getRandomValues](https://www.w3.org/TR/webcrypto/#Crypto-method-getRandomValues).
