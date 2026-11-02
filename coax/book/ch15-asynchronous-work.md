# 15. Asynchronous work and lifetimes

A phone app starts an operation, keeps drawing, and receives its result
later. The result may arrive after the person cancelled, opened another
screen, retried the operation or put the phone to sleep. Correct code
must account for those events even when the platform calls its completion
handler exactly once.

`coax0` has no async functions, promises, scheduler or cancellation runtime.
The job system designed in Chapter 14 is about parallel work and effects;
it does not supply those asynchronous I/O facilities. We can nevertheless
write and check the decision logic today: the host performs an operation,
and Coax decides what each command or completion means.

## A request is a state machine

Our example owns one request slot. The host serializes access to it on
one queue. A request can be idle, running, succeeded, failed, cancelled or
expired. Starting increments an epoch, which identifies this use of the
slot. A callback captures that epoch before the host starts the effect.

The complete runnable source is
[ch15_requests.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/examples/ch15_requests.coax).
The demo loads its checked policy from
[lib/ch15_request_policy.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/examples/lib/ch15_request_policy.coax).
Here are its types:

<!-- checked-source: examples/lib/ch15_request_policy.coax -->
```jai
RequestPhase :: enum u8 { idle; running; succeeded; failed; cancelled; expired; }
Request :: struct { epoch: u32; phase: RequestPhase; deadline: u64; value: int; }
```

There are four commands:

| Command | Rule |
| --- | --- |
| `request_begin(state, now, deadline)` | Refuse an already running slot, a deadline that has arrived, or an exhausted epoch; otherwise start the next epoch with an empty result |
| `request_cancel(state)` | Change a running request to cancelled |
| `request_tick(state, now)` | Expire a running request at or after its deadline |
| `request_complete(state, epoch, now, ok, value)` | Ignore a stale or terminal completion; enforce the deadline; then record success or failure |

All procedures return a new state. The caller stores it. The clock is an
input: the example uses integer ticks and never reads a clock itself.
The host must use one monotonic time base and units consistently. A wall
clock correction must not unexpectedly extend an operation.

Start with the behavior you need to preserve. The source writes its
checks before the implementations, including this trace:

```jai
first := request_begin(Request.{}, 10, 20);
second := request_begin(request_cancel(first), 11, 30);
stale := request_complete(second, first.epoch, 12, true, 99);
#assert stale.phase == RequestPhase.running && stale.value == 0;
done := request_complete(stale, second.epoch, 13, true, 42);
#assert done.phase == RequestPhase.succeeded && done.value == 42;
```

The old operation completed successfully, but it no longer owns the slot.
Its `99` must never replace the new operation's result.

The implementations are short. Completion is the important one:

<!-- checked-source: examples/lib/ch15_request_policy.coax -->
```jai
request_complete :: (s: Request, epoch: u32, now: u64, ok: bool, value: int) -> Request #export_c {
    if s.phase != RequestPhase.running || epoch != s.epoch { return s; }
    r := request_tick(s, now);
    if r.phase != RequestPhase.running { return r; }
    if ok { r.phase = RequestPhase.succeeded; r.value = value; }
    else { r.phase = RequestPhase.failed; }
    return r;
}
```

A completion exactly at the deadline expires. Cancelling a completed
request preserves its result. A duplicate completion does too. Epochs
never wrap: after `4294967295` starts, this slot refuses further starts.
A real long-lived service can use a wider identity or replace the slot
under a separately checked lifetime rule.

Run it from the repository root:

```shell
coax/bin/coax test coax/book/examples/ch15_requests.coax
coax/bin/coax mutate coax/book/examples/ch15_requests.coax
coax/bin/coax run coax/book/examples/ch15_requests.coax
```

The authored version has seven checks and kills all 27 mutants. Its output
is:

```jai
old completion: epoch 2, phase 1
current completion: epoch 2, result 42
deadline: expired true
```

Phase 1 is `running`; the stale callback left the current request alone.
These counts describe this example, not a guarantee that every request
state machine needs this many checks.

## What this does not cancel

The state transition does not stop an operating-system task. The host must
request cancellation through the actual API, then still tolerate a late
callback. Likewise, expiring a payment or remote command locally does not
undo an effect that already happened elsewhere.

For externally visible effects, use the remote system's idempotency and
reconciliation contract. A local epoch distinguishes callbacks; it is
not an authorization token, a globally unique job ID, or proof that a
server did no work. Do not automatically retry a send merely because the
local timer expired. The wallet in Chapter 22 treats unknown outcomes
explicitly for this reason.

A timer must call `request_tick` even if no completion ever arrives. The
machine has no autonomous wake-up. Use the same serialized queue for
commands, timer events and callbacks. `#authority` checks logical traces;
it does not make unsynchronized native reads and writes safe.

## Cancellation and memory are separate

It is tempting to free a request's buffers when its state becomes
cancelled. That is safe only if no platform operation can still access
them. An epoch prevents a stale callback from changing state; it cannot
make a freed address valid.

For every outstanding native task, retain its input bytes and any output
storage until the API confirms that access has ended. Then deliver a
completion event or discard its result, and release the storage. Capture
an owned operation record or stable handle in a callback, rather than a
view into a screen's temporary array.

The same rule applies to generated codecs. A decoded string or byte view
borrows the original packet. Returning the view from a callback does not
retain the packet. Either keep the packet with the view or copy the fields
you need into bounded resident state before the callback returns.

