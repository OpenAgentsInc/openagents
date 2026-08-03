# Loupe versus OpenAgents: prompt, source, and runtime gap assessment

Date: 2026-08-03
Status: source-grounded reference assessment; not product, release, deployment,
reporting, or adoption authority

## Executive answer

Loupe has a better **discovery-task loop** than the currently exposed Omega
entropy task. It gives the agent a precise vulnerability-hunting workflow,
typed finding and deduplication tools, a required regression-test candidate, a
separate verdict-before-fix pass, and a durable server-side finding lifecycle.
Those are the parts worth absorbing.

Loupe does not have a stronger end-to-end forensic truth model than the
OpenAgents target design. It silently omits submodules, has no coverage
manifest, checks only whether a proposed test diff applies, can mark a scan
successful after scanner failures, gives the verifier no copy of the original
PoC, and does not bind findings to the prompt, model, source-completeness,
worker, usage, or evidence receipts needed by OpenAgents.

The phrase “Loupe system prompt” is also imprecise. Loupe supplies a **task
prompt** as the positional prompt to Codex or Claude CLI. Codex and Claude keep
their own provider-owned system instructions. Omega likewise sends its
file-analysis request as a user-role message, while its visible entropy action
opens a normal agent task with a user message. The transferable artifact is
therefore Loupe’s task and tool protocol, not a provider system prompt.

There are three different OpenAgents truth layers that must not be conflated:

1. The **current Omega entropy surface** opens a normal visible agent task with
   readable Markdown instructions. Its defensive fallback performs sequential,
   typed, one-file model calls with no tools.
2. The **OpenAgents forensic contracts and adapters** already define immutable
   prompt artifacts, findings versus hypotheses, coverage manifests, managed
   workers, evidence receipts, exact cleanup, and independent verification.
