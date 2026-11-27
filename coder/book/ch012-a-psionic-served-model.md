# 12. A Psionic-served model as an engine

> **Status: partly implemented.** Local Psionic serving now runs live in
> OpenAgents Desktop and `oa-worker` via **`psionic-inkling-serve`** on Apple
> Silicon **Metal 4** (FlashAttention, 110–150+ tok/s decode on M5 Max, 32K KV
> context, 21 GGML quantizations + LoRA adapters in `#14028`) and **CUDA**
> (RTX 4080 75.7+ tok/s decode / 634 tok/s prefill, Linux and Windows) as the
> `$0.00` local compute tier (`#14055`, `#14072`, `#14087`), alongside
> `coder run "TASK" --engine psionic` against any local Chat Completions
> server. Still to come: Pylon-served remote inference in the CLI plan,
> constrained tool-argument decoding in the server loop, and side-by-side
> prefix-cache ablations against hosted models on a shared held-out coding
> suite.

## Live local serving: `psionic-inkling-serve`, 32K tuning, and `$0` compute tier

Since the initial CLI stub below, Psionic's local model serving moved into
production for OpenAgents Desktop and `oa-worker`:

- **Metal 4 and CUDA native servers (`#13966`, `#13975`–`#13977`, `#14028`):**
  `psionic-inkling-serve` serves **Inkling 2 Mini** (and the 8-variant local
  catalog including AtomicChat Uncensored, OrcaRouter Abliterated, 21 GGML
  quantizations, and hot-swappable LoRA adapters, `#14028`) with a 32,768-token
  Q8S/Q8_0 KV cache, FlashAttention, post-decode KV cache retention
  (`0fe366840e`), and zero API keys or outbound traffic.
- **32K local budget tuning in `oa-worker` (`#14055`):** `coder/tui/oa-worker.coax`
  applies local-aware read/tool budgets (`FWW_LOCAL_TOOL_OUT_MAX = 8192`,
  `FWW_READ_LOCAL_LINES = 200`, `FWW_TURN_TOOLS_BUDGET_LOCAL = 16384`) and
  five-layer compaction so Inkling 2 Mini stays responsive inside 32K context.
- **Unified cloud-first onboarding + `$0.00` local tier (`#14072`, `#14087`):**
  Desktop onboarding lets users start immediately on OpenAgents Cloud and opt
  into local Inkling 2 Mini (`$0.00` enforced in Usage HUD, zero cloud tokens
  billed).

The CLI path below documents the original `--engine psionic` adapter shared
with legacy Microcoder.

Psionic serves models on this machine and on Pylons. This chapter runs
Microcoder on one and measures what owning both the harness and the server
buys.

## The engine

The psionic engine is Microcoder (chapter 9) with a different plan. Coder's
terminal UI and `coder run` send Microcoder's worker the task with
`"local": true`, and the worker builds a plan of one gateway
(`mk_plan_local` in
[microcoder_keys.coax](../../coax/lib/microcoder/keys.coax)):

- The base URL is `PSIONIC_BASE_URL`, or `http://127.0.0.1:8080`.
- The model is `PSIONIC_MODEL`, or `local`.
- The gateway is `gw_psionic` from
  [coax/lib/gateway/psionic.coax](../../coax/lib/gateway/psionic.coax): Chat
  Completions with no key. A local server makes no promise to hold a strict
  JSON schema, so the gateway carries `GW_NO_STRICT_SCHEMA` and Microcoder
  reads the action out of the reply's text.

Before each run the worker asks `GET /v1/models` with a one-second limit.
When nothing answers, the run ends `no_capacity` and says where it looked:

```text
› say ok
  └ failed · 0.1s · No model answered: No model server answers at http://127.0.0.1:8080/v1/models. Start one there (psionic, llama-server or any Chat Completions server), or set PSIONIC_BASE_URL.
```

The worker is the same process as the microcoder engine's, so switching
between them with `/engine psionic` and `/engine microcoder` keeps the
conversation.

## The check

[local_model.py](../../coder/tui/tests/local_model.py) is a stand-in server for
tests. It lists one model and answers with Microcoder's action as text:
first a command, `echo local-step`, then, once the state holds that
command, the reply `local-ok`. The offline smoke starts it, runs `coder run
--json` against it in a fresh `HOME`, and checks the log's lines in order:

```sh
python3 coder/tui/smoke.py
```

```text
ok   run --json exits 0
ok   run --json: session, user, tool, result, reply, end
ok   run --json: the local model answered
ok   ls --json lists the session
ok   log by ID prefix
ok   status of an unknown ID exits 6
```

## What remains

- **A Pylon as the server.** `coax/lib/gateway/pylon.coax` buys one free
  NIP-90 text job from a pinned Pylon. Its relay runtime (part b) isn't
  written, so it isn't in the plan yet.
- **The prefix cache.** Microcoder's state changes from step to step, so the
  request's prefix is stable only up to the facts. The measurement is the
  hit rate, prefill saved and time to first token across a long session,
  against Psionic's prefix cache (Psionic chapter 176).
- **Constrained tool arguments.** With the server in hand, the action schema
  can constrain generation (Psionic chapter 53), so a malformed action is
  never produced and `bad_replies` can't happen.
- **The comparison.** The same tasks on the local model and on Vertex Gemini
  Flash: task success, time and cost.

## Dependencies

- [Chapter 3](PLAN.md#i-the-loop) and [Chapter 9](ch009-microcoder.md).
- Psionic [chapter 53](../../psionic/book/ch053-choosing-the-next-token.md),
  [Psionic chapter 174](../../psionic/book/ch174-clients-streams-and-background-jobs.md) and
  [Psionic chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md).

## Done means

- [x] `coder run --engine psionic "TASK"` runs against a local server.
- [ ] Cache hit rate, prefill saved, time to first token, task success and
  cost are reported for the local model and a hosted one on the same tasks.

## Hybrid executor models (`hybrid-exec-v1`, W3.1 / W6.1)

In addition to plain local or Pylon inference, Microcoder and OpenAgents
Desktop run the live **hybrid executor** ([Psionic chapter 236](../../psionic/book/ch236-a-frozen-exact-core-with-a-learned-interface.md),
[`psionic/lib/executor-desktop.coax`](../../psionic/lib/executor-desktop.coax),
[`coax/lib/agent/desktop/executor-store.coax`](../../coax/lib/agent/desktop/executor-store.coax)):
the real 16.38 GB `Inkling-2-Mini` GGUF on Apple Silicon Metal 4 paired with a
hot-swappable rank-8 LoRA routing adapter that invokes frozen compiled weight
cores while reusing the KV cache (`11/48 → 48/48` on held-out tasks, `0` silent
hallucinations; [W3.1 verification record](../../docs/verification/executor-w31-hybrid-exec.md),
[W6.1 verification record](../../docs/verification/executor-w61-coding-agent.md)).

## Sources

- `coder/tui/oa-worker.coax`, `coder/tui/inky.coax`, `psionic/lib/executor-desktop.coax`.
- `coder/tui/microcoder_keys.coax` (`mk_plan_local`), `coder/tui/mcworker.coax`,
  `coder/tui/local_model.py`.
- `coax/lib/gateway/psionic.coax`, `coax/lib/gateway/pylon.coax`.
