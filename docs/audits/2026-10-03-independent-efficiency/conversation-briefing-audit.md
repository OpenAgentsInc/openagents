# Briefing opportunities in actual agent conversations

Recorded October 3, 2026 UTC (October 2 in America/Chicago).
This extends the [independent efficiency audit](README.md) and
[System One experiments](system-one-briefing.md).

## Assessment

**The clearest opportunity is to prepare operational facts, task intent, and
prior attempt outcomes alongside source context.** Recent conversations show
agents losing work through stale ownership, incompatible execution state,
forgotten command corrections, and shared build artifacts. Better source
retrieval would help, but Tree-sitter alone would miss these failures.

Several observed problems need deterministic enforcement as well as a brief.
Claims must be checked when work starts; build logs must belong to a run;
runtime versions must be checked before deployment. Asking an agent to
remember these facts leaves races and stale state unresolved.

The useful division of work is:

- **Code establishes facts:** repository and runtime revisions, ownership,
  package boundaries, available tools, artifact paths, and prior results.
- **System One judges relevance:** which evidence supports the requested
  behavior, which historical candidate matches the user's clues, and which
  unresolved question deserves another lookup.
- **The executor handles uncertainty:** diagnosis, implementation, novel
  failures, and requirements that the user clarifies during the task.

This review identifies experiments; it does not measure how much money or
completion time a new briefing would save. No new Claude-versus-Codex trial
was run. The earlier head-to-head results remain separate.

## Evidence and scope

The owner authorized inspection of recent local Claude conversations for
this repository. Quantitative analysis covers **October 1, 00:00 through
October 2, 21:36 America/Chicago**, or October 1, 05:00 through October 3,
02:36 UTC, with the endpoint excluded. Qualitative review emphasizes the
recent coordinator and eight deliberately selected OpenAgents child tasks,
with September 28–29 sessions used for concrete recurring problems. The
qualitative sample is not random and cannot establish population failure rates.

The quantitative population is the **local Claude project history**, which
also contains coordination, maintenance, and some adjacent workspace work.
It is not a census of OpenAgents product development. A child task devoted
to another repository was excluded from the qualitative examples.

Private transcripts, prompts, commands, session identifiers, and evidence
anchors stay outside this repository. The committed
[aggregate measurements](conversation-activity.json) contain counts only.
The owner retains a private evidence index with timestamps and record
anchors. These observations are not independently reproducible without
authorized access to that history.

### Counting rules

- Deduplicate tool invocations by tool-use ID across files. Agent calls can
  resume work, so their count is not a count of distinct agents.
- Deduplicate assistant usage by message ID. Streaming records repeat
  input counts and can increase the recorded output count; retain the
  maximum of each field per message. Do not sum streaming chunks.
- Count only records whose timestamps fall inside the window. A task can
  start before the window or finish after it; these are partial histories.
- Weight context statistics by message, not by task. A streamed response
  crossing the boundary can have partial output usage in this window.
- Keep top-level and child records separate. Top-level records take
  precedence if an ID occurs in both.
- Treat command categories as nonexclusive lexical matches. A Bash request
  can batch several commands or contain a quoted command that never runs.
- Do not turn timestamp gaps into reasoning time, serial elapsed time,
  idle time, or wasted time. Concurrent work and user pauses overlap.
- Treat usage metadata as recorded input exposure, not verified billing.
  Cache reads repeatedly count existing context; they are not unique text.

### Scale of the observed activity

| Measure | Coordinator | Child transcripts | Combined |
| --- | ---: | ---: | ---: |
| Files with records in the window | 1 | 167 | 168 |
| Distinct tool invocations | 1,460 | 17,929 | 19,389 |
| Bash invocations | 1,200 | 17,138 | 18,338 |
| Assistant message IDs with usage | 1,692 | 17,596 | 19,288 |
| Median recorded input tokens per message | 632,998 | 200,427 | 212,366 |
| 95th percentile recorded input tokens per message | 939,799 | 456,854 | 643,846 |

The calculation removes 200 repeated tool-use records. An independent
recalculation of the coordinator's counts and token summaries agrees.
Child transcript file counts do not establish the number of active agents.

