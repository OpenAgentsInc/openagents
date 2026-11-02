# 12. A Psionic-served model as an engine

> **Status: partly implemented.** `coder run "TASK" --engine psionic` runs
> Microcoder on a Chat Completions server on this machine: Psionic's, or
> any server with the same API. Nothing leaves the machine, and the run
> needs no key. The offline smoke runs it against a stand-in server. Still
> to come: a Pylon as the server, the prefix-cache measurements, tool
> arguments constrained during generation, and the comparison with a hosted
> model on the same tasks. Psionic's GPUs are off for budget, so no run on a
> real Psionic-served model is recorded yet.

Psionic serves models on this machine and on Pylons. This chapter runs
Microcoder on one and measures what owning both the harness and the server
buys.

## The engine

The psionic engine is Microcoder (chapter 9) with a different plan. Coder's
terminal UI and `coder run` send Microcoder's worker the task with
`"local": true`, and the worker builds a plan of one gateway
(`mk_plan_local` in
[microcoder_keys.coax](../../coder/tui/microcoder_keys.coax)):

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

[local_model.py](../../coder/tui/local_model.py) is a stand-in server for
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

## Sources

- `coder/tui/microcoder_keys.coax` (`mk_plan_local`), `coder/tui/mcworker.coax`,
  `coder/tui/local_model.py`.
- `coax/lib/gateway/psionic.coax`, `coax/lib/gateway/pylon.coax`.
