# Experimental ECDH public ABI migration

This is an explicit compatibility break for the experimental public ECDH
build. The previous `ecdh_stage`, `coax_ecdh_raw` and `coax_ecdh_cleanup`
entrypoints are not supplied by the new owner-only artifact. Existing binary
users must migrate to the six `coax_ecdh_owned_*` entrypoints and their
documented acquisition, pending, retry, cancellation and close protocol.
Repository search found only test consumers of the former host API; that
does not establish the absence of external users. The old host source remains
available for historical fixture use and is not part of this public build.

Removing the raw stage's C export makes its generated procedure internal.
An adapter forwards the same three views to it. The adapter is only a build
bridge: compiling it independently without hidden visibility can export it.
The experimental macOS build tool produces a new directory atomically without
replacing an existing destination, hides native internals, and checks the
exact exported native symbols. Its Wasm build explicitly exports the six
owner operations, linear memory, allocator functions and reactor initializer,
then checks that exact set. Native and Wasm negative controls demonstrate that
an unsafe link can expose the bridge. The tool and stable filesystem,
compiler, linker and runtime are trust premises, not an artifact signature.
The Wasm host can inspect and modify linear memory; this ABI does not isolate
secrets from that host. Neither arbitrary builds nor prior binaries acquire
the new export boundary automatically.

The checked and release public artifacts matched 783 official, adapted and
adversarial raw-x fixtures on native and Wasm. The harness binds the actual
artifact and manifest to the current compiled source closure and host files;
a deliberately foreign source identity was refused. Native and Wasm export
checks included deliberately exposed bridge controls. C and C++ consumers
linked and ran against relocated published native artifacts. An earlier
prototype recorded a temporary native install path and failed that linked
consumer test; the current build checks an `@rpath` install name. Native
consumers must configure an rpath to the published library. The migrated
test-only bridge also passed the previous native/Wasm vector and host fixtures.
The [evidence record](2026-10-29-ecdh-abi-migration.json) pins the source,
builds and logs, including the 107 Coax checks, 1,842 killed mutations and
the 35 killed registry-policy mutations.

The versioned owner retains the explicit live, exclusive, disjoint and stable
caller-buffer premises and synchronous wipe-effect contract described in the
[prototype](2026-10-29-ecdh-owned-prototype.md). Busy means the call took no
action; pending is incomplete and requires retaining the original handle and
private output as documented. No completed error for the matching accepted
operation is reported while its tracked work, staging or private output wipe
is pending. Invalid
handles and pre-acceptance metadata refusals do not establish ownership over
arbitrary caller memory. The source-level conditional audit does not analyze
the C host, allocator, runtime, lowering, registers or generated target code.

| Entry | Return and ownership interpretation |
| --- | --- |
| `acquire` | Zero means no slot was acquired. A nonzero handle starts with the statically initialized or previously wiped slot in its ready state. |
| `start` | An invalid handle or invalid input metadata refuses before secret acceptance into that handle's ready slot. Peer and scalar errors are returned only after tracked work and staging wipes; a wipe failure remains pending. A valid result remains staged and returns pending. |
| `collect` | An invalid handle owns no matching generation. Invalid output metadata leaves staging owned and returns pending without writing that output. A valid output is marked dirty before copying; success requires staged-result wipe and the serialized transfer. Failed wipe leaves pending cleanup of staging and output. |
| `retry` | A matching generation reports a completed result only after every pending dirty region has been wiped; otherwise it remains pending. A stale handle cannot operate on a later generation. |
| `cancel` | A ready slot can close without secret work. During work or transfer, cancellation remains pending until tracked dirty regions are wiped; after completion it reports the already recorded result. |
| `close` | A ready or completed generation can close. An active or pending generation cannot be freed or reused. |

Each status-returning operation can return busy without taking action when
another operation holds the slot lock; `acquire` returns zero in that case.
The caller retries. These statements rely on the stated
bearer-handle and buffer premises and the actual whole-region wipe effect.

The revised classified stage audit remains `accepted=false` with the same
two public-unproved field guards. The former registry refuses the changed
source graph. A separate opt-in registry pins the new source and reviewed
adapter, exact guard set, classification, proof, tool identities and manual
correspondence, and emits only a conditional source-scope decision. Its
outer launcher is an explicit trusted anchor. The earlier multiplication
post-fold lemma remains a reviewed premise. Neither decision establishes
target timing, hidden-copy erasure, crash/fork/reload cleanup, authenticated
peer identity or production-secret suitability.
