# Vector-runner evidence, 2026-10-28

Task `crypto-vector-runner` (#0e80).

- Test-only fingerprint adapter: 20 combined checks pass; 363/363 mutants
  killed in its loaded authority scope, with no exemptions. This includes the
  fingerprint module's existing checks; it is not 363 new algorithm discoveries.
- Nine focused tooling tests pass, including malformed/empty/truncated input,
  modified expected bytes, a real incorrect Coax/native candidate and unsupported
  suites. Reported success requires every native per-row result.
- All 129 exact NIST SHA-256 ShortMsg/LongMsg rows pass through emitted checked
  C with clang ASan/UBSan, and through release C at -O2.
- The in-development streaming SHA module's explicit adapter independently
  passes the same runner/SPI and 129 rows with sanitizers. That algorithm's
  mutation, streaming/tagged/Wasm/performance evidence is owned by its task.

The runner pins a temporary compiler copy, hashes transitive source inputs and
native artifacts, and records vector/record identities. Source integrity uses
only the already pinned upstream corpus; no network refetch or full compiler/app
suite was run here. Other unimplemented suites fail closed. This is native
vector infrastructure and correctness evidence, not general cryptographic,
side-channel or production approval.
