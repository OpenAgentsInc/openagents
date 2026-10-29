# System One client for Coax

This library sends typed decision requests to [Baseten's decision models](https://docs.baseten.co/reference/inference-api/system-one),
**mammoth-1** (27B) and **elephant-1** (9B), and checks their answers. The
models are described in
[`docs/research/baselabs-decision-models/README.md`](../../../docs/research/baselabs-decision-models/README.md).
The library works with any host that speaks the same System One wire format,
including our own: Psionic's `/v1/systemone`
([chapter 16](../../../psionic/book/ch016-serve-systemone.md)), keyless on
loopback, answering as `SO_PSIONIC_ELEPHANT` at `SO_PSIONIC_URL`
(`SoClient.{url = SO_PSIONIC_URL, key_env = ""}`; a `key_env` of `""` sends no
Authorization header). The Mac app's Lab uses it.

```coax
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
| `request.coax` | The body builder: `so_begin`, `so_state_text` or `so_state_json` (an object, an array, or `{"messages": [...]}` with images), then `so_choice`, `so_noul` and `so_score`, then `so_finish`. It refuses a question shape the API would refuse. |
| `answers.coax` | `so_check` holds a response to the request that asked it: every question has an answer of its own type, and the probabilities form a distribution. Readers: `so_choice_of`, `so_noul_of`, `so_score_of`, `so_probability`, `so_usage`, `so_error_of`. |
| `transport.coax` | `so_post` and `so_decide` run `curl` (8.3 or later) as a child; `so_start` and `so_step` do the same without blocking, for a frame loop (`so_step(call, 0)` reads what has arrived), and `so_cancel` stops one. The key stays in the environment: curl expands it into the header, so it never appears in Coax memory, curl's arguments or a process listing. |
| `smoke.sh` | A live run against both models, plus a 401 and a 422 refusal, which cost no credit. |

```sh
coax/bin/coax test --library coax/lib/systemone/answers.coax   # unit checks, offline
coax/lib/systemone/smoke.sh                                    # live; reads ~/work/.secrets/baseten.env
```

Both models refuse requests over 16,384 tokens (`SO_CONTEXT_TOKENS`). A
choice's `confidence` measures how concentrated its distribution is. It is
not the probability of the chosen option, which is `SoChoice.probability`.
