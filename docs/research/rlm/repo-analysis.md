# RLM Repository Analysis (`alexzhang13/rlm`)

**STATUS: Point-in-time research record (2026-07-21).** Based on local clone
commit `72d6940142ddfb84ee6be573dc999a37e633e671`. Not product authority.

## Overview

| Field | Detail |
| --- | --- |
| Package | `rlms` 0.1.3 on PyPI (`pip install rlms`) |
| Role | Inference engine + sandbox adapters + training harness + trajectory UI |
| Language | Python 3.11+ |
| License | MIT |
| Maintainer framing | MIT OASYS lab / paper authors |
| Approx. library size | ~9.5k lines under `rlm/` (core + clients + environments + utils + logger) |
| Tests | 18 `test_*.py` modules under `tests/` |
| Tooling | `uv`, `ruff`, `pytest`, pre-commit, GitHub Actions (style, test, docs, publish) |

The public pitch matches the paper: replace `llm.completion(prompt, model)` with
`rlm.completion(prompt, model)`. The long prompt becomes a REPL variable. The
model writes ```repl``` code blocks. Code may call `llm_query` / `rlm_query`
(and batched variants).

## Layout

```
rlm/                     # installable package
  core/                  # RLM loop, LMHandler, types, TCP framing
  clients/               # OpenAI, Anthropic, Gemini, Portkey, Azure, …
  environments/          # local, ipython, docker, modal, prime, daytona, e2b
  utils/                 # prompts, parsing, tokens, exceptions
  logger/                # trajectory logging + rich verbose printer
training/                # verifiers + prime-rl harness (separate pyproject)
  src/rlm_train/         # RLMTrainEnv, rubric, sub-LM proxy, subprocess REPL
  environments/oolong/   # example long-context training env
  configs/               # example TOML
docs/                    # Next.js docs site sources + architecture.md
examples/                # quickstart and environment demos
tests/                   # unit + integration tests
visualizer/              # Next.js + shadcn trajectory viewer
```

## Core runtime architecture

Three cooperating pieces per `completion()` (from `docs/architecture.md` and
`rlm/core/rlm.py`):

1. **`RLM`** — iteration controller.
2. **`LMHandler`** — localhost TCP server that routes LM API calls for the
   root and for code running in the environment.
3. **Environment (`LocalREPL` or remote)** — executes model code. It holds
   `context`, tools, and `answer`.

### Why a TCP LM handler?

Isolated sandboxes (Docker, Modal, …) cannot import the host LM client. Model
code calls `llm_query` → opens a socket to the handler → handler selects a
backend client by `model` / `depth` → returns text. Local REPL uses the same
protocol for consistency.

Wire format: **4-byte big-endian length + UTF-8 JSON**
(`rlm/core/comms_utils.py`).

### Completion loop (simplified)

1. Spawn handler + environment. Inject `context` and optional history.
2. Build system prompt from `RLM_SYSTEM_PROMPT` + optional orchestrator addendum
   + tool descriptions + metadata about the context.
3. For each iteration until ready / limits:
   - Call root LM with history.
   - Parse ```repl``` fenced blocks.
   - Execute in environment.
   - Append truncated stdout / errors to history.
   - If `answer["ready"]` is true, return `answer["content"]`.
4. Tear down handler and environment (unless `persistent=True`).

### Recursion depth

```
depth 0 (user root)
  rlm_query → depth 1 child RLM (own handler + REPL)
    rlm_query → …
      when next_depth >= max_depth → plain LM completion (leaf, no REPL)
```

- `llm_query` is always a plain one-shot LM call.
- `rlm_query` creates a child RLM when depth allows. Otherwise it falls back to
  `llm_query`.
- Remaining budget / timeout / tokens are passed to children (not full parent
  allotments).

Default constructor values of note (`RLM.__init__`):