3. The **live managed forensic program is not accepted yet**. Open issues
   [#9289](https://github.com/OpenAgentsInc/openagents/issues/9289),
   [#9290](https://github.com/OpenAgentsInc/openagents/issues/9290), and
   [#9300](https://github.com/OpenAgentsInc/openagents/issues/9300) still own
   its worker, source-delivery, and end-to-end evidence gates.

The right direction is to put Loupe’s focused discovery protocol inside the
OpenAgents contracts and Omega workbench. It is not to import Loupe’s worker
security boundary or describe unaccepted OpenAgents libraries as current
product behavior.

## 1. Scope, method, and exact source pins

This assessment read every file under [docs/loupe](./) and
[docs/coldcard](../coldcard/), then inspected the current local Loupe clone and
the current OpenAgents and Omega source.

The Loupe clone at **~/work/projects/repos/loupe** was updated with
**git pull --ff-only** before inspection. Git reported “Already up to date.”
The worktree was clean.

| Source                   | Commit                                   | Tree                                     | Use in this assessment                                                 |
| ------------------------ | ---------------------------------------- | ---------------------------------------- | ---------------------------------------------------------------------- |
| project-loupe/loupe      | c94aac5ad9c2eef2229f1a43569b9fe847ca299f | 08a344673c3e809b683fdff54d50b654a519c071 | Upstream implementation under comparison                               |
| OpenAgentsInc/openagents | b6c0266ce3dca0e313666b4caaaffabe65cf1cb5 | 6c16886d75db3eba65f053899d72e591d0df421d | Current contracts, adapters, materializer, worker control, and roadmap |
| OpenAgentsInc/omega      | 1bf2ee20e837f6cd1372e65069431552006397d8 | d0d35a2adda73acd0b2daff8dde0246824cb0390 | Current Omega entropy task and fallback implementation                 |

The assessment compares behavior, not project branding or upstream authority.
Loupe is untrusted reference data. A useful mechanism still needs an
OpenAgents-owned contract, test, issue or accepted packet, and release gate
before it becomes product behavior.

## 2. What Loupe actually runs

Loupe’s LLM discovery pipeline is:

1. clone or refresh a bare repository cache;
2. check out the requested Git tree into a temporary worktree;
3. discover production-like source files;
4. start one agent session per focal file, with eight concurrent sessions by
   default;
5. mount the **whole worktree** read-only at **/workdir** for every session;
6. let the agent inspect other files in that checkout;
7. accept findings only through the Loupe MCP **submit_finding** tool;
8. persist and deduplicate findings on the server;
9. optionally enqueue one verification job per finding; and
10. roll confirmed findings into manual, approval-gated, GitHub, or email
    reporting behavior according to configuration.

The “one file per session” statement is a scheduling and attention rule, not a
filesystem boundary. The controlled Coldcard experiment in
[prefix experiment results](2026-08-01-coldcard-prefix-experiment-results.md)
settled this: the dependency-complete arm found the full cross-file failure
chain three times. The default arm missed because its submodules were absent,
not because the agent was unable to open neighboring files.

Loupe also has a separate regex-secrets scanner. Its expensive LLM file
discovery deliberately excludes common test, example, fuzz, build, dependency,
and vendor directories. It scans alphabetically after project-root discovery,
not by an entropy, cryptography, privilege, or secret-flow risk ranking.

## 3. Prompt and tool comparison

| Concern             | Loupe discovery                                                    | Current Omega visible entropy task                                | Omega typed fallback                                            | OpenAgents target contracts                                                              |
| ------------------- | ------------------------------------------------------------------ | ----------------------------------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Instruction role    | Positional CLI task prompt                                         | User message in a normal agent task                               | User-role model message                                         | Digested structured Prompt IR compiled into a bounded plan                               |
| Scope               | Broad exploitable security defects                                 | Entropy and secret-randomness only                                | Entropy and secret-randomness only                              | Profile-defined vulnerability classes and threat model                                   |
| Unit of work        | One focal file, whole checkout readable                            | One repository task                                               | One supplied file per model request                             | Ranked tranches over a bound source bundle                                               |
| Source completeness | Assumed; no submodule preflight                                    | Agent told to initialize catalog submodules                       | Dependency status recorded after local inspection               | Every declared submodule needs a plan; required absence is incomplete                    |
| Candidate workflow  | Enumerate, dedup, produce test diff, apply-check, submit           | Traverse, cite, separate findings and hypotheses, return Markdown | Return typed observations, hypotheses, and limitations          | Typed findings, hypotheses, evidence, claims, and events                                 |
| Emission            | Only MCP **submit_finding** creates a finding                      | Final Markdown is not ingested as a finding                       | JSON is parsed into an entropy projection                       | Only typed finding/hypothesis submissions create envelopes                               |
| Uncertain lead      | Prompt says submit it as a finding with assumptions                | Put it under hypotheses or limitations                            | Dedicated hypothesis array                                      | Dedicated hypothesis schema and promotion lifecycle                                      |
| Deduplication       | Agent semantic search plus source-window hash                      | None in this task path                                            | None                                                            | Not yet a complete live identity service; contracts can retain exact refs                |
| PoC                 | Regression-test diff required; apply-check only                    | Not required                                                      | Not required                                                    | PoC and executed vulnerable/fixed controls are separate evidence                         |
| Verification        | Separate task, verdict locked before optional patch                | None                                                              | Citation validation only                                        | Distinct verifier identity and receipt-gated control execution                           |
| Output provenance   | Repo/job/finding IDs, source-window fingerprint                    | Repository, path, HEAD, prompt digest in prose                    | Run, file, file digest, prompt digest, model route, source refs | Source, coverage, prompt, model, parameters, worker, evaluator, usage, and event digests |
| Reporting           | Configurable automatic or manual reporters                         | None                                                              | None                                                            | Private and manual by default; disclosure is typed                                       |
| Network authority   | Prompt says no Internet; backend actually shares network           | Normal Omega task capabilities                                    | Direct model-provider call; no agent tools                      | Broker-only managed-worker policy                                                        |
| Failure truth       | Per-session errors counted, but runner can swallow scanner failure | Agent reports limitations in prose                                | Per-file typed failure and completed-with-limitations           | Typed failure, incomplete, cleanup, recovery, and censoring states                       |

### 3.1 Loupe’s discovery task is materially sharper

The upstream discovery task contains several useful controls that the current
Omega prompt does not yet encode:

- a concrete list of vulnerability classes;
- severity-ordered candidate enumeration;
- a one-candidate-at-a-time loop;
- semantic prior-finding search before every submission;
- one finding per root vulnerability;
- explicit exclusion of style, hardening, and non-testable notes;
- conservative severity language;
- a typed tool as the only emission path;
- a regression-test diff requirement; and
- explicit rules for off-tree uncertainty and unsupported cross-reference
  claims.

Omega’s visible task is clearer about target identity, exact HEAD, dirty
worktrees, cross-file traversal, citations, observations versus hypotheses,
artifact limitations, and readable final structure. It does not turn those
instructions into typed workbench records. The task can finish with excellent
Markdown and still create no finding, hypothesis, evidence receipt, or run
event in the forensic data model.

### 3.2 Omega’s fallback is typed but much narrower

The fallback path freezes a deterministic manifest, sends one file’s source
text per request, requires typed JSON, checks its schema, and validates cited
paths, commits, content digests, line ranges, and optional symbols. That is a
real mechanical advantage over Loupe’s finding intake.

It is not equivalent to Loupe’s agent session:

- it sends no tools;
- it gives the model one source value rather than a readable checkout;
- it has no prior-finding search;
- it does not ask for a regression test;
- it does not independently verify candidates; and
- it classifies source observations and hypotheses, not general security
  findings.

The fallback is also defensive. A selected working folder normally takes the
visible-agent-task path first. Product descriptions should not attribute the
fallback’s typed validation to every visible entropy task.

### 3.3 Loupe’s verification prompt has one excellent ordering rule

The verifier must submit exactly one initial verdict before it may propose a
patch. This prevents the verifier from starting a fix and then rationalizing
the original finding to match its own work. The patch is optional and the
prompt tells the verifier to skip it when the change is uncertain, broad, or a
design decision.

That ordering belongs in the OpenAgents verifier experience. Loupe’s evidence
standard does not.

## 4. What Loupe does that the current Omega surface does not

### 4.1 A complete discovery protocol, not only a topic prompt

Omega’s editable entropy prompt says what to inspect. Loupe additionally says
how to turn inspection into bounded candidate work: enumerate, rank, search,
falsify, apply-check, emit, and continue. This is the largest immediately
useful prompt gap.

### 4.2 Typed in-session finding intake

Loupe does not parse prose after the agent finishes. The **submit_finding** MCP
call is the state transition. Omega’s visible task asks for Markdown, while the
typed fallback parses a whole response after generation. A typed in-session
submission boundary would let Omega:

- show findings as they arrive;
- validate each finding independently;
- reject one malformed finding without losing the rest;
- preserve first-identification time;
- stop or redirect work after a qualified hit;
- separate diagnostic prose from product state; and
- drive a verifier without reparsing chat text.

### 4.3 Prior-finding retrieval during analysis

Loupe exposes semantic search and full finding lookup to the agent. The
mechanical hash is a last line of defense. Omega’s entropy paths have no
equivalent historical query in the task loop, so repeated runs can rediscover
the same candidate without knowing its prior disposition.

### 4.4 A durable repository/job/finding service

Loupe has leases, worker identities, retries, encrypted storage, scan history,
finding states, verification jobs, approval, and reporter dispatch. Omega has
far richer forensic data types, but the current entropy slice is primarily a
workbench and task-launch path. The accepted live service loop is still open.

### 4.5 A second-opinion loop connected to each finding

Loupe automatically enqueues verifier work after a successful scan when
verification is enabled. Current Omega entropy output is not automatically
turned into a verifier assignment.

### 4.6 Simple operational configuration

Loupe exposes model, effort, concurrency, timeout, extensions, excluded paths,
verification, approval, and reporter behavior through deployable server and
worker configuration. OpenAgents has stronger typed policies, but current
Omega does not give the operator an equally direct per-run view of which of
these knobs are effective.

### 4.7 A companion deterministic scanner

The regex-secrets scanner is independent of the LLM review. OpenAgents should
preserve this composition principle: cheap deterministic observations should
run beside model work and feed the same evidence graph, not be reimplemented as
prompt instructions.

### 4.8 A mechanically read-only checkout

Loupe’s Bubblewrap mount makes the checkout read-only even though the inner
agent CLI runs with its own permission checks bypassed. The current Omega
visible task says “read-only” and “do not modify repository source,” but it is
a normal agent task rather than the managed forensic sandbox. Its effective
write, command, and network capabilities come from that task’s ordinary agent
runtime, not from the forensic prompt.

The defensive Omega fallback reads frozen source and calls the model without
tools, which is narrower. The accepted product needs Loupe’s mechanical
read-only property combined with OpenAgents’ stronger source, credential,
network, and cleanup controls.

### 4.9 Optional protocol and history retrieval

When **bkb-mcp** is available, Loupe attaches protocol, specification, related
repository, commit, and historical lookup tools. Its prompt correctly tells the
agent to identify those results as external tool claims rather than direct
source observations.

Omega’s typed fallback has no external-context tool. A normal visible task may
have broader general tools, but it does not bind an external claim to a
forensic tool receipt. The useful pattern is an admitted, cited context tool
whose output remains below direct source and executed evidence.

## 5. What OpenAgents already models better

These advantages exist in checked-in contracts and tests. They are target
capabilities until the live gates close.

### 5.1 Source completeness is a first-class result

The OpenAgents source materializer:

- resolves the exact repository commit and Git tree;
- parses declared submodules and observed gitlinks;
- requires an explicit plan for every declared submodule;
- checks repository URL and gitlink revision against the plan;
- makes excluded or stale declarations explicit;
- records required, optional, absent, generated, excluded, and oversized
  entries;
- binds generated inputs to generator and toolchain pins;
- emits a coverage digest and materialization receipt; and
- returns an incomplete authority with no source bundle when a required input
  is absent.

An important current limitation remains: the implementation detects a nested
submodule and refuses it with “nested submodules require an explicit recursive
dependency plan”; it does not yet execute that recursive plan. This is safer
than silently omitting the dependency, but it is not recursive-submodule
parity. Issue #9290 must close that gap or retain an explicit refusal.

Loupe’s plain bare clone and libgit2 checkout do none of this. The Coldcard
experiment proves the consequence: identical prompts and focal files changed
from miss to hit when dependencies were present.

### 5.2 Findings and hypotheses are different data

Loupe instructs the agent to submit a finding even when an off-tree invariant
is unknown. The description is expected to carry the assumption. That avoids a
false negative, but it pollutes the finding queue and loses a machine-readable
next check.

OpenAgents has a separate hypothesis type with supporting references, missing
evidence, consequence, next check, state, expiration, and promotion link. A
finding carries causal steps, source refs, assumptions, evidence tier, verifier
state, and disclosure state. Omega’s fallback also separates observations,
hypotheses, and limitations.

### 5.3 Applicability is not execution

Loupe’s **validate_poc** and **validate_patch** tools run only **git apply
--check**. That proves the diff is syntactically applicable to the checkout. It
does not prove:

- the proposed test compiles;
- the test fails on the vulnerable revision;
- the failure is caused by the claimed defect;
- the test passes after a fix;
- the relevant artifact contains the vulnerable path; or
- the production configuration can reach it.

OpenAgents records source, artifact, and executed evidence separately. Its
verifier requires a distinct identity, locks the initial verdict, then requires
an admitted-worker PoC receipt, an observed failure on the immutable vulnerable
target, and an observed success on the immutable fixed target before a
confirmation can become independently verified.

### 5.4 Prompt lineage and evaluation are durable

Loupe’s prompt is a source-code constant rendered with a file path and optional
tool hint. Its finding does not retain a prompt digest.

OpenAgents Prompt IR carries the role, threat model, vulnerability classes,
security invariants, evidence requirements, dependency, uncertainty, tool,
PoC, severity, context, schema, and budget policies. The prompt artifact binds
that IR, parent lineage, examples, parameters, dataset revision, and
compatibility refs into a canonical digest.

The optimizer contracts additionally separate train, development, holdout, and
clean-holdout data; prevent holdout visibility during generation; freeze metric
definitions; retain failed examples; and require a gated activation or
rollback. Omega’s entropy prompt snapshot already preserves immutable text,
parent and source-run refs, and a digest, but the visible task does not yet use
the full Prompt IR and evaluation path.

### 5.5 Evidence, lifecycle, and cleanup are typed

OpenAgents binds a run to the target, source, coverage, prompt, model, model
parameters, worker image, worker profile, usage, findings, hypotheses, errors,
and ordered events. Complete and completed-incomplete are different states.
Post-run states require observed zero-residue cleanup.

Loupe has a sound temporary-worktree and Bubblewrap posture for host-file
protection, but it does not have an equivalent run contract for coverage,
model provenance, exact provider usage, evidence tiers, cleanup receipts, or
recovery-required truth.

### 5.6 Coldcard claims have explicit proof rungs

The OpenAgents contracts distinguish a source flaw from:

- artifact selection;
- a state-space model;
- an owned-fixture recovery;
- a program fingerprint;
- an entity cluster;
- unauthorized movement; and
- identity attribution.

Each rung has different required evidence. Loupe’s generic finding and verdict
cannot express those non-implications. A Loupe confirmation can therefore be a
useful source-review vote, but never by itself the stronger Coldcard claim.

## 6. Material discrepancies and defects in current Loupe

### 6.1 Silent source incompleteness

Loupe clones a bare repository without recursive submodules and checks out the
top-level tree. It has no submodule initializer, gitlink inventory, coverage
manifest, generated-input plan, or incomplete result.

This is the most consequential discrepancy because a healthy run can report
zero relevant findings from an incomplete program and look clean.

### 6.2 The prompt’s network claim does not match the capability boundary

Both discovery and verification prompts say the agent cannot access the
Internet or anything outside the worktree. The actual Codex and Claude
backends:

- call **allow_network**, which makes Bubblewrap use the host network
  namespace;
- pass **dangerously-bypass-approvals-and-sandbox** or
  **dangerously-skip-permissions** to the agent CLI;
- expose an API key or read-only login state inside the sandbox; and
- mount system binaries and the agent CLI installation.

The read-only worktree still protects repository bytes from mutation, and the
fresh home and temporary directory are useful. The prompt is not a network
control. A prompt-injected or misbehaving agent has outbound capability and
credential material that should never be considered unavailable merely
because prose says so.

OpenAgents should keep provider secrets behind its provider broker and give a
guest only a short-lived, run-bound, model-bound, budget-bound capability.
Source acquisition should be control-plane work. The analysis guest should
have default-deny egress except to the broker, with network and cleanup truth
measured.

### 6.3 PoC validation is overstated

The prompt says the agent must write a regression test that fails on HEAD, but:

- the MCP server does not enforce that **validate_poc** was called before
  **submit_finding**;
- **validate_poc** only checks whether the diff applies;
- no test command is captured or executed;
- no failed-test output is retained; and
- no fixed control is run.

Docs that call this “proof” need the narrower phrase **applicable proposed
regression-test diff** unless a separate execution receipt exists.

### 6.4 The verifier does not receive the proposed PoC

The verifier serializes severity, title, file, lines, description, and CWE into
its prompt. It omits the finding’s **poc_unified** field. The verifier can
re-read the repository and vote on the description, but it cannot inspect or
challenge the exact regression test that discovery supplied.

This weakens the connection between discovery’s purported falsifier and the
verification verdict.

### 6.5 Intended verifier wiring and observed behavior disagree

Current source implements an atomic end-of-session verdict flush and rejects a
successful verify completion without a stored verdict. The preserved Omega
scan observed 132 findings and zero settled verifier verdicts, with the runner
reporting that the MCP child had already posted.

The correct conclusion is not “the current verifier is broken” or “the current
verifier is proven fixed.” It is:

- source and unit tests express the intended atomic path;
- the historical live run disproved the prior implementation in its deployed
  environment; and
- no new end-to-end acceptance receipt in this corpus establishes that the
  current source closes that exact seam.

OpenAgents must retain the historical failure as a regression case and require
an end-to-end verifier receipt before claiming parity.

### 6.6 Scanner failure can still become successful job completion

The LLM scanner deliberately returns an error when every per-file agent session
errors. Its comment says this prevents “succeeded with 0 findings.”

The runner then catches each scanner error, logs a warning, continues, and
returns success with the accumulated batch. The server’s successful scan gate
requires a non-empty HEAD SHA, not successful completion of every configured
scanner or a coverage/error summary.

Therefore the documented fail-closed intention is not achieved at the job
boundary. Partial or total scanner failure can be recorded as a successful
scan.

### 6.7 Finding intake accepts weaker source truth than the prompt implies

The MCP path normalizes the submitted relative path and blocks traversal, which
is good. It does not require:

- that the file exists;
- that the line range is inside the file;
- that the cited source supports the description;
- that the PoC touches a valid test location; or
- that the agent called its validation tool.

When a file cannot be read, fingerprint generation deliberately falls back to
a hash of the submitted path and line range so the finding can still be
stored. Omega’s fallback is stricter because it checks citations against a
frozen manifest and source bytes.

### 6.8 Finding provenance is too thin for OpenAgents

Loupe’s Finding carries scanner, severity, title, description, file and line
range, CWE, candidate patch, candidate PoC, and fingerprint. It does not bind:

- prompt and prompt lineage;
- model and model parameters;
- source bundle and completeness;
- dependency manifest;
- tool surface;
- worker image or profile;
- assumptions as structured data;
- causal steps;
- evidence tier;
- verification identity;
- disclosure state;
- exact usage and cost; or
- the event sequence that produced it.

Those are not cosmetic fields. They are required to compare models and prompts,
keep incomplete runs out of qualified results, reproduce a claim, and decide
what can leave the private workbench.

### 6.9 Human approval is optional, not invariant

Loupe supports an approval gate, but the server default is false and a
repository can choose its effective setting. A confirmed finding can dispatch
immediately when approval is off.

Any OpenAgents integration must preserve private, manual reporting as the
default and require a separate disclosure authority. It must not inherit
Loupe’s deployment default.

### 6.10 Deduplication can split one root cause

The mechanical fingerprint includes the focal file path and a normalized local
source window. It is stable across many formatting changes and deliberately
unstable across renames. The semantic search is useful.

A cross-file defect submitted from different focal files can receive different
hashes even when the causal chain is the same. Conversely, aggressive
lowercasing and whitespace collapse can merge source windows whose distinctions
matter in some languages. OpenAgents needs an explicit root-cause identity and
causal graph above the source-occurrence identity.

### 6.11 Default scheduling is coverage-oriented, not risk-oriented

Eight-way alphabetical file fanout is easy to operate and broad. It spends the
same initial attention on low-risk production files as on entropy providers,
authentication gates, key derivation, secret consumers, and unsafe boundaries.
Tests, examples, fuzz harnesses, and vendored directories are excluded as
focal work even when they are useful context or evidence.

OpenAgents should retain full eligible coverage while ranking initial tranches
by the selected threat model. It should record which files were focal, merely
read as context, excluded, skipped, oversized, or never reached.

### 6.12 Repository credentials can appear in subprocess arguments

Loupe documents that a repository token is embedded in the clone URL and is
temporarily visible in the Git subprocess argument vector. This is an explicit
known hardening gap. OpenAgents source acquisition should use a brokered or
credential-helper flow that does not expose reusable credentials to process
inspection.

## 7. Documentation reconciliation

The corpus contains both predictions and later experimental results. Read them
in evidence order:

1. [Coldcard prefix experiment results](2026-08-01-coldcard-prefix-experiment-results.md)
   is the strongest direct evidence about cross-file discovery.
2. [Coordination, not scanners](2026-08-01-coordination-not-scanners.md)
   correctly identifies the missing-source and run-truth problem.
3. [Omega first scan preliminary](2026-07-31-omega-first-scan-preliminary.md)
   remains historical evidence of the 132-finding verifier failure.
4. [Would Loupe have caught Coldcard?](2026-08-01-would-loupe-have-caught-coldcard.md)
   and [hardening against AI-assisted attacks](2026-08-01-hardening-against-ai-assisted-attacks.md)
   contain pre-experiment predictions that were superseded.
5. [Loupe in plain words](loupe-in-plain-words.md) is useful orientation but
   now overstates several properties.

Specific corrections:

| Existing statement                                            | Current correction                                                                                         |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Loupe can see only one file                                   | It receives one focal file but can read the whole materialized worktree                                    |
| Cross-file Coldcard discovery was almost certainly impossible | It succeeded repeatedly when the pinned submodules were present                                            |
| The PoC proves the bug                                        | The tool proves only that the proposed diff applies                                                        |
| A human approves before anything is filed                     | Approval is configurable and defaults off at the server                                                    |
| Four files run concurrently                                   | The current source default is eight                                                                        |
| The verifier is fixed                                         | Current source expresses a fix; the preserved live failure still needs a new end-to-end acceptance receipt |
| The verifier is necessarily still broken                      | The historical run proves that deployment failed; it does not prove current pinned source still fails      |

The durable lesson from the entire Loupe and Coldcard corpus is:

> A model result is only as complete as the materialized program, only as
> strong as the executed evidence, and only as trustworthy as the lifecycle
> that preserves failures and limitations.

## 8. Recommended absorption plan

### P0 — unify the prompt boundary

Compile the visible Omega entropy task from OpenAgents Prompt IR and preserve
the user-editable domain text as one bounded field. Add Loupe-derived workflow
fields for:

- candidate enumeration and severity order;
- prior-finding search;
- root-cause identity;
- falsifier construction;
- uncertainty disposition;
- one-finding-per-root-cause;
- continue-after-duplicate;
- no style or hardening submissions; and
- conservative severity.

The compiled prompt must name the exact coverage, tools, unavailable tools,
source bundle, prompt digest, model route, and output schemas. Prompt prose must
never grant tools, network, reporting, or authority.

### P0 — add typed in-session submissions to Omega

Expose OpenAgents-owned tools:

- **query_prior_forensic_work**;
- **get_forensic_work_by_ref**;
- **submit_forensic_hypothesis**;
- **submit_forensic_finding**;
- **submit_forensic_limitation**;
- **validate_candidate_diff_applicability**; and
- a separate executed-control tool available only to the verifier.

Diagnostic Markdown can remain visible in the task, but it must not create
forensic state. Each tool result should update the workbench immediately.

Do not reuse Loupe’s instruction to turn every uncertain dependency claim into
a finding. Route incomplete but actionable leads to the hypothesis lane with a
required next check.

### P0 — make completeness mechanical without blocking task creation

Opening the visible task should stay fast. Catalog tasks may let the agent
perform recursive cloning as current Omega does. Comprehensive analysis status
must wait for a mechanical source inspector to record:

- top-level commit and tree;
- every declared gitlink;
- recursive submodule status;
- required generated inputs;
- excluded and oversized paths; and
- exact incomplete reasons.

The agent can continue on available source while materialization fails. The
workbench must label the run incomplete, and no clean result or qualified miss
can emerge from it.

Managed or benchmark runs should continue to use the stronger OpenAgents
source materializer before inference.

### P0 — close the false-success path

A run summary must count:

- eligible focal units;
- sessions attempted, settled, timed out, cancelled, and failed;
- tools requested, available, denied, and failed;
- findings, hypotheses, duplicates, and limitations;
- files and dependency trees reached; and
- source coverage status.

All-session backend failure must fail the run. Partial failure must produce
completed-incomplete or failed-with-partial-output, never ordinary success.
The server must enforce this, not trust a scanner comment.

### P1 — preserve the verifier ordering and strengthen its evidence

Keep Loupe’s initial-verdict-before-patch lock. Change the verifier input to
include the exact full finding, assumptions, causal chain, source bundle,
coverage, original PoC, discovery actor, prompt digest, and model provenance.

A confirmation must require:

1. distinct discovery and verifier identities;
2. source-reference and dependency checks;
3. the original candidate PoC or an explicitly superseding PoC;
4. successful PoC application;
5. observed failure on the vulnerable revision;
6. observed success on the fixed revision;
7. immutable environment and command receipts; and
8. atomic result persistence before job completion.

An applicable diff remains artifact evidence, not executed evidence.

### P1 — add prior-work search over root causes and occurrences

Keep two identities:

- **occurrence identity** for exact source location and source window; and
- **root-cause identity** for the causal mechanism across files, revisions, and
  products.

Search should include confirmed, dismissed, inconclusive, expired,
superseded, and retained work. A duplicate disposition must not end the task;
it should let the agent continue to lower-ranked candidates, as Loupe does.

### P1 — use brokered model access

Do not expose provider API keys or reusable CLI login state to the analysis
guest. The managed worker should receive only a short-lived provider
capability bound to:

- run and turn;
- provider and model;
- maximum tokens, cost, duration, and network bytes;
- exact source and tool surface; and
- generation-fenced cancellation.

Default-deny egress, provider-broker-only traffic, measured network truth, and
zero-residue cleanup remain hard gates.

### P1 — make disclosure an explicit work transition

Loupe’s reporter adapters are useful reference implementations, but OpenAgents
must keep:

- findings private at creation;
- approval for contact separate from verification;
- public claims separate from maintainer disclosure;
- embargo and owner decisions durable; and
- no automatic reporting inherited from upstream defaults.

### P2 — replace alphabetical spending with evidence-ranked tranches

Start with threat-model-specific boundary maps: entropy sources, provider
selection, key derivation, secret sinks, authentication, authorization,
parsers, unsafe boundaries, and external-input crossings. Preserve eventual
coverage and record all exclusions. Use cheap deterministic scanners and
dependency inspection before expensive per-file sessions.

Prompt and model candidates should be evaluated on matched source, tools,
budgets, repetitions, holdouts, fixed controls, clean controls, and retained
failures. Never promote a faster prompt that loses source completeness,
qualified recall, fixed-control precision, or evidence quality.

## 9. Keep, modify, or reject

| Loupe mechanism                                     | Disposition       | OpenAgents adaptation                                            |
| --------------------------------------------------- | ----------------- | ---------------------------------------------------------------- |
| Whole checkout with one focal file                  | Keep              | Preserve whole bound source; record focal and contextual reads   |
| Typed finding emission                              | Keep              | Use canonical finding, hypothesis, and limitation tools          |
| Continue after duplicate                            | Keep              | Search both occurrence and root-cause identities                 |
| Semantic plus deterministic dedup                   | Modify            | Add causal identity, dispositions, revisions, and evidence refs  |
| Regression-test candidate                           | Keep              | Call it a candidate until executed                               |
| Diff applicability check                            | Keep narrowly     | Label it artifact applicability, never PoC proof                 |
| Verdict before patch                                | Keep              | Enforce in one typed verifier transaction                        |
| Optional patch                                      | Keep              | Separate remediation quality from finding truth                  |
| Eight-way file fanout                               | Modify            | Evidence-ranked, budgeted tranches with eventual coverage        |
| Submit uncertain off-tree cases as findings         | Reject            | Use typed hypotheses and missing-evidence checks                 |
| Bare clone without dependency manifest              | Reject            | Materialize pins or record incomplete coverage                   |
| Prompt-only Internet prohibition                    | Reject            | Enforce broker-only networking mechanically                      |
| Raw provider keys/login state in guest              | Reject            | Use short-lived provider capabilities                            |
| Permission-bypassed CLI in shared network namespace | Reject            | Use admitted managed-worker tool and network policy              |
| Scanner error logged while job succeeds             | Reject            | Enforce terminal counts and false-success guards                 |
| Approval default off                                | Reject            | Private/manual disclosure is the default                         |
| Repository token in Git argv                        | Reject            | Use brokered SCM credentials or a helper                         |
| SQLCipher, mTLS, leases, durable audit              | Keep conceptually | Reuse OpenAgents identity, storage, lease, and receipt contracts |

## 10. Acceptance tests for the combined design

Before claiming that Omega has absorbed Loupe’s relevant behavior, prove:

1. **Dependency A/B:** identical vulnerable target and prompt, absent versus
   materialized submodules; absent is incomplete and complete finds the frozen
   chain.
2. **Whole-tree context:** a focal-file task can cite an exact neighboring file
   from the immutable bundle.
3. **Typed-only emission:** prose containing a fake finding creates no record.
4. **Hypothesis separation:** a missing dependency creates a hypothesis and
   limitation, not a qualified finding.
5. **Duplicate continuation:** the first candidate is a known duplicate and a
   lower-ranked new candidate is still submitted.
6. **PoC applicability:** a malformed diff is rejected without modifying
   source.
7. **PoC execution:** an applicable but passing-on-vulnerable test cannot
   confirm the finding.
8. **Control pair:** the test fails on vulnerable and passes on fixed before
   independent verification.
9. **Verifier input:** the verifier receives and digests the original PoC.
10. **Verdict ordering:** patch work is impossible before the immutable initial
    verdict.
11. **Verifier settlement:** a verify job cannot succeed without an atomically
    persisted verdict and evidence refs.
12. **Total backend failure:** all agent sessions failing cannot complete as a
    clean scan.
13. **Partial failure:** partial source or session failure is visible,
    quantified, and excluded from qualified misses.
14. **Credential isolation:** guest-visible files, environment, process
    arguments, and logs contain no reusable provider or SCM secret.
15. **Network isolation:** only the provider broker is reachable; a direct
    arbitrary HTTPS request fails and is recorded.
16. **Dirty target:** uncommitted files never enter the immutable analysis
    bundle.
17. **Source citation:** missing files, invalid lines, wrong revisions, and
    changed bytes are rejected.
18. **Root-cause dedup:** the same causal defect submitted from two focal files
    becomes one root cause with two occurrences.
19. **Disclosure:** verified does not imply approved for contact or reported.
20. **Cleanup:** terminal success is impossible without generation-fenced
    cancellation and observed zero compute, disk, firewall, process, scratch,
    ingress, and source residue.

## 11. Source evidence index

### Loupe

- [Discovery task and tool workflow](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/prompts.rs#L15-L107)
- [Verifier task and verdict-before-patch workflow](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/prompts.rs#L133-L249)
- [Whole-worktree, per-file scanner orchestration](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/scanners/llm_code_review.rs#L1-L133)
- [Default eight-file concurrency and discovery exclusions](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/source_discovery.rs#L16-L180)
- [Bare clone and token-in-argument note](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/repo_cache.rs#L298-L334)
- [Codex network, credentials, permission bypass, and positional prompt](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/codex_cli.rs#L145-L194)
  and [invocation arguments](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/codex_cli.rs#L339-L356)
- [Claude network, login-state mount, permission bypass, and prompt](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/claude_cli.rs#L180-L266)
  and [invocation arguments](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/llm/claude_cli.rs#L369-L378)
- [Bubblewrap defaults and network opt-in](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/sandbox.rs#L1-L125)
  and [effective mounts and network namespace](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/sandbox.rs#L177-L255)
- [Finding construction and typed submission](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/mcp.rs#L772-L835)
- [Diff applicability check](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/mcp.rs#L837-L859)
  and [git apply implementation](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/mcp.rs#L973-L1020)
- [Missing-file fingerprint fallback](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/mcp.rs#L1022-L1049)
- [Verifier omits the original PoC](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/scanners/llm_verifier.rs#L60-L98)
- [Scanner errors logged and dropped by the runner](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/runner.rs#L293-L324)
- [Server verdict transaction and optional approval](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-server/src/routes/jobs.rs#L400-L505)
- [Server successful-completion gates](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-server/src/routes/jobs.rs#L508-L654)
- [Approval defaults off](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-server/src/state.rs#L39-L49)
- [Thin Finding wire shape](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-core/src/finding.rs#L5-L35)
- [Source-window fingerprint tradeoffs](https://github.com/project-loupe/loupe/blob/c94aac5ad9c2eef2229f1a43569b9fe847ca299f/crates/loupe-worker/src/fingerprint.rs#L1-L64)

### OpenAgents

- [Target, source, coverage, Prompt IR, run, receipt, and event contracts](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/packages/forensic-contract/src/run.ts)
- [Claim rungs, finding, hypothesis, and evidence graph](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/packages/forensic-contract/src/claims.ts)
- [Loupe-style plan compiler and typed discovery adapter](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/packages/forensic-loupe-adapter/src/adapter.ts)
- [Receipt-gated independent verifier](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/packages/forensic-loupe-adapter/src/verifier.ts)
- [Submodule-aware source materializer](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/apps/openagents.com/workers/api/src/forensic-source-materializer.ts)
- [Managed forensic worker control](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/apps/openagents.com/workers/api/src/forensic-managed-sandbox.ts)
- [Brokered provider capability](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/apps/openagents.com/workers/api/src/managed-sandbox-provider-broker.ts)
- [Prompt candidate and holdout governance](https://github.com/OpenAgentsInc/openagents/blob/b6c0266ce3dca0e313666b4caaaffabe65cf1cb5/apps/openagents.com/workers/api/src/blueprint/services/forensic-prompt-compiler.ts)
- [Current implementation and operator boundary](2026-08-01-omega-forensics-implementation-and-operator-guide.md)
- [Canonical forensic roadmap](2026-08-01-omega-forensic-analysis-roadmap.md)

### Omega

- [Visible entropy task prompts and catalog submodule instruction](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/agent_ui/src/agent_panel.rs#L379-L462)
- [Catalog checkout materialization modes](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/agent_ui/src/agent_panel.rs#L464-L564)
- [Dependency inspection, source-reference validation, and typed fallback request](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/agent_ui/src/agent_panel.rs#L979-L1210)
- [Visible catalog task creation](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/agent_ui/src/agent_panel.rs#L14151-L14269)
- [Visible selected-repository task and fallback boundary](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/agent_ui/src/agent_panel.rs#L14285-L14595)
- [Entropy manifest, prompt snapshot, observations, hypotheses, and limitations](https://github.com/OpenAgentsInc/omega/blob/1bf2ee20e837f6cd1372e65069431552006397d8/crates/omega_forensics/src/entropy_repository.rs)

## Conclusion

Loupe’s strongest contribution is not its model choice or the phrase “AI
scanner.” It is a disciplined agent protocol: focal work, whole-repository
context, deduplication, falsifier construction, typed submission, continued
search, and verdict-before-fix ordering.

OpenAgents’ strongest contribution is the truth boundary Loupe lacks:
dependency-complete source, explicit incomplete states, finding/hypothesis
separation, evidence rungs, executed controls, prompt and model provenance,
brokered credentials, measured lifecycle, private disclosure, and durable
cleanup.

The combined product should feel as immediate as Loupe and be substantially
harder to fool than Loupe. The current Omega surface has the immediacy. The
OpenAgents contracts have much of the rigor. The remaining work is to connect
them without turning prompt claims into capabilities or checked-in contracts
into premature product claims.
