# Owned ECDH scratch lifecycle

The controlled owner-only public artifacts introduced by the
[ABI migration](2026-10-29-ecdh-abi-migration.md) do not accept a caller
workspace. The host owns one statically initialized slot and tracks work,
result staging and any private output through the separately checked Coax
state policy. The migration document maps every owner return, including
pre-acceptance refusal, busy, pending cleanup, cancellation, retry and
completion. Existing binaries and independently built raw adapters are not
retroactively changed; callers must migrate to the new explicit ABI.

The slot starts without previously owned secret data. After secret work begins,
peer/scalar errors or cancellation complete only after the owner's dirty
regions have been wiped. A failed effect remains pending and prevents slot
reuse. If output was copied before a later failure, it remains private and
must be wiped together with staging before an error completes. The input key
remains caller-owned. Invalid or overlapping input is refused before secret
work; no claim is made that arbitrary caller memory was wiped or initially
clean. Valid, live, stable, exclusive, disjoint storage and truthful handle
use remain caller premises, as does the synchronous whole-region wipe effect.

The [evidence](2026-10-29-ecdh-owned-lifecycle.json) retains the migration's
107/107 Coax checks, 1,842/1,842 changed-stage mutations and independent
783-case actual-artifact oracle, plus the unchanged policy's 222/222 mutation
result. Updated direct and failure-injected fixtures passed checked/release
native, native ASan/UBSan and Wasm; they cover ready cancellation, failure
during cancellation, retry and cancellation after a copied output. Native
contention and reentrant callbacks remain finite test observations.

A separate test-only shared-memory Wasm threads build used two worker
instances with one memory and manually assigned stacks. During an injected
paused wipe, five competing calls refused without writing the proposed output;
after release, cancellation and close completed. This passed checked and
release modes. A temporary lock-removed negative control produced the expected
contention violation in both. The worker uses the specific one-time
initialization pattern in the fixture. Boundary canaries stayed unchanged,
but do not establish a general stack high-water bound. The published Wasm
artifact is unshared, and its host can inspect and mutate linear memory.

The classified raw stage audit remains refused; its separately reviewed
source decision is conditional on exact inputs and a manual proof premise.
Neither that decision nor these tests establish hidden compiler/register/ABI
copy erasure, crash or fork/reload cleanup, target/JIT constant time,
untrusted-host isolation or production-secret suitability.