| Knob | Default | Meaning |
| --- | --- | --- |
| `max_depth` | 1 | One level of recursive RLM / then leaf LM |
| `max_iterations` | 30 | Root (or child) loop cap |
| `max_budget` | None | USD stop (needs cost-tracking backend) |
| `max_timeout` | None | Wall-clock stop |
| `max_tokens` | None | Aggregate token stop |
| `max_errors` | None | Consecutive error stop |
| `max_concurrent_subcalls` | 4 | Parallelism for batched RLM subcalls |
| `compaction` | False | Optional root-history summarization path |
| `persistent` | False | Reuse env across `completion()` turns |
| `orchestrator` | True | Extra system-prompt steering |

On limit exceed, the engine prefers returning a **best partial answer** when
available (`BudgetExceededError`, `TimeoutExceededError`, … in
`rlm/utils/exceptions.py`).

## Prompt contract

Current default system prompt (`rlm/utils/prompts.py`, `RLM_SYSTEM_PROMPT`) is
much shorter than an older deprecated mega-prompt still kept in-file for
reference. It teaches:

- REPL tools: `context`, `llm_query`, `llm_query_batched`, `rlm_query`,
  `rlm_query_batched`, `SHOW_VARS`, `answer`.
- Truncation of long stdout (~20K chars).
- Rule: only `print(...)` is visible. Bare expressions are discarded.
- Strategy: probe first. Do not finalize on turn 1 without inspection.

Optional **`ORCHESTRATOR_ADDENDUM`** (enabled by default) pushes the model to:

- Plan decomposition before heavy execution.
- Keep long text out of the root window.
- Prefer fat prompts + modest batches (~100K chars / prompt, ~20 prompts /
  batch heuristics in the prompt text) over tiny mega-batches.
- Act as orchestrator, not sole solver.

This is an engineering distillation of the paper’s qualitative findings
(decomposition sensitivity, cost of bad first plans).

**Answer protocol in code:** set `answer["content"]` and `answer["ready"] =
True` inside a REPL block. (Older paper / trajectories mention `FINAL` /
`FINAL_VAR` tags. The training appendix discusses cleaning those mistakes.)

## Environments

| Name | Isolation | Notes |
| --- | --- | --- |
| `local` | Same process `exec` | Default. Soft sandbox (dangerous builtins removed). Shared venv |
| `ipython` | In-process or ipykernel subprocess | Optional extra. Cell timeouts in subprocess mode |
| `docker` | Container | Default image `python:3.11-slim`. Host proxy for LM. Persistence and parallel subcalls supported |
| `modal` | Modal sandbox | Cloud isolation |
| `prime` | Prime Intellect sandboxes | Beta. Authors note slow runtime |
| `daytona` | Daytona sandboxes | Optional extra |
| `e2b` | E2B code interpreter | Optional extra |

### LocalREPL details that matter

- Namespace persistence across iterations (true REPL).
- Scaffold restoration after each exec so models cannot permanently clobber
  `llm_query` / `context` / `answer`.
- Reserved tool names blocked from custom tool overrides.
- Custom tools as callables (local) or serializable / code strings (isolated).
- Batched LM failures are **per-prompt soft errors** (one failure does not kill
  the whole batch list alignment).

**Security:** local mode is explicitly not a production multi-tenant boundary.
Isolated environments exist for that class of risk. The host still brokers LM calls.

## Clients

`ClientBackend` literals include: `openai`, `portkey`, `openrouter`, `vercel`,
`vllm`, `anthropic`, `azure_openai`, `gemini`.

Depth-based routing: root client vs `other_backends[0]` for recursive depth
(currently only **one** additional backend supported).

Sampling args can be split: `sampling_args` for root, `sub_sampling_args` for
depth-1 clients.

## Training package

`training/` is a separate install surface that plugs `rlm.RLM` into
Prime Intellect’s stack:

