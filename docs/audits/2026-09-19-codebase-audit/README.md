# Workspace audit: September 19, 2026

The workspace has useful separation between protocol, storage, decision models,
measurement, and the agent. Its strongest foundations are the relay's database
integration tests, protocol fixtures, explicit refusal types, and recorded model
provenance. The main risks are at the boundaries between these components:
execution does not always honor its bounds, several measurements can certify
the wrong behavior, and the clients accept incomplete or insufficiently bound
responses.

Fix the execution and measurement findings before using autonomous fan-out to
clear the backlog or using CoderBench results to tune the agent. Passing the
current tests does not establish those properties: the audit reproduced failures
through public APIs while the applicable test suites passed.

**Release blocker:** #9413 must not start unattended backlog fan-out until A01,
A17, and A18 have verified fixes. Execution intent, trusted executable probes,
and enforced admission bounds are prerequisites, not optional follow-up work.
The [remediation register](remediation.md) links the implementation issues and
the additional recommendations outside A01–A25.

## Scope and evidence

The reviewed source snapshot is
[`1843fa6c18a05537bf2b022f69361a9ba3ef12a1`](https://github.com/OpenAgentsInc/openagents/commit/1843fa6c18a05537bf2b022f69361a9ba3ef12a1).
It contains 10 workspace crates and 164 tracked Rust files under `crates/`,
totaling 71,819 lines including tests and comments. Source links below point to
that commit. The snapshot includes the restored CoderBench golden, its provenance
sidecar, headless turns, delegation, the capability and program registries, and
the CoderBench driver and preflight checks.
Later changes on `main` are outside this snapshot unless explicitly identified
as follow-up evidence in the [remediation register](remediation.md).

This is a risk-based review across the workspace, public interfaces, deployment
assets, migrations, training tooling, and documentation. It is not a claim that
every line has been proved correct. The review also considers all 27 open issues
at the final issue snapshot and their comments. See the
[issue map](issues.md) for existing work and overlap with findings.

The sibling `~/work/coder` repository was used as structural reference material.
No private source, prompts, endpoints, or credentials were copied into this
report or the workspace. The recommendations below come from behavior and code
in OpenAgents. A future port should retain its public specification, provenance,
and independent implementation tests.

Evidence labels distinguish **reproduced** failures from **source-confirmed**
control-flow or contract gaps. Source-confirmed findings include a proposed
regression test; they do not imply that an adversarial integration test ran.
The [verification record](verification.md) contains commands, results, limitations,
and instructions for the retained [Rust reproduction harness](reproduce.rs).

## Priorities

P1 means fix before relying on unattended execution or the affected evidence.
P2 means schedule a concrete correction or hardening change. These priorities
describe engineering impact, not a vulnerability scoring system.

| ID | Priority | Finding | Evidence |
| --- | --- | --- | --- |
| A01 | P1 | Prose examples and clarification replies can execute commands | Reproduced |
| A02 | P1 | Execution continues after shell or delegation deadlines | Reproduced |
| A03 | P1 | Output caps apply after unbounded buffering | Source-confirmed |
| A04 | P1 | CoderBench accepts unverified answers and incomplete evidence | Reproduced and source-confirmed |
| A05 | P1 | Calibration can change the answer without changing its correctness label | Reproduced |
| A06 | P1 | Concurrent callers can spend the same locked partition as a first read | Reproduced |
| A07 | P1 | Gym classifies Kev's refusals as harness failures | Reproduced |
| A08 | P1 | Relay responses are not bound locally to the current job | Source-confirmed |
| A09 | P2 | HTTP streaming corrupts split UTF-8 and accepts premature EOF | Reproduced |
| A10 | P2 | Relay subscriptions accumulate and failed sockets remain cached | Source-confirmed |
| A11 | P2 | Identity creation can replace or race an existing identity | Source-confirmed |
| A12 | P2 | Typed SDK answers accept invalid probabilities | Reproduced |
| A13 | P2 | A torn UTF-8 tail makes an otherwise valid trace unreadable | Reproduced |
| A14 | P2 | Lev's synchronous helper path can stall the async server | Source-confirmed |
| A15 | P2 | Kev lacks a total inference budget and concurrency admission | Source-confirmed |
| A16 | P2 | Kev advertises an alias it does not resolve | Source-confirmed |
| A17 | P2 | Capability probing executes repository-defined commands without a trust boundary | Source-confirmed |
| A18 | P2 | Declared bounds and read-only intent do not establish enforcement | Reproduced |
| A19 | P2 | Live and historical search use different matching semantics | Source-confirmed |
| A20 | P2 | Media uploads reach disk before authentication is checked | Source-confirmed |
| A21 | P2 | Database and media backups do not share a consistent snapshot | Source-confirmed |
| A22 | P2 | The SDK's whole-call retry budget does not bound attempts | Reproduced |
| A23 | P2 | The documented verification baseline is not clean or reproducibly pinned | Tool-confirmed |
| A24 | P2 | Generated build products are tracked | Tool-confirmed |
| A25 | P2 | Dependency maintenance policy is implicit and its default check fails | Tool-confirmed |

## Findings

### A01. Require an execution intent before parsing a shell plan

Tracking: [#9415](https://github.com/OpenAgentsInc/openagents/issues/9415).
Fixed by `25f0b54e4a`; see [A01 after the fix](verification.md#a01-after-the-fix).

The [plan parser](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/shell.rs#L119) extracts fenced JSON from
anywhere in an answer and accepts a `commands` array without requiring the
documented version. The [turn loop](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/agent.rs#L357) calls it
even when `clarify` is true. Clarification changes the prompt, but does not change
what the host permits.

The harness recognizes an example explicitly introduced with “do not run it” as
a plan. A stub generator returning a harmless file-writing plan during
`turn(true, ...)` creates the file. This is an execution boundary failure,
independent of the model's classification quality.

Represent a generated answer and an executable plan as distinct validated
outcomes. Require an exact supported schema and an execution-permitted turn;
reject command execution during clarification. Keep execution policy in the host,
including the operator's selected permissions. A substring denylist and a model
instruction are insufficient to establish that policy. Add regressions for
quoted examples, mixed prose, unsupported versions, and clarification replies.
This complements the routing work in #9395–#9397; those issues do not fix parsing.

### A02. Terminate and reap the process tree when execution ends

Tracking: [#9416](https://github.com/OpenAgentsInc/openagents/issues/9416).
Fixed by `a96845c40d`; see [A02 and A03 after the fix](verification.md#a02-and-a03-after-the-fix).

The [shell runner](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/shell.rs#L194) wraps `Command::output()`
in a 15-second timeout without `kill_on_drop`. The timeout cancels the wait, and
the command continues. The [delegate runner](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/delegate.rs#L523)
sets `kill_on_drop(true)`, but it does not establish or terminate a process group.

This finding supersedes the partial deadline fix in `f2a4bc79cb`; it does not
duplicate an already-completed fix. That change kills the direct delegate, but
leaves descendants alive, and the shell runner still lacks even that behavior.

The harness records `timed out` for a shell that writes a harmless marker at
16 seconds; the marker appears afterward. A delegated shell is killed after
100 milliseconds, but its background child still writes its marker. Aborting a
turn has the same ownership problem as timing it out.

Use one subprocess supervisor for both paths. On supported Unix hosts, put the
job in its own process group, terminate the group on deadline or cancellation,
and reap the direct child. Specify equivalent behavior before supporting other
platforms. Retain bounded output on failure. Test grandchildren, cancellation,
normal exit, and concurrent jobs. Tokio documents that dropping a child does not
kill it by default and that `kill_on_drop` concerns the child process:
[Tokio process documentation](https://docs.rs/tokio/latest/tokio/process/struct.Command.html).
The new [CoderBench waiter](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coderbench/src/drive.rs#L233)
also kills only the direct child, so use the same supervisor there. Complete this before the unattended runs in #9413.

### A03. Bound captured output while reading it

Tracking: [#9417](https://github.com/OpenAgentsInc/openagents/issues/9417).
Fixed by `a96845c40d`; see [A02 and A03 after the fix](verification.md#a02-and-a03-after-the-fix).

Both [shell execution](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/shell.rs#L210) and
[delegation](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/delegate.rs#L537) collect complete stdout and
stderr with `output()` and only then truncate the resulting strings. The 16 KiB
shell limit and 64 KiB per-stream delegation limit bound the retained result,
not peak memory. A fast producer can exhaust memory within the allowed time;
parallel delegates multiply the exposure.

Drain stdout and stderr concurrently into capped buffers. Keep a truncation
indicator and total byte counts, and decide whether exceeding the cap terminates
the job or discards further bytes while draining. Avoid holding a second complete
lossy UTF-8 copy. Test finite oversized output on both streams and a producer
that continues until cancelled. Correct comments that describe the current cap
as a bound on everything the process holds.

### A04. Make benchmark success require verified evidence

Tracking: [#9418](https://github.com/OpenAgentsInc/openagents/issues/9418).

[`Task::judge`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coderbench/src/lib.rs#L278) checks the delegation
count but does not enforce `grade.delegations_correct`. It faults only
`Some(false)`, so an unknown correctness value passes. Decisions and checks pass
when their names exist, regardless of their answers or outcomes.
[`observe`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coderbench/src/lib.rs#L354) discards call outcomes,
completion state, unreadable-line counts, and ordering; write detection depends
on an optional self-reported `wrote` string.

A constructed run with six empty, ungraded delegations, null decision answers,
and the required check names receives no faults from a task requiring six correct
delegations. This demonstrates the grader defect without claiming that the
current staged golden is an observed successful run. Its new provenance sidecar
correctly distinguishes those cases.

Preserve completed, failed, and unknown outcomes through observation. Enforce
the required number of verified answers, decision predicates, step order, and
trace integrity. For a task that forbids writes, compare the actual workspace or
run in an environment that enforces the constraint. Missing evidence must produce
an unverifiable or failing grade. Add negative fixtures for the newly landed
#9406 driver and before #9411 uses this grade as its optimization signal. The driver prints failed or timed-out execution, but still
returns the trace grade without making that execution outcome a grading fault.

### A05. Align calibration, served choices, and correctness labels

Tracking: [#9419](https://github.com/OpenAgentsInc/openagents/issues/9419).

[`Map::apply_distribution`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/calibrate.rs#L233) rescales the
original winner to its calibrated probability and redistributes the remainder.
This can change the winner. [`mapped_observations`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/eval.rs#L275)
then uses the new maximum with the old `row.correct` value. In contrast,
[Lev serving](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/lev/src/serve.rs#L673) derives its typed answer from the
rescaled distribution.

The harness maps `{yes: 0.8, no: 0.2}` to approximately
`{yes: 0.25, no: 0.75}` while retaining the original `correct: false`. When the
truth is `no`, the served answer is now correct but the evaluation still calls
it wrong. Other transitions can flatter results. The current row does not retain
enough target information to recompute arbitrary multiclass correctness.

Choose one contract: calibrate confidence in a fixed selected answer, or permit
the predictor to change and recompute its correctness against retained labels.
Use that contract consistently in fitting, gates, metrics, and serving. Test
binary and multiclass winner changes and ties. This is a separate implementation
defect from #9381, #9394, and #9401, although their measurements depend on it.

The fix also requires a provenance review and regeneration of #9376's metric
derivations and mapped-calibration claims. The adopted ECE `0.0266`, Brier
`0.0119`, and NLL `0.6428` values are **raw block standard deviations**, not the
two-sigma comparison thresholds. Inspection of `gym::spread::Draw::observation`
and `gym`'s `report_blocks` shows that their raw derivation does not call
`mapped_observations`. A05 therefore does not by itself prove these three numbers
wrong. Re-derive them after settling the predictor/label contract, distinguish
raw from mapped paths, and report whether each value and dependent claim changes.
Recompute the affected mapped results; publish a new gate digest if its semantics
or numeric basis changes. See the [follow-up evidence](verification.md#follow-up-evidence-review).

### A06. Make the locked-partition read a transaction

Tracking: [#9420](https://github.com/OpenAgentsInc/openagents/issues/9420).

[`LockedLedger::read_locked`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/suite.rs#L593) reads the ledger
and later appends a spend record without holding a lock across both operations.
[`spend`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/suite.rs#L648) also returns the items without syncing
the record to durable storage.

In a private temporary ledger, 15 of 16 concurrent callers were accepted as the
first reader of the same suite digest. The probe neither invokes a model nor
spends the repository's measurement ledger. The exact count is scheduling-dependent.

Serialize read, eligibility check, append, and durable commit under a
cross-process lock. Reuse the result store's transaction discipline where
appropriate. Return the partition only after the spend is committed. Test
multiple processes and interrupted writes. This is distinct from #9399's
training-data contamination: a clean partition still needs a reliable read budget.

### A07. Give Kev refusals the contract Gym consumes

Tracking: [#9421](https://github.com/OpenAgentsInc/openagents/issues/9421).

[Kev's refusal helper](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/kev/src/serve.rs#L97) returns HTTP `422` with
`{"detail": "..."}` for request and inference errors.
[Gym's classifier](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/eval.rs#L123) recognizes a refusal only
when `error.code` or `code` exists. It therefore treats Kev's own refusals as
harness failures. The harness confirms this for the exact empty-questions
response body asserted by Kev's HTTP test.

This affects valid requests that Kev cannot answer, such as states beyond its
limit, as well as malformed input. Door-owned failures can disappear from the
scored/refused denominator, undermining comparisons with Lev. A test named
`refusals_are_typed` currently verifies the JSON shape, not this interoperability.

Publish stable refusal codes for each failure class and test a real Kev response
through `jev` and `gym::eval::classify`. Preserve compatibility with `detail` if
needed. Do not compensate by parsing English error strings. Include over-budget
states, unknown models, invalid requests, and inference failures. This complements
#9398's capacity measurements.

This invalidates relying on `gym::eval::classify` to establish that Kev's
door-owned refusals were counted as refusals in #9384 and the
[variant-score record](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/docs/kev/measurements/2026-09-19-variant-scores.md#L124).
It does not establish that the published scores lost items. A follow-up comparison
of committed rows against the suite finds exactly the 157 expected open item/split
pairs for each of the four Kev variants, with no duplicates, missing pairs,
unexpected pairs, or refusal rows. That is independent coverage evidence; the
classifier could not distinguish a Kev refusal from a harness failure, but the
runner does count unrecorded harness outcomes as lost.

After the fix, re-check #9384's rows, suite and receipt integrity, run provenance,
and any retained failed-response evidence. Explicitly distinguish complete
scored-row coverage from correct classification of failures. Correct the wording
and any affected statistics or downstream comparisons; preserve the numbers when
the independent reconciliation supports them. Do not infer historical omissions
solely from this classifier bug, or infer correct refusal accounting solely from
zero recorded refusals.

### A08. Verify the signed response's job binding locally

Tracking: [#9422](https://github.com/OpenAgentsInc/openagents/issues/9422).

The [relay receive loop](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/relay.rs#L302) verifies the worker
pubkey and event signature, then decrypts the content. It trusts the surrounding
subscription label to associate the event with the current request. It does not
verify that the signed event's `e` tag names this request or that its recipient
tag names this client.

A relay controls that unsigned label. It can replay an older valid worker result
for the same client under a new subscription; signature verification and
decryption still succeed. This is a source-confirmed replay path, not a recorded
attack against the deployed relay.

Check the signed event's job ID, recipient, allowed kind, and supported payload
version before delivering text. Deduplicate event IDs and enforce the protocol's
ordering rules for feedback. Add a mock relay that relabels a valid old result,
duplicates a partial, and supplies an event for a different job. Make these
negative cases part of #9410's transport proof.

### A09. Treat streaming as a byte protocol with explicit completion

Tracking: [#9423](https://github.com/OpenAgentsInc/openagents/issues/9423).

[HTTP streaming](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/generate.rs#L210) runs
`String::from_utf8_lossy` on each network chunk. Network chunks need not end at
character boundaries. The harness splits `é` across two chunks and receives
`caf��` instead of `café`. The same loop returns success when EOF follows any
text, even without `response.completed`; the harness receives a successful
`partial` response. It also lacks a total or idle deadline and a framing-buffer cap.

Buffer bytes until complete SSE records can be decoded, or use a maintained SSE
parser with the required semantics. Require a successful completion event,
handle incomplete responses explicitly, and bound time and buffered bytes.
Preserve partial text as diagnostic output, never as an executable completed
plan. Test every byte boundary, multiple data lines, malformed records, early
EOF, an explicit incomplete event, and a stalled connection.

### A10. Close relay subscriptions and discard broken connections

Tracking: [#9423](https://github.com/OpenAgentsInc/openagents/issues/9423).

Each [relay turn](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/relay.rs#L268) creates a new subscription
and never sends `CLOSE`. The relay's default is
[32 active subscriptions](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/nostr-relay/src/gateway/config.rs#L30).
Once full, the relay returns `CLOSED`, which the client ignores. A later turn can
therefore wait for its full deadline rather than reporting admission failure.

The [connection cache](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/relay.rs#L364) is cleared only for
the outer timeout. An ordinary socket error returns through `Ok(result)` and
leaves the broken socket cached, contrary to the comment above it. Connection
establishment is outside that turn timeout.

Close each job subscription on every terminal path, handle `CLOSED` explicitly,
and invalidate the socket after transport failure. Apply a connection deadline.
Test more than 32 successive jobs, disconnect/reconnect, refusal, and cancellation.
Include these lifetime tests in #9410.

### A11. Create persistent identities atomically

Tracking: [#9423](https://github.com/OpenAgentsInc/openagents/issues/9423).

[`Identity::load`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/relay.rs#L83) treats every file-read
error as absence and generates a new key. It writes with truncation and sets
permissions afterward. Two first-time processes can each adopt a different key,
with only the last one preserved on disk. An existing unreadable file can also
enter the replacement path instead of producing its original I/O error.

Generate only for `NotFound`. Use atomic exclusive creation with private
permissions from the start, or a lock and atomic installation protocol. If
another process wins creation, read that identity. Propagate protection failures.
Test simultaneous startup, permission errors, and interrupted creation. Do not
log the key while diagnosing any of these cases.

### A12. Validate numeric answer invariants at the SDK boundary

Tracking: [#9424](https://github.com/OpenAgentsInc/openagents/issues/9424).

[`SystemOneResponse::decode`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/answers.rs#L148) validates
deserialization shape without establishing the numeric contract. A Noul value
of `-2.0` is accepted as a typed answer. Downstream code uses such values as
probabilities and thresholds, so a malformed or incompatible door can produce
meaningless decisions and metrics.

Validate ranges, probability mass within a stated tolerance, selected options,
and score legend bounds. At the request-aware boundary, check answer coverage and
option identity. If raw compatibility is intentional, retain it separately and
require strict validation for Coder and Gym. Add malformed-door fixtures for all
three answer types. The public contract is documented in the
[TypeSafe API reference](https://docs.typesafe.ai/api.md).

### A13. Recover the valid prefix of a torn trace

Tracking: [#9425](https://github.com/OpenAgentsInc/openagents/issues/9425).

The [ATIF reader](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/atif/src/log.rs#L231) skips malformed JSON lines,
but `BufRead::lines()` fails before parsing when a final record ends in partial
UTF-8. A valid session and step followed by a torn two-byte character cause the
whole read to return `InvalidData` in the harness.

Read newline-delimited bytes first, then decode each record under an explicit
recovery policy. Recover a torn final record for interactive viewing while
reporting the damage. Let benchmark ingestion demand stronger integrity rather
than silently treating recovered data as complete. Also reject or report repeated
session headers, records after an end record, and malformed interior records.
Test truncation at every byte of a non-ASCII final record.

### A14. Bound and supervise Lev's blocking helper calls

Tracking: [#9426](https://github.com/OpenAgentsInc/openagents/issues/9426).

The [async HTTP handler](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/lev/src/serve.rs#L611) directly calls the
synchronous estimator and bridge pool. The pool uses blocking mutexes and scoped
threads; [bridge exchange](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/lev/src/bridge.rs#L263) uses `read_line`
without a response limit or deadline. Helper stderr is piped at
[startup](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/lev/src/bridge.rs#L166) but never drained. A hung helper or
a full stderr pipe can indefinitely occupy a pool slot and a Tokio worker.

Move blocking work behind a bounded worker queue, drain diagnostic output, bound
message size, correlate response IDs, and supervise deadlines with helper
termination and restart. An async timeout around an uninterruptible blocking
call alone does not recover the resource. Tokio likewise documents that started
[`spawn_blocking` work cannot be aborted](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html).
Test a fake helper that hangs, floods stderr, emits a long line, exits, and returns
the wrong response ID. The existing local helper tests passing does not cover
these failure modes.

### A15. Admit Kev work against total compute and memory bounds

Tracking: [#9426](https://github.com/OpenAgentsInc/openagents/issues/9426).

[Encoding](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/kev/src/encode.rs#L105) bounds state and individual
branches, but appends all question branches into one sequence. The
[attention mask](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/kev/src/encode.rs#L205) allocates quadratically in
the total length. Many individually valid branches can therefore create an
unreasonable allocation. The [HTTP handler](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/kev/src/serve.rs#L108)
starts blocking inference per request without a model-capacity semaphore.

Set limits on total tokens, question and option counts, and estimated attention
memory before allocation. Admit concurrent work through a measured model-specific
queue and return a typed busy refusal when it is full. Request-body limits alone
do not bound inference cost. Test admission using synthetic encodings, without
deliberately exhausting memory. This becomes more important when a currently
local service is exposed through the planned mesh.

### A16. Resolve every advertised model alias

Tracking: [#9426](https://github.com/OpenAgentsInc/openagents/issues/9426).

[`ServeState::select`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/kev/src/serve.rs#L70) resolves an exact model
ID, `kev-latest`, or an empty string. It never reads `aliases`, although the
model listing advertises `jev-latest` for the default model. A Jev client using
its normal default can therefore receive `UnknownModel` from a supposedly
compatible door. The existing round-trip test overrides the model to `kev-latest`.

Resolve aliases from the same registry used to publish model cards. Test a client
with its default model unchanged, explicit variant IDs, and genuinely unknown
names. Avoid publishing aliases that the serving path cannot honor.

### A17. Separate registry discovery from trusted executable probes

Tracking: [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427).

[`Survey::read`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/survey.rs#L43) loads repository manifests
and probes all of them. A manifest controls the executable and its arguments;
[`run`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/capability.rs#L754) executes them with the caller's
permissions, no deadline, and unbounded output. An argv avoids accidental shell
interpolation, but a manifest can name an interpreter or another program that
performs side effects.

The registry API is newly implemented; the ordinary turn loop does not yet call
`Agent::survey`. This finding is a current API hazard and a prerequisite for
#9409, not a claim that opening any repository already triggers it in the TUI.

Keep discovery inert. Require host-owned trust in the executable adapter before
running a repository-supplied probe, especially before accepting relay manifests.
Use the bounded subprocess supervisor from A02–A03. Validate argv and transport,
and distinguish a failed probe from proven availability; currently an execution
error in the workspace probe leaves a capability present. Test hostile manifests,
hanging probes, and missing or malformed registries without launching a real
external agent. The new [CoderBench preflight](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coderbench/src/preflight.rs#L230)
also executes manifest-defined version commands. It has a ten-second timeout, but needs the same trust boundary.
Share the validated manifest contract so the two implementations do not drift.

### A18. Treat unknown enforcement and unobserved writes explicitly

Tracking: [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427).

[`Manifest::ignored_bounds`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/capability.rs#L449) only
intersects required bounds with `cannot_enforce`. A bound absent from both lists
is admitted by this helper. The harness asks the checked-in manifest about
`read_only` and gets an empty ignored-bounds list.

The [delegate task](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/delegate.rs#L328) records `writes: false`,
but [`Delegator::run`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/delegate.rs#L511) gives the executor
ordinary write access to the shared directory. A harmless test executor writes
a file and returns `answered` for a task built with `Task::reading`. Refusing
explicit write tasks is useful, but it does not establish read-only execution.

For every required bound, record whether the host enforces it, the executor
verifiably enforces it, or enforcement is unknown. Refuse unsupported requirements.
Use filesystem isolation or an explicit trusted-executor contract for read-only
work, and record actual workspace changes for grading. Worktrees separate edits;
they do not themselves prohibit writes. At this snapshot, the runner refuses
worktree isolation outright with `isolation_unavailable`: every admitted
delegation runs in the shared directory. There is no implemented isolated
execution option, and `Task::reading` supplies only a declaration.

Add negative acceptance cases to #9409
and #9413 before claiming that fan-out preserves these constraints. Issue #9414 now
tracks a related model judgment failure; that experiment cannot replace host
enforcement of permissions.

**Publication update:** [commit `1eb60eccea`](https://github.com/OpenAgentsInc/openagents/commit/1eb60eccea31873af59fe2df65e16ce46b99b928)
landed during this follow-up. It refuses unknown bounds at runtime admission and
implements worktrees when the delegator has a repository configured. The
shared-directory-only limitation above describes the original snapshot. A18
remains open: runtime admission still treats a manifest's `enforces` declaration
as executor enforcement, and a worktree does not enforce read-only access. See
the [follow-up checks](verification.md#runtime-admission-and-worktrees).

### A19. Use the same search semantics for history and live events

Tracking: [#9428](https://github.com/OpenAgentsInc/openagents/issues/9428).

[`Filter::matches`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/nostr/src/domain/filter.rs#L84) lowercases content
and checks substrings. The [historical SQL](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/nostr-relay/src/store/statements.rs#L194)
uses `tsvector` and `plainto_tsquery('simple', ...)`, which match lexemes. For
example, searching for `cat` matches `catwalk` in the live path but not through
that historical full-text predicate.

Choose and document one search interpretation, then make replay and live
delivery agree. Add a differential fixture that publishes before and after a
subscription using punctuation, word boundaries, case, and non-ASCII text. Keep
privacy exclusions aligned as well. The PostgreSQL acceptance suite passes but
does not establish this equivalence.

### A20. Reject unauthenticated uploads before writing their bodies

Tracking: [#9428](https://github.com/OpenAgentsInc/openagents/issues/9428).

The [upload handler](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/nostr-relay/src/gateway/media.rs#L185) creates a
temporary file and streams the entire body before checking for an authorization
header at line 216. Per-pubkey rate and quota checks happen later still. Existing
IP, size, and timeout limits constrain the exposure, but unauthenticated requests
can consume disk bandwidth and temporary storage up to those limits.

Check header presence and verify the signed authorization's method, URL, time,
and identity before accepting the body. Reserve applicable capacity first, then
verify the body digest after streaming. Keep cleanup reliable on cancellation
and error. Test that a request without authorization cannot create a temporary
upload and that rejected authenticated uploads release reservations.

### A21. Make a restorable database-and-media backup unit

Tracking: [#9428](https://github.com/OpenAgentsInc/openagents/issues/9428).

The [backup script](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/deploy/backup/nostr-relay-backup#L26) completes
`pg_dump`, then archives the live media directory. A media deletion between those
operations can leave a database record in the dump whose blob is absent from the
archive. Matching timestamps in filenames do not make the snapshots consistent.

Use a shared snapshot or a documented barrier and retention scheme that keeps
every blob referenced by the database snapshot available until the archive is
complete. Publish a manifest only when both components are complete. Verify a
restore into an isolated database and media root, including concurrent upload and
deletion scenarios. This audit did not execute production backup or restore jobs.

### A22. Enforce the advertised whole-call retry budget

Tracking: [#9424](https://github.com/OpenAgentsInc/openagents/issues/9424).

[`RetryPolicy::budget`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/retry.rs#L65) is documented as the
whole call's budget, including its first attempt. The
[retry loop](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/jev/src/client.rs#L348) checks it only after a failed
attempt, while deciding whether to sleep and retry. A successful attempt can run
past the budget and still return success.

The loopback harness sets a 50-millisecond budget and a one-second attempt
timeout. A server answering after approximately 200 milliseconds succeeds.
Apply a monotonic total deadline to attempts and waits, with each attempt's
timeout capped by the remaining budget. Alternatively rename and document the
setting as a retry-admission budget if that narrower contract is intended.
Test delayed success, body stalls, and a retry with little time remaining.

### A23. Establish a clean, repeatable Rust verification baseline

Tracking: [#9429](https://github.com/OpenAgentsInc/openagents/issues/9429).

At the audited snapshot, Rust 1.95.0 reports formatting differences in 51 files.
Strict Clippy with serving and TUI features fails on three findings:
`cloned_ref_to_slice_refs` in `kev/src/decision.rs:108`,
`manual_is_multiple_of` in `lev/src/adapter.rs:217`, and `double_must_use` in
`lev/src/serve.rs:485`. The machine's default Rust 1.94.1 cannot build crates
declaring Rust 1.95. There is no checked-in toolchain pin.

Issue #9402 already owns the formatting failure. Resolve it once on an agreed
toolchain, then keep the gate clean. Centralize intended edition, minimum Rust
version, and package policy. The latest CoderBench change corrected its edition,
minimum version, publish setting, and lint inheritance during this audit.
`nostr` and `nostr-relay` still omit workspace lint inheritance. These differences
can be intentional, but the policy should be explicit rather than an accidental
consequence of adding a crate.

Add a manual verification entry point that checks formatting, strict Clippy,
the supported feature matrix, and the PostgreSQL acceptance script. Pin the
formatter/compiler used for the gate and test any separately promised minimum
version. Keep this on contributor machines or non-GitHub infrastructure, as the
repository contract requires. Do not introduce GitHub workflows.

### A24. Remove tracked build products from the source baseline

Tracking: [#9430](https://github.com/OpenAgentsInc/openagents/issues/9430).

Git tracks 80 generated paths under `swift/lev-bridge/.build` and
`training/lev-adapter/__pycache__` combined. They include compiled products and
machine-specific build state. The root [ignore file](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/.gitignore#L1) ignores
only `/target`, so rebuilding can continue to produce noisy or accidental commits.

Remove generated artifacts from the index in a dedicated change and ignore Swift
build state and Python bytecode. Keep intentional fixtures and provenance records.
Verify that the documented helper build succeeds from a clean checkout. If a
binary must be distributed, publish it as a versioned release artifact with its
source revision, target, and digest rather than an incidental build-directory file.

### A25. Make dependency maintenance decisions explicit

Tracking: [#9431](https://github.com/OpenAgentsInc/openagents/issues/9431).

`cargo deny check advisories` fails under its default policy on transitive
`paste 1.0.15`, [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436).
This is an **unmaintained-crate advisory**, not evidence of an exploitable
vulnerability. It arrives through the Candle/GEMM/Pulp and tokenizer dependency
graph; the advisory reports no safe upgrade for `paste` itself.

Add a reviewed dependency policy and document the chosen response: an upstream
upgrade, a vetted replacement through the dependent libraries, or a narrowly
scoped exception with an owner and review date. Do not blindly patch a low-level
numeric dependency or suppress the entire advisory category. Include licenses
and source provenance in release review; the advisory command does not check
either of those on its own.

## Rust practices and maintainability

These improvements complement the concrete findings. They should not displace
the execution and measurement fixes.

- **Model invariants in types.** Introduce validated probability, supported
  plan-version, job-binding, and execution-outcome types at boundaries. Keep
  unknown, refused, failed, and completed states distinct. Use checked arithmetic
  for caller-provided durations such as `Bounds::minutes`, and validate public
  serving-state constructors before indexing the default variant.
- **Keep blocking work off async workers.** Use bounded ownership of subprocesses
  and model workers. `spawn_blocking` helps scheduling but does not supply
  cancellation or admission control. Make those contracts visible in APIs.
- **Use RAII for terminal restoration.** The TUI enters raw mode before several
  fallible setup operations and restores it only on the ordinary cleanup path
  ([terminal setup](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/main.rs#L287)). A guard and coordinated
  panic cleanup should restore raw mode, alternate screen, cursor, and colors
  after partial setup or unwinding.
- **Separate lossy rendering from reliable control events.** The TUI ignores
  `try_send` failure for every event, including command outcomes
  ([event forwarding](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/coder/src/main.rs#L315)). Coalesce text deltas
  if needed, but preserve execution and failure events. Bound scrollback and
  avoid rewrapping the entire accumulated transcript on each draw.
- **Use structured errors where callers branch on them.** Reserve descriptive
  strings for presentation. Version refusal codes and program/admission errors;
  retain context without making English wording a protocol. Preserve the SDK's
  existing secret-redaction behavior when adding diagnostics.
- **Strengthen persistence APIs.** Make append-only state transitions explicit,
  keep readers' recovery policy separate from graders' integrity requirements,
  and record exact model, question, suite, gate, and artifact identities. Retain
  the result store's receipt-chain and locking discipline.
- **Add differential and property tests at boundaries.** Prioritize byte-level
  streaming, interrupted logs, randomized filter equivalence, process-tree
  cancellation, and cross-door contract fixtures. Keep unit tests for pure
  invariants; avoid testing only that an implementation repeats its own output.
- **Reduce custom security-sensitive machinery where practical.** The Nostr
  crate has useful pinned vectors and no cryptographic break was established in
  this audit. Its hand-implemented NIP-44 primitives still warrant independent
  vectors, fuzzing, and consideration of maintained RustCrypto primitives.
  Reusing a cryptographic primitive does not require adopting a third-party
  Nostr stack.

## Coverage and what to preserve

| Area | Review focus | Assessment and next checks |
| --- | --- | --- |
| `atif` | Append-only records, document rendering, interrupted sessions | Keep ordered records and decision calls; fix byte recovery and formalize integrity checks. |
| `coder` | Routing, shell loop, streaming, relay, trace, headless, delegation, registries | Keep the shared `turn::run`; enforce host permissions, lifetime bounds, and transport contracts before the program runtime depends on them. |
| `coder-terminal` | Editor, grapheme handling, layout, rendering | Preserve the shared intensity/frame/rail system and existing editor tests; exercise terminal cleanup, resize, and long sessions interactively. |
| `coderbench` | Task schema, golden provenance, driver, preflight, observer, grader | Staged/observed/authored provenance is useful; add negative grading fixtures before generating optimization claims. |
| `gym` | Rows, calibration, gates, suites, ledger, receipt store, TUI | Preserve digests and explicit refusal accounting; repair label alignment and ledger concurrency, then address the experimental-design backlog. |
| `jev` | Questions, answer decoding, HTTP, retries, errors, blocking client | Preserve typed request construction and redacted keys; validate numeric output and honor total deadlines. |
| `kev` | Packing, isolation mask, readout, model loading, API and server | The selected 0.5b numerical and HTTP tests passed; enforce resource admission and the same alias/refusal contract other doors consume. |
| `lev` and Swift helper | Compilation, estimators, calibration, admission, bridge, serving | Preserve signed-helper checks and release identity validation; isolate blocking work and test helper failure and recovery. |
| `nostr` | Events, signatures, filters, encoding, NIP validators | Preserve pinned protocol fixtures and pure boundaries; add history/live equivalence and broader adversarial parsing tests. |
| `nostr-relay` | Gateway, authentication, worker queues, SQL, migration and media paths | Parameterized SQL, database locking, bounded queues, and disposable-Postgres tests are strong foundations; harden upload admission and backup consistency. |
| Deployment and tooling | Docker, systemd, scripts, backups, manifests, lockfile, training files | Keep runtime hardening and manual acceptance checks; remove generated files and specify a reproducible verification and restore procedure. |

The audit did not find evidence of SQL injection in the reviewed prepared-query
paths. This is a scoped observation, not a security certification. The running
production service and real operator credentials were not probed.

Documentation also needs reconciliation. The README's crate table lists only six
of the ten crates. The rebuild plan still describes a `none` halt and a `0.45`
confidence floor that do not describe the current router. Mark historical plans
as historical and link to current behavior. The repository contract describes
Swift as the only non-Rust source tree, while 18 Python source files are tracked
for training and tooling; clarify the intended exception rather than forcing
an unrelated rewrite. Preserve `docs/transcripts/` as instructed.

## Order of work

1. **Make execution bounded and explicit:** A01–A03, then complete A17–A18 for
   the runtime introduced in #9409. A01, A17, and A18 are hard blockers on
   #9413; a passing golden or an independence judgment does not waive them.
   Include process-tree cancellation and actual no-write evidence in its
   acceptance criteria.
2. **Repair the evidence used to choose models and programs:** A04–A07, A12–A13,
   and A22. Then address contamination, variance, and question quality in the
   existing Gym and decision-model issues. Do not optimize against an unsound grade.
   Re-derive #9376's measurement basis after A05 and reconcile #9384's rows and
   refusal-accounting claims after A07.
3. **Prove transport and serving failure behavior:** A08–A11 and A14–A16. Extend
   #9410 beyond a successful episode to replay, disconnect, saturation, timeout,
   and refusal cases.
4. **Close operational and repository gaps:** A19–A25, terminal lifecycle tests,
   documentation reconciliation, and a tested restore. Resolve #9402 in a separate
   formatting change so behavior fixes remain reviewable.

For designs brought over from the reference repository, port the observable
contract and its counterexamples first. Implement it in the existing Rust crate
that owns the concern, keep the shared terminal and turn runner, and state the
independent reimplementation in the implementation commit. New program steps
should consume proven host capabilities rather than assuming that a manifest,
a prompt, or a trace assertion enforces them.
