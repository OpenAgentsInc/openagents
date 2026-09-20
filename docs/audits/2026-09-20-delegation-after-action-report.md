# After-action report: stalled issue burndown and missing task history

Date: 2026-09-20. Author: the supervising Codex agent.

## Outcome and responsibility

The requested outcome was to finish the prerequisites for Coder-managed Devin
workers, then supervise and merge their work until all OpenAgents issues were
honestly complete. I did not deliver that outcome. At this review, GitHub reports
34 open issues. The user reports approximately five hours of work and an
unacceptably small number of completed issues, together with missing visible
chat history for the second time.

The primary execution failure was mine: I let prerequisite implementation,
worker recovery, and repeated integration review consume the run without a
reliable completion cadence. Useful code landed, but I did not reach the
requested operating mode of Coder doing issue work while I mostly supervised
and merged. A successful six-worker episode answered read-only questions; it
was not a six-issue implementation run. I also left its publication unfinished.

The missing chat history is a separate incident. There is evidence of a
history retrieval or display discrepancy, but not enough evidence to name the
underlying Codex defect or claim that the messages were deleted. Missing UI
history does not excuse the execution failure.

This report records the failure rather than resuming the backlog. The broader
goal was explicitly paused after the user's instruction to summarize and stop.

## Evidence and limits

I checked these sources when preparing this report:

- Local Git history and working-tree status, and the remote `main` reference.
  Both local HEAD and remote main were `34df6bc026aa68947979f172614a2ae533e4ed77`
  before this report.
- GitHub issue state and closure timestamps, retrieved with `gh issue list`.
- The retained supervisor prompts, result files, test logs, and checkpoint
  document under the machine-local `/tmp/openagents-supervision/` directory.
- The captured live CoderBench grade and trace.
- The task-history API and metadata from the local JSONL conversation log.
- Uncommitted changes in the main checkout and the gate and SDK worktrees.

Commit times below are America/Chicago, UTC−05:00. They establish when commits
were recorded, not how every minute was spent. Git history includes work from
other agents, so a commit count is not a measure of this agent's productivity.
Supervisor attempts overlap; adding their durations does not produce elapsed
wall time. Retained attempt files are not guaranteed to be an exhaustive
accounting of every invocation.

The diagnostic logs remain local. They can contain private conversation,
provider instructions, or machine details and must not be uploaded wholesale.
This report preserves the relevant findings without publishing those contents.

## What actually completed

GitHub records these seven closures between 23:34 on September 19 and 02:24
on September 20. This is a verifiable interval, not a claim that all seven were
completed entirely by this agent or that the user's count was unreasonable.

