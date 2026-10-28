# Browser random import

Task `host-random-import` (#d0d7), worker `codex-nostr-codecs`, 2026-10-28.

## Consumer contract

The browser host now supplies the scalar Wasm import:

```c
__attribute__((import_module("env"), import_name("coaxial_random")))
extern uint32_t random_bytes(uint32_t output, int32_t length);
```

Zero means a complete request succeeded. Other results are the shared Coax
`ENTROPY_*` errors. Zero bytes succeed without growing memory or calling the
provider. Positive requests have the Coax authority's 1 MiB ceiling; WebCrypto
calls have its separate 65,536-byte chunk ceiling. There is no clock/PRNG
fallback or production seed/provider argument.

An app loads `coax/lib/crypto/entropy.coax`, then includes the scalar
`coax/platform/entropy-wasm.c` face in its own C face. Define
`COAX_ENTROPY_HEADER` to its generated header name (for example `"App.h"` for a
separately compiled library), or use the default `"entropy.h"`. The import is
available only when the module actually exports the seven shared contract
functions. `prepareRandomImport` binds its own instance before `_initialize`,
including during hot reload; an unbound call throws rather than succeeding.
The Coax contract still owns limits, admission and completion/copy/reset
policy. A separate output-only Coax preflight runs before scratch allocation,
so the host does not claim a scratch span exists before reserving it.
JavaScript owns address observation, host scratch reservation and the
WebCrypto effect. The following CLI/iOS integration tasks remain separate.

The host reserves **1 MiB** of scratch lazily in its Wasm memory on the first
positive valid request and reuses it for synchronous calls and successive
instances. Unused apps pay no reservation. It is outside the app's existing
data/arena when allocated. If a larger reloaded module owns that interval,
the next call reserves a new interval beyond current memory. Wasm memory
cannot shrink, so each such relocation retains the old memory footprint until
the page closes; it does not retain completed entropy bytes. The host's 64 MiB
maximum remains in force. Failure to grow reports the Coax capacity error,
preserves output and never invokes WebCrypto. Views are rebuilt against the
current memory buffer on every call.

The shipped Coax completion commits output only after a full provider success.
Missing providers, exceptions after partial scratch writes and mid-call buffer
growth failures preserve the output. Admitted completion resets its scratch
prefix. This observable reset **does not claim secure erasure**, and does not
destroy or own the caller's output. No keys, storage or app UI were added.

Unknown function imports now refuse at module loading instead of receiving an
implicit successful no-op. This includes unsupported WASI `random_get`; a
different RNG ABI cannot silently bypass the explicit contract. Import tables
have null prototypes, so an inherited name such as `env.toString` also refuses.
The existing optional WASI libc probes `fd_fdstat_get`, `fd_seek`, `fd_close`
and `clock_time_get` return ENOSYS (52) without claiming initialized output.
Existing `fd_write` and `proc_exit` handling remains available.

Development loads the source modules through the existing `/coaxial/` route.
Deployment bundles all three host modules before content naming and stripping,
so the immutable deployed script has no missing relative dependencies.

## Evidence

```sh
coax/bin/coax test coax/lib/crypto/entropy.coax
coax/bin/coax mutate coax/lib/crypto/entropy.coax
coax/bin/coax test coax/tests/entropy_import.coax
python3 coax/tests/entropy_import.py
coax/web/check-visuals.sh apps/openagents-web
```

- The new request-limit and output-preflight checks failed before adding their
  implementations. The authority passes 2 check groups and **71/71 mutation
  kills**, with no exemptions. This extends the prior entropy scope's 57 mutants
  with the public limit accessor and output-only preflight; existing completion
  behavior is unchanged.
- **29 actual Chrome 156.0.8078.48 cases** pass against compiled Wasm imports,
  real WebCrypto and the bundled/stripped production host. They cover sizes
  zero through 1 MiB; absent/throwing providers; partial scratch writes;
  invalid pointers/counts, aliasing and the request budget; external and
  mid-call memory growth; failed `memory.grow`; shared reservation footprint;
  independent instance dispatch; and relocation around a larger module.
- Full host startup invokes a real Wasm consumer through the normal message
  path and verifies that reactor initialization can call the bound import.
  A libc `puts`/`fflush` variant also passes with optional WASI probes returning
  ENOSYS. Missing-policy, unsupported-WASI-RNG and inherited-name modules fail
  before being usable. No random bytes are printed.
- Chrome runs with a temporary profile and a local fixture server. A test-only
  CDP reader uses real wall time because virtual-time deadlines can run ahead
  of IndexedDB's asynchronous startup callbacks. It observes pass/fail text;
  it supplies no randomness or app policy.

Compiler: existing coax0 0.13.0. No compiler bootstrap, app rebuild, TestFlight
build, or unrelated crypto suite was repeated. This proves the browser host
boundary; it is not production Nostr/key lifecycle approval or a claim of
Safari/Firefox/physical-phone conformance. The previous native/iOS entropy
adapter evidence and its explicit Linux runtime limitation still apply.

Authority SHA-256 at verification:

```text
entropy.coax efbe909c032939134e83db8ceb296d4da04baac37cd5a291f4276aad955e84fe
```

## Shared-host compatibility check

Before integration, downloaded the currently published OpenAgents/Verse Wasm
files referenced by `/`, `/projects`, `/projects/coax`, `/verse` and
`/verse/galaxy`, and inspected their actual `WebAssembly.Module.imports`.
These four public app artifacts import only:

- `env.memory` and `env.coaxial_http` on all four apps;
- `env.coaxial_socket_open`, `coaxial_socket_send`, `coaxial_socket_close` on
  the project app;
- `wasi_snapshot_preview1.fd_fdstat_get`, `fd_seek`, `fd_write` on all four.

Every observed import has an explicit handler. No newly rejected libc import
was found. The audited artifact SHA-256 values were:

```text
splash   adae9868ecc23c4be4824cc323f8cc235a2d973a4ebb691003bfaf6cd8a537f2
projects 370f89d19e8692c371e496bc0036b6a43d88589791b7ee904ef9ac694485aef3
project  a935571d7636df52c148199579de609547c31df60d7e072332b1df9abf015bd0
verse    6c4620602d7adcab30e5c1949d25dee85490906e0481074bfb8e65143be17de2
```

Then served the actual published project HTML, Wasm and WGSL locally, replacing
only the host URL with this change's bundled/stripped host and resolving its
asset URLs to those unchanged downloaded bytes. A fresh headless Chrome profile
with WebGPU enabled reached `coaxial.ready=true`, kept GPU rendering active and
rendered a **756×469** project frame. The host's `?probe` readback reported
**peak 255, mean 16.05**, with **zero Runtime exceptions**. This exercises the
real project state, C face, Coax initialization and renderer, beyond the small
test consumer. No OpenAgents/Verse source or binary was rebuilt for this check.
Private/unpublished app artifacts were not part of this public-artifact audit.
