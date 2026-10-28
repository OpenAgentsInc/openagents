# Identified-buffer erasure

This slice provides an explicit erasure boundary for **identified caller-owned
buffers**. `coax/lib/crypto/zeroize.coax` owns span admission, bounded copy
registration and cleanup state. `coax/lib/crypto/platform/zeroize.c` binds IDs to
pointers, transfers admitted bytes and performs volatile byte-zero stores with
a C11 compiler fence. It adds no compiler intrinsic or compiler source change.
A value barrier by itself is not this erasure effect.

The mechanism does **not** erase hidden compiler/register/ABI copies, arbitrary
Coax by-value arrays/structs, prior string conversions, another scope's copies,
OS/swap/crash snapshots or hardware caches. Existing HMAC/HKDF/scalar code has
not been retrofitted or approved for production secrets by adding this module.
There is no general promise that every copy of a secret can be found or erased.

## Standalone range effect

```c
uint32_t coax_secure_zero(void *buffer, int64_t capacity,
                          int64_t offset, int64_t length);
```

Coax admits capacity 0–1 MiB and a nonnegative range wholly inside that
capacity, using subtraction after its bounds checks. A nonempty range requires
a nonnull pointer. Empty ranges accept a null pointer and perform no pointer
arithmetic or writes. Refused calls change nothing; accepted calls write only
the specified range. Native pointer provenance, writable/live storage and
exclusive synchronization are caller obligations. The model cannot turn a
lying pointer/count into valid memory.

The C adapter performs no secret-dependent algorithm, allocation or logging.
Volatile unsigned-byte stores are observable C effects; `atomic_signal_fence`
adds a compiler ordering boundary. This is a scoped toolchain/artifact claim,
not a physical memory sanitization or side-channel certification.

## Registered scratch and copies

`coax_secret_scope` is a caller-owned, initially zeroed registry of at most
eight **disjoint** writable buffers. Its Coax policy holds IDs, capacities,
dirty-prefix lengths and cleanup state; it never holds payload bytes. C holds
the matching pointers. The API never passes a secret payload by value.

1. Start with `coax_secret_scope scope = {0}`. Register each existing secret
   buffer with `coax_secret_register(&scope,id,pointer,capacity,secret_prefix)`.
   Register empty destination scratch with prefix zero **before copying**.
   IDs are nonzero and unique; aliasing registered capacities is refused.
2. Copy only through `coax_secret_copy(&scope,source_id,destination_id,length)`.
   The source prefix must cover the request, and the destination capacity must
   fit it. Unknown destinations and overlapping nonempty copies refuse. Any
   nonempty copy marks the destination's **entire exclusive capacity** dirty
   before byte transfer. This conservatively covers partial writes and stale
   scratch tails; caller guards must lie outside registered capacity. A shorter
   later copy cannot shrink that cleanup obligation.
3. On success, error or cooperative cancellation, call
   `coax_secret_close(&scope,reason)` with reason 0, 1 or 2 respectively. All
   reasons require the same complete cleanup. Every dirty prefix is wiped and
   acknowledged; a failed effect stays pending. Later buffers are still
   attempted. Retain the buffers and scope until retry succeeds.
4. Require both successful return and `zero_scope_complete(scope.policy)`
   before releasing/reusing storage. `coax_secret_reset` refuses incomplete
   cleanup; after success it clears metadata/pointer bindings for reuse.

Registration failure leaves that new buffer under its caller's original
cleanup obligation. The caller must wire cleanup into every exit path (for
example a platform `defer`/cleanup guard). Process termination, crashes,
asynchronous destruction during an instruction and external writes/copies that
bypass this API are not covered. Public scope structs are a cooperative native
contract, not tamper-proof authority or untrusted wire objects. Do not overwrite
or clone a live registry to discard pending work. Scope calls are serialized.

