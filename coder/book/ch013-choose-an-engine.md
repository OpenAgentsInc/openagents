# 13. Choose an engine

> **Status: partly implemented.** `coder run "TASK"` with no `--engine`
> asks System One (Baseten's mammoth-1) which engine should take the task,
> how hard it is and how much effort to give it, and runs the answer. On the
> labelled set, both halves route 22 of 22 on mammoth-1 (2026-11-02), with
> a mean probability of 0.95 on the held-out half. Still to come: the
> decision's effort and model passed on to the engine, the question asked
> through our Decisions API (Clef first) instead of straight to Baseten,
> and the terminal UI asking on every message instead of keeping the agent
> the person picked.

Coder picks an engine, a model and an effort for each task. The choice is a
typed decision from System One, not a keyword rule.

## The request

[coder/tui/route.coax](../../coder/tui/route.coax) sends the task and this
machine's environment as the state:

```json
{"message":"fix the failing test","environment":{"os":"macOS","app":"Coder in the terminal","agents":{"claude_code":{"installed":true},"codex":{"installed":false}},"local_model_server":false}}
```

An agent counts as installed when its CLI is in a `PATH` directory. A local
model server counts when `PSIONIC_BASE_URL` is set, so the route makes no
probe of its own.

It asks three typed questions:

| Question | Type | Answers |
| --- | --- | --- |
| `engine` | choice | `claude`, `codex`, `micro`, `psionic`, each with a label that says what it is for |
| `difficulty` | score | trivial, easy, moderate, hard, very hard |
| `effort` | choice | `low`, `medium`, `high` |

The labels split Claude Code and Codex by the job's shape: work that spans
files or needs understanding first goes to Claude Code, and quick contained
jobs go to Codex. They don't split by whether the task mentions code. A
plain question or small talk goes to Microcoder. The desktop Lab's router
(`apps/openagents-desktop/router.coax`) asks the same way; earlier labels
that split by "code" sent refactors and one-line edits near 50/50.

The key comes from `BASETEN_API_KEY`, or from `~/work/.secrets/baseten.env`
into curl's environment only (`SoClient.key_value` in
`coax/lib/systemone/transport.coax`). It never appears in Coder's own
environment, curl's arguments or a process listing.

## What runs

The decision is the session's second line, and the transcript says what
ran and why:

```text
› what is 17 times 23? Reply with only the number.
  session 20261102-070956-632
  Engine: micro (0.96) · trivial · effort low, chosen by System One
● 391
  └ finished · 3.7s
```

```json
{"kind":"route","ok":true,"engine":"micro","probability":0.936,"difficulty":0,"effort":"low","ran":"microcoder"}
```

Coder runs the decision only when it can:

- Below a probability of 0.5, the person's default runs: Claude Code when it
  is installed, else Microcoder.
- A pick that isn't installed runs the default instead.
- With no decision (no key, no network, a refused request), the default runs
  and the log says why.
- `--engine` always overrides the decision, and no question is asked.

`coder route "TASK"` prints the decision alone, with `--json` for scripts.

## Checks

```sh
coax test coder/tui/coder.coax           # the request's state and questions, the decision's text
python3 coder/tui/smoke.py               # offline: no key, so the default runs and the log says why
python3 coder/tui/route_eval.py          # live: the held-out half on mammoth-1
python3 coder/tui/route_eval.py train    # live: the tuning half
```

```text
held: 22/22 right on mammoth-1, mean probability of the right picks 0.95
train: 22/22 right on mammoth-1, mean probability of the right picks 0.92
```

[route-labels.tsv](../../coder/tui/route-labels.tsv) is the labelled set:
44 messages, 22 to tune and 22 held out, 16 for Claude Code, 16 for Codex
and 12 for Microcoder. It is the desktop Lab's set with its labels renamed.
No message in it needs the psionic engine, so that answer has no check yet.

## Dependencies

- [Chapters 9–12](PLAN.md#ii-engines): the engines.
- [Chapter 16](PLAN.md#iii-many-tasks): System One decisions as a tool.

## Done means

- [x] The held-out half routes at a recorded accuracy on a named decision
  model: 22/22 on mammoth-1, 2026-11-02.
- [x] `coder run "TASK"` with no `--engine` prints the decision and runs
  it.
- [ ] The decision's effort and model reach the engine.
- [ ] The question goes through the Decisions API, Clef first.

## Sources

- `coder/tui/route.coax`, `coder/tui/route-labels.tsv`, `coder/tui/route_eval.py`.
- `coax/lib/systemone/` (`request.coax`, `answers.coax`, `transport.coax`).
- `apps/openagents-desktop/router.coax` and `router-labels.tsv`.
