# Repository journey and coding pilot

This pilot connects the Coder book's bounded experiments to the actual CLI.
The offline journey checks transitions deterministically; live tasks measure
whether an engine repairs source code. Neither is a security audit or a
claim of broad coding superiority. The current
[guarantee and authorization contract](../../coder/book/CONTRACT.md) states
what is enforced and which evidence remains engine-owned.

## Run the offline journey

From the repository root, use a qualified compiler and run the Coax harness:

```sh
export COAX=/absolute/path/to/coax
coder/tui/coder.sh bench journey --json
```

`openagents coder bench journey --json` uses the same suite. The direct
entry point is `coax/web/tool.sh coder/tests/repository.coax offline`.
The public suite has a three-minute outer bound.

The harness creates a disposable local Git repository and a fresh
`CODER_HOME`. The person's own Coder memory, plugins and Router are not
loaded. It invokes `coder/tui/coder.sh`, the same implementation the
`openagents coder` domain dispatches to. No model login, gateway response or
network service is needed. Echo executes explicit harness commands; it
does not pretend to be a coding model.

The journey records a failing visible Coax check, forks the conversation
with an edit, checks the repair against acceptance cases outside the fixture,
reads the log, and verifies the fork's submitted input using `coder replay`.
It stops a detached `sleep; touch` command before the marker write,
continues through a new fork, and saves a source-only `reviewed.diff` in a
local delivery sink. `UNRELATED.txt`, instructions and visible checks must
stay unchanged. The fixture's history must still contain its one baseline
commit, and no untracked file may hide an unintended edit.

The artifact directory is printed. It is kept for review, not automatically
pushed, published or erased. This checks a reviewed diff delivery, not a
live PR service, an interactive terminal resume, or recovery from every
possible power-loss window.

## Ten real coding tasks

`coder-repository-pilot-v1` freezes ten small cases. Each case has a task
specification, broken source, visible examples and an external acceptance
checker. Even-numbered cases are development; odd-numbered cases are held
out for this pilot. No prompt or engine settings are tuned using held-out
outcomes. Both partitions are public and intentionally simple; use a larger
fresh set for later generalization claims.

| Case | Split | Required repair |
| --- | --- | --- |
| 0 | Development | Absolute value with negative inputs |
| 1 | Held | Inclusive clamp with lower and upper boundaries |
| 2 | Development | Ceiling of half a nonnegative count |
| 3 | Held | Nonnegative modulo including negative inputs |
| 4 | Development | Inclusive triangular sum |
| 5 | Held | Quote calculation requiring a helper and source change |
| 6 | Development | Inclusive boiling threshold |
| 7 | Held | Count even values including zero |
| 8 | Development | Batch rounding and capacity helper repair |
| 9 | Held | Weekday wrap, diagnosis then actual conversation fork |

The harness first verifies that every original fixture fails and that a
known repair passes. The external checker covers the domain boundaries and
additional values beyond the three visible examples. It computes expected
values independently of the candidate implementation. The model receives
the specification and compiler command, not the final acceptance file.
Acceptance files are outside its checkout; this is withholding evaluation
feedback, not a filesystem sandbox against a malicious agent with host
access. All engine commands still obey their own configured permissions.

## Run live cases

One case per invocation keeps the command bounded. A repair gets a
120-second CLI deadline, with a 135-second outer process bound. Case 9 gets
a diagnostic turn and a repair turn with the same per-turn bounds for each
engine; its reported time includes both. No automatic repair retries or
manual interventions are added by the evaluator.

```sh
coax/web/tool.sh coder/tests/repository.coax run --engine micro --case 0 --out /absolute/path/micro.jsonl
coax/web/tool.sh coder/tests/repository.coax run --engine codex --case 0 --out /absolute/path/codex.jsonl
coax/web/tool.sh coder/tests/repository.coax run --engine claude --case 0 --out /absolute/path/claude.jsonl
coax/web/tool.sh coder/tests/repository.coax run --engine auto --case 0 --out /absolute/path/auto.jsonl
```

Run cases 0 through 9 in separate invocations. `psionic` is supported when a
local server is available. A provider failure is a recorded observation, not
a task pass; skip additional calls to an unavailable account and record the
missing coverage. Explicit live runs use the person's installed engine
login or configured Microcoder gateway plan, while keeping Coder state in
a fresh `CODER_HOME`. The run is authorized to send these synthetic tasks
for inference. Auto runs additionally grant only synthetic session text to
System One in that disposable ledger. Remembered facts are disabled.