`ZeroScope` changes are metadata values, so copying the model itself makes no
payload copy. A host wipe acknowledgement is trusted evidence from the effect,
not proof that the model can inspect physical memory. The negative tests
deliberately make an effect lie and show why both artifact and independent
payload checks are needed.

Status values are `OK=0`, bad capacity 1, bad range 2, missing address 3,
alias 4, bad state 5, effect failure 6, unknown/duplicate ID 7, bad close reason
8 and pending cleanup 9. These are local library statuses, not new protocol
error codes. `zero_scope_pending` returns the admitted pending prefix for a slot;
zero means no work for that slot. An initially open empty scope is pending
until explicitly closed; it does not invent a wipe.

## Build boundary

Generate the policy header and C with the existing Python-free compiler:

```sh
mkdir -p /tmp/coax-zeroize-example
coax/bin/coax c coax/lib/crypto/zeroize.coax --library \
  --header /tmp/coax-zeroize-example/zeroize-policy.h \
  > /tmp/coax-zeroize-example/policy.c
cc -std=c11 -O3 -I/tmp/coax-zeroize-example -Icoax/lib/crypto/platform \
  /tmp/coax-zeroize-example/policy.c coax/lib/crypto/platform/zeroize.c \
  tools/coax-nostr/zeroize/probe.c -lm -o /tmp/coax-zeroize-example/probe
```

The platform header includes that generated header and has a compile-time
eight-slot ABI assertion. Consumers must bind the policy and platform object
together. This slice does not add a new app build dependency, a source-level
Coax erasure intrinsic, automatic destruction or target-independent lifetime
checking. It supplies a reusable boundary for subsequent explicit integration.

## Focused evidence and limits

```sh
coax/bin/coax test coax/lib/crypto/zeroize.coax
coax/bin/coax mutate coax/lib/crypto/zeroize.coax
python3 tools/coax-nostr/zeroize/test_zeroize.py
```

On 2026-10-28, Coax 0.15.0 passed **6 checks** (including 3,380 span cases)
and killed **252/252 mutants**, with no equivalent waivers. Six independent
test methods cover actual native checked/release emission at `-O3`, with and
without LTO; actual wasm32-wasip1 execution in Node/V8/WASI in those same four
configurations; and native ASan/UBSan. They check full 1 MiB erasure, range
guards, refused/extreme metadata, aliases, registered copies, successful/error/
cancel cleanup, failed-wipe retry and partial-copy failure.

`probe.c` deliberately never reads, hashes, compares or returns buffer contents
after the wipe. The buffer's lifetime ends on return. Artifact inspections
are separate from observable functional tests: on Apple arm64, each LTO probe
retains **64 byte-zero store instructions**; the optimized dynamic range
effect retains its byte-store loop. LLVM 23's Wasm artifacts retain 64 stores
in the LTO probe and an eight-store loop body without LTO. Binaryen disassembly
is used for Wasm (the installed LLVM objdump crashes on these linked modules).
The Node probe observes only its status, not post-wipe contents. Each functional
Wasm configuration independently checks 2,145 ranges and all three cleanup
reasons, plus the full 1 MiB bound.

Faults that omit the wipe, return early before close, or forget to register a
copy's cleanup obligation are rejected. Omitted-wipe LTO artifacts fail both
native and Wasm instruction gates; a partial copy failure remains registered
and is wiped on cancellation. Native/Wasm artifacts, source hashes, compiler
versions, observation boundaries and functional results are retained in
`evidence/optimized-2026-10-28.json`. These are finite scoped observations,
not proof over arbitrary compilers or programs.

The native instruction audit is explicitly Apple arm64/Apple Clang 21. The
Wasm audit uses LLVM 23 and the installed Binaryen disassembler; it refuses
missing tools rather than silently skipping them. No Linux instruction audit,
Safari/JavaScriptCore, physical iPhone or application cleanup adoption is
claimed. Those consumers and all hidden-copy/secret-codegen review remain
separate gates. Python is explicit test tooling only.