Within the combined Bash population, 11,225 requests mention search commands,
2,555 mention Git inspection, 2,068 mention Cargo test/check/format/lint
commands, 419 mention Cargo builds, and 1,009 contain a literal sleep.
These categories overlap. They describe where to inspect, not how much work
was unnecessary. In particular, 12,521 Bash requests match multiple tags.

Large retained contexts make narrow handoffs and compact state worth testing.
However, the coordinator's recorded input is overwhelmingly cache reads.
Replacing it with freshly generated briefings can lose cache benefits.
Compare actual preparation, cache creation, cache reads, output, and retries
before claiming lower cost. Required instructions and useful history are not
automatically removable overhead.

## What happened, and what preparation could change

### 1. Work ownership was sometimes stale

The coordinator discovered a Coder run still working an issue another agent
had closed, then stopped it. In a separate child task, the initial assignment
omitted an active claim on overlapping integration work. That child wrote
duplicate flow code, checked updated main and issue state late in the task,
and removed the duplicate after the other change landed. Some of its
remaining work was useful; the full task duration is not a loss estimate.

The overlapping task's [claim comment][overlap-claim] existed more than
12 minutes before that child launched. The child fetched only the issue's
title and body, then continued after its installed CLI rejected the claim
subcommand. The needed ownership fact was available. Admission failed to
enforce it; the problem was more specific than a vague lack of context.

There is a useful counterexample: another issue was scheduled twice, but
the existing claim check made Coder skip it after a different agent claimed
it. Duplicate scheduling intent did not become duplicate execution.

**Prepare:** issue state, claim holder, task identity, active path scope,
base revision, blockers, terminal result, and observation time.

**Enforce:** recheck existing claim records before admission and integration;
invalidate queued work after closure or a new claim. A semantic overlap
warning can supplement exact claims, but must not block independent work
solely because two tasks touch the same crate.
An unavailable claim command must produce a supported fallback or explicit
blocker, never an implicit assumption that the issue is unclaimed.

### 2. Build setup repeated known failures

An older OpenAgents session invoked the mobile package through workspace
`-p` commands, corrected the failure using `--manifest-path`, then repeated
the wrong form later. The separate mobile workspace is a deterministic
manifest fact that can be supplied before any agent chooses a command.

In the child sample, three tasks independently encountered missing
`protoc`. One of those tasks later encountered missing well-known protobuf
includes even though the compiler itself existed. Several shell waits were
refused and then replaced with another observation path. These are execution-contract
problems, not difficult source reasoning.

**Prepare:** package owner, workspace root, exact supported check commands,
target/features, compiler and include prerequisites, permitted execution
surface, warm target directory, and supported completion-wait mechanism.

**Persist:** a correction as a scoped record: the rejected command, observed
failure category, working alternative, environment identity, and evidence
time. Reuse it while those facts remain valid. A stale workaround should not
become a permanent instruction.

### 3. Two agents shared a build log

Two sampled children launched remote checks with the same shared log path
less than a minute apart. One then read the other task's test results and
reran with a unique path to expose its own compile failure.

**Prepare and allocate:** a run ID, checkout, target slot, process handle,
log path, result path, and cleanup owner. Require the result receipt to name
the same run, source revision, and command as the request.

Older sessions also piped Cargo output through `tail` or `grep` without
preserving the compiler's exit status. The tool-level error flag could be
false while the result text contained a failed build or test. Some truncated
diagnostics required another run to obtain the relevant lines. Preserve the real process
exit status, retain the complete log, and return a bounded actionable diagnostic.
A successful shell pipeline is insufficient evidence that its build passed.

This is one of the strongest isolated experiments because code can prevent
the collision outright. Better natural-language summaries are secondary.

### 4. Source discovery still takes several hops

A mobile scrolling investigation followed the feature through chat state,
tab presentation, conversation handling, and the shared native renderer
before applying a narrow fix. The useful preparation is a path through the
behavior, including the event producer, state transition, view projection,
native callback, and tests.

A contrasting child had a good commit-and-path briefing and still found a
real unwired host entrypoint. That discovery was valuable. A brief should
accelerate checking integration boundaries, not assert that a plausible
implementation is reachable.