- `RLMTrainEnv` — `verifiers.MultiTurnEnv` mirroring depth=1 completion.
- Subprocess-isolated REPL backend (safer than in-process during RL).
- Sub-LM proxy so training inference servers answer `llm_query` from workers.
- Example environment: **OOLONG** long-context QA.
- Example config: `configs/rlm-qwen3-30b-example.toml`.
- Documented HF artifact:
  `mit-oasys/rlm-qwen3-30b-a3b-v0.1`.

This is the open path for the paper’s “train native RLMs” agenda. It is still
thin compared to a full multi-domain recipe, but it is real code, not a sketch.

## Observability

- **`RLMLogger`** — structured trajectory / iteration metadata.
- **Rich verbose printer** — live console view.
- **Callbacks** — `on_subcall_*`, `on_iteration_*` for custom UIs.
- **`visualizer/`** — Next.js app to inspect logged trajectories (shadcn UI).
- Nested metadata: child RLM trajectories attach under parent REPL results
  (`rlm_calls`).

## Tests and quality bar

Tests cover parsing, local REPL, persistence, Docker robustness, depth
metadata, subcalls, `rlm_query`, IPython, multi-turn integration, clients, and
types. CI workflows: style, test, docs, publish.

`AGENTS.md` encodes contributor norms: ruff, fail-fast errors, minimal new
core dependencies, optional extras for sandboxes.

## Dependence map (product view)

```
User / trainer
    │
    ▼
RLM.completion
    ├── LMHandler ──► API providers / vLLM
    └── Environment
            ├── exec / sandbox code
            ├── llm_query*  ──► handler
            └── rlm_query*  ──► child RLM …
```

No Effect/TypeScript runtime. No OpenAgents-specific authority model. Pure
Python research/production hybrid library.

## Gaps and risks relative to the paper

| Paper claim / need | Repo status at pin |
| --- | --- |
| Algorithm 1 REPL + recursive subcalls | Implemented |
| Depth 0–3 style control | `max_depth` + leaf fallback |
| Cost control | Budget/token/timeout/error caps. Needs provider cost data for USD |
| Async subcalls (paper future work) | Batched concurrency exists. Much of the loop is still turn-serial |
| Hard multi-tenant safety | Depends on choosing docker/modal/… not local |
| Native trained models | Training harness + example HF model. Not a full public SOTA recipe |
| FINAL tag protocol from older writeups | Migrated toward `answer` dict. Training docs still mention cleanup of old tags |
| Multi other-backends | Explicitly limited to one extra backend today |

## Fit as a library (engineering judgment)

**Strengths**

- Faithful implementation of the paper’s central invariant.
- Practical knobs (depth, budgets, persistence, compaction, custom tools).
- Multiple isolation backends.
- Training hook into an active RL stack (`prime-rl` / verifiers).
- Clear architecture docs and agent contributor guide.

**Weaknesses / costs**

- Python-only. Not embeddable in OpenAgents Effect workers.
- Local default is unsafe for hostile prompts.
- Operational complexity: TCP handler lifecycle, sandbox credentials, cost
  variance.
- Prompt/orchestrator text is long and model-sensitive (paper’s negative result).
- Still beta (0.1.x). APIs and prompt contracts can move.

## Minimal usage sketch (from README)

```python
from rlm import RLM

rlm = RLM(
    backend="openai",
    backend_kwargs={"model_name": "gpt-5-nano"},
    verbose=True,
)
print(rlm.completion("Print me the first 100 powers of two, each on a newline.").response)
```

Isolated variant:

```python
rlm = RLM(environment="docker")  # or modal / prime / daytona / e2b
```

## Bottom line on the repo

`alexzhang13/rlm` is a **serious, usable reference implementation** of the RLM
paper: not a toy notebook, not only a blog demo. For OpenAgents it is best
treated as:

1. A **leaf long-context executor** behind sandboxed capacity (Pylon / Cloud /
   container), and
2. A **training environment source** if we ever post-train orchestrator models,

not as something to reimplement wholesale in TypeScript on the hot governance
path. That reading is consistent with the earlier decision audit
`docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`.
