# 23. Asynchronous work and lifetimes

A phone app starts an operation, keeps drawing, and receives its result
later. The result may arrive after the person cancelled, opened another
screen, retried the operation or put the phone to sleep. Correct code
must account for those events even when the platform calls its completion
handler exactly once.

`coax0` has no async functions, promises, scheduler or cancellation runtime.
The job system designed in Chapter 12 is about parallel work and effects;
it does not supply those asynchronous I/O facilities. We can nevertheless
write and check the decision logic today: the host performs an operation,
and Coax decides what each command or completion means.

## A request is a state machine

Our example owns one request slot. The host serializes access to it on
one queue. A request can be idle, running, succeeded, failed, cancelled or
expired. Starting increments an epoch, which identifies this use of the
slot. A callback captures that epoch before the host starts the effect.

The complete runnable source is
[ch23_requests.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/examples/ch23_requests.coax).
The demo loads its checked policy from
[lib/ch23_request_policy.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/examples/lib/ch23_request_policy.coax).
Here are its types:

<!-- checked-source: examples/lib/ch23_request_policy.coax -->
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

<!-- checked-source: examples/lib/ch23_request_policy.coax -->
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

```sh
coax/bin/coax test coax/book/examples/ch23_requests.coax
coax/bin/coax mutate coax/book/examples/ch23_requests.coax
coax/bin/coax run coax/book/examples/ch23_requests.coax
```

The authored version has seven checks and kills all 27 mutants. Its output
is:

```text
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
local timer expired. The wallet in Chapter 19 treats unknown outcomes
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

Chapter 26 links this machine into a native host, writes actual protobuf
bytes to a file and checks the result. It deliberately does not claim a
background task service or a network scheduler.