**Prepare:** exact symbol locations, callers and callees where resolvable,
Cargo ownership, nearby tests, relevant changes, and unresolved edges.
Tree-sitter can extract declarations, imports, attributes, and spans.
Text matches, syntax relationships, resolved references, and observed runtime
reachability must retain different labels.

Another mobile fix followed a passing suite because an upgraded installation
restored old persisted UI state. A useful brief should connect state loading
and migration to clean-install and upgraded-install fixtures. Existing passing
checks are evidence only for the cases they actually exercise.

The [earlier syntax experiment](briefing-syntax-results.md) is a warning:
complete required-span hits rose from 1/18 to 3/18 overall, while the two
real-issue cases regressed from 1/6 to 0/6. Better extraction cannot rescue a
candidate pool that misses the right file. Exact identifier-to-file lookup
and cross-layer candidates should precede another excerpt-only treatment.

### 5. Running software differed from the source being discussed

The coordinator found a stale host unable to read current task storage,
updated it, and updated again after another change landed. A later automatic
restart interrupted an active chat. Another launch used the wrong working
directory. These are distinct from whether the code in the checkout passes.

**Prepare:** checkout and remote revisions, CLI/host/engine build identities,
task-store schema, working directory, active jobs, deployment owner, and
restart policy. Attach observation time and source to each fact.

A briefing can expose mismatches before investigation. Coordinated deployment,
compatibility checks, and safe restart behavior still require product code.
Do not credit a document with solving those runtime defects.

### 6. The agent retrieved the wrong historical assessment

The user described an earlier assessment through several remembered clues.
The coordinator found a real study, presented it, then widened the search
and rewrote the audit after the user clarified that it was a different
assessment. A numerically valid artifact can still answer the wrong question.

**Prepare:** the user's discriminating clues, candidate artifact identities,
matched and missing clues, searched source families, prior exclusions, and
exact evidence pointers. Index conversations, commits, issues, and retained
results as separate source types.

This is a plausible System One task: select among candidate records using
explicit criteria, preserve uncertainty, then have code retrieve the exact
selected artifact. A scalar confidence score without clue-level evidence
would make premature selection harder to detect.

### 7. The required user journey changed the acceptance test

A plugin request was initially delegated as direct implementation. The user
then specified that it should be created and registered through OpenAgents
as a user would. The coordinator redirected the work to exercise that flow.

This is a clarification, not proof that the earlier instruction was ignored.
Once clarified, it should survive every handoff: requested surface, execution
path, allowed direct repairs, resulting artifacts, and evidence of completion.
The same principle applies to a visual symptom: retain what the user did,
what they saw, and what should happen, not only the feature's noun.

### 8. Many delays need scheduling or product fixes

The coordinator repeatedly investigated disk pressure and moved work after
another out-of-space failure. It also reported a long Boat no-op despite a
prepared template. A template's existence does not establish compatibility
of restored artifacts, target paths, permissions, or dependencies.

**Prepare:** available capacity, current reservations, expected build size,
target compatibility, prior failure, and the condition that changed before
retry. Admission and scheduling must consume those facts.

The review also found stale monitor notifications for completed work. Use
task-generation IDs and terminal-state tracking to suppress obsolete events
while preserving new failures. Requested board upkeep remains useful work;
its repeated waits are not established waste.

Repeated tests also provided useful evidence. The Wasm replay implementation
needed equality-trait and fixture fuel-budget repairs; its reruns followed
edits. A task-store child repeated checks after an intermittent lock failure
and found a remaining affected test path. These are reasons to preserve
targeted verification, not to reward a lower test-call count by itself.

## A briefing should have three independently refreshed parts

| Part | Contents | Invalidation |
| --- | --- | --- |
| Repository evidence | Source spans, symbols, package ownership, tests, recent relevant history, applicable instructions | Blob, path, manifest, configuration, or instruction changes |
| Execution and ownership | Claims, current task, runtime identities, build slot, prerequisites, artifact paths, capacity | Claim/task transitions, deployment, environment changes, reservations; recheck at action boundaries |
| Intent and attempt memory | Complete request, accepted corrections, acceptance path, failed approaches, passed checks, unresolved questions | User steering, new result, changed source or environment, superseded decision |

Each fact needs a source, observation time or immutable revision, scope,
validity rule, and status such as observed, reported, inferred, or unknown.
Attempt records should name the next unresolved question. Preserve failures
without encouraging retries whose inputs and relevant conditions are unchanged.