`GW_REPLAY` and `GW_RECORD` are removed from children so inherited playback
cannot silently masquerade as a live measurement. Credentials are neither
printed nor stored in receipts. Engine-owned local sessions retain their
normal behavior. Each live artifact directory retains the JSONL events,
candidate patch and acceptance file for inspection.

## Read receipts and compare engines

```sh
coax/web/tool.sh coder/tests/repository-summary.coax /absolute/path/micro.jsonl /absolute/path/codex.jsonl /absolute/path/auto.jsonl
```

The summarizer reads existing receipts and performs no inference or tool
re-execution. It refuses malformed data, duplicate engine/case rows,
incompatible suite/compiler/harness identities or mismatched case prompts.
It reports observed versus missing coverage, accepted patches, eligible
task runs, provider failures, harness failures, held-out counts, time and
observable tool log rows. Missing cases never become passes.

A routing opportunity miss means a fixed engine solved a paired case and
the actual automatic run did not. It includes availability misses. A routing
quality miss counts only automatic runs with an eligible task outcome. The
best observed fixed result is an optimistic reference, not an engine the
router could know in advance. Decision time is included in end-to-end auto
latency; it is not separately exposed by this CLI measurement.

Receipts bind the suite/check content, exact task prompt, harness source and
compiler SHA-256, along with the repository revision. They record the engine
version where observable. Model and effort may remain engine defaults;
`model: null` means unobserved identity, not that every engine used the same
model. These compare installed engine bundles at the recorded time. Token
usage, cache usage and subscription price are null when unavailable.

`tool_calls` counts observed `tool` JSONL rows, not an exhaustive count of
engine-internal calls. In particular, some Codex tool starts are rendered
without a corresponding Coder `tool` log row. `tool_errors` counts observed
error result rows; it cannot supply the missing starts. Interpret these as
observability measurements, not comparable provider billing counters.

One trial on ten elementary cases, five held out, has little power to rank
engines. Publish the outcomes and failures, retain unavailable fields, and
use fresh tasks/repeated runs before changing routing policy. Policy-label
accuracy from chapter 13 remains a different measurement.

## Checks and evidence

```sh
coax test coder/tests/repository.coax --library
coax test coder/tests/repository-summary.coax --library
python3 psionic/tests/book.py --book coder
```

The [2026-11-03 evidence](../verification/coder-repository-2026-11-03/README.md)
records the offline journey and live pilot results, including account
availability and limits. These checks do not repeat passing model runs to
confirm them; the summary is recomputed from retained receipts.

## Structural follow-up suite

[`repository-v2.coax`](../../coder/tests/repository-v2.coax) keeps the original
v1 cases and receipts intact. From the repository root:

```sh
COAX=/absolute/path/to/coax coax/web/tool.sh coder/tests/repository-v2.coax offline
COAX=/absolute/path/to/coax coax/web/tool.sh coder/tests/repository-v2.coax availability
COAX=/absolute/path/to/coax coax/web/tool.sh coder/tests/repository-v2.coax interrupt
COAX=/absolute/path/to/coax coax/web/tool.sh coder/tests/repository-v2.coax run --case 0 --engine codex
```

The four fresh cases exercise strict parsing (0), epoch/sequence admission
state (1), exhaustive inclusive intersections across two modules (2), and a
superseded proposal followed by an explicit fork objective (3). Cases 0..2
are development cases. Case 3 is reserved for a later live continuation
measurement; its reference repair is checked offline, but it has not been
sent to a live engine in this follow-up. These public fixtures are not a
private evaluation set. Acceptance lives outside the working directory;
AGENTS.md must remain intact. Live calls have a three-minute bound per turn.
The fourth case permits two bounded turns, so its command remains under
15 minutes.

The availability mode uses an explicitly synthetic local Claude protocol
fixture: it exercises real CLI parsing and persistence without consuming
provider quota. It checks a typed failure and reported reset, exclusion on
both `route` and `run`, explicit retry, and invalid allowlist refusal.
The interruption mode kills real command processes after a non-idempotent
append and during a partial edit, then checks unknown status, inert replay
and an explicit read-only continuation without duplicated or completed edits.
It does not simulate power loss, transactional writes or every provider's
internal recovery.

The initial [follow-up evidence](../verification/coder-followup-2026-11-03/README.md)
contains a Codex parser smoke run and a Microcoder state-machine smoke run.
Different tasks are not an engine ranking. Larger repositories, more cases,
repeated trials and a fresh private holdout remain future evaluation work.
