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
| `transport.coax` | `so_post` and `so_decide` run `curl` (8.3 or later) as a child; `so_start` and `so_step` do the same without blocking, for a frame loop (`so_step(call, 0)` reads what has arrived), and `so_cancel` stops one. The key stays in the environment: curl expands it into the header, so it never appears in Coax memory, curl's arguments or a process listing. |
| `smoke.sh` | A live run against both models, plus a 401 and a 422 refusal, which cost no credit. |

```shell
coax/bin/coax test --library coax/lib/systemone/answers.coax   # unit checks, offline
coax/lib/systemone/smoke.sh                                    # live; reads ~/work/.secrets/baseten.env
```

Both models refuse requests over 16,384 tokens (`SO_CONTEXT_TOKENS`). A
choice's `confidence` measures how concentrated its distribution is. It is
not the probability of the chosen option, which is `SoChoice.probability`.

## The asynchronous owner and its buffers

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