Historical prose can nominate candidates. It cannot establish current issue
ownership, a running binary revision, or a passing check. Likewise, a passing
check applies to its recorded source and environment; it is not a permanent
property of a package.

For the under-one-second preview, precompute stable repository facts and
assemble from already observed operational records. Report their age.
Network refresh, cold indexing, model judgments, and compilation need separate
timings. If a required fact is stale, return an explicit refresh requirement
or refresh it before execution; a fast display must not imply fresh admission.

## Components to test independently

These are proposed tests, not measured improvements. Start with inexpensive
deterministic cases before buying more agent runs.

| Priority | Component | Isolated test | Evidence of success |
| --- | --- | --- | --- |
| 1 | Execution manifest and command selection | Cases for nested workspaces, absent tools/includes, wrong cwd, supported waits, and changed environment identity | Correct command or explicit blocker; no repeated known-invalid command; invalidated workaround after environment change |
| 1 | Run-owned artifacts and diagnostics | Interleave two concurrent mock builds; include failures before and after long output | No cross-run result attribution; receipts preserve request identity and actual exit status |
| 1 | Claim and terminal-state refresh | Replay open→claimed→closed transitions and unrelated parallel tasks | Duplicate work refused/cancelled; unrelated work remains admitted; measured detection delay |
| 2 | Attempt and correction memory | Replay prefixes before repeated workspace mistakes or obsolete notifications | Relevant correction survives a handoff; stale correction/event is rejected; next action cites current evidence |
| 2 | Behavior-oriented source retrieval | Mobile callback chain and unwired-entrypoint cases at pre-fix revisions | Recall of required integration edges and test spans; exact provenance; explicit unresolved edges |
| 2 | Historical artifact selection | Multiple similar studies with differing clues and outcome scopes | Correct artifact or abstention; all selection claims trace to candidate evidence |
| 3 | Semantic evidence ranking | Identical high-recall candidates with deterministic versus System One ranking | Additional requirement coverage per byte and per dollar without unsupported evidence |
| 3 | Compact handoff versus retained session | Matched executors, task, tools, cache policy, budget, and independent checks | Accepted result at lower total cost or elapsed time, with no increase in missing constraints |

### A replay panel from these conversations

Create fixtures at the decision points before the observed outcome. Include
the workspace-command recurrence, duplicate issue work, shared build log,
missing compiler/include path, cross-layer UI search, wrong historical study,
and required product journey. Include successful claim admission and useful
integration discovery as counterexamples so the evaluator does not reward
blocking everything or skipping investigation.

Only evidence available at the cutoff may enter a brief. Later corrections,
patches, task outcomes, and passing results can provide labels. They cannot
be retrieval inputs. When a reliable historical environment or issue snapshot
is unavailable, label the case reconstructed and exclude it from strict
historical effectiveness claims. Keep private raw history and replay fixtures
local; use separately authored fixtures for public test suites.

Use four treatments in stages: current context; deterministic source evidence;
source plus execution/intent/attempt records; and that same candidate pool
with semantic selection. Isolate each component before combining them. Hold
the optional evidence budget fixed, and preserve the full task and required
instructions in every arm.

Measure first useful action, invalid setup attempts, wrong-run result reads,
duplicate work actually started, forgotten corrections, missing evidence,
independent correctness, and total cost through acceptance. Record preparation
and refresh cost, cache behavior, failed attempts, and checking time. Parallel
tool durations cannot simply be added to obtain completion time.

## Recommended next step

Extend the existing preview with **an execution manifest and a short attempt
record**, then test them against the workspace-command and build-log cases.
Their facts are cheap to establish and their failures are concrete. Add live
ownership refresh through the existing claim mechanism before prospective
issue execution. Next improve identifier-to-file and integration-edge
retrieval. Run System One ranking after candidate recall is adequate.

The key hypothesis is that preparing the right facts can prevent false starts
and preserve corrections. The conversations support testing that hypothesis.
They do not support attributing all searches, test reruns, build waits, or
large cached contexts to waste.

[overlap-claim]: https://github.com/OpenAgentsInc/openagents/issues/10195#issuecomment-5962621464
