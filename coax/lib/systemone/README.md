# System One client for Coax

This library sends typed decision requests to [Baseten's decision models](https://docs.baseten.co/reference/inference-api/system-one),
**mammoth-1** (27B) and **elephant-1** (9B), and checks their answers. The
models are described in
[`docs/research/baselabs-decision-models/README.md`](../../../docs/research/baselabs-decision-models/README.md).
The library works with any host that speaks the same System One wire format,
including our own: Psionic's `/v1/systemone`
([chapter 20](../../../psionic/book/ch020-serve-systemone.md)), keyless on
loopback, answering as `SO_PSIONIC_ELEPHANT` at `SO_PSIONIC_URL`
(`SoClient.{url = SO_PSIONIC_URL, key_env = ""}`; a `key_env` of `""` sends no
Authorization header). The Mac app's Lab uses it.

```jai
#load "coax/lib/systemone/systemone.coax";   // link coax/platform/process-native.c

buf: [4096] u8; out: [65536] u8; err: [4096] u8;
b: [1] SoBody;
so_begin(b[..], buf[..], SO_ELEPHANT, "default");
so_state_text(b[..], "I was charged twice for my subscription. Please refund the duplicate charge.");
so_noul(b[..], "refund", "Is the customer asking for a refund?");
c: SoClient;                                  // Baseten, key from BASETEN_API_KEY
r := so_decide(c, b[..], out[..], err[..]);
if r.problem.count == 0 && so_noul_of(r.body, "refund").value > 0.9 { ... }
```

| File | What |
| --- | --- |
| `request.coax` | The body builder: `so_begin`, `so_state_text` or `so_state_json` (an object, an array, or `{"messages": [...]}` with images), then `so_choice`, `so_noul` and `so_score`, then `so_body`. It refuses a question shape the API would refuse. |
| `answers.coax` | `so_check` holds a response to the request that asked it: every question has an answer of its own type, and the probabilities form a distribution. Readers: `so_choice_of`, `so_noul_of`, `so_score_of`, `so_probability`, `so_usage`, `so_error_of`. |
| `owner.coax`, `outcomes.coax` | The canonical generation owner and actual typed asynchronous API: start/poll/snapshot/cancel/discard, independent terminal snapshots, exhaustive outcomes. |
| `transport.coax` | `so_post` and `so_decide` run `curl` (8.3 or later) as a child; `so_start` and `so_step` do the same without blocking, for a frame loop (`so_step(call, 0)` reads what has arrived), and `so_cancel` stops one. An inherited `key_env` is expanded by curl without entering Coax memory. An explicit `key_value` is already in Coax memory and is copied synchronously into the child environment. Both paths keep it out of curl arguments and process listings. |
| `smoke.sh` | A live run against both models, plus a 401 and a 422 refusal, which cost no credit. |

```shell
coax/bin/coax test --library coax/lib/systemone/answers.coax   # unit checks, offline
coax/lib/systemone/smoke.sh                                    # live; reads ~/work/.secrets/baseten.env
```

Both models refuse requests over 16,384 tokens (`SO_CONTEXT_TOKENS`). A
choice's `confidence` measures how concentrated its distribution is. It is
not the probability of the chosen option, which is `SoChoice.probability`.

## Typed asynchronous requests

The Lab uses `SoOwnedCall` through `outcomes.coax`. Construct the canonical
owner once, keep its storage and generation for its whole lifetime, and exclude
concurrent access. Copy tickets; do not copy, reconstruct or transfer the owner.
A ticket belongs to that same owner storage. This is a runtime request protocol,
not compiler-enforced linearity, native-handle authentication or a pool-free API.

`so_request_start` returns `Result(int, SoStartFailure)`. Its successful ticket
owns a pending request or a ready setup failure. A ready result blocks a new
start until snapshot or discard. `so_request_poll` returns an exhaustive
`SoRequestPoll` (`pending`, `ready`, `refused`). Stale tickets have no process,
buffer or owner effect, including after reuse has advanced the generation.
Generation exhaustion refuses; it never wraps.

`so_request_snapshot` returns `Result(SoTerminal, SoSnapshotFailure)`. Supply
independent body, stderr and problem buffers, disjoint from each other, the
wire buffers and owner storage. All capacities and the ticket are checked
before any byte changes. Success copies every string and scalar, publishes the
whole result, then retires the owner. Every returned payload borrows these
caller snapshot buffers; keep them live through every use. Reusing the owner,
wire storage or process slot cannot revoke a completed snapshot.

`SoTerminal` distinguishes `answered`, `refused`, `cancelled`, `not_connected`
and `unknown`, each carrying the full `SoResult`. Answered means HTTP success;
validate the answer against the original questions with `so_check` before
applying it. Only the transport's explicit curl exit-7 observation becomes
`not_connected`. Other status-zero failures remain `unknown`, which gives no
permission to retry. The Lab retains its existing one-time door fallback only
for `not_connected`; it never retries an unknown outcome.

`so_request_cancel` returns a handled unit Result and keeps a terminal result
available to snapshot. `so_request_discard` cancels pending work or drops a ready
result and retires it. Both reject stale tickets without effects. The Lab's
Stop/quit paths handle discard explicitly. Its completion path snapshots into
local buffers, validates/formats/routes while those buffers live, and copies
all displayed strings into the application-life transcript pool.

The runnable offline source, literal transcript and refused misuses are in
[`coax/tests/systemone-outcomes.coax`](../../tests/systemone-outcomes.coax),
[`systemone-outcomes.out`](../../tests/systemone-outcomes.out) and
[`systemone-outcomes/`](../../tests/systemone-outcomes/). The bounded runner
compiles an owned fixture child and links the actual process host; it makes no
HTTP request:

```shell
python3 coax/tests/systemone-outcomes.py --compiler /path/to/coax --out /tmp/new-outcomes-proof
python3 apps/openagents-desktop/tests/lab-consumer.py --coax /path/to/coax --out /tmp/new-lab-proof
```

The [consumer record](../../../docs/verification/coax-request-consumer-2026-11-02/README.md)
keeps canonical-owner, typed transcript, actual Lab projection and full-application
scope separate. The desktop's selected storage lasts for the application; no
copied-owner transfer, foreign retention or general lifetime claim is added.

## Raw transport compatibility and buffers

Create a `SoCall` with Coax's declared defaults before its first use. A C host
allocating a containing record must invoke that record's actual generated
`coax_default_*` constructor after successful allocation and before attachment.
Zero-filled memory is insufficient: a fresh call's process handle is `-1`,
while process slot `0` can belong to a different live request. Construct a
root once; constructing a view descriptor does not initialize its pointee.

`so_start` admits only a nonempty call view whose handle is exactly `-1`.
An active handle (including `0`) or any other sentinel returns false before
changing the owner, results, buffer bytes or process/environment effects. That
refusal preserves the existing result; it does not install a new `r.problem`.
After admission, setup failures still return false with a new problem.
`so_step` and `so_cancel` require a nonempty initialized call view.
`so_step(call, wait_ms)` treats every nonpositive wait as a poll, including
when stdout has closed but the child has not exited. It returns pending and
retains ownership until exit is observed; only then is the result published.
Positive waits preserve the existing five-second finish wait. The
[focused polling record](../../../docs/verification/coax-request-polling-2026-11-01/README.md)
measures the changed path and keeps the full desktop analysis refusal separate.


A call has one owner. Keep its output/error storage alive; do not overwrite or
reuse it from another operation while the call runs or while borrowed results
are still in use. `SoResult`
copies descriptors, not their backing bytes. Completion, cancellation and
process-slot release do not invalidate copied descriptors or release caller
storage. Finish all those uses before reusing the buffers for another call.
The desktop's selected pool lasts for the application; it has no new pool-free
or ownership-transfer contract.

The [retained owner checks](../../../docs/verification/coax-request-owner-2026-11-01/README.md)
cover generated transport, real offline child processes and exact desktop
state/entry projections. Full desktop generation remains unqualified by this
slice. The [issue execution route](../../../docs/books/coax-outcomes-request-ownership.md)
keeps the remaining work in order.
The compiler's [B2/B3 boundary](../../verification-retained-stores.md#remaining-boundary)
remains explicit: runtime admission alone is not a proof of arbitrary backing
lifetimes, foreign retention, copied active handles or ownership transfer.
