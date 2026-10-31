# 19. Keep the model resident

Authentication, model mapping, device compilation and placement belong to
startup. A request reuses one authenticated session and preallocated arenas.
The [host](../platform/serve-host.c) owns those allocations and their lifetimes;
Coax owns their sizes, admission and the numerical work.

## Plan before allocating

`sv_plan` derives worst-case execution storage from the model and configured
token capacity. `ls_memory_for` accounts for text preparation, tokens,
probabilities, model state, scratch and an immutable CPU prefix snapshot.
`--host-mib` bounds this execution payload; a short budget refuses startup
with the typed `limit_exceeded` reason before allocating those arenas.

Every decoded question is then admitted against its actual token/prefix
layout and the reserved capacity. A long question gets a bounded typed HTTP
refusal before execution. There is one active request, with no unbounded queue
and no request-sized allocation on the device path.

| Storage | Lifetime and admission |
| --- | --- |
| Original mapped artifacts and bound parameter vectors | One immutable authenticated session |
| Device-resident and pinned streaming weights | One placed device plan, with an explicit device budget |
| CPU state, scratch and prefix snapshot | One reserved capacity plan; fresh request metadata and isolated suffix state |
| Device model state/scratch | The device plan owns these; the host allocates no redundant CPU model-state arenas |
| HTTP/protobuf buffers and metadata | Fixed protocol bounds, separate from the execution-payload budget |

The execution budget is not total RSS. Mapped files, driver memory, pinned
weights, fixed HTTP/protobuf storage, allocator overhead and page cache have
separate costs. A memory plan is arithmetic, not a reservation guarantee from
the operating system. Chapter 18 records observations alongside the plans.

For the measured original Elephant at capacity 512, the device host plan is
3,288,560 bytes, versus 691,746,800 bytes for the conservative CPU execution
plan. Removing 656.6 MiB of reserved execution payload does not remove the
mapped weights, pinned streaming weights or driver memory, and is not a
656.6 MiB measured RSS claim.

## Bind verification records to the mapped file

The [first verification-cache work](../../docs/verification/psionic-ch15-verified-files-2026-10-29.md)
already retained evidence of a successful original-file hash. Its record binds
pinned shard name/digest, length, device, inode, mtime and ctime. A mismatch
falls back to hashing; config, tokenizer and bound inventory are still checked.

This chapter closes its outstanding pathname race. The
[mapper](../platform/verified-file.c) now opens a descriptor, maps that
descriptor, and obtains identity through `fstat` on the same descriptor.
Replacing the pathname cannot attach the replacement file's identity to the
old mapping. Both original-weight device harnesses use the same mapper.
The [native check](../tests/verified_file.c) replaces a path between mapping
and attachment and proves the old/new mappings retain their own inodes/bytes.

The cache is trusted local evidence from a prior successful hash, not a
signature or remote attestation. Keep it in the owner's private cache directory.
Record failure preserves correctness by hashing again. Keep the versioned
checkpoint files immutable throughout a session: a read-only mapping does not
stop another process writing the underlying file, and startup authentication
does not authenticate subsequent external mutations.

## Reuse allocations without reusing another request's state

The CPU child inherits the authenticated session and reserved arena. Each
request starts with empty prefix metadata; chapter 23 may reuse a snapshot only
among that request's questions. The parent joins/kills the child before reading
its shared timing observations. A device decision runs in the owning process
and checks cancellation between layers. Neither path borrows a previous
request's mutable model state.

The [example](../examples/resident-decision.coax) compares plans on the owned
toy and refuses a budget one byte short. Its device calculation is a storage
plan, not a claim that the toy executed on a GPU:

<!-- source: psionic/examples/resident-decision.coax example -->
```jai
main :: () {
    toy:[1]SvToy;e:=sv_test_toy(toy[..],128);
    cpu:=ls_memory_for(e,68719476736,false);gpu:=ls_memory_for(e,68719476736,true);
    #assert cpu.code==SvCode.none && gpu.code==SvCode.none && gpu.state==0 && gpu.work==0;
    print("CPU execution payload: % bytes; device execution host payload: % bytes\n",cpu.bytes,gpu.bytes);
    print("One byte short: %\n",sv_code_name(ls_memory(e,cpu.bytes-1).code));
    print("The authenticated session is shared; each decision starts with fresh state.\n");
}
```

```shell
psionic/platform/serve.sh --elephant-gpu ORIGINAL_DIRECTORY \
  --capacity 512 --device-mib 1024 --host-mib 64
```

A 64 MiB host execution budget suffices for this device's text/control payload;
large pinned/mapped weights and fixed protocol buffers remain separate. Use
CPU plans' actual reported requirement rather than applying that device value
to a CPU model.

The [Part III record](../../docs/verification/psionic-partIII-2026-10-29.md)
retains cold/verified starts, actual allocation differences and focused
sanitizer checks. No original whole-model CPU decision is repeated.
Continue with [the endpoint](ch020-serve-systemone.md), its
[protobuf carriage](ch021-the-protobuf-wire.md), then
[supervised operation](ch022-run-it-live.md).
