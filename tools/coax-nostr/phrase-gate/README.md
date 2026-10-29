# Public phrase-gate fixture tools

These tools exercise the real Coax worker with the reviewed public fixtures. No production phrase, key, unlock entry or relay is used. `test_phrase_gate.py` is a development test harness; the actual build and stage artifact path is Python-free.

```sh
coax/bin/coax test coax/lib/access/phrase-gate-worker.coax --library
coax/bin/coax mutate coax/lib/access/phrase-gate-worker.coax --library
COAX_PHRASE_GATE_REPORT=/tmp/phrase-gate.json python3 tools/coax-nostr/phrase-gate/test_phrase_gate.py
bun tools/coax-nostr/phrase-gate/build.ts /tmp/phrase-gate --release
bun tools/coax-nostr/phrase-gate/browser.ts /tmp/phrase-gate/experimental/phrase-gate-worker.wasm /tmp/phrase-gate/build/observe.wasm /tmp/phrase-browser.json /tmp/phrase-gate
node tools/coax-nostr/phrase-gate/extra-vectors.ts verify-browser /tmp/phrase-browser.json
```

Native tests use Clang ASan/UBSan and the actual C face; Wasm tests use emitted modules and the same fixed-round state machine. The independent Node/OpenSSL oracle generates complete 4,096/4,097-byte boundary packets and opens newly OS-randomized packets returned by the browser. There is no shortcut KDF in the worker. The Coax state unit check alone models a public completed PBKDF2 state to qualify the terminal transitions; actual native and browser integrations run all 210,000 rounds.

`build.ts` uses `coax`, Clang/WASI and Bun. It creates a `baseline` artifact containing the untouched legacy reader and entry, and a separate `experimental` artifact. Both have content-addressed manifests. Normal deployable Wasm excludes fixture fault/observation exports. Only the separate `build/observe.wasm` enables controlled trap/uncertain-cleanup observations. Neither a production key nor a fixture phrase is embedded as a worker fallback.

The browser harness uses a fresh headless Chrome profile and actual module workers, not user browser tabs. Test-only entropy wrappers simulate unavailable, throwing, partially writing and wrong-return providers. Test-only workers exercise death, malformed limits replies and silence. Its local HTTP activation endpoint copies complete baseline/experimental artifact sets, hashes every served asset, exercises the actual old and new readers, then restores and rechecks the baseline. That endpoint is not part of the staging service.

## Isolated staging

```sh
bun tools/coax-nostr/phrase-gate/stage.ts /tmp/coax-phrase-gate-cloud --release
```

`stage.ts` assembles only public manifests/assets plus `server.cjs` and a minimal container. `build/` is excluded from the image. The fixture service has no project keys, module loader, event fetch, relay connection, authentication API or production injection. Its root redirects to the explicitly selected `GATE_PROFILE=baseline|experimental`; both artifacts remain available at their separate paths. A future real migration still requires the declared minimum 30-day rollback window, owner approval and independent security acceptance. This fixture does not flip production defaults.

The selected staging deployment and exact rollback observations are retained in `evidence/`. Production `gate.ts` and `inject.ts` are compared byte-for-byte with reviewed commit `fa594f441d` in the native harness, and the legacy source hash is pinned by the original public fixture.
