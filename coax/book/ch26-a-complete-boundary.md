# 26. A complete boundary

The Mandelbrot project in Chapter 2 introduced the language. This project
connects the pieces you need in an application: an existing protocol,
generated codecs, checked request decisions, a native effect, retained
storage, failures and independent verification.

It is a local command-line program, not a new app screen or cloud service.
It writes a real protobuf file, closes it, reads it back and delivers a
completion. We control the order of callback delivery so the stale-result
case is repeatable, without depending on thread timing.

The full source lives in
[examples/ch26_workflow](https://github.com/OpenAgentsInc/openagents/tree/main/coax/book/examples/ch26_workflow).
It reuses Chapter 23's request machine.

## Choose an existing contract

The repository's `ios-home` descriptor profile includes the published
`openagents.engine.v1.HudInvocation` message. Its fields are:

```protobuf
message HudInvocation {
  uint32 schema = 1;
  string world = 2;
  string action_id = 3;
  string request_id = 4;
  uint64 revision = 5;
}
```

We do not change that schema. The book uses `world = "book"` and
`action_id = "preview"` as a local convention. These names do not register
an action in Verse. A decoded invocation is data, not permission to
invoke a screen or run code.

For the exercise, `revision` carries the request epoch. This is a local
test convention, not a new definition of revision for every HUD consumer.
A production surface must follow its actual invocation and admission
contract.

## Generate the representation adapter

`workflow.coax` imports the checked-in descriptor relative to its own
location and loads `../lib/ch23_request_policy.coax`. `run.sh` compiles
that real source graph directly. The example builds with the compiler, host C
compiler and shell; neither Python nor `protoc` is a build dependency.

The Coax adapter has two exported procedures:

| Procedure | Result and ownership |
| --- | --- |
| `workflow_packet(epoch, out)` | Writes into caller-owned output; returns its byte count, or `-1` for a codec failure |
| `workflow_read(wire)` | Borrows the input during the call; returns a copied `u64` revision and an error code |

The writer alternates two 128-byte scratch buffers while applying
setters, then encodes into the caller's output. It returns no view into
its local arrays. The reader copies a scalar, so no borrowed packet view
escapes it either. Its full-width `u64` result is deliberate: a wire
revision must not silently narrow to the request machine's `u32` epoch.

The representation adapter does not apply a domain authorization policy.
The request decisions live in their own `#authority` source unit, loaded
into the same program. Source-unit authority in 0.11.0 keeps the policy
checked without marking every generated accessor in the unmarked adapter.
Both sets of exports share one generated C library and header. Importing
the descriptor inside the authority unit would still make its generated
declarations subject to that unit's obligations.

## Write the checks at the right boundary

The decoder checks use an explicit independent 33-byte packet and a
malformed varint. A second packet exercises the full `uint64` range.
Those checks run in the evaluator.

The writer uses runtime storage-overlap checks. Calling it from a
compile-time check reports E0404, because the evaluator cannot observe
native addresses. The C host therefore checks exact output bytes and
all output capacities shorter than the expected packet, plus the exact
fitting capacity, zero and maximum `u32` revisions and malformed input.

Those are different tests with different scopes. The native checks are
not an excuse to remove the request machine's Coax checks or mutation
gate. The build runs the decoder checks before generating the library,
and compilation enforces the loaded request policy's authority checks.

## Perform the effect

The host's sequence is deliberately visible:

1. Start request 1, cancel it, then start request 2.
2. Capture request 2's epoch in the operation record.
3. Encode its preview invocation into a retained 128-byte array.
4. Write the bytes to a temporary file and close it.
5. Read the file into another retained array; decode its revision.
6. Deliver request 1's delayed completion. Coax ignores it.
7. Deliver request 2's completion. Coax stores result `42`.
8. Exercise a failed completion and a timer at its exact deadline.

The host checks every file operation. This small harness uses assertions
to stop on unexpected I/O failures; a UI adapter should instead translate
an expected failure into the request's failed-completion event and render
its state. No Coax procedure opens files or decides which platform queue
to use.

The host keeps both packet buffers alive throughout their use. It copies
the decoded scalar before releasing the storage. With actual asynchronous
I/O, the operation record would retain its buffers until the platform
completion ended, even after cancellation. Chapter 23 explains why an
epoch alone cannot protect memory.

This is a real file round trip, but not crash-safe persistence. There is
no atomic replacement, `fsync` durability promise or recovery log. An app
that stores durable state needs a versioned record, bounded reads,
atomic-write policy and tested restart behavior. Do not persist the
native request struct as an alternative.

## Run the whole path

From the repository root:

```shell
coax/book/examples/ch26_workflow/run.sh
```

It builds and runs both debug and optimized native programs. Each prints:

```jai
file: 33 bytes, revision 2; result 42; stale ignored; failure and timeout checked
```

To add the request mutation gate and native sanitizers:

```shell
BOOK_MUTATE=1 BOOK_SANITIZE=1 coax/book/examples/ch26_workflow/run.sh
```

To also check interoperability, set `PROTOC` to your pinned official
`protoc` executable:

```shell
PROTOC=/absolute/path/to/protoc BOOK_SANITIZE=1 coax/book/examples/ch26_workflow/run.sh
```

`protoc` decodes the emitted file and compares it with the independently
written text fixture. It then encodes that fixture and compares the bytes
with Coax's output. Only if both agree does the script print:

```jai
protoc: both directions agree
```

Everything is built in a fresh temporary directory and removed when the
script exits. The shared book check command also runs this example;
`PROTOC` and `BOOK_SANITIZE` can be supplied there too.

## Follow the same seam into the app

The current iOS app uses this shape with richer native adapters:

- `home.coax` imports `ios-home`, admits the resource and emits the
  existing HUD invocation through generated codecs.
- `camera.coax`, `player.coax` and `grid.coax` decide movement and
  presentation values; the native host owns input and Metal objects.
- `haunt.coax` owns local Haunted House behavior, editing and protocol
  documents; `haunt-buffer.coax` independently admits GPU spans, and
  `haunt-light.coax` supplies shader math. Local documents and invitations
  do not imply that the retired public-world services are available.

The actual app's verification entry points are documented in
[apps/openagents-ios/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-ios/README.md)
and
[verse/haunt/verification.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/haunt/verification.md).
The capstone does not run those device or simulator tests for you.

When extending it, keep the same discipline. Add a contract before a new
interchange value, checks before decisions, native tests for effects and
storage, and a measured workload before a speed claim. The next useful
step is a real feature crossing this seam, not an unbounded framework
around the demonstration.