Avoid keeping every historical packet merely to keep its views valid.
Bound the number of outstanding operations and their bytes. Refuse or
queue additional work deliberately. For a stream, coalesce replaceable
progress events, preserve terminal results, and put a bound on the queue.
Dropping arbitrary chunks of a protocol stream can corrupt its meaning;
backpressure belongs in the stream's actual contract.

## The actual System One request

The small request machine above isolates the rule. The production
[System One adapter](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/systemone/outcomes.coax)
applies it to the actual polling transport used by the desktop Lab. The C host
spawns, polls, cancels and reaps the child. Coax owns admission, generations,
terminal classification and copying results before wire-buffer reuse.

| Operation | Handled result |
| --- | --- |
| `so_request_start` | `Result(int,SoStartFailure)`; a positive ticket owns pending work or an immediate terminal setup failure |
| `so_request_poll` | Exhaustive `pending`, `ready` or `refused` selection; a stale ticket cannot touch newer work |
| `so_request_snapshot` | `Result(SoTerminal,SoSnapshotFailure)`; pending, stale or insufficient-capacity refusal preserves storage |
| `so_request_cancel` | A handled Result; terminal cancellation leaves a result available for snapshot |
| `so_request_discard` | A handled Result; pending work is cancelled and retired, or an unread terminal result is discarded |

A successful snapshot is a tagged terminal outcome. `answered` still needs
the application's response validation. `refused` retains the response and
diagnostics. `cancelled` records local cancellation. `not_connected` is the
transport's specific no-connection observation, used for the Lab's existing
alternate door. `unknown` includes outcomes such as a lost response or timeout;
it never authorizes automatic retry. None proves that a remote server undid
work. Snapshot payloads borrow the separate caller buffers described in
[Chapter 6](ch06-memory.md).

The actual Lab handles the start Result, exhaustively polls, takes a snapshot,
validates the answer and copies its displayed details into the transcript.
Stop and quit explicitly handle discard. A ready result prevents a new start
until taken or discarded. The generation counter refuses exhaustion instead
of wrapping. Keep one canonical owner at a stable address and serialize all
access; this API does not prohibit arbitrary copies of the owner value.

The [checked typed fixture](https://github.com/OpenAgentsInc/openagents/blob/main/coax/tests/systemone-outcomes.coax)
loads the adapter's authored checks for every terminal alternative,
pending/stale/capacity refusals and detached snapshots. Its main drives real
request operations against the fixture child. Run its Coax checks:

```shell
coax/bin/coax test coax/tests/systemone-outcomes.coax
```

To run that same source against the real native process host, with an owned
offline child instead of a server:

```shell
python3 coax/tests/systemone-outcomes.py --compiler coax/bin/coax --out /tmp/book-systemone
cat /tmp/book-systemone/O0-run.stdout
```

The literal transcript is checked at O0, O2 and ASan/UBSan:

```jai
answered 200 reply-body
refused 503 refused-body
cancelled 0 cancelled
not connected 0 the server is unreachable
unknown 0 the request timed out
```

The fixture child supplies independent bytes and exit statuses. The Coax
transport interprets them; the oracle compares the whole transcript and the
consumer checks separately verify retained text after wire reuse. Use a fresh
output directory to preserve failures.

Then try the intentional mistakes separately:

```shell
coax/bin/coax check coax/tests/systemone-outcomes/ignored-start.coax --library
coax/bin/coax check coax/tests/systemone-outcomes/incomplete-terminal.coax --library
coax/bin/coax check coax/tests/systemone-outcomes/local-snapshot.coax --library
```

These are expected refusals: ignoring a Result, omitting a terminal alternative,
and returning a view of local snapshot storage. Giving a Result a name does not
handle it. Exhaustive handling does not extend the lifetime of its payload.
The observed diagnostics are `E0907` at line 2, `E0904` at line 3 and `E0383`
at line 4, respectively; the runner checks those codes and source locations.
The [qualification record](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/coax-outcomes-12625-2026-11-02/README.md)
retains actual diagnostics and offline native request traces, including HTTP
refusal, timeout, cancellation, child failure and wire reuse. It states the
scope of the actual Lab consumer checks separately from whole-desktop build
and device readiness.

## GPU work is asynchronous too

Returning from a draw call does not mean the GPU has finished reading its
buffers. The platform owns that lifetime. Retain buffers and textures
until the command buffer completes; reuse a frame's scratch slot only
after its completion or fence. If you use a ring of in-flight frames,
associate each slot with the command that releases it.

A Coax bounds gate can prove that a proposed draw fits an admitted byte
span. It cannot prove that Metal still owns a live allocation, that the
CPU did not overwrite it during execution, or that a native binding used
the correct stride. Exercise those cases in the native adapter and GPU
validation tools as well.

## Backgrounding and recovery

A screen disappearing, an app suspending and a process dying are different
events. Make each subsystem's policy explicit:

- Does leaving the screen cancel the operation or merely stop observing it?
- Which operations can legitimately continue in the background?
- What bounded state must be persisted before suspension?
- After relaunch, which remote outcomes must be queried before retrying?

Do not persist a raw `Request` struct, clock deadline, pointer or borrowed
view. Use a versioned protocol record for durable state, and reconcile it
with the external effect. A monotonic clock value from an old process may
not be meaningful in the next one. The example's epoch is process-local;
a new process must not accept callbacks or jobs from an old session under
a coincidentally reused identity.

Chapter 24 links this machine into a native host, writes actual protobuf
bytes to a file and checks the result. It deliberately does not claim a
background task service or a network scheduler.
