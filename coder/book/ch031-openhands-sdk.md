# 31. OpenHands Software Agent SDK

> **Status: written.** Read at OpenHands/software-agent-sdk@98849457621881ae2414719b82b8bc1cf535b4ab on 2026-11-02.

Source: [`OpenHands/software-agent-sdk`](https://github.com/OpenHands/software-agent-sdk/tree/98849457621881ae2414719b82b8bc1cf535b4ab),
and its companion [`OpenHands/benchmarks`](https://github.com/OpenHands/benchmarks/tree/542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2)
at `542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2`, both cloned shallowly into
`projects/repos/` and read only. Nothing was installed or run. Paths below
are relative to the SDK unless they start with `benchmarks:`.

## Why study it

The OpenHands Software Agent SDK is the Python library and server behind the
OpenHands agent. It splits into four packages: `openhands-sdk` (agent,
conversation, events, LLM, condensers, security), `openhands-tools` (terminal,
file editor, browser, delegation and others), `openhands-workspace` (Docker,
Apptainer, cloud and remote workspaces) and `openhands-agent-server` (a REST
and WebSocket API) (`AGENTS.md`, "Cross-Repository Boundaries"). Its
evaluation lives in a separate repository that pins the SDK as a git submodule
(`benchmarks:README.md`).

Two designs matter to Coder. First, everything the agent does is a typed,
serialized event in a persistent log, and condensing is itself an event that
names what it replaced (`openhands/sdk/event/`, `openhands/sdk/conversation/event_store.py`).
That is [chapter 2](ch002-model-visible-means-logged.md) and
[chapter 7](ch007-condense-the-history.md). Second, the project keeps a strict
line between fast tests that run on every change and slow tests that call real
models, which run only on request (`tests/README.md`). That is the line
[chapter 21](ch021-measure-the-harness.md) draws between checks and
measurements.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| Typed events: actions, observations, messages, system prompt, condensation, errors | `openhands-sdk/openhands/sdk/event/` | [2](ch002-model-visible-means-logged.md) |
| A persistent event log, one JSON file per event, appended under a lock | `openhands-sdk/openhands/sdk/conversation/event_store.py` (`EventLog`) | [2](ch002-model-visible-means-logged.md) |
| Tools as typed action and observation pairs with an executor | `openhands-sdk/openhands/sdk/tool/tool.py` (`ToolDefinition`, `ToolExecutor`) | [4](ch004-tools-as-typed-capabilities.md) |
| Parallel tool calls serialized per shared resource | `openhands-sdk/openhands/sdk/agent/parallel_executor.py` | [4](ch004-tools-as-typed-capabilities.md) |
| A condensation event that lists forgotten events and places a summary | `openhands-sdk/openhands/sdk/event/condenser.py` (`Condensation`) | [7](ch007-condense-the-history.md) |
| A summarizing condenser with a size limit and protected first events | `openhands-sdk/openhands/sdk/context/condenser/llm_summarizing_condenser.py` | [7](ch007-condense-the-history.md) |
| Risk levels on actions and confirmation policies | `openhands-sdk/openhands/sdk/security/risk.py`, `confirmation_policy.py` | [5](ch005-a-sandbox-and-approvals.md) |
| A stuck detector over five repetition patterns | `openhands-sdk/openhands/sdk/conversation/stuck_detector.py` | [5](ch005-a-sandbox-and-approvals.md) |
| A persistent terminal on tmux, with a subprocess fallback | `openhands-tools/openhands/tools/terminal/terminal/factory.py` | [5](ch005-a-sandbox-and-approvals.md), [30](ch030-mini-swe-agent.md) |
| Workspaces: Docker, Apptainer, cloud, remote API | `openhands-workspace/openhands/workspace/` | [5](ch005-a-sandbox-and-approvals.md), [20](ch020-an-agent-that-stays-online.md) |
| A scripted LLM for tests, including scripted errors | `openhands-sdk/openhands/sdk/testing/test_llm.py` (`TestLLM`) | [21](ch021-measure-the-harness.md) |
| Unit suites selected by changed paths; integration tests only by label | `tests/README.md`, `.github/workflows/tests.yml`, `integration-runner.yml` | [21](ch021-measure-the-harness.md) |
| Integration tests in four classes: task, behavior, condenser, API compliance | `tests/integration/README.md`, `tests/integration/tests/` | [21](ch021-measure-the-harness.md) |
| Golden fixtures for persisted settings, checked in CI | `tests/sdk/persisted_settings_baselines/`, `.github/workflows/persisted-settings-compat.yml` | [2](ch002-model-visible-means-logged.md) |
| Benchmarks in a separate repository with the SDK pinned as a submodule | `benchmarks:README.md` | [21](ch021-measure-the-harness.md) |
| Iterative evaluation with critics, and failure classes that decide retries | `benchmarks:benchmarks/utils/iterative.py`, `benchmarks:benchmarks/utils/failure_classifier.py` | [21](ch021-measure-the-harness.md), [22](ch022-generate-candidates-then-check-them.md) |

### Events and the log

Every model-visible item is an event class. `event/llm_convertible/` holds the
events that become model messages: `action.py`, `observation.py`,
`message.py` and `system.py`. Other events record condensation, conversation
state, errors, hooks, security decisions, streaming deltas and token counts
(`openhands-sdk/openhands/sdk/event/`). `EventLog` persists each event as its
own JSON file, named by its index, and takes a file lock for each `append`
so concurrent writers do not interleave (`conversation/event_store.py`).
Events are read back with `Event.model_validate_json`, so the log is the
conversation.

The repository treats serialized events and stored conversations as
compatibility surfaces. Changes to persisted settings need a schema version
bump, a migration step and a golden fixture of the old shape, and a CI check
builds an environment from an older PyPI release to load them (`AGENTS.md`,
"Repository Memory"; `tests/sdk/persisted_settings_baselines/`).

### Tools and steps

`ToolDefinition[ActionT, ObservationT]` is a frozen Pydantic model with an
action schema, an observation schema and a `ToolExecutor`. It validates input
before running, coerces output to the observation model when one is defined,
and can export itself as an MCP tool description (`tool/tool.py`). When a model
returns several tool calls, `parallel_executor.py` runs them on threads up to a
per-agent limit; tools that touch the same resource (a file, the terminal
session, the browser) are serialized through `ResourceLockManager`, based on
each tool's `declared_resources()`.

The terminal tool keeps a session. `factory.py` picks a tmux session when tmux
is available and a subprocess terminal otherwise, and `terminal_session.py`
strips the echoed command from the output. This is the opposite choice from
[mini-SWE-agent](ch030-mini-swe-agent.md), which runs each command in a new
process.

### Condensing

`Condensation` is an event with `forgotten_event_ids`, an optional `summary`
and a `summary_offset`. Applying it to a list of events removes the forgotten
ones and inserts the summary at the offset (`event/condenser.py`). A
`CondensationRequest` event asks for one. `LLMSummarizingCondenser` condenses
when the view passes `max_size` events (240 by default) or a token limit, and
never touches the first `keep_first` events (2 by default). It also has a hard
context reset: when that summarization fails, it shrinks each event's text by
a factor of 0.8 and tries again, up to 5 attempts
(`context/condenser/llm_summarizing_condenser.py`). Both the condenser and the
original events stay in the log.

### Safety and stuck runs

Each action carries a `SecurityRisk`: `UNKNOWN`, `LOW`, `MEDIUM` or `HIGH`
(`security/risk.py`). A confirmation policy decides whether to ask the person:
`AlwaysConfirm`, `NeverConfirm`, or `ConfirmRisky` with a threshold of `HIGH`
and `confirm_unknown` true by default (`security/confirmation_policy.py`).
An action's `security_risk` field is predicted by the model when the LLM risk
analyzer is on (`event/llm_convertible/action.py`); `security/` also holds a
shell parser and other analyzers. `StuckDetector` scans the last 20
events for repeated action and observation cycles, repeated action and error
cycles, a monologue without user input, alternating patterns, and context
window errors (`conversation/stuck_detector.py`).

### Fast tests and slow tests

`tests/README.md` sets the split:

- **Unit suites** (`sdk/`, `tools/`, `agent_server/`, `workspace/`, `cross/`)
  run in CI when their package, their tests or a package they depend on
  changes. The README lists the paths that trigger each suite.
- **Integration tests** call real models. Code changes do not trigger them and
  they do not run on a schedule. They run when a maintainer adds the
  `integration-test`, `behavior-test`, `condenser-test` or
  `api-compliance-test` label to a pull request, or by manual dispatch.

The integration tests come in classes named by file prefix
(`tests/integration/README.md`, `tests/integration/tests/`). `t*` tests are
required tasks, such as fixing a typo or staging files with git. `b*` tests
check behavior, such as not starting to implement before understanding or not
over-verifying, and do not block a release; an LLM judge scores some of them
(`tests/integration/utils/llm_judge.py`). `c*` tests stress condensing on a
small set of models. `a*` tests send malformed histories, such as a tool call
without a result or a duplicated call ID, to each provider API to record how
it responds (`a01_unmatched_tool_use.py`). Success rates are kept separately
per class.

For unit tests, `TestLLM` replaces patching the LLM client. It is an `LLM` subclass that
returns scripted messages in order and can raise scripted errors, such as a
context-window error, at a chosen call (`testing/test_llm.py`).

### Evaluation in its own repository

`OpenHands/benchmarks` holds SWE-bench and its variants, GAIA, Commit0,
terminal-bench, OpenAgentSafety, ProgramBench and others under
`benchmarks:benchmarks/`. It pins the SDK as a submodule in
`vendor/software-agent-sdk/`, so a result is tied to an exact SDK commit
(`benchmarks:README.md`). `iterative.py` runs an instance more than once and
uses an SDK critic to decide whether it succeeded. `failure_classifier.py`
sorts failures into categories so the retry loop adds CPU and memory only
for failures that look resource-related; its docstring says that before this,
every retry doubled resources, although most failures were protocol, model or
session errors. In the SDK, a `run-eval-N` label (1, 50, 200 or 500) on a
pull request starts an evaluation of that many instances
(`.github/workflows/run-eval.yml`).

## What Coder takes

- **Condensing as a logged event that names what it replaced.** This is
  already [chapter 7](ch007-condense-the-history.md)'s design: a begin event
  that names each replaced event by digest, a summary, and a verifier that
  rebuilds requests from the log. OpenHands confirms the shape at scale. Coder
  keeps its stricter rule that each replaced event is named by digest, not
  only by ID.
- **Compatibility fixtures for the log.** Old logs must keep loading. Coder
  takes the practice of a golden fixture per log format version and a check
  that loads every old fixture, which fits [chapter 2](ch002-model-visible-means-logged.md)'s
  versioned formats with readers for old ones.
- **The fast and slow split, enforced by the trigger.** Checks that run on
  every change use scripted models and recorded fixtures, as Microcoder's
  `McScript` does in [loop.coax](../../coax/lib/microcoder/loop.coax) and the
  Codex driver's recorded sessions do in [`coax/lib/agent/codex/`](../../coax/lib/agent/codex/tests.coax).
  Runs against real models are measurements: they run when asked and report
  numbers, and never block an unrelated change. Coder keeps them in
  `coder bench SUITE` ([chapter 23](ch023-openagents-coder.md)).
- **Separate rates for required tasks and behavior.** Coder reports
  "did the task pass" apart from "did the agent behave well". A behavior check
  that fails does not change the task pass rate.
- **API compliance as data.** The `a*` tests record how each provider answers
  a malformed history. Coder's gateways ([docs/coder/microcoder.md](../../docs/coder/microcoder.md#part-2-the-gateways))
  handle several providers; a recorded fixture per provider and per malformed
  case belongs in their offline tests.
- **Classify failures before retrying.** A suite runner that retries should
  know why a run failed. Chapter 21 records an infrastructure failure apart
  from a task failure, so that retries and verdicts do not mix them.

## What Coder leaves, and why

- **A persistent tmux terminal.** Microcoder runs each command in its own
  process and kills the process group at the deadline
  ([tools.coax](../../coax/lib/microcoder/tools.coax)). That keeps every step
  replayable from the log, with no hidden shell state. A persistent session
  is a change chapter 21 can measure, not the default.
- **The model's own risk label as an input to approval.** Coder's approvals
  come from policy and confinement ([chapter 5](ch005-a-sandbox-and-approvals.md)),
  and [chapter 6](ch006-a-document-tries-to-become-the-system-prompt.md) tests
  what a hostile document can make the model say. A risk label written by the
  model is not trusted to lower a check.
- **An LLM judge as a gate.** Coder uses deterministic checks for pass or fail.
  A judge can describe behavior in a report, but its score does not decide a
  verdict.
- **An SDK in Python with a REST server.** Coder's harness is Coax, and the
  host does processes, files and network ([plan](PLAN.md#rules-for-every-chapter)).
  Other agents drive Coder through the CLI and the same verbs as MCP tools
  ([chapter 23](ch023-openagents-coder.md)), not through a separate server API.
- **Resource escalation on retry.** Coder's sandboxes run on the person's
  machine or OpenAgents' own cloud with budgets the person sets; a runner that
  silently grows resources would change the cost of a result. A retry runs with
  the same resources, and the receipt records it.

## Where a borrowed design is measured

- **Condensing.** [Chapter 7](ch007-condense-the-history.md) measures condensing
  schedules and their prefill cost. OpenHands' defaults (condense past 240
  events, keep the first 2) are one schedule to add, and its condenser stress
  cases (`c01` to `c05`) suggest long-session tasks for the held-out set.
- **Parallel tool calls.** [Chapter 21](ch021-measure-the-harness.md) can
  compare sequential tool calls with parallel calls serialized per resource,
  measuring wall time and task success.
- **Stuck detection.** [Chapter 5](ch005-a-sandbox-and-approvals.md)'s guard
  against repeated failing calls covers one of the five patterns. Chapter 21
  measures whether the other four end failing runs earlier without ending
  good ones.
- **A persistent shell.** Chapter 21 runs the same suite with Microcoder's
  independent commands and with a persistent session, and compares steps,
  timeouts and success. [Chapter 30](ch030-mini-swe-agent.md) records the
  case against the persistent shell.

## Sources

SDK at `98849457621881ae2414719b82b8bc1cf535b4ab`:

- [`AGENTS.md`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/AGENTS.md)
- [`openhands-sdk/openhands/sdk/event/`](https://github.com/OpenHands/software-agent-sdk/tree/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/event)
- [`openhands-sdk/openhands/sdk/event/condenser.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/event/condenser.py)
- [`openhands-sdk/openhands/sdk/conversation/event_store.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/conversation/event_store.py)
- [`openhands-sdk/openhands/sdk/conversation/stuck_detector.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/conversation/stuck_detector.py)
- [`openhands-sdk/openhands/sdk/tool/tool.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/tool/tool.py)
- [`openhands-sdk/openhands/sdk/agent/parallel_executor.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/agent/parallel_executor.py)
- [`openhands-sdk/openhands/sdk/context/condenser/llm_summarizing_condenser.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/context/condenser/llm_summarizing_condenser.py)
- [`openhands-sdk/openhands/sdk/security/risk.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/security/risk.py)
- [`openhands-sdk/openhands/sdk/security/confirmation_policy.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/security/confirmation_policy.py)
- [`openhands-sdk/openhands/sdk/testing/test_llm.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-sdk/openhands/sdk/testing/test_llm.py)
- [`openhands-tools/openhands/tools/terminal/terminal/factory.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-tools/openhands/tools/terminal/terminal/factory.py)
- [`openhands-tools/openhands/tools/terminal/terminal/terminal_session.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-tools/openhands/tools/terminal/terminal/terminal_session.py)
- [`openhands-workspace/openhands/workspace/`](https://github.com/OpenHands/software-agent-sdk/tree/98849457621881ae2414719b82b8bc1cf535b4ab/openhands-workspace/openhands/workspace)
- [`tests/README.md`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/tests/README.md)
- [`tests/integration/README.md`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/tests/integration/README.md)
- [`tests/integration/tests/`](https://github.com/OpenHands/software-agent-sdk/tree/98849457621881ae2414719b82b8bc1cf535b4ab/tests/integration/tests)
- [`tests/integration/utils/llm_judge.py`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/tests/integration/utils/llm_judge.py)
- [`tests/sdk/persisted_settings_baselines/`](https://github.com/OpenHands/software-agent-sdk/tree/98849457621881ae2414719b82b8bc1cf535b4ab/tests/sdk/persisted_settings_baselines)
- [`.github/workflows/run-eval.yml`](https://github.com/OpenHands/software-agent-sdk/blob/98849457621881ae2414719b82b8bc1cf535b4ab/.github/workflows/run-eval.yml)

Benchmarks at `542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2`:

- [`README.md`](https://github.com/OpenHands/benchmarks/blob/542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2/README.md)
- [`benchmarks/utils/iterative.py`](https://github.com/OpenHands/benchmarks/blob/542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2/benchmarks/utils/iterative.py)
- [`benchmarks/utils/failure_classifier.py`](https://github.com/OpenHands/benchmarks/blob/542cb71c860c11d6a70238a1dd4b4ac7a3e0d4d2/benchmarks/utils/failure_classifier.py)

Coder: [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax),
[`tools.coax`](../../coax/lib/microcoder/tools.coax),
[`coax/lib/agent/codex/tests.coax`](../../coax/lib/agent/codex/tests.coax),
[docs/coder/microcoder.md](../../docs/coder/microcoder.md).
[Chapter plan](PLAN.md), Part VII; the shape follows the
[DeepSeek Harness notes](../../psionic/docs/harness.md).