| Issue | Closed, local time | Delivered result |
| --- | --- | --- |
| [#9442](https://github.com/OpenAgentsInc/openagents/issues/9442) | Sep 19, 23:34 | Coordinated worktree creation and cleanup to prevent missing delegations. |
| [#9422](https://github.com/OpenAgentsInc/openagents/issues/9422) | Sep 19, 23:59 | Bound relay feedback and results to the signed current job. |
| [#9421](https://github.com/OpenAgentsInc/openagents/issues/9421) | Sep 20, 00:39 | Classified Kev refusals and reconciled the retained measurement evidence. |
| [#9430](https://github.com/OpenAgentsInc/openagents/issues/9430) | Sep 20, 01:26 | Removed tracked generated Swift and Python artifacts. |
| [#9420](https://github.com/OpenAgentsInc/openagents/issues/9420) | Sep 20, 01:57 | Serialized and durably recorded locked-partition reads. |
| [#9419](https://github.com/OpenAgentsInc/openagents/issues/9419) | Sep 20, 02:24 | Preserved selected-answer provenance through calibration and rechecked measurements. |
| [#9394](https://github.com/OpenAgentsInc/openagents/issues/9394) | Sep 20, 02:24 | Resolved the related Score wire-contract mismatch. |

The final two closures came from related implementation work. They are two
issues, not two independent development efforts. Increasing the count through
that distinction would not change the poor throughput.

Several prerequisite changes landed without completing their containing issues:
capability approval, a filesystem write boundary, independent workspace
snapshots, runtime enforcement, dependency policy, and the manual Rust gate.
Those changes explain some effort, but are not substitutes for issue completion.

## Timeline of the critical path

| Time | Evidence | What it means |
| --- | --- | --- |
| Sep 19, 23:34 | `6160c140ab` | Fixed a real worktree metadata race before trusting concurrent delegation. |
| Sep 19, 23:59 | `aa10ab30b4` | Landed signed relay-job binding. |
| Sep 20, 00:04 | `dff056763a` | Made the benchmark check manifest-owned expected answers. |
| Sep 20, 00:49 | `b14cfedb83` | Added the filesystem boundary component. Integration remained. |
| Sep 20, 01:03 | `4010baadbc` | Added shared, pinned host approval for capability probes. |
| Sep 20, 01:35–01:36 | `54d33bec05`, `1aca30819a` | Added independent workspace snapshots and distinct escaped path labels. |
| Sep 20, 01:55 | `9ec50704cb` | Landed the locked-partition ledger fix. |
| Sep 20, 02:22 | `ed1cc8c8d3` | Landed the calibrated-answer wire-contract fix. |
| Sep 20, 02:31–02:38 | `b91de03c42`, `b64585ce0c` | Landed dependency and Rust verification policies, still with open acceptance work. |
| Sep 20, 02:41 | `693e9d10ee` | Integrated delegation boundaries and dispatch-time approval checks. |
| Sep 20, 02:42 | `34df6bc026` | Published a handoff note before the final live proof. |
| Sep 20, about 02:45 | Live trace session `20260920T074515Z-9a4668b0` | Six real Devin sessions completed under Coder; the live grader passed in 48.4 seconds. |
| At the stop | Dirty main checkout and failing harness log | The observed golden and documentation were still uncommitted. |

The live grade records six answers verified against the task's expectations,
an unchanged workspace, and no faults. The trace SHA-256 is
`be816833193205cb511af43326f8aa7972d94aaf81ce6c1708566edb8d23c0db`.
That is meaningful evidence for local read-only delegation. It does not prove
relay execution, conflict recovery for writing tasks, or completed issue work.

## Why the work stalled

### 1. I let prerequisites become the dominant project

The original audit exposed genuine safety and correctness requirements. It was
right not to run unattended work under unenforced declarations, count unknown
answers as correct, or treat worktrees as a sandbox. However, I did not keep a
small, explicit critical path to the first supervised issue implementation.
Capability trust, filesystem enforcement, snapshots, runtime admission, adapter
verification, calibration, and toolchain policy became concurrent projects.

The resulting work improved the repository, but kept moving the point at which
the requested workflow could begin. I should have distinguished requirements
for a supervised, manually selected issue from requirements for unattended
whole-backlog execution, and maintained an acceptance checklist for each.
That distinction must not weaken required enforcement.

### 2. Worker execution repeatedly stopped without usable completion

There are 39 retained supervisor result files: 30 report exit code 0, two report
124 after approximately 1,800 seconds, and seven report −15. Their recorded
aggregate duration is about 7.23 worker-hours, with overlap. This is neither a
billing estimate nor five hours of sequential execution.

Exit code 0 was not a reliable success signal. For example, the SDK and gate
logs contain a noninteractive tool-confirmation rejection followed by a stopped
attempt. SDK attempts consumed about 131 and 346 seconds before later resumption;
the gate implementation also stopped after making a partial patch. Some −15
results were deliberate supervisor interruptions for review, not crashes.

I inspected failures and resumed narrower, file-only tasks, but recovery was
reactive and repeated. I did not establish a dependable worker execution contract
early enough. The correct response is not to disable approval safeguards. It is
to prove the permitted tool path in a small preflight and classify terminal
outcomes by patch and verification evidence, not process exit alone.

### 3. Delegation shifted substantial implementation back to the parent

Worker patches needed consequential review and repair. Recorded examples include
approval by directory location, trusting declared executor bounds, alias paths
that could expose approval metadata to writes, cancellation and worktree-lifetime
handling, and ledger durability and alias locking. Parent fixes also addressed
compile errors and test assumptions.

Review found real defects and was necessary. The process failure was repeatedly
accepting scopes large enough to discover interface and invariant disagreements
late. I became the integration engineer and test runner for several unfinished
branches, rather than primarily reviewing and merging completed issue patches.

A later gate review illustrates the problem: a previously recorded digest alias
could still match after policy thresholds changed. A failing regression exposed
that the migration could misattribute historical evidence. That issue correctly
remained open, but the flaw should have been part of the initial acceptance
contract rather than a late review surprise.

### 4. Verification and recovery lacked a completion budget

Some repeated verification was justified: changes crossed crate boundaries, and
an earlier shared Cargo target produced stale-artifact concerns. Isolated target
directories were the right correction. Broad checks also caught genuine lint and
integration failures.

However, I did not maintain a concise matrix of source revision, changed
invariant, required check, and completed result. The retained directory contains
many overlapping test and review logs. There is insufficient timing evidence to
assign an exact percentage of the delay to redundant tests. The defensible
conclusion is that verification management was fragmented and the parent was a
bottleneck, not that any particular full test run was unnecessary.

### 5. I failed to finish and publish near-complete work

The strongest example is the live golden. The actual episode passed, but the
subsequent CoderBench test run had two failures:
`a_run_judges_the_trace_it_captured` and
`the_right_commit_holds_the_requirement`.

The harness fixtures omitted independent expected answers and still expected a
pass from the replacement trace. The observed trace did not assert correctness,
so the grader correctly returned unverifiable. The log reports ten harness tests
passed and two failed. This was unfinished fixture migration, not evidence that
the live run failed or permission to weaken the grader.

At this report's start, main contained ten modified tracked files plus new
observed-golden artifacts and measurement documents. The gate and SDK worktrees
also contained uncommitted patches. Both latest worker result records were
terminal, with exit code 0; that establishes neither reviewed correctness nor
merge readiness. I should have brought a completed worker result through review,
verification, commit, push, and acceptance before expanding work in progress.

### 6. Progress reporting did not provide durable accountability

The local checkpoint file accumulated long append-only updates. Its first state
was obsolete; later entries superseded it. Important evidence lived under `/tmp`,
and the latest live proof had not reached a committed report. That made recovery
harder and left the user dependent on a chat UI that was not showing history.

I should have maintained a short, current, committed status with completed issues,
remaining blockers, active worker identities, and links to accepted evidence.
Frequent commentary does not replace that record. A successful experiment that
remains unpublished is not a delivered result.

## Missing Codex chat history

The user's report is that visible history disappeared twice. During this review,
the task-history API returned the correct task and recent turn records, including
two completed turns, but their `items` arrays were empty. It also reported that
older pages existed.

Separately, the local conversation JSONL still existed and parsed without invalid
JSON rows. At inspection it contained about 32.1 MB, 5,700 records, 161 assistant
message records, 36 user message records, and eight compaction records. The first
record was at 2026-09-20 02:24:52 UTC. The file was still growing during this report.
These are structural counts, not proof that every earlier visible message remains
complete or that every record should render as a chat bubble.

The two observations support a history retrieval, projection, or rendering
problem rather than a conclusion that the entire local conversation was erased.
I cannot distinguish those causes from the available evidence. Compaction is
present, but correlation does not establish it as the cause. I did not inspect
a failing UI render or obtain an application stack trace. I have not repaired the
app or submitted a product bug report, and this report must not claim otherwise.

A useful product bug investigation should preserve the original local log,
record the app version and affected turn IDs, compare API pagination with the
persisted message records, and inspect application errors around the failure.
Any support export must be reviewed for secrets and private content. Do not
clear app data or delete the conversation as a speculative fix.

## Corrective operating procedure

These are proposed corrections, not completed remediation:

1. **Limit work in progress.** Start with two implementation workers. Increase
   concurrency only after complete cycles produce verified, merged issue fixes.
   Count accepted issues, not active sessions or generated patches.
2. **Define the first deliverable before dispatch.** Give each worker the exact
   invariant, owned files, interface constraints, regression to reproduce, and
   acceptance evidence. Split large cross-crate work at explicit interfaces.
3. **Preflight the execution path.** Confirm a worker can read, edit, and return
   a small artifact using permitted tools before assigning a long task. Treat
   approval rejection, timeout, no patch, and verified completion as distinct
   terminal outcomes. Do not bypass approval protections.
4. **Resolve repeated failure immediately.** After a repeated execution-contract
   failure, change the task or supported tool path instead of issuing essentially
   the same attempt. Preserve the partial patch and exact failure evidence.
5. **Finish before expanding.** Review and test terminal worker patches promptly.
   Publish the accepted change and issue evidence before starting another scope
   on that dependency chain. Keep unfinished experiments off main when practical.
6. **Track verification by revision.** Record which invariant each check covers,
   its source revision, and result. Repeat checks when changes invalidate them.
   Retain the full required manual gate; do not equate a narrow test with it.
7. **Keep a durable status record.** Update a concise committed checkpoint after
   each accepted merge and report completed issues, unfinished work, and the next
   concrete action. Keep detailed logs separate, with reviewed evidence summaries.
8. **Use the intended architecture honestly.** Begin issue-writing delegation
   with manually selected nonconflicting work, hosted Jev, enforced boundaries,
   and the independence judgment in shadow as the brief requires. Do not call a
   read-only benchmark an issue burndown or a local run a relay proof.
9. **Close only against acceptance evidence.** Preserve missing evidence as
   unknown. Do not inflate output by closing partially implemented issues.

## State left for the next authorized work session

- The broad backlog goal is paused. This report does not resume it.
- The observed-golden publication is unfinished, including the two fixture
  failures described above. Preserve the raw successful trace unchanged.
- The gate digest and SDK deadline branches contain unreviewed work. Their
  terminal process results are not an acceptance decision.
- The dependency-policy work still has an unresolved repository-license choice.
  That is a specific decision, not a justification for stopping unrelated work.
- Relay proof and writing-task conflict recovery remain separate from the local
  six-answer demonstration. The 34 open issues remain the authoritative backlog.
- User-owned `.claude/` files and existing unfinished source changes are outside
  this report's commit and must remain untouched.

The failure was not a total absence of useful engineering. It was failing to
convert that engineering into the requested, observable issue-completion process
at an acceptable pace. I am responsible for that planning, execution, and
publication failure. The chat-history discrepancy compounds the loss of
visibility and needs its own evidence-based product investigation.
